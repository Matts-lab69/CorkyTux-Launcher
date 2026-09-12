use glib::subclass::prelude::*;
use gtk::prelude::*;
use regex::Regex;
use std::cell::{Cell, RefCell};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};

fn home_dir() -> Option<PathBuf> {
    std::env::var("HOME").ok().map(PathBuf::from)
}

/// Parseo de argsBefore/argsAfter respetando comillas simples y dobles.
/// Paridad con ProtonManager.cpp (parser manual char-a-char, sin escapes).
fn split_quoted_args(input: &str) -> Vec<String> {
    let mut parsed = Vec::new();
    let mut current = String::new();
    let mut in_quote = false;
    let mut quote_char = '"';
    for c in input.chars() {
        if in_quote {
            if c == quote_char {
                in_quote = false;
            } else {
                current.push(c);
            }
        } else if c == '"' || c == '\'' {
            in_quote = true;
            quote_char = c;
        } else if c == ' ' {
            if !current.is_empty() {
                parsed.push(std::mem::take(&mut current));
            }
        } else {
            current.push(c);
        }
    }
    if !current.is_empty() {
        parsed.push(current);
    }
    parsed
}

#[derive(Clone, Debug, Default)]
pub struct ProtonInfo {
    pub name: String,
    pub path: PathBuf,
    pub version: String,
}

#[derive(Clone, Debug, PartialEq)]
pub enum GraphicsComponent {
    GameMode,
    MangoHud,
}

#[derive(Clone, Debug, Default)]
pub struct ComponentStatus {
    pub available: bool,
    pub installed64: bool,
    pub installed32: bool,
    pub game_arch: String,
}

mod imp {
    use super::*;
    use glib::subclass::prelude::*;
    use std::cell::{Cell, RefCell};
    use std::path::PathBuf;

    #[derive(Default)]
    pub struct ProtonManager {
        pub installed: RefCell<Vec<ProtonInfo>>,
        pub running_child: RefCell<Option<Child>>,
        pub game_running: Cell<bool>,
        pub session_game: RefCell<String>,
        pub session_start: RefCell<Option<std::time::Instant>>,
        pub session_prefix: RefCell<String>,
        pub session_proton_dir: RefCell<String>,
        pub session_appimage: Cell<bool>,
        pub session_watch: RefCell<String>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for ProtonManager {
        const NAME: &'static str = "CorkyTuxProtonManager";
        type Type = super::ProtonManager;
        type ParentType = glib::Object;

        fn new() -> Self {
            Self::default()
        }
    }

    impl ObjectImpl for ProtonManager {
        fn constructed(&self) {
            self.parent_constructed();
            self.obj().refresh_installed();
        }
    }
}

glib::wrapper! {
    pub struct ProtonManager(ObjectSubclass<imp::ProtonManager>);
}

impl ProtonManager {
    pub fn new() -> Self {
        glib::Object::new()
    }

    fn heroic_legendary_bin() -> Option<PathBuf> {
        let home = std::env::var("HOME").unwrap_or_default();
        let owned = PathBuf::from(&home)
            .join(".config/CorkyTux/plugins/heroic-store/bin/legendary");
        if owned.is_file() {
            return Some(owned);
        }
        which("legendary").map(PathBuf::from)
    }

    /// Build `legendary launch` for Heroic Epic games (None = fall back
    /// to the direct exe path). Legendary owns EOS/EAC wiring; we only
    /// add our env (anticheat runtimes, custom k=v) and track the child.
    #[allow(clippy::too_many_arguments)]
    fn legendary_launch_cmd(
        game: &std::collections::HashMap<String, String>,
        proton_path: &std::path::Path,
        executable: &str,
        main_path: &str,
        actual_prefix: &std::path::Path,
        real_prefix: &std::path::Path,
        environment: &str,
        args_before: &str,
        args_after: &str,
        launch_args: &str,
    ) -> Option<Command> {
        let app = game.get("heroicappid").cloned().unwrap_or_default();
        if app.is_empty() {
            return None;
        }
        let leg = Self::heroic_legendary_bin()?;
        // Record must exist in a legendary config (ours or Heroic's own),
        // else `legendary launch` errors out.
        let home = std::env::var("HOME").unwrap_or_default();
        let cfgs = [
            PathBuf::from(&home).join(".config/legendary"),
            PathBuf::from(&home).join(".config/heroic/legendaryConfig/legendary"),
        ];
        let mut use_cfg: Option<PathBuf> = None;
        for cfg in &cfgs {
            let has = std::fs::read_dir(cfg.join("manifests")).ok().map(|it| {
                it.flatten().any(|e| {
                    e.file_name().to_string_lossy().contains(app.as_str())
                })
            }).unwrap_or(false);
            if has {
                use_cfg = Some(cfg.clone());
                break;
            }
        }
        let use_cfg = use_cfg?;
        // Legendary invokes --wine like plain wine (no verb), but the
        // Proton script requires one ("Need a verb"). Stable wrapper that
        // injects waitforexitandrun (also keeps session tracking, since it
        // waits for the game to exit).
        let home = std::env::var("HOME").unwrap_or_default();
        let tools = PathBuf::from(&home).join(".local/share/CorkyTux/tools");
        std::fs::create_dir_all(&tools).ok();
        let wrapper = tools.join("legendary-proton");
        let wrapper_src = "#!/bin/sh\nexec \"$CORKY_PROTON_BIN\" waitforexitandrun \"$@\"\n";
        let write_it = std::fs::read_to_string(&wrapper).map(|c| c != wrapper_src).unwrap_or(true);
        if write_it {
            use std::os::unix::fs::PermissionsExt;
            if std::fs::write(&wrapper, wrapper_src).is_ok() {
                let _ = std::fs::set_permissions(&wrapper, std::fs::Permissions::from_mode(0o755));
            }
        }
        let wine_bin = find_proton_binary(proton_path).unwrap_or_else(|| proton_path.to_path_buf());
        let mut cmd = Command::new(&leg);
        cmd.env("CORKY_PROTON_BIN", &wine_bin);
        // Proton-as-wine needs these like any Steam-less Proton launch
        // (Heroic sets them too); without them proton exits silently.
        cmd.env("STEAM_COMPAT_DATA_PATH", actual_prefix);
        cmd.env("STEAM_COMPAT_CLIENT_INSTALL_PATH",
                PathBuf::from(&home).join(".steam/steam"));
        // protonfixes derives the game id from SteamAppId first; without it
        // it parses digits out of DATA_PATH and dies on name-based prefixes.
        if let Some(sid) = game.get("steamid").cloned() {
            if !sid.is_empty() {
                cmd.env("SteamAppId", sid);
            } else {
                cmd.env("SteamAppId", "0");
            }
        } else {
            cmd.env("SteamAppId", "0");
        }
        if let Some(mp) = game.get("mainpath").cloned() {
            if !mp.is_empty() {
                cmd.env("STEAM_COMPAT_INSTALL_PATH", mp);
            }
        }
        // Non-default session (Heroic's) must be selected explicitly.
        let default_cfg = PathBuf::from(&home).join(".config/legendary");
        if use_cfg != default_cfg {
            cmd.env("LEGENDARY_CONFIG_PATH", &use_cfg);
        }
        cmd.arg("launch").arg(&app);
        cmd.arg("--wine").arg(&wrapper);
        cmd.arg("--wine-prefix").arg(real_prefix);
        if !executable.is_empty() {
            let rel = std::path::Path::new(executable)
                .strip_prefix(main_path)
                .map(|p| p.to_string_lossy().to_string())
                .unwrap_or_else(|_| {
                    std::path::Path::new(executable)
                        .file_name().map(|n| n.to_string_lossy().to_string())
                        .unwrap_or_default()
                });
            let rel = rel.trim_start_matches('/').to_string();
            if !rel.is_empty() {
                cmd.arg("--override-exe").arg(rel);
            }
        }
        for arg in split_quoted_args(args_before) { cmd.arg(arg); }
        for arg in split_quoted_args(args_after) { cmd.arg(arg); }
        for arg in split_quoted_args(launch_args) { cmd.arg(arg); }
        if game.get("heroiceac").map(|v| v == "true").unwrap_or(false) {
            let rt = PathBuf::from(&home).join(".config/heroic/tools/runtimes/eac_runtime");
            if rt.is_dir() {
                cmd.env("PROTON_EAC_RUNTIME", &rt);
            }
        }
        if game.get("heroicbattleye").map(|v| v == "true").unwrap_or(false) {
            let rt = PathBuf::from(&home).join(".config/heroic/tools/runtimes/battleye_runtime");
            if rt.is_dir() {
                cmd.env("PROTON_BATTLEYE_RUNTIME", &rt);
            }
        }
        if !environment.is_empty() {
            for part in environment.split_whitespace() {
                if let Some((key, val)) = part.split_once('=') {
                    cmd.env(key, val);
                }
            }
        }
        let _ = actual_prefix;
        Some(cmd)
    }

    /// Force Wine virtual desktop at WxH (borderless, no exclusive
    /// fullscreen mode switches) by editing the idle prefix's user.reg.
    /// geom "WxH"; empty string removes it. Never touches a running prefix.
    pub fn apply_vdesktop(real_prefix: &std::path::Path, geom: &str) -> bool {
        let reg = real_prefix.join("user.reg");
        if !reg.is_file() {
            return false;
        }
        // wineserver running in this prefix? Don't touch live registries.
        let lock = real_prefix.join("wineserver.lock");
        let server_running = std::process::Command::new("sh")
            .args(["-c", "pgrep -f wineserver >/dev/null 2>&1"])
            .status().map(|s| s.success()).unwrap_or(false);
        if server_running && lock.exists() {
            return false;
        }
        let Ok(orig) = std::fs::read_to_string(&reg) else {
            return false;
        };
        let bak = real_prefix.join("user.reg.corky.bak");
        if !bak.exists() {
            let _ = std::fs::copy(&reg, &bak);
        }
        let ts = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
        let mut out: Vec<String> = Vec::new();
        let mut section = String::new();
        let mut did_desktops = false;
        let mut did_explorer = false;
        for line in orig.lines() {
            if line.starts_with('[') {
                if section == r"[Software\\Wine\\Explorer\\Desktops]" && !did_desktops && !geom.is_empty() {
                    out.push(format!("\"Default\"=\"{}\"", geom));
                    did_desktops = true;
                }
                section = line.split_whitespace().next().unwrap_or("").to_string();
                out.push(line.to_string());
                continue;
            }
            if section == r"[Software\\Wine\\Explorer\\Desktops]" {
                if line.starts_with("\"Default\"=") {
                    if geom.is_empty() {
                        continue;
                    }
                    out.push(format!("\"Default\"=\"{}\"", geom));
                    did_desktops = true;
                    continue;
                }
            }
            if section == r"[Software\\Wine\\Explorer]" {
                if line.starts_with("\"Desktop\"=") {
                    if geom.is_empty() {
                        continue;
                    }
                    out.push("\"Desktop\"=\"Default\"".to_string());
                    did_explorer = true;
                    continue;
                }
            }
            out.push(line.to_string());
        }
        if !geom.is_empty() {
            if !did_desktops {
                out.push(format!("[Software\\\\Wine\\\\Explorer\\\\Desktops] {}", ts));
                out.push(format!("\"Default\"=\"{}\"", geom));
            }
            if !did_explorer {
                let mut found = false;
                for i in 0..out.len() {
                    if out[i].starts_with("[Software\\\\Wine\\\\Explorer] ") {
                        out.insert(i + 1, "\"Desktop\"=\"Default\"".to_string());
                        found = true;
                        break;
                    }
                }
                if !found {
                    out.push(format!("[Software\\\\Wine\\\\Explorer] {}", ts));
                    out.push("\"Desktop\"=\"Default\"".to_string());
                }
            }
        }
        std::fs::write(&reg, out.join("\n") + "\n").is_ok()
    }

    pub fn steam_client_path(&self) -> PathBuf {
        home_dir()
            .unwrap_or_default()
            .join(".steam")
            .join("steam")
    }

    pub fn proton_paths(&self) -> Vec<PathBuf> {
        super::ConfigManager::new().all_proton_paths()
    }

    /// First monitor size as "WxH" for the virtual desktop default.
    pub fn primary_display_size() -> String {
        if let Some(display) = gtk::gdk::Display::default() {
            if let Some(mon) = display.monitors().item(0).and_then(|o| o.downcast::<gtk::gdk::Monitor>().ok()) {
                let r = mon.geometry();
                if r.width() > 0 && r.height() > 0 {
                    return format!("{}x{}", r.width(), r.height());
                }
            }
        }
        "1920x1080".to_string()
    }

    pub fn prefix_path(&self, game_name: &str) -> PathBuf {
        let config = super::ConfigManager::new();
        if let Some(prefix) = config.game_value(game_name, "PrefixPath") {
            if !prefix.is_empty() {
                return PathBuf::from(prefix);
            }
        }
        home_dir()
            .unwrap_or_default()
            .join(".local")
            .join("share")
            .join("Steam")
            .join("steamapps")
            .join("compatdata")
            .join("0")
            .join("pfx")
    }

    /// Only accept real Proton/compatibility-tool dirs: must contain a
    /// `proton` launcher, a `version` file, or a tool manifest/dist layout.
    /// Random folders (downloads, documents, game dirs) are skipped.
    fn is_proton_dir(path: &Path) -> bool {
        if path.join("version").is_file() {
            return true;
        }
        if path.join("compatibilitytool.vdf").is_file()
            || path.join("toolmanifest.vdf").is_file()
        {
            return true;
        }
        for base in ["dist", "files"] {
            let d = path.join(base);
            if d.join("bin").is_dir() || d.join("share").is_dir() {
                return true;
            }
        }
        if let Ok(entries) = fs::read_dir(path) {
            for entry in entries.flatten() {
                let p = entry.path();
                if p.is_file() {
                    if let Some(name) = p.file_name().and_then(|n| n.to_str()) {
                        if name == "proton" || name.starts_with("proton_") {
                            return true;
                        }
                    }
                }
            }
        }
        false
    }

    /// Skip builds for a foreign CPU arch (e.g. an aarch64 Proton dir on
    /// x86_64): they pass is_proton_dir but can never launch, and worse,
    /// they make the list look non-empty so auto-install never triggers.
    fn is_foreign_arch(name: &str) -> bool {
        let lower = name.to_lowercase();
        let has_arm = lower.contains("aarch64") || lower.contains("arm64");
        let has_x86 = lower.contains("x86_64");
        match std::env::consts::ARCH {
            "aarch64" => has_x86,
            _ => has_arm,
        }
    }

    fn scan_proton_dir(&self, dir: &Path) -> Vec<ProtonInfo> {
        let mut result = Vec::new();
        if !dir.exists() {
            return result;
        }
        if let Ok(entries) = fs::read_dir(dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() && Self::is_proton_dir(&path) {
                    let name = path.file_name().unwrap_or_default().to_string_lossy().to_string();
                    if Self::is_foreign_arch(&name) {
                        continue;
                    }
                    let version = fs::read_to_string(path.join("version"))
                        .unwrap_or_default();
                    result.push(ProtonInfo {
                        name,
                        path: path.clone(),
                        version: version.trim().to_string(),
                    });
                }
            }
        }
        result
    }

    /// Paridad con ProtonManager::findSteamRuntime (C++): localiza el
    /// run.sh del Steam Linux Runtime que corresponde a la versión de
    /// GE-Proton instalada, para dar soporte 32-bit en sistemas puramente
    /// 64-bit (ausente por completo en la reescritura Rust original).
    pub fn find_steam_runtime(&self, proton_name: &str) -> Option<PathBuf> {
        let home = home_dir()?;

        let version = self
            .imp()
            .installed.borrow()
            .iter()
            .find(|p| p.name == proton_name)
            .map(|p| p.version.clone())
            .unwrap_or_default();

        let proton_version: u32 = Regex::new(r"GE-Proton(\d+)-")
            .ok()
            .and_then(|re| re.captures(&version))
            .and_then(|c| c.get(1))
            .and_then(|m| m.as_str().parse().ok())
            .unwrap_or(11);

        let aarch64 = std::env::consts::ARCH == "aarch64";
        let dirname = if proton_version >= 11 {
            if aarch64 { "SteamLinuxRuntime_4-arm64" } else { "SteamLinuxRuntime_4" }
        } else if proton_version >= 8 {
            "SteamLinuxRuntime_sniper"
        } else {
            "SteamLinuxRuntime_soldier"
        };

        let roots = [
            home.join(".local/share/Steam"),
            home.join(".steam/steam"),
            home.join(".var/app/com.valvesoftware.Steam/data/Steam"),
            home.join(".var/app/com.valvesoftware.Steam/.steam/steam"),
        ];
        for root in &roots {
            for candidate in [
                root.join("ubuntu12_32/steam-runtime/run.sh"),
                root.join("ubuntu12_64/steam-runtime/run.sh"),
                root.join("compatibilitytools.d").join(dirname).join("run.sh"),
                root.join("steamapps/compatibilitytools.d").join(dirname).join("run.sh"),
            ] {
                if candidate.exists() {
                    return Some(candidate);
                }
            }
        }
        None
    }

    pub fn refresh_installed(&self) {
        let mut all = Vec::new();
        for dir in self.proton_paths() {
            all.extend(self.scan_proton_dir(&dir));
        }
        all.sort_by(|a, b| a.name.cmp(&b.name));
        *self.imp().installed.borrow_mut() = all;
    }

    pub fn installed_protons(&self) -> Vec<String> {
        self.imp().installed.borrow().iter().map(|p| p.name.clone()).collect()
    }

    pub fn installed_proton_details(&self) -> Vec<(String, String, String)> {
        self.imp()
            .installed
            .borrow()
            .iter()
            .map(|p| (p.name.clone(), p.version.clone(), p.path.display().to_string()))
            .collect()
    }

    pub fn installed_proton_entries(&self) -> Vec<ProtonInfo> {
        self.imp().installed.borrow().clone()
    }

    /// C++ parity: "GE-Proton Latest" or empty resolves to the global
    /// defaultProton, else the newest installed GE build, else any installed.
    pub fn resolve_proton(&self, wanted: &str) -> Result<(String, PathBuf), String> {
        let entries = self.installed_proton_entries();
        if entries.is_empty() {
            return Err("No Proton builds installed".into());
        }
        if !wanted.is_empty() && wanted != "GE-Proton Latest" {
            if let Some(p) = entries.iter().find(|p| p.name == wanted) {
                return Ok((p.name.clone(), p.path.clone()));
            }
            // Dir names vary by source (Steam suffixes-arch: the stored
            // short name "cachyos-11.0-20260703-slr" must match dir
            // "proton-cachyos-11.0-20260703-slr-x86_64_v3").
            let mut fuzzy: Vec<_> = entries
                .iter()
                .filter(|p| p.name.contains(wanted) || wanted.contains(p.name.as_str()))
                .collect();
            fuzzy.sort_by(|a, b| b.name.cmp(&a.name));
            if let Some(p) = fuzzy.first() {
                return Ok((p.name.clone(), p.path.clone()));
            }
            return Err(format!("Proton '{}' not found", wanted));
        }
        let config = super::ConfigManager::new();
        if let Some(def) = config.launcher_value("defaultProton").filter(|v| !v.is_empty()) {
            if let Some(p) = entries.iter().find(|p| p.name == def) {
                return Ok((p.name.clone(), p.path.clone()));
            }
        }
        // Newest GE build preferred, else first installed
        let mut sorted = entries.clone();
        sorted.sort_by(|a, b| b.name.cmp(&a.name));
        let pick = sorted
            .iter()
            .find(|p| p.name.contains("GE-Proton"))
            .or_else(|| sorted.first())
            .unwrap();
        Ok((pick.name.clone(), pick.path.clone()))
    }

    pub fn bundled_umu() -> Option<PathBuf> {
        let home = std::env::var("HOME").unwrap_or_default();
        let p = PathBuf::from(home).join(".local/share/CorkyTux/tools/umu/umu");
        if p.is_file() {
            Some(p)
        } else {
            None
        }
    }

    pub fn is_umu_available(&self) -> bool {
        if Self::bundled_umu().is_some() {
            return true;
        }
        Command::new("umu-run")
            .arg("--version")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .is_ok()
    }

    pub fn umu_executable(&self) -> Option<String> {
        if let Some(p) = Self::bundled_umu() {
            return Some(p.display().to_string());
        }
        if self.is_umu_available() {
            return Some("umu-run".to_string());
        }
        which("umu-run")
    }

    /// C++ graphicsComponentStatus parity: availability + 64/32-bit install
    /// state + the game's own architecture (from its PE header).
    pub fn component_status(&self, which: &str, exe: &str) -> ComponentStatus {
        let (bin, lib) = match which {
            "mangohud" => ("mangohud", "libMangoHud"),
            _ => ("gamemoderun", "libgamemodeauto"),
        };
        let available = Command::new(bin)
            .arg("--version")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .map(|s| s.success())
            .unwrap_or(false);
        let mut installed64 = available;
        let mut installed32 = false;
        if available {
            if let Ok(output) = Command::new("ldconfig").arg("-p").output() {
                let text = String::from_utf8_lossy(&output.stdout);
                for line in text.lines() {
                    if line.contains(lib) {
                        if line.contains("i386") || line.contains("lib32") {
                            installed32 = true;
                        }
                        if line.contains("x86-64") || line.contains("lib64") {
                            installed64 = true;
                        }
                    }
                }
            }
        } else {
            installed64 = false;
        }
        let game_arch = if exe.is_empty() {
            String::new()
        } else {
            match Self::detect_game_arch_simple(exe).as_str() {
                "64" => "64".to_string(),
                "32" => "32".to_string(),
                _ => String::new(),
            }
        };
        ComponentStatus { available, installed64, installed32, game_arch }
    }

    fn detect_game_arch_simple(exe_path: &str) -> String {
        let full = shellexpand_tilde(exe_path);
        let content = match fs::read(&full) {
            Ok(c) => c,
            Err(_) => return String::new(),
        };
        if content.len() < 0x40 || content[0] != b'M' || content[1] != b'Z' {
            return String::new();
        }
        let off = u32::from_le_bytes([content[0x3C], content[0x3D], content[0x3E], content[0x3F]]) as usize;
        if content.len() < off + 26 || content[off] != b'P' || content[off + 1] != b'E' {
            return String::new();
        }
        match u16::from_le_bytes([content[off + 24], content[off + 25]]) {
            0x20b => "64".to_string(),
            0x10b => "32".to_string(),
            _ => String::new(),
        }
    }

    /// Status line exactly like the QML (ready per-arch / partial / missing).
    pub fn component_status_text(status: &ComponentStatus, display: &str) -> String {
        let label = match status.game_arch.as_str() {
            "32" => "32-bit",
            "64" => "64-bit",
            _ => "all",
        };
        if status.available {
            return format!("{}: ready ({})", display, label);
        }
        if status.installed64 {
            let need = if status.game_arch == "32" { "32-bit" } else { "64-bit" };
            return format!("{}: 64-bit only (needs {})", display, need);
        }
        if status.installed32 {
            let need = if status.game_arch == "64" { "64-bit" } else { "32-bit" };
            return format!("{}: 32-bit only (needs {})", display, need);
        }
        format!("{}: not installed", display)
    }

    pub fn graphics_component_status(&self) -> Vec<(GraphicsComponent, bool)> {
        let gamemode = Command::new("gamemoded")
            .arg("--version")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .map(|s| s.success())
            .unwrap_or(false);
        let mangohud = Command::new("mangohud")
            .arg("--version")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .map(|s| s.success())
            .unwrap_or(false);
        vec![
            (GraphicsComponent::GameMode, gamemode),
            (GraphicsComponent::MangoHud, mangohud),
        ]
    }

    pub fn run_game(&self, game_name: &str) -> Result<(), String> {
        let config = super::ConfigManager::new();
        let game = config.game_section(game_name);
        let executable = game.get("executable")
            .cloned()
            .unwrap_or_default();
        if executable.is_empty() {
            // Steam shortcut (not installed locally): open via the client.
            // No child to track, so no session/time banking.
            let steam_id = game.get("steamid").cloned().unwrap_or_default();
            let sid = steam_id.trim();
            if !sid.is_empty() && sid.chars().all(|c| c.is_ascii_digit()) {
                Command::new("xdg-open")
                    .arg(format!("steam://rungameid/{}", sid))
                    .stdin(Stdio::null())
                    .stdout(Stdio::null())
                    .stderr(Stdio::null())
                    .spawn()
                    .map_err(|e| format!("Failed to open Steam: {}", e))?;
                return Ok(());
            }
            return Err("No executable set".into());
        }
        // Emulator games bypass Proton entirely (C++ parity: executor path)
        let executor = game.get("executor").cloned().unwrap_or_default();
        let source = game.get("source").cloned().unwrap_or_default();
        if executor == "appimage-launcher" || source.eq_ignore_ascii_case("appimage") {
            return self.run_appimage_game(game_name, &game);
        }
        if executor == "rpgmaker-runtime" || source.eq_ignore_ascii_case("rpgmaker") {
            return self.run_rpg_game(game_name, &game);
        }
        if !executor.is_empty() {
            return self.run_emulator_game(game_name, &game);
        }
        let wanted_proton = game.get("proton").cloned().unwrap_or_default();
        // Auto-create the individual prefix when the game has none.
        let prefix = self.ensure_individual_prefix(game_name);

        let (proton_name, proton_path) = self.resolve_proton(&wanted_proton)?;

        let proton_bin = find_proton_binary(&proton_path)
            .ok_or("Proton binary not found")?;

        let steam_client = self.steam_client_path();
        let main_path = game.get("mainpath").cloned().unwrap_or_default();
        let install_path = if !main_path.is_empty() { main_path.clone() } else { String::from("/") };
        let steam_id = game.get("steamid").cloned().unwrap_or_default();
        let use_umu = game.get("useumu").map(|v| v == "true").unwrap_or(false);
        let overrides = game.get("overrides").cloned().unwrap_or_default();
        let use_shared_prefix = game.get("usesharedprefix").map(|v| v == "true").unwrap_or(false);
        let shared_prefix_name = game.get("sharedprefix").cloned().unwrap_or_default();
        let game_mode = game.get("gamemode").map(|v| v == "true").unwrap_or(false);
        let mango_hud = game.get("mangohud").map(|v| v == "true").unwrap_or(false);
        let wined3d = game.get("usewined3d").map(|v| v == "true").unwrap_or(false);
        let native_wayland_set = game.get("nativewayland").cloned();
        let environment = game.get("environment").cloned().unwrap_or_default();
        let args_before = game.get("argsbefore").cloned().unwrap_or_default();
        let args_after = game.get("argsafter").cloned().unwrap_or_default();
        let mut launch_args = game.get("launchargs").cloned().unwrap_or_default();
        // Epic Online Services auth (Fall Guys & co. abort with
        // "no se encontró un código de intercambio" without it).
        // Same as Heroic: fresh exchange code passed as launch args.
        // Explicit toggle wins; otherwise auto-on for Heroic Epic games.
        let eos_on = match game.get("eosauth").map(|v| v.as_str()) {
            Some("false") | Some("0") => false,
            Some(_) => true,
            None => game.get("heroicstore").map(|v| v == "epic").unwrap_or(false),
        };
        if eos_on {
            if let Ok(doc) = super::external::StoreManager::eos_code() {
                let code = doc.get("code").and_then(|x| x.as_str()).unwrap_or("");
                if !code.is_empty() {
                    let uid = doc.get("account_id").and_then(|x| x.as_str()).unwrap_or("");
                    let uname = doc.get("display_name").and_then(|x| x.as_str()).unwrap_or("");
                    launch_args.push_str(&format!(
                        " -auth_login=unused -auth_password={} -auth_type=exchangecode -epicenv=Prod -EpicPortal -epicusername={} -epicuserid={}",
                        code, uname, uid));
                }
            }
        }
        let _fake_steam_id = {
            let v = game.get("fakesteamid").cloned().unwrap_or_default();
            if v.is_empty() { "480".to_string() } else { v }
        };

        // Resolve actual prefix (shared o por juego).
        // C++ parity: shared prefix se indexa por proton; SharedPrefix
        // del juego lo sobreescribe cuando está seteado.
        let shared_key = if !shared_prefix_name.is_empty() {
            shared_prefix_name.clone()
        } else {
            proton_name.clone()
        };
        let actual_prefix = if use_shared_prefix {
            config.shared_prefix_path(&shared_key)
                .unwrap_or_else(|| prefix.clone())
        } else {
            prefix.clone()
        };
        fs::create_dir_all(&actual_prefix).ok();

        // Proton appends "/pfx" internally: the compat dir IS actual_prefix,
        // so the real prefix is actual_prefix/pfx — per-game, never shared
        // in the parent dir.
        let compat_data_path = actual_prefix.clone();
        let real_prefix = actual_prefix.join("pfx");

        // Wine virtual desktop (borderless window at desktop size instead
        // of exclusive fullscreen): avoids mode switches that crash some
        // compositors. Applied to user.reg while the prefix is idle.
        if let Some(geom) = game.get("winevdesktop").cloned() {
            if !geom.is_empty() {
                Self::apply_vdesktop(&real_prefix, &geom);
            }
        }

        // Heroic-pattern Epic launch: `legendary launch` injects the game
        // token, -epicapp/sandbox/ovt and metadata params, and drives wine
        // itself (EOS overlay + EAC wiring included). Falls through to the
        // direct-exe path when legendary or its record is missing.
        if game.get("heroicstore").map(|v| v == "epic").unwrap_or(false) {
            if let Some(mut leg_cmd) = Self::legendary_launch_cmd(
                &game, &proton_path, &executable, &main_path,
                &actual_prefix, &real_prefix, &environment,
                &args_before, &args_after, &launch_args,
            ) {
                if mango_hud {
                    leg_cmd.env("MANGOHUD", "1");
                }
                *self.imp().session_prefix.borrow_mut() = real_prefix.display().to_string();
                *self.imp().session_proton_dir.borrow_mut() = proton_path.display().to_string();
                let mut wrapped_cmd = if game_mode {
                    let mut gm = Command::new("gamemoderun");
                    gm.arg(leg_cmd.get_program());
                    for arg in leg_cmd.get_args() {
                        gm.arg(arg);
                    }
                    for (key, val) in leg_cmd.get_envs() {
                        if let Some(v) = val {
                            gm.env(key, v);
                        }
                    }
                    gm
                } else {
                    leg_cmd
                };
                Self::attach_log_file(&mut wrapped_cmd, game_name);
                let child = wrapped_cmd
                    .spawn()
                    .map_err(|e| format!("Failed to launch: {}", e))?;
                *self.imp().running_child.borrow_mut() = Some(child);
                self.begin_session(game_name);
                return Ok(());
            }
        }

        // steamapps dir (para STEAM_COMPAT_LIBRARY_PATHS): padre de "common"
        let steamapps_dir = if !main_path.is_empty() {
            Path::new(&main_path).parent().and_then(|p| p.parent()).map(|p| p.to_path_buf())
        } else {
            None
        };

        let mut final_cmd;

        if use_umu {
            let umu_path = self.umu_executable().ok_or("umu-run not found")?;
            final_cmd = Command::new(umu_path);
            final_cmd.env("PROTONPATH", &proton_path);
            // umu uses WINEPREFIX directly (no /pfx append).
            final_cmd.env("WINEPREFIX", &actual_prefix);
            *self.imp().session_prefix.borrow_mut() = actual_prefix.display().to_string();
            *self.imp().session_proton_dir.borrow_mut() = proton_path.display().to_string();
            let gameid = if steam_id.is_empty() { "0".to_string() } else { steam_id.clone() };
            final_cmd.env("GAMEID", &gameid);
            final_cmd.env("EXE", &executable);
            final_cmd.env("STORE", "steam");
            // Never inherit UMU_LOG=1 from the session: it makes umu dump
            // the whole environment (~100 DEBUG lines) into every log.
            final_cmd.env_remove("UMU_LOG");
            // args: [argsBefore...] EXE [argsAfter...]
            for arg in split_quoted_args(&args_before) { final_cmd.arg(arg); }
            final_cmd.arg(&executable);
            for arg in split_quoted_args(&args_after) { final_cmd.arg(arg); }
        } else {
            final_cmd = Command::new(&proton_bin);
            if !overrides.is_empty() {
                final_cmd.env("WINEDLLOVERRIDES", &overrides);
            }
            // err+all: wine errors stay visible for the Debug log
            // (plain -all hid every err: line the legend mentions).
            final_cmd.env("WINEDEBUG", "err+all");
            final_cmd.env("WINEPREFIX", &real_prefix);
            *self.imp().session_prefix.borrow_mut() = real_prefix.display().to_string();
            *self.imp().session_proton_dir.borrow_mut() = proton_path.display().to_string();
            final_cmd.env("STEAM_COMPAT_DATA_PATH", &compat_data_path);
            final_cmd.env("STEAM_COMPAT_CLIENT_INSTALL_PATH", &steam_client);
            final_cmd.env("STEAM_COMPAT_INSTALL_PATH", &install_path);
            // CachyOS Proton solo da DXVK d3d8 si se pide en compat_config
            final_cmd.env("STEAM_COMPAT_CONFIG", "dxvkd3d8");
            if let Some(dir) = &steamapps_dir {
                final_cmd.env("STEAM_COMPAT_LIBRARY_PATHS", dir);
            }
            if use_shared_prefix {
                if let Some(shared_path) = config.shared_prefix_path(&shared_key) {
                    if shared_path.exists() {
                        final_cmd.env("STEAM_COMPAT_LIBRARY_PATHS", &shared_path);
                    }
                }
            }
            final_cmd.env("PROTON_USE_WINED3D", if wined3d { "1" } else { "0" });
            let steam_app_id = if steam_id.is_empty() { "0".to_string() } else { steam_id.clone() };
            final_cmd.env("SteamAppId", &steam_app_id);
            if let Some(wayland) = &native_wayland_set {
                final_cmd.env("PROTON_ENABLE_WAYLAND", if wayland == "true" { "1" } else { "0" });
            }

            // Environment personalizado ("k=v k2=v2")
            if !environment.is_empty() {
                for part in environment.split_whitespace() {
                    if let Some((key, val)) = part.split_once('=') {
                        final_cmd.env(key, val);
                    }
                }
            }

            // Heroic anti-cheat runtimes (Heroic pattern: point Proton at
            // the EAC/BattlEye runtimes shipped with Heroic).
            if game.get("heroicstore").map(|v| !v.is_empty()).unwrap_or(false) {
                let home = std::env::var("HOME").unwrap_or_default();
                if game.get("heroiceac").map(|v| v == "true").unwrap_or(false) {
                    let rt = std::path::PathBuf::from(&home)
                        .join(".config/heroic/tools/runtimes/eac_runtime");
                    if rt.is_dir() {
                        final_cmd.env("PROTON_EAC_RUNTIME", &rt);
                    }
                }
                if game.get("heroicbattleye").map(|v| v == "true").unwrap_or(false) {
                    let rt = std::path::PathBuf::from(&home)
                        .join(".config/heroic/tools/runtimes/battleye_runtime");
                    if rt.is_dir() {
                        final_cmd.env("PROTON_BATTLEYE_RUNTIME", &rt);
                    }
                }
            }

            if mango_hud {
                final_cmd.env("MANGOHUD", "1");
            }

            // Comando estándar Proton: proton waitforexitandrun game.exe
            final_cmd.arg("waitforexitandrun");
            for arg in split_quoted_args(&args_before) { final_cmd.arg(arg); }
            final_cmd.arg(&executable);
            for arg in split_quoted_args(&args_after) { final_cmd.arg(arg); }
            for arg in split_quoted_args(&launch_args) { final_cmd.arg(arg); }

            // Steam Runtime (paridad con C++ findSteamRuntime): provee libs
            // 32-bit (pulse/alsa) en sistemas puramente 64-bit. Solo aplica
            // a la ruta Proton estándar, no a umu-launcher.
            let runtime_flag = game.get("steamruntime").cloned().unwrap_or_else(|| {
                if config.launcher_value("gamesUsesSteamRuntime").map(|v| v == "1").unwrap_or(true) {
                    "true".to_string()
                } else {
                    "false".to_string()
                }
            });
            if runtime_flag == "true" {
                if let Some(runtime_sh) = self.find_steam_runtime(&proton_name) {
                    let mut rt_cmd = Command::new(runtime_sh);
                    rt_cmd.arg(final_cmd.get_program());
                    for arg in final_cmd.get_args() {
                        rt_cmd.arg(arg);
                    }
                    for (key, val) in final_cmd.get_envs() {
                        if let Some(v) = val {
                            rt_cmd.env(key, v);
                        }
                    }
                    final_cmd = rt_cmd;
                }
            }
        }

        // Wrapper GameMode (gamemoderun antepuesto al programa completo)
        let mut wrapped_cmd = if game_mode {
            let mut gm = Command::new("gamemoderun");
            gm.arg(final_cmd.get_program());
            for arg in final_cmd.get_args() {
                gm.arg(arg);
            }
            for (key, val) in final_cmd.get_envs() {
                if let Some(v) = val {
                    gm.env(key, v);
                }
            }
            gm
        } else {
            final_cmd
        };

        Self::attach_log_file(&mut wrapped_cmd, game_name);
        let child = wrapped_cmd
            .spawn()
            .map_err(|e| format!("Failed to launch: {}", e))?;
        *self.imp().running_child.borrow_mut() = Some(child);
        self.begin_session(game_name);
        Ok(())
    }

    /// Lutris-style header prepended to the Debug view: game setup +
    /// system info + a legend for the noisiest harmless lines, so raw
    /// Proton output (ProtonFixes INFO, fsync, FSR4 WARNs) reads clearly.
    pub fn debug_header(&self, game_name: &str) -> String {
        let config = super::ConfigManager::new();
        let game = config.game_section(game_name);
        let get = |k: &str| game.get(k).cloned().unwrap_or_default();
        let flag = |k: &str| get(k) == "true";
        let exe = get("executable");
        let executor = get("executor");
        let source = get("source");
        let is_appimage = executor == "appimage-launcher" || source.eq_ignore_ascii_case("appimage");
        let is_rpg = executor == "rpgmaker-runtime" || source.eq_ignore_ascii_case("rpgmaker");
        let is_emu = !executor.is_empty() && !is_appimage && !is_rpg;
        let wanted_proton = get("proton");
        let (proton_name, proton_path) = self
            .resolve_proton(&wanted_proton)
            .unwrap_or((wanted_proton.clone(), PathBuf::new()));
        let wine_ver = proton_path
            .join("version")
            .is_file()
            .then(|| {
                std::fs::read_to_string(proton_path.join("version"))
                    .unwrap_or_default()
                    .lines()
                    .next()
                    .unwrap_or("")
                    .trim()
                    .to_string()
            })
            .unwrap_or_default();
        // Same layout as run_game: compat dir + real prefix at /pfx.
        let shared_key = {
            let sp = get("sharedprefix");
            if !sp.is_empty() { sp } else { proton_name.clone() }
        };
        let compat = if flag("usesharedprefix") {
            config
                .shared_prefix_path(&shared_key)
                .unwrap_or_else(|| self.prefix_path(game_name))
        } else {
            self.prefix_path(game_name)
        };
        let prefix_disp = if is_appimage {
            String::from("(none - native AppImage)")
        } else if is_rpg {
            String::from("(none - box-rpg managed)")
        } else if is_emu {
            String::from("(none - native emulator)")
        } else if flag("useumu") {
            compat.display().to_string()
        } else {
            compat.join("pfx").display().to_string()
        };
        let runtime = game.get("steamruntime").cloned().unwrap_or_else(|| {
            if config
                .launcher_value("gamesUsesSteamRuntime")
                .map(|v| v == "1")
                .unwrap_or(true)
            {
                "true".to_string()
            } else {
                "false".to_string()
            }
        });
        let on = |b: bool| if b { "on" } else { "off" };
        let mut h = String::new();
        h.push_str("CorkyTux debug log\n");
        h.push_str("=========================================\n");
        h.push_str(&format!("Game: {}\n", game_name));
        h.push_str(&format!("Executable: {}\n", exe));
        h.push_str(&format!("Prefix: {}\n", prefix_disp));
        if is_appimage {
            h.push_str(&format!("Runner: AppImage (native, {})\n", executor));
        } else if is_rpg {
            h.push_str("Runner: RPG Maker (box-rpg, native)\n");
        } else if is_emu {
            h.push_str(&format!("Runner: emulator ({})\n", get("executor")));
        } else {
            h.push_str(&format!(
                "Proton: {}{}\n",
                proton_name,
                if wine_ver.is_empty() { String::new() } else { format!(" ({})", wine_ver) }
            ));
        }
        if is_rpg {
            h.push_str(&format!(
                "Options: engine={} runtime={} gamemode={} mangohud={}\n",
                get("rpgengine"),
                {
                    let r = get("rpgruntime");
                    if r.is_empty() { "auto".to_string() } else { r }
                },
                on(flag("gamemode")),
                on(flag("mangohud")),
            ));
        } else if is_appimage {
            h.push_str(&format!(
                "Options: args={} env={} gamemode={} mangohud={}\n",
                get("launchargs"),
                get("environment"),
                on(flag("gamemode")),
                on(flag("mangohud")),
            ));
        } else {
            h.push_str(&format!(
                "Options: steam-runtime={} umu={} gamemode={} mangohud={} wined3d={} wayland={}\n",
                on(runtime == "true"),
                on(flag("useumu")),
                on(flag("gamemode")),
                on(flag("mangohud")),
                on(flag("usewined3d")),
                {
                    let w = get("nativewayland");
                    if w.is_empty() { "default".to_string() } else { w }
                },
            ));
        }
        h.push_str("--- System ---\n");
        h.push_str(&format!("OS: {}\n", sys_distro()));
        h.push_str(&format!("Kernel: {}\n", sys_kernel()));
        h.push_str(&format!("CPU: {}\n", sys_cpu()));
        h.push_str(&format!("Memory: {}\n", sys_mem()));
        h.push_str(&format!("GPU: {}\n", sys_gpu()));
        h.push_str(&format!("Display: {}\n", sys_display()));
        h.push_str("--- How to read this log ---\n");
        if is_rpg {
            h.push_str("box-rpg launches are fire-and-forget: no exit code is tracked.\n");
            h.push_str("NW.js console errors and EasyRPG interpreter messages appear above.\n");
        } else if is_appimage {
            h.push_str("Native AppImage output: Qt/GTK warnings are usually harmless.\n");
            h.push_str("FUSE/dwarfs mount lines come from the AppImage runtime.\n");
            h.push_str("Real failures usually show as tracebacks or missing-library errors.\n");
        } else {
            h.push_str("ProtonFixes INFO lines are normal Proton setup checks, not errors.\n");
            h.push_str("\"fsync: up and running\" means sync primitives are OK.\n");
            h.push_str("FSR4/upscaler_files WARNs are a harmless upscaler-cache note.\n");
            h.push_str("ALSA dmix/dsnoop errors are harmless audio-probe noise (game uses PipeWire/Pulse).\n");
            h.push_str("ProtonFixes 'unit test' WARN only means no Steam game id was matched.\n");
            h.push_str("Real game failures usually show as err: lines (wine) or tracebacks.\n");
        }
        h.push_str("=========================================\n");
        h
    }

    pub fn log_path(game_name: &str) -> PathBuf {
        let safe: String = game_name
            .chars()
            .map(|c| if c.is_alphanumeric() || c == '-' || c == '_' { c } else { '_' })
            .collect();
        let dir = home_dir()
            .unwrap_or_default()
            .join(".local")
            .join("share")
            .join("CorkyTux")
            .join("logs");
        fs::create_dir_all(&dir).ok();
        dir.join(format!("{}.log", safe))
    }

    pub fn read_log(game_name: &str) -> String {
        let path = Self::log_path(game_name);
        let content = fs::read_to_string(&path).unwrap_or_default();
        // Keep the tail so the UI never chokes on huge logs
        const MAX: usize = 200_000;
        if content.len() > MAX {
            content[content.len() - MAX..].to_string()
        } else {
            content
        }
    }

    fn attach_log_file(cmd: &mut Command, game_name: &str) {
        if let Ok(f) = fs::File::create(Self::log_path(game_name)) {
            if let Ok(f2) = f.try_clone() {
                cmd.stdout(Stdio::from(f));
                cmd.stderr(Stdio::from(f2));
            }
        }
    }

    pub fn stop_game(&self) -> Result<(String, u64), String> {
        let mut child = self.imp().running_child.borrow_mut().take();
        if child.is_none() && self.imp().session_game.borrow().is_empty() {
            return Ok((String::new(), 0));
        }
        if child.is_none() && !self.imp().session_appimage.get()
            && self.imp().session_prefix.borrow().is_empty()
            && self.imp().session_proton_dir.borrow().is_empty()
            && self.imp().session_watch.borrow().is_empty()
        {
            return Ok((String::new(), 0));
        }
            // C++ parity: wineserver -k with WINEPREFIX kills the whole wine
            // tree for this prefix; killing the wrapper alone leaves the
            // game running.
            let prefix = self.imp().session_prefix.borrow().clone();
            let proton_dir = self.imp().session_proton_dir.borrow().clone();
            if !prefix.is_empty() && !proton_dir.is_empty() {
                if let Some(ws) = find_wineserver(Path::new(&proton_dir)) {
                    let _ = Command::new("timeout")
                        .args(["5", ws.to_str().unwrap_or(""), "-k"])
                        .env("WINEPREFIX", &prefix)
                        .stdout(Stdio::null())
                        .stderr(Stdio::null())
                        .status();
                }
            }
            // AppImages mount (dwarfs/fuse) + fork helpers beside the direct
            // child: kill everything whose command line is the AppImage,
            // then lazy-unmount its stale FUSE mount, if any.
            if self.imp().session_appimage.get() {
                let game = self.imp().session_game.borrow().clone();
                if !game.is_empty() {
                    if let Some(exe) = super::ConfigManager::new().game_value(&game, "Executable") {
                        Self::stop_appimage_tree(&exe);
                    }
                }
                self.imp().session_appimage.set(false);
            }
            // Best-effort: the wrapper may already have exited once
            // wineserver -k tore the tree down; time must still bank.
            // Adopted sessions (no child) fall back to the watch pattern.
            if let Some(mut c) = child {
                let _ = c.kill();
            } else {
                let watch = self.imp().session_watch.borrow().clone();
                if !watch.trim().is_empty() {
                    let _ = Command::new(Self::tool_path("pkill"))
                        .args(["-9", "-f", &watch])
                        .stdout(Stdio::null())
                        .stderr(Stdio::null())
                        .status();
                }
            }
            self.imp().game_running.set(false);
            let secs = self
                .imp()
                .session_start
                .borrow_mut()
                .take()
                .map(|t| t.elapsed().as_secs())
                .unwrap_or(0);
            let game = std::mem::take(&mut *self.imp().session_game.borrow_mut());
            *self.imp().session_prefix.borrow_mut() = String::new();
            *self.imp().session_proton_dir.borrow_mut() = String::new();
            self.imp().session_appimage.set(false);
            *self.imp().session_watch.borrow_mut() = String::new();
            Ok((game, secs))
    }

    pub fn is_game_running(&self) -> bool {
        self.imp().game_running.get()
    }

    pub fn session_game_name(&self) -> String {
        self.imp().session_game.borrow().clone()
    }

    fn begin_session(&self, game_name: &str) {
        *self.imp().session_game.borrow_mut() = game_name.to_string();
        *self.imp().session_start.borrow_mut() = Some(std::time::Instant::now());
        self.imp().game_running.set(true);
        self.imp().session_appimage.set(false);
        *self.imp().session_watch.borrow_mut() = String::new();
    }

    fn watch_alive(pattern: &str) -> bool {
        if pattern.trim().is_empty() {
            return false;
        }
        if let Ok(out) = Command::new(Self::tool_path("pgrep"))
            .arg("-f")
            .arg(pattern)
            .output()
        {
            if out.status.success() && !String::from_utf8_lossy(&out.stdout).trim().is_empty() {
                return true;
            }
        }
        false
    }

    pub(crate) fn adopt_session(
        &self,
        game_name: &str,
        prefix: &str,
        proton_dir: &str,
        appimage: bool,
        watch: &str,
    ) {
        *self.imp().session_game.borrow_mut() = game_name.to_string();
        *self.imp().session_start.borrow_mut() = Some(std::time::Instant::now());
        self.imp().game_running.set(true);
        *self.imp().session_prefix.borrow_mut() = prefix.to_string();
        *self.imp().session_proton_dir.borrow_mut() = proton_dir.to_string();
        self.imp().session_appimage.set(appimage);
        *self.imp().session_watch.borrow_mut() = watch.to_string();
        eprintln!("[session] adopted already-running game: {}", game_name);
    }

    pub(crate) fn detect_running_sessions(&self) {
        if self.imp().game_running.get() {
            return;
        }
        let config = super::ConfigManager::new();
        for name in config.game_names() {
            let sec = config.game_section(&name);
            let get = |k: &str| sec.get(k).cloned().unwrap_or_default();
            let exe = get("executable");
            if exe.trim().is_empty() {
                continue;
            }
            let executor = get("executor");
            let source = get("source");
            if executor == "appimage-launcher" || source.eq_ignore_ascii_case("appimage") {
                if Self::appimage_tree_alive(&exe) {
                    self.adopt_session(&name, "", "", true, "");
                    return;
                }
                continue;
            }
            if !executor.is_empty() {
                let pattern = Self::appimage_ere_escape(&exe);
                if Self::watch_alive(&pattern) {
                    self.adopt_session(&name, "", "", false, &pattern);
                    return;
                }
                continue;
            }
            let pattern = Self::appimage_ere_escape(&exe);
            if Self::watch_alive(&pattern) {
                let prefix = self.real_prefix_for(&name).display().to_string();
                let proton_dir = config
                    .game_value(&name, "Proton")
                    .and_then(|w| self.resolve_proton(&w).ok())
                    .map(|(_, p)| p.display().to_string())
                    .unwrap_or_default();
                self.adopt_session(&name, &prefix, &proton_dir, false, &pattern);
                return;
            }
        }
    }

    /// C++ onGameFinished parity: call periodically; returns (game, seconds)
    /// once when the running child exits so time can be banked.
    pub fn check_finished(&self) -> Option<(String, u64)> {
        let code: Option<i32> = {
            let mut slot = self.imp().running_child.borrow_mut();
            match slot.as_mut() {
                Some(child) => match child.try_wait() {
                    Ok(Some(st)) => Some(st.code().unwrap_or(-1)),
                    _ => None,
                },
                None => None,
            }
        };
        if self.imp().session_appimage.get() {
            let game = self.imp().session_game.borrow().clone();
            if !game.is_empty() {
                let exe = super::ConfigManager::new()
                    .game_value(&game, "Executable")
                    .unwrap_or_default();
                if Self::appimage_tree_alive(&exe) {
                    if code.is_some() {
                        *self.imp().running_child.borrow_mut() = None;
                    }
                    self.imp().game_running.set(true);
                    return None;
                }
            }
        }
        let code = match code {
            Some(c) => c,
            None => {
                if self.imp().session_appimage.get() {
                    let young = self
                        .imp()
                        .session_start
                        .borrow()
                        .map(|t| t.elapsed().as_secs() < 8)
                        .unwrap_or(false);
                    if young {
                        return None;
                    }
                    -1
                } else {
                    let watch = self.imp().session_watch.borrow().clone();
                    if watch.trim().is_empty() {
                        return None;
                    }
                    if Self::watch_alive(&watch) {
                        return None;
                    }
                    -1
                }
            }
        };
        *self.imp().running_child.borrow_mut() = None;
        self.imp().game_running.set(false);
        let was_appimage = self.imp().session_appimage.get();
        self.imp().session_appimage.set(false);
        let secs = self
            .imp()
            .session_start
            .borrow_mut()
            .take()
            .map(|t| t.elapsed().as_secs())
            .unwrap_or(0);
        let game = std::mem::take(&mut *self.imp().session_game.borrow_mut());
        *self.imp().session_prefix.borrow_mut() = String::new();
        *self.imp().session_proton_dir.borrow_mut() = String::new();
        if was_appimage && !game.is_empty() {
            if let Some(exe) = super::ConfigManager::new().game_value(&game, "Executable") {
                Self::stop_appimage_tree(&exe);
            }
        }
        *self.imp().session_watch.borrow_mut() = String::new();
        if game.is_empty() {
            None
        } else {
            // Lutris parity: stamp how the process ended into its own log.
            let path = Self::log_path(&game);
            if let Ok(mut f) = std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(&path)
            {
                use std::io::Write;
                let _ = writeln!(f, "\n[CorkyTux] Exit with return code {}", code);
            }
            Some((game, secs))
        }
    }

    pub fn run_emulator_game(
        &self,
        game_name: &str,
        game: &std::collections::HashMap<String, String>,
    ) -> Result<(), String> {
        let plugins = super::PluginManager::new();
        let executor = game.get("executor").cloned().unwrap_or_default();
        let rom = game.get("executable").cloned().unwrap_or_default();
        if rom.is_empty() {
            return Err("No ROM set".into());
        }
        let emu_path = plugins
            .emulator_path(&executor)
            .ok_or_else(|| format!("Emulator '{}' not found", executor))?;

        let mut cmd = Command::new(&emu_path);
        // Fullscreen flag from emu settings (common convention: -f / --fullscreen)
        if game.get("emu_fullscreen").map(|v| v == "true").unwrap_or(false) {
            cmd.arg("--fullscreen");
        }
        cmd.arg(&rom);
        let launch_args = game.get("launchargs").cloned().unwrap_or_default();
        if !launch_args.is_empty() {
            for arg in launch_args.split_whitespace() {
                cmd.arg(arg);
            }
        }
        // GameMode / MangoHud wrappers (same as Proton path)
        let mut wrapped = cmd;
        if game.get("gamemode").map(|v| v == "true").unwrap_or(false) {
            let mut gm = Command::new("gamemoderun");
            gm.arg(wrapped.get_program());
            for arg in wrapped.get_args() {
                gm.arg(arg);
            }
            for (key, val) in wrapped.get_envs() {
                if let Some(v) = val {
                    gm.env(key, v);
                }
            }
            wrapped = gm;
        }
        // Paridad con run_game / ProtonManager.cpp: MANGOHUD=1 por env var,
        // no wrapper de proceso (evita romper el orden gamemoderun -> mangohud).
        if game.get("mangohud").map(|v| v == "true").unwrap_or(false) {
            wrapped.env("MANGOHUD", "1");
        }

        Self::attach_log_file(&mut wrapped, game_name);
        let child = wrapped
            .spawn()
            .map_err(|e| format!("Failed to launch emulator: {}", e))?;
        *self.imp().running_child.borrow_mut() = Some(child);
        // Emulators are single native processes: direct kill suffices.
        *self.imp().session_prefix.borrow_mut() = String::new();
        *self.imp().session_proton_dir.borrow_mut() = String::new();
        self.begin_session(game_name);
        Ok(())
    }

    pub fn run_appimage_game(
        &self,
        game_name: &str,
        game: &std::collections::HashMap<String, String>,
    ) -> Result<(), String> {
        let exe = game.get("executable").cloned().unwrap_or_default();
        if exe.trim().is_empty() {
            return Err("No AppImage set".into());
        }
        let expanded = Self::appimage_expand(&exe);
        let path = std::path::Path::new(&expanded);
        if !path.is_file() {
            return Err(format!("AppImage not found: {}", expanded));
        }
        if let Ok(out) = Command::new(Self::tool_path("pgrep"))
            .arg("-f")
            .arg(format!("^{}( |$)", Self::appimage_ere_escape(&expanded)))
            .output()
        {
            if out.status.success() && !String::from_utf8_lossy(&out.stdout).trim().is_empty() {
                return Err("Already running: close the app first, then press Play".into());
            }
        }
        if Self::appimage_tree_alive(&expanded) {
            Self::stop_appimage_tree(&expanded);
            std::thread::sleep(std::time::Duration::from_millis(800));
            if Self::appimage_tree_alive(&expanded) {
                return Err("Could not close the previous instance: close the app manually".into());
            }
        }
        Self::appimage_unmount_stale(&expanded);
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            if let Ok(meta) = std::fs::metadata(path) {
                let mut perm = meta.permissions();
                if perm.mode() & 0o111 == 0 {
                    perm.set_mode(perm.mode() | 0o755);
                    std::fs::set_permissions(path, perm).ok();
                }
            }
        }
        let setsid_bin = Self::tool_path("setsid");
        let has_setsid = Command::new(&setsid_bin)
            .arg("--version")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .map(|s| s.success())
            .unwrap_or(false);
        let mut cmd = if has_setsid {
            let mut c = Command::new(&setsid_bin);
            c.arg("--wait");
            c.arg(path);
            c
        } else {
            Command::new(path)
        };
        let launch_args = game.get("launchargs").cloned().unwrap_or_default();
        if !launch_args.trim().is_empty() {
            for arg in split_quoted_args(&launch_args) {
                cmd.arg(arg);
            }
        }
        let args_after = game.get("argsafter").cloned().unwrap_or_default();
        if !args_after.trim().is_empty() {
            for arg in split_quoted_args(&args_after) {
                cmd.arg(arg);
            }
        }
        let environment = game.get("environment").cloned().unwrap_or_default();
        if !environment.trim().is_empty() {
            for part in environment.split_whitespace() {
                if let Some((key, val)) = part.split_once('=') {
                    if !key.is_empty() {
                        cmd.env(key, val);
                    }
                }
            }
        }
        if game.get("gamemode").map(|v| v == "true").unwrap_or(false) {
            let gm_bin = Self::tool_path("gamemoderun");
            if gm_bin.is_file() {
                let mut gm = Command::new(&gm_bin);
                gm.arg(cmd.get_program());
                for arg in cmd.get_args() {
                    gm.arg(arg);
                }
                for (key, val) in cmd.get_envs() {
                    if let Some(v) = val {
                        gm.env(key, v);
                    }
                }
                cmd = gm;
            }
        }
        if game.get("mangohud").map(|v| v == "true").unwrap_or(false) {
            cmd.env("MANGOHUD", "1");
        }
        if let Some(parent) = path.parent() {
            if !parent.as_os_str().is_empty() {
                cmd.current_dir(parent);
            }
        }
        cmd.stdin(Stdio::null());
        Self::attach_log_file(&mut cmd, game_name);
        let child = cmd.spawn().map_err(|e| format!("Failed to launch AppImage: {}", e))?;
        *self.imp().running_child.borrow_mut() = Some(child);
        *self.imp().session_prefix.borrow_mut() = String::new();
        *self.imp().session_proton_dir.borrow_mut() = String::new();
        self.begin_session(game_name);
        self.imp().session_appimage.set(has_setsid);
        Ok(())
    }

    fn tool_path(name: &str) -> std::path::PathBuf {
        for dir in ["/usr/sbin", "/usr/bin", "/bin", "/sbin"] {
            let p = std::path::PathBuf::from(dir).join(name);
            if p.is_file() {
                return p;
            }
        }
        std::path::PathBuf::from(name)
    }

    fn appimage_ere_escape(s: &str) -> String {
        let mut pattern = String::new();
        for c in s.chars() {
            if ".^$*+?()[]{}|\\".contains(c) {
                pattern.push('\\');
            }
            pattern.push(c);
        }
        pattern
    }

    fn appimage_expand(exe: &str) -> String {
        if exe.starts_with("~/") || exe == "~" {
            if let Ok(home) = std::env::var("HOME") {
                exe.replacen("~", &home, 1)
            } else {
                exe.to_string()
            }
        } else {
            exe.to_string()
        }
    }

    fn appimage_mounts(expanded: &str) -> Vec<String> {
        let mut out = Vec::new();
        if let Ok(info) = std::fs::read_to_string("/proc/self/mountinfo") {
            for line in info.lines() {
                let (left, right) = match line.split_once(" - ") {
                    Some(p) => p,
                    None => continue,
                };
                let left_fields: Vec<&str> = left.split_whitespace().collect();
                let right_fields: Vec<&str> = right.split_whitespace().collect();
                if left_fields.len() < 5 || right_fields.len() < 2 {
                    continue;
                }
                if right_fields[1] == expanded {
                    out.push(left_fields[4].to_string());
                }
            }
        }
        out
    }

    fn appimage_pids(pattern: &str) -> Vec<String> {
        let mut out = Vec::new();
        if let Ok(res) = Command::new(Self::tool_path("pgrep"))
            .arg("-f")
            .arg(pattern)
            .output()
        {
            if res.status.success() {
                for line in String::from_utf8_lossy(&res.stdout).lines() {
                    if let Some(pid) = line.split_whitespace().next() {
                        if !pid.is_empty() && pid.chars().all(|c| c.is_ascii_digit()) {
                            out.push(pid.to_string());
                        }
                    }
                }
            }
        }
        out
    }

    fn appimage_tree_alive(exe: &str) -> bool {
        let expanded = Self::appimage_expand(exe);
        if expanded.trim().is_empty() {
            return false;
        }
        if let Ok(out) = Command::new(Self::tool_path("pgrep"))
            .arg("-f")
            .arg(format!("^{}( |$)", Self::appimage_ere_escape(&expanded)))
            .output()
        {
            if out.status.success() && !String::from_utf8_lossy(&out.stdout).trim().is_empty() {
                return true;
            }
        }
        for mp in Self::appimage_mounts(&expanded) {
            if let Ok(out) = Command::new(Self::tool_path("pgrep"))
                .arg("-f")
                .arg(format!("^{}/", Self::appimage_ere_escape(&mp)))
                .output()
            {
                if out.status.success() && !String::from_utf8_lossy(&out.stdout).trim().is_empty() {
                    return true;
                }
            }
        }
        false
    }

    fn appimage_unmount_stale(expanded: &str) {
        for mp in Self::appimage_mounts(expanded) {
            let _ = Command::new(Self::tool_path("umount"))
                .args(["-l", &mp])
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status();
        }
    }

    fn stop_appimage_tree(exe: &str) {
        let expanded = Self::appimage_expand(exe);
        if expanded.trim().is_empty() {
            return;
        }
        let pattern = Self::appimage_ere_escape(&expanded);
        for pid in Self::appimage_pids(&format!("^{}( |$)", pattern)) {
            let _ = Command::new(Self::tool_path("pkill"))
                .args(["-9", "-g", &pid])
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status();
        }
        let _ = Command::new(Self::tool_path("pkill"))
            .args(["-9", "-f", &pattern])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();
        for mp in Self::appimage_mounts(&expanded) {
            let _ = Command::new(Self::tool_path("pkill"))
                .args(["-9", "-f", &format!("^{}/", Self::appimage_ere_escape(&mp))])
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status();
        }
        Self::appimage_unmount_stale(&expanded);
    }

    pub fn run_rpg_game(
        &self,
        game_name: &str,
        game: &std::collections::HashMap<String, String>,
    ) -> Result<(), String> {
        let dir = game.get("executable").cloned().unwrap_or_default();
        if dir.trim().is_empty() {
            return Err("No game folder set".into());
        }
        if !super::external::RpgMakerManager::available() {
            return Err("RPG Maker plugin not installed".into());
        }
        let runtime = game.get("rpgruntime").cloned().unwrap_or_default();
        super::external::RpgMakerManager::run_with_runtime(dir.trim(), runtime.trim())
            .map(|_| ())
            .map_err(|e| format!("RPG launch failed: {}", e))
    }

    pub fn run_game_debug(&self, game_name: &str) -> Result<(), String> {
        let config = super::ConfigManager::new();
        let game = config.game_section(game_name);
        let executable = game.get("executable")
            .cloned()
            .unwrap_or_default();
        if executable.is_empty() {
            return Err("No executable set".into());
        }
        let wanted_proton = game.get("proton").cloned().unwrap_or_default();
        let prefix = self.ensure_individual_prefix(game_name);
        let main_path = game.get("mainpath").cloned().unwrap_or_default();
        let use_shared_prefix = game.get("usesharedprefix").map(|v| v == "true").unwrap_or(false);
        let shared_prefix_name = game.get("sharedprefix").cloned().unwrap_or_default();
        let (proton_name, proton_path) = self.resolve_proton(&wanted_proton)?;

        let proton_bin = find_proton_binary(&proton_path)
            .ok_or("Proton binary not found")?;

        // C++ parity: shared prefix is keyed by proton; the game's
        // SharedPrefix name overrides when set.
        let shared_key = if !shared_prefix_name.is_empty() {
            shared_prefix_name.clone()
        } else {
            proton_name.clone()
        };
        let actual_prefix = if use_shared_prefix {
            config.shared_prefix_path(&shared_key)
                .unwrap_or_else(|| prefix.clone())
        } else {
            prefix.clone()
        };

        // Debug runs straight into the log file (tailed by the LogModal),
        // no xterm needed and output is never lost.
        let real_prefix = actual_prefix.join("pfx");
        let mut cmd = Command::new(&proton_bin);
        cmd.env("WINEPREFIX", &real_prefix);
        cmd.env("WINEDEBUG", "1");
        // Proton appends /pfx internally: compat dir is actual_prefix.
        cmd.env("STEAM_COMPAT_DATA_PATH", &actual_prefix);
        cmd.env("STEAM_COMPAT_CLIENT_INSTALL_PATH", self.steam_client_path());
        if !main_path.is_empty() {
            cmd.env("STEAM_COMPAT_INSTALL_PATH", &main_path);
        }
        cmd.arg("waitforexitandrun");
        cmd.arg(&executable);

        Self::attach_log_file(&mut cmd, game_name);
        let child = cmd
            .spawn()
            .map_err(|e| format!("Failed to launch debug: {}", e))?;
        *self.imp().running_child.borrow_mut() = Some(child);
        *self.imp().session_prefix.borrow_mut() = real_prefix.display().to_string();
        *self.imp().session_proton_dir.borrow_mut() = proton_path.display().to_string();
        self.begin_session(game_name);
        Ok(())
    }

    /// C++ runCustomExe parity: resolves the game's proton + prefix internally.
    pub fn run_custom_exe(&self, game_name: &str, executable: &str) -> Result<(), String> {
        let config = super::ConfigManager::new();
        let wanted = config.game_value(game_name, "Proton").unwrap_or_default();
        let (_, proton_path) = self.resolve_proton(&wanted)?;

        let proton_bin = find_proton_binary(&proton_path)
            .ok_or("Proton binary not found")?;

        let prefix_path = self.ensure_individual_prefix(game_name);
        // Same layout as run_game: compat dir + real prefix at /pfx.
        let real_prefix = prefix_path.join("pfx");

        let mut cmd = Command::new(&proton_bin);
        cmd.env("STEAM_COMPAT_DATA_PATH", &prefix_path);
        cmd.env("WINEPREFIX", &real_prefix);
        // Modern Proton (like run_game) requires the Steam client path and
        // an app id; exes also need their own dir as cwd for local DLLs.
        cmd.env("STEAM_COMPAT_CLIENT_INSTALL_PATH", self.steam_client_path());
        cmd.env("SteamAppId", "0");
        cmd.arg("waitforexitandrun");
        cmd.arg(executable);
        if let Some(parent) = Path::new(executable).parent() {
            if !parent.as_os_str().is_empty() {
                cmd.current_dir(parent);
            }
        }

        let child = cmd
            .spawn()
            .map_err(|e| format!("Failed to launch custom exe: {}", e))?;
        *self.imp().running_child.borrow_mut() = Some(child);
        *self.imp().session_prefix.borrow_mut() = real_prefix.display().to_string();
        *self.imp().session_proton_dir.borrow_mut() = proton_path.display().to_string();
        self.begin_session(game_name);
        Ok(())
    }

    pub fn run_wine_tool(&self, game_name: &str, tool: &str) -> Result<(), String> {
        let config = super::ConfigManager::new();
        let wanted = config
            .game_value(game_name, "Proton")
            .unwrap_or_default();
        let (proton_name, _) = self.resolve_proton(&wanted)?;
        let prefix = self.ensure_individual_prefix(game_name);

        // C++ parity: los tools de Wine (winecfg/taskmgr/control/explorer/cmd)
        // se ejecutan con el binario wine/wine64 real dentro del Proton,
        // NO con el script "proton" (que no soporta esos subcomandos).
        // Se busca en el dir instalado detectado + todos los proton paths.
        let mut candidate_dirs: Vec<PathBuf> = self
            .installed_proton_entries()
            .iter()
            .filter(|p| p.name == proton_name)
            .map(|p| p.path.clone())
            .collect();
        for dir in self.proton_paths() {
            let d = dir.join(&proton_name);
            if !candidate_dirs.contains(&d) {
                candidate_dirs.push(d);
            }
        }
        let mut wine_bin: Option<PathBuf> = None;
        'outer: for dir in &candidate_dirs {
            for rel in ["files/bin/wine64", "files/bin/wine", "dist/bin/wine64"] {
                let cand = dir.join(rel);
                if cand.exists() {
                    wine_bin = Some(cand);
                    break 'outer;
                }
            }
        }
        let wine_bin = wine_bin.ok_or("No Wine binary found")?;

        // Wine tools run inside the real prefix (compat dir + /pfx).
        let real_prefix = prefix.join("pfx");
        let mut cmd = Command::new(&wine_bin);
        cmd.env("WINEPREFIX", &real_prefix);
        cmd.arg(tool);

        let child = cmd
            .spawn()
            .map_err(|e| format!("Failed to run wine tool: {}", e))?;
        *self.imp().running_child.borrow_mut() = Some(child);
        *self.imp().session_prefix.borrow_mut() = real_prefix.display().to_string();
        if let Some(dir) = candidate_dirs.first() {
            *self.imp().session_proton_dir.borrow_mut() = dir.display().to_string();
        }
        self.begin_session(game_name);
        Ok(())
    }

    pub fn release_source_url(source: &str) -> Result<&'static str, String> {
        match source {
            "ge" => Ok("https://api.github.com/repos/GloriousEggroll/proton-ge-custom/releases"),
            "cachyos" => Ok("https://api.github.com/repos/CachyOS/Proton-CachyOS/releases"),
            _ => Err("Unknown source".into()),
        }
    }

    fn releases_cache_path(source: &str) -> PathBuf {
        home_dir()
            .unwrap_or_default()
            .join(".cache")
            .join("CorkyTux")
            .join(format!("proton-releases-{}.json", source))
    }

    /// Cached release list: (entries, fresh?). GitHub allows 60 API
    /// calls/hour unauthenticated, so fresh cache (< 1h) wins and any
    /// cache at all is used as fallback when rate-limited.
    fn read_releases_cache(source: &str, max_age_secs: u64) -> Option<Vec<(String, String)>> {
        let path = Self::releases_cache_path(source);
        let meta = fs::metadata(&path).ok()?;
        let age = std::time::SystemTime::now()
            .duration_since(meta.modified().ok()?)
            .ok()?
            .as_secs();
        if age > max_age_secs {
            return None;
        }
        let content = fs::read_to_string(&path).ok()?;
        let v: serde_json::Value = serde_json::from_str(&content).ok()?;
        let list: Vec<(String, String)> = v
            .as_array()?
            .iter()
            .filter_map(|e| {
                Some((
                    e.get(0)?.as_str()?.to_string(),
                    e.get(1)?.as_str()?.to_string(),
                ))
            })
            .collect();
        if list.is_empty() {
            return None;
        }
        Some(list)
    }

    fn write_releases_cache(source: &str, releases: &[(String, String)]) {
        let path = Self::releases_cache_path(source);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).ok();
        }
        let arr: Vec<Vec<&str>> = releases
            .iter()
            .map(|(t, u)| vec![t.as_str(), u.as_str()])
            .collect();
        if let Ok(s) = serde_json::to_string(&arr) {
            fs::write(&path, s).ok();
        }
    }

    pub fn fetch_releases(source: &str) -> Result<Vec<(String, String)>, String> {
        let url = Self::release_source_url(source)?;
        if let Some(cached) = Self::read_releases_cache(source, 3600) {
            return Ok(cached);
        }
        let client = reqwest::blocking::Client::builder()
            .user_agent("CorkyTux")
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .map_err(|e| format!("HTTP client failed: {}", e))?;
        let resp = client
            .get(url)
            .header("Accept", "application/vnd.github.v3+json")
            .send()
            .map_err(|e| format!("Request failed: {}", e))?;
        if resp.status().as_u16() == 403 {
            // Rate-limited: serve any cache, however stale, before failing.
            if let Some(cached) = Self::read_releases_cache(source, u64::MAX) {
                return Ok(cached);
            }
            let reset = resp
                .headers()
                .get("x-ratelimit-reset")
                .and_then(|v| v.to_str().ok())
                .and_then(|v| v.parse::<i64>().ok())
                .map(|ts| {
                    let now = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .map(|d| d.as_secs() as i64)
                        .unwrap_or(0);
                    let wait = (ts - now).max(0);
                    format!(" — retry in ~{} min", (wait + 59) / 60)
                })
                .unwrap_or_default();
            return Err(format!("GitHub API rate limit exceeded (60/hour){}.", reset));
        }
        if !resp.status().is_success() {
            return Err(format!("GitHub API error: {}", resp.status()));
        }
        let json: serde_json::Value = resp
            .json()
            .map_err(|e| format!("Invalid JSON: {}", e))?;
        let mut releases = Vec::new();
        for rel in json.as_array().cloned().unwrap_or_default() {
            let tag = rel
                .get("tag_name")
                .and_then(|v| v.as_str())
                .unwrap_or_default()
                .to_string();
            if tag.is_empty() {
                continue;
            }
            let mut dl: Option<String> = None;
            for asset in rel
                .get("assets")
                .and_then(|v| v.as_array())
                .cloned()
                .unwrap_or_default()
            {
                let name = asset
                    .get("name")
                    .and_then(|v| v.as_str())
                    .unwrap_or_default();
                let url = asset
                    .get("browser_download_url")
                    .and_then(|v| v.as_str())
                    .unwrap_or_default();
                if (name.ends_with(".tar.gz") || name.ends_with(".tar.xz"))
                    && !url.is_empty()
                    && !Self::is_foreign_arch(name)
                {
                    dl = Some(url.to_string());
                    break;
                }
            }
            if let Some(download_url) = dl {
                releases.push((tag, download_url));
            }
        }
        if releases.is_empty() {
            return Err("No downloadable releases found".into());
        }
        Self::write_releases_cache(source, &releases);
        Ok(releases)
    }

    pub fn default_download_dir(&self) -> PathBuf {
        if let Some(first) = self.proton_paths().first() {
            return first.clone();
        }
        let p = home_dir()
            .unwrap_or_default()
            .join(".local")
            .join("share")
            .join("Steam")
            .join("compatibilitytools.d");
        fs::create_dir_all(&p).ok();
        p
    }

    pub fn format_speed(bps: f64) -> String {
        if bps < 1024.0 {
            format!("{:.0} B/s", bps)
        } else if bps < 1024.0 * 1024.0 {
            format!("{:.1} KB/s", bps / 1024.0)
        } else {
            format!("{:.1} MB/s", bps / (1024.0 * 1024.0))
        }
    }

    pub fn download_proton(
        name: &str,
        url: &str,
        target_dir: &Path,
        mut on_progress: impl FnMut(f64, f64),
    ) -> Result<PathBuf, String> {
        fs::create_dir_all(target_dir).map_err(|e| format!("Cannot create dir: {}", e))?;
        let archive = target_dir.join(format!("{}.tar.gz", name));
        let client = reqwest::blocking::Client::builder()
            .user_agent("CorkyTux")
            .timeout(std::time::Duration::from_secs(3600))
            .build()
            .map_err(|e| format!("HTTP client failed: {}", e))?;
        let mut resp = client
            .get(url)
            .send()
            .map_err(|e| format!("Download failed: {}", e))?;
        if !resp.status().is_success() {
            return Err(format!("Download HTTP error: {}", resp.status()));
        }
        let total = resp.content_length().unwrap_or(0);
        let mut file =
            fs::File::create(&archive).map_err(|e| format!("Cannot write file: {}", e))?;
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
        {
            let secs = start.elapsed().as_secs_f64().max(0.001);
            on_progress(1.0, downloaded as f64 / secs);
        }
        let status = Command::new("tar")
            .args(["-xzf", archive.to_str().unwrap_or(""), "-C", target_dir.to_str().unwrap_or("")])
            .status()
            .map_err(|e| format!("Extract failed: {}", e))?;
        fs::remove_file(&archive).ok();
        if !status.success() {
            return Err("Extract failed".into());
        }
        Ok(target_dir.to_path_buf())
    }

    pub fn remove_proton(&self, name: &str) -> Result<(), String> {
        for dir in self.proton_paths() {
            let target = dir.join(name);
            if target.exists() {
                fs::remove_dir_all(&target).map_err(|e| e.to_string())?;
                self.refresh_installed();
                // Games pointing at the deleted build fall back to
                // GE-Proton Latest (empty Proton field), same for the
                // global default.
                let config = super::ConfigManager::new();
                for game in config.game_names() {
                    if config.game_value(&game, "Proton").as_deref() == Some(name) {
                        config.set_game_value(&game, "Proton", "");
                    }
                }
                if config.launcher_value("defaultProton").as_deref() == Some(name) {
                    config.set_launcher_value("defaultProton", "");
                }
                return Ok(());
            }
        }
        Err("Proton not found".into())
    }

    pub fn ensure_prefix_path(&self, game_name: &str) -> Result<PathBuf, String> {
        let prefix = self.prefix_path(game_name);
        fs::create_dir_all(&prefix).map_err(|e| format!("Failed to create prefix: {}", e))?;
        Ok(prefix)
    }

    /// The wine prefix a game REALLY runs in: shared or individual compat
    /// dir, plus /pfx (Proton appends it); plain compat dir for umu.
    /// Mirrors run_game's resolution for scans/tools.
    pub fn real_prefix_for(&self, game_name: &str) -> PathBuf {
        let config = super::ConfigManager::new();
        let compat = match config.game_value(game_name, "PrefixPath") {
            Some(p) if !p.trim().is_empty() => PathBuf::from(p.trim()),
            _ => config.prefixes_dir().join(game_name),
        };
        let use_shared = config.game_value(game_name, "UseSharedPrefix").as_deref() == Some("true");
        let base = if use_shared {
            let sp = config.game_value(game_name, "SharedPrefix").unwrap_or_default();
            let key = if !sp.is_empty() {
                sp
            } else {
                config.game_value(game_name, "Proton").unwrap_or_default()
            };
            config.shared_prefix_path(&key).unwrap_or(compat)
        } else {
            compat
        };
        if config.game_value(game_name, "UseUmu").as_deref() == Some("true") {
            base
        } else {
            base.join("pfx")
        }
    }

    /// If the game has no prefix configured (empty PrefixPath), assign and
    /// create its individual default (prefixesDir/gameName). Emulator games
    /// never reach here (they return before any prefix handling).
    pub fn ensure_individual_prefix(&self, game_name: &str) -> PathBuf {
        let config = super::ConfigManager::new();
        let cur = config
            .game_value(game_name, "PrefixPath")
            .unwrap_or_default();
        if !cur.trim().is_empty() {
            let p = PathBuf::from(cur.trim());
            fs::create_dir_all(&p).ok();
            return p;
        }
        let p = config.prefixes_dir().join(game_name);
        fs::create_dir_all(&p).ok();
        config.set_game_value(game_name, "PrefixPath", &p.display().to_string());
        p
    }

    pub fn query_folder_size(&self, path: &Path) -> u64 {
        let mut total = 0;
        if let Ok(entries) = fs::read_dir(path) {
            for entry in entries.flatten() {
                let p = entry.path();
                if p.is_file() {
                    total += fs::metadata(&p).map(|m| m.len()).unwrap_or(0);
                } else if p.is_dir() {
                    total += self.query_folder_size(&p);
                }
            }
        }
        total
    }

    pub fn format_size(bytes: u64) -> String {
        if bytes < 1024 { return format!("{} B", bytes); }
        if bytes < 1024 * 1024 { return format!("{:.1} KB", bytes as f64 / 1024.0); }
        if bytes < 1024 * 1024 * 1024 { return format!("{:.1} MB", bytes as f64 / (1024.0 * 1024.0)); }
        format!("{:.2} GB", bytes as f64 / (1024.0 * 1024.0 * 1024.0))
    }

    pub fn find_exe_in_game_dir(&self, game_name: &str) -> Vec<String> {
        let config = super::ConfigManager::new();
        let main_path = config.game_value(game_name, "MainPath").unwrap_or_default();
        if main_path.is_empty() {
            return Vec::new();
        }
        let expanded = shellexpand_tilde(&main_path);
        let path = Path::new(&expanded);
        if !path.exists() {
            return Vec::new();
        }
        let mut results = Vec::new();
        Self::collect_exes(path, &mut results, 3);
        results.sort();
        results
    }

    fn collect_exes(dir: &Path, out: &mut Vec<String>, depth: u8) {
        let entries = match fs::read_dir(dir) {
            Ok(e) => e,
            Err(_) => return,
        };
        for entry in entries.flatten() {
            let p = entry.path();
            if p.is_file() {
                let name = p.file_name().unwrap_or_default().to_string_lossy().to_string();
                let lower = name.to_lowercase();
                if lower.ends_with(".exe") || lower.ends_with(".bat") || lower.ends_with(".msi") {
                    // Skip crash handlers/uninstallers noise? No: show all,
                    // the user picks. Skip Wine/Proton internals just in case.
                    let s = p.display().to_string();
                    if !s.contains("/drive_c/") {
                        out.push(s);
                    }
                }
            } else if p.is_dir() && depth > 0 {
                if let Some(n) = p.file_name().and_then(|n| n.to_str()) {
                    if n.starts_with('.') {
                        continue;
                    }
                }
                Self::collect_exes(&p, out, depth - 1);
            }
        }
    }

    pub fn find_wine_tools(&self, game_name: &str) -> Vec<(String, String)> {
        let tools = vec![
            ("winecfg", "Wine Configuration"),
            ("taskmgr", "Task Manager"),
            ("control", "Control Panel"),
            ("explorer", "File Explorer"),
            ("cmd", "Command Prompt"),
        ];
        tools.into_iter().map(|(cmd, desc)| (cmd.to_string(), desc.to_string())).collect()
    }

    pub fn detect_game_arch(&self, exe_path: &str) -> String {
        let expanded = shellexpand_tilde(exe_path);
        if let Ok(content) = fs::read(&expanded) {
            if content.len() > 0x40 {
                // Check PE header magic
                if content[0] == b'M' && content[1] == b'Z' {
                    if let Some(pe_offset) = content.get(0x3C..0x40) {
                        let offset = u32::from_le_bytes([
                            pe_offset[0], pe_offset[1], pe_offset[2], pe_offset[3],
                        ]) as usize;
                        if let Some(magic) = content.get(offset..offset + 2) {
                            if magic == b"PE" {
                                if let Some(opt_hdr) = content.get(offset + 24..offset + 26) {
                                    let magic_val = u16::from_le_bytes([opt_hdr[0], opt_hdr[1]]);
                                    if magic_val == 0x20b { return "PE32+ (64-bit)".to_string(); }
                                    else if magic_val == 0x10b { return "PE32 (32-bit)".to_string(); }
                                }
                            }
                        }
                    }
                    return "PE (unknown bitness)".to_string();
                }
            }
        }
        "Unknown".to_string()
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

/// wineserver inside a Proton tree (GE/CachyOS: files/bin/wineserver,
/// some builds: dist/bin/wineserver), plus C++-style relatives.
fn find_wineserver(proton_dir: &Path) -> Option<PathBuf> {
    for rel in ["files/bin/wineserver", "dist/bin/wineserver", "bin/wineserver"] {
        let cand = proton_dir.join(rel);
        if cand.is_file() {
            return Some(cand);
        }
    }
    // C++ parity: search next to/above the proton binary dir.
    let mut dir = Some(proton_dir);
    for _ in 0..3 {
        let d = match dir {
            Some(d) => d,
            None => break,
        };
        let cand = d.join("wineserver");
        if cand.is_file() {
            return Some(cand);
        }
        dir = d.parent();
    }
    None
}

/// Best-effort system info for the Lutris-style debug header.
fn sys_distro() -> String {
    std::fs::read_to_string("/etc/os-release")
        .unwrap_or_default()
        .lines()
        .find_map(|l| l.strip_prefix("PRETTY_NAME="))
        .unwrap_or("")
        .trim_matches(|c| c == '"' || c == '\'')
        .to_string()
}

fn sys_kernel() -> String {
    Command::new("uname")
        .args(["-srm"])
        .output()
        .ok()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "Unknown".into())
}

fn sys_cpu() -> String {
    let info = std::fs::read_to_string("/proc/cpuinfo").unwrap_or_default();
    let model = info
        .lines()
        .find_map(|l| l.strip_prefix("model name"))
        .and_then(|l| l.split_once(':'))
        .map(|(_, v)| v.trim().to_string())
        .unwrap_or_else(|| "Unknown CPU".into());
    let cores = info.lines().filter(|l| l.starts_with("processor")).count();
    if cores > 1 {
        format!("{} ({} threads)", model, cores)
    } else {
        model
    }
}

fn sys_mem() -> String {
    let info = std::fs::read_to_string("/proc/meminfo").unwrap_or_default();
    info.lines()
        .find_map(|l| l.strip_prefix("MemTotal:"))
        .and_then(|l| l.split_whitespace().next())
        .and_then(|kb| kb.parse::<u64>().ok())
        .map(|kb| format!("{:.1} GiB", kb as f64 / 1024.0 / 1024.0))
        .unwrap_or_else(|| "Unknown".into())
}

fn sys_gpu() -> String {
    // glxinfo first (gives renderer + Mesa/OpenGL version), lspci fallback.
    if let Ok(o) = Command::new("glxinfo").arg("-B").output() {
        if o.status.success() {
            let body = String::from_utf8_lossy(&o.stdout);
            let dev = body
                .lines()
                .find(|l| l.starts_with("Device:"))
                .map(|l| l["Device:".len()..].trim().to_string());
            let ver = body
                .lines()
                .find(|l| l.starts_with("OpenGL version string:"))
                .map(|l| l["OpenGL version string:".len()..].trim().to_string());
            match (dev, ver) {
                (Some(d), Some(v)) => return format!("{} ({})", d, v),
                (Some(d), None) => return d,
                _ => {}
            }
        }
    }
    if let Ok(o) = Command::new("lspci").output() {
        if o.status.success() {
            let body = String::from_utf8_lossy(&o.stdout);
            let mut gpus: Vec<String> = body
                .lines()
                .filter(|l| l.contains("VGA") || l.contains("3D controller"))
                .filter_map(|l| l.split_once(": ").map(|(_, v)| v.trim().to_string()))
                .collect();
            gpus.dedup();
            if !gpus.is_empty() {
                return gpus.join(" / ");
            }
        }
    }
    "Unknown".into()
}

fn sys_display() -> String {
    if let Ok(w) = std::env::var("WAYLAND_DISPLAY") {
        if !w.is_empty() {
            return format!("Wayland ({})", w);
        }
    }
    format!(
        "X11 ({})",
        std::env::var("DISPLAY").unwrap_or_else(|_| "unknown".into())
    )
}

fn find_proton_binary(path: &Path) -> Option<PathBuf> {
    let candidates = ["proton", "proton_9", "proton_8", "proton_7"];
    for name in candidates {
        let p = path.join(name);
        if p.exists() {
            return Some(p);
        }
    }
    fs::read_dir(path).ok()?.find_map(|e| {
        let e = e.ok()?;
        let p = e.path();
        if p.is_file() {
            let name = p.file_name()?.to_string_lossy();
            if name.starts_with("proton") || name == "run" {
                return Some(p);
            }
        }
        None
    })
}

fn which(name: &str) -> Option<String> {
    if let Ok(path) = Command::new("which").arg(name).output() {
        if path.status.success() {
            return Some(String::from_utf8_lossy(&path.stdout).trim().to_string());
        }
    }
    None
}
