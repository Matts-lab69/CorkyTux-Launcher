//! System tray (StatusNotifierItem) and background mode.
//!
//! The tray service runs on its own thread inside ksni and never touches a
//! GTK object. Menu events travel to the main loop through an async channel,
//! drained with `glib::MainContext::spawn_local`.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

/// What the tray asks the main loop to do.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TrayMsg {
    Show,
    Quit,
}

/// Close-to-tray decision (D1): hide only when a tray exists and the user
/// asked for it; otherwise the X must really close the app.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CloseAction {
    Hide,
    Quit,
    Ask,
}

/// Pure close decision. `remembered` is `Some("tray")` or `Some("quit")`.
pub fn should_hide_on_close(
    tray_available: bool,
    setting: bool,
    remembered: Option<&str>,
) -> CloseAction {
    if !tray_available || !setting {
        return CloseAction::Quit;
    }
    match remembered {
        Some("tray") => CloseAction::Hide,
        Some("quit") => CloseAction::Quit,
        _ => CloseAction::Ask,
    }
}

/// Exec line for the autostart file. An AppImage's `current_exe()` points at
/// the temporary mount, so `APPIMAGE` wins when it is set.
pub fn build_autostart_exec(appimage_env: Option<&str>, current_exe: &Path) -> String {
    let program = match appimage_env {
        Some(path) if !path.trim().is_empty() => path.trim(),
        _ => current_exe.to_str().unwrap_or(""),
    };
    // A replaced binary shows up as "path (deleted)": it is not a valid exec.
    let program = program.strip_suffix(" (deleted)").unwrap_or(program);
    format!("{} --background", shell_quote(program))
}

fn shell_quote(value: &str) -> String {
    let escaped = value.replace('\\', "\\\\").replace('"', "\\\"");
    format!("\"{}\"", escaped)
}

/// `~/.config/autostart/com.corkytux.CorkyTux.desktop`.
pub fn autostart_path() -> Option<PathBuf> {
    let home = std::env::var("HOME").ok().filter(|h| !h.is_empty())?;
    Some(
        PathBuf::from(home)
            .join(".config/autostart/com.corkytux.CorkyTux.desktop"),
    )
}

/// Writes or removes the autostart entry. Best effort: errors only log.
pub fn apply_autostart(enabled: bool) -> Result<(), String> {
    let Some(path) = autostart_path() else {
        return Err("no HOME for the autostart entry".to_string());
    };
    if !enabled {
        match std::fs::remove_file(&path) {
            Ok(()) => return Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(()),
            Err(e) => return Err(format!("cannot remove {}: {}", path.display(), e)),
        }
    }
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("cannot create {}: {}", parent.display(), e))?;
    }
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let appimage = std::env::var("APPIMAGE").ok();
    let exec = build_autostart_exec(appimage.as_deref(), &exe);
    let body = format!(
        "[Desktop Entry]\nType=Application\nName=CorkyTux\nComment=Game launcher\nExec={}\nX-GNOME-Autostart-enabled=true\nTerminal=false\n",
        exec
    );
    std::fs::write(&path, body).map_err(|e| format!("cannot write {}: {}", path.display(), e))
}

/// Icon as ARGB32 for the tray when the theme has no `corkytux` icon.
fn icon_pixmap() -> Vec<ksni::Icon> {
    const PNG: &[u8] = include_bytes!("../../release/corkytux.png");
    let Ok(pix) = gdk_pixbuf::Pixbuf::from_read(std::io::Cursor::new(PNG)) else {
        return Vec::new();
    };
    let pix = if pix.width() > 64 || pix.height() > 64 {
        pix.scale_simple(64, 64, gdk_pixbuf::InterpType::Bilinear)
            .unwrap_or(pix)
    } else {
        pix
    };
    let width = pix.width();
    let height = pix.height();
    let channels = pix.n_channels() as usize;
    let stride = pix.rowstride() as usize;
    let bytes = pix.read_pixel_bytes();
    let bytes = bytes.as_ref();
    let mut data = Vec::with_capacity((width * height * 4) as usize);
    for y in 0..height as usize {
        for x in 0..width as usize {
            let i = y * stride + x * channels;
            if i + 2 >= bytes.len() {
                continue;
            }
            let (r, g, b, a) = if channels >= 4 {
                (bytes[i], bytes[i + 1], bytes[i + 2], bytes[i + 3])
            } else {
                (bytes[i], bytes[i + 1], bytes[i + 2], 255)
            };
            data.extend_from_slice(&[a, r, g, b]);
        }
    }
    vec![ksni::Icon {
        width,
        height,
        data,
    }]
}

/// Shared state the tray reads. No GTK in here.
pub struct TrayShared {
    pub playing: Arc<Mutex<Option<String>>>,
    pub watcher_alive: Arc<AtomicBool>,
    pub tx: async_channel::Sender<TrayMsg>,
}

pub struct CorkyTray {
    playing: Arc<Mutex<Option<String>>>,
    watcher_alive: Arc<AtomicBool>,
    tx: async_channel::Sender<TrayMsg>,
}

impl ksni::Tray for CorkyTray {
    fn id(&self) -> String {
        "corkytux".to_string()
    }

    fn title(&self) -> String {
        "CorkyTux".to_string()
    }

    fn icon_name(&self) -> String {
        "corkytux".to_string()
    }

    fn icon_pixmap(&self) -> Vec<ksni::Icon> {
        icon_pixmap()
    }

    fn watcher_online(&self) {
        self.watcher_alive.store(true, Ordering::Relaxed);
    }

    fn watcher_offline(&self, reason: ksni::OfflineReason) -> bool {
        self.watcher_alive.store(false, Ordering::Relaxed);
        eprintln!("[tray] StatusNotifierWatcher offline: {:?}", reason);
        // Never leave the app hidden and unreachable.
        let _ = self.tx.try_send(TrayMsg::Show);
        true
    }

    fn menu(&self) -> Vec<ksni::MenuItem<Self>> {
        use ksni::menu::{MenuItem, StandardItem};
        let mut items = Vec::new();
        items.push(MenuItem::Standard(StandardItem {
            label: "Show CorkyTux".to_string(),
            activate: Box::new(|tray: &mut Self| {
                let _ = tray.tx.try_send(TrayMsg::Show);
            }),
            ..Default::default()
        }));
        let playing = self.playing.lock().ok().and_then(|p| p.clone());
        if let Some(game) = playing {
            items.push(MenuItem::Standard(StandardItem {
                label: format!("Playing: {}", game),
                enabled: false,
                ..Default::default()
            }));
        }
        items.push(MenuItem::Separator);
        items.push(MenuItem::Standard(StandardItem {
            label: "Quit".to_string(),
            activate: Box::new(|tray: &mut Self| {
                let _ = tray.tx.try_send(TrayMsg::Quit);
            }),
            ..Default::default()
        }));
        items
    }
}

/// Spawns the tray. `false` when there is no StatusNotifierWatcher (GNOME
/// without AppIndicator, headless sessions): the X must close for real then.
pub fn spawn(shared: TrayShared) -> Option<ksni::blocking::Handle<CorkyTray>> {
    use ksni::blocking::TrayMethods;
    let tray = CorkyTray {
        playing: shared.playing,
        watcher_alive: shared.watcher_alive,
        tx: shared.tx,
    };
    match tray.spawn() {
        Ok(handle) => Some(handle),
        Err(e) => {
            eprintln!("[tray] no StatusNotifierWatcher: {} (close to tray disabled)", e);
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn should_hide_on_close_table() {
        assert_eq!(should_hide_on_close(false, true, None), CloseAction::Quit);
        assert_eq!(should_hide_on_close(false, true, Some("tray")), CloseAction::Quit);
        assert_eq!(should_hide_on_close(true, false, Some("tray")), CloseAction::Quit);
        assert_eq!(should_hide_on_close(true, true, Some("quit")), CloseAction::Quit);
        assert_eq!(should_hide_on_close(true, true, Some("tray")), CloseAction::Hide);
        assert_eq!(should_hide_on_close(true, true, None), CloseAction::Ask);
        assert_eq!(should_hide_on_close(true, true, Some("other")), CloseAction::Ask);
    }

    #[test]
    fn without_a_tray_the_window_always_closes() {
        for remembered in [None, Some("tray"), Some("quit"), Some("other")] {
            assert_eq!(
                should_hide_on_close(false, true, remembered),
                CloseAction::Quit,
                "no host means no hide, ever"
            );
            assert_eq!(should_hide_on_close(false, false, remembered), CloseAction::Quit);
        }
    }

    #[test]
    fn autostart_exec_prefers_appimage() {
        let exe = Path::new("/tmp/.mount_Corky1/usr/bin/corkytux");
        assert_eq!(
            build_autostart_exec(Some("/home/me/Apps/CorkyTux.AppImage"), exe),
            "\"/home/me/Apps/CorkyTux.AppImage\" --background"
        );
        assert_eq!(
            build_autostart_exec(None, exe),
            "\"/tmp/.mount_Corky1/usr/bin/corkytux\" --background"
        );
        assert_eq!(
            build_autostart_exec(None, Path::new("/opt/corkytux (deleted)")),
            "\"/opt/corkytux\" --background"
        );
        assert_eq!(
            build_autostart_exec(Some("  "), exe),
            "\"/tmp/.mount_Corky1/usr/bin/corkytux\" --background"
        );
        assert_eq!(
            build_autostart_exec(Some("/home/me/My \"Apps\"/CorkyTux.AppImage"), exe),
            "\"/home/me/My \\\"Apps\\\"/CorkyTux.AppImage\" --background"
        );
    }

    #[test]
    fn autostart_path_uses_config_home() {
        let path = autostart_path().expect("HOME is set in tests");
        assert!(path.ends_with(".config/autostart/com.corkytux.CorkyTux.desktop"));
    }
}
