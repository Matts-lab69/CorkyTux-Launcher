//! Import Manager: the dialog shown before a game installed by Heroic or
//! Lutris is registered in the library.
//!
//! It decides nothing. It shows the modes, the reasons a game cannot be
//! moved, and hands the chosen mode back. The caller builds the `ExecPlan`
//! with `import_move::plan_for_mode` and executes it.

use std::cell::Cell;
use std::path::{Path, PathBuf};
use std::rc::Rc;

use adw::prelude::*;
use gtk::{self};

use crate::backend::import_move::{Blocker, ImportMode, Preflight};
use crate::ui::helpers;

/// Description of "Import permanente". From the spec, untranslated.
const DESC_PERMANENT: &str = "Your games will be imported into the launcher and also moved to a Games folder in your /home (only install_path and prefix_path are moved). This avoids warnings or bugs caused by unregistered paths or lost executables.";

const DESC_TEST: &str = "Registers the game with its current paths, as they are. No files are moved.";

/// Opens the Import Manager and hands the chosen mode to `on_done`.
///
/// `on_done` gets `Some(mode)` on confirm and `None` on cancel, window close
/// or Escape. It is called exactly once, whichever way the dialog closes.
///
/// `pf` was already computed by the caller (`import_move::preflight`), which
/// only reads: nothing here touches the disk or rechecks anything.
pub fn ask<F>(parent: &adw::ApplicationWindow, label: &str, pf: &Preflight, on_done: F)
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
    // No space, or nothing to move: permanent makes no sense.
    let can_permanent = !space_blocked && !pf.plans.is_empty();

    // `F` cannot be cloned, so it goes in an Rc to reach all three close
    // paths. `fired` makes sure it runs only once.
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

    let head = gtk::Label::new(Some(&format!("Import {} into the library", label)));
    head.set_halign(gtk::Align::Start);
    head.add_css_class("title-label");
    body.append(&head);

    let lead = gtk::Label::new(Some("Choose how it is registered. Import test does not move any files."));
    lead.set_halign(gtk::Align::Start);
    lead.set_wrap(true);
    lead.add_css_class("time-label");
    body.append(&lead);

    if space_blocked {
        if let Some(msg) = space_blocker_message(&pf.blockers) {
            let warn = gtk::Label::new(Some(&format!("Nothing can be moved: {}", msg)));
            warn.set_halign(gtk::Align::Start);
            warn.set_wrap(true);
            warn.add_css_class("import-warn");
            body.append(&warn);
        }
    }

    // Mode 1: test. Always available, it is the current behaviour.
    let test = mode_toggle("Import test", DESC_TEST, false, None);
    body.append(&test);

    // Mode 2: permanent, with the colour dot.
    let permanent = mode_toggle("Import permanente", DESC_PERMANENT, true, Some(&test));
    permanent.set_sensitive(can_permanent);
    if !can_permanent && !space_blocked {
        let why = gtk::Label::new(Some("No games can be moved in this selection."));
        why.set_halign(gtk::Align::Start);
        why.set_wrap(true);
        why.add_css_class("time-label");
        body.append(&why);
    }
    body.append(&permanent);

    // Mode 3: shared-prefix group. Only shown when one really exists. The
    // toggle is kept so it can be wired below: wire THIS one, the visible
    // one, not a copy.
    let mut group_btn: Option<gtk::ToggleButton> = None;
    if !shared.is_empty() {
        let desc = format!(
            "Moves {} game{} that share a prefix as a group. If one of them cannot be moved, the whole group stays where it is.",
            shared.len(),
            if shared.len() == 1 { "" } else { "s" }
        );
        let group = mode_toggle("Move the shared-prefix group", &desc, true, Some(&test));
        group.set_sensitive(can_permanent);
        body.append(&group);

        let names = gtk::Label::new(Some(&format!("Affected games: {}", shared.join(", "))));
        names.set_halign(gtk::Align::Start);
        names.set_wrap(true);
        names.add_css_class("time-label");
        body.append(&names);

        group_btn = Some(group);
    }

    // Per-game blockers do not stop a test import, so they are a note.
    if !hard_blockers.is_empty() {
        body.append(&gtk::Separator::new(gtk::Orientation::Horizontal));
        let mut lines: Vec<String> = hard_blockers.iter().map(|b| b.message()).collect();
        lines.sort();
        lines.dedup();
        let notes = gtk::Label::new(Some(&format!(
            "These games can only be registered in test mode:\n{}",
            lines.join("\n")
        )));
        notes.set_halign(gtk::Align::Start);
        notes.set_wrap(true);
        notes.add_css_class("time-label");
        body.append(&notes);
    }

    // Footer: cancel and import.
    let footer = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    footer.set_halign(gtk::Align::End);
    footer.set_margin_top(4);
    let cancel = gtk::Button::with_label("Cancel");
    let go = gtk::Button::with_label("Import");
    go.add_css_class("add-btn");
    go.set_sensitive(false);
    footer.append(&cancel);
    footer.append(&go);
    body.append(&footer);

    content.append(&body);
    dlg.set_child(Some(&content));

    // Toggle group: activating one releases the others.
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
            let _ = d.close();
        });
    }
    {
        let d = dlg.clone();
        let finish = finish.clone();
        // close() devuelve bool: el let _ lo descarta para que el
        // closure devuelva ().
        x_btn.connect_clicked(move |_| {
            finish(None);
            let _ = d.close();
        });
    }
    {
        let d = dlg.clone();
        let chosen = chosen.clone();
        let finish = finish.clone();
        go.connect_clicked(move |_| {
            let mode = chosen.get().unwrap_or(ImportMode::Test);
            finish(Some(mode));
            let _ = d.close();
        });
    }
    {
        // Escape or any other close: None unless it was confirmed.
        // adw::Dialog no es un gtk::Window y no tiene close-request: la
        // senal es "closed", sin valor de retorno.
        let finish = finish.clone();
        dlg.connect_closed(move |_| finish(None));
    }

    dlg.present(Some(parent));
}

/// Indeterminate progress dialog for a move. `cp -a` does not report bytes,
/// so the caller pulses the bar from its polling loop and sets the total when
/// the move finishes.
pub fn progress(parent: &adw::ApplicationWindow, title: &str) -> (gtk::ProgressBar, gtk::Label) {
    let dlg = adw::Dialog::new();
    dlg.set_title(title);
    dlg.set_content_width(400);
    let (header, x_btn) = helpers::modal_header(title);
    let content = gtk::Box::new(gtk::Orientation::Vertical, 0);
    content.add_css_class("modal-bg");
    content.append(&header);
    let body = gtk::Box::new(gtk::Orientation::Vertical, 8);
    body.set_margin_top(12);
    body.set_margin_bottom(16);
    body.set_margin_start(16);
    body.set_margin_end(16);
    let status = gtk::Label::new(Some("Starting…"));
    status.set_halign(gtk::Align::Start);
    status.set_wrap(true);
    body.append(&status);
    let bar = gtk::ProgressBar::new();
    bar.set_show_text(true);
    body.append(&bar);
    let hide = gtk::Button::with_label("Hide");
    hide.add_css_class("settings-btn");
    body.append(&hide);
    content.append(&body);
    dlg.set_child(Some(&content));
    {
        let d = dlg.clone();
        x_btn.connect_clicked(move |_| {
            let _ = d.close();
        });
    }
    {
        let d = dlg.clone();
        hide.connect_clicked(move |_| {
            let _ = d.close();
        });
    }
    dlg.present(Some(parent));
    (bar, status)
}

/// Rewrites the executable after the folder was moved.
///
/// A relative one is left alone: `import_to_library` prefixes it with the new
/// main_path. An absolute one that lived inside the moved folder is remapped.
/// One outside the folder is left alone too: that file did not move.
pub fn remap_exe(exe: &str, old_install: &str, new_install: &Path) -> String {
    if exe.is_empty() {
        return String::new();
    }
    if !exe.starts_with('/') && !exe.contains(':') {
        return exe.to_string();
    }
    match Path::new(exe).strip_prefix(old_install) {
        Ok(rel) => format!(
            "{}/{}",
            new_install.to_string_lossy().trim_end_matches('/'),
            rel.display()
        ),
        Err(_) => exe.to_string(),
    }
}

/// Mode row: optional dot, name and description. The dot is an empty label
/// with the `import-dot` class; there is no glyph anywhere in the text.
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

/// Labels sharing a prefix, deduplicated and sorted.
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

/// Convenience for callers: the Games root the move writes into.
pub fn games_root() -> PathBuf {
    PathBuf::from(std::env::var("HOME").unwrap_or_default()).join("Games")
}
