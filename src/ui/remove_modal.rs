use adw::prelude::*;
use gtk::prelude::*;
use std::cell::RefCell;
use std::rc::Rc;

use crate::ui::center::CenterHandle;
use crate::ui::details_panel::DetailsPanel;
use crate::ui::helpers;
use crate::ui::sidebar::Sidebar;
use crate::AppState;

/// install_path values from heroic-store's installs.json (Epic+GOG). The
/// launcher had no reader for it, so this one is local and minimal.
/// Missing → empty; present but unreadable → None (the caller denies).
fn plugin_install_paths_at(cfg: &std::path::Path) -> Option<Vec<String>> {
    let p = cfg.join("plugins/heroic-store/installs.json");
    if !p.exists() {
        return Some(vec![]);
    }
    let raw = std::fs::read_to_string(&p).ok()?;
    let v: serde_json::Value = serde_json::from_str(&raw).ok()?;
    Some(v.as_object()?.values()
        .filter_map(|e| e.get("path")?.as_str().map(str::to_string))
        .collect())
}

fn plugin_install_paths() -> Option<Vec<String>> {
    let home = std::env::var("HOME").unwrap_or_default();
    if home.is_empty() {
        return None;
    }
    plugin_install_paths_at(std::path::Path::new(&home).join(".config/CorkyTux").as_path())
}

/// Otros installs salvo uno mismo (por nombre y por ruta canonicalizada;
/// cubre la misma app en ficha nativa + registro del plugin).
fn others_except(state: &AppState, selected: &str, main: &str) -> Option<Vec<String>> {
    let self_c = canon(std::path::Path::new(main))
        .unwrap_or_else(|| std::path::PathBuf::from(main));
    let mut out: Vec<String> = state.game_model.ordered_names().iter()
        .filter(|n| n.as_str() != selected)
        .filter_map(|n| state.game_model.get_game(n))
        .map(|g| g.main_path)
        .chain(plugin_install_paths()?.into_iter())
        .filter(|o| {
            let oc = canon(std::path::Path::new(o)).unwrap_or_else(|| std::path::PathBuf::from(o));
            oc != self_c
        })
        .collect();
    out.sort();
    out.dedup();
    Some(out)
}

/// Roots that never get deleted: /, home, ~/Games, the legacy shared base
/// (Heroic) and the per-store bases. Single source together with the
/// per-store `default_games_dir` (stores_view).
pub(crate) fn store_deny_roots(games_root: &str, home: &str) -> Vec<String> {
    let base = games_root.trim_end_matches('/');
    let h = home.trim_end_matches('/');
    let mut v = vec!["/".to_string(), h.to_string(), base.to_string(),
         format!("{}/Heroic", base),
         format!("{}/Epic-Games", base), format!("{}/GOG-Games", base)];
    // Ancestro de HOME (/home respecto de /home/usuario).
    if let Some(parent) = std::path::Path::new(h).parent() {
        let p = parent.to_string_lossy().trim_end_matches('/').to_string();
        if !p.is_empty() && p != h {
            v.push(p);
        }
    }
    // Dirs de usuario estándar (XDG), aunque todavía no existan.
    for d in ["Documents", "Downloads", "Desktop", "Music", "Pictures", "Videos"] {
        v.push(format!("{}/{}", h, d));
    }
    // Raíces de biblioteca Steam reales: sólo el directorio `common`, sin
    // tocar sus subdirectorios, y sólo si existen.
    for root in crate::backend::proton::ProtonManager::steam_roots(std::path::Path::new(h)) {
        let common = root.join("steamapps/common");
        if common.exists() {
            v.push(common.display().to_string());
        }
    }
    v
}

/// Canonicalizes if it exists; None if not (the caller decides: a missing
/// path is never approved for deletion).
fn canon(p: &std::path::Path) -> Option<std::path::PathBuf> {
    std::fs::canonicalize(p).ok()
}

/// Reads the `UseSharedPrefix` flag without caring about case or padding:
/// "true", "True", " true " and "TRUE" all count, plus "1". Anything else
/// (including "false", "0" and empty) means a private prefix.
fn use_shared_prefix_on(raw: &str) -> bool {
    let v = raw.trim();
    v.eq_ignore_ascii_case("true") || v == "1"
}

/// Pure core: may `prefix` be deleted as a game prefix?
/// Denies: empty/relative/unresolvable paths, HOME and root, the shared
/// bases (or any overlap with them in either direction: the base itself, a
/// parent of it, or something inside it), and any other registered game's
/// prefix. Canonical comparison, so symlinks and `..` can't smuggle the
/// shared base past the check.
fn prefix_path_safe(prefix: &str, shared_bases: &[String], other_prefixes: &[String]) -> bool {
    let p = prefix.trim_end_matches('/');
    if p.is_empty() {
        return false;
    }
    if !std::path::Path::new(p).is_absolute() {
        return false;
    }
    let pc = match canon(std::path::Path::new(p)) {
        Some(c) => c,
        None => return false,
    };
    if pc.as_os_str() == "/" {
        return false;
    }
    if let Ok(home) = std::env::var("HOME") {
        let h = home.trim_end_matches('/');
        if !h.is_empty() {
            if let Some(hc) = canon(std::path::Path::new(h)) {
                if pc == hc {
                    return false;
                }
            }
        }
    }
    for b in shared_bases {
        let bb = b.trim_end_matches('/');
        if bb.is_empty() {
            continue;
        }
        let bc = canon(std::path::Path::new(bb))
            .unwrap_or_else(|| std::path::PathBuf::from(bb));
        if pc == bc || pc.starts_with(&bc) || bc.starts_with(&pc) {
            return false;
        }
    }
    for o in other_prefixes {
        let oo = o.trim_end_matches('/');
        if oo.is_empty() {
            continue;
        }
        let oc = canon(std::path::Path::new(oo))
            .unwrap_or_else(|| std::path::PathBuf::from(oo));
        if pc == oc {
            return false;
        }
    }
    true
}

/// Canonical containment: `path` inside `dir` (or equal). Anything
/// unresolvable denies (fail closed). Pure; covered by tests.
pub(crate) fn path_within(dir: &std::path::Path, path: &std::path::Path) -> bool {
    match (canon(dir), canon(path)) {
        (Some(root), Some(pc)) => pc == root || pc.starts_with(&root),
        _ => false,
    }
}

/// Gathers the deny inputs from the live config: every shared base plus the
/// prefixes of all other registered games. Read-only; decides nothing.
/// Reused by the Settings confirm dialogs (selected="" checks everything).
pub(crate) fn prefix_delete_safe(state: &AppState, selected: &str, prefix: &std::path::Path) -> bool {
    let mut shared: Vec<String> = Vec::new();
    for name in state.config.shared_prefixes() {
        if let Some(p) = state.config.shared_prefix_path(&name) {
            // Skip the target itself: a registered prefix must not block
            // its own deletion (callers check users separately first).
            let same = match (canon(prefix), canon(&p)) {
                (Some(t), Some(b)) => t == b,
                _ => {
                    p.display().to_string().trim_end_matches('/')
                        == prefix.display().to_string().trim_end_matches('/')
                }
            };
            if !same {
                shared.push(p.display().to_string());
            }
        }
    }
    let others: Vec<String> = state.game_model.ordered_names().iter()
        .filter(|n| n.as_str() != selected)
        .filter_map(|n| state.config.game_value(n, "PrefixPath"))
        .filter(|v| !v.is_empty())
        .collect();
    // Containment: shared-prefix deletion never leaves prefixes_dir().
    if !path_within(&state.config.prefixes_dir(), prefix) {
        return false;
    }
    prefix_path_safe(&prefix.display().to_string(), &shared, &others)
}

/// Can `main_path` be deleted as "the game folder"?
/// Denies: empty/relative/missing, a symlink as `main_path`, a path shallower
/// than 3 components (/, /home, /tmp), the deny list and its ancestors (after
/// canonicalizing), an exe outside main (relpath with an escaping `..`, a
/// foreign absolute, a symlink) and any overlap with another registered
/// install (main inside the other, or the other inside main). With an empty
/// exe (manifest, Epic) the list + overlap checks are the whole decision.
/// `others`: main_paths of the other installs; None (unreadable record) always
/// denies.
pub(crate) fn removal_target_safe(
    main_path: &str,
    exe_rel: &str,
    deny_roots: &[String],
    others: Option<&[String]>,
) -> bool {
    let m = main_path.trim_end_matches('/');
    if m.is_empty() {
        return false;
    }
    if !std::path::Path::new(m).is_absolute() {
        return false;
    }
    // Un symlink como main_path nunca se borra: remove_dir_all tocaría el
    // enlace (no el contenido) y su destino podría escapar de la raíz de juegos.
    if let Ok(md) = std::fs::symlink_metadata(std::path::Path::new(m)) {
        if md.file_type().is_symlink() {
            return false;
        }
    }
    // Canonicalize main: a failure on an existing path denies. A missing path
    // has nothing to delete, so it denies too (the lexical fallback only
    // compares already-resolved denies, and doesn't approve either).
    let main_c = match canon(std::path::Path::new(m)) {
        Some(p) => p,
        None => return false,
    };
    // Rutas demasiado someras (/, /home, /tmp): nunca son la carpeta de un
    // juego. <3 componentes = RootDir + a lo sumo un nombre.
    if main_c.components().count() < 3 {
        return false;
    }
    for d in deny_roots {
        let dd = d.trim_end_matches('/');
        if dd.is_empty() {
            continue;
        }
        let dp = std::path::Path::new(dd);
        let resolved = canon(dp).unwrap_or_else(|| dp.to_path_buf());
        // Igualdad, o main es ANCESTRO de la raíz protegida (/home ⊃ HOME,
        // ~ ⊃ ~/Documents): borrarlo se llevaría la raíz entera.
        if resolved == main_c || resolved.starts_with(&main_c) {
            return false;
        }
    }
    let rest = match others {
        Some(o) => o,
        None => return false,
    };
    for o in rest.iter() {
        let oo = o.trim_end_matches('/');
        if oo.is_empty() {
            continue;
        }
        let oc = canon(std::path::Path::new(oo)).unwrap_or_else(|| std::path::PathBuf::from(oo));
        if oc == main_c || oc.starts_with(&main_c) || main_c.starts_with(&oc) {
            return false;
        }
    }
    if exe_rel.is_empty() {
        return true;
    }
    // Exe absoluto o relativo a main; debe existir CONTENIDO en main.
    let cand = if std::path::Path::new(exe_rel).is_absolute() {
        std::path::PathBuf::from(exe_rel)
    } else {
        main_c.join(exe_rel)
    };
    match canon(&cand) {
        // Archivo dentro de main (caso general), o la propia carpeta del
        // juego cuando el "exe" registrado ES la carpeta (juegos RPG Maker
        // por carpeta: Executable == MainPath). Deny-list y solape con
        // otros installs ya se comprobaron arriba.
        Some(c) => {
            if c.is_file() {
                c.starts_with(&main_c)
            } else {
                c == main_c && c.is_dir()
            }
        }
        None => false,
    }
}

/// Motivo mostrado/registrado cuando el guard de carpeta niega el borrado en
/// la confirmación (revalidación al pulsar "Remove").
const FILES_NOT_VERIFIED_MSG: &str =
    "Folder no longer verified. Uncheck 'Remove game files' and try again.";

/// Decisión pura (sin GTK) del borrado de archivos al confirmar: `Ok` si la
/// carpeta puede borrarse; `Err(motivo)` si `removal_target_safe` la niega.
/// Encapsula el redirect del handler para poder testearlo sin una ventana.
fn confirm_files_removal(
    main_path: &str,
    exe_rel: &str,
    deny_roots: &[String],
    others: Option<&[String]>,
) -> Result<(), &'static str> {
    if removal_target_safe(main_path, exe_rel, deny_roots, others) {
        Ok(())
    } else {
        Err(FILES_NOT_VERIFIED_MSG)
    }
}

#[cfg(test)]
mod tests {
    use super::{confirm_files_removal, path_within, prefix_path_safe, removal_target_safe,
                store_deny_roots, use_shared_prefix_on, FILES_NOT_VERIFIED_MSG};

    fn deny() -> Vec<String> {
        vec!["/".to_string(), "/h".to_string(), "/h/Games".to_string(),
             "/h/Games/Heroic".to_string()]
    }

    #[test]
    fn niega_base_home_raiz_y_relativos() {
        let d = deny();
        let none: Option<&[String]> = None;
        let empty: Vec<String> = vec![];
        let some = Some(empty.as_slice());
        assert!(!removal_target_safe("/h/Games/Heroic", "J.exe", &d, some));
        assert!(!removal_target_safe("/h/Games", "J.exe", &d, some));
        assert!(!removal_target_safe("/h", "J.exe", &d, some));
        assert!(!removal_target_safe("/", "J.exe", &d, some));
        assert!(!removal_target_safe("", "J.exe", &d, some));
        assert!(!removal_target_safe("relativo/dir", "J.exe", &d, some));
        assert!(!removal_target_safe("/h/Games/Heroic/", "J.exe", &d, some));
        // Sin registro legible se niega aunque la forma valga.
        assert!(!removal_target_safe("/h/Games/Heroic/Juego", "J.exe", &d, none));
    }

    fn mktmp(name: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(name);
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn exige_exe_directo() {
        let dir = mktmp("corky_rm_test");
        let d = deny();
        let empty: Vec<String> = vec![];
        let some = Some(empty.as_slice());
        let ms = dir.display().to_string();
        // No exe inside: not deleted, however deep the folder.
        assert!(!removal_target_safe(&ms, "J.exe", &d, some));
        // Empty exe (manifest): passes if not denied.
        assert!(removal_target_safe(&ms, "", &d, some));
        std::fs::write(dir.join("J.exe"), b"x").unwrap();
        assert!(removal_target_safe(&ms, "J.exe", &d, some));
        // Exe en subcarpeta con relpath que queda dentro: legacy, permite.
        let _ = std::fs::remove_file(dir.join("J.exe"));
        std::fs::create_dir_all(dir.join("sub")).unwrap();
        std::fs::write(dir.join("sub").join("J.exe"), b"x").unwrap();
        assert!(!removal_target_safe(&ms, "J.exe", &d, some));
        assert!(removal_target_safe(&ms, "sub/J.exe", &d, some));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn aprueba_juego_por_carpeta() {
        // RPG Maker por carpeta: Executable == MainPath (un dir).
        let dir = mktmp("corky_rm_folder");
        let d = deny();
        let empty: Vec<String> = vec![];
        let some = Some(empty.as_slice());
        let ms = dir.display().to_string();
        assert!(removal_target_safe(&ms, &ms, &d, some));
        // Subcarpeta como exe: sigue negado (conservador).
        assert!(!removal_target_safe(&ms, "sub", &d, some));
        // Carpeta denegada aunque coincida consigo misma.
        assert!(!removal_target_safe("/h/Games", "/h/Games", &d, some));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn puntos_no_burlan_deny() {
        // main = <tmp>/Games/sub/.. → canonical = <tmp>/Games (denegado).
        let base = mktmp("corky_rm_dotdot");
        let games = base.join("Games");
        std::fs::create_dir_all(games.join("sub")).unwrap();
        let d = vec!["/".to_string(), games.display().to_string()];
        let empty: Vec<String> = vec![];
        let some = Some(empty.as_slice());
        let main = format!("{}/sub/..", games.display());
        assert!(!removal_target_safe(&main, "", &d, some));
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn symlink_a_base_no_pasa() {
        let base = mktmp("corky_rm_link");
        let real = base.join("Heroic");
        std::fs::create_dir_all(&real).unwrap();
        #[cfg(unix)]
        std::os::unix::fs::symlink(&real, base.join("link")).unwrap();
        let d = vec!["/".to_string(), real.display().to_string()];
        let empty: Vec<String> = vec![];
        let some = Some(empty.as_slice());
        assert!(!removal_target_safe(
            &base.join("link").display().to_string(), "", &d, some));
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn solape_con_otro_install_niega() {
        let base = mktmp("corky_rm_overlap");
        let outer = base.join("J");
        let inner = outer.join("J");
        std::fs::create_dir_all(&inner).unwrap();
        std::fs::write(inner.join("J.exe"), b"x").unwrap();
        let d = vec!["/".to_string()];
        let ms_out = outer.display().to_string();
        let ms_in = inner.display().to_string();
        // Legacy con exe en sub: permite solo.
        let solo: Vec<String> = vec![];
        assert!(removal_target_safe(&ms_out, "J/J.exe", &d, Some(solo.as_slice())));
        // Otro install dentro de main: niega.
        let dentro = vec![ms_in.clone()];
        assert!(!removal_target_safe(&ms_out, "J/J.exe", &d, Some(dentro.as_slice())));
        // Main dentro de otro install: niega.
        let fuera = vec![ms_out.clone()];
        assert!(!removal_target_safe(&ms_in, "J.exe", &d, Some(fuera.as_slice())));
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn plugin_solo_dentro_de_main_niega() {
        // installs.json con ruta ajena dentro de main: solape → niega.
        let base = mktmp("corky_rm_plugin");
        let main = base.join("J");
        let inner = main.join("otrojuego");
        std::fs::create_dir_all(&inner).unwrap();
        std::fs::write(main.join("J.exe"), b"x").unwrap();
        let d = vec!["/".to_string()];
        let ms = main.display().to_string();
        let resto = vec![inner.display().to_string()];
        assert!(!removal_target_safe(&ms, "J.exe", &d, Some(resto.as_slice())));
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn misma_ruta_ficha_y_plugin_permite() {
        // After excluding itself only others remain: the direct exe decides.
        let base = mktmp("corky_rm_self");
        std::fs::write(base.join("J.exe"), b"x").unwrap();
        let d = vec!["/".to_string()];
        let empty: Vec<String> = vec![];
        assert!(removal_target_safe(
            &base.display().to_string(), "J.exe", &d, Some(empty.as_slice())));
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn installs_ausente_vacio_corrupto_none() {
        use super::plugin_install_paths_at;
        let base = mktmp("corky_rm_reg");
        let cfg = base.join("cfg");
        std::fs::create_dir_all(cfg.join("plugins/heroic-store")).unwrap();
        // Missing → empty (doesn't deny on its own).
        assert_eq!(plugin_install_paths_at(&cfg), Some(vec![]));
        // Corrupt → None (the caller denies).
        std::fs::write(cfg.join("plugins/heroic-store/installs.json"), b"{no-json").unwrap();
        assert_eq!(plugin_install_paths_at(&cfg), None);
        // Valid → paths.
        std::fs::write(cfg.join("plugins/heroic-store/installs.json"),
            br#"{"gog:9": {"path": "/juegos/X", "exe": "X.exe"}}"#).unwrap();
        assert_eq!(plugin_install_paths_at(&cfg), Some(vec!["/juegos/X".to_string()]));
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn bases_nuevas_y_legacy_se_niegan() {
        use super::store_deny_roots;
        let d = store_deny_roots("/h/Games", "/h");
        assert!(d.contains(&"/h/Games/Epic-Games".to_string()));
        assert!(d.contains(&"/h/Games/GOG-Games".to_string()));
        assert!(d.contains(&"/h/Games/Heroic".to_string()));
        let empty: Vec<String> = vec![];
        let some = Some(empty.as_slice());
        // Las tres bases se niegan aunque tuvieran exe adentro.
        for base in ["/h/Games/Epic-Games", "/h/Games/GOG-Games", "/h/Games/Heroic"] {
            assert!(!removal_target_safe(base, "J.exe", &d, some));
        }
    }

    #[test]
    fn exe_con_puntos_que_escapa_niega() {
        let base = mktmp("corky_rm_escape");
        let outer = base.join("J");
        std::fs::create_dir_all(outer.join("sub")).unwrap();
        std::fs::write(base.join("evil.exe"), b"x").unwrap();
        let d = vec!["/".to_string()];
        let empty: Vec<String> = vec![];
        let some = Some(empty.as_slice());
        assert!(!removal_target_safe(
            &outer.display().to_string(), "sub/../../evil.exe", &d, some));
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn flag_sin_mayusculas_ni_espacios() {
        for v in ["true", "True", "TRUE", " true ", "TrUe", "1"] {
            assert!(use_shared_prefix_on(v), "debe valer: {:?}", v);
        }
        for v in ["", "false", "False", "0", "yes", "2"] {
            assert!(!use_shared_prefix_on(v), "no debe valer: {:?}", v);
        }
    }

    #[test]
    fn prefix_propio_se_permite() {
        let base = mktmp("corky_pfx_own");
        let shared = base.join("shared-proton-x");
        let own = base.join("prefixes").join("My Game");
        std::fs::create_dir_all(&shared).unwrap();
        std::fs::create_dir_all(&own).unwrap();
        let shared_s = vec![shared.display().to_string()];
        let empty: Vec<String> = vec![];
        assert!(prefix_path_safe(&own.display().to_string(), &shared_s, &empty));
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn prefix_igual_a_la_base_se_niega() {
        let base = mktmp("corky_pfx_base");
        let shared = base.join("shared-proton-x");
        std::fs::create_dir_all(&shared).unwrap();
        let shared_s = vec![shared.display().to_string()];
        let empty: Vec<String> = vec![];
        assert!(!prefix_path_safe(&shared.display().to_string(), &shared_s, &empty));
        // Con barra final también.
        assert!(!prefix_path_safe(&format!("{}/", shared.display()), &shared_s, &empty));
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn prefix_dentro_de_la_base_se_niega() {
        // El caso real: PrefixPath = <shared>/pfx.
        let base = mktmp("corky_pfx_inside");
        let shared = base.join("shared-proton-x");
        let inner = shared.join("pfx");
        std::fs::create_dir_all(&inner).unwrap();
        let shared_s = vec![shared.display().to_string()];
        let empty: Vec<String> = vec![];
        assert!(!prefix_path_safe(&inner.display().to_string(), &shared_s, &empty));
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn ancestro_de_la_base_se_niega() {
        let base = mktmp("corky_pfx_parent");
        let shared = base.join("prefixes").join("shared-proton-x");
        std::fs::create_dir_all(&shared).unwrap();
        let shared_s = vec![shared.display().to_string()];
        let empty: Vec<String> = vec![];
        assert!(!prefix_path_safe(
            &base.join("prefixes").display().to_string(), &shared_s, &empty));
        assert!(!prefix_path_safe(&base.display().to_string(), &shared_s, &empty));
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn symlink_y_puntos_hacia_la_base_se_niegan() {
        let base = mktmp("corky_pfx_trick");
        let shared = base.join("shared-proton-x");
        std::fs::create_dir_all(shared.join("pfx")).unwrap();
        let shared_s = vec![shared.display().to_string()];
        let empty: Vec<String> = vec![];
        #[cfg(unix)]
        std::os::unix::fs::symlink(&shared, base.join("link")).unwrap();
        #[cfg(unix)]
        assert!(!prefix_path_safe(
            &base.join("link").display().to_string(), &shared_s, &empty));
        // ".." que resuelve dentro de la base.
        let dotdot = format!("{}/pfx/..", shared.display());
        assert!(!prefix_path_safe(&dotdot, &shared_s, &empty));
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn prefix_de_otro_juego_se_niega() {
        let base = mktmp("corky_pfx_other");
        let mine = base.join("Mine");
        let other = base.join("Other");
        std::fs::create_dir_all(&mine).unwrap();
        std::fs::create_dir_all(&other).unwrap();
        let shared_s: Vec<String> = vec![];
        let others = vec![other.display().to_string()];
        assert!(!prefix_path_safe(&other.display().to_string(), &shared_s, &others));
        assert!(prefix_path_safe(&mine.display().to_string(), &shared_s, &others));
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn own_base_without_users_passes_gate() {
        // New gate contract: the caller excludes the target itself from
        // the bases list, so an unused registered prefix passes while
        // any other registered base still denies.
        let base = mktmp("corky_pfx_selfgate");
        let own = base.join("shared-proton-x");
        let other = base.join("shared-proton-y");
        std::fs::create_dir_all(&own).unwrap();
        std::fs::create_dir_all(&other).unwrap();
        let empty: Vec<String> = vec![];
        assert!(prefix_path_safe(
            &own.display().to_string(),
            &empty,
            &empty
        ));
        // Overlap with another registered base still denies.
        let inner = other.join("pfx");
        std::fs::create_dir_all(&inner).unwrap();
        assert!(!prefix_path_safe(
            &inner.display().to_string(),
            &vec![other.display().to_string()],
            &empty
        ));
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn path_within_contains_and_equal() {
        let base = mktmp("corky_pfx_within");
        let inner = base.join("prefixes").join("shared-x");
        std::fs::create_dir_all(&inner).unwrap();
        assert!(path_within(&base, &inner));
        assert!(path_within(&base, &base));
        assert!(path_within(
            &base,
            &std::path::PathBuf::from(format!("{}/", inner.display()))
        ));
        assert!(!path_within(&base, &base.join("..").join("other")));
        assert!(!path_within(&base, &std::path::PathBuf::from("/nonexistent-xyz")));
        assert!(!path_within(
            &std::path::PathBuf::from("/nonexistent-xyz"),
            &inner
        ));
        #[cfg(unix)]
        {
            std::os::unix::fs::symlink(&inner, base.join("link")).unwrap();
            assert!(path_within(&base, &base.join("link")));
        }
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn prefix_inexistente_y_relativo_se_niegan() {
        let base = mktmp("corky_pfx_missing");
        let shared_s: Vec<String> = vec![];
        let empty: Vec<String> = vec![];
        assert!(!prefix_path_safe(
            &base.join("no-existe").display().to_string(), &shared_s, &empty));
        assert!(!prefix_path_safe("relativo/dir", &shared_s, &empty));
        assert!(!prefix_path_safe("", &shared_s, &empty));
        let _ = std::fs::remove_dir_all(&base);
    }

    // ---------- Hardening probes: guards de rutas de borrado ----------

    #[test]
    fn removal_target_home_real_se_niega() {
        // HOME sí está en la deny-list (store_deny_roots) → verde.
        let _g = crate::test_env::lock();
        let home = std::env::var("HOME").unwrap_or_default();
        if home.is_empty() {
            return;
        }
        let deny = store_deny_roots(&format!("{}/Games", home), &home);
        let empty: Vec<String> = vec![];
        let some = Some(empty.as_slice());
        assert!(!removal_target_safe(&home, "", &deny, some), "HOME debe negarse");
        assert!(!removal_target_safe(&format!("{}/", home), "", &deny, some),
            "HOME con barra final también");
    }

    #[test]
    fn removal_target_ancestro_de_home_se_niega() {
        // Modela /home respecto de /home/usuario: store_deny_roots agrega el
        // ancestro de HOME y el guard niega cuando main es ancestro de una raíz
        // protegida (resolved.starts_with(main)).
        let root = mktmp("corky_guard_ancestor");
        let home = root.join("home");
        std::fs::create_dir_all(&home).unwrap();
        let deny = store_deny_roots(&home.join("Games").display().to_string(),
                                    &home.display().to_string());
        let empty: Vec<String> = vec![];
        let some = Some(empty.as_slice());
        assert!(!removal_target_safe(&root.display().to_string(), "", &deny, some),
            "un ancestro de HOME ({}) no debe ser borrable", root.display());
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn removal_target_xdg_user_dirs_se_niegan() {
        // Árbol sintético para no tocar el HOME real; replica ~/Documents etc.
        // store_deny_roots agrega los dirs XDG relativos a HOME.
        let root = mktmp("corky_guard_home");
        let home = root.join("home");
        for d in ["Documents", "Downloads", "Desktop"] {
            std::fs::create_dir_all(home.join(d)).unwrap();
        }
        let games = home.join("Games");
        std::fs::create_dir_all(games.join("GameA")).unwrap();
        let deny = store_deny_roots(&games.display().to_string(), &home.display().to_string());
        let empty: Vec<String> = vec![];
        let some = Some(empty.as_slice());
        for d in ["Documents", "Downloads", "Desktop"] {
            assert!(!removal_target_safe(&home.join(d).display().to_string(), "", &deny, some),
                "{} no debe ser borrable", d);
        }
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn removal_target_raiz_biblioteca_con_un_juego_se_niega_pero_el_juego_no() {
        // La raíz de la biblioteca (~/Games) está en la deny-list aun con un
        // solo juego dentro; el juego con exe directo sí se aprueba.
        let root = mktmp("corky_guard_lib");
        let home = root.join("home");
        let games = home.join("Games");
        let gamea = games.join("GameA");
        std::fs::create_dir_all(&gamea).unwrap();
        std::fs::write(gamea.join("A.exe"), b"x").unwrap();
        let deny = store_deny_roots(&games.display().to_string(), &home.display().to_string());
        let empty: Vec<String> = vec![];
        assert!(!removal_target_safe(&games.display().to_string(), "", &deny,
                                     Some(empty.as_slice())),
            "la raíz de la biblioteca no se borra");
        assert!(removal_target_safe(&gamea.display().to_string(), "A.exe", &deny,
                                    Some(empty.as_slice())),
            "el juego con exe dentro sí se aprueba");
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn prefix_raiz_y_tilde_se_niegan() {
        let empty: Vec<String> = vec![];
        assert!(!prefix_path_safe("/", &empty, &empty), "\"/\" no es prefijo borrable");
        assert!(!prefix_path_safe("~", &empty, &empty), "\"~\" es relativo: no se aprueba");
    }

    #[test]
    fn prefix_home_real_se_niega() {
        let _g = crate::test_env::lock();
        let home = std::env::var("HOME").unwrap_or_default();
        if home.is_empty() {
            return;
        }
        let empty: Vec<String> = vec![];
        assert!(!prefix_path_safe(&home, &empty, &empty), "HOME no es prefijo borrable");
        assert!(!prefix_path_safe(&format!("{}/", home), &empty, &empty));
    }

    /// A canonical, existing 2-component dir (RootDir + one name), not a
    /// symlink: /tmp on the usual Linux layout. None when unavailable.
    fn two_component_dir() -> Option<std::path::PathBuf> {
        let mut p = std::fs::canonicalize(std::env::temp_dir()).ok()?;
        while p.components().count() > 2 {
            p = p.parent()?.to_path_buf();
        }
        if p.components().count() == 2
            && p.is_dir()
            && !std::fs::symlink_metadata(&p).map(|m| m.file_type().is_symlink()).unwrap_or(true)
        {
            Some(p)
        } else {
            None
        }
    }

    #[test]
    fn symlink_a_destino_fuera_prefix_sigue_permitido() {
        // prefix_path_safe NO cambió: canonicaliza y compara contra
        // shared/otros, pero no exige raíz permitida; un symlink a un dir
        // ajeno sigue aprobándose (gap documentado del guard de prefijos).
        let base = mktmp("corky_guard_link_out");
        let target = base.join("outside");
        std::fs::create_dir_all(&target).unwrap();
        #[cfg(unix)]
        {
            std::os::unix::fs::symlink(&target, base.join("link")).unwrap();
            let link = base.join("link").display().to_string();
            let empty: Vec<String> = vec![];
            assert!(prefix_path_safe(&link, &empty, &empty),
                "documentado: prefix_path_safe APRUEBA symlink a destino fuera de deny/otros");
        }
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn removal_target_symlink_como_main_se_niega() {
        // Guard nuevo: un symlink como main_path (sin canonicalizar) se niega.
        let base = mktmp("corky_guard_main_link");
        let real = base.join("real");
        std::fs::create_dir_all(&real).unwrap();
        #[cfg(unix)]
        {
            std::os::unix::fs::symlink(&real, base.join("link")).unwrap();
            let link = base.join("link").display().to_string();
            let d = vec!["/".to_string()];
            let empty: Vec<String> = vec![];
            assert!(!removal_target_safe(&link, "", &d, Some(empty.as_slice())),
                "un symlink como main_path nunca se borra");
        }
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn removal_target_dos_componentes_se_niega() {
        // Rutas someras (/, /home, /tmp) nunca son carpeta de juego: <3
        // componentes = RootDir + a lo sumo un nombre.
        let d = vec!["/".to_string()];
        let empty: Vec<String> = vec![];
        let some = Some(empty.as_slice());
        assert!(!removal_target_safe("/", "", &d, some));
        if let Some(two) = two_component_dir() {
            assert!(!removal_target_safe(&two.display().to_string(), "", &d, some),
                "una ruta de 2 componentes ({}) no debe ser borrable", two.display());
        }
    }

    #[test]
    fn removal_target_exe_absoluto_fuera_de_main_niega() {
        // exe_rel absoluto: debe quedar dentro de main (no basta con que exista).
        let base = mktmp("corky_guard_exe_abs");
        let main = base.join("J");
        std::fs::create_dir_all(&main).unwrap();
        std::fs::write(base.join("evil.exe"), b"x").unwrap();
        let d = vec!["/".to_string()];
        let empty: Vec<String> = vec![];
        let some = Some(empty.as_slice());
        let evil = base.join("evil.exe").display().to_string();
        assert!(!removal_target_safe(&main.display().to_string(), &evil, &d, some),
            "un exe absoluto fuera de main no se aprueba");
        // El mismo exe dentro de main sí.
        std::fs::write(main.join("ok.exe"), b"x").unwrap();
        let ok = main.join("ok.exe").display().to_string();
        assert!(removal_target_safe(&main.display().to_string(), &ok, &d, some));
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn removal_target_juego_de_tres_componentes_permitido() {
        // Modela un juego legítimo tipo /mnt/juegos/X: no se niega por profundidad.
        let top = mktmp("corky_mnt");
        let game = top.join("X");
        std::fs::create_dir_all(&game).unwrap();
        let d = vec!["/".to_string()];
        let empty: Vec<String> = vec![];
        let some = Some(empty.as_slice());
        assert!(removal_target_safe(&game.display().to_string(), "", &d, some),
            "un juego con >=3 nombres debe permitirse");
        // Frontera exacta: 3 componentes (RootDir + 2 nombres), p.ej. /tmp/foo.
        let three = std::env::temp_dir().join("corky_mnt_x");
        let _ = std::fs::remove_dir_all(&three);
        std::fs::create_dir_all(&three).unwrap();
        assert!(removal_target_safe(&three.display().to_string(), "", &d, some),
            "3 componentes (RootDir+2) debe permitirse");
        let _ = std::fs::remove_dir_all(&three);
        let _ = std::fs::remove_dir_all(&top);
    }

    #[test]
    fn removal_target_juego_dentro_de_games_sigue_permitido() {
        // ~/Games/FNAF (carpeta compartida): la raíz ~/Games se niega, un juego
        // adentro no.
        let root = mktmp("corky_guard_fnaf");
        let home = root.join("home");
        let games = home.join("Games");
        let fnaf = games.join("FNAF");
        std::fs::create_dir_all(&fnaf).unwrap();
        let deny = store_deny_roots(&games.display().to_string(), &home.display().to_string());
        let empty: Vec<String> = vec![];
        let some = Some(empty.as_slice());
        assert!(!removal_target_safe(&games.display().to_string(), "", &deny, some),
            "la raíz de la biblioteca no se borra");
        assert!(removal_target_safe(&fnaf.display().to_string(), "", &deny, some),
            "un juego dentro de ~/Games sí se aprueba");
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn confirm_files_removal_niega_con_motivo() {
        // La decisión del else (redirect del handler) es pura y testeable.
        let d = vec!["/".to_string()];
        let empty: Vec<String> = vec![];
        let some = Some(empty.as_slice());
        // Ruta insegura → Err con el motivo exacto que ve/registra el usuario.
        let err = confirm_files_removal("/", "", &d, some).unwrap_err();
        assert_eq!(err, FILES_NOT_VERIFIED_MSG);
        assert!(err.contains("Uncheck 'Remove game files'"));
        // Ruta válida → Ok.
        let base = mktmp("corky_confirm_ok");
        std::fs::create_dir_all(base.join("sub")).unwrap();
        assert!(confirm_files_removal(&base.display().to_string(), "", &d, some).is_ok());
        let _ = std::fs::remove_dir_all(&base);
    }
}

pub fn show_remove_modal(
    state: &AppState,
    parent: &adw::ApplicationWindow,
    sidebar: &Rc<RefCell<Option<Sidebar>>>,
    center: &Rc<RefCell<Option<CenterHandle>>>,
    details: &Rc<RefCell<Option<DetailsPanel>>>,
) {
    let selected = state.selected_game.borrow().clone();
    if selected.is_empty() { return; }

    let dialog = adw::Dialog::new();
    dialog.set_title("Remove Game");
    dialog.set_content_width(460);
    dialog.set_content_height(320);

    let content = gtk::Box::new(gtk::Orientation::Vertical, 0);
    content.add_css_class("modal-bg");
    content.set_hexpand(true);
    content.set_vexpand(true);
    let inner = gtk::Box::new(gtk::Orientation::Vertical, 12);
    inner.set_hexpand(true);
    inner.set_vexpand(true);
    inner.set_margin_top(12);
    inner.set_margin_bottom(12);
    inner.set_margin_start(16);
    inner.set_margin_end(16);
    content.append(&inner);

    // Header with title + X (launcher reference style)
    let (header_row, x_btn) = helpers::modal_header("Remove Game");
    header_row.set_margin_top(4);
    {
        let dlg = dialog.clone();
        x_btn.connect_clicked(move |_| { dlg.close(); });
    }
    inner.append(&header_row);

    let msg = gtk::Label::new(Some(&format!("Remove \"{}\" from launcher?", selected)));
    msg.set_wrap(true);
    msg.set_halign(gtk::Align::Start);
    inner.append(&msg);

    // Check if game is an emulator game (has executor, no proton)
    let is_emu = state.config.game_value(&selected, "Executor")
        .map(|v| !v.is_empty())
        .unwrap_or(false);
    // UseSharedPrefix: other games live in that prefix too, so it's never
    // deleted from this dialog.
    let use_shared_prefix = state.config.game_value(&selected, "UseSharedPrefix")
        .map(|v| use_shared_prefix_on(&v))
        .unwrap_or(false);

    let chk_prefix = gtk::CheckButton::with_label("Remove game prefix (Wine/Proton data)");
    chk_prefix.set_visible(!is_emu && !use_shared_prefix);
    inner.append(&chk_prefix);
    // Path-level backstop: even if the flag above misfires (hand-edited ini),
    // a shared or foreign prefix can never be armed for deletion. The click
    // handler re-checks the same predicate.
    let warn_prefix = gtk::Label::new(None);
    warn_prefix.set_wrap(true);
    warn_prefix.set_halign(gtk::Align::Start);
    warn_prefix.add_css_class("time-label");
    warn_prefix.set_visible(false);
    inner.append(&warn_prefix);
    {
        // Never arm deletion without an explicit stored path: the fallback
        // would point at a default dir the game never used.
        let explicit = state
            .config
            .game_value(&selected, "PrefixPath")
            .map(|v| !v.trim().is_empty())
            .unwrap_or(false);
        let prefix_now = state.proton.prefix_path(&selected);
        if !explicit || (chk_prefix.is_visible() && !prefix_delete_safe(state, &selected, &prefix_now)) {
            chk_prefix.set_active(false);
            chk_prefix.set_sensitive(false);
            warn_prefix.set_text("Shared prefix: files will be kept. Remove unregisters the game only.");
            warn_prefix.set_visible(true);
        }
    }

    let chk_files = gtk::CheckButton::with_label("Remove game files from disk");
    inner.append(&chk_files);
    // Guard: la carpeta debe ser la del juego (con su exe adentro), nunca
    // la base compartida ni un padre. Si no es segura, se bloquea el
    // borrado y Remove solo quita del registro.
    let warn_files = gtk::Label::new(None);
    warn_files.set_wrap(true);
    warn_files.set_halign(gtk::Align::Start);
    warn_files.add_css_class("time-label");
    warn_files.set_visible(false);
    inner.append(&warn_files);
    {
        let home = std::env::var("HOME").unwrap_or_default();
        let games_root = crate::ui::import_manager::games_root().display().to_string();
        let deny = store_deny_roots(&games_root, &home);

        let main = state.config.game_value(&selected, "MainPath").unwrap_or_default();
        let exe = state.config.game_value(&selected, "Executable").unwrap_or_default();
        let others = others_except(state, &selected, &main);
        if !removal_target_safe(&main, &exe, &deny, others.as_deref()) {
            chk_files.set_active(false);
            chk_files.set_sensitive(false);
            warn_files.set_text("Game folder not verified: files will be kept. Remove unregisters the game only.");
            warn_files.set_visible(true);
        }
    }

    let spacer = gtk::Box::new(gtk::Orientation::Vertical, 0);
    spacer.set_vexpand(true);
    inner.append(&spacer);

    let btn_box = gtk::Box::new(gtk::Orientation::Horizontal, 10);
    btn_box.set_halign(gtk::Align::Fill);

    // Fixed neon colors (NOT theme affected)
    let no_btn = gtk::Button::with_label("Cancel");
    no_btn.add_css_class("neon-green");
    no_btn.set_hexpand(true);
    let yes_btn = gtk::Button::with_label("Remove");
    yes_btn.add_css_class("neon-red");
    yes_btn.set_hexpand(true);

    let state_clone = state.clone();
    let dialog_clone = dialog.clone();
    let sel = selected.clone();
    let sidebar_clone = sidebar.clone();
    let center_clone = center.clone();
    let details_clone = details.clone();
    let warn_prefix_c = warn_prefix.clone();
    let warn_files_c = warn_files.clone();
    yes_btn.connect_clicked(move |_| {
        let remove_prefix = chk_prefix.is_active();
        let remove_files = chk_files.is_active();

        if remove_prefix {
            let prefix = state_clone.proton.prefix_path(&sel);
            if prefix.exists() {
                // Re-validate by path, not by checkbox: the flag may have
                // changed (or lied) since the dialog opened. On denial the
                // dialog stays open with the reason visible; nothing is
                // deleted.
                if !prefix_delete_safe(&state_clone, &sel, &prefix) {
                    warn_prefix_c.set_text("Shared prefix: files will be kept. Remove unregisters the game only.");
                    warn_prefix_c.set_visible(true);
                    return;
                }
                if let Err(e) = state_clone
                    .integration
                    .remove_dir_recursive(&prefix.display().to_string())
                {
                    warn_prefix_c.set_text(&format!(
                        "Could not delete prefix: {}. Nothing was unregistered.",
                        e
                    ));
                    warn_prefix_c.set_visible(true);
                    return;
                }
            }
        }
        if remove_files {
            if let Some(g) = state_clone.game_model.get_game(&sel) {
                let home = std::env::var("HOME").unwrap_or_default();
                let games_root = crate::ui::import_manager::games_root().display().to_string();
                let deny = store_deny_roots(&games_root, &home);

                let others = others_except(&state_clone, &sel, &g.main_path);
                match confirm_files_removal(&g.main_path, &g.executable, &deny, others.as_deref()) {
                    Ok(()) => {
                        if let Err(e) = state_clone.integration.remove_dir_recursive(&g.main_path) {
                            warn_files_c.set_text(&format!(
                                "Could not delete game files: {}. The game was not unregistered, but some files may have been partially deleted.",
                                e
                            ));
                            warn_files_c.set_visible(true);
                            return;
                        }
                    }
                    Err(reason) => {
                        eprintln!("[remove] files not verified, keeping them (reason: {}): {}",
                                  reason, g.main_path);
                        warn_files_c.set_text(reason);
                        warn_files_c.set_visible(true);
                        return;
                    }
                }
            }
        }

        state_clone.game_model.remove_game(&sel);
        // The removed game must leave the header warning counters without a
        // restart (Refresh reads from the game model, where it no longer is).
        crate::refresh_warn_buttons(&state_clone);
        // Las tarjetas de Stores se refrescan por la misma ruta que
        // Refresh manual (hook registrado por StoresView).
        if let Some(f) = state_clone.stores_changed.borrow().as_ref() {
            f();
        }
        crate::backend::shortcuts::cleanup(&sel);
        state_clone.recent_model.refresh(30);
        *state_clone.selected_game.borrow_mut() = String::new();

        if let Some(ref sb) = *sidebar_clone.borrow() {
            sb.apply_current_filter();
        }
        // The card must vanish from Recently Played too, and the details
        // panel must close (it would otherwise open for a deleted game).
        if let Some(ref c) = *center_clone.borrow() {
            c.rebuild(&state_clone, &details_clone);
        }
        if let Some(ref d) = *details_clone.borrow() {
            d.hide();
        }

        dialog_clone.close();
    });

    let dialog_clone2 = dialog.clone();
    no_btn.connect_clicked(move |_| { dialog_clone2.close(); });

    btn_box.append(&no_btn);
    btn_box.append(&yes_btn);
    inner.append(&btn_box);

    dialog.set_child(Some(&content));
    dialog.present(Some(parent));
}
