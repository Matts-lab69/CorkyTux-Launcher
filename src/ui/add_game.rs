use adw::prelude::*;
use gtk::prelude::*;
use std::cell::RefCell;
use std::rc::Rc;

use crate::backend::game_model::{GameEntry, GameSource};
use crate::ui::sidebar::Sidebar;
use crate::ui::center::CenterHandle;
use crate::ui::details_panel::DetailsPanel;
use crate::AppState;

#[derive(Default)]
struct WizardData {
    executor: String,
    is_emu: bool,
    exe: String,
    dir: String,
    candidates: Vec<String>,
}

/// AddGame wizard (C++ AddGameModal parity):
/// step 1 runner -> step 2 executable -> step 3 game info.
/// Steam ID and install path are auto-detected, not asked.
pub fn show_add_game_modal(
    state: &AppState,
    parent: &adw::ApplicationWindow,
    sidebar: &Rc<RefCell<Option<Sidebar>>>,
    center: &Rc<RefCell<Option<CenterHandle>>>,
    details: &Rc<RefCell<Option<DetailsPanel>>>,
) {
    let dialog = adw::Dialog::new();
    dialog.set_title("Add Game");
    dialog.set_content_width(440);
    dialog.set_content_height(500);

    let outer = gtk::Box::new(gtk::Orientation::Vertical, 0);
    outer.add_css_class("modal-bg");
    outer.set_hexpand(true);
    outer.set_vexpand(true);

    // Full-bleed background: padding lives on the inner box so the
    // Adwaita dialog background never shows through as a gray frame.
    let inner = gtk::Box::new(gtk::Orientation::Vertical, 10);
    inner.set_hexpand(true);
    inner.set_vexpand(true);
    inner.set_margin_top(12);
    inner.set_margin_bottom(12);
    inner.set_margin_start(16);
    inner.set_margin_end(16);

    let (header_row, x_btn) = crate::ui::helpers::modal_header("Add Game");
    {
        let dlg = dialog.clone();
        x_btn.connect_clicked(move |_| { dlg.close(); });
    }
    inner.append(&header_row);

    let stack = gtk::Stack::new();
    stack.set_vexpand(true);
    stack.set_transition_type(gtk::StackTransitionType::SlideLeftRight);
    inner.append(&stack);
    outer.append(&inner);

    let wiz: Rc<RefCell<WizardData>> = Rc::new(RefCell::new(WizardData::default()));
    let goto = {
        let stack_c = stack.clone();
        Rc::new(move |page: &str| stack_c.set_visible_child_name(page))
    };

    // Step 2 title labels live early so step-1 cards can retitle them.
    let exe_title = gtk::Label::new(Some("Select executable"));
    exe_title.set_halign(gtk::Align::Center);
    exe_title.add_css_class("details-title");
    let exe_sub = gtk::Label::new(Some("Choose the main file that starts the game"));
    exe_sub.set_halign(gtk::Align::Center);
    exe_sub.set_opacity(0.6);
    exe_sub.add_css_class("time-label");

    // ================= Step 1: executor =================
    let page1 = gtk::Box::new(gtk::Orientation::Vertical, 10);
    let exec_title = gtk::Label::new(Some("Select executor"));
    exec_title.set_halign(gtk::Align::Center);
    exec_title.add_css_class("details-title");
    page1.append(&exec_title);
    let exec_sub = gtk::Label::new(Some("How do you want to run this game?"));
    exec_sub.set_halign(gtk::Align::Center);
    exec_sub.set_opacity(0.6);
    exec_sub.add_css_class("time-label");
    page1.append(&exec_sub);

    let cards_box = gtk::Box::new(gtk::Orientation::Vertical, 8);
    page1.append(&cards_box);
    let mk_card = |title: &str, sub: &str| -> (gtk::Button, gtk::Box) {
        let card = gtk::Button::new();
        card.add_css_class("filter-btn");
        card.set_halign(gtk::Align::Fill);
        card.set_height_request(50);
        let vbox = gtk::Box::new(gtk::Orientation::Vertical, 2);
        let lbl = gtk::Label::new(Some(title));
        lbl.set_halign(gtk::Align::Start);
        lbl.add_css_class("details-title");
        let s = gtk::Label::new(Some(sub));
        s.set_halign(gtk::Align::Start);
        s.set_opacity(0.6);
        s.add_css_class("time-label");
        vbox.append(&lbl);
        vbox.append(&s);
        card.set_child(Some(&vbox));
        (card, vbox)
    };
    let (wine_card, _) = mk_card("Wine / Proton", "Run .exe / .msi games via Proton or Wine");
    cards_box.append(&wine_card);
    {
        let wiz_c = wiz.clone();
        let goto_c = goto.clone();
        let title_c = exe_title.clone();
        let sub_c = exe_sub.clone();
        wine_card.connect_clicked(move |_| {
            let mut w = wiz_c.borrow_mut();
            w.executor.clear();
            w.is_emu = false;
            drop(w);
            title_c.set_text("Select executable");
            sub_c.set_text("Choose the main file that starts the game");
            goto_c("exe");
        });
    }
    let (app_card, _) = mk_card("AppImage", "Run portable .AppImage apps natively");
    cards_box.append(&app_card);
    {
        let wiz_c = wiz.clone();
        let goto_c = goto.clone();
        let title_c = exe_title.clone();
        let sub_c = exe_sub.clone();
        app_card.connect_clicked(move |_| {
            let mut w = wiz_c.borrow_mut();
            w.executor = "appimage-launcher".to_string();
            w.is_emu = false;
            drop(w);
            title_c.set_text("Select AppImage");
            sub_c.set_text("Choose the .AppImage file that starts the app");
            goto_c("exe");
        });
    }
    let (rpg_card, _) = mk_card("RPG Maker", "Run RPG Maker MV/MZ/2k3 games via box-rpg");
    cards_box.append(&rpg_card);
    {
        let wiz_c = wiz.clone();
        let goto_c = goto.clone();
        let title_c = exe_title.clone();
        let sub_c = exe_sub.clone();
        rpg_card.connect_clicked(move |_| {
            let mut w = wiz_c.borrow_mut();
            w.executor = "rpgmaker-runtime".to_string();
            w.is_emu = false;
            drop(w);
            title_c.set_text("Select RPG Maker folder");
            sub_c.set_text("Choose the folder containing the game");
            goto_c("exe");
        });
    }
    // Installed emulators only (C++: emulators.filter(e => e.installed))
    for (emu_id, emu_name) in state.plugins.list_emulator_plugins() {
        let (emu_card, _) = mk_card(&emu_name, &format!("Run {} ROMs", emu_name));
        cards_box.append(&emu_card);
        let wiz_c = wiz.clone();
        let goto_c = goto.clone();
        let title_c = exe_title.clone();
        let sub_c = exe_sub.clone();
        let emu_id_c = emu_id.clone();
        let emu_name_c = emu_name.clone();
        emu_card.connect_clicked(move |_| {
            let mut w = wiz_c.borrow_mut();
            w.executor = emu_id_c.clone();
            w.is_emu = true;
            drop(w);
            title_c.set_text(&format!("Select {} ROM", emu_name_c));
            sub_c.set_text("Choose the ROM file that starts the game");
            goto_c("exe");
        });
    }
    let p1_cancel = gtk::Button::with_label("Cancel");
    p1_cancel.add_css_class("settings-btn");
    p1_cancel.set_halign(gtk::Align::Fill);
    {
        let dlg = dialog.clone();
        p1_cancel.connect_clicked(move |_| { dlg.close(); });
    }
    page1.append(&p1_cancel);
    stack.add_named(&page1, Some("exec"));

    // ================= Step 2: executable (C++ pickBox parity) =================
    let page2 = gtk::Box::new(gtk::Orientation::Vertical, 10);
    page2.append(&exe_title);
    page2.append(&exe_sub);

    // C++ parity: single full-width "Choose Folder…" button
    let choose_btn = gtk::Button::with_label("Choose Folder…");
    choose_btn.add_css_class("settings-btn");
    choose_btn.set_halign(gtk::Align::Fill);
    page2.append(&choose_btn);

    // Candidate list: rounded dark well, 50px two-line rows (name + dir)
    let list_well = gtk::Box::new(gtk::Orientation::Vertical, 6);
    list_well.add_css_class("cand-list");
    let cand_scroll = gtk::ScrolledWindow::new();
    cand_scroll.set_min_content_height(220);
    cand_scroll.set_hscrollbar_policy(gtk::PolicyType::Never);
    cand_scroll.set_propagate_natural_height(false);
    let rows_box = gtk::Box::new(gtk::Orientation::Vertical, 6);
    rows_box.set_hexpand(true);
    cand_scroll.set_child(Some(&rows_box));
    list_well.append(&cand_scroll);
    page2.append(&list_well);

    let exe_error = gtk::Label::new(None);
    exe_error.set_halign(gtk::Align::Center);
    exe_error.add_css_class("time-label");
    exe_error.set_visible(false);
    page2.append(&exe_error);

    // C++ parity: single centered 144px accent Next pill
    let p2_nav = gtk::Box::new(gtk::Orientation::Horizontal, 10);
    p2_nav.set_halign(gtk::Align::Center);
    let p2_back = gtk::Button::with_label("Back");
    p2_back.add_css_class("settings-btn");
    p2_back.set_width_request(100);
    let p2_next = gtk::Button::with_label("Next");
    p2_next.add_css_class("add-btn");
    p2_next.set_width_request(144);
    p2_nav.append(&p2_back);
    p2_nav.append(&p2_next);
    page2.append(&p2_nav);
    stack.add_named(&page2, Some("exe"));

    // Rebuild candidate rows (C++ candList delegate parity)
    let refresh_candidates = {
        let wiz_c = wiz.clone();
        let rows_c = rows_box.clone();
        let err_c = exe_error.clone();
        Rc::new(move || {
            while let Some(child) = rows_c.first_child() {
                rows_c.remove(&child);
            }
            err_c.set_visible(false);
            let found = wiz_c.borrow().candidates.clone();
            if found.is_empty() {
                let empty = gtk::Label::new(Some("No runnable files — choose another folder"));
                empty.set_halign(gtk::Align::Center);
                empty.set_opacity(0.6);
                empty.add_css_class("time-label");
                rows_c.append(&empty);
                return;
            }
            let selected = wiz_c.borrow().exe.clone();
            for f in &found {
                let row_btn = gtk::Button::new();
                row_btn.add_css_class("filter-btn");
                row_btn.set_halign(gtk::Align::Fill);
                row_btn.set_height_request(50);
                let col = gtk::Box::new(gtk::Orientation::Vertical, 0);
                col.set_margin_top(5);
                col.set_margin_bottom(5);
                col.set_margin_start(12);
                col.set_margin_end(8);
                let name_lbl = gtk::Label::new(Some(
                    &std::path::Path::new(f)
                        .file_name()
                        .map(|n| n.to_string_lossy().to_string())
                        .unwrap_or_else(|| f.clone()),
                ));
                name_lbl.set_halign(gtk::Align::Start);
                name_lbl.add_css_class("details-title");
                name_lbl.set_ellipsize(gtk::pango::EllipsizeMode::Middle);
                let dir_lbl = gtk::Label::new(Some(
                    &std::path::Path::new(f)
                        .parent()
                        .map(|p| p.display().to_string())
                        .unwrap_or_default(),
                ));
                dir_lbl.set_halign(gtk::Align::Start);
                dir_lbl.set_opacity(0.6);
                dir_lbl.add_css_class("time-label");
                dir_lbl.set_ellipsize(gtk::pango::EllipsizeMode::Middle);
                col.append(&name_lbl);
                col.append(&dir_lbl);
                row_btn.set_child(Some(&col));
                if *f == selected {
                    row_btn.add_css_class("card-selected");
                }
                let f_c = f.clone();
                let wiz_cc = wiz_c.clone();
                let rows_cc = rows_c.clone();
                let btn_c = row_btn.clone();
                row_btn.connect_clicked(move |_| {
                    wiz_cc.borrow_mut().exe = f_c.clone();
                    let mut child = rows_cc.first_child();
                    while let Some(c) = child {
                        if let Ok(b) = c.clone().downcast::<gtk::Button>() {
                            b.remove_css_class("card-selected");
                        }
                        child = c.next_sibling();
                    }
                    btn_c.add_css_class("card-selected");
                });
                rows_c.append(&row_btn);
            }
        })
    };
    // Scan: pick folder, fill candidates, preselect first (C++ parity)
    {
        let state_c = state.clone();
        let wiz_c = wiz.clone();
        let refresh_c = refresh_candidates.clone();
        choose_btn.connect_clicked(move |_| {
            if let Some(dir) = state_c.config.pick_folder("Select game folder") {
                let dir_str = dir.display().to_string();
                let (emu, exec) = {
                    let w = wiz_c.borrow();
                    (w.is_emu, w.executor.clone())
                };
                if exec == "rpgmaker-runtime" {
                    let mut w = wiz_c.borrow_mut();
                    w.dir = dir_str.clone();
                    w.candidates = vec![dir_str.clone()];
                    w.exe = dir_str;
                    drop(w);
                    refresh_c();
                    return;
                }
                let found = scan_candidates(&dir_str, emu, &exec);
                let mut w = wiz_c.borrow_mut();
                w.dir = dir_str;
                w.candidates = found.clone();
                w.exe = found.first().cloned().unwrap_or_default();
                drop(w);
                refresh_c();
            }
        });
    }
    // ================= Step 3: game info =================
    let page3 = gtk::Box::new(gtk::Orientation::Vertical, 10);
    let info_title = gtk::Label::new(Some("Game Info"));
    info_title.set_halign(gtk::Align::Center);
    info_title.add_css_class("details-title");
    page3.append(&info_title);

    let name_entry = gtk::Entry::new();
    name_entry.set_placeholder_text(Some("Game name in launcher"));
    name_entry.set_hexpand(true);
    name_entry.add_css_class("path-entry");
    page3.append(&name_entry);
    let name_error = gtk::Label::new(None);
    name_error.set_halign(gtk::Align::Start);
    name_error.add_css_class("time-label");
    name_error.set_visible(false);
    page3.append(&name_error);
    {
        let err = name_error.clone();
        name_entry.connect_changed(move |_| err.set_visible(false));
    }

    let prefix_entry = gtk::Entry::new();
    prefix_entry.set_placeholder_text(Some("Prefix path (auto if empty)"));
    prefix_entry.set_hexpand(true);
    prefix_entry.add_css_class("path-entry");
    page3.append(&prefix_entry);

    let protons = state.proton.installed_protons();
    let proton_dropdown = gtk::DropDown::new(None::<gtk::StringList>, None::<gtk::Expression>);
    let sl = gtk::StringList::new(&[]);
    for p in &protons { sl.append(p); }
    proton_dropdown.set_model(Some(&sl));
    let def = state.config.launcher_value("defaultProton").unwrap_or_default();
    if let Some(idx) = protons.iter().position(|p| *p == def) {
        proton_dropdown.set_selected(idx as u32);
    }
    let proton_lbl = gtk::Label::new(Some("Proton Version"));
    proton_lbl.set_halign(gtk::Align::Start);
    page3.append(&proton_lbl);
    page3.append(&proton_dropdown);

    let icon_box = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    let icon_entry = gtk::Entry::new();
    icon_entry.set_hexpand(true);
    icon_entry.set_placeholder_text(Some("Icon image path (optional)"));
    icon_entry.add_css_class("path-entry");
    let icon_browse = gtk::Button::with_label("Browse");
    icon_browse.add_css_class("settings-btn");
    icon_box.append(&icon_entry);
    icon_box.append(&icon_browse);
    let icon_lbl = gtk::Label::new(Some("Game Icon"));
    icon_lbl.set_halign(gtk::Align::Start);
    page3.append(&icon_lbl);
    page3.append(&icon_box);
    {
        let state_c = state.clone();
        let icon_e = icon_entry.clone();
        icon_browse.connect_clicked(move |_| {
            if let Some(path) = state_c.config.pick_file("Select icon image", &["png", "jpg", "jpeg", "svg"]) {
                icon_e.set_text(&path.display().to_string());
            }
        });
    }

    let banner_box = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    let banner_entry = gtk::Entry::new();
    banner_entry.set_hexpand(true);
    banner_entry.set_placeholder_text(Some("Banner image path (optional)"));
    banner_entry.add_css_class("path-entry");
    let banner_browse = gtk::Button::with_label("Browse");
    banner_browse.add_css_class("settings-btn");
    banner_box.append(&banner_entry);
    banner_box.append(&banner_browse);
    let banner_lbl = gtk::Label::new(Some("Game Banner"));
    banner_lbl.set_halign(gtk::Align::Start);
    page3.append(&banner_lbl);
    page3.append(&banner_box);
    {
        let state_c = state.clone();
        let banner_e = banner_entry.clone();
        banner_browse.connect_clicked(move |_| {
            if let Some(path) = state_c.config.pick_file("Select banner image", &["png", "jpg", "jpeg"]) {
                banner_e.set_text(&path.display().to_string());
            }
        });
    }

    let p3_nav = gtk::Box::new(gtk::Orientation::Horizontal, 10);
    p3_nav.set_halign(gtk::Align::Fill);
    let p3_back = gtk::Button::with_label("Back");
    p3_back.add_css_class("settings-btn");
    p3_back.set_hexpand(true);
    let add_btn = gtk::Button::with_label("Add");
    add_btn.add_css_class("add-btn");
    add_btn.set_hexpand(true);
    p3_nav.append(&p3_back);
    p3_nav.append(&add_btn);
    page3.append(&p3_nav);
    stack.add_named(&page3, Some("info"));

    // ---- Navigation ----
    {
        let goto_c = goto.clone();
        p2_back.connect_clicked(move |_| { goto_c("exec"); });
    }
    {
        let goto_c = goto.clone();
        p3_back.connect_clicked(move |_| { goto_c("exe"); });
    }
    // exe step title tracks the chosen runner (set by step-1 cards)
    {
        // Next from step 2: require an executable, prefill step 3
        let wiz_c = wiz.clone();
        let goto_c = goto.clone();
        let err = exe_error.clone();
        let state_rar_c = state.clone();
        let refresh_rar_c = refresh_candidates.clone();
        let name_e = name_entry.clone();
        let prefix_e = prefix_entry.clone();
        let proton_d = proton_dropdown.clone();
        p2_next.connect_clicked(move |_| {
            let w = wiz_c.borrow();
            let exe = w.exe.trim().to_string();
            let is_emu = w.is_emu;
            let executor_c = w.executor.clone();
            drop(w);
            let is_appimage = executor_c == "appimage-launcher";
            let is_rpg = executor_c == "rpgmaker-runtime";
            if exe.is_empty() {
                err.set_text("Pick a file from the list first");
                err.set_visible(true);
                return;
            }
            // C++ parity: .rar extracts first, then the unpacked files list
            if exe.to_lowercase().ends_with(".rar") {
                let path = std::path::PathBuf::from(&exe);
                let stem = path.file_stem()
                    .map(|s| s.to_string_lossy().to_string())
                    .unwrap_or_else(|| "game".to_string());
                let home = std::env::var("HOME").unwrap_or_default();
                let dest = format!("{}/.local/share/CorkyTux/installs/{}", home, stem);
                if state_rar_c.integration.extract_rar(&exe, &dest).is_ok() {
                    let found = scan_candidates(&dest, is_emu, &executor_c);
                    let mut w = wiz_c.borrow_mut();
                    w.dir = dest;
                    w.exe = found.first().cloned().unwrap_or_default();
                    w.candidates = found;
                    drop(w);
                    refresh_rar_c();
                } else {
                    err.set_text("Extraction failed");
                    err.set_visible(true);
                }
                return;
            }
            if name_e.text().trim().is_empty() {
                if let Some(stem) = std::path::Path::new(&exe).file_stem() {
                    name_e.set_text(&stem.to_string_lossy());
                }
            }
            // Proton/prefix rows only make sense for Wine games
            prefix_e.set_visible(!is_emu && !is_appimage && !is_rpg);
            proton_d.set_visible(!is_emu && !is_appimage && !is_rpg);
            proton_lbl.set_visible(!is_emu && !is_appimage && !is_rpg);
            goto_c("info");
        });
    }

    // ---- Add (step 3) ----
    {
        let state_c = state.clone();
        let wiz_c = wiz.clone();
        let dlg = dialog.clone();
        let name_e = name_entry.clone();
        let prefix_e = prefix_entry.clone();
        let proton_d = proton_dropdown.clone();
        let protons_c = protons.clone();
        let icon_e = icon_entry.clone();
        let banner_e = banner_entry.clone();
        let sidebar_c = sidebar.clone();
        let center_c = center.clone();
        let details_c = details.clone();
        let parent_c = parent.clone();
        let name_err = name_error.clone();
        add_btn.connect_clicked(move |btn| {
            if !btn.is_sensitive() { return; }
            let name = name_e.text().to_string().trim().to_string();
            if name.is_empty() {
                name_err.set_text("Game name is required");
                name_err.set_visible(true);
                return;
            }
            if state_c.game_model.get_game(&name).is_some() {
                name_err.set_text("A game with this name already exists");
                name_err.set_visible(true);
                return;
            }
            btn.set_sensitive(false);
            let w = wiz_c.borrow();
            let executable = w.exe.trim().to_string();
            // Install path auto-detected: scan dir, else exe parent dir
            let main_path = if !w.dir.trim().is_empty() {
                w.dir.trim().to_string()
            } else if let Some(parent) = std::path::Path::new(&executable).parent() {
                parent.display().to_string()
            } else {
                String::new()
            };
            let is_emu = w.is_emu;
            let executor = w.executor.clone();
            drop(w);
            let is_appimage = executor == "appimage-launcher";
            let is_rpg = executor == "rpgmaker-runtime";
            let mut prefix = prefix_e.text().to_string().trim().to_string();
            // C++ parity: default individual prefix is prefixesDir/gameName
            if !is_emu && !is_appimage && !is_rpg && prefix.is_empty() {
                prefix = state_c.config.base_path_for("prefixes")
                    .join(&name).display().to_string();
            }
            let proton_idx = proton_d.selected();
            let mut proton_name = if (proton_idx as usize) < protons_c.len() {
                protons_c[proton_idx as usize].clone()
            } else { String::new() };
            if proton_name.is_empty() {
                proton_name = state_c.config.launcher_value("defaultProton").unwrap_or_default();
            }
            // C++ parity: new games inherit the global Misc defaults
            let flag = |key: &str, fallback: bool| {
                state_c.config.launcher_value(key)
                    .map(|v| v == "1")
                    .unwrap_or(fallback)
            };
            // C++ parity: pull the real embedded icon out of the .exe
            // when the user did not pick one (icoextract+ffmpeg).
            let mut icon = icon_e.text().to_string().trim().to_string();
            if !is_emu && !is_appimage && !is_rpg && icon.is_empty() {
                if executable.to_lowercase().ends_with(".exe") {
                    if let Some(found) = state_c.integration.extract_exe_icon(&executable, &name) {
                        icon = found;
                    }
                }
            }
            // AppImages never use online artwork: embedded .DirIcon only.
            if is_appimage && icon.is_empty() {
                if let Some(found) = state_c.integration.extract_appimage_icon(&executable, &name) {
                    icon = found;
                }
            }
            // RPG Maker games use their own icon/icon.png, never exe icons.
            if is_rpg && icon.is_empty() {
                if let Some(found) = state_c.integration.extract_rpg_icon(&executable, &name) {
                    icon = found;
                }
            }
            let banner = banner_e.text().to_string().trim().to_string();
            let art_name = name.clone();
            let art_dir = main_path.clone();
            let entry = GameEntry {
                name, executable, main_path,
                prefix_path: if is_emu || is_appimage || is_rpg { String::new() } else { prefix },
                proton: if is_emu || is_appimage || is_rpg { String::new() } else { proton_name },
                overrides: String::new(), steam_id: String::new(),
                banner, icon,
                time_spent: 0, last_played: 0, favorite: false,
                source: if is_appimage {
                    GameSource::AppImage
                } else if is_rpg {
                    GameSource::RpgMaker
                } else {
                    GameSource::Manual
                },
                executor: if is_emu || is_appimage || is_rpg { executor } else { String::new() },
                emu_settings: std::collections::HashMap::new(),
                lutris_runner: String::new(),
                environment: String::new(),
                args_before: String::new(),
                args_after: String::new(),
                steam_overlay: false,
                steam_runtime: flag("gamesUsesSteamRuntime", true),
                use_umu: flag("gamesUsesUmu", false),
                use_shared_prefix: false,
                shared_prefix_name: String::new(),
                wined3d: flag("gamesUsesWined3d", false),
                native_wayland: flag("gamesUsesWayland", false),
                game_mode: flag("gamesUsesGameMode", false),
                mango_hud: flag("gamesUsesMangoHud", false),
                lutris_slug: String::new(),
                fake_steam_id: String::new(),
                install_size: String::new(),
                heroic_store: String::new(),
                heroic_app_id: String::new(),
                heroic_eac: false,
                heroic_battleye: false,
            };
            state_c.game_model.add_game(entry);
            state_c.recent_model.refresh(30);
            if let Some(ref sb) = *sidebar_c.borrow() {
                let names = state_c.game_model.ordered_names();
                sb.refresh_list(&names);
            }
            if let Some(ref c) = *center_c.borrow() {
                c.rebuild(&state_c, &details_c);
            }
            // C++ parity (AddGameModal.qml:101): auto-resolve artwork in a
            // background thread, then patch banner/icon back on main thread.
            // The thread builds its own IntegrationManager (resolve_artwork
            // touches no self fields) and returns plain data over mpsc.
            // AppImages skip online lookup entirely (embedded icon only,
            // already extracted above).
            if is_appimage {
                crate::run_plugin_scans(&state_c, &parent_c, &art_name, &art_dir);
                dlg.close();
                return;
            }
            let (art_tx, art_rx) = std::sync::mpsc::channel::<(Option<String>, Option<String>)>();
            let art_name_send = art_name.clone();
            std::thread::spawn(move || {
                let fresh = crate::backend::integration::IntegrationManager::new();
                let (i, b) = fresh.resolve_artwork(&art_name_send, "");
                let _ = art_tx.send((i, b));
            });
            let gm_c2 = state_c.game_model.clone();
            let state_c3 = state_c.clone();
            let center_c3 = center_c.clone();
            let details_c3 = details_c.clone();
            let art_name2 = art_name.clone();
            glib::idle_add_local(move || {
                match art_rx.try_recv() {
                    Ok((i, b)) => {
                        let cur_icon = gm_c2.get_game(&art_name2)
                            .map(|g| g.icon).unwrap_or_default();
                        // Never clobber a real exe-extracted icon (C++ onArtworkReady).
                        let icon = if cur_icon.contains("-exe.png") {
                            String::new()
                        } else {
                            i.unwrap_or_default()
                        };
                        let banner = b.unwrap_or_default();
                        if !icon.is_empty() || !banner.is_empty() {
                            gm_c2.set_artwork(&art_name2, &banner, &icon);
                            if let Some(ref c) = *center_c3.borrow() {
                                c.rebuild(&state_c3, &details_c3);
                            }
                        }
                        glib::ControlFlow::Break
                    }
                    Err(std::sync::mpsc::TryRecvError::Empty) => glib::ControlFlow::Continue,
                    Err(_) => glib::ControlFlow::Break,
                }
            });
            // C++ applyScanPlugins parity: dep + dll scans (overrides apply,
            // missing deps ask). Skips emulator games internally.
            crate::run_plugin_scans(&state_c, &parent_c, &art_name, &art_dir);
            dlg.close();
        });
    }

    outer.set_vexpand(true);
    dialog.set_child(Some(&outer));
    dialog.present(Some(parent));
}

fn wine_candidates() -> Vec<&'static str> {
    vec!["exe", "msi", "bat", "sh", "bin", "rar"]
}

fn emu_candidates() -> Vec<&'static str> {
    vec![
        "zip", "7z", "rar", "nds", "n64", "z64", "v64", "sfc", "smc",
        "gb", "gbc", "gba", "nes", "nez", "iso", "cso", "wbfs", "rvz",
        "xci", "nsp", "cue", "bin", "chd", "pbp",
    ]
}

fn appimage_candidates() -> Vec<&'static str> {
    vec!["AppImage", "appimage"]
}

/// Scan a folder (2 levels deep) for runnable files, shortest names first
/// so the most likely main binary/ROM comes first.
fn scan_candidates(dir: &str, emu: bool, executor: &str) -> Vec<String> {
    let exts: Vec<&str> = if executor == "appimage-launcher" {
        appimage_candidates()
    } else if emu {
        emu_candidates()
    } else {
        wine_candidates()
    };
    let mut out = Vec::new();
    scan_dir_depth(std::path::Path::new(dir), &exts, 0, &mut out);
    out.sort_by(|a, b| {
        let la = std::path::Path::new(a)
            .file_name().map(|n| n.len()).unwrap_or(usize::MAX);
        let lb = std::path::Path::new(b)
            .file_name().map(|n| n.len()).unwrap_or(usize::MAX);
        la.cmp(&lb).then_with(|| a.cmp(b))
    });
    out.truncate(50);
    out
}

fn scan_dir_depth(dir: &std::path::Path, exts: &[&str], depth: u32, out: &mut Vec<String>) {
    if depth > 2 {
        return;
    }
    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return,
    };
    for entry in entries.flatten() {
        let p = entry.path();
        if p.is_file() {
            if let Some(ext) = p.extension().and_then(|e| e.to_str()) {
                let lower = ext.to_lowercase();
                if exts.iter().any(|e| *e == lower) {
                    out.push(p.display().to_string());
                }
            }
        } else if p.is_dir() && depth < 2 {
            scan_dir_depth(&p, exts, depth + 1, out);
        }
    }
}
