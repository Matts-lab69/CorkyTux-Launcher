use glib::subclass::prelude::*;
use gtk::prelude::*;
use std::cell::RefCell;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

fn home_dir() -> Option<PathBuf> {
    std::env::var("HOME").ok().map(PathBuf::from)
}

#[derive(Clone, Debug, PartialEq)]
pub enum PluginType {
    Proton,
    Emulator,
    EmulatorManager,
    Tool,
    Integration,
    AppImage,
    Store,
    Minecraft,
    RpgMaker,
}

impl Default for PluginType {
    fn default() -> Self {
        PluginType::Tool
    }
}

impl PluginType {
    pub fn as_str(&self) -> &str {
        match self {
            PluginType::Proton => "Proton",
            PluginType::Emulator => "Emulator",
            PluginType::EmulatorManager => "emulator-manager",
            PluginType::Tool => "Tool",
            PluginType::Integration => "Integration",
            PluginType::AppImage => "AppImage",
            PluginType::Store => "Store",
            PluginType::Minecraft => "Minecraft",
            PluginType::RpgMaker => "RpgMaker",
        }
    }

    pub fn from_str(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "proton" => PluginType::Proton,
            "emulator" => PluginType::Emulator,
            "emulator-manager" | "emulatormanager" => PluginType::EmulatorManager,
            "integration" => PluginType::Integration,
            "appimage" | "app-image" | "app_image" => PluginType::AppImage,
            "store" => PluginType::Store,
            "minecraft" => PluginType::Minecraft,
            "rpgmaker" | "rpg-maker" | "rpg_maker" => PluginType::RpgMaker,
            _ => PluginType::Tool,
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct RegistryEntry {
    pub tag: String,
    pub name: String,
    pub description: String,
    pub date: String,
    pub url: String,
    pub asset_name: String,
}

#[derive(Clone, Debug, Default)]
pub struct EmuSettingDef {
    pub id: String,
    pub stype: String,
    pub default_bool: bool,
}

#[derive(Clone, Debug, Default)]
pub struct EmuInfo {
    pub name: String,
    pub path: String,
    pub description: String,
    pub installed: bool,
    pub native: bool,
    pub source: String,
    pub settings: Vec<EmuSettingDef>,
}

#[derive(Clone, Debug, Default)]
pub struct PluginInfo {
    pub id: String,
    pub name: String,
    pub version: String,
    pub description: String,
    pub enabled: bool,
    pub capabilities: Vec<String>,
    pub plugin_type: PluginType,
}

mod imp {
    use super::*;
    use glib::subclass::prelude::*;
    use std::cell::RefCell;
    use std::path::PathBuf;

    #[derive(Default)]
    pub struct PluginManager {
        pub plugins: RefCell<Vec<PluginInfo>>,
        pub plugins_dir: RefCell<PathBuf>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for PluginManager {
        const NAME: &'static str = "CorkyTuxPluginManager";
        type Type = super::PluginManager;
        type ParentType = glib::Object;

        fn new() -> Self {
            Self::default()
        }
    }

    impl ObjectImpl for PluginManager {
        fn constructed(&self) {
            self.parent_constructed();
            let data_dir = home_dir()
                .unwrap_or_default()
                .join(".local")
                .join("share")
                .join("CorkyTux");
            let plugins_dir = data_dir.join("plugins");
            fs::create_dir_all(&plugins_dir).ok();
            *self.plugins_dir.borrow_mut() = plugins_dir;
            self.obj().refresh();
        }
    }
}

glib::wrapper! {
    pub struct PluginManager(ObjectSubclass<imp::PluginManager>);
}

pub struct ScanBundle {
    pub overrides: String,
    pub missing: Vec<(String, String)>,
}

impl PluginManager {
    pub fn new() -> Self {
        glib::Object::new()
    }

    pub fn plugins_dir(&self) -> PathBuf {
        self.imp().plugins_dir.borrow().clone()
    }

    pub fn refresh(&self) {
        let dir = self.plugins_dir();
        let mut plugins = Vec::new();
        if dir.exists() {
            if let Ok(entries) = fs::read_dir(&dir) {
                for entry in entries.flatten() {
                    let path = entry.path();
                    if path.is_dir() {
                        let id = path
                            .file_name()
                            .unwrap_or_default()
                            .to_string_lossy()
                            .to_string();
                        // I match the legacy layout: manifest is plugin.json; I keep plugin.ini as fallback
                        let parsed = self
                            .parse_plugin_json(&path.join("plugin.json"), &id)
                            .or_else(|| self.parse_plugin_ini(&path.join("plugin.ini")));
                        if let Some(mut info) = parsed {
                            if info.id.is_empty() {
                                info.id = id.clone();
                            }
                            if info.name.is_empty() {
                                info.name = id.clone();
                            }
                            info.enabled = self.read_enabled(&info.id);
                            plugins.push(info);
                        }
                    }
                }
            }
        }
        plugins.sort_by(|a, b| a.name.cmp(&b.name));
        *self.imp().plugins.borrow_mut() = plugins;
    }

    fn read_enabled(&self, plugin_id: &str) -> bool {
        super::ConfigManager::new()
            .launcher_value_in("Plugins", &format!("{}.enabled", plugin_id))
            .map(|v| v != "0")
            .unwrap_or(true)
    }

    fn parse_plugin_json(&self, path: &Path, fallback_id: &str) -> Option<PluginInfo> {
        let content = fs::read_to_string(path).ok()?;
        let v: serde_json::Value = serde_json::from_str(&content).ok()?;
        if !v.is_object() {
            return None;
        }
        let get = |k: &str| v.get(k).and_then(|x| x.as_str()).unwrap_or("").to_string();
        let mut caps = Vec::new();
        if let Some(arr) = v.get("capabilities").and_then(|x| x.as_array()) {
            for c in arr {
                if let Some(s) = c.as_str() {
                    caps.push(s.to_string());
                }
            }
        }
        let type_str = get("type");
        Some(PluginInfo {
            id: fallback_id.to_string(),
            name: get("name"),
            version: get("version"),
            description: get("description"),
            enabled: true,
            capabilities: caps,
            plugin_type: PluginType::from_str(&type_str),
        })
    }

    fn parse_plugin_ini(&self, path: &Path) -> Option<PluginInfo> {
        let content = fs::read_to_string(path).ok()?;
        let mut info = PluginInfo::default();
        let mut current_section = String::new();
        for line in content.lines() {
            let trimmed = line.trim();
            if trimmed.is_empty() || trimmed.starts_with(';') {
                continue;
            }
            if trimmed.starts_with('[') && trimmed.ends_with(']') {
                current_section = trimmed[1..trimmed.len() - 1].trim().to_string();
                continue;
            }
            if let Some((key, value)) = trimmed.split_once('=') {
                let key = key.trim();
                let value = value.trim().to_string();
                match current_section.as_str() {
                    "Plugin" => match key {
                        "Id" => info.id = value,
                        "Name" => info.name = value,
                        "Version" => info.version = value,
                        "Description" => info.description = value,
                        "Type" => info.plugin_type = PluginType::from_str(&value),
                        _ => {}
                    },
                    "Settings" => match key {
                        "Enabled" => info.enabled = value.eq_ignore_ascii_case("true"),
                        _ => {}
                    },
                    "Capabilities" => {
                        if !value.is_empty() {
                            info.capabilities
                                .extend(value.split(',').map(|s| s.trim().to_string()));
                        }
                    }
                    _ => {}
                }
            }
        }
        if info.id.is_empty() {
            return None;
        }
        Some(info)
    }

    /// UI gating: true when the plugin is installed (listed under this id
    /// or manifest dir present) and not explicitly disabled.
    /// Sidebar buttons (Minecraft/Stores) and AddGame cards (AppImage/RPG
    /// Maker) are only shown when this returns true.
    pub fn is_available(&self, plugin_id: &str) -> bool {
        if let Some(p) = self.imp().plugins.borrow().iter().find(|p| p.id == plugin_id) {
            return p.enabled;
        }
        if self.plugins_dir().join(plugin_id).join("plugin.json").exists() {
            return super::ConfigManager::new()
                .launcher_value_in("Plugins", &format!("{}.enabled", plugin_id))
                .map(|v| v != "0")
                .unwrap_or(true);
        }
        false
    }

    pub fn is_enabled(&self, plugin_id: &str) -> bool {
        self.imp()
            .plugins
            .borrow()
            .iter()
            .find(|p| p.id == plugin_id)
            .map(|p| p.enabled)
            .unwrap_or(false)
    }

    pub fn set_enabled(&self, plugin_id: &str, enabled: bool) {
        // I match the legacy layout: [Plugins] section, "1"/"0"
        super::ConfigManager::new().set_launcher_value_in(
            "Plugins",
            &format!("{}.enabled", plugin_id),
            if enabled { "1" } else { "0" },
        );
        for plugin in self.imp().plugins.borrow_mut().iter_mut() {
            if plugin.id == plugin_id {
                plugin.enabled = enabled;
                break;
            }
        }
    }

    pub fn remove_plugin(&self, plugin_id: &str) {
        let dir = self.plugins_dir().join(plugin_id);
        if dir.exists() {
            fs::remove_dir_all(&dir).ok();
        }
        super::ConfigManager::new().remove_launcher_value_in(
            "Plugins",
            &format!("{}.enabled", plugin_id),
        );
        self.refresh();
    }

    // --- Remote registry (GitHub Releases, C++ parity) ---

    /// I check whether this asset fits my host CPU.
    ///
    /// I accept an asset with no arch token (`plugin.tar.gz`): that is the
    /// shape of current releases and I cannot tell it apart from a universal
    /// one. I only accept an asset WITH a token (`plugin-x86_64.tar.gz`) when
    /// it maps to my `ARCH`: without this check an aarch64 host could grab
    /// the x86_64 binary, "install" it fine, and fail every launch with
    /// `Exec format error`.
    ///
    /// I split tokens on `-` and `.`, never `_`, because `x86_64` carries an
    /// underscore and splitting it would yield two meaningless tokens.
    fn asset_arch_ok(name: &str) -> bool {
        let host = std::env::consts::ARCH;
        let mut saw_arch = false;
        for token in name.split(|c| c == '-' || c == '.') {
            let arch = match token.to_ascii_lowercase().as_str() {
                "x86_64" | "amd64" | "x64" => "x86_64",
                "aarch64" | "arm64" => "aarch64",
                "i386" | "i486" | "i586" | "i686" | "x86" => "x86",
                "arm" | "armv7" | "armv7l" | "armhf" => "arm",
                "ppc64" | "ppc64le" | "powerpc64" | "powerpc64le" => "powerpc64",
                "riscv64" => "riscv64",
                "s390x" => "s390x",
                _ => continue,
            };
            saw_arch = true;
            if arch == host {
                return true;
            }
        }
        // No recognized arch token: single asset, I assume it fits.
        !saw_arch
    }

    pub fn fetch_registry() -> Result<Vec<RegistryEntry>, String> {
        let url = "https://api.github.com/repos/Matts-lab69/CorkyTux-Plugins/releases?per_page=20";
        let client = reqwest::blocking::Client::builder()
            .user_agent("CorkyTux/3.0")
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .map_err(|e| format!("HTTP client failed: {}", e))?;
        let resp = client
            .get(url)
            .send()
            .map_err(|e| format!("Request failed: {}", e))?;
        if !resp.status().is_success() {
            return Err(format!("Registry HTTP error: {}", resp.status()));
        }
        let json: serde_json::Value =
            resp.json().map_err(|e| format!("Invalid JSON: {}", e))?;
        let mut out = Vec::new();
        for v in json.as_array().cloned().unwrap_or_default() {
            let tag = v
                .get("tag_name")
                .and_then(|x| x.as_str())
                .unwrap_or_default()
                .to_string();
            // I consider every .tar.gz in the release, not just the first: API
            // order is no compatibility guarantee.
            let mut candidates: Vec<(String, String)> = Vec::new();
            for a in v
                .get("assets")
                .and_then(|x| x.as_array())
                .cloned()
                .unwrap_or_default()
            {
                let n = a.get("name").and_then(|x| x.as_str()).unwrap_or_default();
                if !n.ends_with(".tar.gz") {
                    continue;
                }
                let u = a
                    .get("browser_download_url")
                    .and_then(|x| x.as_str())
                    .unwrap_or_default();
                if u.is_empty() {
                    continue;
                }
                candidates.push((n.to_string(), u.to_string()));
            }
            if candidates.is_empty() {
                continue;
            }
            let picked = candidates
                .iter()
                .find(|(name, _)| Self::asset_arch_ok(name))
                .cloned();
            // Release with assets for another arch only: I skip it instead of
            // offering a download that cannot work.
            let Some((asset_name, asset_url)) = picked else {
                let names: Vec<&str> = candidates.iter().map(|(n, _)| n.as_str()).collect();
                eprintln!(
                    "[plugins] release {} has no asset for {} (has: {})",
                    tag,
                    std::env::consts::ARCH,
                    names.join(", ")
                );
                continue;
            };
            let body = v.get("body").and_then(|x| x.as_str()).unwrap_or_default();
            out.push(RegistryEntry {
                tag,
                name: v.get("name").and_then(|x| x.as_str()).unwrap_or_default().to_string(),
                description: body.chars().take(200).collect(),
                date: v.get("published_at").and_then(|x| x.as_str()).unwrap_or_default().to_string(),
                url: asset_url,
                asset_name,
            });
        }
        Ok(out)
    }

    pub fn download_plugin(
        &self,
        tag: &str,
        url: &str,
        on_progress: impl FnMut(f64, f64),
    ) -> Result<String, String> {
        let res = Self::download_plugin_in(&self.plugins_dir(), tag, url, on_progress);
        self.refresh();
        res
    }

    /// Thread-safe (no self): downloads + extracts a plugin tarball.
    pub fn download_plugin_in(
        dir: &Path,
        tag: &str,
        url: &str,
        mut on_progress: impl FnMut(f64, f64),
    ) -> Result<String, String> {
        let safe_tag: String = tag
            .chars()
            .map(|c| {
                if c.is_ascii_alphanumeric() || c == '.' || c == '_' || c == '-' {
                    c
                } else {
                    '_'
                }
            })
            .collect();
        fs::create_dir_all(&dir).map_err(|e| format!("Cannot create dir: {}", e))?;
        let before: Vec<String> = fs::read_dir(&dir)
            .map(|entries| {
                entries
                    .flatten()
                    .filter(|e| e.path().is_dir())
                    .map(|e| e.file_name().to_string_lossy().to_string())
                    .collect()
            })
            .unwrap_or_default();
        let dest = dir.join(format!("{}.tar.gz", safe_tag));
        let client = reqwest::blocking::Client::builder()
            .user_agent("CorkyTux/3.0")
            .timeout(std::time::Duration::from_secs(600))
            .build()
            .map_err(|e| format!("HTTP client failed: {}", e))?;
        let mut resp = client.get(url).send().map_err(|e| format!("Download failed: {}", e))?;
        if !resp.status().is_success() {
            return Err(format!("Download HTTP error: {}", resp.status()));
        }
        let total = resp.content_length().unwrap_or(0);
        let mut file = fs::File::create(&dest).map_err(|e| format!("Cannot write: {}", e))?;
        let mut downloaded: u64 = 0;
        let start = std::time::Instant::now();
        loop {
            let mut buf = vec![0u8; 256 * 1024];
            let n = std::io::Read::read(&mut resp, &mut buf)
                .map_err(|e| format!("Read failed: {}", e))?;
            if n == 0 {
                break;
            }
            std::io::Write::write_all(&mut file, &buf[..n])
                .map_err(|e| format!("Write failed: {}", e))?;
            downloaded += n as u64;
            if total > 0 {
                let secs = start.elapsed().as_secs_f64().max(0.001);
                on_progress(downloaded as f64 / total as f64, downloaded as f64 / secs);
            }
        }
        drop(file);
        let size = fs::metadata(&dest).map(|m| m.len()).unwrap_or(0);
        if size < 100 {
            fs::remove_file(&dest).ok();
            return Err(format!("Downloaded file too small ({} bytes)", size));
        }
        let status = Command::new("tar")
            .args(["-xf", dest.to_str().unwrap_or("")])
            .current_dir(&dir)
            .status()
            .map_err(|e| format!("Extract failed: {}", e))?;
        fs::remove_file(&dest).ok();
        if !status.success() {
            return Err("Extraction failed".into());
        }
        // I detect the fresh dir holding plugin.json, like the legacy code did
        let after: Vec<String> = fs::read_dir(&dir)
            .map(|entries| {
                entries
                    .flatten()
                    .filter(|e| e.path().is_dir())
                    .map(|e| e.file_name().to_string_lossy().to_string())
                    .collect()
            })
            .unwrap_or_default();
        let secs = start.elapsed().as_secs_f64().max(0.001);
        let bps = downloaded as f64 / secs;
        for d in after.iter().filter(|d| !before.contains(d)) {
            if dir.join(d).join("plugin.json").exists() {
                on_progress(1.0, bps);
                return Ok(d.clone());
            }
        }
        on_progress(1.0, bps);
        Ok(tag.to_string())
    }

    // --- Dependency Installer + DLL Overrides Automator (I port the legacy applyScanPlugins behavior) ---
    //
    // Both are JSON-protocol scripts. All helpers below are thread-safe
    // associated fns (fresh state, plain-data results) for background use.

    fn dep_exe_in(dir: &Path) -> PathBuf {
        dir.join("dependency-installer").join("dependency-installer")
    }

    fn dll_exe_in(dir: &Path) -> PathBuf {
        dir.join("dll-overrides-automator").join("dll-overrides-automator")
    }

    /// I run a plugin and return its final JSON document. `limit_secs` is my
    /// time budget; I enforce it in-process, without relying on `timeout(1)`.
    fn run_json(cmd: &mut Command, limit_secs: u64) -> Result<serde_json::Value, String> {
        let output = super::plugin_process::output_with_timeout(
            cmd,
            std::time::Duration::from_secs(limit_secs),
        )
        .map_err(|e| format!("Plugin failed to start: {}", e))?;
        if !output.success {
            let err = output.stderr.trim().to_string();
            // Scripts print usage/help to stdout on arg errors; surface it.
            let out = output.stdout.trim().to_string();
            let msg = if !err.is_empty() { err } else { out };
            return Err(if msg.is_empty() {
                format!("Plugin returned an error (code {:?})", output.code)
            } else if msg.len() > 300 {
                msg[..300].to_string()
            } else {
                msg
            });
        }
        // Installers stream {"type": "progress", ...} lines before the final
        // document: parse per line and take the final result object.
        let body = &output.stdout;
        let mut last: Option<serde_json::Value> = None;
        let mut final_doc: Option<serde_json::Value> = None;
        for line in body.lines() {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }
            if let Ok(v) = serde_json::from_str::<serde_json::Value>(line) {
                if v.is_object() {
                    if v.get("ok").is_some() {
                        final_doc = Some(v);
                    } else {
                        last = Some(v);
                    }
                }
            }
        }
        final_doc
            .or(last)
            .ok_or_else(|| "Plugin returned invalid JSON".into())
    }

    /// `dependency-installer scan <gamedir> [--prefix P] [--proton PATH]`.
    /// Returns (missing [(id, desc)], message). Missing = detected entries
    /// whose status is not "installed".
    pub fn dep_scan_in(
        dir: &Path,
        game_dir: &str,
        prefix: &str,
        proton: &str,
    ) -> Result<(Vec<(String, String)>, String), String> {
        let exe = Self::dep_exe_in(dir);
        if !exe.exists() {
            return Err("Dependency Installer plugin not installed".into());
        }
        let mut cmd = Command::new(&exe);
        cmd.arg("scan").arg(game_dir);
        if !prefix.is_empty() {
            cmd.arg("--prefix").arg(prefix);
        }
        if !proton.is_empty() {
            cmd.arg("--proton").arg(proton);
        }
        let v = Self::run_json(&mut cmd, 120)?;
        if v.get("ok").and_then(|x| x.as_bool()).unwrap_or(false) != true {
            let e = v.get("error").and_then(|x| x.as_str()).unwrap_or("scan failed");
            return Err(e.to_string());
        }
        let mut missing = Vec::new();
        for d in v.get("deps").and_then(|x| x.as_array()).cloned().unwrap_or_default() {
            if d.get("status").and_then(|x| x.as_str()).unwrap_or("") == "installed" {
                continue;
            }
            let id = d.get("id").and_then(|x| x.as_str()).unwrap_or_default();
            if id.is_empty() {
                continue;
            }
            let desc = d.get("desc").and_then(|x| x.as_str()).unwrap_or(id);
            missing.push((id.to_string(), desc.to_string()));
        }
        let msg = v.get("message").and_then(|x| x.as_str()).unwrap_or("").to_string();
        Ok((missing, msg))
    }

    pub fn scan_bundle_in(
        dir: &Path,
        game_dir: &str,
        prefix: &str,
        proton: &str,
    ) -> ScanBundle {
        let (overrides, missing) = std::thread::scope(|s| {
            let a = s.spawn(|| Self::dll_scan_in(dir, game_dir).unwrap_or_default());
            let b = s.spawn(|| {
                Self::dep_scan_in(dir, game_dir, prefix, proton)
                    .map(|(m, _)| m)
                    .unwrap_or_default()
            });
            (a.join().unwrap_or_default(), b.join().unwrap_or_default())
        });
        ScanBundle { overrides, missing }
    }

    /// `dependency-installer install --prefix P [--proton PATH] dep...`
    /// (winetricks into the prefix; slow for .NET, capped here for safety).
    pub fn dep_install_in(
        dir: &Path,
        prefix: &str,
        proton: &str,
        dep_ids: &[String],
    ) -> Result<String, String> {
        let exe = Self::dep_exe_in(dir);
        if !exe.exists() {
            return Err("Dependency Installer plugin not installed".into());
        }
        if prefix.is_empty() || dep_ids.is_empty() {
            return Err("Nothing to install".into());
        }

        let has_dotnet = dep_ids
            .iter()
            .any(|d| matches!(d.as_str(), "dotnet48" | "dotnet472" | "dotnet40" | "dotnet20" | "dotnet35sp1"));
        let timeout_secs = if has_dotnet { 1800 } else { 300 };

        let mut cmd = Command::new(&exe);
        cmd.arg("install").arg("--prefix").arg(prefix);
        if !proton.is_empty() {
            cmd.arg("--proton").arg(proton);
        }
        for d in dep_ids {
            cmd.arg(d);
        }
        let v = Self::run_json(&mut cmd, timeout_secs)?;

        if v.get("ok").and_then(|x| x.as_bool()).unwrap_or(false) != true {
            if let Some(e) = v.get("error").and_then(|x| x.as_str()) {
                return Err(e.to_string());
            }
            let failed: Vec<String> = v
                .get("failed")
                .and_then(|x| x.as_array())
                .cloned()
                .unwrap_or_default()
                .into_iter()
                .filter_map(|x| x.as_str().map(|s| s.to_string()))
                .collect();
            let details: Vec<String> = v
                .get("results")
                .and_then(|x| x.as_array())
                .cloned()
                .unwrap_or_default()
                .into_iter()
                .filter_map(|r| {
                    let ok = r.get("ok").and_then(|x| x.as_bool()).unwrap_or(false);
                    if ok {
                        return None;
                    }
                    let id = r.get("id").and_then(|x| x.as_str()).unwrap_or("");
                    let desc = r.get("desc").and_then(|x| x.as_str()).unwrap_or(id);
                    let reason = r
                        .get("error")
                        .and_then(|x| x.as_str())
                        .unwrap_or("install failed");
                    Some(format!("{} ({}): {}", id, desc, reason))
                })
                .collect();
            let mut msg = if !details.is_empty() {
                details.join("\n")
            } else if !failed.is_empty() {
                format!("Failed: {}", failed.join(", "))
            } else {
                "install failed".into()
            };
            if msg.len() > 400 {
                msg.truncate(400);
                msg.push_str("…");
            }
            return Err(msg);
        }

        Ok(v.get("message").and_then(|x| x.as_str()).unwrap_or("Installed").to_string())
    }

    /// `dll-overrides-automator scan <gamedir>` → WINEDLLOVERRIDES string
    /// (empty when the game needs none).
    pub fn dll_scan_in(dir: &Path, game_dir: &str) -> Result<String, String> {
        let exe = Self::dll_exe_in(dir);
        if !exe.exists() {
            return Err("DLL Overrides Automator plugin not installed".into());
        }
        let mut cmd = Command::new(&exe);
        cmd.arg("scan").arg(game_dir);
        let v = Self::run_json(&mut cmd, 120)?;
        if v.get("ok").and_then(|x| x.as_bool()).unwrap_or(false) != true {
            let e = v.get("error").and_then(|x| x.as_str()).unwrap_or("scan failed");
            return Err(e.to_string());
        }
        Ok(v.get("overrides").and_then(|x| x.as_str()).unwrap_or("").to_string())
    }

    // --- Emulator Manager integration (I match the legacy layout) ---

    fn emulator_manager_exe(&self) -> PathBuf {
        Self::emulator_manager_exe_in(&self.plugins_dir())
    }

    fn emulator_manager_exe_in(dir: &Path) -> PathBuf {
        dir.join("emulator-manager").join("emulator-manager")
    }

    pub fn emulator_manager_installed(&self) -> bool {
        let exe = self.emulator_manager_exe();
        exe.exists()
            && self.plugins_dir().join("emulator-manager").join("plugin.json").exists()
    }

    pub fn list_emulators(&self) -> Vec<EmuInfo> {
        Self::list_emulators_in(&self.plugins_dir())
    }

    /// Thread-safe (no self): queries `emulator-manager corky-list`.
    ///
    /// I enforce the 15s budget in-process. It used to rely on `timeout(1)`,
    /// which is missing from the default NixOS PATH and minimal containers: when
    /// absent this function fell into the `_` branch and my Emulators tab stayed
    /// empty with no warning. I still report failures as an empty list because
    /// that is my callers' contract, but now I leave the reason on stderr.
    pub fn list_emulators_in(dir: &Path) -> Vec<EmuInfo> {
        let exe = Self::emulator_manager_exe_in(dir);
        if !exe.exists() {
            return Vec::new();
        }
        // Guarded so a stuck manager can never freeze the UI thread.
        let mut cmd = Command::new(&exe);
        cmd.arg("corky-list");
        let output = match super::plugin_process::output_with_timeout(
            &mut cmd,
            std::time::Duration::from_secs(15),
        ) {
            Ok(o) if o.success => o,
            Ok(o) => {
                eprintln!(
                    "[emu] corky-list exited with code {:?}: {}",
                    o.code,
                    o.stderr.trim()
                );
                return Vec::new();
            }
            Err(e) => {
                eprintln!("[emu] corky-list failed: {}", e);
                return Vec::new();
            }
        };
        let v: serde_json::Value = match serde_json::from_str(&output.stdout) {
            Ok(v) => v,
            Err(e) => {
                eprintln!("[emu] corky-list returned invalid JSON: {}", e);
                return Vec::new();
            }
        };
        if v.get("ok").and_then(|x| x.as_bool()).unwrap_or(false) != true {
            eprintln!(
                "[emu] corky-list reported ok=false: {}",
                v.get("error").and_then(|e| e.as_str()).unwrap_or("no detail")
            );
            return Vec::new();
        }
        let plugins_dir = dir.display().to_string();
        let mut out = Vec::new();
        for e in v.get("emulators").and_then(|x| x.as_array()).cloned().unwrap_or_default() {
            let path = e.get("path").and_then(|x| x.as_str()).unwrap_or_default().to_string();
            // Backend >=2.10 returns an explicit "source" field (linked/appimage/system/none).
            // For older backends, infer from the "linked" flag + path heuristic.
            let raw_source = e.get("source").and_then(|x| x.as_str()).unwrap_or("").to_string();
            let source = if raw_source.is_empty() {
                if e.get("linked").and_then(|x| x.as_bool()).unwrap_or(false)
                    || (!path.is_empty() && !path.starts_with(&plugins_dir)) {
                    "linked"
                } else if e.get("installed").and_then(|x| x.as_bool()).unwrap_or(false) {
                    "appimage"
                } else {
                    "none"
                }
            } else {
                &raw_source
            };
            let installed = source != "none"
                || e.get("installed").and_then(|x| x.as_bool()).unwrap_or(false);
            let native = source == "linked";
            let mut defs = Vec::new();
            for s in e.get("settings").and_then(|x| x.as_array()).cloned().unwrap_or_default() {
                let id = s.get("id").and_then(|x| x.as_str()).unwrap_or_default();
                if id.is_empty() {
                    continue;
                }
                defs.push(EmuSettingDef {
                    id: id.to_string(),
                    stype: s.get("type").and_then(|x| x.as_str()).unwrap_or("bool").to_string(),
                    default_bool: s.get("default").and_then(|x| x.as_bool()).unwrap_or(false),
                });
            }
            out.push(EmuInfo {
                name: e.get("name").and_then(|x| x.as_str()).unwrap_or_default().to_string(),
                path,
                description: e.get("description").and_then(|x| x.as_str()).unwrap_or_default().to_string(),
                installed,
                native,
                source: source.to_string(),
                settings: defs,
            });
        }
        out.sort_by(|a, b| a.name.cmp(&b.name));
        out
    }

    pub fn install_emulator(&self, name: &str) -> Result<String, String> {
        Self::install_emulator_in(&self.plugins_dir(), name)
    }

    /// Thread-safe (no self).
    pub fn install_emulator_in(dir: &Path, name: &str) -> Result<String, String> {
        let exe = Self::emulator_manager_exe_in(dir);
        if !exe.exists() {
            return Err("Emulator Manager plugin not installed".into());
        }
        let output = Command::new(&exe)
            .args(["corky-install", name])
            .output()
            .map_err(|e| format!("Install failed: {}", e))?;
        if !output.status.success() {
            let err = String::from_utf8_lossy(&output.stderr).trim().to_string();
            return Err(if err.is_empty() {
                format!("Install failed for {}", name)
            } else {
                err
            });
        }
        let v: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap_or_default();
        let ok = v.get("ok").and_then(|x| x.as_bool()).unwrap_or(false);
        let msg = v.get("message").and_then(|x| x.as_str()).unwrap_or("").to_string();
        if ok {
            Ok(if msg.is_empty() { name.to_string() } else { msg })
        } else {
            Err(if msg.is_empty() {
                format!("Install failed for {}", name)
            } else {
                msg
            })
        }
    }

    pub fn link_emulator(&self, name: &str, path: &str) -> Result<String, String> {
        Self::link_emulator_in(&self.plugins_dir(), name, path)
    }

    /// Thread-safe (no self). Pins an already-detected binary (typically a
    /// `system` emulator found via PATH) to an explicit linked.json entry,
    /// so it survives PATH changes as long as the file itself exists.
    pub fn link_emulator_in(dir: &Path, name: &str, path: &str) -> Result<String, String> {
        let exe = Self::emulator_manager_exe_in(dir);
        if !exe.exists() {
            return Err("Emulator Manager plugin not installed".into());
        }
        let output = Command::new(&exe)
            .args(["corky-link", name, path])
            .output()
            .map_err(|e| format!("Link failed: {}", e))?;
        if !output.status.success() {
            let err = String::from_utf8_lossy(&output.stderr).trim().to_string();
            return Err(if err.is_empty() {
                format!("Link failed for {}", name)
            } else {
                err
            });
        }
        let v: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap_or_default();
        let ok = v.get("ok").and_then(|x| x.as_bool()).unwrap_or(false);
        let msg = v.get("message").and_then(|x| x.as_str()).unwrap_or("").to_string();
        if ok {
            Ok(if msg.is_empty() { name.to_string() } else { msg })
        } else {
            Err(if msg.is_empty() {
                format!("Link failed for {}", name)
            } else {
                msg
            })
        }
    }

    pub fn remove_emulator(&self, name: &str) -> Result<String, String> {
        Self::remove_emulator_in(&self.plugins_dir(), name)
    }

    /// Thread-safe (no self).
    pub fn remove_emulator_in(dir: &Path, name: &str) -> Result<String, String> {
        let exe = Self::emulator_manager_exe_in(dir);
        if !exe.exists() {
            return Err("Emulator Manager plugin not installed".into());
        }
        let output = Command::new(&exe)
            .args(["corky-remove", name])
            .output()
            .map_err(|e| format!("Remove failed: {}", e))?;
        if !output.status.success() {
            let err = String::from_utf8_lossy(&output.stderr).trim().to_string();
            return Err(if err.is_empty() {
                format!("Remove failed for {}", name)
            } else {
                err
            });
        }
        Ok(name.to_string())
    }

    /// I release a linked emulator, leaving the binary intact.
    ///
    /// The plugin always exposed `corky-unlink`, but I never called it: linking
    /// was a one-way door and the only exit was deleting the binary or editing
    /// `linked.json` by hand.
    pub fn unlink_emulator(&self, name: &str) -> Result<String, String> {
        Self::unlink_emulator_in(&self.plugins_dir(), name)
    }

    /// Thread-safe (no self).
    pub fn unlink_emulator_in(dir: &Path, name: &str) -> Result<String, String> {
        let exe = Self::emulator_manager_exe_in(dir);
        if !exe.exists() {
            return Err("Emulator Manager plugin not installed".into());
        }
        let output = Command::new(&exe)
            .args(["corky-unlink", name])
            .output()
            .map_err(|e| format!("Unlink failed: {}", e))?;
        if !output.status.success() {
            let err = String::from_utf8_lossy(&output.stderr).trim().to_string();
            return Err(if err.is_empty() {
                format!("Unlink failed for {}", name)
            } else {
                err
            });
        }
        // The plugin answers {"ok":bool,"message":str}: I propagate the reason
        // instead of swallowing it, because "was not linked" is useful info
        // when a row went stale.
        let v: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap_or_default();
        let ok = v.get("ok").and_then(|x| x.as_bool()).unwrap_or(false);
        let msg = v.get("message").and_then(|x| x.as_str()).unwrap_or("").to_string();
        if ok {
            Ok(if msg.is_empty() { name.to_string() } else { msg })
        } else {
            Err(if msg.is_empty() {
                format!("Unlink failed for {}", name)
            } else {
                msg
            })
        }
    }

    pub fn enabled_ids(&self) -> Vec<String> {
        self.imp()
            .plugins
            .borrow()
            .iter()
            .filter(|p| p.enabled)
            .map(|p| p.id.clone())
            .collect()
    }

    pub fn list_plugins(&self) -> Vec<PluginInfo> {
        self.imp().plugins.borrow().clone()
    }

    pub fn get_plugin(&self, id: &str) -> Option<PluginInfo> {
        self.imp().plugins.borrow().iter().find(|p| p.id == id).cloned()
    }

    /// Resolve a runnable emulator binary for an executor id or name:
    /// emulator-manager list first, then emulator-type plugins, then PATH.
    pub fn emulator_path(&self, emulator_id: &str) -> Option<PathBuf> {
        for emu in self.list_emulators() {
            if emu.name == emulator_id && !emu.path.is_empty() {
                let p = PathBuf::from(&emu.path);
                if p.exists() {
                    return Some(p);
                }
            }
        }
        let plugins_dir = self.plugins_dir();
        let plugin = self
            .imp()
            .plugins
            .borrow()
            .iter()
            .find(|p| p.id == emulator_id)
            .cloned();
        if let Some(plugin) = plugin {
            if plugin.plugin_type == PluginType::Emulator {
                let plugin_dir = plugins_dir.join(emulator_id);
                let exe_json = plugin_dir.join("emulator.json");
                if let Ok(content) = fs::read_to_string(&exe_json) {
                    if let Ok(v) = serde_json::from_str::<serde_json::Value>(&content) {
                        if let Some(p) = v.get("path").and_then(|x| x.as_str()) {
                            let path = plugin_dir.join(p);
                            if path.exists() {
                                return Some(path);
                            }
                        }
                    }
                }
                if let Ok(content) = fs::read_to_string(plugin_dir.join("plugin.ini")) {
                    for line in content.lines() {
                        if let Some((key, value)) = line.trim().split_once('=') {
                            if key.trim() == "ExecutablePath" {
                                let path = plugin_dir.join(value.trim());
                                if path.exists() {
                                    return Some(path);
                                }
                            }
                        }
                    }
                }
                return which(&plugin.name);
            }
        }
        which(emulator_id)
    }

    /// Executor choices for AddGame: only INSTALLED emulator-manager
    /// entries (I match the legacy filter: emulators where installed),
    /// then emulator-type plugins (installed by definition).
    pub fn list_emulator_plugins(&self) -> Vec<(String, String)> {
        let mut out: Vec<(String, String)> = self
            .list_emulators()
            .into_iter()
            .filter(|e| e.installed)
            .map(|e| (e.name.clone(), e.name.clone()))
            .collect();
        for p in self.imp().plugins.borrow().iter() {
            if p.plugin_type == PluginType::Emulator
                && !out.iter().any(|(id, _)| id == &p.id)
            {
                out.push((p.id.clone(), p.name.clone()));
            }
        }
        out.sort_by(|a, b| a.1.cmp(&b.1));
        out
    }

    pub fn build_emulator_command(&self, emulator_id: &str, rom: &str, args: &[String]) -> Option<Command> {
        let path = self.emulator_path(emulator_id)?;
        let mut cmd = Command::new(path);
        cmd.arg(rom);
        for arg in args {
            cmd.arg(arg);
        }
        Some(cmd)
    }

    pub fn get_emulator_name(&self, emulator_id: &str) -> String {
        self.imp()
            .plugins
            .borrow()
            .iter()
            .find(|p| p.id == emulator_id)
            .map(|p| p.name.clone())
            .unwrap_or_else(|| emulator_id.to_string())
    }
}

fn which(name: &str) -> Option<PathBuf> {
    if let Ok(output) = Command::new("which").arg(name).output() {
        if output.status.success() {
            let path_str = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if !path_str.is_empty() {
                return Some(PathBuf::from(path_str));
            }
        }
    }
    None
}
