use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::mpsc;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ProtocolMode {
    SingleJson,
    JsonLines,
}

#[derive(Debug, Clone)]
pub enum PluginEvent {
    Progress { stage: Option<String>, percent: Option<f64>, extra: serde_json::Value },
    Done(serde_json::Value),
    Error { message: String, exit_code: Option<i32> },
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
    let (tx, rx) = mpsc::channel::<PluginEvent>();
    std::thread::spawn(move || {
        let child = Command::new(&exe)
            .args(&args)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn();
        let mut child = match child {
            Ok(c) => c,
            Err(e) => {
                let _ = tx.send(PluginEvent::Error {
                    message: format!("no se pudo ejecutar {}: {}", exe.display(), e),
                    exit_code: None,
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
                    });
                }
            }
            Err(e) => {
                let _ = tx.send(PluginEvent::Error {
                    message: format!("espera del proceso falló: {}", e),
                    exit_code: None,
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
