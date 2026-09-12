use std::path::PathBuf;
use std::sync::mpsc;

use super::plugin_process::{self, PluginEvent};

fn exe(plugin_id: &str, entry: &str) -> PathBuf {
    plugin_process::plugin_exe(plugin_id, entry)
}

pub struct AppImageManager;

impl AppImageManager {
    pub const ID: &'static str = "appimage-launcher";
    pub const ENTRY: &'static str = "appimage-launcher";

    pub fn exe() -> PathBuf {
        exe(Self::ID, Self::ENTRY)
    }

    pub fn available() -> bool {
        plugin_process::plugin_available(Self::ID, Self::ENTRY)
    }

    pub fn scan(path: &str) -> Result<serde_json::Value, String> {
        plugin_process::run_single_json(&Self::exe(), &["scan", path])
    }

    pub fn status() -> Result<serde_json::Value, String> {
        plugin_process::run_single_json(&Self::exe(), &["status"])
    }

    pub fn extract_icon(path: &str, out: &str) -> Result<serde_json::Value, String> {
        plugin_process::run_single_json(&Self::exe(), &["icon", path, "--out", out])
    }

    pub fn integrate(path: &str, move_to_apps: bool) -> Result<serde_json::Value, String> {
        if move_to_apps {
            plugin_process::run_single_json(&Self::exe(), &["integrate", path, "--move"])
        } else {
            plugin_process::run_single_json(&Self::exe(), &["integrate", path])
        }
    }

    pub fn run_detached(path: &str, args: &[String]) -> Result<(), String> {
        let exe_path = Self::exe();
        if !exe_path.exists() {
            return Err("AppImage plugin no instalado".into());
        }
        let mut cmd = std::process::Command::new(&exe_path);
        cmd.arg("run").arg(path);
        for a in args {
            cmd.arg(a);
        }
        cmd.stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null());
        cmd.spawn().map(|_| ()).map_err(|e| e.to_string())
    }

    pub fn spawn_run(path: String, args: Vec<String>) -> mpsc::Receiver<PluginEvent> {
        let mut full = vec!["run".to_string(), path];
        full.extend(args);
        plugin_process::spawn_streaming(Self::exe(), full)
    }
}

pub struct RpgMakerManager;

impl RpgMakerManager {
    pub const ID: &'static str = "rpgmaker-runtime";
    pub const ENTRY: &'static str = "rpgmaker-runtime";

    pub fn exe() -> PathBuf {
        exe(Self::ID, Self::ENTRY)
    }

    pub fn available() -> bool {
        plugin_process::plugin_available(Self::ID, Self::ENTRY)
    }

    pub fn scan(path: &str) -> Result<serde_json::Value, String> {
        plugin_process::run_single_json(&Self::exe(), &["scan", path])
    }

    pub fn status() -> Result<serde_json::Value, String> {
        plugin_process::run_single_json(&Self::exe(), &["status"])
    }

    pub fn install_check() -> Result<serde_json::Value, String> {
        plugin_process::run_single_json(&Self::exe(), &["install", "check"])
    }

    pub fn install_box() -> Result<serde_json::Value, String> {
        plugin_process::run_single_json(&Self::exe(), &["install", "box"])
    }

    pub fn install_runtime(kind: &str, version: Option<&str>) -> Result<serde_json::Value, String> {
        match version {
            Some(v) => plugin_process::run_single_json(&Self::exe(), &["install", "runtime", kind, v]),
            None => plugin_process::run_single_json(&Self::exe(), &["install", "runtime", kind]),
        }
    }

    pub fn run(path: &str) -> Result<serde_json::Value, String> {
        plugin_process::run_single_json(&Self::exe(), &["run", path])
    }

    pub fn run_with_runtime(path: &str, runtime: &str) -> Result<serde_json::Value, String> {
        if runtime.trim().is_empty() {
            return Self::run(path);
        }
        plugin_process::run_single_json(&Self::exe(), &["run", path, "--runtime", runtime])
    }

    pub fn diagnose(path: &str, runtime: Option<&str>) -> Result<serde_json::Value, String> {
        match runtime {
            Some(r) if !r.trim().is_empty() => {
                plugin_process::run_single_json(&Self::exe(), &["diagnose", path, "--runtime", r])
            }
            _ => plugin_process::run_single_json(&Self::exe(), &["diagnose", path]),
        }
    }

    pub fn engine_of(scan_doc: &serde_json::Value) -> String {
        scan_doc.get("engine").and_then(|v| v.as_str()).unwrap_or("unknown").to_string()
    }
}

pub struct StoreManager;

impl StoreManager {
    pub const ID: &'static str = "heroic-store";
    pub const ENTRY: &'static str = "heroic-store";

    pub fn exe() -> PathBuf {
        exe(Self::ID, Self::ENTRY)
    }

    pub fn available() -> bool {
        plugin_process::plugin_available(Self::ID, Self::ENTRY)
    }

    pub fn status() -> Result<serde_json::Value, String> {
        plugin_process::run_single_json(&Self::exe(), &["status"])
    }

    pub fn setup() -> Result<serde_json::Value, String> {
        plugin_process::run_single_json(&Self::exe(), &["setup"])
    }

    pub fn auth(store: &str, code: &str) -> Result<serde_json::Value, String> {
        let mut args = vec!["auth", "--store", store];
        let code_owned;
        if !code.is_empty() {
            code_owned = code.to_string();
            args.push("--code");
            args.push(&code_owned);
        }
        let refs: Vec<&str> = args.iter().map(|s| *s).collect();
        plugin_process::run_single_json(&Self::exe(), &refs)
    }

    pub fn spawn_login_window(store: String) -> mpsc::Receiver<PluginEvent> {
        plugin_process::spawn_streaming(
            Self::exe(),
            vec!["login-window".to_string(), "--store".to_string(), store],
        )
    }

    pub fn spawn_auth(store: String, code: String) -> mpsc::Receiver<PluginEvent> {
        let mut args = vec!["auth".to_string(), "--store".to_string(), store];
        if !code.is_empty() {
            args.push("--code".to_string());
            args.push(code);
        }
        plugin_process::spawn_streaming(Self::exe(), args)
    }

    pub fn library(store: &str, refresh: bool) -> Result<serde_json::Value, String> {
        let mut args = vec!["library", "--store", store];
        if refresh {
            args.push("--refresh");
        }
        let refs: Vec<&str> = args.iter().map(|s| *s).collect();
        plugin_process::run_single_json(&Self::exe(), &refs)
    }

    pub fn free_promos() -> Result<serde_json::Value, String> {
        plugin_process::run_single_json(&Self::exe(), &["free-promos"])
    }

    pub fn epic_deals() -> Result<serde_json::Value, String> {
        plugin_process::run_single_json(&Self::exe(), &["epic-deals"])
    }

    pub fn gog_store_search(query: &str, limit: u32) -> Result<serde_json::Value, String> {
        plugin_process::run_single_json(
            &Self::exe(),
            &["gog-store-search", "--query", query, "--limit", &limit.to_string()],
        )
    }

    pub fn spawn_install(store: String, app_id: String, path: String) -> mpsc::Receiver<PluginEvent> {
        let mut args = vec![
            "install".to_string(),
            "--store".to_string(), store,
            "--app-id".to_string(), app_id,
        ];
        if !path.is_empty() {
            args.push("--path".to_string());
            args.push(path);
        }
        plugin_process::spawn_streaming(Self::exe(), args)
    }

    pub fn uninstall(store: &str, app_id: &str) -> Result<serde_json::Value, String> {
        plugin_process::run_single_json(&Self::exe(), &["uninstall", "--store", store, "--app-id", app_id])
    }

    pub fn launch_info(store: &str, app_id: &str) -> Result<serde_json::Value, String> {
        plugin_process::run_single_json(&Self::exe(), &["launch-info", "--store", store, "--app-id", app_id])
    }

    pub fn game_info(store: &str, app_id: &str) -> Result<serde_json::Value, String> {
        plugin_process::run_single_json(&Self::exe(), &["game-info", "--store", store, "--app-id", app_id])
    }

    pub fn heroic_scan() -> Result<serde_json::Value, String> {
        plugin_process::run_single_json(&Self::exe(), &["heroic-scan"])
    }

    pub fn eos_code() -> Result<serde_json::Value, String> {
        plugin_process::run_single_json(&Self::exe(), &["eos-code"])
    }

    pub fn logout(store: &str) -> Result<serde_json::Value, String> {
        plugin_process::run_single_json(&Self::exe(), &["logout", "--store", store])
    }

    pub fn umu_status() -> Result<serde_json::Value, String> {
        plugin_process::run_single_json(&Self::exe(), &["umu-status"])
    }

    pub fn spawn_umu_setup() -> mpsc::Receiver<PluginEvent> {
        plugin_process::spawn_streaming(Self::exe(), vec!["umu-setup".to_string()])
    }
}

#[derive(Default)]
pub struct LaunchOpts {
    pub max_mb: u32,
    pub min_mb: u32,
    pub jvm_args: Vec<String>,
    pub resolution: String,
    pub wrapper: String,
    pub pre_hook: String,
}

pub struct MinecraftManager;

impl MinecraftManager {
    pub const ID: &'static str = "minecraft-launcher";
    pub const ENTRY: &'static str = "minecraft-launcher";

    pub fn exe() -> PathBuf {
        exe(Self::ID, Self::ENTRY)
    }

    pub fn available() -> bool {
        plugin_process::plugin_available(Self::ID, Self::ENTRY)
    }

    pub fn status() -> Result<serde_json::Value, String> {
        plugin_process::run_single_json(&Self::exe(), &["status"])
    }

    pub fn setup() -> Result<serde_json::Value, String> {
        plugin_process::run_single_json(&Self::exe(), &["setup"])
    }

    pub fn offline(name: &str) -> Result<serde_json::Value, String> {
        plugin_process::run_single_json(&Self::exe(), &["offline", "--name", name])
    }

    pub fn unaccount(account: &str) -> Result<serde_json::Value, String> {
        plugin_process::run_single_json(&Self::exe(), &["unaccount", "--account", account])
    }

    pub fn java_detect() -> Result<serde_json::Value, String> {
        plugin_process::run_single_json(&Self::exe(), &["java-detect"])
    }

    pub fn java_use(path: &str) -> Result<serde_json::Value, String> {
        plugin_process::run_single_json(&Self::exe(), &["java-use", "--path", path])
    }

    pub fn manifest_java(mc_version: &str) -> Result<serde_json::Value, String> {
        plugin_process::run_single_json(&Self::exe(), &["manifest-java", "--mc-version", mc_version])
    }

    pub fn java_required(version: &str, mc_dir: &str) -> Result<serde_json::Value, String> {
        plugin_process::run_single_json(
            &Self::exe(),
            &["java-required", "--version", version, "--mc-dir", mc_dir],
        )
    }

    pub fn auth_config(client_id: &str) -> Result<serde_json::Value, String> {
        plugin_process::run_single_json(&Self::exe(), &["auth-config", "--client-id", client_id])
    }

    pub fn auth_config_status() -> Result<serde_json::Value, String> {
        plugin_process::run_single_json(&Self::exe(), &["auth-config", "--show"])
    }

    pub fn ely_auth(username: &str, password: &str) -> Result<serde_json::Value, String> {
        plugin_process::run_single_json(&Self::exe(), &["ely-auth", "--username", username, "--password", password])
    }

    pub fn ely_refresh(account: &str) -> Result<serde_json::Value, String> {
        plugin_process::run_single_json(&Self::exe(), &["ely-refresh", "--account", account])
    }

    pub fn ely_validate(account: &str) -> Result<serde_json::Value, String> {
        plugin_process::run_single_json(&Self::exe(), &["ely-validate", "--account", account])
    }

    pub fn ely_skin(name: &str, uuid: &str, refresh: bool) -> Result<serde_json::Value, String> {
        let mut args = vec!["ely-skin".to_string(), "--name".to_string(), name.to_string()];
        if !uuid.is_empty() {
            args.push("--uuid".to_string());
            args.push(uuid.to_string());
        }
        if refresh {
            args.push("--refresh".to_string());
        }
        let refs: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
        plugin_process::run_single_json(&Self::exe(), &refs)
    }

    pub fn mods_list() -> Result<serde_json::Value, String> {
        plugin_process::run_single_json(&Self::exe(), &["mods-list"])
    }

    pub fn mod_toggle(file: &str) -> Result<serde_json::Value, String> {
        plugin_process::run_single_json(&Self::exe(), &["mod-toggle", "--file", file])
    }

    pub fn mods_add(path: &str, force: bool) -> Result<serde_json::Value, String> {
        if force {
            plugin_process::run_single_json(&Self::exe(), &["mods-add", "--path", path, "--force"])
        } else {
            plugin_process::run_single_json(&Self::exe(), &["mods-add", "--path", path])
        }
    }

    pub fn stop(account: Option<&str>) -> Result<serde_json::Value, String> {
        match account {
            Some(a) if !a.is_empty() => {
                plugin_process::run_single_json(&Self::exe(), &["stop", "--account", a])
            }
            _ => plugin_process::run_single_json(&Self::exe(), &["stop"]),
        }
    }

    pub fn loader_versions(loader: &str, mc_version: &str) -> Result<serde_json::Value, String> {
        plugin_process::run_single_json(
            &Self::exe(),
            &["loader-versions", "--loader", loader, "--mc-version", mc_version],
        )
    }

    pub fn spawn_install_isolated(
        version: String,
        loader: String,
        loader_version: String,
        mc_dir: String,
    ) -> mpsc::Receiver<PluginEvent> {
        let mut args = vec!["install".to_string(), "--version".to_string(), version];
        if !loader.is_empty() && loader != "vanilla" {
            args.push("--loader".to_string());
            args.push(loader);
            if !loader_version.is_empty() {
                args.push("--loader-version".to_string());
                args.push(loader_version);
            }
        }
        if !mc_dir.is_empty() {
            args.push("--mc-dir".to_string());
            args.push(mc_dir);
        }
        plugin_process::spawn_streaming(Self::exe(), args)
    }

    pub fn spawn_launch_isolated(
        version: String,
        account: Option<String>,
        max_memory_mb: u32,
        java: String,
        mc_dir: String,
        opts: Option<LaunchOpts>,
    ) -> mpsc::Receiver<PluginEvent> {
        let mut args = vec!["launch".to_string(), "--version".to_string(), version];
        if let Some(a) = account {
            if !a.is_empty() {
                args.push("--account".to_string());
                args.push(a);
            }
        }
        args.push("--max-memory-mb".to_string());
        args.push(max_memory_mb.to_string());
        if !java.is_empty() {
            args.push("--java".to_string());
            args.push(java);
        }
        if !mc_dir.is_empty() {
            args.push("--mc-dir".to_string());
            args.push(mc_dir);
        }
        if let Some(o) = opts {
            if o.min_mb > 0 {
                args.push("--min-memory-mb".to_string());
                args.push(o.min_mb.to_string());
            }
            for j in o.jvm_args {
                if !j.trim().is_empty() {
                    args.push("--jvm-arg".to_string());
                    args.push(j);
                }
            }
            if !o.resolution.is_empty() {
                args.push("--resolution".to_string());
                args.push(o.resolution);
            }
            if !o.wrapper.is_empty() {
                args.push("--wrapper".to_string());
                args.push(o.wrapper);
            }
            if !o.pre_hook.is_empty() {
                args.push("--pre-hook".to_string());
                args.push(o.pre_hook);
            }
        }
        plugin_process::spawn_streaming(Self::exe(), args)
    }

    pub fn mod_search(
        query: &str,
        project_type: &str,
        mc_version: &str,
        loader: &str,
        limit: u32,
    ) -> Result<serde_json::Value, String> {
        plugin_process::run_single_json(
            &Self::exe(),
            &[
                "mod-search",
                "--query", query,
                "--project-type", project_type,
                "--mc-version", mc_version,
                "--loader", loader,
                "--limit", &limit.to_string(),
            ],
        )
    }

    pub fn mod_versions(
        project_id: &str,
        mc_version: &str,
        loader: &str,
        project_type: &str,
    ) -> Result<serde_json::Value, String> {
        plugin_process::run_single_json(
            &Self::exe(),
            &[
                "mod-versions",
                "--project-id", project_id,
                "--mc-version", mc_version,
                "--loader", loader,
                "--project-type", project_type,
            ],
        )
    }

    pub fn addon_project_type(addon_type: &str) -> &'static str {
        match addon_type {
            "shaders" => "shader",
            "resourcepacks" => "resourcepack",
            _ => "mod",
        }
    }

    pub fn spawn_mod_install(
        project_id: String,
        version_id: String,
        mc_version: String,
        loader: String,
        addon_type: String,
        mc_dir: String,
    ) -> mpsc::Receiver<PluginEvent> {
        let mut args = vec![
            "mod-install".to_string(),
            "--project-id".to_string(), project_id,
            "--addon-type".to_string(), addon_type,
        ];
        if !mc_dir.is_empty() {
            args.push("--mc-dir".to_string());
            args.push(mc_dir);
        }
        if !version_id.is_empty() {
            args.push("--version-id".to_string());
            args.push(version_id);
        }
        if !mc_version.is_empty() {
            args.push("--mc-version".to_string());
            args.push(mc_version);
        }
        if !loader.is_empty() {
            args.push("--loader".to_string());
            args.push(loader);
        }
        plugin_process::spawn_streaming(Self::exe(), args)
    }

    pub fn addons_list(addon_type: &str) -> Result<serde_json::Value, String> {
        plugin_process::run_single_json(&Self::exe(), &["addons-list", "--addon-type", addon_type])
    }

    pub fn mod_check_updates(
        addon_type: &str,
        mc_version: &str,
        loader: &str,
        mc_dir: &str,
    ) -> Result<serde_json::Value, String> {
        let mut args = vec![
            "mod-check-updates".to_string(),
            "--addon-type".to_string(), addon_type.to_string(),
            "--mc-version".to_string(), mc_version.to_string(),
            "--loader".to_string(), loader.to_string(),
        ];
        if !mc_dir.is_empty() {
            args.push("--mc-dir".to_string());
            args.push(mc_dir.to_string());
        }
        let refs: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
        plugin_process::run_single_json(&Self::exe(), &refs)
    }

    pub fn spawn_modpack_install(
        project_id: String,
        version_id: String,
        mc_dir: String,
    ) -> mpsc::Receiver<PluginEvent> {
        let mut args = vec![
            "modpack-install".to_string(),
            "--project-id".to_string(), project_id,
        ];
        if !version_id.is_empty() {
            args.push("--version-id".to_string());
            args.push(version_id);
        }
        if !mc_dir.is_empty() {
            args.push("--mc-dir".to_string());
            args.push(mc_dir);
        }
        plugin_process::spawn_streaming(Self::exe(), args)
    }

    pub fn spawn_cf_modpack_install(
        pack_id: String,
        file_id: String,
        mc_dir: String,
    ) -> mpsc::Receiver<PluginEvent> {
        let mut args = vec![
            "cf-modpack-install".to_string(),
            "--pack-id".to_string(), pack_id,
        ];
        if !file_id.is_empty() {
            args.push("--file-id".to_string());
            args.push(file_id);
        }
        if !mc_dir.is_empty() {
            args.push("--mc-dir".to_string());
            args.push(mc_dir);
        }
        plugin_process::spawn_streaming(Self::exe(), args)
    }

    pub fn spawn_modpack_import(path: String, mc_dir: String) -> mpsc::Receiver<PluginEvent> {
        plugin_process::spawn_streaming(
            Self::exe(),
            vec!["modpack-import".to_string(), "--path".to_string(), path,
                 "--mc-dir".to_string(), mc_dir],
        )
    }

    pub fn mod_project(project_id: &str) -> Result<serde_json::Value, String> {
        plugin_process::run_single_json(&Self::exe(), &["mod-project", "--project-id", project_id])
    }

    pub fn curse_save_key(api_key: &str) -> Result<serde_json::Value, String> {
        plugin_process::run_single_json(
            &Self::exe(),
            &["curse-auth-config", "--api-key", api_key],
        )
    }

    pub fn curse_test() -> Result<serde_json::Value, String> {
        plugin_process::run_single_json(&Self::exe(), &["curse-test"])
    }

    pub fn curse_configured() -> bool {
        plugin_process::run_single_json(&Self::exe(), &["curse-auth-config", "--show"])
            .ok()
            .and_then(|d| d.get("configured").and_then(|x| x.as_bool()))
            .unwrap_or(false)
    }

    pub fn cf_search(
        query: &str,
        kind: &str,
        mc_version: &str,
        loader: &str,
        limit: u32,
    ) -> Result<serde_json::Value, String> {
        plugin_process::run_single_json(
            &Self::exe(),
            &[
                "cf-search",
                "--query", query,
                "--kind", kind,
                "--mc-version", mc_version,
                "--loader", loader,
                "--limit", &limit.to_string(),
            ],
        )
    }

    pub fn cf_versions(
        mod_id: &str,
        mc_version: &str,
        loader: &str,
        kind: &str,
    ) -> Result<serde_json::Value, String> {
        plugin_process::run_single_json(
            &Self::exe(),
            &[
                "cf-versions",
                "--mod-id", mod_id,
                "--mc-version", mc_version,
                "--loader", loader,
                "--kind", kind,
            ],
        )
    }

    pub fn cf_project(mod_id: &str) -> Result<serde_json::Value, String> {
        plugin_process::run_single_json(&Self::exe(), &["cf-project", "--mod-id", mod_id])
    }

    pub fn spawn_cf_install(
        mod_id: String,
        file_id: String,
        mc_version: String,
        loader: String,
        kind: String,
        addon_type: String,
        mc_dir: String,
    ) -> mpsc::Receiver<PluginEvent> {
        let mut args = vec![
            "cf-install".to_string(),
            "--mod-id".to_string(), mod_id,
            "--kind".to_string(), kind,
            "--addon-type".to_string(), addon_type,
        ];
        if !mc_dir.is_empty() {
            args.push("--mc-dir".to_string());
            args.push(mc_dir);
        }
        if !file_id.is_empty() {
            args.push("--file-id".to_string());
            args.push(file_id);
        }
        if !mc_version.is_empty() {
            args.push("--mc-version".to_string());
            args.push(mc_version);
        }
        if !loader.is_empty() {
            args.push("--loader".to_string());
            args.push(loader);
        }
        plugin_process::spawn_streaming(Self::exe(), args)
    }

    pub fn mod_match(addon_type: &str, mc_dir: &str) -> Result<serde_json::Value, String> {
        plugin_process::run_single_json(
            &Self::exe(),
            &["mod-match", "--addon-type", addon_type, "--mc-dir", mc_dir],
        )
    }

    pub fn spawn_mod_match(addon_type: String, mc_dir: String) -> mpsc::Receiver<PluginEvent> {
        plugin_process::spawn_streaming(
            Self::exe(),
            vec!["mod-match".to_string(), "--addon-type".to_string(), addon_type,
                 "--mc-dir".to_string(), mc_dir],
        )
    }

    pub fn mod_update(file: &str, addon_type: &str, mc_dir: &str) -> Result<serde_json::Value, String> {
        let mut args = vec![
            "mod-update".to_string(),
            "--file".to_string(), file.to_string(),
            "--addon-type".to_string(), addon_type.to_string(),
        ];
        if !mc_dir.is_empty() {
            args.push("--mc-dir".to_string());
            args.push(mc_dir.to_string());
        }
        let refs: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
        plugin_process::run_single_json(&Self::exe(), &refs)
    }

    pub fn running_pids() -> Vec<(String, i32)> {
        let home = std::env::var("HOME").unwrap_or_default();
        let locks = std::path::PathBuf::from(home)
            .join(".config/CorkyTux/plugins/minecraft-launcher/locks");
        let mut out = Vec::new();
        if let Ok(entries) = std::fs::read_dir(&locks) {
            for e in entries.flatten() {
                let p = e.path();
                if p.extension().and_then(|x| x.to_str()) != Some("lock") {
                    continue;
                }
                let aid = p.file_stem().and_then(|x| x.to_str()).unwrap_or("").to_string();
                let pid: i32 = std::fs::read_to_string(&p)
                    .unwrap_or_default().trim().parse().unwrap_or(0);
                if pid > 0 && std::path::Path::new(&format!("/proc/{}", pid)).exists() {
                    out.push((aid, pid));
                } else {
                    // stale lock: game is gone, drop it so Play/Stop state is true
                    std::fs::remove_file(&p).ok();
                }
            }
        }
        out
    }

    pub fn spawn_java_install(version: String) -> mpsc::Receiver<PluginEvent> {
        plugin_process::spawn_streaming(Self::exe(), vec!["java-install".to_string(), "--version".to_string(), version])
    }

    pub fn versions(all: bool) -> Result<serde_json::Value, String> {
        if all {
            plugin_process::run_single_json(&Self::exe(), &["versions", "--all"])
        } else {
            plugin_process::run_single_json(&Self::exe(), &["versions"])
        }
    }

    pub fn spawn_auth() -> mpsc::Receiver<PluginEvent> {
        plugin_process::spawn_streaming(Self::exe(), vec!["auth".to_string()])
    }

    pub fn spawn_install(version: String) -> mpsc::Receiver<PluginEvent> {
        plugin_process::spawn_streaming(Self::exe(), vec!["install".to_string(), "--version".to_string(), version])
    }

    pub fn spawn_launch(version: String, account: Option<String>, max_memory_mb: u32) -> mpsc::Receiver<PluginEvent> {
        let mut args = vec!["launch".to_string(), "--version".to_string(), version];
        if let Some(a) = account {
            if !a.is_empty() {
                args.push("--account".to_string());
                args.push(a);
            }
        }
        args.push("--max-memory-mb".to_string());
        args.push(max_memory_mb.to_string());
        plugin_process::spawn_streaming(Self::exe(), args)
    }

    pub fn java_candidates() -> Vec<PathBuf> {
        let mut out = Vec::new();
        for c in ["java", "/usr/bin/java", "/opt/java/bin/java"] {
            let p = PathBuf::from(c);
            if c == "java" {
                if let Ok(which) = std::process::Command::new("which").arg("java").output() {
                    if which.status.success() {
                        let s = String::from_utf8_lossy(&which.stdout).trim().to_string();
                        if !s.is_empty() {
                            out.push(PathBuf::from(s));
                        }
                    }
                }
            } else if p.exists() {
                out.push(p);
            }
        }
        if let Ok(jh) = std::env::var("JAVA_HOME") {
            let p = PathBuf::from(jh).join("bin").join("java");
            if p.exists() && !out.contains(&p) {
                out.push(p);
            }
        }
        out
    }
}

