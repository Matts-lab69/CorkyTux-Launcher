use glib::subclass::prelude::*;
use gtk::prelude::*;
use std::cell::RefCell;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicBool, Ordering};

/// Set only when the first Games.ini write of this run already snapshotted
/// the on-disk file (see `save_games_ini`). Process-wide, NOT per instance:
/// several call sites build a fresh `ConfigManager::new()` per action, and
/// each instance would otherwise re-backup on its first write.
static GAMES_BACKED_UP: AtomicBool = AtomicBool::new(false);

/// Keep only the 5 newest `Games.ini.bak-YYYYmmdd-HHMMSS` snapshots next to
/// Games.ini; delete older ones. Only touches files matching the pattern.
fn prune_games_backups(games_ini_path: &Path) {
    let dir = match games_ini_path.parent() {
        Some(d) => d,
        None => return,
    };
    let Ok(rd) = fs::read_dir(dir) else { return };
    let mut snaps: Vec<PathBuf> = rd
        .flatten()
        .map(|e| e.path())
        .filter(|p| {
            p.file_name()
                .and_then(|n| n.to_str())
                .map(|n| n.starts_with("Games.ini.bak-"))
                .unwrap_or(false)
        })
        .collect();
    snaps.sort();
    for extra in snaps.iter().rev().skip(5) {
        let _ = fs::remove_file(extra);
    }
}

#[derive(Clone, Debug)]
pub struct IniSection {
    pub name: String,
    pub keys: Vec<(String, String)>,
}

#[derive(Clone, Debug, Default)]
pub struct IniFile {
    pub sections: Vec<IniSection>,
    pub global_keys: Vec<(String, String)>,
}

impl IniFile {
    pub fn parse(content: &str) -> Self {
        let mut result = IniFile::default();
        let mut current_section: Option<usize> = None;
        for line in content.lines() {
            let trimmed = line.trim();
            if trimmed.is_empty() || trimmed.starts_with(';') || trimmed.starts_with('#') {
                continue;
            }
            if trimmed.starts_with('[') && trimmed.ends_with(']') {
                let name = trimmed[1..trimmed.len() - 1].trim().to_string();
                result.sections.push(IniSection {
                    name: name.clone(),
                    keys: Vec::new(),
                });
                current_section = Some(result.sections.len() - 1);
                continue;
            }
            if let Some((key, value)) = trimmed.split_once('=') {
                let key = key.trim().to_string();
                let value = value.trim().to_string();
                match current_section {
                    Some(idx) => result.sections[idx].keys.push((key, value)),
                    None => result.global_keys.push((key, value)),
                }
            }
        }
        result
    }

    pub fn to_string(&self) -> String {
        let mut out = String::new();
        for (key, value) in &self.global_keys {
            out.push_str(&format!("{}={}\n", key, value));
        }
        for section in &self.sections {
            out.push_str(&format!("[{}]\n", section.name));
            for (key, value) in &section.keys {
                out.push_str(&format!("{}={}\n", key, value));
            }
            out.push('\n');
        }
        out
    }

    pub fn get_section(&self, name: &str) -> Option<&IniSection> {
        self.sections.iter().find(|s| s.name == name)
    }

    pub fn get_section_mut(&mut self, name: &str) -> &mut IniSection {
        if let Some(idx) = self.sections.iter().position(|s| s.name == name) {
            &mut self.sections[idx]
        } else {
            self.sections.push(IniSection {
                name: name.to_string(),
                keys: Vec::new(),
            });
            self.sections.last_mut().unwrap()
        }
    }

    pub fn get_value(&self, section: &str, key: &str) -> Option<String> {
        self.get_section(section).and_then(|s| {
            // Exact match first, then case-insensitive (I match the legacy
            // layout, which writes lowercase keys: banner, mainpath, steamid...).
            s.keys
                .iter()
                .find(|(k, _)| k == key)
                .or_else(|| {
                    s.keys
                        .iter()
                        .find(|(k, _)| k.eq_ignore_ascii_case(key))
                })
                .map(|(_, v)| v.clone())
        })
    }

    pub fn set_value(&mut self, section: &str, key: &str, value: &str) {
        let sec = self.get_section_mut(section);
        // Collapse case-variants so a write never leaves "banner=" and
        // "Banner=" duplicates behind.
        sec.keys.retain(|(k, _)| !(k != key && k.eq_ignore_ascii_case(key)));
        if let Some(entry) = sec.keys.iter_mut().find(|(k, _)| k == key) {
            entry.1 = value.to_string();
        } else {
            sec.keys.push((key.to_string(), value.to_string()));
        }
    }

    pub fn remove_value(&mut self, section: &str, key: &str) {
        if self.get_section(section).is_some() {
            let sec = self.get_section_mut(section);
            sec.keys.retain(|(k, _)| k != key);
        }
    }

    pub fn section_names(&self) -> Vec<String> {
        self.sections.iter().map(|s| s.name.clone()).collect()
    }

    pub fn has_section(&self, name: &str) -> bool {
        self.sections.iter().any(|s| s.name == name)
    }

    pub fn remove_section(&mut self, name: &str) {
        self.sections.retain(|s| s.name != name);
    }
}

mod imp {
    use super::*;
    use glib::subclass::prelude::*;
    use std::cell::RefCell;
    use std::path::PathBuf;

    #[derive(Default)]
    pub struct ConfigManager {
        pub config_dir: RefCell<PathBuf>,
        pub data_dir: RefCell<PathBuf>,
        pub games_ini: RefCell<IniFile>,
        pub launcher_ini: RefCell<IniFile>,
        pub last_games_content: RefCell<Option<String>>,
        pub last_launcher_content: RefCell<Option<String>>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for ConfigManager {
        const NAME: &'static str = "CorkyTuxConfigManager";
        type Type = super::ConfigManager;
        type ParentType = glib::Object;

        fn new() -> Self {
            Self::default()
        }
    }

    impl ObjectImpl for ConfigManager {
        fn constructed(&self) {
            self.parent_constructed();
            let obj = self.obj();
            obj.setup_directories();
            obj.load_games_ini();
            obj.load_launcher_ini();
        }
    }
}

glib::wrapper! {
    pub struct ConfigManager(ObjectSubclass<imp::ConfigManager>);
}

impl ConfigManager {
    /// Sharing bug fix: ConfigManager used to allocate an independent
    /// in-memory copy per `new()`. A write through one instance (e.g.
    /// artwork resolution) was invisible to the long-lived instance held
    /// by AppState, whose next `save_games_ini()` clobbered the change.
    /// Now every `new()` aliases a single per-thread instance.
    pub fn new() -> Self {
        thread_local! {
            static SHARED: RefCell<Option<ConfigManager>> = const { RefCell::new(None) };
        }
        SHARED.with(|slot| {
            let mut slot = slot.borrow_mut();
            if slot.is_none() {
                let fresh: ConfigManager = glib::Object::new();
                *slot = Some(fresh);
            }
            slot.as_ref().unwrap().clone()
        })
    }

    fn setup_directories(&self) {
        let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
        let config_dir = home.join(".config").join("CorkyTux");
        let data_dir = home.join(".local").join("share").join("CorkyTux");
        fs::create_dir_all(&config_dir).ok();
        fs::create_dir_all(&data_dir).ok();
        *self.imp().config_dir.borrow_mut() = config_dir;
        *self.imp().data_dir.borrow_mut() = data_dir;
    }

    pub fn expected_home() -> PathBuf {
        if let Ok(sudo_user) = std::env::var("SUDO_USER") {
            if sudo_user != "root" {
                return PathBuf::from(format!("/home/{}", sudo_user));
            }
        }
        dirs::home_dir().unwrap_or_else(|| PathBuf::from("."))
    }

    pub fn config_dir(&self) -> PathBuf {
        self.imp().config_dir.borrow().clone()
    }

    pub fn data_dir(&self) -> PathBuf {
        self.imp().data_dir.borrow().clone()
    }

    fn games_ini_path(&self) -> PathBuf {
        self.config_dir().join("Games.ini")
    }

    fn launcher_ini_path(&self) -> PathBuf {
        self.config_dir().join("Launcher.ini")
    }

    fn load_games_ini(&self) {
        let path = self.games_ini_path();
        let content = fs::read_to_string(&path).unwrap_or_default();
        *self.imp().games_ini.borrow_mut() = IniFile::parse(&content);
    }

    fn load_launcher_ini(&self) {
        let path = self.launcher_ini_path();
        let content = fs::read_to_string(&path).unwrap_or_default();
        *self.imp().launcher_ini.borrow_mut() = IniFile::parse(&content);
    }

    pub fn save_games_ini(&self) {
        let data = self.imp().games_ini.borrow().to_string();
        let mut last = self.imp().last_games_content.borrow_mut();
        if last.as_deref() == Some(data.as_str()) {
            return;
        }
        // Atomic write: temp file in the same dir, then rename, so a
        // crash mid-write never leaves a truncated Games.ini behind.
        let path = self.games_ini_path();
        // First real write of this run: snapshot the on-disk Games.ini to a
        // timestamped `Games.ini.bak-YYYYmmdd-HHMMSS` (keep the 5 newest).
        // GAMES_BACKED_UP flips to true ONLY when the copy succeeded; a
        // missing origin file is no error (the write below just creates it).
        if !GAMES_BACKED_UP.load(Ordering::SeqCst) {
            let ts = glib::DateTime::now_local()
                .and_then(|d| d.format("%Y%m%d-%H%M%S"))
                .map(|s| s.to_string())
                .unwrap_or_else(|_| {
                    std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .map(|d| d.as_secs().to_string())
                        .unwrap_or_default()
                });
            let stem = path.file_name().unwrap_or_default().to_string_lossy().to_string();
            let bak = path.with_file_name(format!("{stem}.bak-{ts}"));
            if path.exists() {
                match fs::copy(&path, &bak) {
                    Ok(_) => {
                        GAMES_BACKED_UP.store(true, Ordering::SeqCst);
                        prune_games_backups(&path);
                    }
                    Err(e) => eprintln!("[CorkyTux] Games.ini backup failed: {}", e),
                }
            }
        }
        let tmp = path.with_file_name(
            format!("{}.tmp", path.file_name().unwrap_or_default().to_string_lossy()));
        let committed = fs::write(&tmp, &data).is_ok() && fs::rename(&tmp, &path).is_ok();
        if !committed {
            let _ = fs::remove_file(&tmp);
        }
        *last = Some(data);
    }

    pub fn save_launcher_ini(&self) {
        let data = self.imp().launcher_ini.borrow().to_string();
        let mut last = self.imp().last_launcher_content.borrow_mut();
        if last.as_deref() == Some(data.as_str()) {
            return;
        }
        fs::write(self.launcher_ini_path(), &data).ok();
        *last = Some(data);
    }

    pub fn game_names(&self) -> Vec<String> {
        self.imp().games_ini.borrow().section_names()
    }

    /// Section map with lowercased keys (I match the legacy layout, which
    /// writes lowercase keys: executable, mainpath, steamid...). Callers must
    /// use lowercase key names.
    pub fn game_section(&self, name: &str) -> HashMap<String, String> {
        let ini = self.imp().games_ini.borrow();
        match ini.get_section(name) {
            Some(sec) => sec
                .keys
                .iter()
                .map(|(k, v)| (k.to_lowercase(), v.clone()))
                .collect(),
            None => HashMap::new(),
        }
    }

    pub fn game_value(&self, game: &str, key: &str) -> Option<String> {
        self.imp().games_ini.borrow().get_value(game, key)
    }

    pub fn set_game_value(&self, game: &str, key: &str, value: &str) {
        self.imp().games_ini.borrow_mut().set_value(game, key, value);
        self.save_games_ini();
    }

    pub fn remove_game(&self, game: &str) {
        self.imp().games_ini.borrow_mut().remove_section(game);
        self.save_games_ini();
    }

    pub fn has_game(&self, game: &str) -> bool {
        self.imp().games_ini.borrow().has_section(game)
    }

    pub fn launcher_value(&self, key: &str) -> Option<String> {
        self.launcher_value_in("User Settings", key)
    }

    pub fn set_launcher_value(&self, key: &str, value: &str) {
        self.set_launcher_value_in("User Settings", key, value);
    }

    pub fn launcher_value_in(&self, section: &str, key: &str) -> Option<String> {
        self.imp().launcher_ini.borrow().get_value(section, key)
    }

    pub fn set_launcher_value_in(&self, section: &str, key: &str, value: &str) {
        self.imp().launcher_ini.borrow_mut().set_value(section, key, value);
        self.save_launcher_ini();
    }

    pub fn remove_launcher_value_in(&self, section: &str, key: &str) {
        self.imp().launcher_ini.borrow_mut().remove_value(section, key);
        self.save_launcher_ini();
    }

    pub fn pick_folder(&self, title: &str) -> Option<PathBuf> {
        if Command::new("zenity").arg("--version").output().is_ok() {
            let output = Command::new("zenity")
                .args(["--file-selection", "--directory", "--title", title])
                .output()
                .ok()?;
            if output.status.success() {
                let path = String::from_utf8_lossy(&output.stdout).trim().to_string();
                if !path.is_empty() {
                    return Some(PathBuf::from(path));
                }
            }
        } else if Command::new("kdialog").arg("--version").output().is_ok() {
            let output = Command::new("kdialog")
                .args(["--getexistingdirectory", "--title", title])
                .output()
                .ok()?;
            if output.status.success() {
                let path = String::from_utf8_lossy(&output.stdout).trim().to_string();
                if !path.is_empty() {
                    return Some(PathBuf::from(path));
                }
            }
        }
        None
    }

    pub fn pick_file(&self, title: &str, extensions: &[&str]) -> Option<PathBuf> {
        let filter_strs: Vec<String> = extensions
            .iter()
            .map(|e| format!("*.{}", e))
            .collect();
        let filter = filter_strs.join(" ");
        if Command::new("zenity").arg("--version").output().is_ok() {
            let output = Command::new("zenity")
                .args(["--file-selection", "--title", title, "--file-filter", &filter])
                .output()
                .ok()?;
            if output.status.success() {
                let path = String::from_utf8_lossy(&output.stdout).trim().to_string();
                if !path.is_empty() {
                    return Some(PathBuf::from(path));
                }
            }
        } else if Command::new("kdialog").arg("--version").output().is_ok() {
            let filter_arg = format!("Executable files ({})", filter);
            let output = Command::new("kdialog")
                .args(["--getopenfilename", "--title", title, &filter_arg])
                .output()
                .ok()?;
            if output.status.success() {
                let path = String::from_utf8_lossy(&output.stdout).trim().to_string();
                if !path.is_empty() {
                    return Some(PathBuf::from(path));
                }
            }
        }
        None
    }

    /// I match the legacy basePathFor layout: ~/.local/share/CorkyTux/<what>,
    /// overridable via "<what>Path" launcher key. Creates the default dir.
    pub fn base_path_for(&self, for_what: &str) -> PathBuf {
        let key = format!("{}Path", for_what);
        if let Some(custom) = self.launcher_value(&key) {
            if !custom.trim().is_empty() {
                return PathBuf::from(custom.trim());
            }
        }
        let def = self.data_dir().join(for_what);
        fs::create_dir_all(&def).ok();
        def
    }

    pub fn prefixes_dir(&self) -> PathBuf {
        let d = self.data_dir().join("prefixes");
        fs::create_dir_all(&d).ok();
        d
    }

    /// Only the user-configured proton paths (base + Path 2/3).
    /// Steam compat dirs are NOT scanned: only what the user added.
    pub fn all_proton_paths(&self) -> Vec<PathBuf> {
        let mut paths = Vec::new();
        let main = self.base_path_for("protons");
        if !main.as_os_str().is_empty() {
            paths.push(main);
        }
        for key in ["protonsPath2", "protonsPath3"] {
            if let Some(extra) = self.launcher_value(key) {
                let trimmed = extra.trim().to_string();
                if !trimmed.is_empty() {
                    let p = PathBuf::from(trimmed);
                    if !paths.contains(&p) {
                        paths.push(p);
                    }
                }
            }
        }
        paths
    }

    /// I match the legacy sharedPrefixes layout: comma-separated proton names
    /// in the "sharedPrefixes" launcher key.
    pub fn shared_prefixes(&self) -> Vec<String> {
        match self.launcher_value("sharedPrefixes") {
            Some(val) => val
                .split(',')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect(),
            None => Vec::new(),
        }
    }

    fn shared_slug(proton_name: &str) -> String {
        let base = proton_name
            .rsplit('/')
            .next()
            .unwrap_or(proton_name);
        let slug: String = base
            .chars()
            .map(|c| {
                if c.is_ascii_alphanumeric() || c == '.' || c == '_' || c == '-' {
                    c
                } else {
                    '_'
                }
            })
            .collect();
        if slug.is_empty() {
            "default".to_string()
        } else {
            slug
        }
    }

    /// I match the legacy sharedPrefixPath layout: prefixesDir/shared-<slug>,
    /// None when the proton has no shared prefix registered.
    pub fn shared_prefix_path(&self, proton_name: &str) -> Option<PathBuf> {
        if !self.shared_prefixes().contains(&proton_name.to_string()) {
            return None;
        }
        Some(
            self.prefixes_dir()
                .join(format!("shared-{}", Self::shared_slug(proton_name))),
        )
    }

    /// I match the legacy addSharedPrefix behavior: register + create dir,
    /// return path.
    pub fn add_shared_prefix(&self, proton_name: &str) -> PathBuf {
        let mut list = self.shared_prefixes();
        if !list.contains(&proton_name.to_string()) {
            list.push(proton_name.to_string());
            self.set_launcher_value("sharedPrefixes", &list.join(","));
        }
        let path = self.prefixes_dir()
            .join(format!("shared-{}", Self::shared_slug(proton_name)));
        fs::create_dir_all(&path).ok();
        path
    }

    /// Games (name) whose prefix resolves into `base`: equal or inside it,
    /// canonical comparison so symlinks and `..` cannot hide. A prefix that
    /// cannot be resolved counts as a user (fail closed, like the remove
    /// guard): a stale record must be cleared first, never silently ignored.
    /// Pure; covered by tests.
    pub(crate) fn shared_prefix_users(
        base: &std::path::Path,
        prefixes: &[(String, String)],
    ) -> Vec<String> {
        let bc = std::fs::canonicalize(base)
            .unwrap_or_else(|_| base.to_path_buf());
        let mut out: Vec<String> = Vec::new();
        for (name, prefix) in prefixes {
            let p = prefix.trim_end_matches('/');
            if p.is_empty() {
                continue;
            }
            match std::fs::canonicalize(p) {
                Ok(pc) => {
                    if pc == bc || pc.starts_with(&bc) {
                        out.push(name.clone());
                    }
                }
                Err(_) => out.push(name.clone()),
            }
        }
        out.sort();
        out.dedup();
        out
    }

    /// Every registered game with its explicit PrefixPath. Read-only.
    pub(crate) fn all_game_prefixes(&self) -> Vec<(String, String)> {
        self.game_names()
            .into_iter()
            .filter_map(|n| {
                let p = self.game_value(&n, "PrefixPath").unwrap_or_default();
                if p.is_empty() {
                    None
                } else {
                    Some((n, p))
                }
            })
            .collect()
    }

    /// Deletes one directory, reporting the real error instead of swallowing
    /// it. Missing counts as already gone.
    pub(crate) fn delete_dir(path: &std::path::Path) -> Result<(), String> {
        if !path.exists() {
            return Ok(());
        }
        std::fs::remove_dir_all(path)
            .map_err(|e| format!("Could not delete {}: {}", path.display(), e))
    }

    pub fn remove_shared_prefix(&self, proton_name: &str) -> Result<(), String> {
        // I match the legacy behavior: the shared prefix directory goes away too.
        let path = self
            .shared_prefix_path(proton_name)
            .ok_or_else(|| format!("Unknown shared prefix: {}", proton_name))?;
        // Deny while any registered game points into it; the UI lists them.
        let users = Self::shared_prefix_users(&path, &self.all_game_prefixes());
        if !users.is_empty() {
            return Err(format!("Still used by: {}", users.join(", ")));
        }
        Self::delete_dir(&path)?;
        let mut list = self.shared_prefixes();
        list.retain(|p| p != proton_name);
        self.set_launcher_value("sharedPrefixes", &list.join(","));
        Ok(())
    }

    pub fn read_file(&self, path: &str) -> Option<String> {
        let expanded = shellexpand_tilde(path);
        fs::read_to_string(&expanded).ok()
    }

    pub fn save_text_file(&self, path: &str, text: &str) -> Result<(), String> {
        let expanded = shellexpand_tilde(path);
        if let Some(parent) = Path::new(&expanded).parent() {
            fs::create_dir_all(parent).ok();
        }
        fs::write(&expanded, text).map_err(|e| e.to_string())
    }

    pub fn ensure_dir(&self, path: &str) -> Result<(), String> {
        let expanded = shellexpand_tilde(path);
        fs::create_dir_all(&expanded).map_err(|e| e.to_string())
    }
}

fn shellexpand_tilde(s: &str) -> String {
    if s.starts_with("~/") || s == "~" {
        if let Ok(home) = std::env::var("HOME") {
            return s.replacen("~", &home, 1);
        }
    }
    s.to_string()
}

mod dirs {
    use std::path::PathBuf;

    pub fn home_dir() -> Option<PathBuf> {
        std::env::var("HOME")
            .ok()
            .map(PathBuf::from)
    }
}

#[cfg(test)]
mod tests {
    use super::ConfigManager;

    fn mktmp(name: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(name);
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn users_equal_inside_symlink_dotdot_and_missing() {
        let base = mktmp("corky_cfg_users");
        let shared = base.join("shared-proton-x");
        std::fs::create_dir_all(shared.join("pfx")).unwrap();
        let other = base.join("Other");
        std::fs::create_dir_all(&other).unwrap();
        let sh = shared.display().to_string();
        #[cfg(unix)]
        std::os::unix::fs::symlink(&shared, base.join("link")).unwrap();
        let pairs = vec![
            ("A".to_string(), sh.clone()),
            ("B".to_string(), shared.join("pfx").display().to_string()),
            ("C".to_string(), base.join("link").display().to_string()),
            ("D".to_string(), format!("{}/pfx/..", sh)),
            ("E".to_string(), base.join("gone").display().to_string()),
            ("F".to_string(), other.display().to_string()),
            ("G".to_string(), String::new()),
        ];
        let mut users = ConfigManager::shared_prefix_users(&shared, &pairs);
        users.sort();
        assert_eq!(users, vec!["A", "B", "C", "D", "E"]);
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn delete_dir_ok_missing_and_denied() {
        use std::os::unix::fs::PermissionsExt;
        let base = mktmp("corky_cfg_del");
        let dir = base.join("gone");
        std::fs::create_dir_all(&dir).unwrap();
        assert!(ConfigManager::delete_dir(&dir).is_ok());
        assert!(!dir.exists());
        assert!(ConfigManager::delete_dir(&dir).is_ok());
        let locked = base.join("locked");
        std::fs::create_dir_all(locked.join("sub")).unwrap();
        std::fs::set_permissions(&locked, std::fs::Permissions::from_mode(0o555)).unwrap();
        assert!(ConfigManager::delete_dir(&locked).is_err());
        std::fs::set_permissions(&locked, std::fs::Permissions::from_mode(0o755)).unwrap();
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn remove_unknown_prefix_errors_without_touching_disk() {
        let cfg = ConfigManager::new();
        let err = cfg.remove_shared_prefix("corky-no-such-prefix-xyz").unwrap_err();
        assert!(err.contains("Unknown shared prefix"), "{}", err);
    }
}
