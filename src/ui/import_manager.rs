//! Import Manager: el dialogo que aparece antes de registrar en la
//! biblioteca un juego instalado por Heroic o Lutris.
//!
//! No decide nada. Solo muestra los modos, los motivos por los que un juego
//! no se puede mover, y devuelve el modo elegido. Quien lo llama calcula el
//! `ExecPlan` con `import_move::plan_for_mode` y ejecuta despues.

use std::cell::Cell;
use std::rc::Rc;

use adw::prelude::*;
use gtk::{self, glib};

use crate::backend::import_move::{Blocker, ImportMode, Preflight};
use crate::ui::helpers;

/// Descripcion de "Import permanente". Texto del enunciado, sin tocar.
const DESC_PERMANENT: &str = "Se importarán tus juegos al launcher y además se moverán a una carpeta de tu /home llamada Games (solo se mueven install_path y prefix_path). Esto evita warnings o bugs por rutas no registradas o ejecutables perdidos.";

const DESC_TEST: &str = "Registra el juego con sus rutas actuales, tal como están. No mueve ningún archivo.";

/// Abre el Import Manager y entrega el modo elegido a `on_done`.
///
/// `on_done` recibe `Some(modo)` al confirmar y `None` al cancelar, cerrar con
/// la X o pulsar Escape. Se llama una sola vez, sea cual sea la via de cierre.
///
/// `pf` ya viene calculado por el llamador (`import_move::preflight`), que solo
/// lee: aqui no se toca el disco ni se vuelve a comprobar nada.
pub fn ask<F>(parent: &gtk::ApplicationWindow, label: &str, pf: &Preflight, on_done: F)
where
    F: Fn(Option<ImportMode>) + 'static,
{
    let space_blocked = pf
        .blockers
        .iter()
        .any(|b| matches!(b, Blocker::NotEnoughSpace { .. }));
    let shared = shared_labels(&pf.blockers);
    let hard_blockers: Vec<&Blocker> = pf
        .blockers
        .iter()
        .filter(|b| !matches!(b, Blocker::SharedPrefix { .. }))
        .collect();
    // Sin espacio, o sin nada que mover, el permanente no tiene sentido.
    let can_permanent = !space_blocked && !pf.plans.is_empty();

    // `F` no se puede clonar, asi que va en un Rc para poder llegar a los tres
    // caminos de cierre. `fired` garantiza una sola llamada.
    let cb: Rc<dyn Fn(Option<ImportMode>)> = Rc::new(on_done);
    let fired: Rc<Cell<bool>> = Rc::new(Cell::new(false));
    let finish: Rc<dyn Fn(Option<ImportMode>)> = Rc::new({
        let cb = cb.clone();
        let fired = fired.clone();
        move |mode| {
            if fired.replace(true) {
                return;
            }
            cb(mode);
        }
    });

    let chosen: Rc<Cell<Option<ImportMode>>> = Rc::new(Cell::new(None));

    let dlg = adw::Dialog::new();
    dlg.set_title("Import Manager");
    dlg.set_content_width(540);

    let (header, x_btn) = helpers::modal_header("Import Manager");
    let content = gtk::Box::new(gtk::Orientation::Vertical, 0);
    content.add_css_class("modal-bg");
    content.append(&header);

    let body = gtk::Box::new(gtk::Orientation::Vertical, 12);
    body.set_margin_top(12);
    body.set_margin_bottom(16);
    body.set_margin_start(16);
    body.set_margin_end(16);

    let head = gtk::Label::new(Some(&format!("Importar {} en la biblioteca", label)));
    head.set_halign(gtk::Align::Start);
    head.add_css_class("title-label");
    body.append(&head);

    let lead = gtk::Label::new(Some("Elige cómo se registra. Import test no mueve ningún archivo."));
    lead.set_halign(gtk::Align::Start);
    lead.set_wrap(true);
    lead.add_css_class("time-label");
    body.append(&lead);

    if space_blocked {
        if let Some(msg) = space_blocker_message(&pf.blockers) {
            let warn = gtk::Label::new(Some(&format!("No se puede mover nada: {}", msg)));
            warn.set_halign(gtk::Align::Start);
            warn.set_wrap(true);
            warn.add_css_class("import-warn");
            body.append(&warn);
        }
    }

    // Modo 1: test. Siempre disponible, es el comportamiento actual.
    let test = mode_toggle("Import test", DESC_TEST, false, None);
    body.append(&test);

    // Modo 2: permanente, con el punto de color.
    let permanent = mode_toggle("Import permanente", DESC_PERMANENT, true, Some(&test));
    permanent.set_sensitive(can_permanent);
    if !can_permanent && !space_blocked {
        let why = gtk::Label::new(Some("No hay juegos que se puedan mover en esta selección."));
        why.set_halign(gtk::Align::Start);
        why.set_wrap(true);
        why.add_css_class("time-label");
        body.append(&why);
    }
    body.append(&permanent);

    // Modo 3: grupo con prefix compartido. Solo si de verdad lo hay. El
    // toggle se guarda para cablearlo abajo: hay que cablear ESTE, el que se
    // ve, no una copia.
    let mut group_btn: Option<gtk::ToggleButton> = None;
    if !shared.is_empty() {
        let desc = format!(
            "Mueve {} juego{} que comparten prefijo como un grupo. Si uno no se puede mover, el grupo entero se queda sin mover.",
            shared.len(),
            if shared.len() == 1 { "" } else { "s" }
        );
        let group = mode_toggle("Mover el grupo con prefijo compartido", &desc, true, Some(&test));
        group.set_sensitive(can_permanent);
        body.append(&group);

        let names = gtk::Label::new(Some(&format!("Juegos afectados: {}", shared.join(", "))));
        names.set_halign(gtk::Align::Start);
        names.set_wrap(true);
        names.add_css_class("time-label");
        body.append(&names);

        group_btn = Some(group);
    }

    // Bloqueos por juego: no impiden el import en test, asi que van como nota.
    if !hard_blockers.is_empty() {
        body.append(&gtk::Separator::new(gtk::Orientation::Horizontal));
        let mut lines: Vec<String> = hard_blockers.iter().map(|b| b.message()).collect();
        lines.sort();
        lines.dedup();
        let notes = gtk::Label::new(Some(&format!(
            "Estos juegos solo se podrán registrar en modo test:\n{}",
            lines.join("\n")
        )));
        notes.set_halign(gtk::Align::Start);
        notes.set_wrap(true);
        notes.add_css_class("time-label");
        body.append(&notes);
    }

    // Pie: cancelar e importar.
    let footer = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    footer.set_halign(gtk::Align::End);
    footer.set_margin_top(4);
    let cancel = gtk::Button::with_label("Cancelar");
    let go = gtk::Button::with_label("Importar");
    go.add_css_class("add-btn");
    go.set_sensitive(false);
    footer.append(&cancel);
    footer.append(&go);
    body.append(&footer);

    content.append(&body);
    dlg.set_child(Some(&content));

    // Grupo de toggles: al activar uno se sueltan los demas.
    let wire = |btn: &gtk::ToggleButton, mode: ImportMode| {
        let chosen = chosen.clone();
        let go = go.clone();
        btn.connect_toggled(move |b| {
            if b.is_active() {
                chosen.set(Some(mode));
                go.set_sensitive(true);
            }
        });
    };
    wire(&test, ImportMode::Test);
    wire(&permanent, ImportMode::Permanent);
    if let Some(g) = &group_btn {
        wire(g, ImportMode::PermanentWithSharedGroups);
    }

    {
        let d = dlg.clone();
        let finish = finish.clone();
        cancel.connect_clicked(move |_| {
            finish(None);
            d.close();
        });
    }
    {
        let d = dlg.clone();
        let finish = finish.clone();
        x_btn.connect_clicked(move |_| {
            finish(None);
            d.close();
        });
    }
    {
        let d = dlg.clone();
        let chosen = chosen.clone();
        let finish = finish.clone();
        go.connect_clicked(move |_| {
            let mode = chosen.get().unwrap_or(ImportMode::Test);
            finish(Some(mode));
            d.close();
        });
    }
    {
        // Escape o cualquier otro cierre: si no se confirmo, es None.
        let finish = finish.clone();
        dlg.connect_close_request(move |_| {
            finish(None);
            glib::Propagation::Proceed
        });
    }

    dlg.present(Some(parent));
}

/// Fila de modo: punto opcional, nombre y descripcion. El punto es una label
/// vacia con la clase `import-dot`; no hay ningun glifo en el texto.
fn mode_toggle(
    title: &str,
    desc: &str,
    dot: bool,
    group: Option<&gtk::ToggleButton>,
) -> gtk::ToggleButton {
    let btn = gtk::ToggleButton::new();
    if let Some(g) = group {
        btn.set_group(Some(g));
    }
    btn.add_css_class("import-mode");

    let outer = gtk::Box::new(gtk::Orientation::Vertical, 4);
    let row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    if dot {
        let d = gtk::Label::new(None);
        d.add_css_class("import-dot");
        d.set_valign(gtk::Align::Center);
        row.append(&d);
    }
    let t = gtk::Label::new(Some(title));
    t.set_halign(gtk::Align::Start);
    t.set_hexpand(true);
    row.append(&t);
    outer.append(&row);

    if !desc.is_empty() {
        let dsc = gtk::Label::new(Some(desc));
        dsc.set_halign(gtk::Align::Start);
        dsc.set_wrap(true);
        dsc.add_css_class("time-label");
        outer.append(&dsc);
    }

    btn.set_child(Some(&outer));
    btn
}

/// Etiquetas con prefix compartido, deduplicadas y ordenadas.
fn shared_labels(blockers: &[Blocker]) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for b in blockers {
        if let Blocker::SharedPrefix { label, .. } = b {
            if !out.contains(label) {
                out.push(label.clone());
            }
        }
    }
    out.sort();
    out
}

fn space_blocker_message(blockers: &[Blocker]) -> Option<String> {
    blockers
        .iter()
        .find_map(|b| match b {
            Blocker::NotEnoughSpace { .. } => Some(b.message()),
            _ => None,
        })
}
