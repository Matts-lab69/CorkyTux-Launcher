use glib::subclass::prelude::*;
use gtk::prelude::*;
use std::cell::RefCell;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

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
            // Exact match first, then case-insensitive (C++ writes
            // lowercase keys: banner, mainPath, steamID, executor...).
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
        fs::write(self.games_ini_path(), data).ok();
    }

    pub fn save_launcher_ini(&self) {
        let data = self.imp().launcher_ini.borrow().to_string();
        fs::write(self.launcher_ini_path(), data).ok();
    }

    pub fn game_names(&self) -> Vec<String> {
        self.imp().games_ini.borrow().section_names()
    }

    /// Section map with lowercased keys (C++ writes lowercase keys:
    /// executable, mainPath→mainpath, steamID, executor...). Callers must
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

    /// C++ basePathFor parity: ~/.local/share/CorkyTux/<what>,
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

    /// C++ sharedPrefixes parity: comma-separated proton names in
    /// the "sharedPrefixes" launcher key.
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

    /// C++ sharedPrefixPath parity: prefixesDir/shared-<slug>, None when
    /// the proton has no shared prefix registered.
    pub fn shared_prefix_path(&self, proton_name: &str) -> Option<PathBuf> {
        if !self.shared_prefixes().contains(&proton_name.to_string()) {
            return None;
        }
        Some(
            self.prefixes_dir()
                .join(format!("shared-{}", Self::shared_slug(proton_name))),
        )
    }

    /// C++ addSharedPrefix parity: registers + creates dir, returns path.
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

    pub fn remove_shared_prefix(&self, proton_name: &str) {
        // C++ parity: the shared prefix directory goes away too.
        if let Some(path) = self.shared_prefix_path(proton_name) {
            std::fs::remove_dir_all(&path).ok();
        }
        let mut list = self.shared_prefixes();
        list.retain(|p| p != proton_name);
        self.set_launcher_value("sharedPrefixes", &list.join(","));
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
