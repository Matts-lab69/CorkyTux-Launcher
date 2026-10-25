use std::fs;
use std::path::PathBuf;
use std::process::Command;

use super::config::ConfigManager;

/// Sanitize a game name into a safe filename slug: lowercase, non
/// alphanumeric runs collapse to a single dash.
pub fn slugify(name: &str) -> String {
    let mut out = String::new();
    let mut dash = false;
    for c in name.chars() {
        if c.is_alphanumeric() {
            out.extend(c.to_lowercase());
            dash = false;
        } else if !dash {
            if !out.is_empty() {
                out.push('-');
            }
            dash = true;
        }
    }
    if out.ends_with('-') {
        out.pop();
    }
    if out.is_empty() {
        "game".to_string()
    } else {
        out
    }
}

fn app_menu_dir() -> PathBuf {
    if let Ok(xdg) = std::env::var("XDG_DATA_HOME") {
        if !xdg.is_empty() && PathBuf::from(&xdg).is_absolute() {
            return PathBuf::from(&xdg).join("applications");
        }
    }
    let home = std::env::var("HOME").unwrap_or_default();
    PathBuf::from(&home).join(".local/share/applications")
}

fn desktop_dir() -> PathBuf {
    if let Ok(out) = Command::new("xdg-user-dir").arg("DESKTOP").output() {
        if out.status.success() {
            let s = String::from_utf8_lossy(&out.stdout).trim().to_string();
            if !s.is_empty() {
                if s.starts_with('/') {
                    return PathBuf::from(&s);
                }
                if let Ok(home) = std::env::var("HOME") {
                    return PathBuf::from(&home).join(&s);
                }
            }
        }
    }
    let home = std::env::var("HOME").unwrap_or_default();
    PathBuf::from(&home).join("Desktop")
}

pub fn app_menu_file(name: &str) -> PathBuf {
    app_menu_dir().join(format!("corkytux-{}.desktop", slugify(name)))
}

pub fn desktop_file(name: &str) -> PathBuf {
    desktop_dir().join(format!("corkytux-{}.desktop", slugify(name)))
}

pub fn app_menu_active(name: &str) -> bool {
    app_menu_file(name).is_file()
}

pub fn desktop_active(name: &str) -> bool {
    desktop_file(name).is_file()
}

/// Escape a value for the `Exec` key: double quotes, backslashes, `$`,
/// backticks and `%` are reserved.
fn exec_quote(s: &str) -> String {
    let mut out = String::new();
    for c in s.chars() {
        match c {
            '"' | '`' | '$' | '%' | '\\' => {
                out.push('\\');
                out.push(c);
            }
            _ => out.push(c),
        }
    }
    format!("\"{}\"", out)
}

/// Escape a value for a single-line `.desktop` entry (backslash, newline, tab, CR).
fn desktop_escape(s: &str) -> String {
    let mut out = String::new();
    for c in s.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\t' => out.push_str("\\t"),
            '\r' => out.push_str("\\r"),
            _ => out.push(c),
        }
    }
    out
}

fn description_for(game_name: &str, cfg: &super::ConfigManager) -> String {
    if let Some(d) = cfg.game_value(game_name, "Description") {
        let d = d.trim().to_string();
        if !d.is_empty() {
            return d;
        }
    }
    let game = cfg.game_section(game_name);
    let exe = game.get("executable").cloned().unwrap_or_default();
    if exe.to_lowercase().ends_with(".appimage") {
        if let Ok(doc) = super::external::AppImageManager::scan(&exe) {
            if let Some(d) = doc.get("description").and_then(|v| v.as_str()) {
                let d = d.trim().to_string();
                if !d.is_empty() {
                    let _ = cfg.set_game_value(game_name, "Description", &d);
                    return d;
                }
            }
        }
    }
    game_name.to_string()
}

fn build_entry(name: &str) -> Result<String, String> {
    let cfg = ConfigManager::new();
    let game = cfg.game_section(name);
    if game.is_empty() {
        return Err(format!("Game \"{}\" not found", name));
    }
    let exe = std::env::current_exe()
        .map(|p| p.display().to_string())
        .map_err(|e| format!("Cannot resolve launcher path: {}", e))?;
    let icon = game.get("icon").cloned().unwrap_or_default();
    let banner = game.get("banner").cloned().unwrap_or_default();
    let icon_line = if icon.is_empty() {
        if banner.is_empty() {
            String::new()
        } else {
            format!("Icon={}\n", banner)
        }
    } else {
        format!("Icon={}\n", icon)
    };
    let comment = description_for(name, &cfg);
    let mut entry = String::new();
    entry.push_str("[Desktop Entry]\n");
    entry.push_str("Type=Application\n");
    entry.push_str(&format!("Name={}\n", desktop_escape(name)));
    entry.push_str(&format!("Comment={}\n", desktop_escape(&comment)));
    entry.push_str("Exec=");
    entry.push_str(&format!("{} --play {}", exec_quote(&exe), exec_quote(name)));
    entry.push_str("\nTerminal=false\n");
    entry.push_str("StartupNotify=false\n");
    entry.push_str(&icon_line);
    entry.push_str("Categories=Game;\n");
    entry.push_str(&format!("X-CorkyTux-Game={}\n", desktop_escape(name)));
    Ok(entry)
}

pub fn enable_app_menu(name: &str) -> Result<(), String> {
    let target = app_menu_file(name);
    let parent = target
        .parent()
        .ok_or_else(|| "Invalid app menu path".to_string())?;
    fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    fs::write(&target, build_entry(name)?).map_err(|e| e.to_string())?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&target, fs::Permissions::from_mode(0o644)).map_err(|e| e.to_string())?;
    }
    Ok(())
}

pub fn disable_app_menu(name: &str) -> Result<(), String> {
    let target = app_menu_file(name);
    if target.is_file() {
        fs::remove_file(&target).map_err(|e| e.to_string())?;
    }
    Ok(())
}

pub fn enable_desktop(name: &str) -> Result<(), String> {
    let target = desktop_file(name);
    let parent = target
        .parent()
        .ok_or_else(|| "Invalid desktop path".to_string())?;
    fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    fs::write(&target, build_entry(name)?).map_err(|e| e.to_string())?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&target, fs::Permissions::from_mode(0o644)).map_err(|e| e.to_string())?;
    }
    Ok(())
}

pub fn disable_desktop(name: &str) -> Result<(), String> {
    let target = desktop_file(name);
    if target.is_file() {
        fs::remove_file(&target).map_err(|e| e.to_string())?;
    }
    Ok(())
}

/// Drop every shortcut file this game may own (used on removal).
pub fn cleanup(name: &str) {
    let _ = disable_app_menu(name);
    let _ = disable_desktop(name);
}

/// Keep shortcuts alive across a rename: drop the old files and
/// recreate them for the new name when they were active.
pub fn rename(game: &str, new_name: &str) {
    let menu = app_menu_active(game);
    let desktop = desktop_active(game);
    cleanup(game);
    if menu {
        let _ = enable_app_menu(new_name);
    }
    if desktop {
        let _ = enable_desktop(new_name);
    }
}
#[cfg(test)]
mod tests {
    use super::app_menu_dir;
    use super::desktop_escape;
    use crate::test_env::lock;
    use std::path::PathBuf;

    #[test]
    fn app_menu_dir_honours_xdg_data_home() {
        let _g = lock();
        let prev_xdg = std::env::var("XDG_DATA_HOME").ok();
        let prev_home = std::env::var("HOME").ok();
        std::env::set_var("XDG_DATA_HOME", "/tmp/xdg-data-test");
        std::env::set_var("HOME", "/tmp/home-test");
        let got = app_menu_dir();
        // restore
        match prev_xdg { Some(v) => std::env::set_var("XDG_DATA_HOME", v), None => std::env::remove_var("XDG_DATA_HOME") }
        match prev_home { Some(v) => std::env::set_var("HOME", v), None => std::env::remove_var("HOME") }
        assert_eq!(got, PathBuf::from("/tmp/xdg-data-test/applications"));
    }

    #[test]
    fn desktop_escape_handles_backslash_and_newline() {
        assert_eq!(desktop_escape("a\\b"), "a\\\\b");
        assert_eq!(desktop_escape("line1\nline2"), "line1\\nline2");
    }

    #[test]
    fn desktop_escape_passthrough_plain() {
        assert_eq!(desktop_escape("Hello World"), "Hello World");
    }
}
