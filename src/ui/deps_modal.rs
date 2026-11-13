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
    pub name: &'static str,
    pub description: &'static str,
    pub size: &'static str,
}

pub const DEPS: [DepInfo; 3] = [
    DepInfo {
        id: DepId::Legendary,
        name: "legendary",
        description: "Epic Games client: library and sign-in.",
        size: "~3 MB",
    },
    DepInfo {
        id: DepId::Gogdl,
        name: "gogdl",
        description: "GOG client: library and sign-in.",
        size: "~1.5 MB",
    },
    DepInfo {
        id: DepId::Chromium,
        name: "Login browser",
        description: "Isolated Chromium for sign-in only. It never touches your own browser.",
        size: "~188 MB",
    },
];

/// How the modal ended (it always closes through one of these two ways).
pub enum InstallOutcome {
    /// All ready: the caller refreshes and the modal already closed itself.
    Ready,
    /// Out without installing: show "Setup incomplete".
    Skip,
}

/// Messages from the installer thread to the GTK thread.
enum InstallProgress {
    RowActive(usize),
    RowProgress(usize, u8, u64),
    RowReady(usize),
    RowFailed(usize, String),
    Finished,
}

#[derive(Clone)]
struct RowWidgets {
    spin: gtk::Spinner,
    status: gtk::Label,
}

/// Logical state of a row, mirrored from the widget for GTK-free decisions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RowState {
    Waiting,
    Active,
    Ready,
    Failed,
}

/// The "Skip for now" link shows as soon as ANY row is Failed, whatever the
/// other rows do (in progress, success, another failure). It doesn't wait for
/// all of them: that wait was the bug (a row in progress blocked the link even
/// after another one had already failed).
fn show_skip(states: &[RowState]) -> bool {
    states.contains(&RowState::Failed)
}

/// Fixed neon green for "Ready": hardcoded on purpose, it does NOT follow the
/// light/dark theme (explicit design decision). Registered once with its own
/// provider, so no theme change can touch it.
const READY_CSS: &str = ".deps-ready { color: #39FF14; }";

fn ensure_ready_css() {
    use std::sync::OnceLock;
    static READY: OnceLock<()> = OnceLock::new();
    READY.get_or_init(|| {
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
/// Backend exceptions and helper diagnostics may include messages from
/// external tools in a Spanish locale. Keep those compatibility patterns;
/// current plugin/helper messages are English, not intrinsically Spanish.
/// Unmatched messages remain raw and truncated rather than inventing a cause.
pub fn short_reason(source: &str, text: &str) -> String {
    let t = format!("{} {}", source, text).to_lowercase();
    for keyword in [
        "temporary failure",
        "name or service",
        "urlerror",
        "connection",
        "timed out",
        "network",
        "unreachable",
        "cut off",
        "rejected the download",
        // Compatibility with external tools in a Spanish locale.
        "red al conectar",
        "conexi",
        "cortó",
        "rechazo la descarga",
    ] {
        if t.contains(keyword) {
            return "no connection".to_string();
        }
    }
    for keyword in [
        "no space",
        "disk space",
        "errno 28",
        "espacio en disco",
        "sin espacio",
    ] {
        if t.contains(keyword) {
            return "no disk space".to_string();
        }
    }
    let raw: String = text.trim().chars().take(90).collect();
    if raw.is_empty() {
        "unknown error".to_string()
    } else {
        raw
    }
}

/// Installs what's pending in its own thread: legendary/gogdl first through
/// `cmd_setup` (which skips whatever is already there), then Chromium through
/// `webdriver_login --prefetch` with JSON progress on stdout.
fn install_pending(
    pending: Vec<DepId>,
    helper: Option<std::path::PathBuf>,
    tx: std::sync::mpsc::Sender<InstallProgress>,
) {
    let send = |m: InstallProgress| {
        let _ = tx.send(m);
    };
    let index_of = |id: DepId| pending.iter().position(|x| *x == id);

    let needs_legendary = pending.contains(&DepId::Legendary);
    let needs_gogdl = pending.contains(&DepId::Gogdl);
    if needs_legendary || needs_gogdl {
        for id in [DepId::Legendary, DepId::Gogdl] {
            if let Some(i) = index_of(id) {
                send(InstallProgress::RowActive(i));
            }
        }
        match crate::backend::external::StoreManager::setup() {
            Ok(v) => {
                let failures = v
                    .get("failed")
                    .and_then(|x| x.as_array())
                    .map(|a| a.iter().filter_map(|x| x.as_str()).collect::<Vec<_>>())
                    .unwrap_or_default();
                for (id, name) in [(DepId::Legendary, "legendary"), (DepId::Gogdl, "gogdl")] {
                    // index_of only returns Some for a pending item.
                    let Some(i) = index_of(id) else { continue };
                    match failures.iter().find(|e| e.starts_with(name)) {
                        Some(e) => send(InstallProgress::RowFailed(i, short_reason("setup", e))),
                        None => send(InstallProgress::RowReady(i)),
                    }
                }
            }
            Err(e) => {
                for id in [DepId::Legendary, DepId::Gogdl] {
                    if let Some(i) = index_of(id) {
                        if pending.contains(&id) {
                            send(InstallProgress::RowFailed(i, short_reason("setup", &e)));
                        }
                    }
                }
            }
        }
    }

    if pending.contains(&DepId::Chromium) {
        let Some(i) = index_of(DepId::Chromium) else {
            send(InstallProgress::Finished);
            return;
        };
        send(InstallProgress::RowActive(i));
        let Some(bin) = helper else {
            send(InstallProgress::RowFailed(
                i,
                "login helper missing".to_string(),
            ));
            send(InstallProgress::Finished);
            return;
        };
        let mut child = match std::process::Command::new(&bin)
            .arg("--prefetch")
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
        {
            Ok(c) => c,
            Err(e) => {
                send(InstallProgress::RowFailed(
                    i,
                    short_reason("chrome", &e.to_string()),
                ));
                send(InstallProgress::Finished);
                return;
            }
        };
        use std::io::{BufRead, Read};
        let mut saw_done = false;
        if let Some(output) = child.stdout.take() {
            for line in std::io::BufReader::new(output).lines() {
                let Ok(line) = line else { break };
                let Ok(v) = serde_json::from_str::<serde_json::Value>(&line) else {
                    continue;
                };
                match v.get("type").and_then(|x| x.as_str()) {
                    Some("chrome_progress") => {
                        let pct = v.get("percent").and_then(|x| x.as_u64()).unwrap_or(0) as u8;
                        let mb = v.get("mb").and_then(|x| x.as_u64()).unwrap_or(0);
                        send(InstallProgress::RowProgress(i, pct, mb));
                    }
                    Some("chrome_done") => {
                        saw_done = true;
                        send(InstallProgress::RowReady(i));
                    }
                    _ => {}
                }
            }
        }
        let mut remaining = String::new();
        if let Some(mut err) = child.stderr.take() {
            let _ = err.read_to_string(&mut remaining);
        }
        match child.wait() {
            Ok(st) if st.success() => {
                if !saw_done {
                    send(InstallProgress::RowReady(i));
                }
            }
            _ => {
                let msg = remaining
                    .lines()
                    .rev()
                    .find(|l| l.contains("ERRO:"))
                    .map(|l| l.splitn(3, ':').nth(2).unwrap_or(l).trim().to_string())
                    .unwrap_or_else(|| remaining.trim().to_string());
                send(InstallProgress::RowFailed(i, short_reason("chrome", &msg)));
            }
        }
        send(InstallProgress::Finished);
    } else {
        send(InstallProgress::Finished);
    }
}

/// Open the modal with pending rows in DEPS order.
///
/// No X and no close button: the only exits are total success (auto-close +
/// `InstallOutcome::Ready`) and the "Skip for now" link, which only shows
/// after ≥1 failed attempt (`InstallOutcome::Skip`).
pub fn present(
    parent: &impl IsA<gtk::Widget>,
    pending: Vec<DepId>,
    helper: Option<std::path::PathBuf>,
    on_finished: impl Fn(InstallOutcome) + 'static,
) {
    if pending.is_empty() {
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
    let title = gtk::Label::new(Some("Install dependencies"));
    title.add_css_class("modal-title");
    title.set_halign(gtk::Align::Start);
    title.set_margin_start(24);
    title.set_margin_end(16);
    title.set_margin_top(16);
    inner.append(&title);
    let sep = gtk::Separator::new(gtk::Orientation::Horizontal);
    sep.set_halign(gtk::Align::Fill);
    inner.append(&sep);

    let intro = gtk::Label::new(Some(
        "Stores needs these tools to work. They are downloaded once.",
    ));
    intro.set_halign(gtk::Align::Start);
    intro.set_wrap(true);
    intro.add_css_class("info-value");
    inner.append(&intro);

    let list = gtk::ListBox::new();
    list.set_selection_mode(gtk::SelectionMode::None);
    inner.append(&list);

    let mut rows: Vec<RowWidgets> = Vec::new();
    for id in &pending {
        let info = DEPS
            .iter()
            .find(|d| d.id == *id)
            .expect("DepId covered by DEPS");
        let row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        row.set_margin_top(4);
        row.set_margin_bottom(4);
        row.set_margin_start(4);
        row.set_margin_end(4);
        let texts = gtk::Box::new(gtk::Orientation::Vertical, 2);
        texts.set_hexpand(true);
        let name = gtk::Label::new(Some(info.name));
        name.set_halign(gtk::Align::Start);
        texts.append(&name);
        let desc = gtk::Label::new(Some(info.description));
        desc.set_halign(gtk::Align::Start);
        desc.set_wrap(true);
        desc.add_css_class("dim-label");
        texts.append(&desc);
        row.append(&texts);
        let size = gtk::Label::new(Some(info.size));
        size.set_halign(gtk::Align::End);
        size.set_valign(gtk::Align::Center);
        size.add_css_class("dim-label");
        row.append(&size);
        let spin = gtk::Spinner::new();
        spin.set_valign(gtk::Align::Center);
        row.append(&spin);
        let status = gtk::Label::new(Some("Waiting…"));
        status.set_valign(gtk::Align::Center);
        status.add_css_class("dim-label");
        row.append(&status);
        list.append(&row);
        rows.push(RowWidgets { spin, status });
    }

    let progress_bar = gtk::ProgressBar::new();
    progress_bar.set_show_text(true);
    progress_bar.set_text(Some("0 of 0"));
    inner.append(&progress_bar);

    let install_button = gtk::Button::with_label("Install All");
    install_button.add_css_class("suggested-action");
    install_button.set_hexpand(true);
    inner.append(&install_button);

    // Escape valve: a small link, only after ≥1 failed attempt. It doesn't
    // compete with "Install All" and never shows on the first render.
    let skip = gtk::Button::with_label("Skip for now");
    skip.add_css_class("flat");
    skip.add_css_class("dim-label");
    skip.set_halign(gtk::Align::Center);
    skip.set_visible(false);
    inner.append(&skip);

    dialog.set_child(Some(&content));
    ensure_ready_css();

    let on_finished = Rc::new(on_finished);
    let dialog_alive: Rc<std::cell::Cell<bool>> = Rc::new(std::cell::Cell::new(true));
    {
        let alive = dialog_alive.clone();
        dialog.connect_closed(move |_| {
            alive.set(false);
        });
    }
    let (tx, rx) = std::sync::mpsc::channel::<InstallProgress>();
    let rx = Rc::new(std::cell::RefCell::new(rx));

    // State shared with the poller.
    struct ModalState {
        rows: Vec<RowWidgets>,
        states: Vec<RowState>,
        progress_bar: gtk::ProgressBar,
        install_button: gtk::Button,
        skip: gtk::Button,
        ready_count: usize,
        total: usize,
        accumulated_failures: u32,
        installing: bool,
    }
    let state = Rc::new(std::cell::RefCell::new(ModalState {
        rows,
        states: vec![RowState::Waiting; pending.len()],
        progress_bar: progress_bar.clone(),
        install_button: install_button.clone(),
        skip: skip.clone(),
        ready_count: 0,
        total: pending.len(),
        accumulated_failures: 0,
        installing: false,
    }));

    let start = {
        let state = state.clone();
        let pending = pending.clone();
        let helper = helper.clone();
        move || {
            let mut e = state.borrow_mut();
            if e.installing {
                return;
            }
            e.installing = true;
            e.ready_count = 0;
            e.install_button.set_sensitive(false);
            for i in 0..e.rows.len() {
                if let Some(f) = e.rows.get(i) {
                    f.spin.stop();
                    f.spin.set_visible(false);
                    f.status.remove_css_class("deps-ready");
                    f.status.set_text("Waiting…");
                }
                // Reset attempt states, but never hide the skip link once a
                // previous failure made it available.
                if let Some(s) = e.states.get_mut(i) {
                    *s = RowState::Waiting;
                }
            }
            e.progress_bar.set_fraction(0.0);
            e.progress_bar.set_text(Some(&format!("0 of {}", e.total)));
            let thread_tx = tx.clone();
            let thread_pending = pending.clone();
            let thread_helper = helper.clone();
            drop(e);
            std::thread::spawn(move || {
                install_pending(thread_pending, thread_helper, thread_tx);
            });
        }
    };
    let start = Rc::new(start);
    {
        let l = start.clone();
        install_button.connect_clicked(move |_| {
            l();
        });
    }
    // No autostart: installing begins ONLY on pressing "Install All". (An
    // earlier version called start() here, so the modal installed itself on
    // open — which also made the skip link appear before any attempt.)
    {
        let t = on_finished.clone();
        let d = dialog.clone();
        skip.connect_clicked(move |_| {
            d.force_close();
            t(InstallOutcome::Skip);
        });
    }

    // Poller like ProtonModal's: drains the channel every 100 ms and dies with
    // the dialog.
    {
        let state = state.clone();
        let rx_poll = rx.clone();
        let alive_poll = dialog_alive.clone();
        let t = on_finished.clone();
        let d = dialog.clone();
        glib::timeout_add_local(std::time::Duration::from_millis(100), move || {
            if !alive_poll.get() {
                return glib::ControlFlow::Break;
            }
            let messages: Vec<InstallProgress> = {
                let r = rx_poll.borrow();
                let mut v = Vec::new();
                while let Ok(m) = r.try_recv() {
                    v.push(m);
                }
                v
            };
            for m in messages {
                let mut e = state.borrow_mut();
                match m {
                    InstallProgress::RowActive(i) => {
                        if let Some(f) = e.rows.get(i) {
                            f.spin.set_visible(true);
                            f.spin.start();
                            f.status.remove_css_class("deps-ready");
                            f.status.set_text("Downloading…");
                        }
                        if let Some(s) = e.states.get_mut(i) {
                            *s = RowState::Active;
                        }
                    }
                    InstallProgress::RowProgress(i, pct, mb) => {
                        if let Some(f) = e.rows.get(i) {
                            f.status.set_text(&format!("{}% ({} MB)", pct, mb));
                        }
                    }
                    InstallProgress::RowReady(i) => {
                        if let Some(f) = e.rows.get(i) {
                            f.spin.stop();
                            f.spin.set_visible(false);
                            f.status.set_text("Ready");
                            f.status.add_css_class("deps-ready");
                        }
                        if let Some(s) = e.states.get_mut(i) {
                            *s = RowState::Ready;
                        }
                        e.ready_count += 1;
                        e.progress_bar
                            .set_fraction(e.ready_count as f64 / e.total.max(1) as f64);
                        e.progress_bar
                            .set_text(Some(&format!("{} of {}", e.ready_count, e.total)));
                    }
                    InstallProgress::RowFailed(i, reason) => {
                        if let Some(f) = e.rows.get(i) {
                            f.spin.stop();
                            f.spin.set_visible(false);
                            f.status.remove_css_class("deps-ready");
                            f.status.set_text(&format!("Failed: {}", reason));
                        }
                        if let Some(s) = e.states.get_mut(i) {
                            *s = RowState::Failed;
                        }
                        // Timing fix: the link shows on the first failure,
                        // without waiting for the rest. Monotonic: once visible
                        // it never hides (rule I settled on).
                        if show_skip(&e.states) {
                            e.skip.set_visible(true);
                        }
                    }
                    InstallProgress::Finished => {
                        e.installing = false;
                        e.install_button.set_sensitive(true);
                        if e.ready_count >= e.total {
                            e.progress_bar.set_text(Some("All dependencies installed."));
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
                                    tt(InstallOutcome::Ready);
                                },
                            );
                        } else {
                            e.accumulated_failures += 1;
                            e.progress_bar.set_text(Some("Press Install All to retry."));
                            // The link already showed on the first failure
                            // (show_skip rule); nothing is decided here.
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
    fn short_reason_detects_no_connection() {
        assert_eq!(
            short_reason(
                "setup",
                "gogdl: <urlopen error [Errno -2] Name or service not known>"
            ),
            "no connection"
        );
        // Compatibility fixtures for external diagnostics in a Spanish locale.
        assert_eq!(
            short_reason("chrome", "fallo de red al conectar: bla"),
            "no connection"
        );
        assert_eq!(
            short_reason("chrome", "la descarga se cortó o falló tras 4 intentos"),
            "no connection"
        );
        // The login helper's own English wording.
        assert_eq!(
            short_reason(
                "chrome",
                "the download was cut off or failed after 4 attempts"
            ),
            "no connection"
        );
        assert_eq!(
            short_reason("chrome", "the server rejected the download: 403"),
            "no connection"
        );
    }

    #[test]
    fn short_reason_detects_no_disk_space() {
        assert_eq!(
            short_reason("setup", "gogdl: [Errno 28] No space left on device"),
            "no disk space"
        );
        assert_eq!(
            short_reason("chrome", "could not write to disk: no space left"),
            "no disk space"
        );
        // Compatibility fixture for external diagnostics in a Spanish locale.
        assert_eq!(
            short_reason("chrome", "no se pudo escribir en disco: sin espacio"),
            "no disk space"
        );
    }

    #[test]
    fn short_reason_preserves_unclassified_text() {
        assert_eq!(
            short_reason("setup", "gogdl: permiso denegado"),
            "gogdl: permiso denegado"
        );
        assert_eq!(short_reason("setup", "   "), "unknown error");
    }

    #[test]
    fn all_three_dependencies_have_metadata() {
        assert_eq!(DEPS.len(), 3);
        assert!(DEPS
            .iter()
            .any(|d| d.id == DepId::Chromium && d.size == "~188 MB"));
    }

    /// The reported bug: row A Failed, row B still in progress (neither
    /// success nor failure) → the link must already be visible, without
    /// waiting for B.
    #[test]
    fn show_skip_with_failure_and_active_row() {
        assert!(show_skip(&[RowState::Failed, RowState::Active]));
    }

    /// With no failure there's no link, in no intermediate or final state.
    #[test]
    fn show_skip_without_failures_stays_hidden() {
        assert!(!show_skip(&[RowState::Waiting, RowState::Waiting]));
        assert!(!show_skip(&[RowState::Active, RowState::Waiting]));
        assert!(!show_skip(&[RowState::Ready, RowState::Ready]));
        assert!(!show_skip(&[]));
    }

    /// A failure next to a success also shows the link (rest ignored).
    #[test]
    fn show_skip_with_failure_and_success() {
        assert!(show_skip(&[RowState::Ready, RowState::Failed]));
    }
}
