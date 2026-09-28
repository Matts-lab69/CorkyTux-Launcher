//! Safe migration of `install_path` and `prefix_path` into the Games dir.
//!
//! My permanent import never deletes before verifying. The sequence is: copy
//! with `cp -a` to a staging dir INSIDE Games (same filesystem, which is why
//! the final rename is atomic), verify size and executable, publish with a
//! rename, and only then retire the original, leaving a symlink at its old
//! path so Heroic and Lutris keep working.
//!
//! I keep `preflight` and `plan_for_mode` separate on purpose: the user picks
//! the mode after seeing the blockers, and only then do I decide what really
//! moves. `execute` is my only point that touches the disk.

use std::collections::{BTreeMap, BTreeSet};
use std::ffi::CString;
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::symlink;
use std::path::{Component, Path, PathBuf};
use std::process::Command;

/// Headroom over the total to move. The copy lives inside Games while the
/// original is still there, so worst case needs double: my margin covers the
/// staging dir plus filesystem metadata.
pub const SPACE_MARGIN_BYTES: u64 = 512 * 1024 * 1024;

/// Staging temp dir. It must sit in Games: under /tmp the move to the final
/// destination would be a copy, not a rename, and I would lose atomicity.
pub const STAGE_DIR: &str = ".corkytux-import";

/// Suffix of the retired original during the rename -> symlink window.
pub const OLD_SUFFIX: &str = "corkytux-old";

/// Mode the user picks in the Import Manager.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ImportMode {
    /// Current behavior: I only register, move nothing.
    Test,
    /// I move games that share no prefix. Shared ones stay in Test mode.
    Permanent,
    /// I also move shared-prefix games, all together.
    PermanentWithSharedGroups,
}

/// Why a candidate stays out of `plans`.
#[derive(Clone, Debug)]
pub enum Blocker {
    GameRunning { label: String },
    /// Global, carries no label: if it fits nothing, I move nothing.
    NotEnoughSpace { need_bytes: u64, free_bytes: u64 },
    SharedPrefix { label: String, shared_with: Vec<String> },
    SourceMissing { label: String, path: PathBuf },
    /// Original retired to `.corkytux-old`, but the session died before I
    /// placed the symlink. The original is intact at `path`.
    OrphanedOriginal { label: String, path: PathBuf },
    /// I could not tell whether the prefix is in use, so I move nothing.
    /// I block on purpose: a pending import beats lost data.
    UsageUndeterminable { label: String, path: PathBuf },
}

impl Blocker {
    pub fn message(&self) -> String {
        match self {
            Blocker::GameRunning { label } => {
                format!("{} has a live wineserver on its prefix", label)
            }
            Blocker::NotEnoughSpace { need_bytes, free_bytes } => format!(
                "does not fit in Games: needs {} but only {} free",
                human_bytes(*need_bytes),
                human_bytes(*free_bytes)
            ),
            Blocker::SharedPrefix { label, shared_with } => {
                format!("{} shares a prefix with {}", label, shared_with.join(", "))
            }
            Blocker::SourceMissing { label, path } => {
                format!("{} folder is missing ({})", label, path.display())
            }
            Blocker::OrphanedOriginal { label, path } => {
                format!("{} has an unfinished original waiting at {}", label, path.display())
            }
            Blocker::UsageUndeterminable { label, path } => format!(
                "could not tell whether {} is in use ({}); close Wine games and retry",
                label,
                path.display()
            ),
        }
    }
}

/// A game I may migrate.
#[derive(Clone, Debug)]
pub struct MoveCandidate {
    pub label: String,
    pub store_tag: String,
    pub install_path: PathBuf,
    /// REAL source prefix. `None` for Heroic: the prefix_path I register for
    /// stores belongs to them, not the launcher, and I leave it alone.
    pub prefix_path: Option<PathBuf>,
    /// Main executable, already resolved, so I can verify it in the copy.
    pub executable: PathBuf,
}

/// Physical move of one prefix. In a shared group I keep a SINGLE
/// PrefixMove for all members: the prefix moves once.
#[derive(Clone, Debug)]
pub struct PrefixMove {
    pub src: PathBuf,
    pub dest: PathBuf,
    pub bytes: u64,
}

/// One game with its destination resolved.
#[derive(Clone, Debug)]
pub struct MovePlan {
    pub candidate: MoveCandidate,
    /// Rough size of this plan. `preflight` computes the authoritative
    /// total with each prefix counted once.
    pub bytes: u64,
    pub dest_install: PathBuf,
    /// This game's own prefix. `None` when there is no real prefix, or when
    /// a `GroupPlan` carries it (it moves once for everyone).
    pub prefix_move: Option<PrefixMove>,
    /// `Some(relative)` when `install_path` lives INSIDE `prefix_path`. No
    /// copy of its own: the data travels with the prefix and I only change
    /// the configured path.
    pub nested_rel: Option<String>,
}

impl MovePlan {
    pub fn is_nested(&self) -> bool {
        self.nested_rel.is_some()
    }

    /// Final `install_path` once the move completes.
    pub fn new_install_path(&self) -> PathBuf {
        match &self.nested_rel {
            Some(rel) => match &self.prefix_move {
                Some(pm) => pm.dest.join(rel),
                None => PathBuf::new(),
            },
            None => self.dest_install.clone(),
        }
    }

    /// Final `prefix_path`, or `None` when this game has no real prefix.
    pub fn new_prefix_path(&self) -> Option<PathBuf> {
        self.prefix_move.as_ref().map(|pm| pm.dest.clone())
    }
}

/// Result of my pre-checks.
#[derive(Clone, Debug)]
pub struct Preflight {
    pub plans: Vec<MovePlan>,
    pub blockers: Vec<Blocker>,
}

/// Equivalence class of shared prefixes.
#[derive(Clone, Debug)]
pub struct SharedGroup {
    pub group_id: String,
    pub members: Vec<String>,
    pub prefix_path: PathBuf,
}

/// The group as one unit: the prefix moves once, the installs N times.
#[derive(Clone, Debug)]
pub struct GroupPlan {
    pub group_id: String,
    pub prefix_move: Option<PrefixMove>,
    pub members: Vec<MovePlan>,
}

/// What the UI receives once the mode is picked.
#[derive(Clone, Debug, Default)]
pub struct ExecPlan {
    pub singles: Vec<MovePlan>,
    pub groups: Vec<GroupPlan>,
    pub test_only: Vec<String>,
}

/// Per-game result. The UI decides what to do with successes.
#[derive(Clone, Debug)]
pub enum MoveOutcome {
    Clean,
    WithWarning(String),
    Failed(String),
}

impl MoveOutcome {
    pub fn is_ok(&self) -> bool {
        !matches!(self, MoveOutcome::Failed(_))
    }

    /// No caller yet: a symmetric accessor to `Blocker::message` that I keep
    /// for debugging results without rebuilding the enum.
    #[allow(dead_code)]
    pub fn message(&self) -> Option<&str> {
        match self {
            MoveOutcome::Clean => None,
            MoveOutcome::WithWarning(m) | MoveOutcome::Failed(m) => Some(m.as_str()),
        }
    }
}

// --- path helpers ---

/// I normalize for comparison: canonicalize when it exists, else clean up.
pub fn norm(p: &Path) -> PathBuf {
    std::fs::canonicalize(p).unwrap_or_else(|_| clean_path(p))
}

fn clean_path(p: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for c in p.components() {
        match c {
            Component::CurDir => {}
            Component::ParentDir => {
                if !out.pop() {
                    out.push("..");
                }
            }
            other => out.push(other.as_os_str()),
        }
    }
    if out.as_os_str().is_empty() {
        PathBuf::from(".")
    } else {
        out
    }
}

/// Does `inner` live inside `outer`? I compare by components, not text
/// prefix: `/games/quake` is NOT inside `/games/quake2`.
pub fn nested_in(inner: &Path, outer: &Path) -> bool {
    let a = norm(inner);
    let b = norm(outer);
    a != b && a.starts_with(&b)
}

/// Safe folder name from a title.
pub fn sanitize(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for ch in s.chars() {
        if ch.is_alphanumeric() || ch == ' ' || ch == '-' || ch == '_' || ch == '.' {
            out.push(ch);
        } else {
            out.push('-');
        }
    }
    let joined = out.split_whitespace().collect::<Vec<_>>().join(" ");
    let trimmed = joined.trim_matches(|c| c == '.' || c == '-');
    if trimmed.is_empty() {
        "game".to_string()
    } else {
        trimmed.chars().take(96).collect()
    }
}

/// First free destination: `Name`, then `Name (GOG)`, `Name (GOG 2)`.
pub fn unique_dest(games_dir: &Path, label: &str, tag: &str) -> PathBuf {
    let base = sanitize(label);
    let first = games_dir.join(&base);
    if !first.exists() {
        return first;
    }
    let tag = sanitize(tag);
    let second = games_dir.join(format!("{} ({})", base, tag));
    if !second.exists() {
        return second;
    }
    for n in 2..10_000 {
        let cand = games_dir.join(format!("{} ({} {})", base, tag, n));
        if !cand.exists() {
            return cand;
        }
    }
    games_dir.join(format!("{} ({} overflow)", base, tag))
}

/// Free sibling of `base` with a suffix: `base-prefix`, `base-prefix 2`...
/// I always keep a game prefix next to the game folder.
fn unique_sibling(base: &Path, suffix: &str) -> PathBuf {
    let parent = base.parent().map(|p| p.to_path_buf()).unwrap_or_else(|| PathBuf::from("."));
    let stem = base
        .file_name()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| "game".to_string());
    let first = parent.join(format!("{}-{}", stem, suffix));
    if !first.exists() {
        return first;
    }
    for n in 2..10_000 {
        let cand = parent.join(format!("{}-{} {}", stem, suffix, n));
        if !cand.exists() {
            return cand;
        }
    }
    parent.join(format!("{}-{} overflow", stem, suffix))
}

/// Free child inside a group directory.
fn unique_child(dir: &Path, label: &str) -> PathBuf {
    let base = sanitize(label);
    let first = dir.join(&base);
    if !first.exists() {
        return first;
    }
    for n in 2..10_000 {
        let cand = dir.join(format!("{} {}", base, n));
        if !cand.exists() {
            return cand;
        }
    }
    dir.join(format!("{} overflow", base))
}

/// Total size of a tree. Symlinks count their own size, never the target's:
/// `symlink_metadata` follows no links. Folders add 0, so my source/copy
/// comparison stays symmetric.
pub fn dir_size(path: &Path) -> u64 {
    let mut total: u64 = 0;
    let mut stack = vec![path.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let rd = match std::fs::read_dir(&dir) {
            Ok(r) => r,
            Err(_) => continue,
        };
        for ent in rd.flatten() {
            match ent.file_type() {
                Ok(ft) if ft.is_dir() => stack.push(ent.path()),
                _ => {
                    if let Ok(md) = std::fs::symlink_metadata(ent.path()) {
                        total = total.saturating_add(md.len());
                    }
                }
            }
        }
    }
    total
}

/// Free space on the filesystem holding `path`, via statvfs.
pub fn free_bytes(path: &Path) -> std::io::Result<u64> {
    let c = CString::new(path.as_os_str().as_bytes())
        .map_err(|_| std::io::Error::new(std::io::ErrorKind::InvalidInput, "path with NUL"))?;
    let mut st: libc::statvfs = unsafe { std::mem::zeroed() };
    let rc = unsafe { libc::statvfs(c.as_ptr(), &mut st) };
    if rc != 0 {
        return Err(std::io::Error::last_os_error());
    }
    let avail = st.f_bavail as u64;
    let bsize = st.f_frsize as u64;
    Ok(avail.saturating_mul(bsize))
}

/// Prefix occupancy from live system processes.
///
/// I keep `Unknown` so "could not check" never reads as "free": moving a
/// prefix while its game runs loses data, so on doubt I block the import.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PrefixUsage {
    /// A wineserver is holding THIS prefix.
    Busy,
    /// Checked for sure: no wineserver points at this prefix.
    Free,
    /// Undetermined. I never treat it as free.
    Unknown,
}

/// PIDs whose process name is `wineserver`, read from `/proc`.
///
/// I avoid `pgrep` on purpose: it comes from procps, not POSIX, and is missing
/// from the default NixOS PATH and minimal containers. With `pgrep` absent my
/// old code returned "prefix free" and the import moved dirs in use.
///
/// I match `comm` by substring, not equality, to keep variants
/// (`wineserver-preloader`). The `wineserver` prefix appears in neither
/// CorkyTux's name nor its processes, so there is no self-match. A matching
/// process that exposes no `WINEPREFIX` counts as unattributable, which blocks
/// instead of freeing.
fn wineserver_pids() -> Option<Vec<u32>> {
    let entries = std::fs::read_dir("/proc").ok()?;
    let mut pids = Vec::new();
    for entry in entries.flatten() {
        let name = entry.file_name();
        let Some(name) = name.to_str() else { continue };
        if name.is_empty() || !name.bytes().all(|b| b.is_ascii_digit()) {
            continue;
        }
        // El proceso puede desaparecer entre readdir y read.
        let Ok(pid) = name.parse::<u32>() else { continue };
        let comm = std::fs::read(format!("/proc/{}/comm", pid)).unwrap_or_default();
        if String::from_utf8_lossy(&comm).contains("wineserver") {
            pids.push(pid);
        }
    }
    Some(pids)
}

/// `WINEPREFIX` declared by the process, read from `/proc/<pid>/environ`.
///
/// This is my exact attribution: wineserver carries no prefix on its command
/// line (it gets it via environment), so comparing `/proc/<pid>/cmdline`
/// against the prefix was unreliable.
fn wineprefix_of(pid: u32) -> Option<PathBuf> {
    let raw = std::fs::read(format!("/proc/{}/environ", pid)).ok()?;
    for entry in raw.split(|b| *b == 0) {
        if entry.is_empty() {
            continue;
        }
        if let Ok(s) = std::str::from_utf8(entry) {
            if let Some(value) = s.strip_prefix("WINEPREFIX=") {
                if !value.is_empty() {
                    return Some(PathBuf::from(value));
                }
            }
        }
    }
    None
}

/// Is a live wineserver holding this prefix?
///
/// Same bar as `proton.rs` (lock + pgrep), but I skip the `pgrep` dependency
/// and attribute exactly via `WINEPREFIX`.
pub fn prefix_usage(prefix: &Path) -> PrefixUsage {
    let Some(pids) = wineserver_pids() else {
        // Without /proc there is no way to check: I never assume free.
        return PrefixUsage::Unknown;
    };
    if pids.is_empty() {
        return PrefixUsage::Free;
    }
    let want = norm(prefix);
    let mut unattributed = 0usize;
    for pid in pids {
        match wineprefix_of(pid) {
            Some(found) => {
                if norm(&found) == want {
                    return PrefixUsage::Busy;
                }
            }
            // A live wineserver whose environment I cannot read (another
            // user, procfs with hidepid) stops me from claiming this prefix
            // is free: I block with a warning instead of risking data.
            None => unattributed += 1,
        }
    }
    if unattributed > 0 {
        PrefixUsage::Unknown
    } else {
        PrefixUsage::Free
    }
}

/// Staging temp root, one dir per process.
pub fn stage_root(games_dir: &Path) -> PathBuf {
    games_dir.join(STAGE_DIR).join(format!("pid-{}", std::process::id()))
}

/// Human-readable bytes for the UI.
pub fn human_bytes(n: u64) -> String {
    const UNITS: [&str; 4] = ["B", "KB", "MB", "GB"];
    let mut v = n as f64;
    let mut i = 0usize;
    while v >= 1024.0 && i + 1 < UNITS.len() {
        v /= 1024.0;
        i += 1;
    }
    if i == 0 {
        format!("{} {}", n, UNITS[0])
    } else {
        format!("{:.1} {}", v, UNITS[i])
    }
}

// --- preflight ---

/// Pre-checks. I decide which candidates enter `plans` and which stay
/// blocked. I touch no disk: reads only.
pub fn preflight(cands: &[MoveCandidate], games_dir: &Path) -> Preflight {
    let mut plans: Vec<MovePlan> = Vec::new();
    let mut blockers: Vec<Blocker> = Vec::new();

    // Step A: real prefixes, grouped by normalized path.
    let mut by_prefix: BTreeMap<PathBuf, Vec<&MoveCandidate>> = BTreeMap::new();
    for c in cands {
        if let Some(p) = &c.prefix_path {
            if p.exists() {
                by_prefix.entry(norm(p)).or_default().push(c);
            }
        }
    }
    let shared: BTreeSet<PathBuf> = by_prefix
        .iter()
        .filter(|(_, v)| v.len() > 1)
        .map(|(k, _)| k.clone())
        .collect();

    // Step B: per-candidate checks.
    // Crash detection for a previous session: with an orphaned .corkytux-old
    // I compute nothing else for that game.
    let orphans: BTreeMap<String, PathBuf> = find_orphaned_originals(cands).into_iter().collect();
    for c in cands {
        if let Some(old) = orphans.get(&c.label) {
            blockers.push(Blocker::OrphanedOriginal {
                label: c.label.clone(),
                path: old.clone(),
            });
            continue;
        }
        if !c.install_path.exists() {
            blockers.push(Blocker::SourceMissing {
                label: c.label.clone(),
                path: c.install_path.clone(),
            });
            continue;
        }
        if let Some(p) = &c.prefix_path {
            if p.exists() {
                match prefix_usage(p) {
                    PrefixUsage::Busy => {
                        blockers.push(Blocker::GameRunning {
                            label: c.label.clone(),
                        });
                        continue;
                    }
                    // Fail-closed: without proof the prefix is free,
                    // I move nothing.
                    PrefixUsage::Unknown => {
                        blockers.push(Blocker::UsageUndeterminable {
                            label: c.label.clone(),
                            path: p.clone(),
                        });
                        continue;
                    }
                    PrefixUsage::Free => {}
                }
            }
        }

        let dest_install = unique_dest(games_dir, &c.label, &c.store_tag);
        let shared_prefix = match &c.prefix_path {
            Some(p) if p.exists() => shared.contains(&norm(p)),
            _ => false,
        };

        // The group moves the prefix when shared; otherwise this plan owns
        // it and moves it itself.
        let prefix_move = match &c.prefix_path {
            Some(p) if p.exists() && !shared_prefix => Some(PrefixMove {
                src: p.clone(),
                dest: unique_sibling(&dest_install, "prefix"),
                bytes: dir_size(p),
            }),
            _ => None,
        };

        // D1: install_path inside prefix_path. I do not block: the data
        // travels with the prefix and I only change the configured path.
        let nested_rel = match (&c.prefix_path, &prefix_move) {
            (Some(p), Some(pm)) if nested_in(&c.install_path, p) => {
                norm(&c.install_path)
                    .strip_prefix(&norm(&pm.src))
                    .ok()
                    .map(|r| r.display().to_string())
            }
            _ => None,
        };

        let bytes = dir_size(&c.install_path).saturating_add(
            prefix_move
                .as_ref()
                .map(|pm| pm.bytes)
                .unwrap_or(0),
        );

        plans.push(MovePlan {
            candidate: c.clone(),
            bytes,
            dest_install: if nested_rel.is_some() {
                prefix_move
                    .as_ref()
                    .map(|pm| pm.dest.clone())
                    .unwrap_or_else(|| dest_install.clone())
            } else {
                dest_install
            },
            prefix_move,
            nested_rel,
        });
    }

    // Step C: one SharedPrefix blocker per member, naming the others. The
    // UI rebuilds groups with `groups_of` so `Preflight` needs no extra field.
    for pfx in &shared {
        let members: Vec<String> = by_prefix[pfx].iter().map(|c| c.label.clone()).collect();
        for m in &members {
            let others: Vec<String> = members.iter().filter(|x| *x != m).cloned().collect();
            blockers.push(Blocker::SharedPrefix {
                label: m.clone(),
                shared_with: others,
            });
        }
    }

    // Step D: global space, each prefix counted ONCE.
    let mut seen: BTreeSet<PathBuf> = BTreeSet::new();
    let mut need: u64 = 0;
    for c in cands {
        if c.install_path.exists() {
            need = need.saturating_add(dir_size(&c.install_path));
        }
        if let Some(p) = &c.prefix_path {
            let n = norm(p);
            if p.exists() && seen.insert(n) {
                need = need.saturating_add(dir_size(p));
            }
        }
    }
    let need_total = need.saturating_add(SPACE_MARGIN_BYTES);
    match free_bytes(games_dir) {
        Ok(free) => {
            if free < need_total {
                blockers.push(Blocker::NotEnoughSpace {
                    need_bytes: need_total,
                    free_bytes: free,
                });
                // Global: if it fits nothing, I move nothing.
                plans.clear();
            }
        }
        Err(e) => eprintln!("[import] statvfs failed on {}: {}", games_dir.display(), e),
    }

    Preflight { plans, blockers }
}

fn uf_find(parent: &mut BTreeMap<String, String>, x: &str) -> String {
    if !parent.contains_key(x) {
        parent.insert(x.to_string(), x.to_string());
    }
    let mut cur = x.to_string();
    // Union-by-roots cannot cycle, but I still guard the walk so corrupt
    // data never hangs the import.
    let mut guard = parent.len() + 1;
    while guard > 0 {
        guard -= 1;
        let nxt = parent.get(&cur).cloned().unwrap_or_default();
        if nxt.is_empty() || nxt == cur {
            break;
        }
        cur = nxt;
    }
    cur
}

fn uf_union(parent: &mut BTreeMap<String, String>, a: &str, b: &str) {
    let ra = uf_find(parent, a);
    let rb = uf_find(parent, b);
    if ra != rb {
        parent.insert(rb, ra);
    }
}

/// I rebuild shared groups from the blockers, no extra `Preflight` fields.
/// I union by equivalence classes over the names.
pub fn groups_of(cands: &[MoveCandidate], blockers: &[Blocker]) -> Vec<SharedGroup> {
    let mut parent: BTreeMap<String, String> = BTreeMap::new();
    for b in blockers {
        if let Blocker::SharedPrefix { label, shared_with } = b {
            for o in shared_with {
                uf_union(&mut parent, label, o);
            }
        }
    }

    let mut by_label: BTreeMap<String, PathBuf> = BTreeMap::new();
    for c in cands {
        if let Some(p) = &c.prefix_path {
            if p.exists() {
                by_label.insert(c.label.clone(), norm(p));
            }
        }
    }

    let mut comps: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for b in blockers {
        if let Blocker::SharedPrefix { label, .. } = b {
            let root = uf_find(&mut parent, label);
            comps.entry(root).or_default().push(label.clone());
        }
    }

    let mut out: Vec<SharedGroup> = Vec::new();
    for (_, mut members) in comps {
        members.sort();
        members.dedup();
        if members.len() < 2 {
            continue;
        }
        let gid = format!("{}-shared", sanitize(&members[0]));
        let pfx = members
            .iter()
            .filter_map(|m| by_label.get(m))
            .next()
            .cloned()
            .unwrap_or_default();
        out.push(SharedGroup {
            group_id: gid,
            members,
            prefix_path: pfx,
        });
    }
    out.sort_by(|a, b| a.group_id.cmp(&b.group_id));
    out
}

// --- plan per mode ---

/// I translate the picked mode into an executable plan. In
/// `PermanentWithSharedGroups` I revalidate each group: something may have
/// started between preflight and confirmation.
pub fn plan_for_mode(
    pf: &Preflight,
    cands: &[MoveCandidate],
    games_dir: &Path,
    mode: ImportMode,
) -> ExecPlan {
    let mut ep = ExecPlan {
        singles: Vec::new(),
        groups: Vec::new(),
        test_only: Vec::new(),
    };

    let space_blocked = pf
        .blockers
        .iter()
        .any(|b| matches!(b, Blocker::NotEnoughSpace { .. }));

    // Without space, or in Test mode, I move nothing.
    if space_blocked || mode == ImportMode::Test {
        ep.test_only = cands.iter().map(|c| c.label.clone()).collect();
        return ep;
    }

    let groups = groups_of(cands, &pf.blockers);
    let mut in_group: BTreeSet<String> = BTreeSet::new();
    for g in &groups {
        for m in &g.members {
            in_group.insert(m.clone());
        }
    }

    // Clean non-group entries go as singles in both permanent modes: a
    // group never blocks the rest of the selection.
    for p in &pf.plans {
        if !in_group.contains(&p.candidate.label) {
            ep.singles.push(p.clone());
        }
    }

    if mode == ImportMode::Permanent {
        for c in cands {
            if in_group.contains(&c.label) {
                ep.test_only.push(c.label.clone());
            }
        }
        ep.test_only.sort();
        return ep;
    }

    // PermanentWithSharedGroups. I spend the budget group by group: two
    // groups may fit apart but not together.
    let mut budget = match free_bytes(games_dir) {
        Ok(f) => f,
        Err(e) => {
            eprintln!("[import] statvfs fallo en {}: {}", games_dir.display(), e);
            0
        }
    };

    for g in &groups {
        let members: Vec<&MoveCandidate> = cands
            .iter()
            .filter(|c| g.members.contains(&c.label))
            .collect();

        // Revalidation. If one member fails, the WHOLE group stays in test:
        // splitting a shared prefix across two places is exactly the bug I
        // prevent here.
        let mut bad: Option<String> = None;
        if !g.prefix_path.as_os_str().is_empty() {
            match prefix_usage(&g.prefix_path) {
                PrefixUsage::Busy => {
                    bad = Some(format!(
                        "shared prefix {} is in use",
                        g.prefix_path.display()
                    ));
                }
                PrefixUsage::Unknown => {
                    bad = Some(format!(
                        "could not tell whether shared prefix {} is in use",
                        g.prefix_path.display()
                    ));
                }
                PrefixUsage::Free => {}
            }
        }
        if bad.is_none() {
            for m in &members {
                if !m.install_path.exists() {
                    bad = Some(format!("{} folder is missing", m.label));
                    break;
                }
            }
        }

        // Group space: prefix once + each member install.
        let mut need_g: u64 = 0;
        if !g.prefix_path.as_os_str().is_empty() && g.prefix_path.exists() {
            need_g = need_g.saturating_add(dir_size(&g.prefix_path));
        }
        for m in &members {
            let inside = !g.prefix_path.as_os_str().is_empty()
                && nested_in(&m.install_path, &g.prefix_path);
            if !inside && m.install_path.exists() {
                need_g = need_g.saturating_add(dir_size(&m.install_path));
            }
        }
        let need_total = need_g.saturating_add(SPACE_MARGIN_BYTES);
        if need_total > budget {
            bad = Some(format!(
                "group does not fit: needs {} but only {} left",
                human_bytes(need_total),
                human_bytes(budget)
            ));
        }

        if let Some(reason) = bad {
            eprintln!("[import] group {} dropped: {}", g.group_id, reason);
            for m in &members {
                ep.test_only.push(m.label.clone());
            }
            continue;
        }

        // Group layout: ~/Games/<group>/prefix + ~/Games/<group>/<game>
        let group_dir = unique_dest(games_dir, &g.group_id, "shared");
        let prefix_move = if g.prefix_path.as_os_str().is_empty() || !g.prefix_path.exists() {
            None
        } else {
            Some(PrefixMove {
                src: g.prefix_path.clone(),
                dest: group_dir.join("prefix"),
                bytes: dir_size(&g.prefix_path),
            })
        };
        let dest_prefix = prefix_move.as_ref().map(|pm| pm.dest.clone());

        let member_plans: Vec<MovePlan> = members
            .iter()
            .map(|m| {
                let inside = !g.prefix_path.as_os_str().is_empty()
                    && nested_in(&m.install_path, &g.prefix_path);
                let nested_rel = if inside {
                    norm(&m.install_path)
                        .strip_prefix(&norm(&g.prefix_path))
                        .ok()
                        .map(|r| r.display().to_string())
                } else {
                    None
                };
                let dest_install = match &nested_rel {
                    Some(rel) => dest_prefix
                        .as_ref()
                        .map(|d| d.join(rel))
                        .unwrap_or_else(|| m.install_path.clone()),
                    None => unique_child(&group_dir, &m.label),
                };
                MovePlan {
                    candidate: (*m).clone(),
                    bytes: if inside { 0 } else { dir_size(&m.install_path) },
                    dest_install,
                    prefix_move: prefix_move.clone(),
                    nested_rel,
                }
            })
            .collect();

        ep.groups.push(GroupPlan {
            group_id: g.group_id.clone(),
            prefix_move,
            members: member_plans,
        });
        budget = budget.saturating_sub(need_total);
    }

    ep.test_only.sort();
    ep
}

/// Orphans from a previous session cut between the rename and the symlink.
pub fn find_orphaned_originals(cands: &[MoveCandidate]) -> Vec<(String, PathBuf)> {
    let mut out: Vec<(String, PathBuf)> = Vec::new();
    for c in cands {
        let (Some(name), Some(parent)) = (c.install_path.file_name(), c.install_path.parent())
        else {
            continue;
        };
        let old = parent.join(format!(".{}.{}", sanitize(&name.to_string_lossy()), OLD_SUFFIX));
        if old.exists() {
            out.push((c.label.clone(), old));
        }
    }
    out
}

// --- execution ---

/// I execute the plan. Groups first: priciest and most likely to abort. I
/// touch no config: I return results and my caller updates Games.ini with
/// the ones that came out fine.
pub fn execute(ep: &ExecPlan, games_dir: &Path) -> Vec<(String, MoveOutcome)> {
    let mut out: Vec<(String, MoveOutcome)> = Vec::new();
    for g in &ep.groups {
        out.extend(execute_group(g, games_dir));
    }
    for p in &ep.singles {
        out.extend(execute_single(p, games_dir));
    }
    out
}

fn execute_single(p: &MovePlan, games_dir: &Path) -> Vec<(String, MoveOutcome)> {
    let label = p.candidate.label.clone();

    // A plan with its own prefix moves it before the install: when the
    // install sits inside the prefix, the data travels with it.
    if let Some(pm) = &p.prefix_move {
        let stage = stage_root(games_dir)
            .join(sanitize(&label))
            .join("prefix");
        if let Err(e) = move_dir(&pm.src, &pm.dest, &stage, None) {
            eprintln!("[import] {}: failed moving the prefix: {}", label, e);
            return vec![(label, MoveOutcome::Failed(e))];
        }
    }

    if p.is_nested() {
        // The data is already in the prefix copy: I only change the path.
        return vec![(label, MoveOutcome::Clean)];
    }

    let stage = stage_root(games_dir).join(sanitize(&label)).join("game");
    let verify = exe_rel(&p.candidate).map(|rel| stage.join(rel));
    let res = move_dir(
        &p.candidate.install_path,
        &p.dest_install,
        &stage,
        verify.as_deref(),
    );
    report(&label, &p.candidate.install_path, &p.dest_install, &res, verify.is_some())
}

fn execute_group(g: &GroupPlan, games_dir: &Path) -> Vec<(String, MoveOutcome)> {
    let mut out: Vec<(String, MoveOutcome)> = Vec::new();
    let group_stage = stage_root(games_dir).join(sanitize(&g.group_id));

    if let Some(pm) = &g.prefix_move {
        if let Err(e) = move_dir(&pm.src, &pm.dest, &group_stage.join("prefix"), None) {
            eprintln!("[import] group {}: prefix failed: {}", g.group_id, e);
            let msg = format!("could not move the shared prefix: {}", e);
            for m in &g.members {
                out.push((m.candidate.label.clone(), MoveOutcome::Failed(msg.clone())));
            }
            return out;
        }
    }

    for m in &g.members {
        if m.is_nested() {
            out.push((m.candidate.label.clone(), MoveOutcome::Clean));
            continue;
        }
        let stage = group_stage.join(sanitize(&m.candidate.label));
        let verify = exe_rel(&m.candidate).map(|rel| stage.join(rel));
        let res = move_dir(
            &m.candidate.install_path,
            &m.dest_install,
            &stage,
            verify.as_deref(),
        );
        out.extend(report(
            &m.candidate.label,
            &m.candidate.install_path,
            &m.dest_install,
            &res,
            verify.is_some(),
        ));
    }
    out
}

/// Executable relative to install_path. `None` when the executable IS the
/// install folder (already covered by dir_size) or falls outside it, in which
/// case there is nothing to check inside the copy.
fn exe_rel(c: &MoveCandidate) -> Option<PathBuf> {
    let ip = norm(&c.install_path);
    let ex = norm(&c.executable);
    if ex == ip {
        return None;
    }
    ex.strip_prefix(&ip).ok().map(|r| r.to_path_buf())
}

/// I copy, verify and publish. All destructive work lives in `finalize`.
fn move_dir(
    src: &Path,
    dest: &Path,
    stage: &Path,
    verify_exe: Option<&Path>,
) -> Result<Option<String>, String> {
    if !src.exists() {
        return Err(format!("source is gone: {}", src.display()));
    }
    if dest.exists() {
        return Err(format!("destination already exists: {}", dest.display()));
    }
    if let Some(p) = stage.parent() {
        std::fs::create_dir_all(p).map_err(|e| format!("could not create staging dir: {}", e))?;
    }
    if stage.exists() {
        let _ = std::fs::remove_dir_all(stage);
    }

    // F1: copia. `cp -a` preserva symlinks, permisos y bits; no lo
    // reimplementamos. Progreso indeterminado, el total al final.
    if let Err(e) = cp_a(src, stage) {
        let _ = std::fs::remove_dir_all(stage);
        return Err(e);
    }

    let res = verify_and_finalize(src, dest, stage, verify_exe);
    // I always clean the staging dir, whatever happens. After a successful
    // rename it is gone, so this is a no-op.
    if stage.exists() {
        let _ = std::fs::remove_dir_all(stage);
    }
    res
}

fn cp_a(src: &Path, dst: &Path) -> Result<(), String> {
    if dst.exists() {
        std::fs::remove_dir_all(dst).map_err(|e| format!("could not clean staging dir: {}", e))?;
    }
    // The trailing slash copies the CONTENT, hidden files included, and
    // leaves dst shaped like src.
    let status = Command::new("cp")
        .arg("-a")
        .arg(src.join("."))
        .arg(dst)
        .status()
        .map_err(|e| format!("could not run cp: {}", e))?;
    if !status.success() {
        return Err(format!("cp -a devolvio {}", status));
    }
    Ok(())
}

fn verify_and_finalize(
    orig: &Path,
    dest: &Path,
    stage: &Path,
    verify_exe: Option<&Path>,
) -> Result<Option<String>, String> {
    // Step 2: I verify before touching anything. On failure the original is intact.
    let copied = dir_size(stage);
    let source = dir_size(orig);
    if copied != source {
        return Err(format!(
            "copy mismatch: {} copied vs {} in the original",
            human_bytes(copied),
            human_bytes(source)
        ));
    }
    if let Some(exe) = verify_exe {
        if !exe.exists() {
            return Err(format!(
                "executable missing from the copy: {}",
                exe.display()
            ));
        }
    }

    // Step 3: I publish. Stage and dest sit in Games, same filesystem:
    // atomic rename. From here two live copies exist.
    std::fs::rename(stage, dest)
        .map_err(|e| format!("could not publish the copy at {}: {}", dest.display(), e))?;

    // Step 4: I retire the original, leaving a compat symlink.
    finalize(orig, dest)
}

/// The risk window. I rename the original, place the symlink, and only then
/// delete: the single destructive step comes last.
fn finalize(orig: &Path, dest: &Path) -> Result<Option<String>, String> {
    let name = orig
        .file_name()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| "game".to_string());
    let parent = orig
        .parent()
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|| PathBuf::from("."));
    let backup = parent.join(format!(".{}.{}", sanitize(&name), OLD_SUFFIX));

    // Step 4a: I retire the original in a single syscall.
    if let Err(e) = std::fs::rename(orig, &backup) {
        return Err(format!(
            "could not retire the original; a valid orphaned copy remains at {}: {}",
            dest.display(),
            e
        ));
    }

    // Step 4b: symlink so Heroic and Lutris keep working.
    if let Err(e) = symlink(dest, orig) {
        // Automatic rollback: the original is one syscall away, intact.
        if std::fs::rename(&backup, orig).is_ok() {
            return Err(format!("could not create the symlink, rolled back: {}", e));
        }
        return Err(format!(
            "CRITICAL: could not create the symlink nor roll back; the original is at {}",
            backup.display()
        ));
    }

    // Step 4c: single destructive step, and the last one. The game already works.
    if std::fs::remove_dir_all(&backup).is_err() {
        return Ok(Some(format!(
            "game works, but I could not delete {}",
            backup.display()
        )));
    }
    Ok(None)
}

fn report(
    label: &str,
    old: &Path,
    new: &Path,
    res: &Result<Option<String>, String>,
    exe_verified: bool,
) -> Vec<(String, MoveOutcome)> {
    match res {
        Ok(None) => {
            eprintln!("[import] {}: {} -> {} (ok)", label, old.display(), new.display());
            if !exe_verified {
                // Traceability note, not a failure: the executable falls
                // outside install_path, so there is nothing to check inside
                // the copy. I log it in case something breaks later.
                eprintln!(
                    "[import] {}: no executable check (outside install_path)",
                    label
                );
            }
            vec![(label.to_string(), MoveOutcome::Clean)]
        }
        Ok(Some(w)) => {
            eprintln!(
                "[import] {}: {} -> {} (ok with warning: {})",
                label,
                old.display(),
                new.display(),
                w
            );
            if !exe_verified {
                eprintln!(
                    "[import] {}: no executable check (outside install_path)",
                    label
                );
            }
            vec![(label.to_string(), MoveOutcome::WithWarning(w.clone()))]
        }
        Err(e) => {
            eprintln!("[import] {}: {} (failed: {})", label, old.display(), e);
            vec![(label.to_string(), MoveOutcome::Failed(e.clone()))]
        }
    }
}
