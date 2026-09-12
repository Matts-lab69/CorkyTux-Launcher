use glib::subclass::prelude::*;
use gtk::prelude::*;
use std::cell::RefCell;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Clone, Debug, PartialEq)]
pub enum GameSource {
    Manual,
    Lutris,
    Steam,
    Imported,
    AppImage,
    Minecraft,
    Epic,
    Gog,
    RpgMaker,
}

impl Default for GameSource {
    fn default() -> Self {
        GameSource::Manual
    }
}

impl GameSource {
    pub fn as_str(&self) -> &str {
        match self {
            GameSource::Manual => "Manual",
            GameSource::Lutris => "Lutris",
            GameSource::Steam => "Steam",
            GameSource::Imported => "Imported",
            GameSource::AppImage => "AppImage",
            GameSource::Minecraft => "Minecraft",
            GameSource::Epic => "Epic",
            GameSource::Gog => "GOG",
            GameSource::RpgMaker => "RPGMaker",
        }
    }

    pub fn from_str(s: &str) -> Self {
        match s {
            "Lutris" => GameSource::Lutris,
            "Steam" => GameSource::Steam,
            "Imported" => GameSource::Imported,
            "AppImage" => GameSource::AppImage,
            "Minecraft" => GameSource::Minecraft,
            "Epic" => GameSource::Epic,
            "GOG" | "Gog" | "gog" => GameSource::Gog,
            "RPGMaker" | "RpgMaker" | "RPG Maker" => GameSource::RpgMaker,
            _ => GameSource::Manual,
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct GameEntry {
    pub name: String,
    pub executable: String,
    pub main_path: String,
    pub prefix_path: String,
    pub proton: String,
    pub overrides: String,
    pub steam_id: String,
    pub banner: String,
    pub icon: String,
    pub time_spent: u64,
    pub last_played: u64,
    pub favorite: bool,
    pub source: GameSource,
    pub executor: String,
    pub emu_settings: HashMap<String, String>,
    // === C++ parity fields ===
    pub lutris_runner: String,
    pub environment: String,
    pub args_before: String,
    pub args_after: String,
    pub steam_overlay: bool,
    pub steam_runtime: bool,
    pub use_umu: bool,
    pub use_shared_prefix: bool,
    pub shared_prefix_name: String,
    pub wined3d: bool,
    pub native_wayland: bool,
    pub game_mode: bool,
    pub mango_hud: bool,
    pub lutris_slug: String,
    pub fake_steam_id: String,
    pub install_size: String,
    pub heroic_store: String,
    pub heroic_app_id: String,
    pub heroic_eac: bool,
    pub heroic_battleye: bool,
}

impl GameEntry {
    pub fn from_config(name: &str, config: &super::ConfigManager) -> Self {
        let get = |key: &str| config.game_value(name, key).unwrap_or_default();
        let get_bool = |key: &str| -> bool {
            config
                .game_value(name, key)
                .map(|v| v.eq_ignore_ascii_case("true") || v == "1")
                .unwrap_or(false)
        };

        // Load emu_settings from all keys with "emu_" prefix (any case)
        let mut emu_settings = HashMap::new();
        let section = config.game_section(name);
        for (k, v) in &section {
            let lower = k.to_lowercase();
            if let Some(rest) = lower.strip_prefix("emu_") {
                emu_settings.insert(rest.to_string(), v.clone());
            }
        }

        GameEntry {
            name: name.to_string(),
            executable: get("Executable"),
            main_path: get("MainPath"),
            prefix_path: get("PrefixPath"),
            proton: get("Proton"),
            overrides: get("Overrides"),
            steam_id: get("SteamID"),
            banner: get("Banner"),
            icon: get("Icon"),
            time_spent: get("TimeSpent").parse().unwrap_or(0),
            last_played: get("LastPlayed").parse().unwrap_or(0),
            favorite: get("Favorite").eq_ignore_ascii_case("true"),
            source: GameSource::from_str(&get("Source")),
            executor: get("Executor"),
            emu_settings,
            lutris_runner: get("LutrisRunner"),
            environment: get("Environment"),
            args_before: get("ArgsBefore"),
            args_after: get("ArgsAfter"),
            steam_overlay: get_bool("SteamOverlay"),
            steam_runtime: get_bool("SteamRuntime"),
            use_umu: get_bool("UseUmu"),
            use_shared_prefix: get_bool("UseSharedPrefix"),
            shared_prefix_name: get("SharedPrefix"),
            wined3d: get_bool("UseWined3d"),
            native_wayland: get_bool("NativeWayland"),
            game_mode: get_bool("GameMode"),
            mango_hud: get_bool("MangoHud"),
            lutris_slug: get("LutrisSlug"),
            fake_steam_id: get("FakeSteamID"),
            install_size: get("InstallSize"),
            heroic_store: get("HeroicStore"),
            heroic_app_id: get("HeroicAppId"),
            heroic_eac: get_bool("HeroicEac"),
            heroic_battleye: get_bool("HeroicBattlEye"),
        }
    }

    pub fn to_section(&self) -> Vec<(String, String)> {
        let mut pairs = vec![
            ("Executable".into(), self.executable.clone()),
            ("MainPath".into(), self.main_path.clone()),
            ("PrefixPath".into(), self.prefix_path.clone()),
            ("Proton".into(), self.proton.clone()),
            ("Overrides".into(), self.overrides.clone()),
            ("SteamID".into(), self.steam_id.clone()),
            ("Banner".into(), self.banner.clone()),
            ("Icon".into(), self.icon.clone()),
            ("TimeSpent".into(), self.time_spent.to_string()),
            ("LastPlayed".into(), self.last_played.to_string()),
            ("Favorite".into(), self.favorite.to_string()),
            ("Source".into(), self.source.as_str().to_string()),
            ("Executor".into(), self.executor.clone()),
        ];
        // C++ parity fields
        if !self.lutris_runner.is_empty() { pairs.push(("LutrisRunner".into(), self.lutris_runner.clone())); }
        if !self.environment.is_empty() { pairs.push(("Environment".into(), self.environment.clone())); }
        if !self.args_before.is_empty() { pairs.push(("ArgsBefore".into(), self.args_before.clone())); }
        if !self.args_after.is_empty() { pairs.push(("ArgsAfter".into(), self.args_after.clone())); }
        if self.steam_overlay { pairs.push(("SteamOverlay".into(), "true".into())); }
        if self.steam_runtime { pairs.push(("SteamRuntime".into(), "true".into())); }
        if self.use_umu { pairs.push(("UseUmu".into(), "true".into())); }
        if self.use_shared_prefix { pairs.push(("UseSharedPrefix".into(), "true".into())); }
        if !self.shared_prefix_name.is_empty() { pairs.push(("SharedPrefix".into(), self.shared_prefix_name.clone())); }
        if self.wined3d { pairs.push(("UseWined3d".into(), "true".into())); }
        if self.native_wayland { pairs.push(("NativeWayland".into(), "true".into())); }
        if self.game_mode { pairs.push(("GameMode".into(), "true".into())); }
        if self.mango_hud { pairs.push(("MangoHud".into(), "true".into())); }
        if !self.lutris_slug.is_empty() { pairs.push(("LutrisSlug".into(), self.lutris_slug.clone())); }
        if !self.fake_steam_id.is_empty() { pairs.push(("FakeSteamID".into(), self.fake_steam_id.clone())); }
        if !self.install_size.is_empty() { pairs.push(("InstallSize".into(), self.install_size.clone())); }
        if !self.heroic_store.is_empty() { pairs.push(("HeroicStore".into(), self.heroic_store.clone())); }
        if !self.heroic_app_id.is_empty() { pairs.push(("HeroicAppId".into(), self.heroic_app_id.clone())); }
        if self.heroic_eac { pairs.push(("HeroicEac".into(), "true".into())); }
        if self.heroic_battleye { pairs.push(("HeroicBattlEye".into(), "true".into())); }
        for (k, v) in &self.emu_settings {
            pairs.push((format!("emu_{}", k), v.clone()));
        }
        pairs
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum FilterMode {
    All,
    Favorites,
    Az,
    MostPlayed,
    Recent,
}

mod imp {
    use super::*;
    use glib::subclass::prelude::*;
    use std::cell::RefCell;

    #[derive(Default)]
    pub struct GameModel {
        pub games: RefCell<Vec<GameEntry>>,
        pub search_text: RefCell<String>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for GameModel {
        const NAME: &'static str = "CorkyTuxGameModel";
        type Type = super::GameModel;
        type ParentType = glib::Object;

        fn new() -> Self {
            Self::default()
        }
    }

    impl ObjectImpl for GameModel {
        fn constructed(&self) {
            self.parent_constructed();
            self.obj().load_from_disk();
        }
    }
}

glib::wrapper! {
    pub struct GameModel(ObjectSubclass<imp::GameModel>);
}

impl GameModel {
    pub fn new() -> Self {
        glib::Object::new()
    }

    pub fn load_from_disk(&self) {
        let config = super::ConfigManager::new();
        let names = config.game_names();
        let mut games = Vec::new();
        for name in &names {
            games.push(GameEntry::from_config(name, &config));
        }
        *self.imp().games.borrow_mut() = games;
    }

    pub fn get_game(&self, name: &str) -> Option<GameEntry> {
        self.imp().games.borrow().iter().find(|g| g.name == name).cloned()
    }

    pub fn add_game(&self, entry: GameEntry) {
        let config = super::ConfigManager::new();
        for (key, value) in entry.to_section() {
            config.set_game_value(&entry.name, &key, &value);
        }
        let mut games = self.imp().games.borrow_mut();
        if let Some(existing) = games.iter_mut().find(|g| g.name == entry.name) {
            *existing = entry;
        } else {
            games.push(entry);
        }
    }

    pub fn import_external_game(&self, entry: GameEntry) {
        let mut imported = entry.clone();
        imported.source = GameSource::Imported;
        self.add_game(imported);
    }

    pub fn remove_game(&self, name: &str) {
        let config = super::ConfigManager::new();
        config.remove_game(name);
        self.imp().games.borrow_mut().retain(|g| g.name != name);
    }

    pub fn set_artwork(&self, name: &str, banner: &str, icon: &str) {
        let config = super::ConfigManager::new();
        if !icon.is_empty() {
            config.set_game_value(name, "Icon", icon);
        }
        if !banner.is_empty() {
            config.set_game_value(name, "Banner", banner);
        }
        let mut games = self.imp().games.borrow_mut();
        if let Some(g) = games.iter_mut().find(|g| g.name == name) {
            if !icon.is_empty() {
                g.icon = icon.to_string();
            }
            if !banner.is_empty() {
                g.banner = banner.to_string();
            }
        }
    }

    pub fn set_overrides(&self, name: &str, overrides: &str) {
        if overrides.is_empty() {
            return;
        }
        let config = super::ConfigManager::new();
        config.set_game_value(name, "Overrides", overrides);
        let mut games = self.imp().games.borrow_mut();
        if let Some(g) = games.iter_mut().find(|g| g.name == name) {
            g.overrides = overrides.to_string();
        }
    }

    pub fn rename_game(&self, old_name: &str, new_name: &str) {
        if old_name == new_name { return; }
        let config = super::ConfigManager::new();
        if let Some(mut entry) = self.get_game(old_name) {
            entry.name = new_name.to_string();
            self.remove_game(old_name);
            self.add_game(entry);
        }
    }

    pub fn set_favorite(&self, name: &str, fav: bool) {
        let config = super::ConfigManager::new();
        config.set_game_value(name, "Favorite", &fav.to_string());
        let mut games = self.imp().games.borrow_mut();
        if let Some(g) = games.iter_mut().find(|g| g.name == name) {
            g.favorite = fav;
        }
    }

    pub fn touch_last_played(&self, name: &str) {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        let config = super::ConfigManager::new();
        config.set_game_value(name, "LastPlayed", &now.to_string());
        let mut games = self.imp().games.borrow_mut();
        if let Some(g) = games.iter_mut().find(|g| g.name == name) {
            g.last_played = now;
        }
    }

    pub fn increment_time_spent(&self, name: &str, seconds: u64) {
        let config = super::ConfigManager::new();
        let current: u64 = config
            .game_value(name, "TimeSpent")
            .and_then(|v| v.parse().ok())
            .unwrap_or(0);
        let new_time = current + seconds;
        config.set_game_value(name, "TimeSpent", &new_time.to_string());
        let mut games = self.imp().games.borrow_mut();
        if let Some(g) = games.iter_mut().find(|g| g.name == name) {
            g.time_spent = new_time;
        }
    }

    pub fn set_search(&self, text: &str) {
        *self.imp().search_text.borrow_mut() = text.to_string();
    }

    pub fn filtered_games(&self, mode: FilterMode, search: &str) -> Vec<GameEntry> {
        let games = self.imp().games.borrow().clone();
        let search_lower = search.to_lowercase();
        let mut filtered: Vec<GameEntry> = games
            .into_iter()
            .filter(|g| {
                if search.is_empty() {
                    return true;
                }
                g.name.to_lowercase().contains(&search_lower)
                    || g.executable.to_lowercase().contains(&search_lower)
            })
            .collect();
        match mode {
            FilterMode::All => {}
            FilterMode::Favorites => {
                filtered.retain(|g| g.favorite);
            }
            FilterMode::Az => {
                filtered.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
            }
            FilterMode::MostPlayed => {
                filtered.sort_by(|a, b| b.time_spent.cmp(&a.time_spent));
            }
            FilterMode::Recent => {
                // C++ parity: Recently Added = orden de inserción invertido,
                // NO lastPlayed (eso alimenta el RecentModel de "Recently Played").
                filtered.reverse();
            }
        }
        filtered
    }

    pub fn ordered_names(&self) -> Vec<String> {
        let games = self.imp().games.borrow().clone();
        let mut names: Vec<String> = games.iter().map(|g| g.name.clone()).collect();
        names.sort_by(|a, b| a.to_lowercase().cmp(&b.to_lowercase()));
        names
    }

    pub fn count(&self) -> usize {
        self.imp().games.borrow().len()
    }
}

mod recent_imp {
    use super::*;
    use glib::subclass::prelude::*;
    use std::cell::RefCell;

    #[derive(Default)]
    pub struct RecentModel {
        pub entries: RefCell<Vec<GameEntry>>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for RecentModel {
        const NAME: &'static str = "CorkyTuxRecentModel";
        type Type = super::RecentModel;
        type ParentType = glib::Object;

        fn new() -> Self {
            Self::default()
        }
    }

    impl ObjectImpl for RecentModel {}
}

glib::wrapper! {
    pub struct RecentModel(ObjectSubclass<recent_imp::RecentModel>);
}

impl RecentModel {
    pub fn new() -> Self {
        glib::Object::new()
    }

    pub fn refresh(&self, top_n: usize) {
        let config = super::ConfigManager::new();
        let names = config.game_names();
        let mut entries: Vec<GameEntry> = names
            .iter()
            .map(|n| GameEntry::from_config(n, &config))
            .filter(|g| g.last_played > 0)
            .filter(|g| {
                g.source != GameSource::AppImage && g.executor != "appimage-launcher"
            })
            .collect();
        entries.sort_by(|a, b| b.last_played.cmp(&a.last_played));
        entries.truncate(top_n);
        *self.imp().entries.borrow_mut() = entries;
    }

    pub fn entries(&self) -> Vec<GameEntry> {
        self.imp().entries.borrow().clone()
    }

    pub fn count(&self) -> usize {
        self.imp().entries.borrow().len()
    }
}
