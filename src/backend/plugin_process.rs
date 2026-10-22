use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::mpsc;
use std::sync::{Arc, Mutex};

/// What a timeout or a cancel is allowed to signal.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum KillScope {
    /// The plugin and every helper it spawned: its process group.
    Group,
    /// Only the plugin process. The game it launched must survive.
    Process,
}

impl Default for KillScope {
    /// A helper that is not registered anywhere is a normal command, so its
    /// whole group goes down.
    fn default() -> Self {
        KillScope::Group
    }
}

/// `run` launches a game that has to outlive the plugin, so I never signal
/// its process group. Every other command can be cancelled, and its helper
/// children must go down with it.
pub fn kill_scope_for(args: &[&str]) -> KillScope {
    match args.first().copied() {
        Some("run") => KillScope::Process,
        _ => KillScope::Group,
    }
}

fn kill_scope_for_owned(args: &[String]) -> KillScope {
    match args.first().map(|s| s.as_str()) {
        Some("run") => KillScope::Process,
        _ => KillScope::Group,
    }
}

/// Signal every process of the group led by `pid`.
///
/// I only call this while the child is still alive (not reaped), so its pid
/// is still reserved and cannot have been reused: the group is still mine.
fn kill_group(pid: u32, sig: libc::c_int) {
    #[cfg(unix)]
    unsafe {
        // A negative pid means "every process in that group".
        let _ = libc::kill(-(pid as libc::pid_t), sig);
    }
    #[cfg(not(unix))]
    {
        let _ = (pid, sig);
    }
}

/// Handle to a streaming plugin child so a background batch (e.g. the
/// library description re-resolution) can be cancelled from the launcher.
/// Killing signals SIGTERM; the child self-exits when the launcher dies
/// via `--ppid`, so the handle is best-effort, not a lifecycle guard.
#[derive(Clone, Default)]
pub struct ProcessKiller {
    pid: Arc<Mutex<Option<u32>>>,
    scope: Arc<Mutex<KillScope>>,
}

impl ProcessKiller {
    pub fn new() -> Self {
        Self {
            pid: Arc::new(Mutex::new(None)),
            scope: Arc::new(Mutex::new(KillScope::Group)),
        }
    }

    /// PID of the managed child, if one is currently registered.
    pub fn current(&self) -> Option<u32> {
        *self.pid.lock().unwrap()
    }

    fn register(&self, child: &Child, scope: KillScope) {
        *self.scope.lock().unwrap() = scope;
        *self.pid.lock().unwrap() = Some(child.id());
    }

    fn clear(&self) {
        *self.pid.lock().unwrap() = None;
    }

    /// SIGTERM the managed child, if any; drops the registration.
    ///
    /// With `KillScope::Group` the whole group goes down, so a cancelled scan
    /// does not leave helper processes behind. `run` keeps `KillScope::Process`:
    /// the game it started must survive the plugin.
    pub fn kill(&self) {
        let pid = self.current();
        let scope = *self.scope.lock().unwrap();
        self.clear();
        if let Some(pid) = pid {
            #[cfg(unix)]
            unsafe {
                if scope == KillScope::Group {
                    kill_group(pid, libc::SIGTERM);
                }
                // The direct child may have left the group on its own
                // (setsid), so I signal it explicitly too.
                let _ = libc::kill(pid as libc::pid_t, libc::SIGTERM);
            }
            #[cfg(not(unix))]
            {
                let _ = (pid, scope);
            }
        }
    }
}

/// Variables a plugin may inherit. Everything else is dropped, especially
/// launcher tokens, `LD_PRELOAD`, `LD_LIBRARY_PATH`, `PYTHONPATH`,
/// `PYTHONHOME`, shell startup hooks and SSH/Git credentials.
const PLUGIN_ENV_ALLOWLIST: &[&str] = &[
    "PATH", "HOME", "USER", "LOGNAME", "SHELL", "LANG", "LANGUAGE", "TZ", "TERM",
    "COLORTERM", "TMPDIR", "TMP", "TEMP", "DISPLAY", "WAYLAND_DISPLAY",
    "WAYLAND_SOCKET", "XAUTHORITY", "DBUS_SESSION_BUS_ADDRESS", "XDG_RUNTIME_DIR",
    "PULSE_SERVER", "PULSE_COOKIE", "PIPEWIRE_RUNTIME_DIR", "PIPEWIRE_REMOTE",
    "PIPEWIRE_NODE", "JAVA_HOME", "PROTONPATH", "GAMEID", "MANGOHUD",
    "WINEDLLOVERRIDES", "WINEFSYNC", "WINEESYNC", "DESKTOP_SESSION",
    "XDG_CURRENT_DESKTOP", "SSL_CERT_FILE", "SSL_CERT_DIR",
    "REQUESTS_CA_BUNDLE", "CURL_CA_BUNDLE", "HTTP_PROXY", "HTTPS_PROXY",
    "NO_PROXY", "http_proxy", "https_proxy", "no_proxy", "ALL_PROXY",
    "all_proxy",
];

fn plugin_env_allowed(key: &str) -> bool {
    PLUGIN_ENV_ALLOWLIST.contains(&key)
        || key.starts_with("LC_")
        || key.starts_with("XDG_")
        || key.starts_with("STEAM_COMPAT_")
        || key.starts_with("PROTON_")
        || key.starts_with("UMU_")
        || key.starts_with("DXVK_")
        || key.starts_with("VKD3D_")
        || key.starts_with("SDL_")
        || key.starts_with("WINE")
}

/// Apply the plugin environment allowlist. Called immediately before every
/// plugin spawn so a plugin never sees the launcher's secrets or loader
/// injection variables.
pub fn apply_plugin_env(cmd: &mut Command) {
    cmd.env_clear();
    for (key, value) in std::env::vars_os() {
        if let Some(key) = key.to_str() {
            if plugin_env_allowed(key) {
                cmd.env(key, value);
            }
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
                .unwrap_or("unknown error")
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

/// Root of the plugin **executables**.
///
/// I keep two plugin roots apart because mixing them broke store logins:
///
/// * `~/.local/share/CorkyTux/plugins/<id>/` — this one. The script I
///   install and run. It is the only copy that executes.
/// * `~/.config/CorkyTux/plugins/<id>/` — the plugin's own `CONFIG_DIR`,
///   its mutable state: `bin/legendary`, `bin/gogdl`, `installs.json`,
///   `accounts.json`, locks, caches. I read those from `proton.rs`,
///   `minecraft_view.rs` and `external.rs`, and that is fine: data, not code.
///
/// I used to mix both roots and the plugin script lived in each, so a manual
/// edit landed on the dead copy while I kept running the old one. When I touch
/// plugin code I edit the plugin repo and reinstall; I never patch the copy.
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

/// Output of a process that finished within its time limit.
///
/// I shape this myself because `std::process::Output` cannot be built by hand
/// on stable; it exposes what my callers need: exit code and both streams.
#[derive(Debug, Clone)]
pub struct TimedOutput {
    pub success: bool,
    pub code: Option<i32>,
    pub stdout: String,
    pub stderr: String,
}

/// Grace window to drain pipes after the child exits. A grandchild holding
/// stdout open would block forever; without this margin the `join` would hang
/// my UI.
const PIPE_GRACE: std::time::Duration = std::time::Duration::from_secs(2);

/// One-shot plugin commands must always answer. Long installs and auth flows
/// get half an hour; scans get five minutes; launch/stop and ordinary queries
/// get two minutes. No plugin call is allowed to wait forever.
const DEFAULT_SINGLE_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(120);
const SCAN_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(300);
const LONG_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(1800);

fn single_timeout(args: &[&str]) -> std::time::Duration {
    match args.first().copied() {
        Some("install")
        | Some("setup")
        | Some("auth")
        | Some("ely-auth")
        | Some("java-install")
        | Some("umu-setup")
        | Some("uninstall")
        | Some("curse-auth-config") => LONG_TIMEOUT,
        Some("scan")
        | Some("diagnose")
        | Some("heroic-scan")
        | Some("curse-test")
        | Some("versions")
        | Some("mods-add")
        | Some("addons-install") => SCAN_TIMEOUT,
        // `run` waits for the plugin to launch, not for the game to exit.
        Some("run") | Some("stop") => std::time::Duration::from_secs(60),
        _ => DEFAULT_SINGLE_TIMEOUT,
    }
}

/// I join a pipe reader with a margin: on timeout I drop its result and let
/// the thread detach instead of blocking my caller.
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

/// I put every plugin child in its own process group (`setpgid(0, 0)` runs
/// in the child before `exec`). With that, a timeout or a cancel can signal
/// the plugin and every helper it spawned, instead of leaving orphans behind.
///
/// The child becomes the leader of the group, so the group id is its pid.
#[cfg(unix)]
fn own_process_group(cmd: &mut Command) {
    use std::os::unix::process::CommandExt;
    cmd.process_group(0);
}

#[cfg(not(unix))]
fn own_process_group(_cmd: &mut Command) {}

/// I run `cmd` capturing stdout/stderr and kill it past `limit`.
///
/// I wrote this to replace `timeout(1)`. That binary comes from coreutils, not
/// POSIX, and it is missing from the default NixOS PATH and minimal containers;
/// when absent each caller failed differently, and `list_emulators_in` even
/// returned an empty list with no warning.
///
/// I keep `cmd` on piped stdout/stderr even if the caller set null. The child
/// runs in its own process group (see `own_process_group`), so a timeout can
/// take the whole plugin tree down.
pub fn output_with_timeout(
    cmd: &mut Command,
    limit: std::time::Duration,
) -> Result<TimedOutput, String> {
    output_with_timeout_full(cmd, limit, KillScope::Group, None)
}

/// The one implementation: a kill scope and an optional stdin payload.
///
/// The scope says what a timeout is allowed to signal: the whole group of
/// the plugin, or only the plugin itself (`run` starts a game that has to
/// survive). The payload is written on the stdin of the child and the pipe
/// is closed right away, so the child sees the end of the input. This is
/// how a secret reaches a plugin: a password in the command line is visible
/// to every local user through `ps` and `/proc/<pid>/cmdline`.
pub fn output_with_timeout_full(
    cmd: &mut Command,
    limit: std::time::Duration,
    scope: KillScope,
    input: Option<&str>,
) -> Result<TimedOutput, String> {
    use std::io::Read;

    own_process_group(cmd);
    let mut child = cmd
        .stdin(if input.is_some() { Stdio::piped() } else { Stdio::null() })
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("Could not spawn process: {}", e))?;

    // The payload is small (a password): it fits in the pipe buffer, so
    // writing it here cannot block. I close the pipe right after, so the
    // child knows the input is complete.
    if let Some(payload) = input {
        if let Some(mut pipe) = child.stdin.take() {
            use std::io::Write;
            let _ = pipe.write_all(payload.as_bytes());
            let _ = pipe.write_all(b"\n");
            let _ = pipe.flush();
            // Dropping the pipe closes it.
        }
    }

    // I drain the streams on side threads: if the plugin fills the pipe
    // buffer (64 KiB) while I wait for its exit, both sides would deadlock.
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
        match scope {
            // The plugin and every helper it spawned go down together.
            KillScope::Group => {
                kill_group(child.id(), libc::SIGKILL);
                // The direct child may have left the group on its own.
                let _ = child.kill();
            }
            // Only the plugin: the game it launched has to survive.
            KillScope::Process => {
                let _ = child.kill();
            }
        }
        let _ = child.wait();
        // I join with a margin, not plain `join`: if the plugin left
        // grandchildren holding the pipe, a direct join would hang.
        if let Some(h) = out_handle.take() {
            let _ = join_reader(h);
        }
        if let Some(h) = err_handle.take() {
            let _ = join_reader(h);
        }
        return Err(format!(
            "process exceeded the {}s limit and was terminated",
            limit.as_secs()
        ));
    }

    let status = match status {
        Some(Ok(status)) => status,
        Some(Err(e)) => return Err(format!("process wait failed: {}", e)),
        None => return Err("process wait failed".to_string()),
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
    run_single_json_with_timeout(exe, args, single_timeout(args))
}

pub fn run_single_json_with_timeout(
    exe: &Path,
    args: &[&str],
    limit: std::time::Duration,
) -> Result<serde_json::Value, String> {
    run_single_json_inner(exe, args, limit, None)
}

/// Run a one-shot plugin command with a payload on its stdin.
///
/// The payload never appears in the command line: I use it for the Ely.by
/// password, which every local user could otherwise read with `ps` or from
/// `/proc/<pid>/cmdline`.
pub fn run_single_json_with_stdin(
    exe: &Path,
    args: &[&str],
    payload: &str,
) -> Result<serde_json::Value, String> {
    run_single_json_inner(exe, args, single_timeout(args), Some(payload))
}

fn run_single_json_inner(
    exe: &Path,
    args: &[&str],
    limit: std::time::Duration,
    input: Option<&str>,
) -> Result<serde_json::Value, String> {
    let mut cmd = Command::new(exe);
    cmd.args(args);
    apply_plugin_env(&mut cmd);
    let command = args.first().copied().unwrap_or("command");
    // `run` starts a game: only the plugin process may be signalled.
    let scope = kill_scope_for(args);
    let output = output_with_timeout_full(&mut cmd, limit, scope, input)
        .map_err(|e| format!("{} {} failed: {}", exe.display(), command, e))?;
    let body = output.stdout;
    if let Some(doc) = parse_final_doc(&body) {
        if doc.get("type").and_then(|t| t.as_str()) == Some("error") {
            return Err(doc.get("message").and_then(|m| m.as_str()).unwrap_or("plugin error").to_string());
        }
        if doc.get("ok").and_then(|o| o.as_bool()) == Some(false) {
            return Err(doc.get("message").and_then(|m| m.as_str()).unwrap_or("operation failed").to_string());
        }
        return Ok(doc);
    }
    if !output.success {
        let err = output.stderr.trim().to_string();
        let out = body.trim().to_string();
        let msg = if !err.is_empty() { err } else { out };
        return Err(if msg.is_empty() {
            format!("Plugin exited with code {:?}", output.code)
        } else if msg.len() > 300 {
            msg[..300].to_string()
        } else {
            msg
        });
    }
    Err("Plugin returned no JSON".into())
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
    let scope = kill_scope_for_owned(&args);
    std::thread::spawn(move || {
        let mut cmd = Command::new(&exe);
        cmd.args(&args)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        apply_plugin_env(&mut cmd);
        own_process_group(&mut cmd);
        let child = cmd.spawn();
        let mut child = match child {
            Ok(c) => {
                if let Some(k) = &killer {
                    k.register(&c, scope);
                }
                c
            }
            Err(e) => {
                let _ = tx.send(PluginEvent::Error {
                    message: format!("could not execute {}: {}", exe.display(), e),
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
                        format!("process exited with code {:?}", status.code())
                    } else {
                        format!("process exited with code {:?}: {}", status.code(), tail)
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
                    message: format!("process wait failed: {}", e),
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

#[cfg(test)]
mod tests {
    use super::*;

    /// The absolute path of a program, so the test does not depend on the
    /// process PATH: the isolation test breaks it on purpose, and the tests
    /// of this module run in parallel with it.
    fn program(name: &str) -> PathBuf {
        for dir in ["/bin", "/usr/bin", "/usr/local/bin"] {
            let candidate = Path::new(dir).join(name);
            if candidate.is_file() {
                return candidate;
            }
        }
        PathBuf::from(name)
    }

    #[test]
    fn one_shot_commands_have_bounded_deadlines() {
        assert_eq!(single_timeout(&["install"]), LONG_TIMEOUT);
        assert_eq!(single_timeout(&["scan", "/tmp"]), SCAN_TIMEOUT);
        assert_eq!(single_timeout(&["run", "/tmp/game"]), std::time::Duration::from_secs(60));
        assert_eq!(single_timeout(&["status"]), DEFAULT_SINGLE_TIMEOUT);
        assert!(LONG_TIMEOUT > SCAN_TIMEOUT);
        assert!(SCAN_TIMEOUT > DEFAULT_SINGLE_TIMEOUT);
    }

    #[test]
    fn run_single_json_terminates_a_hung_plugin() {
        // The lock keeps the isolation test from breaking the PATH while I
        // spawn a program by name.
        let _env = crate::test_env::lock();
        let started = std::time::Instant::now();
        let err = run_single_json_with_timeout(
            &program("sleep"),
            &["30"],
            std::time::Duration::from_millis(200),
        )
        .expect_err("sleep must be terminated");
        assert!(err.contains("terminated") || err.contains("limit"), "{}", err);
        assert!(
            started.elapsed() < std::time::Duration::from_secs(5),
            "timeout took too long: {:?}",
            started.elapsed()
        );
    }

    #[test]
    fn plugin_environment_policy_drops_secrets_and_loaders() {
        assert!(plugin_env_allowed("PATH"));
        assert!(plugin_env_allowed("HOME"));
        assert!(plugin_env_allowed("LC_ALL"));
        assert!(plugin_env_allowed("SSL_CERT_FILE"));
        assert!(plugin_env_allowed("REQUESTS_CA_BUNDLE"));
        assert!(plugin_env_allowed("HTTPS_PROXY"));
        assert!(!plugin_env_allowed("LD_PRELOAD"));
        assert!(!plugin_env_allowed("LD_LIBRARY_PATH"));
        assert!(!plugin_env_allowed("PYTHONPATH"));
        assert!(!plugin_env_allowed("CORKYTUX_API_KEY"));
        assert!(!plugin_env_allowed("SSH_AUTH_SOCK"));
    }

    /// The proxy variables are part of the allowlist: a plugin that talks
    /// HTTPS has to go through the same proxy as the launcher, and the
    /// `no_proxy` list decides what stays direct. Both spellings count,
    /// because the tools do not agree on the case.
    #[test]
    fn plugin_environment_keeps_the_proxy_variables() {
        for key in [
            "HTTP_PROXY",
            "HTTPS_PROXY",
            "NO_PROXY",
            "http_proxy",
            "https_proxy",
            "no_proxy",
            "ALL_PROXY",
            "all_proxy",
        ] {
            assert!(plugin_env_allowed(key), "{} must reach the plugin", key);
        }
    }

    #[test]
    fn plugin_environment_is_cleared_before_spawn() {
        std::env::set_var("CORKYTUX_TEST_SECRET", "leak");
        let mut cmd = Command::new(program("sh"));
        cmd.arg("-c")
            .arg("printf '%s' \"${CORKYTUX_TEST_SECRET-unset}\"");
        apply_plugin_env(&mut cmd);
        let output = cmd.output().expect("sh must run");
        std::env::remove_var("CORKYTUX_TEST_SECRET");
        assert_eq!(String::from_utf8_lossy(&output.stdout), "unset");
    }

    /// A fake plugin: it starts a helper process and then hangs, so the
    /// launcher has to time it out.
    #[cfg(unix)]
    fn fake_plugin(dir: &Path, pidfile: &Path) -> PathBuf {
        use std::os::unix::fs::PermissionsExt;
        let script = dir.join("fake-plugin");
        let body = format!(
            "#!/bin/sh\nsleep 30 &\necho $! > {}\nsleep 30\n",
            pidfile.display()
        );
        std::fs::write(&script, body).unwrap();
        let mut perms = std::fs::metadata(&script).unwrap().permissions();
        perms.set_mode(0o755);
        std::fs::set_permissions(&script, perms).unwrap();
        script
    }

    /// The pid the fake plugin wrote for its helper.
    #[cfg(unix)]
    fn wait_for_pid(pidfile: &Path) -> i32 {
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        while std::time::Instant::now() < deadline {
            if let Ok(text) = std::fs::read_to_string(pidfile) {
                if let Ok(pid) = text.trim().parse::<i32>() {
                    return pid;
                }
            }
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
        panic!("the fake plugin never wrote {}", pidfile.display());
    }

    /// A zombie is dead: it only waits for its parent to reap it.
    #[cfg(unix)]
    fn process_alive(pid: i32) -> bool {
        let stat = std::path::PathBuf::from(format!("/proc/{}/stat", pid));
        match std::fs::read_to_string(&stat) {
            Ok(text) => {
                let after = match (text.find('('), text.rfind(')')) {
                    (Some(a), Some(b)) if b > a => &text[b + 1..],
                    _ => return true,
                };
                let state = after.split_whitespace().next().unwrap_or("");
                state != "Z" && state != "X"
            }
            Err(_) => false,
        }
    }

    #[cfg(unix)]
    #[test]
    fn timeout_kills_the_plugin_group() {
        // The lock keeps the isolation test from breaking the PATH while the
        // script of this test calls `sleep` by name.
        let _env = crate::test_env::lock();
        let dir = std::env::temp_dir().join(format!("corky-group-kill-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let pidfile = dir.join("child.pid");
        let script = fake_plugin(&dir, &pidfile);

        let err = run_single_json_with_timeout(
            &script,
            &["scan", "/tmp"],
            std::time::Duration::from_millis(400),
        )
        .expect_err("the plugin must time out");
        assert!(err.contains("terminated") || err.contains("limit"), "{}", err);

        let pid = wait_for_pid(&pidfile);
        assert!(
            !process_alive(pid),
            "the helper of the plugin must die with the plugin group"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[cfg(unix)]
    #[test]
    fn run_timeout_keeps_the_launched_process_alive() {
        let dir = std::env::temp_dir().join(format!("corky-group-run-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let pidfile = dir.join("child.pid");
        let script = fake_plugin(&dir, &pidfile);

        // `run` launches a game: the timeout may only take the plugin down.
        let err = run_single_json_with_timeout(
            &script,
            &["run", "/tmp/game"],
            std::time::Duration::from_millis(400),
        )
        .expect_err("the plugin must time out");
        assert!(err.contains("terminated") || err.contains("limit"), "{}", err);

        let pid = wait_for_pid(&pidfile);
        assert!(
            process_alive(pid),
            "the process launched by `run` must survive the plugin timeout"
        );
        unsafe {
            libc::kill(pid, libc::SIGKILL);
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// The two halves together: the stdin arrives complete and closed, and
    /// the timeout takes the whole plugin group down.
    #[cfg(unix)]
    #[test]
    fn the_stdin_arrives_and_the_timeout_kills_the_group() {
        let dir = std::env::temp_dir().join(format!("corky-stdin-group-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let stdin_file = dir.join("stdin.txt");
        let pidfile = dir.join("child.pid");
        let script = dir.join("fake-plugin");
        {
            use std::os::unix::fs::PermissionsExt;
            let body = format!(
                "#!/bin/sh\ncat > {}\nsleep 30 &\necho $! > {}\nsleep 30\n",
                stdin_file.display(),
                pidfile.display()
            );
            std::fs::write(&script, body).unwrap();
            let mut perms = std::fs::metadata(&script).unwrap().permissions();
            perms.set_mode(0o755);
            std::fs::set_permissions(&script, perms).unwrap();
        }

        let mut cmd = Command::new(&script);
        cmd.args(["scan", "/tmp"]);
        apply_plugin_env(&mut cmd);
        let scope = kill_scope_for(&["scan", "/tmp"]);
        let err = output_with_timeout_full(
            &mut cmd,
            std::time::Duration::from_millis(400),
            scope,
            Some("s3cret"),
        )
        .expect_err("the plugin must time out");
        assert!(err.contains("terminated") || err.contains("limit"), "{}", err);

        // The stdin arrived complete and the pipe was closed.
        let got = std::fs::read_to_string(&stdin_file).unwrap_or_default();
        assert_eq!(got, "s3cret\n", "the payload must arrive whole");

        // The helper of the plugin died with the group.
        let pid = wait_for_pid(&pidfile);
        assert!(!process_alive(pid), "the helper must die with the group");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[cfg(unix)]
    #[test]
    fn plugin_children_lead_their_own_process_group() {
        // The lock keeps the isolation test from breaking the PATH while I
        // spawn a program by name.
        let _env = crate::test_env::lock();
        let mut cmd = Command::new(program("sh"));
        cmd.arg("-c").arg("sleep 5");
        own_process_group(&mut cmd);
        let mut child = cmd.spawn().expect("spawn");
        let pgid = unsafe { libc::getpgid(child.id() as libc::pid_t) };
        assert_eq!(
            pgid,
            child.id() as libc::pid_t,
            "the plugin must lead its own process group"
        );
        let _ = child.kill();
        let _ = child.wait();
    }

    #[cfg(unix)]
    #[test]
    fn process_killer_uses_the_syscall_directly() {
        let mut child = Command::new(program("sleep"))
            .arg("30")
            .spawn()
            .expect("sleep");
        let killer = ProcessKiller::new();
        killer.register(&child, KillScope::Process);
        killer.kill();
        let status = child.wait().expect("wait");
        assert!(!status.success());
        assert!(status.code().is_none(), "expected signal death, got {:?}", status.code());
    }
}
