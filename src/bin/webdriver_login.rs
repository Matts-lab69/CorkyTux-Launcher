//! Epic/GOG login with a real Chromium, driven over CDP.
//!
//! ## Why Chromium and not Firefox
//!
//! I started on Firefox through geckodriver and it **does not work**: Epic's
//! hCaptcha rejects the challenge even when it solves correctly, because any
//! WebDriver-driven Firefox exposes `navigator.webdriver === true`, which is an
//! automation signal. I measured that there's no way to turn it off:
//!
//! | path                                | `navigator.webdriver` |
//! |-------------------------------------|-----------------------|
//! | geckodriver / Marionette            | `true`                |
//! | `dom.webdriver.enabled=false`       | `true`                |
//! | the same pref in `user.js`          | `true`                |
//! | WebDriver BiDi (remote agent)       | `true`                |
//!
//! Marionette forces the flag from C++ (`dom/base/WebDriver.cpp`), so it isn't
//! a pref: no setting removes it.
//!
//! Chromium doesn't have that problem **when launched by hand**.
//! `navigator.webdriver` only turns on with `--enable-automation` or headless.
//! Launching it with just `--remote-debugging-port` and connecting over CDP
//! afterwards measures `false`, and the page is still readable. That's the mode
//! this binary runs in.
//!
//! Worth noting **what this helper doesn't do**: it doesn't click buttons, it
//! doesn't type into fields and it doesn't submit the form. The person types
//! their password and solves the challenge by hand, in a real window. The
//! helper only *reads* the URL and the page body. So no evasion flag is needed
//! (nor included): the browser isn't automated anywhere in the flow hCaptcha
//! evaluates.
//!
//! ## Why Epic forces reading the body
//!
//! Epic's redirect endpoint returns the code in the **body** of the response,
//! and the URL never changes:
//!
//! ```text
//! {"redirectUrl":"https://localhost/launcher/authorized",
//!  "authorizationCode":null,"exchangeCode":null,"sid":null}
//! ```
//!
//! Watching the URL or the profile history is therefore useless: for Epic I
//! have to read the page. GOG returns the code in the query instead, and both
//! cases are covered because I read both.
//!
//! ## Decisions
//!
//! * **No async runtime.** CDP is a WebSocket and I speak it by hand over
//!   `std::net::TcpStream`. That keeps the helper synchronous like the rest of
//!   the project and avoids dragging in tokio/hyper/hyper-util, which is what
//!   forced WebDriver on me in the first place.
//! * **Ephemeral profile per attempt.** Deleted on exit, whatever happens.
//!   Store cookies never mix with the user's own browsing.
//! * **Its own process group.** Chromium starts in its own group, so the guard
//!   can kill all of it without ever touching the user's browser.
//! * **Two-step shutdown** in the drop guard: `Browser.close` and then the
//!   `SIGKILL` to the group, so no exit path leaves orphans.

use std::io::{Read, Write};
use std::net::TcpStream;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use regex::Regex;
use serde_json::{json, Value};

// ─── CONFIG ──────────────────────────────────────────────────────────────────

/// Chrome for Testing: Google's own binaries, published for exactly this. I
/// prefer them over a system Chrome because the version number matters: both
/// the folder layout and the protocol shift between versions.
const CHROME_VERSION: &str = "154.0.8037.57";
const CHROME_URL: &str = "https://storage.googleapis.com/chrome-for-testing-public/154.0.8037.57/linux64/chrome-linux64.zip";
/// Informational only: Chrome for Testing ships under Google's terms, not a
/// project license. See `assets/icons/ATTRIBUTION.md`.
const CHROME_LICENSE: &str = "Google terms (Chrome for Testing)";

/// Ceiling for the login. Epic's code expires in ~1 minute, so 180 s is plenty
/// to type the credentials and solve a challenge by hand.
const LOGIN_TIMEOUT: Duration = Duration::from_secs(180);
/// How often I poll the page.
const POLL: Duration = Duration::from_millis(600);

/// Failure reasons, so the launcher never has to guess.
mod why {
    pub const CHROME: &str = "chrome";
    pub const TIMEOUT: &str = "timeout";
    pub const SESSION: &str = "session";
    pub const NAV: &str = "nav";
    pub const CDP: &str = "cdp_lost";
    pub const CANCELLED: &str = "cancelled";
    pub const USAGE: &str = "usage";
}

type Failure = (&'static str, String);

fn main() {
    install_stop_handlers();
    let args: Vec<String> = std::env::args().collect();
    match args.get(1).map(|s| s.as_str()) {
        // Filesystem only: I report whether Chromium is already cached, without
        // launching any Chrome process or touching the network. The deps modal
        // uses it to decide whether to bother you at all.
        Some("--probe") => {
            println!("{}", sonda_chrome());
        }
        // I download Chromium if it's missing, with JSON progress on stdout for
        // the deps modal. Same path the login uses.
        Some("--prefetch") => match ensure_chrome_con(&|pct, hecho| {
            println!(
                "{}",
                json!({
                    "type": "chrome_progress",
                    "percent": pct,
                    "mb": hecho / 1048576,
                })
            );
        }) {
            Ok(p) => println!(
                "{}",
                json!({"type": "chrome_done", "path": p.display().to_string()})
            ),
            Err(e) => fail(why::CHROME, &e),
        },
        Some(url) => {
            if url.trim().is_empty() {
                fail(why::USAGE, "the authentication URL is empty");
            }
            match run(url.trim()) {
                Ok(code) => println!("{}", code),
                Err((reason, msg)) => fail(reason, &msg),
            }
        }
        None => fail(why::USAGE, "missing the authentication URL"),
    }
}

fn fail(reason: &str, msg: &str) -> ! {
    eprintln!("ERRO:{}:{}", reason, msg);
    std::process::exit(1);
}

// ─── CHROMIUM ────────────────────────────────────────────────────────────────

fn base_dir() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".into());
    PathBuf::from(home).join(".local/share/corkytux")
}

fn chrome_dir() -> PathBuf {
    base_dir().join(format!("chrome-{}", CHROME_VERSION))
}

fn chrome_bin() -> PathBuf {
    chrome_dir().join("chrome-linux64/chrome")
}

/// Chromium cache report for the deps modal.
///
/// Filesystem I/O only (`is_file` on the cached path): I launch no Chrome
/// process and touch no network. `cached:false` means the first login (or a
/// `--prefetch`) will download ~188 MB.
fn sonda_chrome() -> Value {
    let bin = chrome_bin();
    json!({
        "type": "chrome_probe",
        "cached": bin.is_file(),
        "version": CHROME_VERSION,
        "path": bin.display().to_string(),
    })
}

/// I download the Chromium zip, with retries and a progress bar.
///
/// 188 MB don't arrive in one go on normal networks: the first time I tried,
/// the connection cut out halfway and the error that came back was the generic
/// decode one, which says nothing useful. So I retry with a growing wait and,
/// if it still fails, the message tells you what to do.
///
/// I write to a temp file and rename it at the end: a half zip can never be
/// left where the code expects the final one.
///
/// `progreso` gets `(percent, bytes)` every ~10 %: the login shows it as human
/// text, `--prefetch` emits it as JSON for the modal.
fn download_zip(progreso: &dyn Fn(u8, u64)) -> Result<std::path::PathBuf, String> {
    /// Wait before each retry, in seconds.
    const BACKOFF: [u64; 4] = [0, 3, 10, 25];
    const ESPERADO: u64 = 188 * 1024 * 1024;

    let dir = base_dir();
    let _ = std::fs::create_dir_all(&dir);
    let final_zip = dir.join("chrome-descarga.zip");
    let parcial = dir.join("chrome-descarga.parcial");

    let mut ultimo_error = String::new();
    for (intento, espera) in BACKOFF.iter().enumerate() {
        if *espera > 0 {
            eprintln!(
                "CHROME:retry {} of {} in {} s (previous reason: {})",
                intento,
                BACKOFF.len() - 1,
                espera,
                ultimo_error
            );
            std::thread::sleep(Duration::from_secs(*espera));
        }

        match intentar_descarga(&parcial, ESPERADO, progreso) {
            Ok(()) => {
                // The rename is atomic inside the same filesystem: either the
                // whole zip is there or nothing is.
                let _ = std::fs::remove_file(&final_zip);
                std::fs::rename(&parcial, &final_zip).map_err(|e| {
                    format!("could not finalize the download: {}", e)
                })?;
                return Ok(final_zip);
            }
            Err(e) => {
                ultimo_error = e;
                let _ = std::fs::remove_file(&parcial);
            }
        }
    }

    Err(format!(
        "the download was cut off or failed after {} attempts. This is a 188 MB \
         download: an unstable network or no disk space cuts it halfway. Try again \
         with Log in; if it keeps failing, free up space or check your connection. \
         Last attempt detail: {}",
        BACKOFF.len(),
        ultimo_error
    ))
}

/// One download attempt, writing to `destino` and reporting progress.
fn intentar_descarga(
    destino: &Path,
    esperado: u64,
    progreso: &dyn Fn(u8, u64),
) -> Result<(), String> {
    descargar_de(CHROME_URL, destino, esperado, progreso)
}

/// I download `url` into `destino` with the same protocol (the caller handles
/// retries). I split it from `intentar_descarga` so I can test it against a
/// local server without touching the real network.
fn descargar_de(
    url: &str,
    destino: &Path,
    esperado: u64,
    progreso: &dyn Fn(u8, u64),
) -> Result<(), String> {
    use std::io::Read as _;

    let mut resp = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(900))
        .build()
        .map_err(|e| format!("could not prepare the connection: {}", e))?
        .get(url)
        .send()
        .map_err(|e| format!("network failure while connecting: {}", e))?
        .error_for_status()
        .map_err(|e| format!("the server rejected the download: {}", e))?;

    // If the server says how big it is, I use that; if not, I estimate from the
    // size actually downloaded so far.
    let total = resp
        .content_length()
        .filter(|n| *n > 0)
        .unwrap_or(esperado);

    let f = std::fs::File::create(destino).map_err(|e| format!("could not create the temp file: {}", e))?;
    let mut w = std::io::BufWriter::new(f);
    let mut buf = vec![0u8; 64 * 1024];
    let mut hecho: u64 = 0;
    let mut ultimo_pct = 0u8;

    loop {
        let n = resp
            .read(&mut buf)
            .map_err(|e| format!("the connection was cut halfway ({} of {} bytes): {}", hecho, total, e))?;
        if n == 0 {
            break;
        }
        std::io::Write::write_all(&mut w, &buf[..n])
            .map_err(|e| format!("could not write to disk: {}", e))?;
        hecho += n as u64;

        let pct = ((hecho as f64 / total as f64) * 100.0) as u8;
        if pct >= ultimo_pct + 10 {
            ultimo_pct = pct;
            progreso(pct, hecho);
        }
    }
    std::io::Write::flush(&mut w).map_err(|e| format!("could not flush to disk: {}", e))?;

    // A truncated body sometimes closes without an error, so I check the size
    // against what the server declared. A zip that doesn't reach the end won't
    // unzip.
    if esperado > 0 && hecho + 1024 * 1024 < esperado {
        return Err(format!(
            "got {} of {} bytes: the connection was cut short",
            hecho, total
        ));
    }
    Ok(())
}

/// I return the path to Chromium, downloading it the first time.
///
/// I prefer the pinned cache (a version I know and have tested); a system
/// Chrome in PATH is the fallback so I don't download 188 MB when the cache is
/// missing. The download is there so I never depend on root.
///
/// `progreso` gets `(percent, bytes)` every ~10 % of the download.
fn ensure_chrome() -> Result<PathBuf, String> {
    ensure_chrome_con(&|pct, hecho| {
        eprintln!(
            "CHROME:descargando {}% ({:.0} MB)",
            pct,
            hecho as f64 / 1048576.0
        );
    })
}

fn ensure_chrome_con(progreso: &dyn Fn(u8, u64)) -> Result<PathBuf, String> {
    if let Ok(custom) = std::env::var("CORKYTUX_CHROME") {
        let p = PathBuf::from(custom);
        if p.is_file() {
            return Ok(p);
        }
        return Err(format!("CORKYTUX_CHROME no apunta a un archivo: {}", p.display()));
    }
    // The pinned cache goes first: it's the version I know and tested against
    // hCaptcha, and most people (on Firefox) have nothing usable in PATH
    // anyway. PATH stays as the fallback so I don't download 188 MB when a
    // system Chrome exists and the cache is missing.
    let cached = chrome_bin();
    if cached.is_file() {
        return Ok(cached);
    }
    if let Ok(px) = std::env::var("PATH") {
        for dir in std::env::split_paths(&px) {
            let p = dir.join("google-chrome");
            if p.is_file() {
                return Ok(p);
            }
            for alt in ["chromium", "chrome"] {
                let q = dir.join(alt);
                if q.is_file() {
                    return Ok(q);
                }
            }
        }
    }

    let dir = chrome_dir();
    eprintln!(
        "CHROME:primera vez, descargando Chrome for Testing {} (188 MB, {}). \
         Tarda unos minutos; se informa cada 10% para que se vea que avanza.",
        CHROME_VERSION, CHROME_LICENSE
    );

    let zip = download_zip(progreso)?;

    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).map_err(|e| format!("could not create {}: {}", dir.display(), e))?;
    unzip(&zip, &dir)?;
    let _ = std::fs::remove_file(&zip);

    if !cached.is_file() {
        return Err("the zip had no chrome-linux64/chrome".into());
    }
    // The zip keeps the permissions, but just in case:
    make_exec(&cached)?;
    Ok(cached)
}

/// I extract a zip. I walk it by hand so no loose files are left in the
/// destination folder if something goes wrong.
fn unzip(zip_path: &Path, dest: &Path) -> Result<(), String> {
    let f = std::fs::File::open(zip_path).map_err(|e| e.to_string())?;
    let mut ar = zip::ZipArchive::new(f).map_err(|e| format!("zip ilegible: {}", e))?;
    for i in 0..ar.len() {
        let mut entry = match ar.by_index(i) {
            Ok(e) => e,
            Err(e) => return Err(format!("corrupt zip entry {}: {}", i, e)),
        };
        // `enclosed_name` drops paths with `..`, which shouldn't exist in an
        // external zip, but checking costs nothing.
        let Some(rel) = entry.enclosed_name() else {
            return Err(format!("the zip contains an unsafe path in entry {}", i));
        };
        let out = dest.join(&rel);
        if entry.is_dir() {
            std::fs::create_dir_all(&out).map_err(|e| e.to_string())?;
            continue;
        }
        if let Some(parent) = out.parent() {
            std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        let mut buf = Vec::with_capacity(entry.size() as usize);
        std::io::copy(&mut entry, &mut buf).map_err(|e| e.to_string())?;
        std::fs::write(&out, &buf).map_err(|e| e.to_string())?;
    }
    // Chrome needs its sandbox and several binaries with the execute bit.
    // Instead of guessing by extension I mark the whole tree: inside that
    // folder everything executable has to be executable.
    mark_tree_exec(dest)
}

fn mark_tree_exec(dir: &Path) -> Result<(), String> {
    use std::os::unix::fs::PermissionsExt;
    for e in std::fs::read_dir(dir).map_err(|e| e.to_string())? {
        let e = e.map_err(|e| e.to_string())?;
        let p = e.path();
        let meta = e.metadata().map_err(|e| e.to_string())?;
        if meta.is_dir() {
            mark_tree_exec(&p)?;
        } else {
            let mut perm = meta.permissions();
            perm.set_mode(0o755);
            let _ = std::fs::set_permissions(&p, perm);
        }
    }
    Ok(())
}

fn make_exec(p: &Path) -> Result<(), String> {
    use std::os::unix::fs::PermissionsExt;
    let mut perm = std::fs::metadata(p).map_err(|e| e.to_string())?.permissions();
    perm.set_mode(0o755);
    std::fs::set_permissions(p, perm).map_err(|e| e.to_string())
}

// ─── guaranteed cleanup ─────────────────────────────────────────────────────

/// I leave the system as I found it, whatever happens.
///
/// I launch the child Chromium in its own process group, so a `SIGKILL` to the
/// group takes down the browser and all its subprocesses (zygote, gpu,
/// renderer) without ever being able to touch anything of yours.
struct Cleanup {
    profile: PathBuf,
    pgid: i32,
    child: Option<Child>,
    cdp: Option<Cdp>,
}

impl Drop for Cleanup {
    fn drop(&mut self) {
        // I kill the whole group and that's it. I don't send `Browser.close`
        // first: the profile gets deleted anyway, so there's nothing for
        // Chrome to save, and its orderly shutdown reparents child processes
        // out of the group, leaving behind utilities that stay alive for a
        // while. The `SIGKILL` to the group leaves nothing.
        unsafe {
            libc::kill(-self.pgid, libc::SIGKILL);
        }
        if let Some(mut c) = self.child.take() {
            let _ = c.wait();
        }
        // The ephemeral profile always disappears.
        let _ = std::fs::remove_dir_all(&self.profile);
    }
}

/// The flag the stop signals raise.
///
/// Without this, a SIGTERM — the launcher cancelling, or the app closing —
/// killed the process without going through `Drop`, and Chromium stayed
/// orphaned with its profile on disk. Handling the signal turns an abrupt
/// death into a clean exit: the handler only sets the flag, and the polling
/// loop sees it and returns through the normal path, which does run the guard.
static STOP: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

extern "C" fn on_stop(_sig: libc::c_int) {
    // I only touch one atomic: nothing else is safe inside a signal handler.
    STOP.store(true, std::sync::atomic::Ordering::SeqCst);
}

fn install_stop_handlers() {
    for sig in [libc::SIGTERM, libc::SIGINT, libc::SIGHUP] {
        unsafe {
            libc::signal(sig, on_stop as *const () as libc::sighandler_t);
        }
    }
}

// ─── code extraction ────────────────────────────────────────────────────────

/// Epic and GOG keys, in the order the plugin accepts them: first
/// `authorizationCode`, then `code`, and `sid` as the last resort because
/// `legendary auth --sid` takes it too.
const KEYS: [&str; 3] = ["authorizationCode", "code", "sid"];

/// I extract the code from a URL, in the query or the fragment.
///
/// That's GOG's path: signing in lands on
/// `embed.gog.com/on_login_success?code=...`.
fn from_url(url: &str) -> Option<String> {
    for key in KEYS {
        let re = Regex::new(&format!(r#"(?i)[?&#]{}=([^&\s"'#]+)"#, key)).ok()?;
        if let Some(m) = re.captures(url) {
            let v = m.get(1)?.as_str().trim();
            if !v.is_empty() {
                return Some(v.to_string());
            }
        }
    }
    None
}

/// I extract the code from the page body.
///
/// That's Epic's path: its redirect endpoint returns JSON with
/// `authorizationCode` already filled in and the URL unchanged. Without a
/// session the field is `null`, and I discard that so I never redeem garbage.
fn from_body(body: &str) -> Option<String> {
    for key in KEYS {
        let re = Regex::new(&format!(r#"(?i)"{key}"\s*[:=]\s*"?([^",\s}}]+)"?"#)).ok()?;
        if let Some(m) = re.captures(body) {
            let v = m.get(1)?.as_str().trim();
            if !v.is_empty() && v != "null" && v != "None" {
                return Some(v.to_string());
            }
        }
    }
    None
}

// ─── minimal WebSocket ──────────────────────────────────────────────────────

/// A WebSocket client with only what CDP needs: the handshake and text frames,
/// reassembled from continuations. I use no library because the protocol is
/// narrow and this way the helper depends on nobody.
///
/// It's not a general-purpose websocket: I don't negotiate permessage-deflate,
/// I don't validate `Sec-WebSocket-Accept` and I don't reconnect. To talk to a
/// local Chromium that does nothing exotic, that's enough.
struct Cdp {
    sock: TcpStream,
    buf: Vec<u8>,
    next_id: u64,
}

impl Cdp {
    fn connect(ws_url: &str) -> Result<Self, String> {
        let rest = ws_url
            .strip_prefix("ws://")
            .ok_or_else(|| format!("URL de WebSocket no soportada: {}", ws_url))?;
        let (hostport, path) = match rest.find('/') {
            Some(i) => (&rest[..i], &rest[i..]),
            None => (rest, "/"),
        };
        let mut sock = TcpStream::connect(hostport).map_err(|e| format!("could not connect: {}", e))?;
        sock.set_nodelay(true).ok();

        // The key value is arbitrary for a local server, but I send 16 random
        // bytes anyway to stay within the spec.
        let key = base64(&random_bytes(16));
        let req = format!(
            "GET {} HTTP/1.1\r\nHost: {}\r\nUpgrade: websocket\r\nConnection: Upgrade\r\n\
             Sec-WebSocket-Key: {}\r\nSec-WebSocket-Version: 13\r\n\r\n",
            path, hostport, key
        );
        sock.write_all(req.as_bytes()).map_err(|e| e.to_string())?;

        let mut raw = Vec::new();
        let mut one = [0u8; 1];
        while !raw.ends_with(b"\r\n\r\n") {
            if sock.read(&mut one).map_err(|e| e.to_string())? == 0 {
                return Err("the browser closed before the handshake".into());
            }
            raw.push(one[0]);
        }
        let head = String::from_utf8_lossy(&raw);
        if !head.starts_with("HTTP/1.1 101") {
            return Err(format!("handshake rechazado: {}", head.lines().next().unwrap_or("")));
        }
        Ok(Cdp {
            sock,
            buf: Vec::new(),
            next_id: 0,
        })
    }

    /// I send a command and wait for its reply, discarding the events Chromium
    /// interleaves on the way.
    fn call(&mut self, method: &str, params: Value, timeout: Duration) -> Result<Value, String> {
        self.next_id += 1;
        let id = self.next_id;
        let msg = json!({"id": id, "method": method, "params": params});
        let text = serde_json::to_string(&msg).map_err(|e| e.to_string())?;
        self.send_frame(text.as_bytes())?;

        let deadline = Instant::now() + timeout;
        loop {
            if Instant::now() > deadline {
                return Err(format!("{} no respondio en {} s", method, timeout.as_secs()));
            }
            self.sock.set_read_timeout(Some(Duration::from_millis(1500))).ok();
            match self.recv_text() {
                Ok(txt) => {
                    let Ok(v) = serde_json::from_str::<Value>(&txt) else {
                        continue;
                    };
                    if v.get("id").and_then(|x| x.as_u64()) != Some(id) {
                        continue; // an event, not my reply
                    }
                    if let Some(err) = v.get("error") {
                        return Err(format!("{}: {}", method, err));
                    }
                    return Ok(v.get("result").cloned().unwrap_or(Value::Null));
                }
                Err(e) => return Err(format!("{}: {}", method, e)),
            }
        }
    }

    fn send_frame(&mut self, payload: &[u8]) -> Result<(), String> {
        let mut f: Vec<u8> = vec![0x81];
        let n = payload.len();
        if n < 126 {
            f.push(0x80 | n as u8);
        } else if n < 65536 {
            f.push(0x80 | 126);
            f.extend_from_slice(&(n as u16).to_be_bytes());
        } else {
            f.push(0x80 | 127);
            f.extend_from_slice(&(n as u64).to_be_bytes());
        }
        // The client side always has to mask.
        let mask = random_bytes(4);
        f.extend_from_slice(&mask);
        f.extend(payload.iter().enumerate().map(|(i, b)| b ^ mask[i % 4]));
        self.sock.write_all(&f).map_err(|e| e.to_string())
    }

    /// I read one complete text frame, joining the continuation ones.
    fn recv_text(&mut self) -> Result<String, String> {
        let mut payload: Vec<u8> = Vec::new();
        loop {
            let (fin, op, data) = self.read_frame()?;
            match op {
                0x8 => return Err("the browser closed the connection".into()),
                0x9 => continue, // ping: I ignore it, the socket doesn't need it
                0xA => continue, // pong
                0x1 => payload = data,
                0x0 => payload.extend_from_slice(&data),
                other => return Err(format!("unexpected frame opcode: {:#x}", other)),
            }
            if fin {
                return String::from_utf8(payload)
                    .map_err(|_| "the response was not UTF-8".to_string());
            }
        }
    }

    fn read_frame(&mut self) -> Result<(bool, u8, Vec<u8>), String> {
        let h = self.read_exact(2)?;
        let fin = h[0] & 0x80 != 0;
        let op = h[0] & 0x0F;
        let masked = h[1] & 0x80 != 0;
        let mut len = (h[1] & 0x7F) as u64;
        if len == 126 {
            let e = self.read_exact(2)?;
            len = u16::from_be_bytes([e[0], e[1]]) as u64;
        } else if len == 127 {
            let e = self.read_exact(8)?;
            len = u64::from_be_bytes([e[0], e[1], e[2], e[3], e[4], e[5], e[6], e[7]]);
        }
        // Chromium doesn't mask (it's the server), but I accept masked frames
        // just in case.
        let mask = if masked { self.read_exact(4)? } else { Vec::new() };
        let mut data = self.read_exact(len as usize)?;
        if masked && !mask.is_empty() {
            for (i, b) in data.iter_mut().enumerate() {
                *b ^= mask[i % 4];
            }
        }
        Ok((fin, op, data))
    }

    fn read_exact(&mut self, n: usize) -> Result<Vec<u8>, String> {
        while self.buf.len() < n {
            let mut chunk = [0u8; 65536];
            let got = self.sock.read(&mut chunk).map_err(|e| e.to_string())?;
            if got == 0 {
                return Err("connection closed".into());
            }
            self.buf.extend_from_slice(&chunk[..got]);
        }
        let out = self.buf[..n].to_vec();
        self.buf.drain(..n);
        Ok(out)
    }
}

fn random_bytes(n: usize) -> Vec<u8> {
    use std::io::Read as _;
    let mut v = vec![0u8; n];
    if let Ok(mut f) = std::fs::File::open("/dev/urandom") {
        if f.read_exact(&mut v).is_ok() {
            return v;
        }
    }
    // Only if `/dev/urandom` can't be opened. The handshake key only has to
    // differ between connections, not be secret.
    let t = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    for (i, b) in v.iter_mut().enumerate() {
        *b = ((t >> (i % 16 * 8)) as u8) ^ (std::process::id() as u8).wrapping_mul(31);
    }
    v
}

fn base64(data: &[u8]) -> String {
    const T: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::new();
    for chunk in data.chunks(3) {
        let b = [
            chunk[0],
            *chunk.get(1).unwrap_or(&0),
            *chunk.get(2).unwrap_or(&0),
        ];
        let n = ((b[0] as u32) << 16) | ((b[1] as u32) << 8) | b[2] as u32;
        out.push(T[(n >> 18) as usize & 63] as char);
        out.push(T[(n >> 12) as usize & 63] as char);
        out.push(if chunk.len() > 1 { T[(n >> 6) as usize & 63] as char } else { '=' });
        out.push(if chunk.len() > 2 { T[n as usize & 63] as char } else { '=' });
    }
    out
}

// ─── main flow ──────────────────────────────────────────────────────────────

fn run(url: &str) -> Result<String, Failure> {
    let chrome = ensure_chrome().map_err(|e| (why::CHROME, e))?;

    let profile = std::env::temp_dir().join(format!(
        "corkytux-login-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0)
    ));
    std::fs::create_dir_all(&profile)
        .map_err(|e| (why::SESSION, format!("temp profile: {}", e)))?;

    let port = free_port().map_err(|e| (why::SESSION, e))?;

    use std::os::unix::process::CommandExt;
    // Fixed size, centered: the login form is narrow and tall, and without
    // these flags Chrome decides on its own (Epic came out tiny, GOG huge). I
    // leave `--app=` out on purpose: it demands the URL at startup instead of
    // `about:blank` + `Page.navigate`, and it changes how the OAuth popups the
    // login needs behave.
    let (pantalla_w, pantalla_h) = pantalla();
    let (win_w, win_h, win_x, win_y) = ventana_login(pantalla_w, pantalla_h);
    let child = Command::new(&chrome)
        .arg(format!("--user-data-dir={}", profile.display()))
        .arg(format!("--remote-debugging-port={}", port))
        // Its own group: that's what lets me kill the child Chromium without
        // touching yours. I skip `--enable-automation` and headless on purpose:
        // they're what would set `navigator.webdriver` to true.
        .process_group(0)
        .arg("--no-first-run")
        .arg("--no-default-browser-check")
        .arg("--disable-session-crashed-bubble")
        .arg(format!("--window-size={},{}", win_w, win_h))
        .arg(format!("--window-position={},{}", win_x, win_y))
        .arg("about:blank")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| (why::CHROME, format!("could not launch the browser: {}", e)))?;
    let pgid = child.id() as i32;

    let mut cleanup = Cleanup {
        profile: profile.clone(),
        pgid,
        child: Some(child),
        cdp: None,
    };

    // I wait for the debugging port to answer and for the tab to show up. The
    // first time takes a while: it's a whole browser. At startup the only tab
    // is the `about:blank` the helper launched, so with no criteria the first
    // tab wins like before. I keep its target ID so I can find that tab again
    // on reconnect even after it has navigated.
    let (ws, mut target_id) = wait_for_page(port, Duration::from_secs(90), None, &[])
        .map_err(|e| (why::SESSION, e))?;
    let cdp = Cdp::connect(&ws).map_err(|e| (why::SESSION, e))?;
    cleanup.cdp = Some(cdp);

    cleanup
        .cdp
        .as_mut()
        .expect("acaba de insertarse")
        .call("Page.navigate", json!({ "url": url }), Duration::from_secs(60))
        .map_err(|e| (why::NAV, format!("could not open the login URL: {}", e)))?;

    // I bring the freshly opened window to the front once: if you were busy
    // somewhere else, the login doesn't end up hidden behind it. `pgid` is the
    // PID of the child Chromium (its group leader), which is what xdotool looks
    // for.
    traer_al_frente(pgid as u32);

    eprintln!("VENTANA:browser open; log in there");

    let deadline = Instant::now() + LOGIN_TIMEOUT;
    let mut last_url = String::new();
    // Streak of CDP transport failures. Each one tries a reconnect with a
    // short wait; once the attempts run out I fail right away instead of
    // burning all 180 s.
    let mut fallos_socket: u32 = 0;
    const MAX_RECONEXIONES: u32 = 2;
    const ESPERA_RECONEXION: [u64; 2] = [1, 2];
    loop {
        if STOP.load(std::sync::atomic::Ordering::SeqCst) {
            // Someone asked us to stop. I leave through the normal path so the
            // guard kills Chromium and deletes the profile.
            return Err((why::CANCELLED, "the login was cancelled".into()));
        }
        if Instant::now() > deadline {
            // I return the last known URL: without it the launcher can't say
            // where the login got stuck.
            return Err((
                why::TIMEOUT,
                format!(
                    "the login did not complete in {} s; the last URL was: {}",
                    LOGIN_TIMEOUT.as_secs(),
                    if last_url.is_empty() { "(not read)".into() } else { last_url }
                ),
            ));
        }

        // I keep `cdp` inside the guard so `Drop`'s orderly shutdown can use
        // it. The borrow is scoped to this block so I can move the guard
        // afterwards.
        let encontrado: Option<(String, &'static str)> = {
            let cdp = match cleanup.cdp.as_mut() {
                Some(c) => c,
                None => return Err((why::SESSION, "the CDP session was lost".into())),
            };
            match cdp.call(
                "Runtime.evaluate",
                json!({
                    "expression":
                        "JSON.stringify({u: location.href, b: document.body ? document.body.innerText : ''})",
                    "returnByValue": true
                }),
                Duration::from_secs(10),
            ) {
                Ok(res) => {
                    // The socket answered: whatever failed before is behind
                    // us and the transport streak resets.
                    fallos_socket = 0;
                    let val = res
                        .get("result")
                        .and_then(|x| x.get("value"))
                        .and_then(|x| x.as_str())
                        .unwrap_or("");
                    match serde_json::from_str::<Value>(val) {
                        Ok(page) => {
                            let href = page.get("u").and_then(|x| x.as_str()).unwrap_or("");
                            let body = page.get("b").and_then(|x| x.as_str()).unwrap_or("");
                            if let Some(code) = from_url(href) {
                                last_url = href.to_string();
                                Some((code, "URL"))
                            } else if let Some(code) = from_body(body) {
                                last_url = href.to_string();
                                // Epic hands the code over in the body, GOG in
                                // the URL. I read both because there's no
                                // reliable way to know in advance which one
                                // will show up, and the code expires in ~1 min.
                                Some((code, "pagina"))
                            } else {
                                last_url = href.to_string();
                                None
                            }
                        }
                        Err(e) => {
                            eprintln!("warning: unreadable response ({})", e);
                            None
                        }
                    }
                }
                Err(e) => {
                    if socket_muerto(&e) {
                        // The socket died: I try to revive it with a short
                        // wait instead of retrying against a corpse until the
                        // timeout, which is what used to leave the login
                        // "hanging".
                        fallos_socket += 1;
                        eprintln!("warning: CDP connection lost ({})", e);
                        if fallos_socket > MAX_RECONEXIONES {
                            return Err((
                                why::CDP,
                                format!(
                                    "the connection to the login browser was lost ({}) after {} reconnection attempts; try again with Log in",
                                    e, MAX_RECONEXIONES
                                ),
                            ));
                        }
                        let espera = ESPERA_RECONEXION
                            .get((fallos_socket - 1) as usize)
                            .copied()
                            .unwrap_or(2);
                        eprintln!(
                            "CDP:reconnecting, attempt {} of {}, in {} s",
                            fallos_socket, MAX_RECONEXIONES, espera
                        );
                        std::thread::sleep(Duration::from_secs(espera));
                        match reconectar_cdp(port, url, &last_url, &target_id) {
                            Ok((nuevo, nuevo_id)) => {
                                cleanup.cdp = Some(nuevo);
                                // The tab we came back to is the login tab by
                                // the best criteria available, so I keep
                                // tracking that one from now on.
                                target_id = nuevo_id;
                                eprintln!("CDP:connection restored");
                            }
                            Err(r) => {
                                eprintln!(
                                    "warning: could not restore the CDP connection ({})",
                                    r
                                );
                                if fallos_socket >= MAX_RECONEXIONES {
                                    return Err((
                                        why::CDP,
                                        format!(
                                            "the connection to the login browser was lost ({}) and could not be restored ({}); try again with Log in",
                                            e, r
                                        ),
                                    ));
                                }
                                // One attempt left: I carry on and the next
                                // poll tries again.
                            }
                        }
                        None
                    } else {
                        // A navigation change can interrupt the read; I keep going.
                        eprintln!("warning: could not read the page ({})", e);
                        None
                    }
                }
            }
        };

        if let Some((code, donde)) = encontrado {
            eprintln!("VENTANA:code found in the {}", donde);
            drop(cleanup);
            return Ok(code);
        }
        std::thread::sleep(POLL);
    }
}

/// I wait for Chromium to expose a tab over CDP and return its
/// `webSocketDebuggerUrl` along with its target ID.
///
/// `id` is the target ID I track since the first connection: Chromium keeps it
/// stable even when the tab navigates (it only changes if the tab closes), so
/// it's the main criteria on reconnect. `dominios` is the per-host fallback for
/// when the original tab is already gone. With a single tab there's no
/// ambiguity and I use it like before. With neither ID nor domains the first
/// tab wins (startup, where the only tab is the `about:blank` the helper
/// launched).
fn wait_for_page(
    port: u16,
    timeout: Duration,
    id: Option<&str>,
    dominios: &[&str],
) -> Result<(String, String), String> {
    let deadline = Instant::now() + timeout;
    let mut last = "the browser did not expose its debugging port".to_string();
    while Instant::now() < deadline {
        if let Ok(txt) = reqwest::blocking::get(format!("http://127.0.0.1:{}/json/list", port))
            .and_then(|r| r.error_for_status())
            .and_then(|r| r.text())
        {
            if let Ok(list) = serde_json::from_str::<Value>(&txt) {
                if let Some(arr) = list.as_array() {
                    let paginas: Vec<&Value> = arr
                        .iter()
                        .filter(|t| {
                            t.get("type").and_then(|x| x.as_str()) == Some("page")
                                && t
                                    .get("webSocketDebuggerUrl")
                                    .and_then(|x| x.as_str())
                                    .is_some()
                        })
                        .collect();
                    if let Some(par) = elegir_pestana(&paginas, id, dominios) {
                        return Ok(par);
                    }
                    last = if paginas.is_empty() {
                        format!(
                            "the browser returned no tabs yet ({} target(s))",
                            arr.len()
                        )
                    } else {
                        format!(
                            "there are {} tab(s) but none is the login tab (neither by ID nor by domain)",
                            paginas.len()
                        )
                    };
                    std::thread::sleep(Duration::from_millis(300));
                    continue;
                }
            }
            last = format!(
                "the browser returned no tabs yet ({} target(s))",
                serde_json::from_str::<Value>(&txt)
                    .ok()
                    .and_then(|v| v.as_array().map(|a| a.len()))
                    .unwrap_or(0)
            );
        }
        std::thread::sleep(Duration::from_millis(300));
    }
    Err(format!("{} tras {} s", last, timeout.as_secs()))
}

/// I pick which tab to connect to among those `/json/list` exposes, and return
/// its `webSocketDebuggerUrl` along with its target ID.
///
/// Main criteria: the target ID, which Chromium keeps stable even when the tab
/// navigates — it only changes if the tab closes. I track it since the first
/// connection, so an OAuth popup (Google SSO) whose URL changes several times
/// doesn't confuse the reconnect.
///
/// Fallback: if the ID is already gone (tab closed), any tab whose host is in
/// `dominios` counts. With a single tab there's no ambiguity and I use it like
/// before; with several and nothing matching I return `None` to keep waiting
/// (or burn the retry) instead of reading someone else's tab until the global
/// timeout.
fn elegir_pestana(
    paginas: &[&Value],
    id: Option<&str>,
    dominios: &[&str],
) -> Option<(String, String)> {
    let ws_e_id = |t: &&Value| {
        let ws = t.get("webSocketDebuggerUrl").and_then(|x| x.as_str())?;
        let tid = t.get("id").and_then(|x| x.as_str())?;
        Some((ws.to_string(), tid.to_string()))
    };
    if paginas.len() == 1 {
        return ws_e_id(&paginas[0]);
    }
    if let Some(id) = id {
        for t in paginas {
            if t.get("id").and_then(|x| x.as_str()) == Some(id) {
                if let Some(par) = ws_e_id(t) {
                    return Some(par);
                }
            }
        }
    }
    if !dominios.is_empty() {
        for t in paginas {
            let url = t.get("url").and_then(|x| x.as_str()).unwrap_or("");
            if url_en_dominios(url, dominios) {
                if let Some(par) = ws_e_id(t) {
                    return Some(par);
                }
            }
        }
    }
    if id.is_none() && dominios.is_empty() {
        // With no criteria the first one wins: the historical startup behavior.
        return paginas.first().and_then(ws_e_id);
    }
    None
}

/// I pull the host out of a URL (`https://auth.gog.com/auth?x=1` →
/// `auth.gog.com`). Only for the per-domain fallback filter; I validate
/// nothing.
fn host_de(url: &str) -> Option<&str> {
    let resto = url.split("://").nth(1)?;
    Some(resto.split(['/', '?', '#']).next().unwrap_or(resto))
}

/// I report whether a tab's URL falls inside one of the expected domains,
/// comparing by host and not by exact URL: the tab navigates during the login
/// and its URL changes several times.
fn url_en_dominios(url: &str, dominios: &[&str]) -> bool {
    match host_de(url) {
        Some(h) => dominios.iter().any(|d| h.eq_ignore_ascii_case(d)),
        None => false,
    }
}

/// I report whether an error from `cdp.call()` means the CDP socket died.
///
/// Only transport errors count: the browser closing the connection, a broken
/// pipe or a connection reset. A `Runtime.evaluate` failure from a navigation
/// mid-read is NOT fatal: that case keeps going through the warn-and-retry path
/// like before.
fn socket_muerto(e: &str) -> bool {
    let t = e.to_lowercase();
    t.contains("connection closed")
        || t.contains("closed the connection")
        || t.contains("closed before")
        || t.contains("broken pipe")
        || t.contains("connection reset")
        || t.contains("os error 32")
        || t.contains("os error 104")
}

/// I reopen the CDP session against the same Chromium, on the login tab.
///
/// I find that tab again by its target ID, which is stable even if the tab
/// navigated (a real case: an OAuth popup whose URL changed between the
/// disconnect and the reconnect). As a fallback, if the ID is already gone,
/// any tab in the domains of the original login or of the last URL read counts
/// (GOG navigates from `auth.gog.com` to `embed.gog.com` when the login
/// completes). I don't relaunch the browser: if the process died or no usable
/// tab is left, I return the error so it fails fast with `cdp_perdido`. I
/// return the session and the ID of the tab we came back to, to keep tracking
/// that one from now on.
fn reconectar_cdp(
    port: u16,
    login_url: &str,
    last_url: &str,
    target_id: &str,
) -> Result<(Cdp, String), String> {
    let mut dominios: Vec<&str> = Vec::with_capacity(2);
    for u in [login_url, last_url] {
        if let Some(h) = host_de(u) {
            if !dominios.contains(&h) {
                dominios.push(h);
            }
        }
    }
    let (ws, id) = wait_for_page(port, Duration::from_secs(10), Some(target_id), &dominios)?;
    Cdp::connect(&ws).map(|cdp| (cdp, id))
}

fn free_port() -> Result<u16, String> {
    let l = std::net::TcpListener::bind("127.0.0.1:0").map_err(|e| e.to_string())?;
    let p = l.local_addr().map_err(|e| e.to_string())?.port();
    drop(l);
    Ok(p)
}

/// Screen resolution, to center the login window.
///
/// I try `xdotool getdisplaygeometry` (it returns `WIDTH HEIGHT`); if xdotool
/// isn't installed or it fails, I fall back to a documented 1920x1080. It never
/// fails: worst case the window comes out centered for 1080p.
fn pantalla() -> (u32, u32) {
    const FALLBACK: (u32, u32) = (1920, 1080);
    let salida = std::process::Command::new("xdotool")
        .arg("getdisplaygeometry")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .output();
    let Ok(salida) = salida else {
        return FALLBACK;
    };
    if !salida.status.success() {
        return FALLBACK;
    }
    let texto = String::from_utf8_lossy(&salida.stdout);
    let mut nums = texto
        .split_whitespace()
        .filter_map(|w| w.parse::<u32>().ok());
    match (nums.next(), nums.next()) {
        (Some(w), Some(h)) if w > 0 && h > 0 => (w, h),
        _ => FALLBACK,
    }
}

/// Login window geometry: narrow and tall for the form, and centered on
/// screen. I return `(width, height, x, y)`. Positions saturate at 0 so I never
/// hand the WM negative coordinates on small screens.
fn ventana_login(pantalla_w: u32, pantalla_h: u32) -> (u32, u32, u32, u32) {
    const W: u32 = 480;
    const H: u32 = 720;
    let x = pantalla_w.saturating_sub(W) / 2;
    let y = pantalla_h.saturating_sub(H) / 2;
    (W, H, x, y)
}

/// I bring the login Chromium's window to the front, one single time.
///
/// I find its windows by PID (`xdotool search --onlyvisible --pid`) and send
/// them `windowraise` + `windowfocus`. It's best-effort with short retries: if
/// xdotool isn't there or the window doesn't exist yet, I only warn on stderr
/// and the login carries on.
///
/// This is NOT a sticky "always on top": xdotool has no primitive to set
/// EWMH's `_NET_WM_STATE_ABOVE` (and `wmctrl`, which does have it, isn't
/// installed). If you click the launcher afterwards, Chromium goes back
/// behind like any normal window. Pinning it for real would mean hand-rolling
/// the EWMH client-message (new X11 code) or depending on `wmctrl`.
fn traer_al_frente(pid: u32) {
    const INTENTOS: u32 = 5;
    for intento in 1..=INTENTOS {
        match ventana_de_pid(pid) {
            Some(wid) => {
                let _ = comando_xdotool(&["windowraise", &wid]);
                let _ = comando_xdotool(&["windowfocus", &wid]);
                eprintln!("VENTANA:login window brought to the front");
                return;
            }
            None if intento < INTENTOS => {
                std::thread::sleep(Duration::from_millis(500));
            }
            None => {
                eprintln!("warning: login window not found to bring to the front");
            }
        }
    }
}

/// I return the ID of the first visible window of the given PID, or `None`.
fn ventana_de_pid(pid: u32) -> Option<String> {
    let out = comando_xdotool(&["search", "--onlyvisible", "--pid", &pid.to_string()])?;
    elegir_ventana(&out)
}

/// I run xdotool and return its stdout if it went well. `None` if xdotool isn't
/// there, fails or prints nothing: the login never depends on this.
fn comando_xdotool(args: &[&str]) -> Option<Vec<u8>> {
    let out = std::process::Command::new("xdotool")
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .output()
        .ok()?;
    if !out.status.success() || out.stdout.is_empty() {
        return None;
    }
    Some(out.stdout)
}

/// I pick a window out of `xdotool search` output: the first non-empty line
/// (one window ID per line).
fn elegir_ventana(salida: &[u8]) -> Option<String> {
    let texto = String::from_utf8_lossy(salida);
    texto
        .lines()
        .map(|l| l.trim())
        .find(|l| !l.is_empty())
        .map(|s| s.to_string())
}

// ─── tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
#[allow(non_snake_case)]
mod tests {
    use super::*;

    #[test]
    fn gog_code_en_la_url() {
        assert_eq!(
            from_url("https://embed.gog.com/on_login_success?origin=client&code=abc123XYZ").as_deref(),
            Some("abc123XYZ")
        );
    }

    #[test]
    fn gog_code_en_el_fragmento() {
        assert_eq!(from_url("https://auth.gog.com/auth/callback#code=frag999").as_deref(), Some("frag999"));
    }

    #[test]
    fn authorizationCode_gana_a_code() {
        // Epic calls theirs authorizationCode; if a URL carries both, the one
        // I redeem is Epic's.
        assert_eq!(from_url("https://x/?code=generic&authorizationCode=epicOne").as_deref(), Some("epicOne"));
    }

    #[test]
    fn sid_es_el_ultimo_recurso() {
        assert_eq!(from_url("https://www.epicgames.com/id/api/redirect?sid=SESSION42").as_deref(), Some("SESSION42"));
    }

    #[test]
    fn una_url_sin_codigo_no_inventa_uno() {
        assert_eq!(from_url("https://www.epicgames.com/id/login?redirectUrl=https%3A%2F%2Fx"), None);
    }

    /// The exact body Epic returns, measured against the live site.
    #[test]
    fn el_json_real_de_epic_da_el_codigo() {
        let b = r#"{"warning":"Do not share this code with any 3rd party service.","redirectUrl":"https://localhost/launcher/authorized","authorizationCode":"eyJhbGciOiJFUzI1NiJ9.abc-_123","exchangeCode":null,"sid":null}"#;
        assert_eq!(from_body(b).as_deref(), Some("eyJhbGciOiJFUzI1NiJ9.abc-_123"));
    }

    /// The same body without a session: `authorizationCode` comes back `null`.
    /// Redeeming that would give a confusing error, so it has to be discarded.
    #[test]
    fn el_json_sin_sesion_da_null_y_no_se_canjea() {
        let b = r#"{"warning":"Do not share.","redirectUrl":"https://localhost/launcher/authorized","authorizationCode":null,"exchangeCode":null,"sid":null}"#;
        assert_eq!(from_body(b), None);
    }

    #[test]
    fn el_json_envuelto_en_pre_tambien_sirve() {
        let b = "<html><body><pre>{\"authorizationCode\":\"JWKvalue9\"}</pre></body></html>";
        assert_eq!(from_body(b).as_deref(), Some("JWKvalue9"));
    }

    #[test]
    fn la_pantalla_de_login_no_inventa_un_codigo() {
        let b = "Sign in to Epic Games Password Continue PlayStation Network Xbox network";
        assert_eq!(from_body(b), None);
    }

    #[test]
    fn base64_codifica_bien() {
        assert_eq!(base64(b"abc"), "YWJj");
        assert_eq!(base64(b"ab"), "YWI=");
        assert_eq!(base64(b"a"), "YQ==");
    }

    #[test]
    fn las_claves_son_aleatorias() {
        assert_ne!(random_bytes(16), random_bytes(16));
    }

    /// Transport errors have to mark the socket dead so the poll reconnects
    /// instead of retrying against a corpse.
    #[test]
    fn socket_muerto_detecta_transporte() {
        // The ones I actually saw in the 19:50 log.
        assert!(socket_muerto("Runtime.evaluate: connection closed"));
        assert!(socket_muerto(
            "Runtime.evaluate: the browser closed the connection"
        ));
        assert!(socket_muerto(
            "Runtime.evaluate: Broken pipe (os error 32)"
        ));
        assert!(socket_muerto(
            "Runtime.evaluate: Connection reset by peer (os error 104)"
        ));
    }

    /// A read failure from navigation or from a timeout is NOT a dead socket:
    /// those keep going through the warn-and-retry path like before.
    #[test]
    fn socket_muerto_no_confunde_navegacion_ni_timeout() {
        assert!(!socket_muerto(
            "Runtime.evaluate did not answer in 10 s"
        ));
        assert!(!socket_muerto(
            "Runtime.evaluate: {\"code\":-32000,\"message\":\"No session\"}"
        ));
        assert!(!socket_muerto("warning: unreadable response (something)"));
    }

    /// I build a `/json/list` target to test tab selection.
    fn pagina(url: &str, id: &str, ws: &str) -> Value {
        json!({"type": "page", "url": url, "id": id, "webSocketDebuggerUrl": ws})
    }

    /// With a single tab there's no ambiguity: I use it even with no criteria
    /// (the `about:blank` just launched, or the only one open).
    #[test]
    fn elegir_pestana_usa_la_unica_aunque_no_haya_criterio() {
        let a = pagina("about:blank", "ID-A", "ws://127.0.0.1:9/devtools/page/AAA");
        let pags = [&a];
        assert_eq!(
            elegir_pestana(&pags, None, &[]),
            Some((
                "ws://127.0.0.1:9/devtools/page/AAA".to_string(),
                "ID-A".to_string()
            ))
        );
    }

    /// The real case that motivated the fix: two active tabs (the main one plus
    /// an SSO popup) and the main tab's URL had changed from `last_url`. I
    /// still find it by target ID, which is stable even while it navigates.
    #[test]
    fn elegir_pestana_encuentra_por_id_aunque_la_url_haya_cambiado() {
        // The main tab navigated from GOG's login to Google's consent screen
        // between the disconnect and the reconnect: neither `last_url`
        // (`auth.gog.com`) nor the current URL match by login domain.
        let principal = pagina(
            "https://accounts.google.com/o/oauth2/consent?x=1",
            "ID-MAIN",
            "ws://127.0.0.1:9/devtools/page/AAA",
        );
        let popup = pagina(
            "https://auth.gog.com/auth?client_id=1",
            "ID-POPUP",
            "ws://127.0.0.1:9/devtools/page/BBB",
        );
        let pags = [&principal, &popup];
        assert_eq!(
            elegir_pestana(&pags, Some("ID-MAIN"), &["auth.gog.com"]),
            Some((
                "ws://127.0.0.1:9/devtools/page/AAA".to_string(),
                "ID-MAIN".to_string()
            ))
        );
    }

    /// If the ID is already gone (tab closed), the per-domain fallback counts:
    /// GOG navigates from `auth.gog.com` to `embed.gog.com` when the login
    /// completes.
    #[test]
    fn elegir_pestana_sin_id_vale_el_respaldo_por_dominio() {
        let exito = pagina(
            "https://embed.gog.com/on_login_success?code=abc",
            "ID-NEW",
            "ws://127.0.0.1:9/devtools/page/BBB",
        );
        let popup = pagina(
            "https://accounts.google.com/o/oauth2/auth?x=1",
            "ID-POPUP",
            "ws://127.0.0.1:9/devtools/page/CCC",
        );
        let pags = [&exito, &popup];
        assert_eq!(
            elegir_pestana(&pags, Some("ID-CERRADA"), &["auth.gog.com", "embed.gog.com"]),
            Some((
                "ws://127.0.0.1:9/devtools/page/BBB".to_string(),
                "ID-NEW".to_string()
            ))
        );
    }

    /// With several tabs and neither ID nor domain matching I pick none: the
    /// caller keeps waiting (or burns the retry) instead of reading someone
    /// else's tab until the global timeout.
    #[test]
    fn elegir_pestana_sin_coincidencia_devuelve_none() {
        let a = pagina("about:blank", "ID-A", "ws://127.0.0.1:9/devtools/page/AAA");
        let b = pagina("chrome://newtab/", "ID-B", "ws://127.0.0.1:9/devtools/page/BBB");
        let pags = [&a, &b];
        assert_eq!(
            elegir_pestana(&pags, Some("ID-OTRA"), &["auth.gog.com"]),
            None
        );
    }

    /// With no criteria I keep the historical behavior: the first one wins.
    #[test]
    fn elegir_pestana_sin_criterio_gana_la_primera() {
        let a = pagina("about:blank", "ID-A", "ws://127.0.0.1:9/devtools/page/AAA");
        let b = pagina("chrome://newtab/", "ID-B", "ws://127.0.0.1:9/devtools/page/BBB");
        let pags = [&a, &b];
        assert_eq!(
            elegir_pestana(&pags, None, &[]),
            Some((
                "ws://127.0.0.1:9/devtools/page/AAA".to_string(),
                "ID-A".to_string()
            ))
        );
    }

    /// I extract the host for the domain filter (with a query, a fragment or a
    /// schemeless URL there's no exact match: `None` beats a false positive).
    #[test]
    fn host_de_extrae_el_host() {
        assert_eq!(
            host_de("https://auth.gog.com/auth?client_id=1"),
            Some("auth.gog.com")
        );
        assert_eq!(
            host_de("https://embed.gog.com/on_login_success?code=abc#frag"),
            Some("embed.gog.com")
        );
        assert_eq!(host_de("about:blank"), None);
    }

    /// The fallback compares by host, not by exact URL: the tab navigates and
    /// its URL changes, the domain doesn't.
    #[test]
    fn url_en_dominios_compara_por_host() {
        let doms = ["auth.gog.com", "embed.gog.com"];
        assert!(url_en_dominios("https://auth.gog.com/auth?x=1", &doms));
        assert!(url_en_dominios("https://embed.gog.com/on_login_success", &doms));
        assert!(!url_en_dominios("https://accounts.google.com/o/oauth2/auth", &doms));
        assert!(!url_en_dominios("about:blank", &doms));
    }

    /// I serve a fixed `/json/list` on localhost to test `wait_for_page`
    /// without a real Chromium. I return the port.
    fn servir_lista_fija(cuerpo: &'static str) -> u16 {
        servir_bytes_fijos(cuerpo.as_bytes())
    }

    /// I serve fixed bytes with `Content-Length` on localhost. I return the port.
    fn servir_bytes_fijos(cuerpo: &'static [u8]) -> u16 {
        let l = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let port = l.local_addr().unwrap().port();
        // I copy the body into the thread: the `&'static` outlives the test.
        let cuerpo: Vec<u8> = cuerpo.to_vec();
        std::thread::spawn(move || {
            for stream in l.incoming() {
                let Ok(mut s) = stream else { break };
                let mut buf = [0u8; 4096];
                let _ = s.read(&mut buf);
                let cabecera = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: application/octet-stream\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                    cuerpo.len(),
                );
                let _ = s.write_all(cabecera.as_bytes());
                let _ = s.write_all(&cuerpo);
            }
        });
        port
    }

    /// Several tabs on reconnect and the main one changed URL (an SSO popup
    /// happened): I find it again by its target ID.
    #[test]
    fn wait_for_page_reencuentra_por_id_aunque_la_url_haya_cambiado() {
        let cuerpo = r#"[{"type":"page","url":"https://accounts.google.com/o/oauth2/consent?x=1","id":"ID-MAIN","webSocketDebuggerUrl":"ws://127.0.0.1:9/devtools/page/AAA"},{"type":"page","url":"https://auth.gog.com/auth?client_id=1","id":"ID-POPUP","webSocketDebuggerUrl":"ws://127.0.0.1:9/devtools/page/BBB"}]"#;
        let port = servir_lista_fija(cuerpo);
        let (ws, id) = wait_for_page(port, Duration::from_secs(5), Some("ID-MAIN"), &["auth.gog.com"])
            .expect("the login tab is present by ID");
        assert_eq!(id, "ID-MAIN", "reconnected to the wrong tab");
        assert!(
            ws.ends_with("/AAA"),
            "picked the wrong tab: {}",
            ws
        );
    }

    /// Several tabs and neither the ID nor any domain matches: it fails on the
    /// short retry instead of hanging until the global timeout (in production
    /// that `Err` becomes `ERRO:cdp_perdido`).
    #[test]
    fn wait_for_page_sin_pestana_del_login_falla_en_el_reintento() {
        let cuerpo = r#"[{"type":"page","url":"about:blank","id":"ID-A","webSocketDebuggerUrl":"ws://127.0.0.1:9/devtools/page/AAA"},{"type":"page","url":"chrome://newtab/","id":"ID-B","webSocketDebuggerUrl":"ws://127.0.0.1:9/devtools/page/BBB"}]"#;
        let port = servir_lista_fija(cuerpo);
        let t0 = Instant::now();
        let err = wait_for_page(port, Duration::from_secs(1), Some("ID-OTRA"), &["auth.gog.com"])
            .expect_err("no tab is the login tab");
        assert!(
            t0.elapsed() < Duration::from_secs(5),
            "hung for {} s instead of failing on the retry",
            t0.elapsed().as_secs()
        );
        assert!(
            err.contains("none is the login tab"),
            "message with no useful diagnostic: {}",
            err
        );
    }

    /// At 1080p the 480x720 window comes out centered: x=(1920-480)/2,
    /// y=(1080-720)/2.
    #[test]
    fn ventana_login_centra_en_1080p() {
        assert_eq!(ventana_login(1920, 1080), (480, 720, 720, 180));
    }

    /// On small screens positions saturate at 0 instead of giving negative
    /// coordinates the WM would reject.
    #[test]
    fn ventana_login_no_da_posiciones_negativas_en_pantalla_chica() {
        assert_eq!(ventana_login(800, 600), (480, 720, 160, 0));
    }

    /// From `xdotool search` output (one ID per line) I take the first non-empty
    /// line.
    #[test]
    fn elegir_ventana_toma_la_primera_linea() {
        assert_eq!(
            elegir_ventana(b"0x1a00003\n0x1a00005\n").as_deref(),
            Some("0x1a00003")
        );
    }

    /// No output means no window: `None`, never an invented ID.
    #[test]
    fn elegir_ventana_sin_salida_da_none() {
        assert_eq!(elegir_ventana(b""), None);
        assert_eq!(elegir_ventana(b"\n  \n"), None);
    }

    /// `--probe` reflects the real cache: `false` with an empty HOME, `true`
    /// when the binary exists. I/O only, no processes and no network.
    #[test]
    fn sonda_chrome_reporta_cache() {
        let orig_home = std::env::var("HOME").unwrap_or_default();
        let tmp = std::env::temp_dir().join(format!("corkytux-sonda-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&tmp);
        std::env::set_var("HOME", &tmp);

        let r = sonda_chrome();
        assert_eq!(r.get("type").and_then(|x| x.as_str()), Some("chrome_probe"));
        assert_eq!(r.get("cached").and_then(|x| x.as_bool()), Some(false));

        let bin = chrome_bin();
        std::fs::create_dir_all(bin.parent().expect("chrome_bin tiene padre")).unwrap();
        std::fs::write(&bin, b"x").unwrap();
        assert_eq!(
            sonda_chrome().get("cached").and_then(|x| x.as_bool()),
            Some(true)
        );

        let _ = std::fs::remove_dir_all(&tmp);
        std::env::set_var("HOME", &orig_home);
    }

    /// The download writes the exact bytes and reports growing progress through
    /// the callback (the path `--prefetch` uses toward the modal).
    #[test]
    fn descargar_de_reporta_progreso_y_escribe_bytes() {
        static CUERPO: [u8; 200 * 1024] = [7; 200 * 1024];
        let port = servir_bytes_fijos(&CUERPO);
        let destino = std::env::temp_dir().join(format!(
            "corkytux-descarga-{}-{}.bin",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        ));

        let pcts: std::sync::Mutex<Vec<u8>> = std::sync::Mutex::new(Vec::new());
        descargar_de(
            &format!("http://127.0.0.1:{}/chrome-linux64.zip", port),
            &destino,
            CUERPO.len() as u64,
            &|pct, _| {
                pcts.lock().expect("mutex de test").push(pct);
            },
        )
        .expect("descarga local");

        let datos = std::fs::read(&destino).unwrap();
        let _ = std::fs::remove_file(&destino);
        assert_eq!(datos.len(), CUERPO.len());
        assert_eq!(datos, CUERPO);

        let pcts = pcts.lock().expect("mutex de test").clone();
        assert!(!pcts.is_empty(), "no progress reports");
        assert!(
            pcts.windows(2).all(|w| w[0] <= w[1]),
            "progreso no creciente: {:?}",
            pcts
        );
        // Reporting every ~10 % is by design: the last update doesn't have to
        // be 100 (in production the `done` event closes it).
        let ultimo = *pcts.last().expect("not empty");
        assert!(ultimo >= 90, "progreso final insuficiente: {:?}", pcts);
    }
}
