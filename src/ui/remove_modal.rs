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
    vec!["/".to_string(), home.trim_end_matches('/').to_string(),
         base.to_string(), format!("{}/Heroic", base),
         format!("{}/Epic-Games", base), format!("{}/GOG-Games", base)]
}

/// Canonicalizes if it exists; None if not (the caller decides: a missing
/// path is never approved for deletion).
fn canon(p: &std::path::Path) -> Option<std::path::PathBuf> {
    std::fs::canonicalize(p).ok()
}

/// Can `main_path` be deleted as "the game folder"?
/// Denies: empty/relative/missing, the deny list (after canonicalizing), an exe
/// outside main (relpath with an escaping `..`, a foreign absolute, a symlink)
/// and any overlap with another registered install (main inside the other, or
/// the other inside main). With an empty exe (manifest, Epic) the list +
/// overlap checks are the whole decision.
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
    // Canonicalize main: a failure on an existing path denies. A missing path
    // has nothing to delete, so it denies too (the lexical fallback only
    // compares already-resolved denies, and doesn't approve either).
    let main_c = match canon(std::path::Path::new(m)) {
        Some(p) => p,
        None => return false,
    };
    for d in deny_roots {
        let dd = d.trim_end_matches('/');
        if dd.is_empty() {
            continue;
        }
        let dp = std::path::Path::new(dd);
        if canon(dp).as_deref().unwrap_or(dp) == main_c {
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

#[cfg(test)]
mod tests {
    use super::removal_target_safe;

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
        .map(|v| v == "true")
        .unwrap_or(false);

    let chk_prefix = gtk::CheckButton::with_label("Remove game prefix (Wine/Proton data)");
    chk_prefix.set_visible(!is_emu && !use_shared_prefix);
    inner.append(&chk_prefix);

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
    yes_btn.connect_clicked(move |_| {
        let remove_prefix = chk_prefix.is_active();
        let remove_files = chk_files.is_active();

        if remove_prefix {
            let prefix = state_clone.proton.prefix_path(&sel);
            if prefix.exists() {
                let _ = state_clone.integration.remove_dir_recursive(&prefix.display().to_string());
            }
        }
        if remove_files {
            if let Some(g) = state_clone.game_model.get_game(&sel) {
                let home = std::env::var("HOME").unwrap_or_default();
                let games_root = crate::ui::import_manager::games_root().display().to_string();
                let deny = store_deny_roots(&games_root, &home);

                let others = others_except(&state_clone, &sel, &g.main_path);
                if removal_target_safe(&g.main_path, &g.executable, &deny, others.as_deref()) {
                    let _ = state_clone.integration.remove_dir_recursive(&g.main_path);
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
