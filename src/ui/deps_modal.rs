//! The mandatory "Install dependencies" modal for Stores.
//!
//! It lists only the missing tools (legendary, gogdl, the login Chromium),
//! with a single "Install All" button and no X and no "Close": while something
//! is missing, Stores can't be used. After ≥1 failed attempt the "Skip for now"
//! link appears and closes without installing anything (the UI then shows the
//! "Setup incomplete" state with a button to reopen).
//!
//! Total success closes by itself: a modal with no way out would be a bug, not
//! a design decision.

use adw::prelude::*;
use std::rc::Rc;

/// Which tool is missing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DepId {
    Legendary,
    Gogdl,
    Chromium,
}

/// Each dependency's card: name, description and measured size.
pub struct DepInfo {
    pub id: DepId,
    pub nombre: &'static str,
    pub descripcion: &'static str,
    pub peso: &'static str,
}

pub const DEPS: [DepInfo; 3] = [
    DepInfo {
        id: DepId::Legendary,
        nombre: "legendary",
        descripcion: "Epic Games client: library and sign-in.",
        peso: "~3 MB",
    },
    DepInfo {
        id: DepId::Gogdl,
        nombre: "gogdl",
        descripcion: "GOG client: library and sign-in.",
        peso: "~1.5 MB",
    },
    DepInfo {
        id: DepId::Chromium,
        nombre: "Login browser",
        descripcion: "Isolated Chromium for sign-in only. It never touches your own browser.",
        peso: "~188 MB",
    },
];

/// How the modal ended (it always closes through one of these two ways).
pub enum InstallOutcome {
    /// All ready: the caller refreshes and the modal already closed itself.
    TodoOk,
    /// Out without installing: show "Setup incomplete".
    Skip,
}

/// Mensajes del hilo instalador al hilo de GTK.
enum Avance {
    FilaActiva(usize),
    FilaProgreso(usize, u8, u64),
    FilaLista(usize),
    FilaFallo(usize, String),
    Terminado,
}

#[derive(Clone)]
struct FilaWidgets {
    spin: gtk::Spinner,
    estado: gtk::Label,
}

/// Logical state of a row, mirrored from the widget for GTK-free decisions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FilaEstado {
    Espera,
    Activa,
    Lista,
    Fallo,
}

/// The "Skip for now" link shows as soon as ANY row is Failed, whatever the
/// other rows do (in progress, success, another failure). It doesn't wait for
/// all of them: that wait was the bug (a row in progress blocked the link even
/// after another one had already failed).
fn mostrar_skip(estados: &[FilaEstado]) -> bool {
    estados.iter().any(|s| *s == FilaEstado::Fallo)
}

/// Fixed neon green for "Ready": hardcoded on purpose, it does NOT follow the
/// light/dark theme (explicit design decision). Registered once with its own
/// provider, so no theme change can touch it.
const READY_CSS: &str = ".deps-ready { color: #39FF14; }";

fn asegurar_css_ready() {
    use std::sync::OnceLock;
    static LISTO: OnceLock<()> = OnceLock::new();
    LISTO.get_or_init(|| {
        if let Some(display) = gdk::Display::default() {
            let p = gtk::CssProvider::new();
            p.load_from_string(READY_CSS);
            gtk::style_context_add_provider_for_display(
                &display,
                &p,
                gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
            );
        }
    });
}
/// Maps a free-form backend error to a short English reason for the row.
///
/// `cmd_setup` texts are unformatted Python exceptions and `--prefetch` ones
/// are human messages in Spanish, so I classify them by keyword and, when
/// nothing matches, show the raw text truncated. I never invent a diagnosis.
/// The Spanish keywords below are the plugin's own wording — they stay on
/// purpose, that's what the tool actually prints.
pub fn motivo_corto(origen: &str, texto: &str) -> String {
    let t = format!("{} {}", origen, texto).to_lowercase();
    for clave in [
        "temporary failure",
        "name or service",
        "urlerror",
        "connection",
        "timed out",
        "network",
        "unreachable",
        "cut off",
        "rejected the download",
        // Plugin wording: heroic-store and the RPG runtime still answer in
        // Spanish, so I keep matching their own words.
        "red al conectar",
        "conexi",
        "cortó",
        "rechazo la descarga",
    ] {
        if t.contains(clave) {
            return "no connection".to_string();
        }
    }
    for clave in ["no space", "disk space", "errno 28", "espacio en disco", "sin espacio"] {
        if t.contains(clave) {
            return "no disk space".to_string();
        }
    }
    let crudo: String = texto.trim().chars().take(90).collect();
    if crudo.is_empty() {
        "unknown error".to_string()
    } else {
        crudo
    }
}

/// Installs what's pending in its own thread: legendary/gogdl first through
/// `cmd_setup` (which skips whatever is already there), then Chromium through
/// `webdriver_login --prefetch` with JSON progress on stdout.
fn instalar(
    pendientes: Vec<DepId>,
    helper: Option<std::path::PathBuf>,
    tx: std::sync::mpsc::Sender<Avance>,
) {
    let enviar = |m: Avance| {
        let _ = tx.send(m);
    };
    let idx_de = |id: DepId| pendientes.iter().position(|x| *x == id);

    let hay_leg = pendientes.contains(&DepId::Legendary);
    let hay_gog = pendientes.contains(&DepId::Gogdl);
    if hay_leg || hay_gog {
        for id in [DepId::Legendary, DepId::Gogdl] {
            if let Some(i) = idx_de(id) {
                enviar(Avance::FilaActiva(i));
            }
        }
        match crate::backend::external::StoreManager::setup() {
            Ok(v) => {
                let fallos = v
                    .get("failed")
                    .and_then(|x| x.as_array())
                    .map(|a| {
                        a.iter().filter_map(|x| x.as_str()).collect::<Vec<_>>()
                    })
                    .unwrap_or_default();
                for (id, nombre) in [(DepId::Legendary, "legendary"), (DepId::Gogdl, "gogdl")] {
                    // `idx_de` only returns `Some` for a pending item.
                    let Some(i) = idx_de(id) else { continue };
                    match fallos.iter().find(|e| e.starts_with(nombre)) {
                        Some(e) => enviar(Avance::FilaFallo(i, motivo_corto("setup", e))),
                        None => enviar(Avance::FilaLista(i)),
                    }
                }
            }
            Err(e) => {
                for id in [DepId::Legendary, DepId::Gogdl] {
                    if let Some(i) = idx_de(id) {
                        if pendientes.contains(&id) {
                            enviar(Avance::FilaFallo(i, motivo_corto("setup", &e)));
                        }
                    }
                }
            }
        }
    }

    if pendientes.contains(&DepId::Chromium) {
        let Some(i) = idx_de(DepId::Chromium) else {
            enviar(Avance::Terminado);
            return;
        };
        enviar(Avance::FilaActiva(i));
        let Some(bin) = helper else {
            enviar(Avance::FilaFallo(i, "login helper missing".to_string()));
            enviar(Avance::Terminado);
            return;
        };
        let mut hijo = match std::process::Command::new(&bin)
            .arg("--prefetch")
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
        {
            Ok(c) => c,
            Err(e) => {
                enviar(Avance::FilaFallo(i, motivo_corto("chrome", &e.to_string())));
                enviar(Avance::Terminado);
                return;
            }
        };
        use std::io::{BufRead, Read};
        let mut visto_done = false;
        if let Some(salida) = hijo.stdout.take() {
            for linea in std::io::BufReader::new(salida).lines() {
                let Ok(linea) = linea else { break };
                let Ok(v) = serde_json::from_str::<serde_json::Value>(&linea) else {
                    continue;
                };
                match v.get("type").and_then(|x| x.as_str()) {
                    Some("chrome_progress") => {
                        let pct = v.get("percent").and_then(|x| x.as_u64()).unwrap_or(0) as u8;
                        let mb = v.get("mb").and_then(|x| x.as_u64()).unwrap_or(0);
                        enviar(Avance::FilaProgreso(i, pct, mb));
                    }
                    Some("chrome_done") => {
                        visto_done = true;
                        enviar(Avance::FilaLista(i));
                    }
                    _ => {}
                }
            }
        }
        let mut resto = String::new();
        if let Some(mut err) = hijo.stderr.take() {
            let _ = err.read_to_string(&mut resto);
        }
        match hijo.wait() {
            Ok(st) if st.success() => {
                if !visto_done {
                    enviar(Avance::FilaLista(i));
                }
            }
            _ => {
                let msg = resto
                    .lines()
                    .rev()
                    .find(|l| l.contains("ERRO:"))
                    .map(|l| {
                        l.splitn(3, ':').nth(2).unwrap_or(l).trim().to_string()
                    })
                    .unwrap_or_else(|| resto.trim().to_string());
                enviar(Avance::FilaFallo(i, motivo_corto("chrome", &msg)));
            }
        }
        enviar(Avance::Terminado);
    } else {
        enviar(Avance::Terminado);
    }
}

/// Abre el modal con las filas pendientes (`pendientes` en orden DEPS).
///
/// No X and no close button: the only exits are total success (auto-close +
/// `InstallOutcome::TodoOk`) and the "Skip for now" link, which only shows
/// after ≥1 failed attempt (`InstallOutcome::Skip`).
pub fn present(
    parent: &impl IsA<gtk::Widget>,
    pendientes: Vec<DepId>,
    helper: Option<std::path::PathBuf>,
    terminado: impl Fn(InstallOutcome) + 'static,
) {
    if pendientes.is_empty() {
        return;
    }
    let dialog = adw::Dialog::new();
    dialog.set_title("Install dependencies");
    dialog.set_content_width(480);
    // No gesture exit: no X (there's no button) and no Esc. Careful: with
    // can-close false, `close()` is BLOCKED even programmatically (libadwaita
    // semantics), so every exit uses `force_close()`.
    dialog.set_can_close(false);
    // Modality/transience comes from `present(parent)`: AdwDialog isn't a
    // GtkWindow and has no set_transient_for/set_modal.

    let content = gtk::Box::new(gtk::Orientation::Vertical, 0);
    content.add_css_class("modal-bg");
    content.set_hexpand(true);
    content.set_vexpand(true);
    let inner = gtk::Box::new(gtk::Orientation::Vertical, 10);
    inner.set_hexpand(true);
    inner.set_vexpand(true);
    inner.set_margin_top(12);
    inner.set_margin_bottom(12);
    inner.set_margin_start(16);
    inner.set_margin_end(16);
    content.append(&inner);

    // Header without X: same title and separator as `modal_header`.
    let titulo = gtk::Label::new(Some("Install dependencies"));
    titulo.add_css_class("modal-title");
    titulo.set_halign(gtk::Align::Start);
    titulo.set_margin_start(24);
    titulo.set_margin_end(16);
    titulo.set_margin_top(16);
    inner.append(&titulo);
    let sep = gtk::Separator::new(gtk::Orientation::Horizontal);
    sep.set_halign(gtk::Align::Fill);
    inner.append(&sep);

    let intro = gtk::Label::new(Some("Stores needs these tools to work. They are downloaded once."));
    intro.set_halign(gtk::Align::Start);
    intro.set_wrap(true);
    inner.append(&intro);

    let lista = gtk::ListBox::new();
    lista.set_selection_mode(gtk::SelectionMode::None);
    inner.append(&lista);

    let mut filas: Vec<FilaWidgets> = Vec::new();
    for id in &pendientes {
        let info = DEPS.iter().find(|d| d.id == *id).expect("DepId cubierto por DEPS");
        let fila = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        fila.set_margin_top(4);
        fila.set_margin_bottom(4);
        fila.set_margin_start(4);
        fila.set_margin_end(4);
        let textos = gtk::Box::new(gtk::Orientation::Vertical, 2);
        textos.set_hexpand(true);
        let nombre = gtk::Label::new(Some(info.nombre));
        nombre.set_halign(gtk::Align::Start);
        textos.append(&nombre);
        let desc = gtk::Label::new(Some(info.descripcion));
        desc.set_halign(gtk::Align::Start);
        desc.set_wrap(true);
        desc.add_css_class("dim-label");
        textos.append(&desc);
        fila.append(&textos);
        let peso = gtk::Label::new(Some(info.peso));
        peso.set_halign(gtk::Align::End);
        peso.set_valign(gtk::Align::Center);
        peso.add_css_class("dim-label");
        fila.append(&peso);
        let spin = gtk::Spinner::new();
        spin.set_valign(gtk::Align::Center);
        fila.append(&spin);
        let estado = gtk::Label::new(Some("Waiting…"));
        estado.set_valign(gtk::Align::Center);
        estado.add_css_class("dim-label");
        fila.append(&estado);
        lista.append(&fila);
        filas.push(FilaWidgets { spin, estado });
    }

    let barra = gtk::ProgressBar::new();
    barra.set_show_text(true);
    barra.set_text(Some("0 of 0"));
    inner.append(&barra);

    let instalar_btn = gtk::Button::with_label("Install All");
    instalar_btn.add_css_class("suggested-action");
    instalar_btn.set_hexpand(true);
    inner.append(&instalar_btn);

    // Escape valve: a small link, only after ≥1 failed attempt. It doesn't
    // compete with "Install All" and never shows on the first render.
    let skip = gtk::Button::with_label("Skip for now");
    skip.add_css_class("flat");
    skip.add_css_class("dim-label");
    skip.set_halign(gtk::Align::Center);
    skip.set_visible(false);
    inner.append(&skip);

    dialog.set_child(Some(&content));
    asegurar_css_ready();

    let terminado = Rc::new(terminado);
    let dlg_vivo: Rc<std::cell::Cell<bool>> = Rc::new(std::cell::Cell::new(true));
    {
        let vivo = dlg_vivo.clone();
        dialog.connect_closed(move |_| {
            vivo.set(false);
        });
    }
    let (tx, rx) = std::sync::mpsc::channel::<Avance>();
    let rx = Rc::new(std::cell::RefCell::new(rx));

    // Estado compartido con el poller.
    struct Estado {
        filas: Vec<FilaWidgets>,
        estados: Vec<FilaEstado>,
        barra: gtk::ProgressBar,
        instalar_btn: gtk::Button,
        skip: gtk::Button,
        listas: usize,
        total: usize,
        fallos_acumulados: u32,
        instalando: bool,
    }
    let estado = Rc::new(std::cell::RefCell::new(Estado {
        filas,
        estados: vec![FilaEstado::Espera; pendientes.len()],
        barra: barra.clone(),
        instalar_btn: instalar_btn.clone(),
        skip: skip.clone(),
        listas: 0,
        total: pendientes.len(),
        fallos_acumulados: 0,
        instalando: false,
    }));

    let lanzar = {
        let estado = estado.clone();
        let pendientes = pendientes.clone();
        let helper = helper.clone();
        move || {
            let mut e = estado.borrow_mut();
            if e.instalando {
                return;
            }
            e.instalando = true;
            e.listas = 0;
            e.instalar_btn.set_sensitive(false);
            for i in 0..e.filas.len() {
                if let Some(f) = e.filas.get(i) {
                    f.spin.stop();
                    f.spin.set_visible(false);
                    f.estado.remove_css_class("deps-ready");
                    f.estado.set_text("Waiting…");
                }
                // Intento fresco, estados frescos. El link de skip NO se
                // oculta: una vez ganado por un fallo anterior, queda.
                if let Some(s) = e.estados.get_mut(i) {
                    *s = FilaEstado::Espera;
                }
            }
            e.barra.set_fraction(0.0);
            e.barra.set_text(Some(&format!("0 of {}", e.total)));
            let tx_hilo = tx.clone();
            let pendientes_hilo = pendientes.clone();
            let helper_hilo = helper.clone();
            drop(e);
            std::thread::spawn(move || {
                instalar(pendientes_hilo, helper_hilo, tx_hilo);
            });
        }
    };
    let lanzar = Rc::new(lanzar);
    {
        let l = lanzar.clone();
        instalar_btn.connect_clicked(move |_| {
            l();
        });
    }
    // No autostart: installing begins ONLY on pressing "Install All". (An
    // earlier version called `lanzar()` here, so the modal installed itself on
    // open — which also made the skip link appear before any attempt.)
    {
        let t = terminado.clone();
        let d = dialog.clone();
        skip.connect_clicked(move |_| {
            d.force_close();
            t(InstallOutcome::Skip);
        });
    }

    // Poller like ProtonModal's: drains the channel every 100 ms and dies with
    // the dialog.
    {
        let estado = estado.clone();
        let rx_poll = rx.clone();
        let vivo_poll = dlg_vivo.clone();
        let t = terminado.clone();
        let d = dialog.clone();
        glib::timeout_add_local(std::time::Duration::from_millis(100), move || {
            if !vivo_poll.get() {
                return glib::ControlFlow::Break;
            }
            let mensajes: Vec<Avance> = {
                let r = rx_poll.borrow();
                let mut v = Vec::new();
                while let Ok(m) = r.try_recv() {
                    v.push(m);
                }
                v
            };
            for m in mensajes {
                let mut e = estado.borrow_mut();
                match m {
                    Avance::FilaActiva(i) => {
                        if let Some(f) = e.filas.get(i) {
                            f.spin.set_visible(true);
                            f.spin.start();
                            f.estado.remove_css_class("deps-ready");
                            f.estado.set_text("Downloading…");
                        }
                        if let Some(s) = e.estados.get_mut(i) {
                            *s = FilaEstado::Activa;
                        }
                    }
                    Avance::FilaProgreso(i, pct, mb) => {
                        if let Some(f) = e.filas.get(i) {
                            f.estado.set_text(&format!("{}% ({} MB)", pct, mb));
                        }
                    }
                    Avance::FilaLista(i) => {
                        if let Some(f) = e.filas.get(i) {
                            f.spin.stop();
                            f.spin.set_visible(false);
                            f.estado.set_text("Ready");
                            f.estado.add_css_class("deps-ready");
                        }
                        if let Some(s) = e.estados.get_mut(i) {
                            *s = FilaEstado::Lista;
                        }
                        e.listas += 1;
                        e.barra.set_fraction(e.listas as f64 / e.total.max(1) as f64);
                        e.barra.set_text(Some(&format!("{} of {}", e.listas, e.total)));
                    }
                    Avance::FilaFallo(i, motivo) => {
                        if let Some(f) = e.filas.get(i) {
                            f.spin.stop();
                            f.spin.set_visible(false);
                            f.estado.remove_css_class("deps-ready");
                            f.estado.set_text(&format!("Failed: {}", motivo));
                        }
                        if let Some(s) = e.estados.get_mut(i) {
                            *s = FilaEstado::Fallo;
                        }
                        // Timing fix: the link shows on the first failure,
                        // without waiting for the rest. Monotonic: once visible
                        // it never hides (rule I settled on).
                        if mostrar_skip(&e.estados) {
                            e.skip.set_visible(true);
                        }
                    }
                    Avance::Terminado => {
                        e.instalando = false;
                        e.instalar_btn.set_sensitive(true);
                        if e.listas >= e.total {
                            e.barra.set_text(Some("All dependencies installed."));
                            // Total success: auto-close. It's the only clean
                            // exit; staying open with nothing left to do is
                            // the trap this design avoids. `force_close`:
                            // `close()` is blocked by can-close false (that was
                            // the auto-close bug).
                            let dd = d.clone();
                            let tt = t.clone();
                            glib::timeout_add_local_once(
                                std::time::Duration::from_millis(900),
                                move || {
                                    dd.force_close();
                                    tt(InstallOutcome::TodoOk);
                                },
                            );
                        } else {
                            e.fallos_acumulados += 1;
                            e.barra.set_text(Some("Press Install All to retry."));
                            // The link already showed on the first failure
                            // (`mostrar_skip` rule); nothing is decided here.
                        }
                    }
                }
            }
            glib::ControlFlow::Continue
        });
    }

    dialog.present(Some(parent));
}

#[cfg(test)]
#[allow(non_snake_case)]
mod tests {
    use super::*;

    #[test]
    fn motivo_corto_detecta_sin_conexion() {
        assert_eq!(motivo_corto("setup", "gogdl: <urlopen error [Errno -2] Name or service not known>"), "no connection");
        // Plugin wording, still matched.
        assert_eq!(motivo_corto("chrome", "fallo de red al conectar: bla"), "no connection");
        assert_eq!(motivo_corto("chrome", "la descarga se cortó o falló tras 4 intentos"), "no connection");
        // The login helper's own English wording.
        assert_eq!(motivo_corto("chrome", "the download was cut off or failed after 4 attempts"), "no connection");
        assert_eq!(motivo_corto("chrome", "the server rejected the download: 403"), "no connection");
    }

    #[test]
    fn motivo_corto_detecta_sin_espacio() {
        assert_eq!(motivo_corto("setup", "gogdl: [Errno 28] No space left on device"), "no disk space");
        assert_eq!(motivo_corto("chrome", "could not write to disk: no space left"), "no disk space");
        // Plugin wording, still matched.
        assert_eq!(motivo_corto("chrome", "no se pudo escribir en disco: sin espacio"), "no disk space");
    }

    #[test]
    fn motivo_corto_muestra_crudo_si_no_clasifica() {
        assert_eq!(motivo_corto("setup", "gogdl: permiso denegado"), "gogdl: permiso denegado");
        assert_eq!(motivo_corto("setup", "   "), "unknown error");
    }

    #[test]
    fn las_tres_deps_estan_fichadas() {
        assert_eq!(DEPS.len(), 3);
        assert!(DEPS.iter().any(|d| d.id == DepId::Chromium && d.peso == "~188 MB"));
    }

    /// The reported bug: row A Failed, row B still in progress (neither
    /// success nor failure) → the link must already be visible, without
    /// waiting for B.
    #[test]
    fn mostrar_skip_con_un_fallo_y_otra_en_progreso() {
        assert!(mostrar_skip(&[FilaEstado::Fallo, FilaEstado::Activa]));
    }

    /// With no failure there's no link, in no intermediate or final state.
    #[test]
    fn mostrar_skip_sin_fallos_oculto() {
        assert!(!mostrar_skip(&[FilaEstado::Espera, FilaEstado::Espera]));
        assert!(!mostrar_skip(&[FilaEstado::Activa, FilaEstado::Espera]));
        assert!(!mostrar_skip(&[FilaEstado::Lista, FilaEstado::Lista]));
        assert!(!mostrar_skip(&[]));
    }

    /// A failure next to a success also shows the link (rest ignored).
    #[test]
    fn mostrar_skip_con_fallo_y_exito() {
        assert!(mostrar_skip(&[FilaEstado::Lista, FilaEstado::Fallo]));
    }
}
