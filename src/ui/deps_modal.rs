//! Modal obligatorio "Install dependencies" de Stores.
//!
//! Lista solo las herramientas que faltan (legendary, gogdl, Chromium de
//! login), con un único botón "Install All" y sin X ni "Close": mientras falte
//! algo no se puede usar Stores. Tras ≥1 intento fallido aparece el link
//! "Skip for now", que cierra sin instalar nada (la UI muestra entonces el
//! estado "Setup incomplete" con botón para reabrir).
//!
//! El éxito total cierra solo: un modal sin ninguna salida posible sería un
//! bug, no una decisión de diseño.

use adw::prelude::*;
use std::rc::Rc;

/// Qué herramienta falta.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DepId {
    Legendary,
    Gogdl,
    Chromium,
}

/// Ficha de cada dependencia: nombre, descripción y peso medidos.
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

/// Cómo terminó el modal (siempre se cierra por una de estas dos vías).
pub enum InstallOutcome {
    /// Todo listo: el llamador refresca y el modal ya se cerró solo.
    TodoOk,
    /// El usuario pidió salir sin instalar: mostrar "Setup incomplete".
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

/// Mapea un error libre del backend a un motivo corto en inglés para la fila.
///
/// Los textos de `cmd_setup` son excepciones de Python sin formato fijo y los
/// de `--prefetch` son mensajes humanos en español: se clasifican por palabra
/// clave y, si nada matchea, se muestra el texto crudo recortado. Nunca se
/// inventa un diagnóstico.
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
        "red al conectar",
        "conexi",
        "cortó",
        "rechazo la descarga",
    ] {
        if t.contains(clave) {
            return "no connection".to_string();
        }
    }
    for clave in ["no space", "errno 28", "espacio en disco", "sin espacio"] {
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

/// Instala lo pendiente en un hilo propio: primero legendary/gogdl vía
/// `cmd_setup` (que salta lo ya presente), después Chromium vía
/// `webdriver_login --prefetch` con progreso JSON por stdout.
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
                    // `idx_de` solo da `Some` si el ítem está pendiente.
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
/// Sin X y sin botón de cierre: las únicas salidas son éxito total
/// (auto-cierre + `InstallOutcome::TodoOk`) o el link "Skip for now", que
/// aparece solo tras ≥1 intento fallido (`InstallOutcome::Skip`).
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
    // Sin salida por gesto: ni X (no hay botón) ni Esc. El cierre
    // programático (`close()`) sigue funcionando para el éxito y el skip.
    dialog.set_can_close(false);
    // La modalidad/transiencia la da `present(parent)`: AdwDialog no es
    // GtkWindow y no tiene set_transient_for/set_modal.

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

    // Cabecera sin X: mismo título y separador que `modal_header`.
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

    // Válvula de escape: link chico, solo tras ≥1 intento fallido. No compite
    // con "Install All" y no aparece en el primer render.
    let skip = gtk::Button::with_label("Skip for now");
    skip.add_css_class("flat");
    skip.add_css_class("dim-label");
    skip.set_halign(gtk::Align::Center);
    skip.set_visible(false);
    inner.append(&skip);

    dialog.set_child(Some(&content));

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
                    f.estado.set_text("Waiting…");
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
    // Sin auto-arranque: la instalación empieza SOLO al apretar "Install All".
    // (Una versión anterior llamaba `lanzar()` acá y el modal instalaba solo
    // al abrirse, lo que además hacía aparecer el link de skip sin que el
    // usuario hubiera intentado nada.)
    {
        let t = terminado.clone();
        let d = dialog.clone();
        skip.connect_clicked(move |_| {
            d.close();
            t(InstallOutcome::Skip);
        });
    }

    // Poller como el de ProtonModal: drena el canal cada 100 ms y muere con
    // el diálogo.
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
                            f.estado.set_text("Downloading…");
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
                            f.estado.set_text("Ready ✓");
                        }
                        e.listas += 1;
                        e.barra.set_fraction(e.listas as f64 / e.total.max(1) as f64);
                        e.barra.set_text(Some(&format!("{} of {}", e.listas, e.total)));
                    }
                    Avance::FilaFallo(i, motivo) => {
                        if let Some(f) = e.filas.get(i) {
                            f.spin.stop();
                            f.spin.set_visible(false);
                            f.estado.set_text(&format!("Failed: {}", motivo));
                        }
                    }
                    Avance::Terminado => {
                        e.instalando = false;
                        e.instalar_btn.set_sensitive(true);
                        if e.listas >= e.total {
                            e.barra.set_text(Some("All dependencies installed."));
                            // Éxito total: auto-cierre. Es la única salida
                            // "limpia"; quedarse abierto sin nada que hacer
                            // sería la trampa que el diseño quiere evitar.
                            let dd = d.clone();
                            let tt = t.clone();
                            glib::timeout_add_local_once(
                                std::time::Duration::from_millis(900),
                                move || {
                                    dd.close();
                                    tt(InstallOutcome::TodoOk);
                                },
                            );
                        } else {
                            e.fallos_acumulados += 1;
                            e.barra.set_text(Some("Press Install All to retry."));
                            e.skip.set_visible(true);
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
        assert_eq!(motivo_corto("chrome", "fallo de red al conectar: bla"), "no connection");
        assert_eq!(motivo_corto("chrome", "la descarga se cortó o falló tras 4 intentos"), "no connection");
    }

    #[test]
    fn motivo_corto_detecta_sin_espacio() {
        assert_eq!(motivo_corto("setup", "gogdl: [Errno 28] No space left on device"), "no disk space");
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
}
