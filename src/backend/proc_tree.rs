//! Process tree tracking for game sessions.
//!
//! The launcher creates the game as a direct child (or as `setsid --wait`).
//! AppImage/Electron apps re-exec or self-update: the wrapper dies and the
//! real process is reparented. To keep counting the session we track every
//! process below the root by parent/child links and by the AppImage mount
//! point, instead of matching a command line with `pgrep -f`.

use std::path::Path;

/// One process as read from `/proc`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProcInfo {
    pub pid: u32,
    pub ppid: u32,
    /// Field 22 of `/proc/<pid>/stat`: start time in clock ticks since boot.
    pub starttime: u64,
    pub exe: String,
    pub cmdline: Vec<String>,
}

impl ProcInfo {
    /// Does this process belong to the AppImage mounted at `mount`?
    pub fn under_mount(&self, mount: &str) -> bool {
        if mount.is_empty() {
            return false;
        }
        let prefix = format!("{}/", mount.trim_end_matches('/'));
        if self.exe.starts_with(&prefix) {
            return true;
        }
        self.cmdline
            .iter()
            .any(|a| a.starts_with(&prefix))
    }
}

/// Source of process data. The real implementation reads `/proc`; tests pass
/// a fixed table.
pub trait ProcSource {
    fn list(&self) -> Vec<ProcInfo>;
}

/// Reads `/proc` on Linux.
pub struct RealProc;

impl ProcSource for RealProc {
    fn list(&self) -> Vec<ProcInfo> {
        let mut out = Vec::new();
        let Ok(entries) = std::fs::read_dir("/proc") else {
            return out;
        };
        for e in entries.flatten() {
            let name = e.file_name();
            let Some(pid) = name.to_str().and_then(|s| s.parse::<u32>().ok()) else {
                continue;
            };
            let base = Path::new("/proc").join(pid.to_string());
            let Ok(stat) = std::fs::read_to_string(base.join("stat")) else {
                continue;
            };
            // stat: pid (comm) state ppid ... ; comm may contain spaces/parens.
            let Some(rp) = stat.rfind(')') else { continue };
            let rest: Vec<&str> = stat[rp + 1..].split_whitespace().collect();
            // after ") " fields: state(0) ppid(1) ... starttime is field 22,
            // i.e. index 19 here (state=3, ppid=4, ... starttime=22).
            if rest.len() < 20 {
                continue;
            }
            let ppid = rest[1].parse::<u32>().unwrap_or(0);
            let starttime = rest[19].parse::<u64>().unwrap_or(0);
            let exe = std::fs::read_link(base.join("exe"))
                .map(|p| p.to_string_lossy().to_string())
                .unwrap_or_default();
            let cmdline = std::fs::read(base.join("cmdline"))
                .map(|b| {
                    b.split(|c| *c == 0)
                        .filter(|s| !s.is_empty())
                        .map(|s| String::from_utf8_lossy(s).to_string())
                        .collect()
                })
                .unwrap_or_default();
            out.push(ProcInfo { pid, ppid, starttime, exe, cmdline });
        }
        out
    }
}

/// A live session rooted at `root_pid`, optionally narrowed to an AppImage
/// mount point.
#[derive(Clone, Debug)]
pub struct SessionTree {
    pub root_pid: u32,
    pub root_starttime: u64,
    pub mount: String,
}

impl SessionTree {
    /// Pids that belong to the tree: the root itself (same starttime),
    /// every descendant by ppid, and every process under the mount point.
    pub fn live_pids(&self, procs: &[ProcInfo]) -> Vec<u32> {
        let mut by_parent: std::collections::HashMap<u32, Vec<u32>> =
            std::collections::HashMap::new();
        for p in procs {
            by_parent.entry(p.ppid).or_default().push(p.pid);
        }
        let mut seen = std::collections::HashSet::new();
        let mut stack = vec![self.root_pid];
        while let Some(pid) = stack.pop() {
            if !seen.insert(pid) {
                continue;
            }
            if let Some(kids) = by_parent.get(&pid) {
                stack.extend(kids.iter().copied());
            }
        }
        // Drop the root when the pid was reused (different starttime).
        let root_ok = procs
            .iter()
            .any(|p| p.pid == self.root_pid && p.starttime == self.root_starttime);
        if !root_ok {
            seen.remove(&self.root_pid);
        }
        if !self.mount.is_empty() {
            for p in procs {
                if p.under_mount(&self.mount) {
                    seen.insert(p.pid);
                }
            }
        }
        seen.into_iter().collect()
    }

    /// True when at least one process of the tree is still alive.
    pub fn alive(&self, procs: &[ProcInfo]) -> bool {
        !self.live_pids(procs).is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(pid: u32, ppid: u32, starttime: u64, exe: &str, cmd: &[&str]) -> ProcInfo {
        ProcInfo {
            pid,
            ppid,
            starttime,
            exe: exe.to_string(),
            cmdline: cmd.iter().map(|s| s.to_string()).collect(),
        }
    }

    struct Fake(Vec<ProcInfo>);
    impl ProcSource for Fake {
        fn list(&self) -> Vec<ProcInfo> {
            self.0.clone()
        }
    }

    #[test]
    fn wrapper_dies_but_mounted_child_keeps_session() {
        // setsid(100) -> wrapper(101) dies, mounted app(102) reparented to init.
        let procs = vec![
            p(1, 0, 0, "/sbin/init", &["/sbin/init"]),
            p(102, 1, 500, "/tmp/.mount_OpenChp0Gux4/openchamber", &["/tmp/.mount_OpenChp0Gux4/openchamber"]),
        ];
        let t = SessionTree { root_pid: 100, root_starttime: 400, mount: "/tmp/.mount_OpenChp0Gux4".into() };
        assert!(t.alive(&procs));
    }

    #[test]
    fn autoupdate_relaunch_keeps_session_via_new_mount() {
        // The old mount is gone; the new process is under the same launcher
        // tree because the launcher is the subreaper.
        let procs = vec![
            p(200, 1, 600, "/tmp/.mount_OpenChNewX/openchamber", &["/tmp/.mount_OpenChNewX/openchamber"]),
        ];
        let t = SessionTree { root_pid: 200, root_starttime: 600, mount: "/tmp/.mount_OpenChNewX".into() };
        assert!(t.alive(&procs));
    }

    #[test]
    fn all_dead_ends_session() {
        let procs = vec![p(1, 0, 0, "/sbin/init", &["/sbin/init"])];
        let t = SessionTree { root_pid: 100, root_starttime: 400, mount: "/tmp/.mount_X".into() };
        assert!(!t.alive(&procs));
    }

    #[test]
    fn reused_pid_with_other_starttime_does_not_count() {
        let procs = vec![p(100, 1, 999, "/tmp/.mount_X/app", &["/tmp/.mount_X/app"])];
        let t = SessionTree { root_pid: 100, root_starttime: 400, mount: String::new() };
        assert!(!t.alive(&procs));
    }
}
