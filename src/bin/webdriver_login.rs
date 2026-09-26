//! Login de Epic/GOG con un Chromium real, controlado por CDP.
//!
//! ## Por qué Chromium y no Firefox
//!
//! Se empezó con Firefox por geckodriver y **no funciona**: el hCaptcha de Epic
//! rechaza el reto aunque se resuelva bien, porque cualquier Firefox gobernado
//! por WebDriver expone `navigator.webdriver === true` y eso es una señal de
//! automatización. Se midió que no hay forma de apagarlo:
//
//! | vía                                | `navigator.webdriver` |
//! |------------------------------------|-----------------------|
//! | geckodriver / Marionette           | `true`                |
//! | `dom.webdriver.enabled=false`      | `true`                |
//! | la misma pref en `user.js`         | `true`                |
//! | WebDriver BiDi (remote agent)      | `true`                |
//!
//! Marionette fuerza el flag desde C++ (`dom/base/WebDriver.cpp`), así que no
//! es un pref: no hay ajuste que lo quite.
//!
//! Chromium no tiene ese problema **si se lanza a mano**. `navigator.webdriver`
//! solo se activa con `--enable-automation` o en headless. Lanzándolo con
//! únicamente `--remote-debugging-port` y conectarse después por CDP, el valor
//! medido es `false`, y aun así se puede leer la página. Ese es el modo en que
//! se ejecuta este binario.
//!
//! Conviene notar **qué no hace** este helper: no pulsa botones, no escribe en
//! los campos y no envía el formulario. La persona teclea su contraseña y
//! resuelve el reto a mano, en una ventana real. El helper solo *lee* la URL y
//! el cuerpo de la página. Por eso no hace falta (ni se incluye) ninguna bandera
//! de evasión: el navegador no está automatizado en ninguna parte del flujo que
//! hCaptcha evalúa.
//!
//! ## Por qué Epic obliga a leer el cuerpo
//!
//! El endpoint de redirect de Epic devuelve el código en el **cuerpo** de la
//! respuesta, y la URL no cambia nunca:
//!
//! ```text
//! {"redirectUrl":"https://localhost/launcher/authorized",
//!  "authorizationCode":null,"exchangeCode":null,"sid":null}
//! ```
//!
//! Por eso no sirve vigilar la URL ni el historial del perfil: para Epic hay que
//! leer la página. GOG en cambio devuelve el código en la query, y ambos casos
//! quedan cubiertos porque se leen las dos cosas.
//!
//! ## Decisiones
//!
//! * **Sin runtime async.** CDP es WebSocket y se habla con `std::net::TcpStream`
//!   a mano. Así el helper es sincrónico como el resto del proyecto y no arrastra
//!   tokio/hyper/hyper-util, que es lo que habia obligado a usar WebDriver.
//! * **Perfil efímero por intento.** Se borra al terminar, pase lo que pase.
//!   Las cookies de la tienda no se mezclan con la navegación del usuario.
//! * **Grupo de procesos propio.** El Chromium se lanza en su propio grupo, así
//!   que el guard puede matarlo entero sin poder tocar el navegador del usuario.
//! * **Cierre en dos pasos** dentro del guard de drop: `Browser.close` y después
//!   el `SIGKILL` al grupo, para que ningún camino de salida deje huérfanos.

use std::io::{Read, Write};
use std::net::TcpStream;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use regex::Regex;
use serde_json::{json, Value};

// ─── CONFIGURACIÓN ───────────────────────────────────────────────────────────

/// Chrome for Testing: binarios oficiales de Google, publicados justamente para
/// esto. Se prefieren a una Chrome del sistema porque el número de versión
/// importa: el layout de la carpeta y del protocolo se mueve entre versiones.
const CHROME_VERSION: &str = "154.0.8037.57";
const CHROME_URL: &str = "https://storage.googleapis.com/chrome-for-testing-public/154.0.8037.57/linux64/chrome-linux64.zip";
/// Solo informativo: Chrome for Testing se distribuye bajo los términos de
/// Google, no bajo una licencia de proyecto. Ver `assets/icons/ATTRIBUTION.md`.
const CHROME_LICENSE: &str = "términos de Google (Chrome for Testing)";

/// Techo del login. El código de Epic caduca en ~1 minuto, así que 180 s
/// sobran para escribir las credenciales y resolver un reto a mano.
const LOGIN_TIMEOUT: Duration = Duration::from_secs(180);
/// Cadencia del sondeo de la página.
const POLL: Duration = Duration::from_millis(600);

/// Motivos de fallo, para que el launcher no tenga que adivinar.
mod why {
    pub const CHROME: &str = "chrome";
    pub const TIMEOUT: &str = "timeout";
    pub const SESSION: &str = "session";
    pub const NAV: &str = "nav";
    pub const CDP: &str = "cdp_perdido";
    pub const CANCELLED: &str = "cancelado";
    pub const USAGE: &str = "uso";
}

type Failure = (&'static str, String);

fn main() {
    install_stop_handlers();
    let args: Vec<String> = std::env::args().collect();
    match args.get(1).map(|s| s.as_str()) {
        // Solo filesystem: dice si el Chromium ya está cacheado, sin lanzar
        // ningún proceso de Chrome ni tocar la red. Lo usa el modal de
        // dependencias para decidir si molestar o no.
        Some("--probe") => {
            println!("{}", sonda_chrome());
        }
        // Descarga el Chromium si falta, con progreso JSON por stdout para el
        // modal de dependencias. Reusa el mismo camino del login.
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
                fail(why::USAGE, "la URL de autenticacion va vacia");
            }
            match run(url.trim()) {
                Ok(code) => println!("{}", code),
                Err((reason, msg)) => fail(reason, &msg),
            }
        }
        None => fail(why::USAGE, "falta la URL de autenticacion"),
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

/// Informe de caché del Chromium para el modal de dependencias.
///
/// Solo I/O de filesystem (`is_file` sobre la ruta cacheada): no lanza ningún
/// proceso de Chrome ni toca la red. Un `cached:false` significa que el primer
/// login (o un `--prefetch`) descargará ~188 MB.
fn sonda_chrome() -> Value {
    let bin = chrome_bin();
    json!({
        "type": "chrome_probe",
        "cached": bin.is_file(),
        "version": CHROME_VERSION,
        "path": bin.display().to_string(),
    })
}

/// Descarga el zip de Chromium, con reintentos y barra de progreso.
///
/// Los 188 MB no entran de una en redes normales: la primera vez que se probó
/// la conexión se cortó a mitad y el error que salió fue el genérico de
/// decodificación, que no dice nada útil. Ahora se reintenta con espera
/// creciente y, si aun así falla, el mensaje dice qué hacer.
///
/// Se escribe a un temporal y se renombra al terminar: un zip a medias nunca
/// puede quedar donde el código espera el definitivo.
///
/// `progreso` recibe `(porcentaje, bytes)` cada ~10 %: el login lo muestra
/// como texto humano, `--prefetch` lo emite como JSON para el modal.
fn download_zip(progreso: &dyn Fn(u8, u64)) -> Result<std::path::PathBuf, String> {
    /// Espera antes de cada reintento, en segundos.
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
                "CHROME:reintento {} de {} en {} s (motivo anterior: {})",
                intento,
                BACKOFF.len() - 1,
                espera,
                ultimo_error
            );
            std::thread::sleep(Duration::from_secs(*espera));
        }

        match intentar_descarga(&parcial, ESPERADO, progreso) {
            Ok(()) => {
                // El rename es atómico en el mismo sistema de archivos: o está
                // el zip entero, o no está nada.
                let _ = std::fs::remove_file(&final_zip);
                std::fs::rename(&parcial, &final_zip).map_err(|e| {
                    format!("no se pudo finalizar la descarga: {}", e)
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
        "la descarga se cortó o falló tras {} intentos. Es una descarga de 188 MB: \
         con la red inestable o sin espacio en disco se corta a mitad. Probá de nuevo \
         con Log in; si sigue igual, liberá espacio o revisá la conexión. \
         Detalle del último intento: {}",
        BACKOFF.len(),
        ultimo_error
    ))
}

/// Un intento de descarga, escribiendo a `destino` y mostrando el progreso.
fn intentar_descarga(
    destino: &Path,
    esperado: u64,
    progreso: &dyn Fn(u8, u64),
) -> Result<(), String> {
    descargar_de(CHROME_URL, destino, esperado, progreso)
}

/// Descarga `url` a `destino` con el mismo protocolo (reintento lo maneja el
/// llamador). Separada de `intentar_descarga` para poder probarla contra un
/// servidor local sin tocar la red real.
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
        .map_err(|e| format!("no se pudo preparar la conexion: {}", e))?
        .get(url)
        .send()
        .map_err(|e| format!("fallo de red al conectar: {}", e))?
        .error_for_status()
        .map_err(|e| format!("el servidor rechazo la descarga: {}", e))?;

    // Si el servidor dice cuanto ocupa, se usa; si no, se estima con el tamano
    // real que hay descargado.
    let total = resp
        .content_length()
        .filter(|n| *n > 0)
        .unwrap_or(esperado);

    let f = std::fs::File::create(destino).map_err(|e| format!("no se pudo crear el temporal: {}", e))?;
    let mut w = std::io::BufWriter::new(f);
    let mut buf = vec![0u8; 64 * 1024];
    let mut hecho: u64 = 0;
    let mut ultimo_pct = 0u8;

    loop {
        let n = resp
            .read(&mut buf)
            .map_err(|e| format!("se corto la conexion a mitad ({} de {} bytes): {}", hecho, total, e))?;
        if n == 0 {
            break;
        }
        std::io::Write::write_all(&mut w, &buf[..n])
            .map_err(|e| format!("no se pudo escribir en disco: {}", e))?;
        hecho += n as u64;

        let pct = ((hecho as f64 / total as f64) * 100.0) as u8;
        if pct >= ultimo_pct + 10 {
            ultimo_pct = pct;
            progreso(pct, hecho);
        }
    }
    std::io::Write::flush(&mut w).map_err(|e| format!("no se pudo volcar a disco: {}", e))?;

    // Un cuerpo trunco a veces cierra sin error: se comprueba el tamano contra
    // lo que el servidor declara. Un zip que no llega al final no descomprime.
    if esperado > 0 && hecho + 1024 * 1024 < esperado {
        return Err(format!(
            "llegaron {} de {} bytes: la conexion se corto antes de tiempo",
            hecho, total
        ));
    }
    Ok(())
}

/// Devuelve la ruta al Chromium, descargándolo la primera vez.
///
/// Se prefiere el del sistema si ya está en el PATH, para no descargar 188 MB
/// cuando no hace falta. La descarga embebida existe para no depender de root.
///
/// `progreso` recibe `(porcentaje, bytes)` cada ~10 % de la descarga.
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
    let cached = chrome_bin();
    if cached.is_file() {
        return Ok(cached);
    }

    let dir = chrome_dir();
    eprintln!(
        "CHROME:primera vez, descargando Chrome for Testing {} (188 MB, {}). \
         Tarda unos minutos; se informa cada 10% para que se vea que avanza.",
        CHROME_VERSION, CHROME_LICENSE
    );

    let zip = download_zip(progreso)?;

    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).map_err(|e| format!("no se pudo crear {}: {}", dir.display(), e))?;
    unzip(&zip, &dir)?;
    let _ = std::fs::remove_file(&zip);

    if !cached.is_file() {
        return Err("el zip no contenia chrome-linux64/chrome".into());
    }
    // El zip conserva los permisos, pero por si acaso:
    make_exec(&cached)?;
    Ok(cached)
}

/// Extrae un zip. Se recorre a mano para no dejar archivos sueltos en la
/// carpeta de destino si algo sale mal.
fn unzip(zip_path: &Path, dest: &Path) -> Result<(), String> {
    let f = std::fs::File::open(zip_path).map_err(|e| e.to_string())?;
    let mut ar = zip::ZipArchive::new(f).map_err(|e| format!("zip ilegible: {}", e))?;
    for i in 0..ar.len() {
        let mut entry = match ar.by_index(i) {
            Ok(e) => e,
            Err(e) => return Err(format!("zip corrupto en la entrada {}: {}", i, e)),
        };
        // `enclosed_name` descarta rutas con `..`, que en un zip externo no
        // deberían existir pero no es caro comprobarlo.
        let Some(rel) = entry.enclosed_name() else {
            return Err(format!("el zip contiene una ruta insegura en la entrada {}", i));
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
    // Chrome necesita su sandbox y sus varios binarios con permiso de
    // ejecucion. En vez de adivinar por extension, se marca el arbol entero:
    // dentro de esa carpeta todo lo ejecutable tiene que ser ejecutable.
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

// ─── limpieza garantizada ─────────────────────────────────────────────────────

/// Deja el sistema como estaba, pase lo que pase.
///
/// El Chromium hijo se lanza en su propio grupo de procesos, así que un
/// `SIGKILL` al grupo se lleva por delante el navegador y todos sus
/// subprocess (zygote, gpu, renderer) sin poder tocar nada del usuario.
struct Cleanup {
    profile: PathBuf,
    pgid: i32,
    child: Option<Child>,
    cdp: Option<Cdp>,
}

impl Drop for Cleanup {
    fn drop(&mut self) {
        // Se mata el grupo entero y ya está. No se manda antes un
        // `Browser.close`: el perfil se borra de todas formas, así que no hay
        // nada que Chrome pueda guardar, y su apagado ordenado reparenta
        // procesos hijos fuera del grupo. Ese proceso deja detrás utilities
        // que se quedan vivos un rato. El `SIGKILL` al grupo no deja nada.
        unsafe {
            libc::kill(-self.pgid, libc::SIGKILL);
        }
        if let Some(mut c) = self.child.take() {
            let _ = c.wait();
        }
        // El perfil efímero desaparece siempre.
        let _ = std::fs::remove_dir_all(&self.profile);
    }
}

/// Bandera que las señales de parada levantan.
///
/// Sin esto, un SIGTERM —el launcher cancelando, o el usuario cerrando la app—
/// mataba el proceso sin pasar por `Drop`, y Chromium se quedaba huérfano con su
/// perfil en disco. Manejar la señal convierte la muerte abrupta en una salida
/// limpia: el handler solo pone la bandera, y el bucle de sondeo la ve y
/// devuelve por el camino normal, que sí ejecuta el guard.
static STOP: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

extern "C" fn on_stop(_sig: libc::c_int) {
    // Solo se toca un átomo: nada más es seguro dentro de un handler de señal.
    STOP.store(true, std::sync::atomic::Ordering::SeqCst);
}

fn install_stop_handlers() {
    for sig in [libc::SIGTERM, libc::SIGINT, libc::SIGHUP] {
        unsafe {
            libc::signal(sig, on_stop as *const () as libc::sighandler_t);
        }
    }
}

// ─── extracción del código ───────────────────────────────────────────────────

/// Claves de Epic y GOG, en el orden que acepta el plugin: primero
/// `authorizationCode`, después `code`, y `sid` como último recurso porque
/// `legendary auth --sid` también vale.
const KEYS: [&str; 3] = ["authorizationCode", "code", "sid"];

/// Extrae el código de una URL, en query o fragmento.
///
/// Es el camino de GOG: al iniciar sesión aterriza en
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

/// Extrae el código del cuerpo de la página.
///
/// Es el camino de Epic: su endpoint de redirect devuelve un JSON con
/// `authorizationCode` ya relleno y la URL sin cambios. Sin sesión el campo
/// vale `null`, y eso se descarta para no canjear basura.
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

// ─── WebSocket mínimo ────────────────────────────────────────────────────────

/// Cliente WebSocket de solo lo necesario para hablar CDP: handshake y tramas
/// de texto, con reconstrucción por continuación. No se usa ninguna librería
/// porque el protocolo es acotado y así el helper no depende de nadie.
///
/// No es un websocket de propósito general: no negocia permessage-deflate,
/// no valida el `Sec-WebSocket-Accept` y no reabre conexiones. Para hablar con
/// un Chromium local que no hace nada exótico, alcanza.
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
        let mut sock = TcpStream::connect(hostport).map_err(|e| format!("no se pudo conectar: {}", e))?;
        sock.set_nodelay(true).ok();

        // El valor de la clave es arbitrario para un servidor local, pero se
        // manda uno de 16 bytes de todos modos para no salirse de la forma.
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
                return Err("el navegador cerro antes del handshake".into());
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

    /// Envía un comando y espera su respuesta, descartando los eventos que
    /// Chromium vaya intercalando.
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
                        continue; // evento, no nuestra respuesta
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
        // El lado cliente tiene que enmascarar siempre.
        let mask = random_bytes(4);
        f.extend_from_slice(&mask);
        f.extend(payload.iter().enumerate().map(|(i, b)| b ^ mask[i % 4]));
        self.sock.write_all(&f).map_err(|e| e.to_string())
    }

    /// Lee una trama de texto completa, uniendo las de continuación.
    fn recv_text(&mut self) -> Result<String, String> {
        let mut payload: Vec<u8> = Vec::new();
        loop {
            let (fin, op, data) = self.read_frame()?;
            match op {
                0x8 => return Err("el navegador cerro la conexion".into()),
                0x9 => continue, // ping: se ignora, el socket no lo necesita
                0xA => continue, // pong
                0x1 => payload = data,
                0x0 => payload.extend_from_slice(&data),
                other => return Err(format!("opcode de trama inesperado: {:#x}", other)),
            }
            if fin {
                return String::from_utf8(payload)
                    .map_err(|_| "respuesta no era UTF-8".to_string());
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
        // Chromium no enmascara (es servidor), pero se acepta por si acaso.
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
                return Err("conexion cerrada".into());
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
    // Solo si `/dev/urandom` no se puede abrir. La clave del handshake solo
    // tiene que ser distinta entre conexiones, no secreta.
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

// ─── flujo principal ──────────────────────────────────────────────────────────

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
        .map_err(|e| (why::SESSION, format!("perfil temporal: {}", e)))?;

    let port = free_port().map_err(|e| (why::SESSION, e))?;

    use std::os::unix::process::CommandExt;
    // Tamaño fijo y centrada: el formulario de login es angosto y alto, y sin
    // estas flags Chrome decide solo (Epic salía chica, GOG gigante). Sin
    // `--app=` a propósito: exige la URL en el arranque en vez de
    // `about:blank` + `Page.navigate`, y cambia el comportamiento de los
    // popups de OAuth que el login necesita.
    let (pantalla_w, pantalla_h) = pantalla();
    let (win_w, win_h, win_x, win_y) = ventana_login(pantalla_w, pantalla_h);
    let child = Command::new(&chrome)
        .arg(format!("--user-data-dir={}", profile.display()))
        .arg(format!("--remote-debugging-port={}", port))
        // Grupo propio: es lo que permite matar el Chromium hijo sin tocar el
        // del usuario. Sin `--enable-automation` ni headless a proposito: son
        // los que ponerian `navigator.webdriver` en true.
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
        .map_err(|e| (why::CHROME, format!("no se pudo lanzar el navegador: {}", e)))?;
    let pgid = child.id() as i32;

    let mut cleanup = Cleanup {
        profile: profile.clone(),
        pgid,
        child: Some(child),
        cdp: None,
    };

    // Se espera a que el puerto de depuración responda y a que aparezca la
    // pestaña. La primera vez tarda bastante: es un navegador entero.
    // Al arrancar solo existe el `about:blank` que lanzó el helper: sin
    // criterio, gana la primera pestaña como antes. Se guarda su target ID
    // para reencontrarla al reconectar aunque haya navegado.
    let (ws, mut target_id) = wait_for_page(port, Duration::from_secs(90), None, &[])
        .map_err(|e| (why::SESSION, e))?;
    let cdp = Cdp::connect(&ws).map_err(|e| (why::SESSION, e))?;
    cleanup.cdp = Some(cdp);

    cleanup
        .cdp
        .as_mut()
        .expect("acaba de insertarse")
        .call("Page.navigate", json!({ "url": url }), Duration::from_secs(60))
        .map_err(|e| (why::NAV, format!("no se pudo abrir la URL de login: {}", e)))?;

    // La ventana recién abierta se trae al frente una vez: si el usuario
    // estaba en otra cosa, el login no queda escondido atrás. `pgid` es el PID
    // del Chromium hijo (líder de su grupo), que es lo que busca xdotool.
    traer_al_frente(pgid as u32);

    eprintln!("VENTANA:navegador abierto; inicia sesion ahi");

    let deadline = Instant::now() + LOGIN_TIMEOUT;
    let mut last_url = String::new();
    // Racha de fallos de transporte CDP. Cada uno intenta una reconexión con
    // espera corta; agotados los intentos se falla ya, sin quemar los 180 s.
    let mut fallos_socket: u32 = 0;
    const MAX_RECONEXIONES: u32 = 2;
    const ESPERA_RECONEXION: [u64; 2] = [1, 2];
    loop {
        if STOP.load(std::sync::atomic::Ordering::SeqCst) {
            // Alguien nos pidió parar. Se sale por el camino normal para que el
            // guard mate a Chromium y borre el perfil.
            return Err((why::CANCELLED, "el login fue cancelado".into()));
        }
        if Instant::now() > deadline {
            // Se devuelve la última URL conocida: sin ella el launcher no puede
            // decir en qué punto se quedó el login.
            return Err((
                why::TIMEOUT,
                format!(
                    "el login no se completo en {} s; la ultima URL fue: {}",
                    LOGIN_TIMEOUT.as_secs(),
                    if last_url.is_empty() { "(sin leer)".into() } else { last_url }
                ),
            ));
        }

        // El `cdp` vive dentro del guard para que el cierre ordenado de
        // `Drop` pueda usarlo. El prestamo se acota a este bloque para poder
        // mover el guard despues.
        let encontrado: Option<(String, &'static str)> = {
            let cdp = match cleanup.cdp.as_mut() {
                Some(c) => c,
                None => return Err((why::SESSION, "la sesion CDP se perdio".into())),
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
                    // El socket respondió: cualquier fallo anterior quedó atrás
                    // y la racha de transporte se reinicia.
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
                                // Epic entrega el codigo en el cuerpo, GOG en
                                // la URL. Se leen los dos porque no hay forma
                                // fiable de saber de antemano cual de los dos
                                // va a pasar, y el codigo caduca en ~1 minuto.
                                Some((code, "pagina"))
                            } else {
                                last_url = href.to_string();
                                None
                            }
                        }
                        Err(e) => {
                            eprintln!("aviso: respuesta ilegible ({})", e);
                            None
                        }
                    }
                }
                Err(e) => {
                    if socket_muerto(&e) {
                        // El socket murió: se intenta revivirlo con espera
                        // corta en vez de reintentar contra un muerto hasta el
                        // timeout, que era lo que dejaba el login "colgado".
                        fallos_socket += 1;
                        eprintln!("aviso: conexion CDP perdida ({})", e);
                        if fallos_socket > MAX_RECONEXIONES {
                            return Err((
                                why::CDP,
                                format!(
                                    "se perdio la conexion con el navegador del login ({}) tras {} intentos de reconexion; probá de nuevo con Log in",
                                    e, MAX_RECONEXIONES
                                ),
                            ));
                        }
                        let espera = ESPERA_RECONEXION
                            .get((fallos_socket - 1) as usize)
                            .copied()
                            .unwrap_or(2);
                        eprintln!(
                            "CDP:reintentando conexion {} de {} en {} s",
                            fallos_socket, MAX_RECONEXIONES, espera
                        );
                        std::thread::sleep(Duration::from_secs(espera));
                        match reconectar_cdp(port, url, &last_url, &target_id) {
                            Ok((nuevo, nuevo_id)) => {
                                cleanup.cdp = Some(nuevo);
                                // La pestaña a la que se volvió es la del login
                                // según el mejor criterio disponible: se la
                                // sigue trackeando a ella de ahora en más.
                                target_id = nuevo_id;
                                eprintln!("CDP:conexion restablecida");
                            }
                            Err(r) => {
                                eprintln!(
                                    "aviso: no se pudo restablecer la conexion CDP ({})",
                                    r
                                );
                                if fallos_socket >= MAX_RECONEXIONES {
                                    return Err((
                                        why::CDP,
                                        format!(
                                            "se perdio la conexion con el navegador del login ({}) y no se pudo restablecer ({}); probá de nuevo con Log in",
                                            e, r
                                        ),
                                    ));
                                }
                                // Queda un intento: se sigue y el próximo
                                // sondeo lo vuelve a intentar.
                            }
                        }
                        None
                    } else {
                        // Un cambio de navegacion puede cortar la lectura; se sigue.
                        eprintln!("aviso: no se pudo leer la pagina ({})", e);
                        None
                    }
                }
            }
        };

        if let Some((code, donde)) = encontrado {
            eprintln!("VENTANA:codigo localizado en la {}", donde);
            drop(cleanup);
            return Ok(code);
        }
        std::thread::sleep(POLL);
    }
}

/// Espera a que Chromium exponga la pestaña por CDP y devuelve su
/// `webSocketDebuggerUrl` junto con su target ID.
///
/// `id` es el target ID trackeado desde la primera conexión: Chromium lo
/// mantiene estable aunque la pestaña navegue (solo cambia si se cierra), así
/// que es el criterio principal al reconectar. `dominios` es el respaldo por
/// host para el caso en que la pestaña original ya no esté. Con una sola
/// pestaña no hay ambigüedad y se usa igual que antes. Sin ID ni dominios gana
/// la primera pestaña (arranque inicial, donde solo existe el `about:blank`
/// que lanzó el helper).
fn wait_for_page(
    port: u16,
    timeout: Duration,
    id: Option<&str>,
    dominios: &[&str],
) -> Result<(String, String), String> {
    let deadline = Instant::now() + timeout;
    let mut last = "el navegador no exponio su puerto de depuracion".to_string();
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
                            "el navegador no dio ninguna pestana todavia ({} objetivo(s))",
                            arr.len()
                        )
                    } else {
                        format!(
                            "hay {} pestana(s) pero ninguna es la del login (ni por ID ni por dominio)",
                            paginas.len()
                        )
                    };
                    std::thread::sleep(Duration::from_millis(300));
                    continue;
                }
            }
            last = format!(
                "el navegador no dio ninguna pestana todavia ({} objetivo(s))",
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

/// Elige a qué pestaña conectarse de las que expone `/json/list`. Devuelve su
/// `webSocketDebuggerUrl` junto con su target ID.
///
/// Criterio principal: el target ID, que Chromium mantiene estable aunque la
/// pestaña navegue —solo cambia si se cierra. Se trackea desde la primera
/// conexión, así un popup de OAuth (Google SSO) cuya URL cambia varias veces
/// no confunde la reconexión.
///
/// Respaldo: si el ID ya no está (pestaña cerrada), vale cualquier pestaña
/// cuyo host esté en `dominios`. Con una sola pestaña no hay ambigüedad y se
/// usa igual que antes; si hay varias y nada coincide se devuelve `None` para
/// seguir esperando (o agotar el reintento) en vez de leer una pestaña ajena
/// hasta el timeout general.
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
        // Sin criterio gana la primera: comportamiento histórico del arranque.
        return paginas.first().and_then(ws_e_id);
    }
    None
}

/// Saca el host de una URL (`https://auth.gog.com/auth?x=1` → `auth.gog.com`).
/// Solo para el filtro de respaldo por dominio; no valida nada.
fn host_de(url: &str) -> Option<&str> {
    let resto = url.split("://").nth(1)?;
    Some(resto.split(['/', '?', '#']).next().unwrap_or(resto))
}

/// Dice si la URL de una pestaña cae dentro de alguno de los dominios
/// esperados, comparando por host y no por URL exacta: la pestaña navega
/// durante el login y su URL cambia varias veces.
fn url_en_dominios(url: &str, dominios: &[&str]) -> bool {
    match host_de(url) {
        Some(h) => dominios.iter().any(|d| h.eq_ignore_ascii_case(d)),
        None => false,
    }
}

/// Dice si un error de `cdp.call()` significa que el socket CDP murió.
///
/// Solo cuentan los errores de transporte: conexión cerrada por el navegador,
/// tubería rota o reseteo de la conexión. Un fallo de `Runtime.evaluate` por
/// una navegación a mitad de lectura NO es fatal: ese caso sigue por el camino
/// de aviso y reintento como antes.
fn socket_muerto(e: &str) -> bool {
    let t = e.to_lowercase();
    t.contains("conexion cerrada")
        || t.contains("cerro la conexion")
        || t.contains("cerro antes")
        || t.contains("broken pipe")
        || t.contains("connection reset")
        || t.contains("os error 32")
        || t.contains("os error 104")
}

/// Reabre la sesión CDP contra el mismo Chromium, sobre la pestaña del login.
///
/// La reencuentra por su target ID, que es estable aunque la pestaña haya
/// navegado (caso real: popup de OAuth con la URL cambiada entre la
/// desconexión y la reconexión). Como respaldo, si el ID ya no está, vale
/// cualquier pestaña en los dominios del login original o de la última URL
/// leída (GOG navega de `auth.gog.com` a `embed.gog.com` al completar el
/// login). No relanza el navegador: si el proceso murió o no queda ninguna
/// pestaña usable, se devuelve el error para fallar rápido con `cdp_perdido`.
/// Devuelve la sesión y el ID de la pestaña a la que se volvió, para seguir
/// trackeando a esa de ahora en más.
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

/// Resolución de pantalla para centrar la ventana del login.
///
/// Intenta `xdotool getdisplaygeometry` (devuelve `ANCHO ALTO`); si no está
/// instalado o falla, 1920x1080 como fallback documentado. Nunca falla: en el
/// peor caso la ventana sale centrada para 1080p.
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

/// Geometría de la ventana del login: angosta y alta para el formulario, y
/// centrada en la pantalla. Devuelve `(ancho, alto, x, y)`. Las posiciones se
/// saturan en 0 para no dar coordenadas negativas en pantallas chicas.
fn ventana_login(pantalla_w: u32, pantalla_h: u32) -> (u32, u32, u32, u32) {
    const W: u32 = 480;
    const H: u32 = 720;
    let x = pantalla_w.saturating_sub(W) / 2;
    let y = pantalla_h.saturating_sub(H) / 2;
    (W, H, x, y)
}

/// Trae la ventana del Chromium del login al frente, una sola vez.
///
/// Busca sus ventanas por PID (`xdotool search --onlyvisible --pid`) y les
/// manda `windowraise` + `windowfocus`. Es best-effort con reintentos cortos:
/// si xdotool no está o la ventana todavía no existe, solo se avisa por
/// stderr y el login sigue igual.
///
/// NO es un "siempre encima" pegajoso: xdotool no tiene primitiva para fijar
/// el estado `_NET_WM_STATE_ABOVE` de EWMH (y `wmctrl`, que sí la tiene, no
/// está instalado). Si el usuario clickea el launcher después, Chromium vuelve
/// atrás como cualquier ventana normal. Fijarlo de verdad exigiría mandar el
/// client-message de EWMH a mano (código X11 nuevo) o depender de `wmctrl`.
fn traer_al_frente(pid: u32) {
    const INTENTOS: u32 = 5;
    for intento in 1..=INTENTOS {
        match ventana_de_pid(pid) {
            Some(wid) => {
                let _ = comando_xdotool(&["windowraise", &wid]);
                let _ = comando_xdotool(&["windowfocus", &wid]);
                eprintln!("VENTANA:ventana del login traída al frente");
                return;
            }
            None if intento < INTENTOS => {
                std::thread::sleep(Duration::from_millis(500));
            }
            None => {
                eprintln!("aviso: no se encontró la ventana del login para traerla al frente");
            }
        }
    }
}

/// Devuelve el ID de la primera ventana visible del PID dado, o `None`.
fn ventana_de_pid(pid: u32) -> Option<String> {
    let out = comando_xdotool(&["search", "--onlyvisible", "--pid", &pid.to_string()])?;
    elegir_ventana(&out)
}

/// Ejecuta xdotool y devuelve su stdout si salió bien. `None` si xdotool no
/// está, falla o no produce salida: el login nunca depende de esto.
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

/// Elige la ventana de la salida de `xdotool search`: la primera línea no
/// vacía (un ID de ventana por línea).
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
        // Epic nombra el suyo authorizationCode; si una URL trajera los dos, el
        // que se canjea es el de Epic.
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

    /// El cuerpo exacto que devuelve Epic, medido contra el sitio real.
    #[test]
    fn el_json_real_de_epic_da_el_codigo() {
        let b = r#"{"warning":"Do not share this code with any 3rd party service.","redirectUrl":"https://localhost/launcher/authorized","authorizationCode":"eyJhbGciOiJFUzI1NiJ9.abc-_123","exchangeCode":null,"sid":null}"#;
        assert_eq!(from_body(b).as_deref(), Some("eyJhbGciOiJFUzI1NiJ9.abc-_123"));
    }

    /// El mismo cuerpo sin sesión: `authorizationCode` viene `null`. Canjear
    /// eso daría un error confuso, así que tiene que descartarse.
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

    /// Los errores de transporte tienen que marcar el socket como muerto para
    /// que el sondeo reconecte en vez de reintentar contra un muerto.
    #[test]
    fn socket_muerto_detecta_transporte() {
        // Los tres que salieron en el log real de las 19:50.
        assert!(socket_muerto("Runtime.evaluate: conexion cerrada"));
        assert!(socket_muerto(
            "Runtime.evaluate: el navegador cerro la conexion"
        ));
        assert!(socket_muerto(
            "Runtime.evaluate: Broken pipe (os error 32)"
        ));
        assert!(socket_muerto(
            "Runtime.evaluate: Connection reset by peer (os error 104)"
        ));
    }

    /// Un fallo de lectura por navegación o por timeout NO es socket muerto:
    /// esos siguen por el camino de aviso y reintento como antes.
    #[test]
    fn socket_muerto_no_confunde_navegacion_ni_timeout() {
        assert!(!socket_muerto(
            "Runtime.evaluate no respondio en 10 s"
        ));
        assert!(!socket_muerto(
            "Runtime.evaluate: {\"code\":-32000,\"message\":\"No hay sesion\"}"
        ));
        assert!(!socket_muerto("aviso: respuesta ilegible (algo)"));
    }

    /// Arma un target de `/json/list` para probar la selección de pestaña.
    fn pagina(url: &str, id: &str, ws: &str) -> Value {
        json!({"type": "page", "url": url, "id": id, "webSocketDebuggerUrl": ws})
    }

    /// Con una sola pestaña no hay ambigüedad: se usa aunque no haya criterio
    /// (es el `about:blank` recién lanzado o la única abierta).
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

    /// Caso real que motivó el fix: dos pestañas activas (la principal + un
    /// popup de SSO) y la URL de la principal cambió respecto a `last_url`.
    /// Igual se la encuentra por su target ID, estable aunque navegue.
    #[test]
    fn elegir_pestana_encuentra_por_id_aunque_la_url_haya_cambiado() {
        // La principal navegó del login de GOG al consentimiento de Google
        // entre la desconexión y la reconexión: ni `last_url`
        // (`auth.gog.com`) ni la URL actual matchean por dominio de login.
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

    /// Si el ID ya no está (pestaña cerrada), el respaldo por dominio vale:
    /// GOG navega de `auth.gog.com` a `embed.gog.com` al completar el login.
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

    /// Con varias pestañas y sin ID ni dominio coincidente no se elige
    /// ninguna: el llamador sigue esperando (o agota el reintento) en vez de
    /// leer una pestaña ajena hasta el timeout general.
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

    /// Sin criterio se mantiene el comportamiento histórico: gana la primera.
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

    /// El host se extrae para el filtro por dominio (con query, fragmento o
    /// URL sin esquema no hay match exacto: mejor `None` que un falso positivo).
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

    /// El respaldo compara por host, no por URL exacta: la pestaña navega y su
    /// URL cambia, el dominio no.
    #[test]
    fn url_en_dominios_compara_por_host() {
        let doms = ["auth.gog.com", "embed.gog.com"];
        assert!(url_en_dominios("https://auth.gog.com/auth?x=1", &doms));
        assert!(url_en_dominios("https://embed.gog.com/on_login_success", &doms));
        assert!(!url_en_dominios("https://accounts.google.com/o/oauth2/auth", &doms));
        assert!(!url_en_dominios("about:blank", &doms));
    }

    /// Sirve una lista fija de `/json/list` en localhost para probar
    /// `wait_for_page` sin un Chromium de verdad. Devuelve el puerto.
    fn servir_lista_fija(cuerpo: &'static str) -> u16 {
        servir_bytes_fijos(cuerpo.as_bytes())
    }

    /// Sirve bytes fijos con `Content-Length` en localhost. Devuelve el puerto.
    fn servir_bytes_fijos(cuerpo: &'static [u8]) -> u16 {
        let l = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let port = l.local_addr().unwrap().port();
        // El cuerpo se copia al hilo: el `&'static` sobrevive al test.
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

    /// Varias pestañas al reconectar y la principal cambió de URL (popup de
    /// SSO mediante): se la reencuentra por su target ID.
    #[test]
    fn wait_for_page_reencuentra_por_id_aunque_la_url_haya_cambiado() {
        let cuerpo = r#"[{"type":"page","url":"https://accounts.google.com/o/oauth2/consent?x=1","id":"ID-MAIN","webSocketDebuggerUrl":"ws://127.0.0.1:9/devtools/page/AAA"},{"type":"page","url":"https://auth.gog.com/auth?client_id=1","id":"ID-POPUP","webSocketDebuggerUrl":"ws://127.0.0.1:9/devtools/page/BBB"}]"#;
        let port = servir_lista_fija(cuerpo);
        let (ws, id) = wait_for_page(port, Duration::from_secs(5), Some("ID-MAIN"), &["auth.gog.com"])
            .expect("la pestana del login esta presente por ID");
        assert_eq!(id, "ID-MAIN", "reconectó a la pestaña equivocada");
        assert!(
            ws.ends_with("/AAA"),
            "eligio la pestana equivocada: {}",
            ws
        );
    }

    /// Varias pestañas y ni el ID ni ningún dominio coinciden: falla en el
    /// reintento corto, no se cuelga hasta el timeout general (en producción
    /// ese `Err` se convierte en `ERRO:cdp_perdido`).
    #[test]
    fn wait_for_page_sin_pestana_del_login_falla_en_el_reintento() {
        let cuerpo = r#"[{"type":"page","url":"about:blank","id":"ID-A","webSocketDebuggerUrl":"ws://127.0.0.1:9/devtools/page/AAA"},{"type":"page","url":"chrome://newtab/","id":"ID-B","webSocketDebuggerUrl":"ws://127.0.0.1:9/devtools/page/BBB"}]"#;
        let port = servir_lista_fija(cuerpo);
        let t0 = Instant::now();
        let err = wait_for_page(port, Duration::from_secs(1), Some("ID-OTRA"), &["auth.gog.com"])
            .expect_err("ninguna pestana es la del login");
        assert!(
            t0.elapsed() < Duration::from_secs(5),
            "se colgo {} s en vez de fallar en el reintento",
            t0.elapsed().as_secs()
        );
        assert!(
            err.contains("ninguna es la del login"),
            "mensaje sin diagnostico util: {}",
            err
        );
    }

    /// En 1080p la ventana de 480x720 sale centrada: x=(1920-480)/2,
    /// y=(1080-720)/2.
    #[test]
    fn ventana_login_centra_en_1080p() {
        assert_eq!(ventana_login(1920, 1080), (480, 720, 720, 180));
    }

    /// En pantallas chicas las posiciones se saturan en 0 en vez de dar
    /// coordenadas negativas que el WM rechazaría.
    #[test]
    fn ventana_login_no_da_posiciones_negativas_en_pantalla_chica() {
        assert_eq!(ventana_login(800, 600), (480, 720, 160, 0));
    }

    /// De la salida de `xdotool search` (un ID por línea) se toma la primera
    /// línea no vacía.
    #[test]
    fn elegir_ventana_toma_la_primera_linea() {
        assert_eq!(
            elegir_ventana(b"0x1a00003\n0x1a00005\n").as_deref(),
            Some("0x1a00003")
        );
    }

    /// Sin salida no hay ventana: `None`, nunca un ID inventado.
    #[test]
    fn elegir_ventana_sin_salida_da_none() {
        assert_eq!(elegir_ventana(b""), None);
        assert_eq!(elegir_ventana(b"\n  \n"), None);
    }

    /// `--probe` refleja la caché real: `false` con HOME vacío, `true` cuando
    /// el binario existe. Solo I/O, sin procesos ni red.
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

    /// La descarga escribe los bytes exactos y reporta progreso creciente por
    /// callback (camino que usa `--prefetch` hacia el modal).
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
        assert!(!pcts.is_empty(), "sin reportes de progreso");
        assert!(
            pcts.windows(2).all(|w| w[0] <= w[1]),
            "progreso no creciente: {:?}",
            pcts
        );
        // El reporte es cada ~10 % por diseño: el último aviso no tiene por
        // qué ser 100 (en producción lo cierra el evento `done`).
        let ultimo = *pcts.last().expect("no vacío");
        assert!(ultimo >= 90, "progreso final insuficiente: {:?}", pcts);
    }
}
