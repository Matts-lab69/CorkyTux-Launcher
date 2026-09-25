//! Migracion segura de `install_path` y `prefix_path` al directorio Games.
//!
//! El import permanente nunca borra antes de verificar. La secuencia es:
//! copiar con `cp -a` a un temporal DENTRO de Games (mismo filesystem, por eso
//! el rename final es atomico), verificar tamano y ejecutable, publicar con
//! un rename, y solo entonces retirar el original dejando un symlink en su
//! ruta vieja para que Heroic y Lutris no se rompan.
//!
//! `preflight` y `plan_for_mode` estan separados a proposito: el usuario
//! elige el modo despues de ver los blockers, y solo entonces se decide que
//! se mueve de verdad. `execute` es el unico punto que toca el disco.

#![allow(dead_code)] // comandos 2-5 (UI) aun no llaman a todo esto; se quita al final

use std::collections::{BTreeMap, BTreeSet};
use std::ffi::CString;
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::symlink;
use std::path::{Component, Path, PathBuf};
use std::process::Command;

/// Margen sobre el total a mover. La copia vive dentro de Games mientras el
/// original sigue ahi, asi que en el peor caso se necesita el doble: el
/// margen cubre el temporal mas los metadatos del filesystem.
pub const SPACE_MARGIN_BYTES: u64 = 512 * 1024 * 1024;

/// Temporal de staging. Tiene que estar en Games: si viviera en /tmp el rename
/// al destino final seria una copia, no un rename, y perderia la atomicidad.
pub const STAGE_DIR: &str = ".corkytux-import";

/// Sufijo del original retirado durante la ventana rename -> symlink.
pub const OLD_SUFFIX: &str = "corkytux-old";

/// Modo elegido por el usuario en el Import Manager.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ImportMode {
    /// Comportamiento actual: solo registra, no mueve nada.
    Test,
    /// Mueve los juegos que no comparten prefix. Los que comparten se
    /// quedan en modo Test.
    Permanent,
    /// Ademas mueve los juegos con prefix compartido, todos juntos.
    PermanentWithSharedGroups,
}

/// Motivo por el que un candidato no entra en `plans`.
#[derive(Clone, Debug)]
pub enum Blocker {
    GameRunning { label: String },
    /// Global, no lleva etiqueta: si no cabe no se mueve nada.
    NotEnoughSpace { need_bytes: u64, free_bytes: u64 },
    SharedPrefix { label: String, shared_with: Vec<String> },
    SourceMissing { label: String, path: PathBuf },
}

impl Blocker {
    pub fn message(&self) -> String {
        match self {
            Blocker::GameRunning { label } => {
                format!("{} tiene un wineserver vivo sobre su prefix", label)
            }
            Blocker::NotEnoughSpace { need_bytes, free_bytes } => format!(
                "no cabe en Games: hacen falta {} y hay {}",
                human_bytes(*need_bytes),
                human_bytes(*free_bytes)
            ),
            Blocker::SharedPrefix { label, shared_with } => {
                format!("{} comparte prefix con {}", label, shared_with.join(", "))
            }
            Blocker::SourceMissing { label, path } => {
                format!("no existe la carpeta de {} ({})", label, path.display())
            }
        }
    }
}

/// Un juego candidato a migrar.
#[derive(Clone, Debug)]
pub struct MoveCandidate {
    pub label: String,
    pub store_tag: String,
    pub install_path: PathBuf,
    /// Prefix REAL del origen. `None` en Heroic: el prefix_path que CorkyTux
    /// registra para los stores es suyo, no del launcher, y no se toca.
    pub prefix_path: Option<PathBuf>,
    /// Ejecutable principal ya resuelto, para verificarlo en la copia.
    pub executable: PathBuf,
}

/// Movimiento fisico de un prefix. En un grupo compartido hay UN solo
/// PrefixMove para todos los miembros: el prefix se mueve una vez.
#[derive(Clone, Debug)]
pub struct PrefixMove {
    pub src: PathBuf,
    pub dest: PathBuf,
    pub bytes: u64,
}

/// Un juego con su destino resuelto.
#[derive(Clone, Debug)]
pub struct MovePlan {
    pub candidate: MoveCandidate,
    /// Tamano indicativo de este plan. El total autoritativo lo calcula
    /// `preflight` con los prefijos contados una sola vez.
    pub bytes: u64,
    pub dest_install: PathBuf,
    /// Prefix propio de este juego. `None` si no hay prefix real, o si el
    /// prefix lo lleva un `GroupPlan` (que lo mueve una vez para todos).
    pub prefix_move: Option<PrefixMove>,
    /// `Some(relativo)` cuando `install_path` vive DENTRO de `prefix_path`.
    /// No hay copia propia: los datos viajan con el prefix y lo unico que
    /// cambia es la ruta configurada.
    pub nested_rel: Option<String>,
}

impl MovePlan {
    pub fn is_nested(&self) -> bool {
        self.nested_rel.is_some()
    }

    /// `install_path` final una vez completado el movimiento.
    pub fn new_install_path(&self) -> PathBuf {
        match &self.nested_rel {
            Some(rel) => match &self.prefix_move {
                Some(pm) => pm.dest.join(rel),
                None => PathBuf::new(),
            },
            None => self.dest_install.clone(),
        }
    }

    /// `prefix_path` final, o `None` si este juego no tiene prefix real.
    pub fn new_prefix_path(&self) -> Option<PathBuf> {
        self.prefix_move.as_ref().map(|pm| pm.dest.clone())
    }
}

/// Resultado de los checks previos.
#[derive(Clone, Debug)]
pub struct Preflight {
    pub plans: Vec<MovePlan>,
    pub blockers: Vec<Blocker>,
}

/// Clase de equivalencia de prefijos compartidos.
#[derive(Clone, Debug)]
pub struct SharedGroup {
    pub group_id: String,
    pub members: Vec<String>,
    pub prefix_path: PathBuf,
}

/// El grupo como unidad: el prefix se mueve una vez, los installs N veces.
#[derive(Clone, Debug)]
pub struct GroupPlan {
    pub group_id: String,
    pub prefix_move: Option<PrefixMove>,
    pub members: Vec<MovePlan>,
}

/// Lo que la UI recibe una vez elegido el modo.
#[derive(Clone, Debug, Default)]
pub struct ExecPlan {
    pub singles: Vec<MovePlan>,
    pub groups: Vec<GroupPlan>,
    pub test_only: Vec<String>,
}

/// Resumen para pintar el dialogo antes de confirmar.
#[derive(Clone, Debug, Default)]
pub struct Summary {
    pub permanent: usize,
    pub in_group: Vec<(String, Vec<String>)>,
    pub in_test: Vec<String>,
}

impl Summary {
    pub fn has_shared(&self) -> bool {
        !self.in_group.is_empty()
    }
}

/// Resultado por juego. La UI decide que hacer con los exitos.
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

    pub fn message(&self) -> Option<&str> {
        match self {
            MoveOutcome::Clean => None,
            MoveOutcome::WithWarning(m) | MoveOutcome::Failed(m) => Some(m.as_str()),
        }
    }
}

// ─── helpers de ruta ────────────────────────────────────────────────

/// Normaliza para comparar: canonicaliza si existe, si no limpia elemetos.
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

/// ¿`inner` vive dentro de `outer`? Comparacion por componentes, no por
/// prefijo de texto: `/juegos/quake` NO esta dentro de `/juegos/quake2`.
pub fn nested_in(inner: &Path, outer: &Path) -> bool {
    let a = norm(inner);
    let b = norm(outer);
    a != b && a.starts_with(&b)
}

/// Nombre de carpeta seguro a partir de un titulo.
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

/// Primer destino libre: `Nombre`, y si existe `Nombre (GOG)`, `Nombre (GOG 2)`.
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

/// Hermano libre de `base` con sufijo: `base-prefix`, `base-prefix 2`...
/// El prefix de un juego queda siempre junto a la carpeta del juego.
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

/// Hijo libre dentro de un directorio de grupo.
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

/// Tamano total de un arbol. Los symlinks cuentan por su propio tamano, nunca
/// por el del destino: `symlink_metadata` no sigue enlaces. Las carpetas
/// aportan 0, de modo que la comparacion origen/copia es simetrica.
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

/// Espacio libre en el filesystem que contiene `path`, via statvfs.
pub fn free_bytes(path: &Path) -> std::io::Result<u64> {
    let c = CString::new(path.as_os_str().as_bytes())
        .map_err(|_| std::io::Error::new(std::io::ErrorKind::InvalidInput, "ruta con NUL"))?;
    let mut st: libc::statvfs = unsafe { std::mem::zeroed() };
    let rc = unsafe { libc::statvfs(c.as_ptr(), &mut st) };
    if rc != 0 {
        return Err(std::io::Error::last_os_error());
    }
    let avail = st.f_bavail as u64;
    let bsize = st.f_frsize as u64;
    Ok(avail.saturating_mul(bsize))
}

/// ¿Hay un wineserver vivo sosteniendo este prefix? Mismo criterio que
/// `proton.rs` (lock + pgrep), mas una segunda comprobacion sobre la linea de
/// proceso por si el lock no llegara a escribirse.
pub fn prefix_in_use(prefix: &Path) -> bool {
    let lock = prefix.join("wineserver.lock");
    let running = match Command::new("pgrep").arg("-f").arg("wineserver").output() {
        Ok(o) => o.status.success(),
        Err(_) => false,
    };
    if !running {
        return false;
    }
    if lock.exists() {
        return true;
    }
    match Command::new("pgrep").arg("-af").arg("wineserver").output() {
        Ok(o) => {
            let want = prefix.display().to_string();
            if want.is_empty() {
                return false;
            }
            let text = String::from_utf8_lossy(&o.stdout);
            text.lines().any(|l| l.contains(&want))
        }
        Err(_) => false,
    }
}

/// Temporal de staging, un directorio por proceso.
pub fn stage_root(games_dir: &Path) -> PathBuf {
    games_dir.join(STAGE_DIR).join(format!("pid-{}", std::process::id()))
}

/// Bytes legibles para la UI.
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

// ─── preflight ──────────────────────────────────────────────────────

/// Checks previos. Decide que candidatos entran en `plans` y cuales quedan
/// bloqueados. No toca el disco: solo lee.
pub fn preflight(cands: &[MoveCandidate], games_dir: &Path) -> Preflight {
    let mut plans: Vec<MovePlan> = Vec::new();
    let mut blockers: Vec<Blocker> = Vec::new();

    // Fase A: prefijos reales, agrupados por ruta normalizada.
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

    // Fase B: checks por candidato.
    for c in cands {
        if !c.install_path.exists() {
            blockers.push(Blocker::SourceMissing {
                label: c.label.clone(),
                path: c.install_path.clone(),
            });
            continue;
        }
        if let Some(p) = &c.prefix_path {
            if p.exists() && prefix_in_use(p) {
                blockers.push(Blocker::GameRunning {
                    label: c.label.clone(),
                });
                continue;
            }
        }

        let dest_install = unique_dest(games_dir, &c.label, &c.store_tag);
        let shared_prefix = match &c.prefix_path {
            Some(p) if p.exists() => shared.contains(&norm(p)),
            _ => false,
        };

        // El prefix lo mueve el grupo si es compartido; si no, este plan es su
        // dueño y lo mueve el mismo.
        let prefix_move = match &c.prefix_path {
            Some(p) if p.exists() && !shared_prefix => Some(PrefixMove {
                src: p.clone(),
                dest: unique_sibling(&dest_install, "prefix"),
                bytes: dir_size(p),
            }),
            _ => None,
        };

        // D1: install_path dentro de prefix_path. No se bloquea: los datos
        // viajan con el prefix y lo unico que cambia es la ruta configurada.
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

    // Fase C: un blocker SharedPrefix por miembro, con los nombres de los
    // demas. La UI reconstruye los grupos con `groups_of` sin que `Preflight`
    // necesite un campo extra.
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

    // Fase D: espacio global, con cada prefix contado UNA vez.
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
                // Global: si no cabe, no se mueve nada.
                plans.clear();
            }
        }
        Err(e) => eprintln!("[import] statvfs fallo en {}: {}", games_dir.display(), e),
    }

    Preflight { plans, blockers }
}

fn uf_find(parent: &mut BTreeMap<String, String>, x: &str) -> String {
    if !parent.contains_key(x) {
        parent.insert(x.to_string(), x.to_string());
    }
    let mut cur = x.to_string();
    // Union por raices no puede crear ciclos, pero el recorrido lleva guardia
    // para que un dato corrupto no deje el import colgado.
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

/// Reconstruye los grupos compartidos desde los blockers, sin campos extra
/// en `Preflight`. Une por clases de equivalencia sobre los nombres.
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

// ─── plan por modo ──────────────────────────────────────────────────

/// Traduce el modo elegido a un plan ejecutable. En `PermanentWithSharedGroups`
/// revalida cada grupo: algo pudo ponerse a correr entre el preflight y el ok.
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

    // Sin espacio, o en modo Test, no se mueve nada.
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

    // Los limpios que no son parte de un grupo van como single en los dos
    // modos permanentes: un grupo nunca bloquea al resto de la seleccion.
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

    // PermanentWithSharedGroups. El presupuesto se consume grupo a grupo: dos
    // grupos pueden caber por separado y no juntos.
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

        // Revalidacion. Si un miembro falla, el grupo ENTERO se queda en test:
        // partir un prefix compartido entre dos sitios es justo el bug que
        // estamos previniendo.
        let mut bad: Option<String> = None;
        if !g.prefix_path.as_os_str().is_empty() && prefix_in_use(&g.prefix_path) {
            bad = Some(format!("el prefix compartido {} esta en uso", g.prefix_path.display()));
        } else {
            for m in &members {
                if !m.install_path.exists() {
                    bad = Some(format!("no existe la carpeta de {}", m.label));
                    break;
                }
            }
        }

        // Espacio del grupo: prefix una vez + install de cada miembro.
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
                "el grupo no cabe: hacen falta {} y quedan {}",
                human_bytes(need_total),
                human_bytes(budget)
            ));
        }

        if let Some(reason) = bad {
            eprintln!("[import] grupo {} descartado: {}", g.group_id, reason);
            for m in &members {
                ep.test_only.push(m.label.clone());
            }
            continue;
        }

        // Layout del grupo: ~/Games/<grupo>/prefix + ~/Games/<grupo>/<juego>
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

/// Resumen para el dialogo: que va en permanente, que va en grupo, que queda
/// en test.
pub fn summary_for_ui(ep: &ExecPlan) -> Summary {
    let in_group: Vec<(String, Vec<String>)> = ep
        .groups
        .iter()
        .map(|g| {
            (
                g.group_id.clone(),
                g.members.iter().map(|m| m.candidate.label.clone()).collect(),
            )
        })
        .collect();
    let permanent = ep.singles.len() + in_group.iter().map(|(_, ms)| ms.len()).sum::<usize>();
    let mut in_test = ep.test_only.clone();
    in_test.sort();
    in_test.dedup();
    Summary {
        permanent,
        in_group,
        in_test,
    }
}

/// Orphans de una sesion anterior interrumpida entre el rename y el symlink.
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

// ─── ejecucion ──────────────────────────────────────────────────────

/// Ejecuta el plan. Los grupos primero: son los mas caros y los mas
/// propensos a abortar. No toca la configuracion: devuelve los resultados y
/// es el llamador quien actualiza Games.ini con los que salieron bien.
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

    // Un plan con prefix propio lo mueve antes que el install: si el install
    // esta dentro del prefix, los datos viajan con el.
    if let Some(pm) = &p.prefix_move {
        let stage = stage_root(games_dir)
            .join(sanitize(&label))
            .join("prefix");
        if let Err(e) = move_dir(&pm.src, &pm.dest, &stage, None) {
            eprintln!("[import] {}: fallo moviendo el prefix: {}", label, e);
            return vec![(label, MoveOutcome::Failed(e))];
        }
    }

    if p.is_nested() {
        // Los datos ya estan en la copia del prefix: solo cambia la ruta.
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
            eprintln!("[import] grupo {}: fallo el prefix: {}", g.group_id, e);
            let msg = format!("no se pudo mover el prefix compartido: {}", e);
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

/// Ejecutable relativo a install_path. `None` si el ejecutable ES la carpeta
/// de install (ya verificado por dir_size) o si cae fuera de ella, en cuyo
/// caso no hay nada que comprobar dentro de la copia.
fn exe_rel(c: &MoveCandidate) -> Option<PathBuf> {
    let ip = norm(&c.install_path);
    let ex = norm(&c.executable);
    if ex == ip {
        return None;
    }
    ex.strip_prefix(&ip).ok().map(|r| r.to_path_buf())
}

/// Copia, verifica y publica. Todo el trabajo destructivo vive en `finalize`.
fn move_dir(
    src: &Path,
    dest: &Path,
    stage: &Path,
    verify_exe: Option<&Path>,
) -> Result<Option<String>, String> {
    if !src.exists() {
        return Err(format!("el origen ya no existe: {}", src.display()));
    }
    if dest.exists() {
        return Err(format!("el destino ya existe: {}", dest.display()));
    }
    if let Some(p) = stage.parent() {
        std::fs::create_dir_all(p).map_err(|e| format!("no se pudo crear el temporal: {}", e))?;
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
    // El temporal se limpia siempre, se llegue donde se llegue. Tras un
    // rename exitoso ya no existe, asi que esto es un no-op.
    if stage.exists() {
        let _ = std::fs::remove_dir_all(stage);
    }
    res
}

fn cp_a(src: &Path, dst: &Path) -> Result<(), String> {
    if dst.exists() {
        std::fs::remove_dir_all(dst).map_err(|e| format!("no se pudo limpiar el temporal: {}", e))?;
    }
    // La barra final copia el CONTENIDO, incluidos los ocultos, y deja dst
    // con la misma forma que src.
    let status = Command::new("cp")
        .arg("-a")
        .arg(src.join("."))
        .arg(dst)
        .status()
        .map_err(|e| format!("no se pudo ejecutar cp: {}", e))?;
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
    // F2: verificar antes de tocar nada. Si falla, el original sigue intacto.
    let copied = dir_size(stage);
    let source = dir_size(orig);
    if copied != source {
        return Err(format!(
            "la copia no cuadra: {} copiados frente a {} del original",
            human_bytes(copied),
            human_bytes(source)
        ));
    }
    if let Some(exe) = verify_exe {
        if !exe.exists() {
            return Err(format!(
                "el ejecutable no aparece en la copia: {}",
                exe.display()
            ));
        }
    }

    // F3: publicar. stage y dest estan en Games, mismo filesystem: rename
    // atomico. A partir de aqui hay dos copias vivas.
    std::fs::rename(stage, dest)
        .map_err(|e| format!("no se pudo publicar la copia en {}: {}", dest.display(), e))?;

    // F4: retirar el original dejando symlink de compatibilidad.
    finalize(orig, dest)
}

/// La ventana de riesgo. Rename del original, symlink, y solo despues el
/// borrado: el unico paso destructivo es el ultimo.
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

    // F4a: retirar el original de una sola syscall.
    if let Err(e) = std::fs::rename(orig, &backup) {
        return Err(format!(
            "no se pudo retirar el original; queda una copia valida y huerfana en {}: {}",
            dest.display(),
            e
        ));
    }

    // F4b: symlink para que Heroic y Lutris no se rompan.
    if let Err(e) = symlink(dest, orig) {
        // Rollback automatico: el original esta entero a una syscall.
        if std::fs::rename(&backup, orig).is_ok() {
            return Err(format!("no se pudo crear el symlink y se revirtio: {}", e));
        }
        return Err(format!(
            "CRITICO: no se pudo crear el symlink ni revertir; el original esta en {}",
            backup.display()
        ));
    }

    // F4c: unico paso destructivo, y el ultimo. El juego ya funciona.
    if std::fs::remove_dir_all(&backup).is_err() {
        return Ok(Some(format!(
            "el juego funciona, pero no se pudo borrar {}",
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
                // Nota de trazabilidad, no un fallo: el ejecutable cae fuera
                // de install_path, asi que no hay nada que comprobar dentro
                // de la copia. Queda registrado por si algo se rompe luego.
                eprintln!(
                    "[import] {}: sin verificacion de ejecutable (no cae dentro de install_path)",
                    label
                );
            }
            vec![(label.to_string(), MoveOutcome::Clean)]
        }
        Ok(Some(w)) => {
            eprintln!(
                "[import] {}: {} -> {} (ok con aviso: {})",
                label,
                old.display(),
                new.display(),
                w
            );
            if !exe_verified {
                eprintln!(
                    "[import] {}: sin verificacion de ejecutable (no cae dentro de install_path)",
                    label
                );
            }
            vec![(label.to_string(), MoveOutcome::WithWarning(w.clone()))]
        }
        Err(e) => {
            eprintln!("[import] {}: {} (fallo: {})", label, old.display(), e);
            vec![(label.to_string(), MoveOutcome::Failed(e.clone()))]
        }
    }
}
