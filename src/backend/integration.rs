use glib::subclass::prelude::*;
use gtk::prelude::*;
use std::cell::RefCell;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

fn home_dir() -> Option<PathBuf> {
    std::env::var("HOME").ok().map(PathBuf::from)
}

#[derive(Clone, Debug, Default)]
pub struct IntegrationEntry {
    pub name: String,
    pub path: String,
    pub source: String,
    pub appid: String,
    pub prefix: String,
    pub runner: String,
    pub executable: String,
    pub slug: String,
    pub playtime_hours: f64,
    pub proton: String,
}

/// Lutris runner slug → launcher executor name (C++ runnerMap parity).
pub fn lutris_runner_to_executor(runner: &str) -> String {
    match runner.to_lowercase().as_str() {
        "mupen64plus" => "Mupen64Plus",
        "pcsx2" => "PCSX2",
        "ppsspp" => "PPSSPP",
        "rpcs3" => "RPCS3",
        "ryujinx" => "Ryujinx",
        "dolphin" => "Dolphin",
        "cemu" => "Cemu",
        "melonds" => "melonDS",
        "desmume" => "Desmume",
        "duckstation" => "DuckStation",
        "vita3k" => "Vita3K",
        "azahar" => "Azahar",
        _ => "",
    }
    .to_string()
}

mod imp {
    use super::*;
    use crate::backend::config::ConfigManager;
    use glib::subclass::prelude::*;
    use std::cell::RefCell;

    #[derive(Default)]
    pub struct IntegrationManager {
        pub enabled: RefCell<bool>,
        pub api_key: RefCell<String>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for IntegrationManager {
        const NAME: &'static str = "CorkyTuxIntegrationManager";
        type Type = super::IntegrationManager;
        type ParentType = glib::Object;

        fn new() -> Self {
            Self::default()
        }
    }

    impl ObjectImpl for IntegrationManager {
        fn constructed(&self) {
            self.parent_constructed();
            let config = ConfigManager::new();
            let enabled = config
                .launcher_value("IntegrationEnabled")
                .map(|v| v.eq_ignore_ascii_case("true"))
                .unwrap_or(false);
            *self.enabled.borrow_mut() = enabled;
            *self.api_key.borrow_mut() = config
                .launcher_value("IntegrationApiKey")
                .unwrap_or_default();
        }
    }
}

glib::wrapper! {
    pub struct IntegrationManager(ObjectSubclass<imp::IntegrationManager>);
}

impl IntegrationManager {
    pub fn new() -> Self {
        glib::Object::new()
    }

    pub fn is_enabled(&self) -> bool {
        *self.imp().enabled.borrow()
    }

    pub fn set_enabled(&self, enabled: bool) {
        *self.imp().enabled.borrow_mut() = enabled;
        let config = super::ConfigManager::new();
        config.set_launcher_value("IntegrationEnabled", &enabled.to_string());
    }

    pub fn api_key(&self) -> String {
        self.imp().api_key.borrow().clone()
    }

    pub fn set_api_key(&self, key: &str) {
        *self.imp().api_key.borrow_mut() = key.to_string();
        let config = super::ConfigManager::new();
        config.set_launcher_value("IntegrationApiKey", key);
    }

    pub fn path_exists(&self, path: &str) -> bool {
        Path::new(path).exists()
    }

    pub fn scan_dir_sync(&self, dir: &str) -> Vec<String> {
        let path = Path::new(dir);
        if !path.exists() || !path.is_dir() {
            return Vec::new();
        }
        let mut results = Vec::new();
        if let Ok(entries) = fs::read_dir(path) {
            for entry in entries.flatten() {
                let p = entry.path();
                if p.is_file() {
                    if let Some(name) = p.file_name() {
                        results.push(name.to_string_lossy().to_string());
                    }
                }
            }
        }
        results.sort();
        results
    }

    pub fn scan_dir_for_extensions(&self, dir: &str, extensions: &[String]) -> Vec<String> {
        let path = Path::new(dir);
        if !path.exists() || !path.is_dir() {
            return Vec::new();
        }
        let mut results = Vec::new();
        if let Ok(entries) = fs::read_dir(path) {
            for entry in entries.flatten() {
                let p = entry.path();
                if p.is_file() {
                    if let Some(ext) = p.extension() {
                        let ext_str = ext.to_string_lossy().to_lowercase();
                        if extensions.iter().any(|e| e.to_lowercase() == ext_str) {
                            results.push(p.display().to_string());
                        }
                    }
                }
            }
        }
        results.sort();
        results
    }

    pub fn find_main_exe(&self, dir: &str) -> Option<String> {
        let path = Path::new(dir);
        if !path.exists() {
            return None;
        }
        let mut candidates = Vec::new();
        if let Ok(entries) = fs::read_dir(path) {
            for entry in entries.flatten() {
                let p = entry.path();
                if p.is_file() {
                    let name = p.file_name().unwrap_or_default().to_string_lossy().to_lowercase();
                    if name.ends_with(".exe") || name.ends_with(".sh") || name.ends_with(".bin") {
                        candidates.push(p);
                    }
                }
            }
        }
        if candidates.is_empty() {
            return None;
        }
        candidates.sort_by(|a, b| {
            let a_name = a.file_name().unwrap_or_default().to_string_lossy().to_lowercase();
            let b_name = b.file_name().unwrap_or_default().to_string_lossy().to_lowercase();
            a_name.len().cmp(&b_name.len())
        });
        Some(candidates[0].display().to_string())
    }

    pub fn remove_dir_recursive(&self, dir: &str) -> Result<(), String> {
        let path = Path::new(dir);
        if !path.exists() {
            return Ok(());
        }
        fs::remove_dir_all(path).map_err(|e| e.to_string())
    }

    pub fn open_folder(&self, path: &str) {
        let expanded = shellexpand::tilde(path);
        let p = Path::new(&expanded);
        if p.exists() {
            let _ = Command::new("xdg-open").arg(p).spawn();
        }
    }

    pub fn open_url(&self, url: &str) {
        let _ = Command::new("xdg-open").arg(url).spawn();
    }

    const SGDB_KEY: &'static str = "0ab12f62e2d5e6b3717161be0c5e68fa";

    /// Download helper: >1000 bytes required, Referer for steamgriddb CDN.
    fn download_art(
        client: &reqwest::blocking::Client,
        url: &str,
        dest: &PathBuf,
    ) -> bool {
        let mut req = client.get(url);
        if url.contains("steamgriddb") {
            req = req.header("Referer", "https://www.steamgriddb.com/");
        }
        let body = match req.send().and_then(|r| r.error_for_status()) {
            Ok(r) => match r.bytes() {
                Ok(b) => b,
                Err(_) => return false,
            },
            Err(_) => return false,
        };
        if body.len() <= 1000 {
            return false;
        }
        if let Some(parent) = dest.parent() {
            fs::create_dir_all(parent).ok();
        }
        fs::write(dest, &body).is_ok()
    }

    fn sgdb_get(
        client: &reqwest::blocking::Client,
        url: &str,
    ) -> Result<String, String> {
        client
            .get(url)
            .bearer_auth(Self::SGDB_KEY)
            .header("User-Agent", "CorkyTux/3.0")
            .send()
            .map_err(|e| e.to_string())?
            .error_for_status()
            .map_err(|e| e.to_string())?
            .text()
            .map_err(|e| e.to_string())
    }

    /// Full artwork resolver (C++ resolveArtwork parity):
    /// 1) Steam CDN by AppID (+ Store API fallback),
    /// 2) local Lutris art by slug, 3) Lutris.net API, 4) SteamGridDB.
    /// Returns (icon, banner). Files land in ~/.config/CorkyTux/{icons,banners}/.
    pub fn resolve_artwork(&self, game_name: &str, steam_id: &str) -> (Option<String>, Option<String>) {
        let home = home_dir().unwrap_or_default();
        let banners_dir = home.join(".config").join("CorkyTux").join("banners");
        let icons_dir = home.join(".config").join("CorkyTux").join("icons");
        fs::create_dir_all(&banners_dir).ok();
        fs::create_dir_all(&icons_dir).ok();
        let client = match reqwest::blocking::Client::builder()
            .user_agent("CorkyTux/3.0")
            .timeout(std::time::Duration::from_secs(30))
            .build()
        {
            Ok(c) => c,
            Err(_) => return (None, None),
        };
        let mut banner: Option<String> = None;
        let mut icon: Option<String> = None;

        // 1) Steam CDN by AppID. Manual games carry no AppID, so look it
        // up by exact name first (e.g. "Machine Party" -> 4108000).
        let id = if !steam_id.trim().is_empty() {
            steam_id.trim().to_string()
        } else {
            steam_appid_by_name(&client, game_name).unwrap_or_default()
        };
        if !id.is_empty() {
            let b = banners_dir.join(format!("{}.jpg", id));
            let i = icons_dir.join(format!("{}.jpg", id));
            if Self::download_art(&client,
                &format!("https://cdn.cloudflare.steamstatic.com/steam/apps/{}/header.jpg", id),
                &b)
            {
                banner = Some(b.display().to_string());
            }
            // Square SGDB icon first: logo.png is a wide logo that reads
            // as a cover when shown as an icon.
            if !game_name.trim().is_empty() {
                let search_url = format!(
                    "https://www.steamgriddb.com/api/v2/search/autocomplete/{}",
                    url_encode_query(game_name.trim())
                );
                if let Ok(body) = Self::sgdb_get(&client, &search_url) {
                    let grid_id = find_grid_id(&body, game_name);
                    if !grid_id.is_empty() {
                        if let Some(found) =
                            Self::sgdb_icon_for(&client, &grid_id, game_name, &icons_dir)
                        {
                            icon = Some(found);
                        }
                    }
                }
            }
            if icon.is_none()
                && Self::download_art(&client,
                    &format!("https://cdn.cloudflare.steamstatic.com/steam/apps/{}/logo.png", id),
                    &i)
            {
                icon = Some(i.display().to_string());
            }
            // 1b) Store API fallback for the banner
            if banner.is_none() {
                if let Ok(resp) = client
                    .get(&format!("https://store.steampowered.com/api/appdetails?appids={}", id))
                    .send()
                {
                    if let Ok(body) = resp.text() {
                        if let Some(u) = json_field(&body, "header_image") {
                            let d = banners_dir.join(format!("{}-store.jpg", id));
                            if Self::download_art(&client, &u.replace("\\/", "/"), &d) {
                                banner = Some(d.display().to_string());
                            }
                        }
                    }
                }
            }
        }

        // Lutris slug for local/API tiers
        let stored_slug = super::ConfigManager::new()
            .game_value(game_name, "LutrisSlug")
            .unwrap_or_default();
        let slug = if stored_slug.is_empty() {
            slugify(game_name)
        } else {
            stored_slug
        };

        // 2) Lutris local art by slug
        if (banner.is_none() || icon.is_none()) && !game_name.is_empty() {
            let xdg = std::env::var("XDG_DATA_HOME").unwrap_or_else(|_| {
                home.join(".local").join("share").display().to_string()
            });
            let mut lutris_dirs = Vec::new();
            let native = PathBuf::from(&xdg).join("lutris");
            if native.exists() {
                lutris_dirs.push(native);
            }
            let flatpak = home
                .join(".var/app/net.lutris.Lutris/data/lutris");
            if flatpak.exists() {
                lutris_dirs.push(flatpak);
            }
            for dir in &lutris_dirs {
                if banner.is_none() {
                    for sub in ["coverart", "banners"] {
                        let src = dir.join(sub).join(format!("{}.jpg", slug));
                        if src.is_file() {
                            let dest = banners_dir.join(format!("{}.jpg", slug));
                            if copy_if_missing(&src, &dest) {
                                banner = Some(dest.display().to_string());
                                break;
                            }
                        }
                    }
                }
                if icon.is_none() {
                    let h = PathBuf::from(&xdg)
                        .join("icons/hicolor/128x128/apps")
                        .join(format!("lutris_{}.png", slug));
                    if h.is_file() {
                        let dest = icons_dir.join(format!("{}.png", slug));
                        if copy_if_missing(&h, &dest) {
                            icon = Some(dest.display().to_string());
                        }
                    }
                }
                if banner.is_some() && icon.is_some() {
                    break;
                }
            }
        }

        // 3) Lutris.net public API (free, no key), strict name match
        if banner.is_none() && !game_name.is_empty() {
            let url = format!(
                "https://lutris.net/api/games?search={}",
                url_encode_query(game_name.trim())
            );
            if let Ok(resp) = client.get(&url).send() {
                if let Ok(body) = resp.text() {
                    let want = norm_name(game_name);
                    // Split result objects, accept only normalized-equal names
                    // ("R.E.P.O." matches "repo", never "Repository").
                    for chunk in body.split("{\"id\"") {
                        let nm = match json_field(chunk, "name") {
                            Some(n) => n,
                            None => continue,
                        };
                        if norm_name(&nm) != want {
                            continue;
                        }
                        // coverart first, then banner_url (one of them is
                        // often null/404 while the other works).
                        let mut candidates: Vec<String> = Vec::new();
                        for field in ["coverart", "banner_url"] {
                            if let Some(u) = json_field(chunk, field) {
                                if !u.is_empty() && u != "null" {
                                    candidates.push(u);
                                }
                            }
                        }
                        let d = banners_dir.join(format!("{}-lutris.jpg", slugify(game_name)));
                        for u in candidates {
                            if Self::download_art(&client, &u, &d) {
                                banner = Some(d.display().to_string());
                                break;
                            }
                        }
                        if banner.is_some() {
                            break;
                        }
                    }
                }
            }
        }

        // 4) SteamGridDB (bundled key): grids for banner, icons endpoint
        if (banner.is_none() || icon.is_none()) && !game_name.is_empty() {
            let search_url = format!(
                "https://www.steamgriddb.com/api/v2/search/autocomplete/{}",
                url_encode_query(game_name.trim())
            );
            if let Ok(body) = Self::sgdb_get(&client, &search_url) {
                let grid_id = find_grid_id(&body, game_name);
                if !grid_id.is_empty() {
                    if banner.is_none() {
                        let url = format!(
                            "https://www.steamgriddb.com/api/v2/grids/game/{}?dimensions=600x900&types=static",
                            grid_id
                        );
                        if let Ok(gb) = Self::sgdb_get(&client, &url) {
                            if let Some(u) = json_field(&gb, "url") {
                                let d = banners_dir.join(format!("{}-sgdb.jpg", slugify(game_name)));
                                if Self::download_art(&client, &u.replace("\\/", "/"), &d) {
                                    banner = Some(d.display().to_string());
                                }
                            }
                        }
                    }
                    if icon.is_none() {
                        if let Some(found) =
                            Self::sgdb_icon_for(&client, &grid_id, game_name, &icons_dir)
                        {
                            icon = Some(found);
                        }
                    }
                }
            }
        }

        (icon, banner)
    }

    /// SteamGridDB icons endpoint → square icon file. Used for Steam games
    /// too: the CDN logo.png is a wide logo (looks like a cover as an
    /// icon), while SGDB serves real square icons.
    fn sgdb_icon_for(
        client: &reqwest::blocking::Client,
        grid_id: &str,
        game_name: &str,
        icons_dir: &std::path::PathBuf,
    ) -> Option<String> {
        let url = format!(
            "https://www.steamgriddb.com/api/v2/icons/game/{}?limit=20",
            grid_id
        );
        let ib = Self::sgdb_get(client, &url).ok()?;
        // Prefer .png URLs
        let mut urls: Vec<String> = Vec::new();
        let mut r = ib.as_str();
        while let Some(p) = r.find("\"url\"") {
            let seg = &r[p..];
            if let Some(u) = json_field(seg, "url") {
                urls.push(u.replace("\\/", "/"));
            }
            r = &seg[5.min(seg.len())..];
            if r.len() < 10 {
                break;
            }
        }
        urls.sort_by(|a, b| {
            b.ends_with(".png").cmp(&a.ends_with(".png"))
        });
        for u in urls {
            let d = icons_dir.join(format!("{}-sgdb.png", slugify(game_name)));
            if Self::download_art(client, &u, &d) {
                return Some(d.display().to_string());
            }
        }
        None
    }

    /// Extract the embedded icon from a Windows .exe via icoextract+ffmpeg
    /// (C++ extractExeIconAsync parity). Returns the stored PNG path.
    pub fn extract_exe_icon(&self, exe_path: &str, game_name: &str) -> Option<String> {
        if !exe_path.to_lowercase().ends_with(".exe") {
            return None;
        }
        let expanded = shellexpand_tilde(exe_path);
        if !Path::new(&expanded).is_file() {
            return None;
        }
        for tool in ["icoextract", "ffmpeg"] {
            if Command::new("which").arg(tool).output()
                .map(|o| !o.status.success()).unwrap_or(true)
            {
                return None;
            }
        }
        let tmp = std::env::temp_dir().join(format!(
            "corkytux-icon-{}-{}",
            std::process::id(),
            game_name.len()
        ));
        fs::create_dir_all(&tmp).ok();
        let ico = tmp.join("icon.ico");
        let st = Command::new("timeout")
            .args(["60", "icoextract", &expanded, ico.to_str().unwrap_or("")])
            .status();
        if !matches!(st, Ok(s) if s.success() && ico.is_file()) {
            fs::remove_dir_all(&tmp).ok();
            return None;
        }
        let png = tmp.join("icon.png");
        let st = Command::new("timeout")
            .args(["60", "ffmpeg", "-y", "-i", ico.to_str().unwrap_or(""), png.to_str().unwrap_or("")])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();
        if !matches!(st, Ok(s) if s.success() && png.is_file()) {
            fs::remove_dir_all(&tmp).ok();
            return None;
        }
        // Validate: loadable and >= 16px (gdk-pixbuf is already a dep)
        if let Ok(pb) = gdk_pixbuf::Pixbuf::from_file(&png) {
            if pb.width() < 16 || pb.height() < 16 {
                fs::remove_dir_all(&tmp).ok();
                return None;
            }
        } else {
            fs::remove_dir_all(&tmp).ok();
            return None;
        }
        let home = home_dir().unwrap_or_default();
        let dest = home.join(".config").join("CorkyTux").join("icons")
            .join(format!("{}-exe.png", slugify(game_name)));
        if let Some(parent) = dest.parent() {
            fs::create_dir_all(parent).ok();
        }
        fs::remove_file(&dest).ok();
        let ok = fs::copy(&png, &dest).is_ok() && dest.is_file();
        fs::remove_dir_all(&tmp).ok();
        if ok {
            Some(dest.display().to_string())
        } else {
            None
        }
    }

    pub fn extract_appimage_icon(&self, appimage_path: &str, game_name: &str) -> Option<String> {
        let is_app = Path::new(&shellexpand_tilde(appimage_path))
            .extension().and_then(|e| e.to_str())
            .map(|e| e.eq_ignore_ascii_case("appimage"))
            .unwrap_or(false);
        if !is_app {
            return None;
        }
        let expanded = shellexpand_tilde(appimage_path);
        if !Path::new(&expanded).is_file() {
            return None;
        }
        let home = home_dir().unwrap_or_default();
        let dest = home.join(".config").join("CorkyTux").join("icons")
            .join(format!("{}-appimage.png", slugify(game_name)));
        if let Some(parent) = dest.parent() {
            fs::create_dir_all(parent).ok();
        }
        let doc = super::external::AppImageManager::extract_icon(
            &expanded,
            &dest.display().to_string(),
        ).ok()?;
        let got = doc.get("icon").and_then(|v| v.as_str()).unwrap_or_default();
        if got.is_empty() || !Path::new(got).is_file() {
            return None;
        }
        if let Ok(pb) = gdk_pixbuf::Pixbuf::from_file(got) {
            if pb.width() < 16 || pb.height() < 16 {
                return None;
            }
        } else {
            return None;
        }
        Some(got.to_string())
    }

    pub fn extract_rpg_icon(&self, game_dir: &str, game_name: &str) -> Option<String> {
        let dir = shellexpand_tilde(game_dir);
        let base = Path::new(&dir);
        if !base.is_dir() {
            return None;
        }
        let mut candidates: Vec<std::path::PathBuf> = Vec::new();
        let pkg = base.join("package.json");
        if pkg.is_file() {
            if let Ok(content) = fs::read_to_string(&pkg) {
                if let Ok(v) = serde_json::from_str::<serde_json::Value>(&content) {
                    if let Some(rel) = v.get("window").and_then(|w| w.get("icon")).and_then(|i| i.as_str()) {
                        candidates.push(base.join(rel));
                    }
                }
            }
        }
        candidates.push(base.join("icon").join("icon.png"));
        candidates.push(base.join("www").join("icon").join("icon.png"));
        let src = candidates.into_iter().find(|p| p.is_file())?;
        if let Ok(pb) = gdk_pixbuf::Pixbuf::from_file(&src) {
            if pb.width() < 16 || pb.height() < 16 {
                return None;
            }
        } else {
            return None;
        }
        let home = home_dir().unwrap_or_default();
        let dest = home.join(".config").join("CorkyTux").join("icons")
            .join(format!("{}-rpg.png", slugify(game_name)));
        if let Some(parent) = dest.parent() {
            fs::create_dir_all(parent).ok();
        }
        fs::remove_file(&dest).ok();
        if fs::copy(&src, &dest).is_ok() && dest.is_file() {
            Some(dest.display().to_string())
        } else {
            None
        }
    }

    pub fn extract_rar(&self, rar_path: &str, dest_dir: &str) -> Result<(), String> {
        let expanded = shellexpand_tilde(rar_path);
        let dest = shellexpand_tilde(dest_dir);
        fs::create_dir_all(&dest).ok();
        let status = Command::new("unrar")
            .args(["x", "-o+", &expanded, &dest])
            .status()
            .map_err(|e| format!("unrar not found or failed: {}", e))?;
        if status.success() {
            Ok(())
        } else {
            Err("RAR extraction failed".into())
        }
    }

    pub fn query_folder_size(&self, path: &str) -> String {
        let expanded = shellexpand_tilde(path);
        let p = Path::new(&expanded);
        if !p.exists() {
            return "--".to_string();
        }
        let size = self.query_folder_size_recursive(p);
        format_size_bytes(size)
    }

    fn query_folder_size_recursive(&self, path: &Path) -> u64 {
        let mut total = 0;
        if let Ok(entries) = fs::read_dir(path) {
            for entry in entries.flatten() {
                let p = entry.path();
                if p.is_file() {
                    total += fs::metadata(&p).map(|m| m.len()).unwrap_or(0);
                } else if p.is_dir() {
                    total += self.query_folder_size_recursive(&p);
                }
            }
        }
        total
    }

    pub fn scan_lutris(&self) -> Vec<IntegrationEntry> {
        // 1) Preferred: Lutris SQLite database pga.db (no new deps: sqlite3 CLI)
        let db = home_dir()
            .unwrap_or_default()
            .join(".local")
            .join("share")
            .join("lutris")
            .join("pga.db");
        if db.exists() {
            let found = self.scan_lutris_db(&db);
            if !found.is_empty() {
                return found;
            }
        }
        // 2) Fallback: `lutris -l` CLI table
        let via_cli = self.scan_lutris_cli();
        if !via_cli.is_empty() {
            return via_cli;
        }
        // 3) Last resort: per-game yml directories
        let mut results = Vec::new();
        let lutris_dir = home_dir()
            .unwrap_or_default()
            .join(".local")
            .join("share")
            .join("lutris")
            .join("games");
        if !lutris_dir.exists() {
            return results;
        }
        // Flat <config>.yml files (e.g. blasphemous-1787265174.yml), NOT
        // per-slug dirs: reuse the same yml parser as the DB tier.
        if let Ok(entries) = fs::read_dir(&lutris_dir) {
            for entry in entries.flatten() {
                let p = entry.path();
                if p.is_file()
                    && p.extension().and_then(|x| x.to_str()) == Some("yml")
                {
                    let stem = p.file_stem().and_then(|s| s.to_str()).unwrap_or_default();
                    // Config files are "<slug>-<timestamp>"; plain "<slug>.yml"
                    // files are runner configs, not games.
                    if !stem.contains('-') {
                        continue;
                    }
                    let slug = stem.rsplit_once('-').map(|(s, _)| s).unwrap_or(stem);
                    let content = fs::read_to_string(&p).unwrap_or_default();
                    // Runner-only yml (no game: exe/main_file) is not a game.
                    if !content.contains("exe:") && !content.contains("main_file:") {
                        continue;
                    }
                    let (bin, prefix, version) = self.lutris_yml_info(stem, slug);
                    if bin.is_empty() {
                        continue;
                    }
                    let name = slug
                        .split('-')
                        .map(|w| {
                            let mut c = w.chars();
                            match c.next() {
                                Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
                                None => String::new(),
                            }
                        })
                        .collect::<Vec<_>>()
                        .join(" ");
                    let mut path_str = String::new();
                    if let Some(parent) = std::path::Path::new(&bin).parent() {
                        path_str = parent.display().to_string();
                    }
                    // Runner from the yml's top-level "<runner>:" section.
                    let mut runner = String::new();
                    for line in content.lines() {
                        let t = line.trim();
                        if !line.starts_with(char::is_whitespace)
                            && t.ends_with(':')
                            && t != "game:"
                        {
                            runner = t.trim_end_matches(':').to_string();
                            break;
                        }
                    }
                    results.push(IntegrationEntry {
                        name,
                        path: path_str,
                        source: "lutris".into(),
                        appid: String::new(),
                        prefix,
                        runner,
                        executable: bin,
                        slug: slug.to_string(),
                        playtime_hours: 0.0,
                        proton: version,
                    });
                }
            }
        }
        results.sort_by(|a, b| a.name.cmp(&b.name));
        results
    }

    /// Parse a Lutris game yml (`~/.local/share/lutris/games/<config>.yml`):
    /// returns (exe_or_main_file, prefix, wine_version).
    fn lutris_yml_info(&self, configpath: &str, slug: &str) -> (String, String, String) {
        let home = home_dir().unwrap_or_default();
        let games_dir = home.join(".local").join("share").join("lutris").join("games");
        // configpath IS the file stem (<config>.yml); fall back to <slug>.yml
        // or the newest <slug>-*.yml.
        let mut candidates = Vec::new();
        if !configpath.trim().is_empty() {
            candidates.push(games_dir.join(format!("{}.yml", configpath.trim())));
        }
        if !slug.trim().is_empty() {
            candidates.push(games_dir.join(format!("{}.yml", slug.trim())));
            if let Ok(entries) = fs::read_dir(&games_dir) {
                let mut prefixed: Vec<_> = entries
                    .flatten()
                    .map(|e| e.path())
                    .filter(|p| {
                        p.extension().and_then(|x| x.to_str()) == Some("yml")
                            && p.file_stem()
                                .and_then(|s| s.to_str())
                                .map(|s| s.starts_with(&format!("{}-", slug.trim())))
                                .unwrap_or(false)
                    })
                    .collect();
                prefixed.sort();
                candidates.extend(prefixed);
            }
        }
        for yml in candidates {
            if let Ok(content) = fs::read_to_string(&yml) {
                let mut exe = String::new();
                let mut main_file = String::new();
                let mut prefix = String::new();
                let mut version = String::new();
                for line in content.lines() {
                    let t = line.trim();
                    if let Some(v) = t.strip_prefix("exe:") {
                        exe = v.trim().to_string();
                    } else if let Some(v) = t.strip_prefix("main_file:") {
                        main_file = v.trim().to_string();
                    } else if let Some(v) = t.strip_prefix("prefix:") {
                        prefix = v.trim().to_string();
                    } else if let Some(v) = t.strip_prefix("version:") {
                        version = v.trim().to_string();
                    }
                }
                if !exe.is_empty() || !main_file.is_empty() {
                    let bin = if !exe.is_empty() { exe } else { main_file };
                    return (bin, prefix, version);
                }
            }
        }
        (String::new(), String::new(), String::new())
    }

    fn scan_lutris_db(&self, db: &std::path::Path) -> Vec<IntegrationEntry> {
        let mut results = Vec::new();
        let output = Command::new("sqlite3")
            .args([
                "-separator",
                "\x1f",
                db.to_str().unwrap_or(""),
                "SELECT name, slug, runner, directory, executable, configpath, service, service_id, playtime FROM games WHERE installed = 1;",
            ])
            .output();
        let output = match output {
            Ok(o) if o.status.success() => o,
            _ => return results,
        };
        for line in String::from_utf8_lossy(&output.stdout).lines() {
            let cols: Vec<&str> = line.split('\x1f').collect();
            if cols.is_empty() || cols[0].trim().is_empty() {
                continue;
            }
            let name = cols[0].trim().to_string();
            let slug = cols.get(1).map(|s| s.trim().to_string()).unwrap_or_default();
            let runner = cols.get(2).map(|s| s.trim().to_string()).unwrap_or_default();
            let mut directory = cols.get(3).map(|s| s.trim().to_string()).unwrap_or_default();
            let db_exe = cols.get(4).map(|s| s.trim().to_string()).unwrap_or_default();
            let configpath = cols.get(5).map(|s| s.trim().to_string()).unwrap_or_default();
            let service = cols.get(6).map(|s| s.trim().to_string()).unwrap_or_default();
            let service_id = cols.get(7).map(|s| s.trim().to_string()).unwrap_or_default();
            // pga.playtime is already in hours (2.28 ~= 2h17m played).
            let playtime: f64 = cols.get(8).and_then(|s| s.trim().parse().ok()).unwrap_or(0.0);
            // Steam-service games carry their AppID (Steam art + launch).
            let appid = if service == "steam" { service_id } else { String::new() };
            // Real binary/prefix/proton live in the game yml.
            let (yml_exe, yml_prefix, yml_version) = self.lutris_yml_info(&configpath, &slug);
            let executable = if !yml_exe.is_empty() {
                yml_exe
            } else {
                db_exe
            };
            // Prefer whichever path really exists on disk: Lutris rows
            // often carry a stale directory (renamed folders, e.g.
            // "New Super MArio Bros" vs the real "New Super Mario Bros").
            let exe_parent = std::path::Path::new(&executable)
                .parent()
                .map(|p| p.display().to_string())
                .unwrap_or_default();
            let dir_ok = !directory.is_empty()
                && std::path::Path::new(&directory).exists();
            let parent_ok =
                !exe_parent.is_empty() && std::path::Path::new(&exe_parent).exists();
            if !dir_ok && parent_ok {
                directory = exe_parent;
            } else if directory.is_empty() && !exe_parent.is_empty() {
                directory = exe_parent;
            }
            results.push(IntegrationEntry {
                name,
                path: directory,
                source: "lutris".into(),
                appid,
                prefix: yml_prefix,
                runner,
                executable,
                slug,
                playtime_hours: playtime,
                proton: yml_version,
            });
        }
        results.sort_by(|a, b| a.name.cmp(&b.name));
        results
    }

    fn scan_lutris_cli(&self) -> Vec<IntegrationEntry> {
        let mut results = Vec::new();
        let output = match Command::new("lutris").arg("-l").output() {
            Ok(o) if o.status.success() => o,
            _ => return results,
        };
        // `lutris -l` prints a table; data rows look like:
        //   12 | Game Name | runner | ...
        for line in String::from_utf8_lossy(&output.stdout).lines() {
            let cols: Vec<&str> = line.split('|').collect();
            if cols.len() < 2 {
                continue;
            }
            if cols[0].trim().parse::<u32>().is_err() {
                continue; // header / separator row
            }
            let name = cols[1].trim().to_string();
            if name.is_empty() {
                continue;
            }
            let runner = cols.get(2).map(|s| s.trim().to_string()).unwrap_or_default();
            results.push(IntegrationEntry {
                name,
                path: String::new(),
                source: "lutris".into(),
                appid: String::new(),
                prefix: String::new(),
                runner,
                executable: String::new(),
                slug: String::new(),
                playtime_hours: 0.0,
                proton: String::new(),
            });
        }
        results.sort_by(|a, b| a.name.cmp(&b.name));
        results
    }

    pub fn scan_steam(&self) -> Vec<IntegrationEntry> {
        let mut results = Vec::new();
        let steam_dir = home_dir()
            .unwrap_or_default()
            .join(".steam")
            .join("steam")
            .join("steamapps");
        // Steam keeps libraries in libraryfolders.vdf (VDF format, NOT json)
        let mut steam_dirs = vec![steam_dir.clone()];
        for candidate in [
            steam_dir.join("libraryfolders.vdf"),
            steam_dir.join("libraryfolders.json"),
        ] {
            if let Ok(content) = fs::read_to_string(&candidate) {
                for lib_path in parse_library_paths(&content) {
                    let expanded = shellexpand::tilde(&lib_path).to_string();
                    let p = PathBuf::from(&expanded).join("steamapps");
                    if p.exists() && !steam_dirs.contains(&p) {
                        steam_dirs.push(p);
                    }
                }
            }
        }
        for sd in &steam_dirs {
            let acf_dir = sd.join("common");
            if let Ok(entries) = fs::read_dir(sd) {
                for entry in entries.flatten() {
                    let p = entry.path();
                    // C++ parity: only appmanifest_*.acf files
                    let fname = p.file_name().unwrap_or_default().to_string_lossy().to_string();
                    if !fname.starts_with("appmanifest_") || !fname.ends_with(".acf") {
                        continue;
                    }
                    if let Ok(content) = fs::read_to_string(&p) {
                        let mut name = String::new();
                        let mut appid = String::new();
                        let mut installdir = String::new();
                        for line in content.lines() {
                            let trimmed = line.trim();
                            if trimmed.starts_with("\"name\"") {
                                name = extract_acf_value(trimmed);
                            }
                            if trimmed.starts_with("\"appid\"") {
                                appid = extract_acf_value(trimmed);
                            }
                            if trimmed.starts_with("\"installdir\"") {
                                installdir = extract_acf_value(trimmed);
                            }
                        }
                        if appid.is_empty() {
                            continue;
                        }
                        // C++ parity: skip Steam tools (numeric/empty names, name == appid)
                        if name.is_empty() || name.parse::<u64>().is_ok() || name == appid {
                            name = if installdir.is_empty() {
                                format!("Steam {}", appid)
                            } else {
                                installdir.clone()
                            };
                        }
                        if is_steam_tool(&name, &appid) {
                            continue;
                        }
                        if !installdir.is_empty() {
                            let game_path = acf_dir.join(&installdir);
                            // C++ parity: existing proton prefix for this app
                            let steam_prefix = sd.join("compatdata").join(&appid).join("pfx");
                            let prefix = if steam_prefix.exists() {
                                steam_prefix.display().to_string()
                            } else {
                                String::new()
                            };
                            results.push(IntegrationEntry {
                                name,
                                path: game_path.display().to_string(),
                                source: "steam".into(),
                                appid: appid.clone(),
                                runner: String::new(),
                                executable: String::new(),
                                slug: String::new(),
                                playtime_hours: 0.0,
                                proton: String::new(),
                                prefix,
                            });
                        }
                    }
                }
            }
        }
        results.sort_by(|a, b| a.name.cmp(&b.name));
        results
    }
}

/// Extract every library "path" value from libraryfolders.vdf (or the
/// legacy json variant). VDF lines look like: `"path"  "/mnt/games/Steam"`.
fn parse_library_paths(content: &str) -> Vec<String> {
    let mut out = Vec::new();
    for line in content.lines() {
        let trimmed = line.trim();
        if !trimmed.starts_with("\"path\"") {
            continue;
        }
        // Grab all quoted strings; the value is the second one
        let mut quoted = Vec::new();
        let mut chars = trimmed.chars();
        while let Some(c) = chars.next() {
            if c == '"' {
                let mut s = String::new();
                for ch in chars.by_ref() {
                    if ch == '"' {
                        break;
                    }
                    s.push(ch);
                }
                quoted.push(s);
            }
        }
        if quoted.len() >= 2 && !quoted[1].is_empty() && !out.contains(&quoted[1]) {
            out.push(quoted[1].clone());
        }
    }
    out
}

/// Skip known Steam tool / runtime appids (C++ isSteamTool parity).
fn is_steam_tool(name: &str, appid: &str) -> bool {
    const TOOL_IDS: &[&str] = &[
        "228980", "250820", "252950", "280390", "311460", "316450", "321360",
        "342710", "359320", "361420", "365670", "414490", "593110", "605740",
        "1054830", "1070560", "1113280", "1161040", "1172470", "1245040",
        "1290000", "1391110", "1493710", "1580130", "1628350", "1730190",
        "1730300", "1845910", "1887720", "1893810", "2348590", "2805730",
    ];
    if TOOL_IDS.contains(&appid) {
        return true;
    }
    let lower = name.to_lowercase();
    for marker in [
        "steamworks",
        "steam linux runtime",
        "proton ",
        "proton experimental",
        "steam deck",
    ] {
        if lower.contains(marker) {
            return true;
        }
    }
    false
}

/// ACF/VDF value extraction (C++ vdfValue parity). Lines look like:
/// `"name"		"Half-Life 2"` — no colon, key and value are both quoted.
fn extract_acf_value(line: &str) -> String {
    let mut quoted = Vec::new();
    let mut chars = line.trim().chars();
    while let Some(c) = chars.next() {
        if c == '"' {
            let mut s = String::new();
            for ch in chars.by_ref() {
                if ch == '"' {
                    break;
                }
                s.push(ch);
            }
            quoted.push(s);
        }
    }
    quoted.get(1).cloned().unwrap_or_default()
}

mod shellexpand {
    pub fn tilde(s: &str) -> String {
        if s.starts_with("~/") || s == "~" {
            if let Ok(home) = std::env::var("HOME") {
                return s.replacen("~", &home, 1);
            }
        }
        s.to_string()
    }
}

/// `"field": "..."` with backslash escapes (C++ grab-lambda parity).
fn json_field(haystack: &str, field: &str) -> Option<String> {
    let key = format!("\"{}\"", field);
    let pos = haystack.find(&key)?;
    let mut rest = &haystack[pos + key.len()..];
    rest = rest.trim_start_matches(|c: char| c == ':' || c.is_whitespace());
    if !rest.starts_with('"') {
        return None;
    }
    let mut out = String::new();
    let mut chars = rest[1..].chars();
    while let Some(c) = chars.next() {
        if c == '\\' {
            if let Some(e) = chars.next() {
                out.push(e);
            }
        } else if c == '"' {
            return Some(out);
        } else {
            out.push(c);
        }
    }
    None
}

/// Steam store suggest: AppID of the first entry whose name matches
/// normalized-equal ("Machine Party" -> "4108000"). Lets manual games
/// without a stored AppID use the Steam CDN / Store API tiers.
fn steam_appid_by_name(client: &reqwest::blocking::Client, game_name: &str) -> Option<String> {
    let want = norm_name(game_name);
    if want.is_empty() {
        return None;
    }
    let url = format!(
        "https://store.steampowered.com/search/suggest?term={}&f=games&cc=US&l=en",
        url_encode_query(game_name.trim())
    );
    let body = client.get(&url).send().ok()?.text().ok()?;
    let mut rest = body.as_str();
    loop {
        let pos = match rest.find("data-ds-appid=\"") {
            Some(p) => p,
            None => break,
        };
        let seg = &rest[pos + "data-ds-appid=\"".len()..];
        let appid: String = seg.chars().take_while(|c| c.is_ascii_digit()).collect();
        // Name shown in the following match_name div (bounded window).
        let window = &seg[..seg.len().min(800)];
        if let Some(npos) = window.find("match_name\">") {
            let raw = &window[npos + "match_name\">".len()..];
            let nm: String = raw.chars().take_while(|c| *c != '<').collect();
            let decoded = nm
                .replace("&amp;", "&")
                .replace("&#39;", "'")
                .replace("&quot;", "\"")
                .replace("&lt;", "<")
                .replace("&gt;", ">");
            if norm_name(&decoded) == want && !appid.is_empty() {
                return Some(appid);
            }
        }
        rest = &seg[5.min(seg.len())..];
        if rest.len() < 10 {
            break;
        }
    }
    None
}

/// Normalized name for artwork matching: lowercase alnum only.
fn norm_name(name: &str) -> String {
    name.to_lowercase()
        .chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .collect()
}

fn slugify(name: &str) -> String {
    let mut s: String = name
        .to_lowercase()
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .collect();
    while s.contains("--") {
        s = s.replace("--", "-");
    }
    s.trim_matches('-').to_string()
}

fn url_encode_query(s: &str) -> String {
    let mut out = String::new();
    for b in s.bytes() {
        if b.is_ascii_alphanumeric() || b"-_.~".contains(&b) {
            out.push(b as char);
        } else if b == b' ' {
            out.push_str("%20");
        } else {
            out.push_str(&format!("%{:02X}", b));
        }
    }
    out
}

fn copy_if_missing(src: &PathBuf, dest: &PathBuf) -> bool {
    if dest.exists() {
        return true;
    }
    if let Some(parent) = dest.parent() {
        fs::create_dir_all(parent).ok();
    }
    fs::copy(src, dest).is_ok() && dest.exists()
}

/// SteamGridDB autocomplete: id whose name matches normalized-equal.
fn find_grid_id(body: &str, game_name: &str) -> String {
    let want = norm_name(game_name);
    let mut rest = body;
    loop {
        let id_pos = match rest.find("\"id\"") {
            Some(p) => p,
            None => break,
        };
        let seg = &rest[id_pos..];
        let id_num: String = seg["\"id\"".len()..]
            .chars()
            .skip_while(|c| !c.is_ascii_digit())
            .take_while(|c| c.is_ascii_digit())
            .collect();
        // Look for the name belonging to this entry (bounded window)
        let window = &seg[..seg.len().min(600)];
        if let Some(npos) = window.find("\"name\"") {
            if let Some(nm) = json_field(&window[npos..], "name") {
                if norm_name(&nm) == want && !id_num.is_empty() {
                    return id_num;
                }
            }
        }
        rest = &seg[5.min(seg.len())..];
        if rest.len() < 10 {
            break;
        }
    }
    String::new()
}

fn shellexpand_tilde(s: &str) -> String {
    shellexpand::tilde(s)
}

fn format_size_bytes(bytes: u64) -> String {
    if bytes < 1024 { return format!("{} B", bytes); }
    if bytes < 1024 * 1024 { return format!("{:.1} KB", bytes as f64 / 1024.0); }
    if bytes < 1024 * 1024 * 1024 { return format!("{:.1} MB", bytes as f64 / (1024.0 * 1024.0)); }
    format!("{:.2} GB", bytes as f64 / (1024.0 * 1024.0 * 1024.0))
}
