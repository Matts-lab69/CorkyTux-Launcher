//! Durable per-session ledger so a launcher crash, or the headless
//! `--play` exit, does not lose the time of a session started by
//! `begin_session`.
//!
//! Model: a checkpoint credits `now - credited_until` to `TimeSpent` and
//! moves `credited_until` to `now`. Recovery for a dead process credits
//! nothing beyond the last checkpoint (it never invents time after it) and
//! deletes the file. Running recovery twice is therefore a no-op.

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

pub const SCHEMA_VERSION: u32 = 1;
pub const CHECKPOINT_SECS: u64 = 60;
/// A wall-clock jump larger than this is treated as a clock change.
pub const MAX_DELTA_SECS: u64 = 24 * 3600;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SessionLedger {
    pub schema_version: u32,
    pub game_name: String,
    pub pid: u32,
    pub pid_starttime: u64,
    pub started_at: u64,
    pub credited_until: u64,
}

pub trait Clock {
    fn now(&self) -> u64;
}

pub struct SystemClock;

impl Clock for SystemClock {
    fn now(&self) -> u64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0)
    }
}

/// Seconds to credit: never negative, never more than a day at once.
pub fn delta(credited_until: u64, now: u64) -> u64 {
    if now <= credited_until {
        return 0;
    }
    (now - credited_until).min(MAX_DELTA_SECS)
}

/// Same policy as `GameModel::merge_time_spent` (#74): never lower a value.
pub fn merge_time_spent(existing: u64, incoming: u64) -> u64 {
    existing.max(incoming)
}

impl SessionLedger {
    pub fn new(game_name: &str, pid: u32, pid_starttime: u64, now: u64) -> Self {
        Self {
            schema_version: SCHEMA_VERSION,
            game_name: game_name.to_string(),
            pid,
            pid_starttime,
            started_at: now,
            credited_until: now,
        }
    }

    pub fn to_json(&self) -> String {
        serde_json::json!({
            "schema_version": self.schema_version,
            "game_name": self.game_name,
            "pid": self.pid,
            "pid_starttime": self.pid_starttime,
            "started_at": self.started_at,
            "credited_until": self.credited_until,
        })
        .to_string()
    }

    pub fn from_json(text: &str) -> Option<Self> {
        let v: serde_json::Value = serde_json::from_str(text).ok()?;
        if v.get("schema_version")?.as_u64()? != SCHEMA_VERSION as u64 {
            return None;
        }
        Some(Self {
            schema_version: SCHEMA_VERSION,
            game_name: v.get("game_name")?.as_str()?.to_string(),
            pid: v.get("pid")?.as_u64()? as u32,
            pid_starttime: v.get("pid_starttime")?.as_u64()?,
            started_at: v.get("started_at")?.as_u64()?,
            credited_until: v.get("credited_until")?.as_u64()?,
        })
    }
}

/// `~/.local/share/CorkyTux/sessions`, created with mode 0700.
pub fn sessions_dir(home: &Path) -> PathBuf {
    home.join(".local").join("share").join("CorkyTux").join("sessions")
}

pub fn ensure_dir(dir: &Path) -> std::io::Result<()> {
    fs::create_dir_all(dir)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(dir, fs::Permissions::from_mode(0o700))?;
    }
    Ok(())
}

pub fn ledger_path(dir: &Path, pid: u32) -> PathBuf {
    dir.join(format!("{}.json", pid))
}

/// Atomic write (temp in the same dir + rename), mode 0600.
pub fn save(dir: &Path, ledger: &SessionLedger) -> Result<(), String> {
    ensure_dir(dir).map_err(|e| format!("cannot create sessions dir: {}", e))?;
    let target = ledger_path(dir, ledger.pid);
    let tmp = dir.join(format!(".{}.tmp", ledger.pid));
    {
        let mut f = fs::File::create(&tmp).map_err(|e| format!("cannot write ledger: {}", e))?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = f.set_permissions(fs::Permissions::from_mode(0o600));
        }
        f.write_all(ledger.to_json().as_bytes())
            .map_err(|e| format!("cannot write ledger: {}", e))?;
        f.sync_all().ok();
    }
    fs::rename(&tmp, &target).map_err(|e| format!("cannot rename ledger: {}", e))
}

pub fn load(dir: &Path, pid: u32) -> Option<SessionLedger> {
    let text = fs::read_to_string(ledger_path(dir, pid)).ok()?;
    SessionLedger::from_json(&text)
}

pub fn remove(dir: &Path, pid: u32) {
    let _ = fs::remove_file(ledger_path(dir, pid));
}

/// Moving a corrupt ledger to `*.bad` keeps it for inspection and makes the
/// next recovery skip it. Never panics.
pub fn move_bad(dir: &Path, path: &Path) {
    let bad = path.with_extension("json.bad");
    let _ = fs::rename(path, bad);
}

/// Credit one checkpoint: compute the delta, credit it, then move
/// `credited_until`. `credit` returns Ok only when the write landed.
pub fn checkpoint<C: Clock>(
    dir: &Path,
    pid: u32,
    clock: &C,
    credit: &mut dyn FnMut(&str, u64) -> Result<(), String>,
) -> Result<u64, String> {
    let Some(mut ledger) = load(dir, pid) else {
        return Ok(0);
    };
    let now = clock.now();
    let d = delta(ledger.credited_until, now);
    if d > 0 {
        credit(&ledger.game_name, d)?;
        ledger.credited_until = now;
        save(dir, &ledger)?;
    }
    Ok(d)
}

/// Delete the ledger on a clean close. The existing launcher path already
/// banks the full session time (`increment_time_spent(secs)`), so crediting
/// here as well would double count. The periodic checkpoint is the crash
/// safety net; the clean-close bank stays where it is (Tarea 5 moves it).
pub fn finish(dir: &Path, pid: u32) -> Result<(), String> {
    remove(dir, pid);
    Ok(())
}

/// Recovery at startup. `alive` reports whether the recorded pid is still
/// the same process (same starttime).
pub fn recover<C: Clock>(
    dir: &Path,
    clock: &C,
    alive: &dyn Fn(u32, u64) -> bool,
    credit: &mut dyn FnMut(&str, u64) -> Result<(), String>,
) -> Result<usize, String> {
    let _ = clock;
    let entries = match fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return Ok(0),
    };
    let mut handled = 0;
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("json") {
            continue;
        }
        let text = match fs::read_to_string(&path) {
            Ok(t) => t,
            Err(_) => continue,
        };
        let Some(ledger) = SessionLedger::from_json(&text) else {
            move_bad(dir, &path);
            continue;
        };
        if alive(ledger.pid, ledger.pid_starttime) {
            eprintln!(
                "[session] live session left for adoption: {} (pid {})",
                ledger.game_name, ledger.pid
            );
            continue;
        }
        // The last checkpoint already credited up to `credited_until`. Do
        // not invent the crash gap, and do not credit again (idempotent).
        let _ = credit;
        remove(dir, ledger.pid);
        handled += 1;
    }
    Ok(handled)
}

/// Read field 22 of `/proc/<pid>/stat` and compare it with the ledger.
pub fn pid_alive_with_starttime(pid: u32, starttime: u64) -> bool {
    let Ok(stat) = fs::read_to_string(format!("/proc/{}/stat", pid)) else {
        return false;
    };
    let Some(rp) = stat.rfind(')') else { return false };
    let rest: Vec<&str> = stat[rp + 1..].split_whitespace().collect();
    if rest.len() < 20 {
        return false;
    }
    rest[19].parse::<u64>().map(|s| s == starttime).unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;
    use std::rc::Rc;

    struct FakeClock(Rc<RefCell<u64>>);
    impl Clock for FakeClock {
        fn now(&self) -> u64 {
            *self.0.borrow()
        }
    }

    struct Creditor {
        total: u64,
        calls: Vec<(String, u64)>,
    }
    impl Creditor {
        fn new() -> Self {
            Self { total: 0, calls: Vec::new() }
        }
        fn credit(&mut self, game: &str, d: u64) -> Result<(), String> {
            self.total = merge_time_spent(self.total, self.total + d);
            self.calls.push((game.to_string(), d));
            Ok(())
        }
    }

    fn tmp(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(name);
        let _ = fs::remove_dir_all(&d);
        d
    }

    #[test]
    fn create_checkpoint_accumulates_without_double_count() {
        let dir = tmp("ledger-accum");
        let clock = FakeClock(Rc::new(RefCell::new(1000)));
        let mut c = Creditor::new();
        save(&dir, &SessionLedger::new("Game", 42, 7, clock.now())).unwrap();
        *clock.0.borrow_mut() = 1060;
        assert_eq!(checkpoint(&dir, 42, &clock, &mut |g, d| c.credit(g, d)).unwrap(), 60);
        *clock.0.borrow_mut() = 1120;
        assert_eq!(checkpoint(&dir, 42, &clock, &mut |g, d| c.credit(g, d)).unwrap(), 60);
        assert_eq!(c.total, 120);
        assert_eq!(load(&dir, 42).unwrap().credited_until, 1120);
    }

    #[test]
    fn recovery_dead_pid_does_not_credit_beyond_checkpoint() {
        let dir = tmp("ledger-dead");
        let clock = FakeClock(Rc::new(RefCell::new(1000)));
        let mut c = Creditor::new();
        save(&dir, &SessionLedger::new("Game", 42, 7, clock.now())).unwrap();
        *clock.0.borrow_mut() = 1060;
        checkpoint(&dir, 42, &clock, &mut |g, d| c.credit(g, d)).unwrap();
        let before = c.total;
        *clock.0.borrow_mut() = 9999;
        let n = recover(&dir, &clock, &|_, _| false, &mut |g, d| c.credit(g, d)).unwrap();
        assert_eq!(n, 1);
        assert_eq!(c.total, before);
        assert!(load(&dir, 42).is_none());
    }

    #[test]
    fn recovery_live_pid_leaves_file_untouched() {
        let dir = tmp("ledger-live");
        let clock = FakeClock(Rc::new(RefCell::new(1000)));
        let mut c = Creditor::new();
        save(&dir, &SessionLedger::new("Game", 42, 7, clock.now())).unwrap();
        let n = recover(&dir, &clock, &|pid, st| pid == 42 && st == 7, &mut |g, d| c.credit(g, d)).unwrap();
        assert_eq!(n, 0);
        assert_eq!(load(&dir, 42).unwrap().credited_until, 1000);
        assert_eq!(c.total, 0);
    }

    #[test]
    fn recovery_reused_pid_is_treated_as_dead() {
        let dir = tmp("ledger-reuse");
        let clock = FakeClock(Rc::new(RefCell::new(1000)));
        let mut c = Creditor::new();
        save(&dir, &SessionLedger::new("Game", 42, 7, clock.now())).unwrap();
        let n = recover(&dir, &clock, &|pid, st| pid == 42 && st == 99, &mut |g, d| c.credit(g, d)).unwrap();
        assert_eq!(n, 1);
        assert!(load(&dir, 42).is_none());
    }

    #[test]
    fn recovery_twice_is_idempotent() {
        let dir = tmp("ledger-twice");
        let clock = FakeClock(Rc::new(RefCell::new(1000)));
        let mut c = Creditor::new();
        save(&dir, &SessionLedger::new("Game", 42, 7, clock.now())).unwrap();
        recover(&dir, &clock, &|_, _| false, &mut |g, d| c.credit(g, d)).unwrap();
        recover(&dir, &clock, &|_, _| false, &mut |g, d| c.credit(g, d)).unwrap();
        assert_eq!(c.total, 0);
    }

    #[test]
    fn clock_back_does_not_subtract() {
        let dir = tmp("ledger-back");
        let clock = FakeClock(Rc::new(RefCell::new(1000)));
        let mut c = Creditor::new();
        save(&dir, &SessionLedger::new("Game", 42, 7, clock.now())).unwrap();
        *clock.0.borrow_mut() = 900;
        assert_eq!(checkpoint(&dir, 42, &clock, &mut |g, d| c.credit(g, d)).unwrap(), 0);
        assert_eq!(c.total, 0);
    }

    #[test]
    fn corrupt_json_goes_bad_without_panic() {
        let dir = tmp("ledger-corrupt");
        ensure_dir(&dir).unwrap();
        fs::write(ledger_path(&dir, 42), "{not json").unwrap();
        let clock = FakeClock(Rc::new(RefCell::new(1000)));
        let mut c = Creditor::new();
        let n = recover(&dir, &clock, &|_, _| false, &mut |g, d| c.credit(g, d)).unwrap();
        assert_eq!(n, 0);
        assert!(!ledger_path(&dir, 42).exists());
        assert!(dir.join("42.json.bad").exists());
    }

    #[test]
    fn checkpoint_respects_max_policy() {
        let dir = tmp("ledger-max");
        let clock = FakeClock(Rc::new(RefCell::new(1000)));
        let mut existing = 5000u64;
        save(&dir, &SessionLedger::new("Game", 42, 7, clock.now())).unwrap();
        *clock.0.borrow_mut() = 1060;
        checkpoint(&dir, 42, &clock, &mut |_g, d| {
            existing = merge_time_spent(existing, existing.saturating_add(d));
            Ok(())
        }).unwrap();
        assert_eq!(existing, 5060);
    }

    #[cfg(unix)]
    #[test]
    fn permissions_are_private() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tmp("ledger-perm");
        let clock = FakeClock(Rc::new(RefCell::new(1000)));
        save(&dir, &SessionLedger::new("Game", 42, 7, clock.now())).unwrap();
        let dm = fs::metadata(&dir).unwrap().permissions().mode() & 0o777;
        let fm = fs::metadata(ledger_path(&dir, 42)).unwrap().permissions().mode() & 0o777;
        assert_eq!(dm, 0o700);
        assert_eq!(fm, 0o600);
    }
}

/// Field 22 of `/proc/<pid>/stat` (start time in clock ticks).
pub fn pid_starttime(pid: u32) -> Option<u64> {
    let stat = fs::read_to_string(format!("/proc/{}/stat", pid)).ok()?;
    let rp = stat.rfind(')')?;
    let rest: Vec<&str> = stat[rp + 1..].split_whitespace().collect();
    if rest.len() < 20 {
        return None;
    }
    rest[19].parse::<u64>().ok()
}

#[cfg(test)]
mod extra_tests {
    #[test]
    fn delta_clamps_to_a_day() {
        assert_eq!(super::delta(0, super::MAX_DELTA_SECS + 5), super::MAX_DELTA_SECS);
        assert_eq!(super::delta(10, 5), 0);
        assert_eq!(super::delta(10, 70), 60);
    }
}
