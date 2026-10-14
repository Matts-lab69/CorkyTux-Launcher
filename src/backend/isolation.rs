//! Bottles-style Wine/Proton isolation (my own reimplementation).
//!
//! I run each isolated launch under `bwrap` with selective binds: the game
//! sees a private `$HOME`, its prefix (rw), its folder (rw), the
//! Proton/umu/Steam-runtime runners (ro), `/usr` + `/etc` (ro), `/proc`,
//! `/dev` (DRI rw for the GPU), a fresh `/tmp` and the host
//! `XDG_RUNTIME_DIR` (audio/display). I hide the real home with tmpfs and
//! only re-expose explicit subpaths. I share the network in v1.
//!
//! See `docs/wine-isolation.md` (design, Bottles credit, limits).

use std::path::{Path, PathBuf};
use std::process::Command;

/// Global key (User Settings): I isolate new prefixes by default.
pub const GLOBAL_KEY: &str = "IsolateNewPrefixes";
/// Per-game key (Games.ini): "true"/"false"; absent means I inherit global.
pub const GAME_KEY: &str = "Isolated";
/// Per-game key: extra writable paths, `;`-separated.
pub const GAME_PATHS_KEY: &str = "IsolatePaths";

/// Private per-game `$HOME` inside my launcher data.
pub fn sandbox_home(game_name: &str) -> PathBuf {
    let base = std::env::var("HOME").unwrap_or_default();
    PathBuf::from(base)
        .join(".local/share/CorkyTux/sandbox-home")
        .join(safe_name(game_name))
}

fn safe_name(game_name: &str) -> String {
    let s: String = game_name
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == '.' {
                c
            } else {
                '_'
            }
        })
        .collect();
    let s = s.trim_matches(|c| c == '.' || c == '_');
    if s.is_empty() {
        "game".to_string()
    } else {
        s.chars().take(64).collect()
    }
}

/// I return `true` when bwrap works with user namespaces.
pub fn bwrap_available() -> bool {
    let bin = match which_bwrap() {
        Some(b) => b,
        None => return false,
    };
    std::process::Command::new(bin)
        .args([
            "--unshare-user",
            "--ro-bind", "/",
            "/",
            "--tmpfs", "/tmp",
            "true",
        ])
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

fn which_bwrap() -> Option<PathBuf> {
    std::env::var_os("PATH").and_then(|paths| {
        std::env::split_paths(&paths).find_map(|dir| {
            let full = dir.join("bwrap");
            if full.is_file() {
                Some(full)
            } else {
                None
            }
        })
    })
}

/// My isolation decision for a game. `game_isolated`: the per-game key
/// value ("true"/"false"/absent). I return (isolate, reason).
pub fn decide(game_isolated: Option<&str>, global_default: bool) -> (bool, &'static str) {
    match game_isolated.map(str::trim) {
        Some("true") | Some("1") => (true, "per-game switch on"),
        Some("false") | Some("0") => (false, "per-game switch off"),
        _ if global_default => (true, "global default for new prefixes"),
        _ => (false, "isolation off"),
    }
}

/// Extra game paths (`;`-separated, `~` expanded). Kept only when an
/// existing dir passes validation; see `validate_isolate_path`.
pub fn extra_paths(game_name: &str, raw: &str) -> Result<Vec<PathBuf>, String> {
    let mut out = Vec::new();
    for part in raw.split(';') {
        let p = part.trim();
        if p.is_empty() {
            continue;
        }
        match validate_isolate_path(game_name, p)? {
            Some(pb) => {
                if !out.contains(&pb) {
                    out.push(pb);
                }
            }
            None => {}
        }
    }
    Ok(out)
}

/// Validate one sandbox extra path for a game. `Ok(None)` skips silently
/// (empty or missing, as before); `Ok(Some)` is the canonical path to
/// bind; `Err` explains a denial. Never allows the HOME root itself,
/// its ancestors, sensitive subdirs, Steam data, another game's
/// sandbox, or entries that escape their own location.
pub fn validate_isolate_path(game_name: &str, raw: &str) -> Result<Option<PathBuf>, String> {
    let home = std::env::var("HOME").unwrap_or_default();
    if home.trim().is_empty() {
        return Err("HOME is not set".to_string());
    }
    let expanded = if raw == "~" || raw.starts_with("~/") {
        raw.replacen('~', &home, 1)
    } else {
        raw.to_string()
    };
    let home_canon = std::fs::canonicalize(&home).unwrap_or_else(|_| PathBuf::from(&home));
    // Lexical normalization (no filesystem): collapse `.` and `..`.
    let mut parts: Vec<std::ffi::OsString> = Vec::new();
    for comp in std::path::Path::new(&expanded).components() {
        use std::path::Component;
        match comp {
            // Never pop a root or prefix: "/.." stays "/".
            Component::ParentDir => {
                let last_is_normal = parts.last().map(|l| {
                    let mut c = std::path::Path::new(l).components();
                    matches!(c.next(), Some(Component::Normal(_))) && c.next().is_none()
                });
                if last_is_normal == Some(true) {
                    parts.pop();
                }
            }
            Component::CurDir => {}
            c => parts.push(c.as_os_str().to_os_string()),
        }
    }
    let lexical: PathBuf = parts.iter().collect();
    let canon = match std::fs::canonicalize(&lexical) {
        Ok(p) => p,
        Err(_) => return Ok(None),
    };
    let within = |root: &Path| canon == root || canon.starts_with(root);
    // HOME itself and anything above it.
    if canon == home_canon || canon == Path::new("/") || canon == Path::new("/home") {
        return Err("points at your HOME directory or above it".to_string());
    }
    // Sensitive subdirs under HOME.
    for sensitive in [".ssh", ".gnupg", ".config", ".mozilla"] {
        let root = home_canon.join(sensitive);
        if within(&root) {
            return Err(format!("points inside ~/{} (not allowed)", sensitive));
        }
    }
    // Steam data.
    for steam in [
        home_canon.join(".local/share/Steam"),
        home_canon.join(".steam"),
    ] {
        if within(&steam) {
            return Err("points at Steam data (not allowed)".to_string());
        }
    }
    // Another game's sandbox home.
    let sandbox_root = home_canon.join(".local/share/CorkyTux/sandbox-home");
    let own_home = sandbox_home(game_name);
    let own_canon =
        std::fs::canonicalize(&own_home).unwrap_or_else(|_| own_home.clone());
    if within(&sandbox_root) && !(canon == own_canon || canon.starts_with(&own_canon)) {
        return Err("points at another game's sandbox (not allowed)".to_string());
    }
    // Escape check: resolved location must stay where the entry points.
    // Compare against the lexical path with the HOME prefix resolved, so
    // a symlinked $HOME itself does not count as an escape.
    let lex_norm = lexical
        .strip_prefix(Path::new(&home))
        .map(|rest| home_canon.join(rest))
        .unwrap_or_else(|_| lexical.clone());
    if canon != lex_norm && !canon.starts_with(&lex_norm) {
        return Err("escapes its location through a symlink or ..".to_string());
    }
    Ok(Some(canon))
}

/// What my sandbox exposes. I filter missing paths when building argv
/// (bwrap fails when a source is absent).
pub struct SandboxSpec {
    pub game_name: String,
    /// Compat prefix (holds `pfx`), game folder, runtime dir...
    pub rw_dirs: Vec<PathBuf>,
    /// Proton, umu, steam-runtime, legendary, runtimes EAC/BE...
    pub ro_dirs: Vec<PathBuf>,
    /// cwd I want when the command brings none of its own.
    pub chdir: Option<PathBuf>,
}

/// I build the bwrap argv. `cmd_envs` are the vars I already set on the
/// command (I re-apply them as `--setenv` because the sandbox redefines
/// HOME/XDG_*). I return `None` when bwrap is missing.
pub fn sandbox_argv(
    cmd_envs: &[(String, String)],
    chdir: Option<&Path>,
    spec: &SandboxSpec,
) -> Option<Vec<String>> {
    if !bwrap_available() {
        return None;
    }
    let home = std::env::var("HOME").unwrap_or_default();
    let shome = sandbox_home(&spec.game_name);
    std::fs::create_dir_all(&shome).ok();
    std::fs::create_dir_all(shome.join(".config")).ok();
    std::fs::create_dir_all(shome.join(".cache")).ok();

    let mut argv: Vec<String> = vec!["bwrap".to_string()];

    argv.push("--unshare-user".to_string());
    // I hide the real home; I re-expose the needed subpaths below.
    if !home.is_empty() && Path::new(&home).is_dir() {
        argv.push("--tmpfs".to_string());
        argv.push(home.clone());
    }
    // Fresh /tmp (pressure-vessel and wine use it for temp files).
    argv.push("--tmpfs".to_string());
    argv.push("/tmp".to_string());
    argv.push("--proc".to_string());
    argv.push("/proc".to_string());
    argv.push("--dev".to_string());
    argv.push("/dev".to_string());
    // GPU: DRI needs write access (a ro-bind of / would leave it read-only).
    let dri = Path::new("/dev/dri");
    if dri.is_dir() {
        for s in ["--dev-bind", &dri.display().to_string(), &dri.display().to_string()] {
            argv.push(s.to_string());
        }
    }
    // Explicit writes: prefix, game folder, extras, HOME.
    let mut rw: Vec<PathBuf> = vec![shome.clone()];
    rw.extend(spec.rw_dirs.iter().cloned());
    // XDG_RUNTIME_DIR (audio/display) lives outside home: I expose it rw.
    if let Ok(rd) = std::env::var("XDG_RUNTIME_DIR") {
        if !rd.is_empty() && Path::new(&rd).is_dir() {
            rw.push(PathBuf::from(rd));
        }
    }
    let mut seen: Vec<String> = rw
        .iter()
        .filter(|p| p.is_dir())
        .map(|p| p.display().to_string())
        .collect();
    // Read-only base system (linker, umu/legendary python, certs,
    // resolv.conf, passwd, locales...).
    for p in ["/usr", "/etc", "/opt"] {
        let pb = Path::new(p);
        if pb.is_dir() {
            seen.push(p.to_string());
            for s in ["--ro-bind", p, p] {
                argv.push(s.to_string());
            }
        }
    }
    // usr-merge: /bin /lib /lib64 /sbin are symlinks into usr/* on the host
    // but missing inside (I only bound /usr): I recreate them.
    for (target, link) in [
        ("usr/bin", "/bin"),
        ("usr/lib", "/lib"),
        ("usr/lib64", "/lib64"),
        ("usr/bin", "/sbin"),
    ] {
        for s in ["--symlink", target, link] {
            argv.push(s.to_string());
        }
    }
    for p in dedup(rw) {
        // I never bind symlinks (e.g. /bin on usr-merge): bwrap would
        // resolve them to the real target (risking a rw /usr).
        if p.is_dir() && !p.is_symlink() {
            let s = p.display().to_string();
            for x in ["--bind", &s, &s] {
                argv.push(x.to_string());
            }
        }
    }
    for p in dedup(spec.ro_dirs.clone()) {
        if p.is_symlink() {
            continue;
        }
        if p.is_file() {
            // Loose binary (umu-run, legendary): I expose the parent dir.
            if let Some(parent) = p.parent() {
                if parent.is_dir() {
                    let s = parent.display().to_string();
                    if !seen.contains(&s) {
                        seen.push(s.clone());
                        for x in ["--ro-bind", &s, &s] {
                            argv.push(x.to_string());
                        }
                    }
                }
            }
        } else if p.is_dir() {
            let s = p.display().to_string();
            if seen.contains(&s) {
                continue;
            }
            seen.push(s.clone());
            for x in ["--ro-bind", &s, &s] {
                argv.push(x.to_string());
            }
        }
    }
    // Command environment (WINEPREFIX, PROTON_*, STEAM_COMPAT_*, ...).
    for (k, v) in cmd_envs {
        argv.push("--setenv".to_string());
        argv.push(k.clone());
        argv.push(v.clone());
    }
    // Sandbox HOME and XDG (after the command ones: these win).
    for (k, v) in [
        ("HOME", shome.display().to_string()),
        ("XDG_CACHE_HOME", shome.join(".cache").display().to_string()),
        ("XDG_CONFIG_HOME", shome.join(".config").display().to_string()),
    ] {
        argv.push("--setenv".to_string());
        argv.push(k.to_string());
        argv.push(v);
    }
    // cwd: the command's own, or the game folder.
    let cd = chdir.or(spec.chdir.as_deref());
    if let Some(d) = cd {
        if d.is_dir() {
            argv.push("--chdir".to_string());
            argv.push(d.display().to_string());
        }
    }
    Some(argv)
}

/// I wrap `cmd` in bwrap per `spec` (takes ownership). Command envs travel
/// as `--setenv`; the rest of the environment is inherited and stdio stays as
/// in the direct spawn. If bwrap is missing I return the command untouched
/// (my caller warns loudly).
pub fn wrap_command(cmd: Command, spec: &SandboxSpec) -> Command {
    let cmd_envs: Vec<(String, String)> = cmd
        .get_envs()
        .filter_map(|(k, v)| {
            v.map(|vv| {
                (
                    k.to_string_lossy().to_string(),
                    vv.to_string_lossy().to_string(),
                )
            })
        })
        .collect();
    let program = cmd.get_program().to_os_string();
    let args: Vec<std::ffi::OsString> = cmd.get_args().map(|a| a.to_os_string()).collect();
    // `std::process::Command` exposes no current_dir: I carry the cwd in
    // `spec.chdir` (game folder) and apply it as --chdir.
    let Some(argv) = sandbox_argv(&cmd_envs, None, spec) else {
        let mut c = Command::new(program);
        for a in &args {
            c.arg(a);
        }
        for (k, v) in cmd.get_envs() {
            match v {
                Some(vv) => {
                    c.env(k, vv);
                }
                None => {
                    c.env_remove(k);
                }
            }
        }
        return c;
    };
    let mut wrapped = Command::new(&argv[0]);
    for a in &argv[1..] {
        wrapped.arg(a);
    }
    // Original program + args at the end of the bwrap argv.
    wrapped.arg(program);
    for a in &args {
        wrapped.arg(a);
    }
    wrapped
}

fn dedup(paths: Vec<PathBuf>) -> Vec<PathBuf> {
    let mut out = Vec::new();
    for p in paths {
        if !out.contains(&p) {
            out.push(p);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decide_explicit_wins_over_global() {
        assert_eq!(decide(Some("true"), false).0, true);
        assert_eq!(decide(Some("false"), true).0, false);
        assert_eq!(decide(None, true).0, true);
        assert_eq!(decide(None, false).0, false);
        assert_eq!(decide(Some(""), true).0, true);
    }

    #[test]
    fn extra_paths_only_existing_dirs() {
        let _env = crate::test_env::lock();
        assert_eq!(
            extra_paths("game", "/tmp;/no-such-dir-xyz;/tmp").unwrap(),
            vec![PathBuf::from("/tmp")]
        );
        assert!(extra_paths("game", "").unwrap().is_empty());
        assert!(extra_paths("game", "/tmp/missing-deep/nested").unwrap().is_empty());
    }

    #[test]
    fn isolate_cmd_fails_without_bwrap() {
        use std::ffi::OsString;
        struct PathGuard(Option<OsString>);
        impl Drop for PathGuard {
            fn drop(&mut self) {
                match self.0.take() {
                    Some(p) => std::env::set_var("PATH", p),
                    None => std::env::remove_var("PATH"),
                }
            }
        }
        let _env = crate::test_env::lock();
        let _path_guard = PathGuard(std::env::var_os("PATH"));
        std::env::set_var("PATH", "/nonexistent-dir-xyz");
        let pm = crate::backend::proton::ProtonManager::new();
        let mut game = std::collections::HashMap::new();
        game.insert("isolated".to_string(), "true".to_string());
        let err = pm
            .isolate_cmd_for_test(
                std::process::Command::new("true"),
                "Game",
                &game,
                Vec::new(),
                Vec::new(),
                None,
            )
            .unwrap_err();
        assert!(err.contains("bubblewrap"), "{}", err);
        assert!(err.contains("Game"), "{}", err);
    }

    #[test]
    fn validate_isolate_paths_deny_and_allow() {
        let _env = crate::test_env::lock();
        let base = std::env::temp_dir().join(format!(
            "corky_iso_val_{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&base);
        std::fs::create_dir_all(&base).unwrap();
        std::env::set_var("HOME", &base);
        let home = base.clone();
        std::fs::create_dir_all(home.join("Games/Saves")).unwrap();
        std::fs::create_dir_all(home.join(".local/share/CorkyTux")).unwrap();
        // HOME itself and ancestors deny.
        assert!(validate_isolate_path("G", home.to_str().unwrap()).is_err());
        assert!(validate_isolate_path("G", "/").is_err());
        assert!(validate_isolate_path("G", "/home").is_err());
        // Sensitive subdirs deny.
        for sensitive in [".ssh", ".gnupg", ".config", ".mozilla"] {
            let dir = home.join(sensitive);
            std::fs::create_dir_all(&dir).unwrap();
            assert!(
                validate_isolate_path("G", dir.to_str().unwrap()).is_err(),
                "{}",
                sensitive
            );
        }
        // Steam data denies.
        let steam = home.join(".local/share/Steam/steamapps/compatdata/0");
        std::fs::create_dir_all(&steam).unwrap();
        assert!(validate_isolate_path("G", steam.to_str().unwrap()).is_err());
        // Symlink to HOME denies.
        #[cfg(unix)]
        std::os::unix::fs::symlink(&home, base.join("link")).unwrap();
        #[cfg(unix)]
        assert!(validate_isolate_path("G", base.join("link").to_str().unwrap()).is_err());
        // Normal subfolders allow (canonical result).
        let saves = home.join("Games/Saves");
        assert_eq!(
            validate_isolate_path("G", saves.to_str().unwrap()).unwrap(),
            Some(saves.clone())
        );
        // Escaping .. denies.
        let esc = home.join("Games/../.ssh");
        assert!(validate_isolate_path("G", esc.to_str().unwrap()).is_err());
        // Another game's sandbox denies; own allows.
        let other = home.join(".local/share/CorkyTux/sandbox-home/Other");
        std::fs::create_dir_all(&other).unwrap();
        assert!(validate_isolate_path("G", other.to_str().unwrap()).is_err());
        let own = home.join(".local/share/CorkyTux/sandbox-home/G");
        std::fs::create_dir_all(&own).unwrap();
        assert_eq!(
            validate_isolate_path("G", own.to_str().unwrap()).unwrap(),
            Some(own)
        );
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn sandbox_home_is_sandboxed() {
        let h = sandbox_home("Hollow Knight: Silksong!");
        assert!(h.ends_with("sandbox-home/Hollow_Knight__Silksong"));
        assert!(!h.to_string_lossy().contains(' '));
        assert_eq!(sandbox_home("..."), sandbox_home("...").parent().unwrap().join("game"));
    }

    #[test]
    fn argv_hides_home_exposes_prefix_and_game() {
        // HOME is process-global and another test swaps it: read it under
        // the shared lock so the value cannot change mid-test.
        let _env = crate::test_env::lock();
        if !bwrap_available() {
            return;
        }
        let home = std::env::var("HOME").unwrap();
        let spec = SandboxSpec {
            game_name: "argv-test".to_string(),
            rw_dirs: vec![PathBuf::from("/tmp"), PathBuf::from("/tmp")],
            ro_dirs: vec![PathBuf::from("/usr"), PathBuf::from("/bin")],
            chdir: Some(PathBuf::from("/tmp")),
        };
        let argv = sandbox_argv(&[("WINEPREFIX".to_string(), "/tmp/pfx".to_string())], None, &spec)
            .expect("bwrap present");
        let joined = argv.join(" ");
        // HOME hidden with tmpfs and redefined to the sandbox.
        assert!(joined.contains(&format!("--tmpfs {}", home)), "{}", joined);
        assert!(joined.contains("sandbox-home/argv-test"), "{}", joined);
        // Binds I expect.
        assert!(joined.contains("--bind /tmp /tmp"), "{}", joined);
        assert!(joined.contains("--ro-bind /usr /usr"), "{}", joined);
        // Command envs travel along.
        assert!(joined.contains("--setenv WINEPREFIX /tmp/pfx"), "{}", joined);
        // cwd.
        assert!(joined.contains("--chdir /tmp"), "{}", joined);
        // I share the network in v1 (no --unshare-net).
        assert!(!joined.contains("--unshare-net"), "{}", joined);
        // No duplicates thanks to dedup.
        assert_eq!(joined.matches("--bind /tmp /tmp").count(), 1);
        // /bin is a symlink (usr-merge): I never bind it (I do recreate
        // it as an internal symlink).
        assert!(!joined.contains("--bind /bin"), "{}", joined);
        assert!(!joined.contains("--ro-bind /bin"), "{}", joined);
    }
}
