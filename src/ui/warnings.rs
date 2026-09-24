//! Shared "Warnings for …" modal used by the prefix-path and install-path
//! checks. One component, two kinds, so the layout, the scroll cap and the
//! per-row rendering never drift apart.

use adw::prelude::*;

use crate::backend::config::ConfigManager;

/// Which list a modal shows.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum WarningKind {
    PrefixPath,
    InstallPath,
}

impl WarningKind {
    fn title(self) -> &'static str {
        match self {
            Self::PrefixPath => "Warnings for prefix path",
            Self::InstallPath => "Warnings for install path",
        }
    }

    fn description(self) -> &'static str {
        match self {
            Self::PrefixPath => "The following games have prefix paths that no longer exist:",
            Self::InstallPath => "The following games do not have a valid install path.",
        }
    }

    fn hint(self) -> &'static str {
        match self {
            Self::PrefixPath => {
                "These games may not launch until you create the prefix manually or update \
                 the prefix path in Game Settings."
            }
            Self::InstallPath => {
                "These games may not launch until you locate the folder again or update the \
                 install path in Game Settings."
            }
        }
    }

    /// Row dot style. Install-path uses libadwaita's `error` utility class
    /// (the theme error colour, no hard-coded hex); prefix-path keeps the
    /// orange warning dot.
    fn dot_classes(self) -> &'static [&'static str] {
        match self {
            Self::PrefixPath => &["warn-dot"],
            Self::InstallPath => &["error-dot", "error"],
        }
    }
}

pub struct WarningModal {
    dialog: adw::Dialog,
    games_list: gtk::Box,
    dot_classes: &'static [&'static str],
}

impl WarningModal {
    pub fn new(kind: WarningKind) -> Self {
        let dialog = adw::Dialog::new();
        dialog.set_title(kind.title());
        // Explicit minimum size: Adwaita warns when an AdwDialog only has
        // content sizes (AdwDialog does not have a minimum size).
        dialog.set_width_request(520);
        dialog.set_height_request(380);
        dialog.set_content_width(520);
        dialog.set_content_height(380);

        // Full-bleed: outer carries .modal-bg to the dialog edges (no
        // gray Adwaita frame), inner holds the padded content.
        let content = gtk::Box::new(gtk::Orientation::Vertical, 0);
        content.add_css_class("modal-bg");
        content.set_hexpand(true);
        content.set_vexpand(true);
        let inner = gtk::Box::new(gtk::Orientation::Vertical, 12);
        inner.set_hexpand(true);
        inner.set_vexpand(true);
        inner.set_margin_top(12);
        inner.set_margin_bottom(12);
        inner.set_margin_start(16);
        inner.set_margin_end(16);
        content.append(&inner);

        let (header_row, x_btn) = crate::ui::helpers::modal_header(kind.title());
        header_row.set_margin_top(4);
        {
            let dlg = dialog.clone();
            x_btn.connect_clicked(move |_| { dlg.close(); });
        }
        inner.append(&header_row);

        let title = gtk::Label::new(Some(kind.description()));
        title.set_halign(gtk::Align::Start);
        title.set_wrap(true);
        title.add_css_class("details-title");
        inner.append(&title);

        // C++ parity: list capped at 220px with scroll (no yellow box,
        // only the small dot per row).
        let games_list = gtk::Box::new(gtk::Orientation::Vertical, 6);
        let games_scroll = gtk::ScrolledWindow::new();
        games_scroll.set_min_content_height(80);
        games_scroll.set_max_content_height(220);
        games_scroll.set_vexpand(true);
        games_scroll.set_child(Some(&games_list));
        inner.append(&games_scroll);

        let hint = gtk::Label::new(Some(kind.hint()));
        hint.set_halign(gtk::Align::Center);
        hint.set_justify(gtk::Justification::Center);
        hint.set_wrap(true);
        hint.set_opacity(0.6);
        hint.add_css_class("time-label");
        inner.append(&hint);

        let ok_btn = gtk::Button::with_label("OK");
        ok_btn.add_css_class("add-btn");
        ok_btn.set_halign(gtk::Align::Center);
        ok_btn.set_width_request(100);
        let dlg = dialog.clone();
        ok_btn.connect_clicked(move |_| { dlg.close(); });
        inner.append(&ok_btn);

        dialog.set_child(Some(&content));

        Self { dialog, games_list, dot_classes: kind.dot_classes() }
    }

    pub fn show_missing(&self, games: &[(String, String)]) {
        // Remove old entries
        while let Some(child) = self.games_list.first_child() {
            self.games_list.remove(&child);
        }
        for (name, path) in games {
            let row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
            let dot = gtk::Label::new(Some("●"));
            dot.set_css_classes(self.dot_classes);
            dot.set_valign(gtk::Align::Center);
            let col = gtk::Box::new(gtk::Orientation::Vertical, 2);
            let name_lbl = gtk::Label::new(Some(name));
            name_lbl.set_halign(gtk::Align::Start);
            name_lbl.add_css_class("warn-game");
            let path_lbl = gtk::Label::new(Some(if path.is_empty() {
                "(no path configured)"
            } else {
                path
            }));
            path_lbl.set_halign(gtk::Align::Start);
            path_lbl.set_ellipsize(gtk::pango::EllipsizeMode::Middle);
            path_lbl.set_opacity(0.6);
            path_lbl.add_css_class("time-label");
            col.append(&name_lbl);
            col.append(&path_lbl);
            row.append(&dot);
            row.append(&col);
            self.games_list.append(&row);
        }
    }

    pub fn present(&self, parent: &impl IsA<gtk::Widget>) {
        self.dialog.present(Some(parent));
    }
}

// ---------------------------------------------------------------------------
// Install-path validation — the single source of truth shared by the Store
// (via the plugin's installed check), Games.ini (Locate control) and both
// Warnings dialogs.
// ---------------------------------------------------------------------------

/// Expand a leading `~/` to `$HOME` (same rule the launcher uses for paths).
pub fn expand_tilde(s: &str) -> String {
    if s == "~" || s.starts_with("~/") {
        if let Ok(home) = std::env::var("HOME") {
            return s.replacen('~', &home, 1);
        }
    }
    s.to_string()
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum InstallPathState {
    Valid,
    FolderMissing,
    ExeMissing,
}

/// Classify a game's install path. `None` means the game has no MainPath at
/// all: those are "no path by design" (e.g. emulator/Steam entries) and are
/// excluded from warnings. The criterion matches the store plugin's
/// installed check: path set AND folder exists AND (exe empty OR exe exists).
pub fn classify_install_path(main_path: &str, exe: &str) -> Option<InstallPathState> {
    let main = main_path.trim();
    if main.is_empty() {
        return None;
    }
    let expanded = expand_tilde(main);
    if !std::path::Path::new(&expanded).is_dir() {
        return Some(InstallPathState::FolderMissing);
    }
    let exe = exe.trim();
    if exe.is_empty() {
        return Some(InstallPathState::Valid);
    }
    let exe_expanded = expand_tilde(exe);
    let ep = std::path::Path::new(&exe_expanded);
    let exe_path = if ep.is_absolute() {
        ep.to_path_buf()
    } else {
        std::path::Path::new(&expanded).join(exe)
    };
    if exe_path.is_file() {
        Some(InstallPathState::Valid)
    } else {
        Some(InstallPathState::ExeMissing)
    }
}

/// Install-path inputs captured on the GTK main thread (config reads only).
/// Plain data so the filesystem checks can run on a worker thread.
pub struct InstallPathInput {
    pub name: String,
    pub main_path: String,
    pub exe: String,
}

/// Capture inputs for every game that has an install path. Games with an
/// empty MainPath are "no path by design" and skipped here.
pub fn collect_inputs(config: &ConfigManager, names: &[String]) -> Vec<InstallPathInput> {
    let mut out = Vec::new();
    for name in names {
        let main_path = config.game_value(name, "MainPath").unwrap_or_default();
        if main_path.trim().is_empty() {
            continue;
        }
        let exe = config.game_value(name, "Executable").unwrap_or_default();
        out.push(InstallPathInput { name: name.clone(), main_path, exe });
    }
    out
}

/// `(name, path)` of games whose install path is invalid. Pure but does
/// blocking filesystem I/O: call from a worker thread, never the main loop.
pub fn invalid_install_paths(inputs: &[InstallPathInput]) -> Vec<(String, String)> {
    inputs
        .iter()
        .filter(|i| {
            !matches!(
                classify_install_path(&i.main_path, &i.exe),
                Some(InstallPathState::Valid)
            )
        })
        .map(|i| (i.name.clone(), i.main_path.clone()))
        .collect()
}

/// Run the install-path check off the GTK main loop and hand the invalid
/// list back on the main loop through `done`.
pub fn check_install_paths_async<F>(config: &ConfigManager, names: Vec<String>, done: F)
where
    F: Fn(Vec<(String, String)>) + 'static,
{
    let inputs = collect_inputs(config, &names);
    let (tx, rx) = std::sync::mpsc::channel::<Vec<(String, String)>>();
    std::thread::spawn(move || {
        let _ = tx.send(invalid_install_paths(&inputs));
    });
    crate::backend::plugin_process::poll_once_local(rx, move |res| match res {
        Ok(list) => {
            done(list);
            glib::ControlFlow::Break
        }
        Err(std::sync::mpsc::TryRecvError::Empty) => glib::ControlFlow::Continue,
        Err(_) => glib::ControlFlow::Break,
    });
}

// ---------------------------------------------------------------------------
// Prefix-path validation — same shape as the install-path checks, so both
// Warnings dialogs and both header buttons come from one shared module.
// ---------------------------------------------------------------------------

/// Prefix-path inputs captured on the GTK main thread (config reads only).
pub struct PrefixInput {
    pub name: String,
    pub prefix: String,
}

/// Capture inputs for every game that has a prefix path. Games with an
/// empty PrefixPath use the launcher default (or none by design) and are
/// skipped here.
pub fn collect_prefix_inputs(config: &ConfigManager, names: &[String]) -> Vec<PrefixInput> {
    let mut out = Vec::new();
    for name in names {
        let prefix = config.game_value(name, "PrefixPath").unwrap_or_default();
        if prefix.trim().is_empty() {
            continue;
        }
        out.push(PrefixInput { name: name.clone(), prefix });
    }
    out
}

/// `(name, prefix)` of games whose prefix path folder no longer exists.
/// Pure but does blocking filesystem I/O: call from a worker thread.
pub fn invalid_prefixes(inputs: &[PrefixInput]) -> Vec<(String, String)> {
    inputs
        .iter()
        .filter(|i| !std::path::Path::new(&expand_tilde(&i.prefix)).is_dir())
        .map(|i| (i.name.clone(), i.prefix.clone()))
        .collect()
}

/// Run the prefix-path check off the GTK main loop and hand the invalid
/// list back on the main loop through `done`.
pub fn check_prefixes_async<F>(config: &ConfigManager, names: Vec<String>, done: F)
where
    F: Fn(Vec<(String, String)>) + 'static,
{
    let inputs = collect_prefix_inputs(config, &names);
    let (tx, rx) = std::sync::mpsc::channel::<Vec<(String, String)>>();
    std::thread::spawn(move || {
        let _ = tx.send(invalid_prefixes(&inputs));
    });
    crate::backend::plugin_process::poll_once_local(rx, move |res| match res {
        Ok(list) => {
            done(list);
            glib::ControlFlow::Break
        }
        Err(std::sync::mpsc::TryRecvError::Empty) => glib::ControlFlow::Continue,
        Err(_) => glib::ControlFlow::Break,
    });
}
