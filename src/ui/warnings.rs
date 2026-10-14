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

        // Centered title (both WarningKinds share this dialog, and the
        // description/hint/OK below are centered too).
        let (header_row, x_btn) = crate::ui::helpers::modal_header_centered(kind.title());
        header_row.set_margin_top(4);
        {
            let dlg = dialog.clone();
            x_btn.connect_clicked(move |_| { dlg.close(); });
        }
        inner.append(&header_row);

        // Centered title: both WarningKinds share this modal, and the description
        // is the dialog's heading — centered to line up with the hint and the
        // OK button below it.
        let title = gtk::Label::new(Some(kind.description()));
        title.set_halign(gtk::Align::Center);
        title.set_justify(gtk::Justification::Center);
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
/// Steam entries may store the installdir *folder* as Executable, and a
/// missing-folder Steam game is only Valid when the manifest + on-disk
/// installdir still confirm the install (see `steam_install_confirmed`).
pub fn classify_install_path(main_path: &str, exe: &str, steam_id: &str) -> Option<InstallPathState> {
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
        return Some(InstallPathState::Valid);
    }
    // Steam import stores the installdir folder (which exists) as
    // Executable for every game — an existing folder is a valid install,
    // not a missing executable.
    if exe_path.is_dir() {
        return Some(InstallPathState::Valid);
    }
    // Last-resort Steam evidence: appmanifest "fully installed" + the
    // installdir folder must still exist on disk (PEAK fails both: its
    // state flag is set but common/PEAK is gone).
    if !steam_id.trim().is_empty() && crate::backend::integration::steam_install_confirmed(steam_id) {
        return Some(InstallPathState::Valid);
    }
    Some(InstallPathState::ExeMissing)
}

/// Install-path inputs captured on the GTK main thread (config reads only).
/// Plain data so the filesystem checks can run on a worker thread.
#[derive(Clone)]
pub struct InstallPathInput {
    pub name: String,
    pub main_path: String,
    pub exe: String,
    pub steam_id: String,
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
        let steam_id = config.game_value(name, "SteamID").unwrap_or_default();
        out.push(InstallPathInput { name: name.clone(), main_path, exe, steam_id });
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
                classify_install_path(&i.main_path, &i.exe, &i.steam_id),
                Some(InstallPathState::Valid)
            )
        })
        .map(|i| (i.name.clone(), i.main_path.clone()))
        .collect()
}

/// Files that are never the game executable during auto-repair: save states
/// (NSMB's `.sav` shares the truncated prefix with its `.nds`), images,
/// docs, archives, launcher metadata.
const AUX_EXTS: &[&str] = &[
    "sav", "png", "jpg", "jpeg", "gif", "bmp", "webp", "txt", "log", "md",
    "json", "ini", "pdf", "tmp", "bak", "db", "desktop", "ico", "zip", "7z",
    "tar", "gz", "so", "dll",
];

/// Lowercased basename with version segments (`-1.23.1`, `-v2.0`) dropped so
/// `OpenChamber-1.23.1-linux-x86_64.AppImage` and the newer
/// `OpenChamber-2.0.0-linux-x86_64.AppImage` compare equal. A segment counts
/// as a version only when it is entirely digits/dots (optionally `v`-prefixed).
fn version_strip(name: &str) -> String {
    let lower = name.to_lowercase();
    let mut out = String::new();
    for seg in lower.split('-') {
        let t = seg.trim_start_matches('v');
        let is_version = !t.is_empty()
            && t.chars().next().map_or(false, |c| c.is_ascii_digit())
            && t.chars().all(|c| c.is_ascii_digit() || c == '.');
        if !is_version {
            if !out.is_empty() {
                out.push('-');
            }
            out.push_str(seg);
        }
    }
    out
}

/// Auto-repair for ExeMissing games: a UNIQUE candidate file inside the
/// (existing) MainPath folder. A candidate matches when the stored exe
/// basename is a prefix of it, or their version-stripped stems are equal.
/// Ambiguous (0 or ≥2) candidates stay warned — never touch, never guess.
/// Never repairs Locate-fixed paths: those classify Valid, not ExeMissing.
/// Returns `(input, candidate full path)` so the apply step can re-validate
/// against the snapshot (old Executable + MainPath) right before writing.
pub fn repair_candidates(inputs: &[InstallPathInput]) -> Vec<(InstallPathInput, String)> {
    let mut out = Vec::new();
    for i in inputs {
        if !matches!(
            classify_install_path(&i.main_path, &i.exe, &i.steam_id),
            Some(InstallPathState::ExeMissing)
        ) {
            continue;
        }
        if let Some(new_exe) = unique_candidate(&i.main_path, &i.exe) {
            out.push((i.clone(), new_exe));
        }
    }
    out
}

/// Unique matching file for an ExeMissing game inside its (existing)
/// MainPath: same matching rule as `repair_candidates` (prefix or
/// version-stripped stem, `AUX_EXTS` excluded). `None` when 0 or ≥2
/// candidates. Used by the worker to propose repairs AND by the main loop
/// to re-verify the candidate is still the only one before writing.
fn unique_candidate(main_path: &str, stored_exe: &str) -> Option<String> {
    let stored = std::path::Path::new(&expand_tilde(stored_exe))
        .file_name()
        .map(|s| s.to_string_lossy().to_lowercase())
        .unwrap_or_default();
    if stored.is_empty() {
        return None;
    }
    let stored_stripped = version_strip(&stored);
    let dir = expand_tilde(main_path);
    let mut candidates = Vec::new();
    if let Ok(rd) = std::fs::read_dir(&dir) {
        for entry in rd.flatten() {
            let Ok(ft) = entry.file_type() else { continue };
            if !ft.is_file() {
                continue;
            }
            let fname = entry.file_name().to_string_lossy().to_lowercase();
            let ext = std::path::Path::new(&fname)
                .extension()
                .map(|e| e.to_string_lossy().to_string())
                .unwrap_or_default();
            if AUX_EXTS.contains(&ext.as_str()) {
                continue;
            }
            if fname.starts_with(&stored) || version_strip(&fname) == stored_stripped {
                candidates.push(entry.file_name().to_string_lossy().to_string());
            }
        }
    }
    if candidates.len() == 1 {
        Some(format!("{}/{}", dir.trim_end_matches('/'), candidates[0]))
    } else {
        None
    }
}

/// Resolve a stored Executable against MainPath: absolute values stay as
/// given, relative values join MainPath. Mirrors `classify_install_path`.
fn resolve_exe(main_path: &str, exe: &str) -> std::path::PathBuf {
    let expanded_exe = expand_tilde(exe.trim());
    let ep = std::path::Path::new(&expanded_exe);
    if ep.is_absolute() {
        ep.to_path_buf()
    } else {
        std::path::Path::new(&expand_tilde(main_path.trim())).join(ep)
    }
}

/// Report Play routing through the Steam client: a numeric SteamID means
/// `run_game` ignores Executable entirely.
pub fn is_steam_launch(steam_id: &str) -> bool {
    let sid = steam_id.trim();
    !sid.is_empty() && sid.chars().all(|c| c.is_ascii_digit())
}

/// Report a stored Executable resolving to a folder, not a file
/// (Steam imports and RPG Maker entries store folders).
pub fn is_folder_exe(main_path: &str, exe: &str) -> bool {
    if exe.trim().is_empty() {
        return false;
    }
    resolve_exe(main_path, exe).is_dir()
}

/// Validate a Change-exe choice: existing file inside MainPath.
/// Canonicalize both sides; reject `..` segments, symlinks escaping
/// MainPath, and blocked helper names. Accept any file name otherwise:
/// native builds carry no extension. Call again at save time; picker
/// output alone stays untrusted.
pub fn validate_exe_choice(main_path: &str, chosen: &str) -> bool {
    if main_path.trim().is_empty() || chosen.trim().is_empty() {
        return false;
    }
    if chosen.split(['/', '\\']).any(|seg| seg == "..") {
        return false;
    }
    if chosen
        .rsplit(['/', '\\'])
        .next()
        .map(|n| is_blocked_exe_name(&n.to_lowercase()))
        .unwrap_or(true)
    {
        return false;
    }
    let Ok(main_canon) = std::fs::canonicalize(expand_tilde(main_path.trim())) else {
        return false;
    };
    let Ok(chosen_canon) = std::fs::canonicalize(expand_tilde(chosen.trim())) else {
        return false;
    };
    chosen_canon.is_file() && chosen_canon.starts_with(&main_canon)
}

/// Executable extensions for the Change-exe picker (lowercase match).
/// Extensionless entries still list when carrying an execution bit.
const EXEC_EXTS: &[&str] = &[
    "exe", "bat", "cmd", "msi", "sh", "appimage", "x86_64", "run",
];

/// Lowercase filename fragments of helpers, runtimes and crash reporters:
/// never the game itself. Matched with `contains`, so `unins` also covers
/// `unins000.exe` and `unitycrashhandler` covers the 32/64-bit variants.
/// Kept specific on purpose: plain `crash` would also block games such as
/// Crash Bandicoot.
const EXE_BLOCKLIST: &[&str] = &[
    "crashhandler", "crashpad", "crashreport", "crashsender", "crashreporter",
    "unitycrashhandler", "unins", "uninstall", "vcredist", "vc_redist",
    "redist", "dxsetup", "dxwebsetup", "directx", "dotnet", "dotnetfx",
    "prereq", "oalinst", "webhelper", "notification_helper", "createdump",
    "bssndrpt",
];

/// Lowercase folder names never descended into: vendor redistributables
/// and installer payloads live here, not game executables.
const BLOCKED_DIRS: &[&str] = &[
    "_commonredist",
    "redist",
    "directx",
    "dotnet",
    "crashpad",
    "__installer",
];

/// Report a blocked helper/runtime/crash-reporter file name (lowercase).
fn is_blocked_exe_name(lower_name: &str) -> bool {
    EXE_BLOCKLIST.iter().any(|frag| lower_name.contains(frag))
}

/// Regular files inside MainPath for the Change-exe picker: depth 3,
/// hidden files and folders skipped, blocked helper names and blocked
/// folders skipped. List entries with an execution bit or a known
/// executable extension. Sort results. Save-time validation stays
/// mandatory.
pub fn list_exe_candidates(main_path: &str) -> Vec<String> {
    let mut out = Vec::new();
    if main_path.trim().is_empty() {
        return out;
    }
    collect_candidates(
        std::path::Path::new(&expand_tilde(main_path.trim())),
        &mut out,
        3,
    );
    out.sort();
    out
}

fn collect_candidates(dir: &std::path::Path, out: &mut Vec<String>, depth: u8) {
    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return,
    };
    for entry in entries.flatten() {
        let p = entry.path();
        if p.is_file() {
            let name = p
                .file_name()
                .map(|n| n.to_string_lossy().to_lowercase())
                .unwrap_or_default();
            if name.starts_with('.') {
                continue;
            }
            if is_blocked_exe_name(&name) {
                continue;
            }
            let ext = std::path::Path::new(&name)
                .extension()
                .map(|e| e.to_string_lossy().to_string())
                .unwrap_or_default();
            #[cfg(unix)]
            let has_exec_bit = {
                use std::os::unix::fs::PermissionsExt;
                entry
                    .metadata()
                    .map(|m| m.permissions().mode() & 0o111 != 0)
                    .unwrap_or(false)
            };
            #[cfg(not(unix))]
            let has_exec_bit = false;
            if !has_exec_bit && !EXEC_EXTS.contains(&ext.as_str()) {
                continue;
            }
            out.push(p.display().to_string().replace('\\', "/"));
        } else if p.is_dir() && depth > 0 {
            if let Some(n) = p.file_name().and_then(|n| n.to_str()) {
                let lower = n.to_lowercase();
                if lower.starts_with('.') || BLOCKED_DIRS.contains(&lower.as_str()) {
                    continue;
                }
            }
            collect_candidates(&p, out, depth - 1);
        }
    }
}

/// Run the install-path check off the GTK main loop. With `apply_repairs`
/// the worker also proposes candidates and the main loop writes them via
/// the same Locate-style `set_game_value` path, re-validating right before
/// the write (still ExeMissing, Executable unchanged, candidate still the
/// unique file) and logging one `[CorkyTux] auto-repair:` line per game.
/// Startup/library refreshes pass `false`: they only detect, so the button
/// count still shows repairable games until the user actually repairs.
/// `done` receives the games that remain invalid after repairs.
pub fn check_install_paths_async<F>(
    config: &ConfigManager,
    names: Vec<String>,
    apply_repairs: bool,
    done: F,
) where
    F: Fn(Vec<(String, String)>) + 'static,
{
    let inputs = collect_inputs(config, &names);
    let (tx, rx) = std::sync::mpsc::channel::<(
        Vec<(String, String)>,
        Vec<(InstallPathInput, String)>,
    )>();
    let cfg = config.clone();
    std::thread::spawn(move || {
        let invalid = invalid_install_paths(&inputs);
        let repairs = if apply_repairs { repair_candidates(&inputs) } else { Vec::new() };
        let _ = tx.send((invalid, repairs));
    });
    crate::backend::plugin_process::poll_once_local(rx, move |res| match res {
        Ok((mut invalid, repairs)) => {
            if apply_repairs {
                // Re-validate on the main loop right before writing: the user
                // may have fixed this game (Locate, manual edit, import)
                // while the worker was scanning. Write only when the game is
                // STILL ExeMissing, its current Executable still equals the
                // snapshot's, and the candidate is still the one and only
                // file on disk.
                let mut applied = Vec::new();
                for (input, new_exe) in &repairs {
                    let cur = cfg.game_value(&input.name, "Executable");
                    let still_same = cur.as_deref() == Some(input.exe.as_str());
                    let still_missing = matches!(
                        classify_install_path(&input.main_path, &input.exe, &input.steam_id),
                        Some(InstallPathState::ExeMissing)
                    );
                    let still_unique = unique_candidate(&input.main_path, &input.exe)
                        .as_deref()
                        == Some(new_exe.as_str())
                        && std::path::Path::new(new_exe).is_file();
                    if still_same && still_missing && still_unique {
                        cfg.set_game_value(&input.name, "Executable", new_exe);
                        eprintln!(
                            "[CorkyTux] auto-repair: \"{}\" Executable \"{}\" -> \"{}\"",
                            input.name, input.exe, new_exe
                        );
                        applied.push(input.name.clone());
                    }
                }
                invalid.retain(|(name, _)| !applied.contains(name));
            }
            done(invalid);
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
        // Steam prefixes belong to Steam: never warned, never repaired.
        if crate::backend::game_model::is_steam_game(
            &config.game_value(name, "Source").unwrap_or_default(),
            &config.game_value(name, "SteamID").unwrap_or_default(),
        ) {
            continue;
        }
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

#[cfg(test)]
mod tests {
    use super::*;

    fn mktmp(name: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(name);
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn write_file(path: &std::path::Path) {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        std::fs::write(path, b"x").unwrap();
    }

    fn mk_game(name: &str) -> (std::path::PathBuf, std::path::PathBuf) {
        let main = mktmp(name);
        let exe = main.join("game.exe");
        write_file(&exe);
        (main, exe)
    }

    #[test]
    fn choice_inside_main_path_passes() {
        let (main, exe) = mk_game("corky_exe_inside");
        assert!(validate_exe_choice(
            &main.display().to_string(),
            &exe.display().to_string()
        ));
        let _ = std::fs::remove_dir_all(&main);
    }

    #[test]
    fn choice_with_dotdot_fails() {
        let (main, exe) = mk_game("corky_exe_dotdot");
        let sub = main.join("sub");
        std::fs::create_dir_all(&sub).unwrap();
        let via_dotdot = format!("{}/../game.exe", sub.display());
        assert!(exe.is_file());
        assert!(!validate_exe_choice(&main.display().to_string(), &via_dotdot));
        let _ = std::fs::remove_dir_all(&main);
    }

    #[test]
    fn choice_absolute_outside_fails() {
        let (main, _exe) = mk_game("corky_exe_outside");
        let other = mktmp("corky_exe_other");
        let outside = other.join("evil.exe");
        write_file(&outside);
        assert!(!validate_exe_choice(
            &main.display().to_string(),
            &outside.display().to_string()
        ));
        let _ = std::fs::remove_dir_all(&main);
        let _ = std::fs::remove_dir_all(&other);
    }

    #[test]
    fn choice_symlink_outside_fails() {
        let (main, _exe) = mk_game("corky_exe_symlink");
        let other = mktmp("corky_exe_symlink_out");
        let target = other.join("real.exe");
        write_file(&target);
        let link = main.join("link.exe");
        #[cfg(unix)]
        std::os::unix::fs::symlink(&target, &link).unwrap();
        assert!(!validate_exe_choice(
            &main.display().to_string(),
            &link.display().to_string()
        ));
        let _ = std::fs::remove_dir_all(&main);
        let _ = std::fs::remove_dir_all(&other);
    }

    #[test]
    fn choice_symlink_inside_passes() {
        let (main, exe) = mk_game("corky_exe_symlink_in");
        let link = main.join("alias.exe");
        #[cfg(unix)]
        std::os::unix::fs::symlink(&exe, &link).unwrap();
        assert!(validate_exe_choice(
            &main.display().to_string(),
            &link.display().to_string()
        ));
        let _ = std::fs::remove_dir_all(&main);
    }

    #[test]
    fn choice_missing_and_empty_fail() {
        let (main, _exe) = mk_game("corky_exe_missing");
        let gone = main.join("gone.exe").display().to_string();
        let main_s = main.display().to_string();
        assert!(!validate_exe_choice(&main_s, &gone));
        assert!(!validate_exe_choice("", &gone));
        assert!(!validate_exe_choice(&main_s, ""));
        assert!(!validate_exe_choice(&main_s, "   "));
        assert!(!validate_exe_choice("/nonexistent-dir-xyz", &gone));
        let _ = std::fs::remove_dir_all(&main);
    }

    #[test]
    fn choice_directory_fails() {
        let (main, _exe) = mk_game("corky_exe_dir");
        let sub = main.join("folder");
        std::fs::create_dir_all(&sub).unwrap();
        assert!(!validate_exe_choice(
            &main.display().to_string(),
            &sub.display().to_string()
        ));
        assert!(is_folder_exe(
            &main.display().to_string(),
            &sub.display().to_string()
        ));
        let _ = std::fs::remove_dir_all(&main);
    }

    #[test]
    fn steam_launch_detection() {
        assert!(is_steam_launch("12345"));
        assert!(is_steam_launch("  480  "));
        assert!(!is_steam_launch(""));
        assert!(!is_steam_launch("abc"));
        assert!(!is_steam_launch("12a45"));
    }

    #[test]
    fn folder_exe_detection() {
        let (main, exe) = mk_game("corky_exe_folder");
        let main_s = main.display().to_string();
        assert!(!is_folder_exe(&main_s, &exe.display().to_string()));
        assert!(!is_folder_exe(&main_s, ""));
        let _ = std::fs::remove_dir_all(&main);
    }

    #[test]
    fn repair_skips_valid_exe() {
        let (main, exe) = mk_game("corky_exe_repair");
        let main_s = main.display().to_string();
        let exe_s = exe.display().to_string();
        let input = InstallPathInput {
            name: "Game".to_string(),
            main_path: main_s.clone(),
            exe: exe_s.clone(),
            steam_id: String::new(),
        };
        assert_eq!(
            classify_install_path(&main_s, &exe_s, ""),
            Some(InstallPathState::Valid)
        );
        assert!(repair_candidates(std::slice::from_ref(&input)).is_empty());
        assert!(invalid_install_paths(std::slice::from_ref(&input)).is_empty());
        let _ = std::fs::remove_dir_all(&main);
    }

    #[test]
    fn candidates_list_exec_bit_and_known_extensions() {
        use std::os::unix::fs::PermissionsExt;
        let main = mktmp("corky_exe_list");
        write_file(&main.join("top.exe"));
        let native = main.join("native_build");
        write_file(&native);
        std::fs::set_permissions(&native, std::fs::Permissions::from_mode(0o755)).unwrap();
        let plain = main.join("plain_file");
        write_file(&plain);
        write_file(&main.join("notes.txt"));
        write_file(&main.join("lib.dll"));
        write_file(&main.join("data.pak"));
        write_file(&main.join(".hidden.exe"));
        write_file(&main.join("a/b/c/deep.exe"));
        write_file(&main.join("a/b/c/d/too_deep.exe"));
        std::fs::create_dir_all(main.join(".hdir")).unwrap();
        write_file(&main.join(".hdir/in.exe"));
        let mut got = list_exe_candidates(&main.display().to_string());
        got.sort();
        let names: Vec<String> = got
            .iter()
            .map(|p| {
                std::path::Path::new(p)
                    .file_name()
                    .unwrap()
                    .to_string_lossy()
                    .to_string()
            })
            .collect();
        assert!(names.contains(&"top.exe".to_string()));
        assert!(names.contains(&"native_build".to_string()));
        assert!(names.contains(&"deep.exe".to_string()));
        assert!(!names.contains(&"plain_file".to_string()));
        assert!(!names.contains(&"notes.txt".to_string()));
        assert!(!names.contains(&"lib.dll".to_string()));
        assert!(!names.contains(&"data.pak".to_string()));
        assert!(!names.contains(&".hidden.exe".to_string()));
        assert!(!names.contains(&"too_deep.exe".to_string()));
        assert!(!names.contains(&"in.exe".to_string()));
        assert!(list_exe_candidates("").is_empty());
        let _ = std::fs::remove_dir_all(&main);
    }

    #[test]
    fn candidates_skip_blocklisted_helpers_and_dirs() {
        let main = mktmp("corky_exe_blocklist");
        for blocked in [
            "UnityCrashHandler64.exe",
            "UnityCrashHandler32.exe",
            "crashpad_handler.exe",
            "CrashReportClient.exe",
            "unins000.exe",
            "vc_redist.x64.exe",
            "dxwebsetup.exe",
            "UE4PrereqSetup_x64.exe",
        ] {
            write_file(&main.join(blocked));
        }
        write_file(&main.join("GameSetup.exe"));
        write_file(&main.join("GameLauncher.exe"));
        write_file(&main.join("CrashBandicoot.exe"));
        write_file(&main.join("CrashTeamRacing.exe"));
        write_file(&main.join("redist/vc_redist.x64.exe"));
        write_file(&main.join("redist/game.exe"));
        write_file(&main.join("_commonredist/DirectX/dxsetup.exe"));
        let got = list_exe_candidates(&main.display().to_string());
        let names: Vec<String> = got
            .iter()
            .map(|p| {
                std::path::Path::new(p)
                    .file_name()
                    .unwrap()
                    .to_string_lossy()
                    .to_string()
            })
            .collect();
        assert!(names.contains(&"GameSetup.exe".to_string()));
        assert!(names.contains(&"GameLauncher.exe".to_string()));
        assert!(names.contains(&"CrashBandicoot.exe".to_string()));
        assert!(names.contains(&"CrashTeamRacing.exe".to_string()));
        for absent in [
            "UnityCrashHandler64.exe",
            "UnityCrashHandler32.exe",
            "crashpad_handler.exe",
            "CrashReportClient.exe",
            "unins000.exe",
            "vc_redist.x64.exe",
            "dxwebsetup.exe",
            "UE4PrereqSetup_x64.exe",
            "game.exe",
            "dxsetup.exe",
        ] {
            assert!(!names.contains(&absent.to_string()), "{}", absent);
        }
        let _ = std::fs::remove_dir_all(&main);
    }

    #[test]
    fn choice_blocklisted_file_fails() {
        let main = mktmp("corky_exe_blocked_save");
        let crash = main.join("UnityCrashHandler64.exe");
        write_file(&crash);
        assert!(crash.is_file());
        assert!(!validate_exe_choice(
            &main.display().to_string(),
            &crash.display().to_string()
        ));
        let _ = std::fs::remove_dir_all(&main);
    }

    #[test]
    fn choice_crash_titled_games_pass() {
        let main = mktmp("corky_exe_crash_games");
        for name in ["CrashBandicoot.exe", "CrashTeamRacing.exe"] {
            let exe = main.join(name);
            write_file(&exe);
            assert!(validate_exe_choice(
                &main.display().to_string(),
                &exe.display().to_string()
            ));
        }
        let _ = std::fs::remove_dir_all(&main);
    }
}
