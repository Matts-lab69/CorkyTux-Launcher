//! External binary lookup and process detection without spawning tools.
//!
//! `which`, `pidof` and `pgrep` are not guaranteed (NixOS, minimal images)
//! and cost a fork per call. PATH lookup and /proc reads cover the cases the
//! launcher needs; the pgrep sites that match a command line with a regex are
//! left alone on purpose (see the PR body).

use std::path::{Path, PathBuf};

/// First executable `name` in PATH (or the given path when it has a slash).
pub fn find_in_path(name: &str) -> Option<PathBuf> {
    if name.trim().is_empty() {
        return None;
    }
    if name.contains('/') {
        let path = PathBuf::from(name);
        return is_executable(&path).then_some(path);
    }
    let path = std::env::var_os("PATH")?;
    for dir in std::env::split_paths(&path) {
        let candidate = dir.join(name);
        if is_executable(&candidate) {
            return Some(candidate);
        }
    }
    None
}

fn is_executable(path: &Path) -> bool {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        match std::fs::metadata(path) {
            Ok(meta) => meta.is_file() && meta.permissions().mode() & 0o111 != 0,
            Err(_) => false,
        }
    }
    #[cfg(not(unix))]
    {
        path.is_file()
    }
}

/// One live process as read from /proc.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProcEntry {
    pub pid: u32,
    pub comm: String,
    pub exe: String,
    pub cmdline: Vec<String>,
}

/// Where the process table comes from; tests inject a fixed one.
pub trait ProcProvider {
    fn processes(&self) -> Vec<ProcEntry>;
}

pub struct RealProc;

impl ProcProvider for RealProc {
    fn processes(&self) -> Vec<ProcEntry> {
        let mut out = Vec::new();
        let Ok(entries) = std::fs::read_dir("/proc") else {
            return out;
        };
        for entry in entries.flatten() {
            let Some(pid) = entry.file_name().to_str().and_then(|s| s.parse::<u32>().ok()) else {
                continue;
            };
            let base = Path::new("/proc").join(pid.to_string());
            let comm = std::fs::read_to_string(base.join("comm"))
                .unwrap_or_default()
                .trim()
                .to_string();
            let exe = std::fs::read_link(base.join("exe"))
                .map(|p| p.display().to_string())
                .unwrap_or_default();
            let cmdline = std::fs::read(base.join("cmdline"))
                .map(|bytes| {
                    bytes
                        .split(|b| *b == 0)
                        .filter(|part| !part.is_empty())
                        .map(|part| String::from_utf8_lossy(part).to_string())
                        .collect()
                })
                .unwrap_or_default();
            out.push(ProcEntry {
                pid,
                comm,
                exe,
                cmdline,
            });
        }
        out
    }
}

pub fn find_process(
    provider: &dyn ProcProvider,
    predicate: impl Fn(&ProcEntry) -> bool,
) -> Option<u32> {
    provider.processes().into_iter().find(|p| predicate(p)).map(|p| p.pid)
}

pub fn any_process(predicate: impl Fn(&ProcEntry) -> bool) -> bool {
    find_process(&RealProc, predicate).is_some()
}

/// pids whose joined command line satisfies the predicate.
pub fn pids_matching(
    provider: &dyn ProcProvider,
    predicate: impl Fn(&ProcEntry) -> bool,
) -> Vec<u32> {
    provider
        .processes()
        .into_iter()
        .filter(|p| predicate(p))
        .map(|p| p.pid)
        .collect()
}

/// Undo the backslash escapes `pgrep` patterns use for a literal path.
fn unescape_ere(pattern: &str) -> String {
    let mut out = String::with_capacity(pattern.len());
    let mut chars = pattern.chars();
    while let Some(c) = chars.next() {
        if c == '\\' {
            if let Some(next) = chars.next() {
                out.push(next);
            }
        } else {
            out.push(c);
        }
    }
    out
}

/// Matches the three pgrep shapes the launcher builds, over a joined
/// command line: `^path( |$)`, `^prefix/` and an unanchored literal path.
pub fn pgrep_like_match(cmdline: &str, pattern: &str) -> bool {
    let literal = unescape_ere(pattern);
    if let Some(rest) = literal.strip_prefix('^') {
        if let Some(base) = rest.strip_suffix("( |$)") {
            return cmdline == base || cmdline.starts_with(&format!("{} ", base));
        }
        return cmdline.starts_with(rest);
    }
    cmdline.contains(&literal)
}

/// Process with this comm (no `pidof`).
pub fn process_named(comm: &str) -> bool {
    any_process(|p| p.comm == comm)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SteamKind {
    Native,
    Flatpak,
    Snap,
}

/// Steam client detection without running flatpak or snap.
pub fn steam_kind(provider: &dyn ProcProvider) -> Option<SteamKind> {
    provider
        .processes()
        .into_iter()
        .find_map(|p| {
            let joined = p.cmdline.join(" ");
            if p.exe.starts_with("/snap/steam")
                || joined.contains("/snap/steam")
                || p.exe.contains("snap/steam")
            {
                return Some(SteamKind::Snap);
            }
            if p.exe.contains("com.valvesoftware.Steam")
                || joined.contains("com.valvesoftware.Steam")
                || p.comm.contains("bwrap") && joined.contains("steam")
            {
                return Some(SteamKind::Flatpak);
            }
            if p.comm == "steam"
                || p.exe.ends_with("/steam")
                || joined.contains("/steam ")
            {
                return Some(SteamKind::Native);
            }
            None
        })
}

pub fn steam_running() -> bool {
    steam_kind(&RealProc).is_some()
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Fake(Vec<ProcEntry>);

    impl ProcProvider for Fake {
        fn processes(&self) -> Vec<ProcEntry> {
            self.0.clone()
        }
    }

    fn proc(pid: u32, comm: &str, exe: &str, args: &[&str]) -> ProcEntry {
        ProcEntry {
            pid,
            comm: comm.to_string(),
            exe: exe.to_string(),
            cmdline: args.iter().map(|a| a.to_string()).collect(),
        }
    }

    #[test]
    fn steam_native_flatpak_and_snap() {
        let native = Fake(vec![proc(10, "steam", "/usr/bin/steam", &["/usr/bin/steam"])]);
        assert_eq!(steam_kind(&native), Some(SteamKind::Native));

        let flatpak = Fake(vec![proc(
            11,
            "bwrap",
            "/usr/bin/bwrap",
            &["bwrap", "--", "com.valvesoftware.Steam"],
        )]);
        assert_eq!(steam_kind(&flatpak), Some(SteamKind::Flatpak));

        let snap = Fake(vec![proc(
            12,
            "steam",
            "/snap/steam/123/usr/bin/steam",
            &["/snap/steam/123/usr/bin/steam"],
        )]);
        assert_eq!(steam_kind(&snap), Some(SteamKind::Snap));

        let absent = Fake(vec![proc(13, "bash", "/bin/bash", &["bash"])]);
        assert_eq!(steam_kind(&absent), None);
    }

    #[test]
    fn path_lookup_skips_missing_and_non_executable() {
        use std::os::unix::fs::PermissionsExt;
        let base = std::env::temp_dir().join(format!("corky-extbin-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        let empty = base.join("empty");
        let good = base.join("good");
        std::fs::create_dir_all(&empty).unwrap();
        std::fs::create_dir_all(&good).unwrap();
        let plain = good.join("tool");
        std::fs::write(&plain, "#!/bin/sh\n").unwrap();
        let old_path = std::env::var_os("PATH");
        std::env::set_var(
            "PATH",
            format!(
                "{}:{}:{}",
                base.join("missing").display(),
                good.display(),
                empty.display()
            ),
        );
        // Not executable yet: not found.
        assert_eq!(find_in_path("tool"), None);
        let mut perms = std::fs::metadata(&plain).unwrap().permissions();
        perms.set_mode(0o755);
        std::fs::set_permissions(&plain, perms).unwrap();
        assert_eq!(find_in_path("tool"), Some(plain.clone()));
        assert_eq!(find_in_path("absent"), None);
        match old_path {
            Some(v) => std::env::set_var("PATH", v),
            None => std::env::remove_var("PATH"),
        }
        let _ = std::fs::remove_dir_all(&base);
    }
    #[test]
    fn pgrep_shapes_match_like_pgrep() {
        let path = "/games/Game.AppImage";
        let exact = ["/games/Game.AppImage", "--foo"];
        assert!(pgrep_like_match(&exact.join(" "), &format!("^{}( |$)", path)));
        assert!(pgrep_like_match(path, &format!("^{}( |$)", path)));
        assert!(!pgrep_like_match(&format!("{}.bak --x", path), &format!("^{}( |$)", path)));
        // Unanchored is a substring match, exactly like `pgrep -f`.
        assert!(pgrep_like_match(&format!("{}.bak --x", path), path));
        assert!(pgrep_like_match("/games/Game.AppImage", path));
        let mounted = "/tmp/.mount_GameX/usr/bin/game";
        assert!(pgrep_like_match(mounted, "^/tmp/.mount_GameX/"));
        assert!(!pgrep_like_match("/tmp/.mount_OtherX/game", "^/tmp/.mount_GameX/"));
        // A literal with regex metacharacters escaped still matches.
        assert!(pgrep_like_match("/games/a.b.exe", &"/games/a\\.b\\.exe"));
    }

}
