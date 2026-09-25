use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::mpsc;
use std::sync::{Arc, Mutex};

/// Handle to a streaming plugin child so a background batch (e.g. the
/// library description re-resolution) can be cancelled from the launcher.
/// Killing signals SIGTERM; the child self-exits when the launcher dies
/// via `--ppid`, so the handle is best-effort, not a lifecycle guard.
#[derive(Clone, Default)]
pub struct ProcessKiller {
    pid: Arc<Mutex<Option<u32>>>,
}

impl ProcessKiller {
    pub fn new() -> Self {
        Self { pid: Arc::new(Mutex::new(None)) }
    }

    /// PID of the managed child, if one is currently registered.
    pub fn current(&self) -> Option<u32> {
        *self.pid.lock().unwrap()
    }

    fn register(&self, child: &Child) {
        *self.pid.lock().unwrap() = Some(child.id());
    }

    fn clear(&self) {
        *self.pid.lock().unwrap() = None;
    }

    /// SIGTERM the managed child, if any; drops the registration.
    pub fn kill(&self) {
        let pid = self.current();
        self.clear();
        if let Some(pid) = pid {
            let _ = Command::new("kill").args(["-TERM", &pid.to_string()]).status();
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ProtocolMode {
    SingleJson,
    JsonLines,
}

#[derive(Debug, Clone)]
pub enum PluginEvent {
    Progress { stage: Option<String>, percent: Option<f64>, extra: serde_json::Value },
    Done(serde_json::Value),
    Error { message: String, exit_code: Option<i32>, code: Option<i32> },
    Custom(serde_json::Value),
}

pub fn classify(value: serde_json::Value) -> PluginEvent {
    match value.get("type").and_then(|t| t.as_str()) {
        Some("progress") => PluginEvent::Progress {
            stage: value.get("stage").and_then(|v| v.as_str()).map(str::to_string),
            percent: value.get("percent").and_then(|v| v.as_f64()),
            extra: value,
        },
        Some("done") => PluginEvent::Done(value),
        Some("error") => PluginEvent::Error {
            message: value
                .get("message")
                .and_then(|v| v.as_str())
                .unwrap_or("error desconocido")
                .to_string(),
            exit_code: None,
            // Set by the plugin's die(): lets the UI differentiate error
            // kinds (e.g. skip the "(codes expire fast…)" suffix when the
            // binary is missing or there is no network).
            code: value.get("code").and_then(|v| v.as_i64()).map(|c| c as i32),
        },
        _ => {
            if value.get("ok").is_some() {
                PluginEvent::Done(value)
            } else {
                PluginEvent::Custom(value)
            }
        }
    }
}

pub fn plugins_base_dir() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".into());
    PathBuf::from(home).join(".local").join("share").join("CorkyTux").join("plugins")
}

pub fn plugin_exe(plugin_id: &str, entry: &str) -> PathBuf {
    plugins_base_dir().join(plugin_id).join(entry)
}

pub fn plugin_available(plugin_id: &str, entry: &str) -> bool {
    let exe = plugin_exe(plugin_id, entry);
    exe.exists()
}

/// Salida de un proceso terminado dentro de su limite de tiempo.
///
/// `std::process::Output` no se puede construir a mano en stable, asi que este
/// tipo expone lo que los llamadores necesitan: codigo de salida y los dos
/// streams.
#[derive(Debug, Clone)]
pub struct TimedOutput {
    pub success: bool,
    pub code: Option<i32>,
    pub stdout: String,
    pub stderr: String,
}

/// Margen para vaciar los pipes tras salir el hijo. Un nieto que herede el
/// stdout lo mantendria abierto para siempre; sin este margen, el `join`
/// colgaria la interfaz.
const PIPE_GRACE: std::time::Duration = std::time::Duration::from_secs(2);

/// Une un lector de pipe con margen: si no termina a tiempo se cede su
/// resultado y el hilo se queda desprendido en vez de bloquear al llamador.
fn join_reader(h: std::thread::JoinHandle<String>) -> String {
    let deadline = std::time::Instant::now() + PIPE_GRACE;
    while !h.is_finished() {
        if std::time::Instant::now() >= deadline {
            return String::new();
        }
        std::thread::sleep(std::time::Duration::from_millis(25));
    }
    h.join().unwrap_or_default()
}

/// Ejecuta `cmd` capturando stdout y stderr, y lo mata si pasa de `limit`.
///
/// Sustituye a `timeout(1)`. Ese binario viene de coreutils, no de POSIX, y no
/// existe en el PATH por defecto de NixOS ni en contenedores minimos; cuando
/// faltaba, cada llamador caia en una rama distinta: la mayoria mostraba un
/// error que no mencionaba la causa, pero `list_emulators_in` devolvia una
/// lista vacia sin avisar.
///
/// `cmd` se queda con stdout y stderr en pipe aunque el llamador los hubiera
/// puesto a null. Misma semantica de muerte que `timeout(1)` sin
/// `--foreground`: solo se senala al hijo directo, no al grupo de procesos.
pub fn output_with_timeout(
    cmd: &mut Command,
    limit: std::time::Duration,
) -> Result<TimedOutput, String> {
    use std::io::Read;

    let mut child = cmd
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("No se pudo ejecutar el proceso: {}", e))?;

    // Los streams se leen en hilos aparte: si el plugin llena el buffer del pipe
    // (64 KiB) mientras esperamos su salida, ambos lados se bloquearian.
    let mut out_handle = child.stdout.take().map(|mut s| {
        std::thread::spawn(move || {
            let mut buf = Vec::new();
            let _ = s.read_to_end(&mut buf);
            String::from_utf8_lossy(&buf).to_string()
        })
    });
    let mut err_handle = child.stderr.take().map(|mut s| {
        std::thread::spawn(move || {
            let mut buf = Vec::new();
            let _ = s.read_to_end(&mut buf);
            String::from_utf8_lossy(&buf).to_string()
        })
    });

    let deadline = std::time::Instant::now() + limit;
    let mut timed_out = false;
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break Some(Ok(status)),
            Ok(None) => {}
            Err(e) => break Some(Err(e)),
        }
        if std::time::Instant::now() >= deadline {
            timed_out = true;
            break None;
        }
        std::thread::sleep(std::time::Duration::from_millis(25));
    };

    if timed_out {
        let _ = child.kill();
        let _ = child.wait();
        // Con margen, no con `join` a secas: si el plugin dejo nietos con el
        // pipe abierto, el join directo colgaria en lugar de devolver el error.
        if let Some(h) = out_handle.take() {
            let _ = join_reader(h);
        }
        if let Some(h) = err_handle.take() {
            let _ = join_reader(h);
        }
        return Err(format!(
            "el proceso excedio el limite de {} s y fue terminado",
            limit.as_secs()
        ));
    }

    let status = match status {
        Some(Ok(status)) => status,
        Some(Err(e)) => return Err(format!("espera del proceso falló: {}", e)),
        None => return Err("espera del proceso falló".to_string()),
    };
    let stdout = out_handle.map(join_reader).unwrap_or_default();
    let stderr = err_handle.map(join_reader).unwrap_or_default();
    Ok(TimedOutput {
        success: status.success(),
        code: status.code(),
        stdout,
        stderr,
    })
}

fn parse_final_doc(body: &str) -> Option<serde_json::Value> {
    let mut last: Option<serde_json::Value> = None;
    let mut final_doc: Option<serde_json::Value> = None;
    for line in body.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        if let Ok(v) = serde_json::from_str::<serde_json::Value>(line) {
            if v.is_object() {
                if v.get("ok").is_some() || v.get("type").is_some() {
                    final_doc = Some(v);
                } else {
                    last = Some(v);
                }
            }
        }
    }
    final_doc.or(last)
}

pub fn run_single_json(exe: &Path, args: &[&str]) -> Result<serde_json::Value, String> {
    let output = Command::new(exe)
        .args(args)
        .stdin(Stdio::null())
        .output()
        .map_err(|e| format!("No se pudo ejecutar {}: {}", exe.display(), e))?;
    let body = String::from_utf8_lossy(&output.stdout).to_string();
    if let Some(doc) = parse_final_doc(&body) {
        if doc.get("type").and_then(|t| t.as_str()) == Some("error") {
            return Err(doc.get("message").and_then(|m| m.as_str()).unwrap_or("error del plugin").to_string());
        }
        if doc.get("ok").and_then(|o| o.as_bool()) == Some(false) {
            return Err(doc.get("message").and_then(|m| m.as_str()).unwrap_or("operación fallida").to_string());
        }
        return Ok(doc);
    }
    if !output.status.success() {
        let err = String::from_utf8_lossy(&output.stderr).trim().to_string();
        let out = body.trim().to_string();
        let msg = if !err.is_empty() { err } else { out };
        return Err(if msg.is_empty() {
            format!("El plugin salió con código {:?}", output.status.code())
        } else if msg.len() > 300 {
            msg[..300].to_string()
        } else {
            msg
        });
    }
    Err("El plugin no devolvió JSON".into())
}

pub fn spawn_streaming(exe: PathBuf, args: Vec<String>) -> mpsc::Receiver<PluginEvent> {
    spawn_streaming_inner(exe, args, None)
}

/// Streaming variant that registers the child in a `ProcessKiller` so a
/// background batch can be cancelled with SIGTERM. Returned alongside the
/// event channel.
pub fn spawn_streaming_managed(exe: PathBuf, args: Vec<String>, killer: &ProcessKiller) -> mpsc::Receiver<PluginEvent> {
    spawn_streaming_inner(exe, args, Some(killer.clone()))
}

fn spawn_streaming_inner(
    exe: PathBuf,
    args: Vec<String>,
    killer: Option<ProcessKiller>,
) -> mpsc::Receiver<PluginEvent> {
    let (tx, rx) = mpsc::channel::<PluginEvent>();
    std::thread::spawn(move || {
        let child = Command::new(&exe)
            .args(&args)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn();
        let mut child = match child {
            Ok(c) => {
                if let Some(k) = &killer {
                    k.register(&c);
                }
                c
            }
            Err(e) => {
                let _ = tx.send(PluginEvent::Error {
                    message: format!("no se pudo ejecutar {}: {}", exe.display(), e),
                    exit_code: None,
                    code: None,
                });
                return;
            }
        };
        let mut saw_error = false;
        let mut stderr_text = String::new();
        let stderr_handle = child.stderr.take().map(|mut stderr| {
            std::thread::spawn(move || {
                use std::io::Read;
                let mut buf = String::new();
                let _ = stderr.read_to_string(&mut buf);
                buf
            })
        });
        if let Some(stdout) = child.stdout.take() {
            let reader = BufReader::new(stdout);
            for line in reader.lines().flatten() {
                let line = line.trim().to_string();
                if line.is_empty() {
                    continue;
                }
                let value: serde_json::Value = match serde_json::from_str(&line) {
                    Ok(v) => v,
                    Err(_) => continue,
                };
                let event = classify(value);
                if matches!(event, PluginEvent::Error { .. }) {
                    saw_error = true;
                }
                if tx.send(event).is_err() {
                    break;
                }
            }
        }
        match child.wait() {
            Ok(status) => {
                if let Some(k) = &killer {
                    k.clear();
                }
                if let Some(handle) = stderr_handle {
                    stderr_text = handle.join().unwrap_or_default();
                }
                if !status.success() && !saw_error {
                    let tail = stderr_tail(&stderr_text);
                    let message = if tail.is_empty() {
                        format!("el proceso terminó con código {:?}", status.code())
                    } else {
                        format!("el proceso terminó con código {:?}: {}", status.code(), tail)
                    };
                    let _ = tx.send(PluginEvent::Error {
                        message,
                        exit_code: status.code(),
                        code: None,
                    });
                }
            }
            Err(e) => {
                if let Some(k) = &killer {
                    k.clear();
                }
                let _ = tx.send(PluginEvent::Error {
                    message: format!("espera del proceso falló: {}", e),
                    exit_code: None,
                    code: None,
                });
            }
        }
    });
    rx
}

fn stderr_tail(text: &str) -> String {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return String::new();
    }
    let mut lines: Vec<&str> = trimmed.lines().collect();
    // Python tracebacks: keep the final exception line, it names the cause.
    if let Some(last) = lines.iter().rfind(|l| {
        let l = l.trim();
        l.starts_with("minecraft_launcher_lib.") || l.contains("Error") || l.contains("Exception")
    }) {
        return last.trim().chars().take(400).collect();
    }
    if lines.len() > 3 {
        lines = lines[lines.len() - 3..].to_vec();
    }
    lines.join(" | ").chars().take(400).collect()
}

pub fn pump_to_idle<F>(rx: mpsc::Receiver<PluginEvent>, mut on_event: F)
where
    F: FnMut(PluginEvent) -> bool + 'static,
{
    // 50ms poller, NOT idle_add: a bare idle re-dispatches in a tight
    // loop and pins a CPU core at 100% while a worker runs (e.g. a slow
    // `legendary list` froze the whole launcher at startup).
    glib::timeout_add_local(std::time::Duration::from_millis(50), move || match rx.try_recv() {
        Ok(ev) => {
            let done = !on_event(ev);
            if done {
                glib::ControlFlow::Break
            } else {
                glib::ControlFlow::Continue
            }
        }
        Err(mpsc::TryRecvError::Empty) => glib::ControlFlow::Continue,
        Err(_) => glib::ControlFlow::Break,
    });
}

/// Same 50ms cadence for one-shot channel polling (replaces ad-hoc
/// idle_add_local busy loops that also spin the main loop hot).
pub fn poll_once_local<T, F>(rx: mpsc::Receiver<T>, mut on_msg: F)
where
    T: Send + 'static,
    F: FnMut(Result<T, mpsc::TryRecvError>) -> glib::ControlFlow + 'static,
{
    glib::timeout_add_local(std::time::Duration::from_millis(50), move || {
        on_msg(rx.try_recv())
    });
}
