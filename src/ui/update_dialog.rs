//! Self-update UI: check button flow, progress dialog, restart prompt.
//!
//! Network and install work runs on worker threads; this module only
//! builds dialogs and polls an mpsc channel on the main loop. Only
//! plain data crosses the thread boundary (AppState is not Send).

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc};

use adw::prelude::*;

use crate::backend::self_update;
use crate::ui::helpers;
use crate::AppState;

/// Plain snapshot for worker threads (no GTK, no glib objects).
struct Snapshot {
    downloads_dir: PathBuf,
    install_dir: PathBuf,
    icon_dir: PathBuf,
    desktop_file: PathBuf,
    symlink: PathBuf,
    current: semver::Version,
}

fn snapshot(state: &AppState) -> Result<Snapshot, String> {
    let home = std::env::var("HOME").map_err(|_| "HOME is not set".to_string())?;
    let dirs = self_update::InstallDirs::fixed(&PathBuf::from(home));
    let exe = std::env::current_exe().map_err(|e| format!("cannot locate running binary: {}", e))?;
    if !self_update::installed_here(&exe, &dirs.install_dir) {
        return Err("This build was not installed with install.sh; update manually".into());
    }
    Ok(Snapshot {
        downloads_dir: state.config.downloads_dir(),
        install_dir: dirs.install_dir,
        icon_dir: dirs.icon_dir,
        desktop_file: dirs.desktop_file,
        symlink: dirs.symlink,
        current: self_update::current_version()?,
    })
}

fn dialog_shell(title: &str, width: i32) -> (adw::Dialog, gtk::Box) {
    let dlg = adw::Dialog::new();
    dlg.set_title(title);
    dlg.set_content_width(width);
    let (header, x_btn) = helpers::modal_header(title);
    {
        let d = dlg.clone();
        x_btn.connect_clicked(move |_| {
            d.close();
        });
    }
    let col = gtk::Box::new(gtk::Orientation::Vertical, 8);
    col.set_margin_top(4);
    col.set_margin_bottom(16);
    col.set_margin_start(16);
    col.set_margin_end(16);
    col.append(&header);
    (dlg, col)
}

fn body_label(text: &str) -> gtk::Label {
    let l = gtk::Label::new(Some(text));
    l.set_halign(gtk::Align::Start);
    l.set_wrap(true);
    l.add_css_class("info-value");
    l
}

/// Manual "Check for launcher updates" from Settings → About.
/// When an install already landed and waits for restart, the button
/// re-opens the restart prompt instead of re-checking.
pub fn check_now(state: &AppState, parent: &adw::ApplicationWindow) {
    // The backend flag is the single source of truth for "restart
    // pending"; the UI cache below only carries the tag/log to show.
    if self_update::install_pending_restart() {
        show_pending_restart(state, parent);
        return;
    }
    start_check(state, parent, true);
}

/// True once an update installed in this session and waits for restart.
pub fn restart_pending() -> bool {
    self_update::install_pending_restart()
}

static PENDING: std::sync::Mutex<Option<(String, Vec<String>)>> =
    std::sync::Mutex::new(None);

fn pending_restart() -> Option<(String, Vec<String>)> {
    PENDING.lock().ok().and_then(|g| g.clone())
}

fn mark_restart_pending(tag: &str, log: &[String]) {
    if let Ok(mut g) = PENDING.lock() {
        *g = Some((tag.to_string(), log.to_vec()));
    }
}

fn show_pending_restart(state: &AppState, parent: &adw::ApplicationWindow) {
    // Gate on the backend flag, not the UI cache: the cache may lag.
    if !self_update::install_pending_restart() {
        return;
    }
    if let Some((tag, log)) = pending_restart() {
        show_restart(state, parent, &tag, &log);
    }
}

/// Automatic startup check: silent unless an update is available.
pub fn auto_check(state: &AppState, parent: &adw::ApplicationWindow) {
    let enabled = state
        .config
        .launcher_value("checkUpdatesOnStartup")
        .map(|v| v == "1")
        .unwrap_or(false);
    let last = state
        .config
        .launcher_value("lastUpdateCheck")
        .and_then(|v| v.parse::<u64>().ok());
    if !self_update::should_auto_check(enabled, last, self_update::now_unix()) {
        return;
    }
    start_check(state, parent, false);
}

enum CheckMsg {
    Available(self_update::ReleaseInfo, Option<String>, bool),
    UpToDate(String, Option<String>),
    Failed(String),
}

fn start_check(state: &AppState, parent: &adw::ApplicationWindow, manual: bool) {
    // Single-flight: a check already in flight refuses this one with no
    // dialog and no thread. The guard lives until the poll ends. A manual
    // click gets a note; an automatic check returns silently.
    let Some(check_guard) = self_update::CheckGuard::acquire() else {
        if manual {
            show_note(parent, "Launcher updates", "A check is already in progress.");
        }
        return;
    };
    let snap = match snapshot(state) {
        Ok(s) => s,
        Err(e) => {
            if manual {
                show_error(parent, &e);
            }
            return;
        }
    };
    let skip = state.config.launcher_value("skipUpdateVersion").unwrap_or_default();
    let etag = state.config.launcher_value("lastUpdateEtag");
    let (tx, rx) = mpsc::channel::<CheckMsg>();
    std::thread::spawn(move || {
        let up_to_date = |etag: Option<String>| {
            CheckMsg::UpToDate(format!("You are up to date (v{}).", snap.current), etag)
        };
        // Parse, cache the body for future 304s, then decide.
        // `manual` ignores skip (always offer, with a note); the auto
        // path treats a skipped tag as up to date and stays silent.
        let finish = |body: &str, etag: Option<String>| -> CheckMsg {
            match self_update::parse_latest_release(body) {
                Err(e) => CheckMsg::Failed(e),
                Ok(info) => {
                    let _ = self_update::store_cached_latest(&snap.downloads_dir, body);
                    let skipped = !skip.trim().is_empty() && skip.trim() == info.tag;
                    match self_update::decide_update(&snap.current, &info, if manual { "" } else { &skip }) {
                        self_update::UpdateDecision::Available => {
                            CheckMsg::Available(info, etag, skipped && manual)
                        }
                        self_update::UpdateDecision::UpToDate => up_to_date(etag),
                    }
                }
            }
        };
        let decide_cached = || -> Option<CheckMsg> {
            let info = self_update::read_cached_latest(&snap.downloads_dir)?;
            let skipped = !skip.trim().is_empty() && skip.trim() == info.tag;
            match self_update::decide_update(&snap.current, &info, if manual { "" } else { &skip }) {
                self_update::UpdateDecision::Available => {
                    Some(CheckMsg::Available(info, etag.clone(), skipped && manual))
                }
                self_update::UpdateDecision::UpToDate => Some(up_to_date(etag.clone())),
            }
        };
        let msg = match self_update::api_get_latest(etag.as_deref()) {
            Err(e) => CheckMsg::Failed(e),
            Ok(self_update::ApiOutcome::Fresh(body, new_etag)) => finish(&body, new_etag),
            // A 304 without a usable cache proves nothing: retry once
            // without If-None-Match instead of reporting up to date.
            Ok(self_update::ApiOutcome::NotModified) => match decide_cached() {
                Some(m) => m,
                None => match self_update::api_get_latest(None) {
                    Err(e) => CheckMsg::Failed(e),
                    Ok(self_update::ApiOutcome::Fresh(body, new_etag)) => finish(&body, new_etag),
                    Ok(self_update::ApiOutcome::NotModified) => {
                        CheckMsg::Failed("update check inconclusive. Try again.".into())
                    }
                },
            },
        };
        let _ = tx.send(msg);
    });
    // Manual checks open a modal "Checking…"; auto checks stay silent.
    let dlg = if manual {
        let (dlg, col) = dialog_shell("Checking for updates", 380);
        col.append(&body_label("Checking for updates…"));
        dlg.set_child(Some(&col));
        dlg.present(Some(parent));
        Some(dlg)
    } else {
        None
    };
    let state_c = state.clone();
    let parent_c = parent.clone();
    glib::timeout_add_local(std::time::Duration::from_millis(100), move || {
        // Hold the single-flight gate for the whole check.
        let _gate = &check_guard;
        match rx.try_recv() {
            Ok(CheckMsg::Available(info, etag, skipped)) => {
                if let Some(d) = &dlg {
                    d.close();
                }
                record_check(&state_c, etag.as_deref());
                if manual {
                    show_available(&state_c, &parent_c, info, skipped);
                } else if !skipped {
                    show_available(&state_c, &parent_c, info, false);
                }
                glib::ControlFlow::Break
            }
            Ok(CheckMsg::UpToDate(text, etag)) => {
                if let Some(d) = &dlg {
                    d.close();
                }
                record_check(&state_c, etag.as_deref());
                if manual {
                    show_note(&parent_c, "Launcher updates", &text);
                }
                glib::ControlFlow::Break
            }
            Ok(CheckMsg::Failed(e)) => {
                if let Some(d) = &dlg {
                    d.close();
                }
                if manual {
                    show_error(&parent_c, &e);
                }
                glib::ControlFlow::Break
            }
            Err(mpsc::TryRecvError::Empty) => glib::ControlFlow::Continue,
            Err(_) => {
                if let Some(d) = &dlg {
                    d.close();
                }
                glib::ControlFlow::Break
            }
        }
    });
}

fn record_check(state: &AppState, etag: Option<&str>) {
    // ETag first, timestamp last: the two writes are separate, so a
    // crash between them leaves the *previous* check timestamp behind
    // (the gate for the next auto check) instead of advancing it with
    // no matching ETag recorded.
    if let Some(tag) = etag {
        if !tag.trim().is_empty() {
            state.config.set_launcher_value("lastUpdateEtag", tag.trim());
        }
    }
    state
        .config
        .set_launcher_value("lastUpdateCheck", &self_update::now_unix().to_string());
}

fn show_note(parent: &adw::ApplicationWindow, title: &str, text: &str) {
    let (dlg, col) = dialog_shell(title, 380);
    col.append(&body_label(text));
    let ok = gtk::Button::with_label("OK");
    ok.add_css_class("settings-btn");
    ok.set_halign(gtk::Align::End);
    {
        let d = dlg.clone();
        ok.connect_clicked(move |_| {
            d.close();
        });
    }
    col.append(&ok);
    dlg.set_child(Some(&col));
    dlg.present(Some(parent));
}

fn show_error(parent: &adw::ApplicationWindow, text: &str) {
    show_note(parent, "Update failed", text);
}

fn show_available(state: &AppState, parent: &adw::ApplicationWindow, info: self_update::ReleaseInfo, was_skipped: bool) {
    let (dlg, col) = dialog_shell("Launcher update available", 480);
    let title = gtk::Label::new(Some(&format!("CorkyTux {} is available", info.tag)));
    title.set_halign(gtk::Align::Start);
    title.add_css_class("details-title");
    col.append(&title);
    if was_skipped {
        col.append(&body_label("You skipped this version."));
    }
    if !info.name.trim().is_empty() && info.name.trim() != info.tag {
        col.append(&body_label(info.name.trim()));
    }
    let scroll = gtk::ScrolledWindow::new();
    scroll.set_min_content_height(140);
    scroll.set_vexpand(false);
    let notes = gtk::Label::new(Some(if info.notes.trim().is_empty() {
        "(No release notes.)"
    } else {
        info.notes.trim()
    }));
    notes.set_halign(gtk::Align::Start);
    notes.set_wrap(true);
    notes.set_selectable(true);
    notes.add_css_class("info-value");
    scroll.set_child(Some(&notes));
    col.append(&scroll);
    let row = gtk::Box::new(gtk::Orientation::Horizontal, 10);
    row.set_halign(gtk::Align::Fill);
    let skip = gtk::Button::with_label("Skip this version");
    skip.set_hexpand(true);
    {
        let d = dlg.clone();
        let state_c = state.clone();
        let tag = info.tag.clone();
        skip.connect_clicked(move |_| {
            state_c.config.set_launcher_value("skipUpdateVersion", &tag);
            d.close();
        });
    }
    let go = gtk::Button::with_label("Download and install");
    go.add_css_class("settings-btn");
    go.set_hexpand(true);
    {
        let d = dlg.clone();
        let state_c = state.clone();
        let parent_c = parent.clone();
        go.connect_clicked(move |_| {
            d.close();
            start_download(&state_c, &parent_c, info.clone());
        });
    }
    row.append(&skip);
    row.append(&go);
    col.append(&row);
    dlg.set_child(Some(&col));
    dlg.present(Some(parent));
}

enum DlMsg {
    Progress(u64, u64),
    Status(String),
    Done(Result<Vec<String>, String>),
}

fn start_download(state: &AppState, parent: &adw::ApplicationWindow, info: self_update::ReleaseInfo) {
    let snap = match snapshot(state) {
        Ok(s) => s,
        Err(e) => {
            show_error(parent, &e);
            return;
        }
    };
    let (dlg, col) = dialog_shell("Downloading update", 440);
    let status = body_label("Downloading…");
    col.append(&status);
    let bar = gtk::ProgressBar::new();
    bar.set_show_text(true);
    col.append(&bar);
    let cancel_btn = gtk::Button::with_label("Cancel");
    cancel_btn.set_halign(gtk::Align::End);
    col.append(&cancel_btn);
    dlg.set_child(Some(&col));
    let parent_c = parent.clone();
    dlg.present(Some(&parent_c));

    let cancel = Arc::new(AtomicBool::new(false));
    {
        let c = cancel.clone();
        let d = dlg.clone();
        cancel_btn.connect_clicked(move |_| {
            c.store(true, Ordering::Relaxed);
            d.close();
        });
    }
    // A second handle for the poll: a cancelled run must close silently.
    let cancel_poll = cancel.clone();
    let (tx, rx) = mpsc::channel::<DlMsg>();
    let info_c = info.clone();
    // Gated start through the tested backend decision: a landed
    // install wins, then single flight across windows. Refusals show
    // the error with no thread spawned; a dropped guard releases.
    let run = self_update::UpdateRunGuard::try_acquire();
    if let Err(e) = self_update::start_run_decision(run.is_none()) {
        drop(run);
        show_error(&parent_c, &e);
        return;
    }
    let run = run.expect("guard held after gate");
    std::thread::spawn(move || {
        let res = download_and_install(&snap, &info_c, &tx, &cancel);
        // A landed install raises the process-wide restart flag and then
        // releases the guard (finish_installed), so any new attempt from
        // any window sees "restart to finish". A failed run just drops
        // the guard.
        if res.is_ok() {
            run.finish_installed();
        }
        let _ = tx.send(DlMsg::Done(res));
    });
    let state_c = state.clone();
    glib::timeout_add_local(std::time::Duration::from_millis(100), move || {
        match rx.try_recv() {
            Ok(DlMsg::Progress(done, total)) => {
                if total > 0 {
                    bar.set_fraction(done as f64 / total as f64);
                    bar.set_text(Some(&format!("{} / {} MB", done / 1_048_576, total / 1_048_576)));
                } else {
                    bar.pulse();
                }
                glib::ControlFlow::Continue
            }
            Ok(DlMsg::Status(s)) => {
                status.set_text(&s);
                if s == "Installing…" {
                    cancel_btn.set_sensitive(false);
                }
                glib::ControlFlow::Continue
            }
            Ok(DlMsg::Done(r)) => {
                dlg.close();
                match r {
                    Ok(log) => show_restart(&state_c, &parent_c, &info.tag, &log),
                    Err(e) => {
                        // A user cancellation is not a failure: close
                        // silently instead of an "Update failed" dialog.
                        if let Some(msg) =
                            self_update::failure_to_show(cancel_poll.load(Ordering::Relaxed), &e)
                        {
                            show_error(&parent_c, &msg);
                        }
                    }
                }
                glib::ControlFlow::Break
            }
            Err(mpsc::TryRecvError::Empty) => glib::ControlFlow::Continue,
            Err(_) => {
                dlg.close();
                glib::ControlFlow::Break
            }
        }
    });
}

/// Worker body: download, verify, extract, install. Cleans staging
/// and partials on every exit path. Never touches Games.ini,
/// Launcher.ini, prefixes, or anything under the user library.
fn download_and_install(
    snap: &Snapshot,
    info: &self_update::ReleaseInfo,
    tx: &mpsc::Sender<DlMsg>,
    cancel: &Arc<AtomicBool>,
) -> Result<Vec<String>, String> {
    let _ = tx.send(DlMsg::Status("Downloading…".into()));
    // Single flight is held by the caller: sweep other runs' stale
    // leftovers first. Verified tarballs and latest.json never match.
    self_update::sweep_stale_downloads(
        &snap.downloads_dir,
        self_update::SWEEP_MAX_AGE_SECS,
    );
    let asset = self_update::select_asset(info)?;
    // Validate the optional checksums.txt size before anything is fetched.
    let sums = self_update::select_checksums_asset(info)?;
    let tarball = snap.downloads_dir.join(&asset.name);
    let part = snap.downloads_dir.join(self_update::partial_name(&asset.name));
    let staging = snap.downloads_dir.join(self_update::staging_name(&info.tag));
    let cleanup_all = || {
        self_update::cleanup(&[staging.as_path(), part.as_path(), tarball.as_path()]);
    };
    let fail = |e: String| {
        cleanup_all();
        Err(e)
    };
    if !self_update::asset_origin_ok(&asset.browser_download_url, &info.tag, &asset.name) {
        return fail("release asset URL is not from this repository. Aborted.".into());
    }
    let want = match self_update::require_digest(asset) {
        Ok(w) => w,
        Err(e) => return fail(e),
    };
    let cancelled = || cancel.load(Ordering::Relaxed);
    let progress = |done: u64, total: u64| {
        let _ = tx.send(DlMsg::Progress(done, total));
    };
    self_update::download_asset(&asset.browser_download_url, &part, asset.size, &progress, cancel)
        .map_err(|e| {
            cleanup_all();
            e
        })?;
    if cancelled() {
        return fail("download cancelled".into());
    }
    std::fs::rename(&part, &tarball).map_err(|e| {
        cleanup_all();
        format!("cannot stage download: {}", e)
    })?;
    let _ = tx.send(DlMsg::Status("Verifying…".into()));
    if cancelled() {
        return fail("download cancelled".into());
    }
    // API digest is mandatory; checksums.txt, when present, must agree.
    // Its size was validated right after select_asset, before any fetch.
    if let Some(sums) = sums {
        if !self_update::asset_origin_ok(&sums.browser_download_url, &info.tag, &sums.name) {
            return fail("checksums.txt URL is not from this repository. Aborted.".into());
        }
        let sums_part = snap.downloads_dir.join(self_update::partial_name("checksums.txt"));
        let sums_file = snap.downloads_dir.join("checksums.txt");
        let dl = self_update::download_asset_limited(
            &sums.browser_download_url,
            &sums_part,
            sums.size,
            self_update::CHECKSUMS_MAX_BYTES,
            &|_, _| {},
            cancel,
        );
        match dl {
            Ok(_) => {
                let _ = std::fs::rename(&sums_part, &sums_file);
                let content = match self_update::read_file_capped(
                    &sums_file,
                    self_update::CHECKSUMS_MAX_BYTES,
                ) {
                    Ok(c) => c,
                    Err(e) => {
                        let _ = std::fs::remove_file(&sums_file);
                        return fail(format!("cannot read checksums.txt: {}", e));
                    }
                };
                let _ = std::fs::remove_file(&sums_file);
                if cancelled() {
                    return fail("download cancelled".into());
                }
                if let Err(e) = self_update::checksums_agree(&content, &asset.name, &want) {
                    return fail(e);
                }
            }
            Err(e) => {
                let _ = std::fs::remove_file(&sums_part);
                return fail(format!("cannot fetch checksums.txt: {}", e));
            }
        }
    }
    if cancelled() {
        return fail("download cancelled".into());
    }
    if let Err(e) = self_update::verify_artifact(&tarball, &want) {
        return fail(e);
    }
    let staged = self_update::extract_tarball(&tarball, &staging).map_err(|e| {
        cleanup_all();
        e
    })?;
    let _ = tx.send(DlMsg::Status("Installing…".into()));
    let dirs = self_update::InstallDirs {
        install_dir: snap.install_dir.clone(),
        icon_dir: snap.icon_dir.clone(),
        desktop_file: snap.desktop_file.clone(),
        symlink: snap.symlink.clone(),
    };
    // No downgrade, even if the flow was started with stale info.
    if !self_update::needs_update(&snap.current, &info.version) {
        return fail(format!("You are up to date (v{}).", snap.current));
    }
    let log = self_update::install_if_healthy(&staged, &dirs, &info.version).map_err(|e| {
        cleanup_all();
        e
    })?;
    // The install landed. The caller raises the restart flag and releases
    // the run guard atomically via `UpdateRunGuard::finish_installed`.
    cleanup_all();
    Ok(log)
}

fn show_restart(state: &AppState, parent: &adw::ApplicationWindow, tag: &str, log: &[String]) {
    mark_restart_pending(tag, log);
    let (dlg, col) = dialog_shell("Update installed", 440);
    col.append(&body_label(&format!(
        "CorkyTux {} is installed.\n{}",
        tag,
        log.join("\n")
    )));
    if state.proton.is_game_running() {
        let game = state.proton.session_game_name();
        col.append(&body_label(&format!(
            "{} is still running. Restarting the launcher will not stop your game.",
            if game.is_empty() { "A game".into() } else { game }
        )));
    }
    let row = gtk::Box::new(gtk::Orientation::Horizontal, 10);
    row.set_halign(gtk::Align::Fill);
    let later = gtk::Button::with_label("Later");
    later.set_hexpand(true);
    {
        let d = dlg.clone();
        later.connect_clicked(move |_| {
            d.close();
        });
    }
    let now = gtk::Button::with_label("Restart now");
    now.add_css_class("settings-btn");
    now.set_hexpand(true);
    {
        let d = dlg.clone();
        let parent_c = parent.clone();
        let exe = std::env::current_exe()
            .ok()
            .and_then(|e| e.parent().map(|p| p.join("corkytux")));
        now.connect_clicked(move |_| {
            // Quit first, relaunch after: this app is single-instance,
            // so spawn-then-quit would rendezvous with the still-alive
            // primary and exit windowless. Failures keep this process.
            let launched = exe
                .clone()
                .map(|bin| self_update::spawn_restart(&bin))
                .unwrap_or(Err("cannot locate new binary".into()));
            if let Err(e) = launched {
                show_error(&parent_c, &e);
                return;
            }
            d.close();
            if let Some(app) = parent_c.application() {
                app.quit();
            } else {
                parent_c.close();
            }
        });
    }
    row.append(&later);
    row.append(&now);
    col.append(&row);
    dlg.set_child(Some(&col));
    dlg.present(Some(parent));
}
