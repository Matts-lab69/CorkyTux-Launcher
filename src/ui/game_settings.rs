use adw::prelude::*;
use gtk::prelude::*;
use std::cell::RefCell;
use std::rc::Rc;

use crate::backend::proton::ProtonManager;
use crate::ui::helpers;
use crate::AppState;

/// Per-game settings: everything writes through to Games.ini immediately,
/// so there is no Save/Cancel. Renames apply once when the dialog closes.
pub fn show_game_settings_modal(
    state: &AppState,
    parent: &adw::ApplicationWindow,
    game_name: &str,
    on_close: impl Fn() + 'static,
) {
    let dialog = adw::Dialog::new();
    dialog.set_title(&format!("{} — Settings", game_name));
    dialog.set_content_width(580);
    dialog.set_content_height(600);

    // Title header (launcher reference style)
    let (header_row, x_btn) = helpers::modal_header(&format!("{} — Settings", game_name));
    header_row.set_margin_top(8);
    header_row.set_margin_start(16);
    header_row.set_margin_end(8);
    {
        let dlg = dialog.clone();
        x_btn.connect_clicked(move |_| { dlg.close(); });
    }

    let content = gtk::Box::new(gtk::Orientation::Vertical, 0);
    content.add_css_class("modal-bg");
    content.append(&header_row);

    let page_stack = gtk::Stack::new();
    page_stack.set_vexpand(true);

    let executor_val = state.config.game_value(game_name, "Executor").unwrap_or_default();
    let is_rpg = executor_val == "rpgmaker-runtime";
    let is_emu = !executor_val.is_empty() && !is_rpg;

    let name_holder: std::rc::Rc<std::cell::RefCell<String>> =
        std::rc::Rc::new(std::cell::RefCell::new(game_name.to_string()));

    // === Tab 1: View — only name, icon, banner ===
    let view_page = build_view_tab(state, game_name, &name_holder);
    page_stack.add_titled(&view_page, Some("view"), "View");

    // === Tab 2: Run (Wine) or Emulator or RPG Maker ===
    if is_rpg {
        let rpg_page = build_rpg_tab(state, game_name, parent);
        page_stack.add_titled(&rpg_page, Some("rpg"), "RPG Maker");
    } else if is_emu {
        let emu_page = build_emulator_tab(state, game_name);
        page_stack.add_titled(&emu_page, Some("emulator"), "Emulator");
    } else {
        let run_page = build_run_tab(state, game_name, parent);
        page_stack.add_titled(&run_page, Some("run"), "Run");
    }

    // === Tab 3: Graphics (Wine games only — C++ parity) ===
    if !is_emu && !is_rpg {
        let gfx_page = build_graphics_tab(state, game_name);
        page_stack.add_titled(&gfx_page, Some("graphics"), "Graphics");
    }

    content.append(&page_stack);

    // Compact tab bar with icons (C++ parity: palette/startup/graphics)
    let tab_bar = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    tab_bar.set_halign(gtk::Align::Fill);
    let tabs: Vec<(&str, &str, &str)> = if is_rpg {
        vec![("View", "view", "palette"), ("RPG Maker", "rpg", "startup")]
    } else if is_emu {
        vec![("View", "view", "palette"), ("Emulator", "emulator", "startup")]
    } else {
        vec![("View", "view", "palette"), ("Run", "run", "startup"), ("Graphics", "graphics", "graphics")]
    };
    let mut tab_buttons: Vec<gtk::ToggleButton> = Vec::new();
    let mut tab_indicators: Vec<gtk::Box> = Vec::new();

    for (i, (label, id, icon_name)) in tabs.iter().enumerate() {
        let btn_box = gtk::Box::new(gtk::Orientation::Vertical, 1);
        btn_box.set_hexpand(true);

        let btn = gtk::ToggleButton::new();
        btn.add_css_class("settings-tab");
        btn.set_halign(gtk::Align::Fill);
        btn.set_hexpand(true);

        let col = gtk::Box::new(gtk::Orientation::Vertical, 2);
        col.set_halign(gtk::Align::Center);
        let icon = helpers::themed_image(icon_name, state.theme.is_dark(), 16);
        let lbl = gtk::Label::new(Some(label));
        lbl.add_css_class("time-label");
        col.append(&icon);
        col.append(&lbl);

        let indicator = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        indicator.add_css_class("settings-tab-indicator");
        indicator.set_visible(i == 0);
        col.append(&indicator);

        btn.set_child(Some(&col));
        if i == 0 { btn.set_active(true); }

        btn_box.append(&btn);
        tab_bar.append(&btn_box);
        tab_buttons.push(btn);
        tab_indicators.push(indicator);
    }
    for btn in &tab_buttons[1..] { btn.set_group(Some(&tab_buttons[0])); }

    for (i, (_, id, _)) in tabs.iter().enumerate() {
        let stack = page_stack.clone();
        let tab_id = id.to_string();
        let all_ind = tab_indicators.clone();
        let my_ind = tab_indicators[i].clone();
        tab_buttons[i].connect_clicked(move |_| {
            stack.set_visible_child_name(&tab_id);
            for ind in &all_ind { ind.set_visible(false); }
            my_ind.set_visible(true);
        });
    }

    let tab_bar_frame = gtk::Frame::new(None);
    tab_bar_frame.add_css_class("settings-tab-bar");
    tab_bar_frame.set_margin_start(8);
    tab_bar_frame.set_margin_end(8);
    tab_bar_frame.set_margin_bottom(8);
    tab_bar_frame.set_child(Some(&tab_bar));
    content.append(&tab_bar_frame);

    // Rename-on-close + refresh hook
    {
        let state_c = state.clone();
        let game = game_name.to_string();
        let holder_c = name_holder.clone();
        dialog.connect_closed(move |_| {
            let new_name = holder_c.borrow().trim().to_string();
            if !new_name.is_empty() && new_name != game {
                if state_c.game_model.get_game(&new_name).is_none() {
                    state_c.game_model.rename_game(&game, &new_name);
                    *state_c.selected_game.borrow_mut() = new_name;
                }
            }
            state_c.recent_model.refresh(30);
            on_close();
        });
    }

    dialog.set_child(Some(&content));
    dialog.present(Some(parent));
}

fn save_entry(state: &AppState, game: &str, key: &str, entry: &gtk::Entry) {
    let held = game.to_string();
    let k = key.to_string();
    let state_c = state.clone();
    entry.connect_changed(move |e| {
        state_c.config.set_game_value(&held, &k, &e.text().to_string());
    });
}

fn save_switch(state: &AppState, game: &str, key: &str, sw: &gtk::Switch) {
    let held = game.to_string();
    let k = key.to_string();
    let state_c = state.clone();
    sw.connect_active_notify(move |s| {
        state_c.config.set_game_value(&held, &k, &s.is_active().to_string());
    });
}

fn make_frame(title: &str) -> (gtk::Frame, gtk::Box) {
    let frame = gtk::Frame::new(None);
    frame.add_css_class("page-card");
    let inner = gtk::Box::new(gtk::Orientation::Vertical, 6);
    inner.set_margin_top(8);
    inner.set_margin_bottom(8);
    inner.set_margin_start(10);
    inner.set_margin_end(10);
    let lbl = gtk::Label::new(Some(title));
    lbl.set_halign(gtk::Align::Start);
    lbl.add_css_class("frame-title");
    inner.append(&lbl);
    frame.set_child(Some(&inner));
    (frame, inner)
}

fn wrap_scroll(page: gtk::Box) -> gtk::Box {
    let outer = gtk::Box::new(gtk::Orientation::Vertical, 0);
    outer.set_hexpand(true);
    outer.set_vexpand(true);
    let scroll = gtk::ScrolledWindow::new();
    scroll.set_vexpand(true);
    scroll.set_hexpand(true);
    scroll.set_hscrollbar_policy(gtk::PolicyType::Never);
    scroll.set_child(Some(&page));
    outer.append(&scroll);
    outer
}

fn build_view_tab(
    state: &AppState,
    game_name: &str,
    name_holder: &std::rc::Rc<std::cell::RefCell<String>>,
) -> gtk::Box {
    let page = gtk::Box::new(gtk::Orientation::Vertical, 12);
    page.set_margin_top(12);
    page.set_margin_bottom(12);
    page.set_margin_start(20);
    page.set_margin_end(20);
    page.set_halign(gtk::Align::Fill);

    // Game name (renamed on dialog close)
    let (name_frame, name_inner) = make_frame("Game Name");
    let name_entry = gtk::Entry::new();
    name_entry.set_text(game_name);
    name_entry.set_hexpand(true);
    {
        let holder = name_holder.clone();
        name_entry.connect_changed(move |e| {
            *holder.borrow_mut() = e.text().to_string();
        });
    }
    name_inner.append(&name_entry);
    page.append(&name_frame);

    // Icon with live preview
    let (icon_frame, icon_inner) = make_frame("Icon");
    let icon_box = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    let icon_preview = gtk::Picture::new();
    icon_preview.set_size_request(32, 32);
    icon_preview.set_content_fit(gtk::ContentFit::Contain);
    icon_preview.set_can_shrink(true);
    let current_icon = state.config.game_value(game_name, "Icon").unwrap_or_default();
    if !current_icon.is_empty() {
        if let Some(tex) = helpers::load_preview(&expand_tilde(&current_icon), 64, 64) {
            icon_preview.set_paintable(Some(&tex));
        }
    }
    let icon_entry = gtk::Entry::new();
    icon_entry.set_text(&current_icon);
    icon_entry.set_hexpand(true);
    icon_entry.set_valign(gtk::Align::Center);
    icon_entry.set_placeholder_text(Some("Path to icon image"));
    icon_entry.add_css_class("path-entry");
    let icon_browse = helpers::icon_button("folder", state.theme.is_dark(), "Choose icon image");
    let state_c = state.clone();
    let icon_e = icon_entry.clone();
    let icon_p = icon_preview.clone();
    icon_browse.connect_clicked(move |_| {
        if let Some(path) = state_c.config.pick_file("Select icon", &["png", "jpg", "svg"]) {
            icon_e.set_text(&path.display().to_string());
        }
    });
    {
        let icon_p2 = icon_preview.clone();
        icon_entry.connect_changed(move |e| {
            let p = e.text().to_string();
            if p.is_empty() {
                icon_p2.set_paintable(None::<&gtk::gdk::Texture>);
            } else if let Some(tex) = helpers::load_preview(&expand_tilde(&p), 64, 64) {
                icon_p2.set_paintable(Some(&tex));
            }
        });
    }
    save_entry(state, game_name, "Icon", &icon_entry);
    icon_box.append(&icon_preview);
    icon_box.append(&icon_entry);
    icon_box.append(&icon_browse);
    icon_inner.append(&icon_box);
    page.append(&icon_frame);

    // Banner with live preview
    let (banner_frame, banner_inner) = make_frame("Banner / Cover");
    let banner_box = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    let banner_preview = gtk::Picture::new();
    banner_preview.set_size_request(64, 36);
    banner_preview.set_content_fit(gtk::ContentFit::Contain);
    banner_preview.set_can_shrink(true);
    banner_preview.set_vexpand(false);
    banner_preview.set_valign(gtk::Align::Center);
    let current_banner = state.config.game_value(game_name, "Banner").unwrap_or_default();
    if !current_banner.is_empty() {
        if let Some(tex) = helpers::load_preview(&expand_tilde(&current_banner), 160, 92) {
            banner_preview.set_paintable(Some(&tex));
        }
    }
    let banner_entry = gtk::Entry::new();
    banner_entry.set_text(&current_banner);
    banner_entry.set_hexpand(true);
    banner_entry.set_valign(gtk::Align::Center);
    banner_entry.set_placeholder_text(Some("Path to banner image"));
    banner_entry.add_css_class("path-entry");
    let banner_browse = helpers::icon_button("folder", state.theme.is_dark(), "Choose banner image");
    let state_c2 = state.clone();
    let banner_e = banner_entry.clone();
    banner_browse.connect_clicked(move |_| {
        if let Some(path) = state_c2.config.pick_file("Select banner", &["png", "jpg", "jpeg"]) {
            banner_e.set_text(&path.display().to_string());
        }
    });
    {
        let banner_p2 = banner_preview.clone();
        banner_entry.connect_changed(move |e| {
            let p = e.text().to_string();
            if p.is_empty() {
                banner_p2.set_paintable(None::<&gtk::gdk::Texture>);
            } else if let Some(tex) = helpers::load_preview(&expand_tilde(&p), 160, 92) {
                banner_p2.set_paintable(Some(&tex));
            }
        });
    }
    save_entry(state, game_name, "Banner", &banner_entry);
    banner_box.append(&banner_preview);
    banner_box.append(&banner_entry);
    banner_box.append(&banner_browse);
    banner_inner.append(&banner_box);
    page.append(&banner_frame);

    wrap_scroll(page)
}

fn build_run_tab(
    state: &AppState,
    game_name: &str,
    parent: &adw::ApplicationWindow,
) -> gtk::Box {
    let page = gtk::Box::new(gtk::Orientation::Vertical, 12);
    page.set_margin_top(12);
    page.set_margin_bottom(12);
    page.set_margin_start(20);
    page.set_margin_end(20);
    page.set_halign(gtk::Align::Fill);

    let cols = gtk::Box::new(gtk::Orientation::Vertical, 12);
    cols.set_halign(gtk::Align::Fill);
    page.append(&cols);

    // ---- Launch options card ----
    let (opts_frame, opts_inner) = make_frame("Launch options");
    let overrides_entry: Rc<RefCell<Option<gtk::Entry>>> = Rc::new(RefCell::new(None));
    for (label, key, placeholder) in [
        ("DLL overrides (WINEDLLOVERRIDES)", "Overrides", "d3d11=n,b"),
        ("Environment variables", "Environment", "KEY=value KEY2=value2"),
        ("Arguments before executable", "ArgsBefore", "Arguments passed before the executable"),
        ("Arguments after executable", "ArgsAfter", "Arguments passed after the executable"),
    ] {
        let lbl = gtk::Label::new(Some(label));
        lbl.set_halign(gtk::Align::Start);
        lbl.add_css_class("time-label");
        opts_inner.append(&lbl);
        let entry = gtk::Entry::new();
        entry.set_text(&state.config.game_value(game_name, key).unwrap_or_default());
        entry.set_hexpand(true);
        entry.set_placeholder_text(Some(placeholder));
        opts_inner.append(&entry);
        if key == "Overrides" {
            *overrides_entry.borrow_mut() = Some(entry.clone());
        }
        save_entry(state, game_name, key, &entry);
    }
    // Steam runtime / umu switches (C++ Run tab parity).
    // umu stays toggleable even when umu-run is missing: the user may
    // install it later and the launcher errors gracefully at launch.
    for (label, key) in [
        ("Steam runtime", "SteamRuntime"),
        ("umu-launcher", "UseUmu"),
    ] {
        let row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        row.set_margin_top(4);
        let lbl = gtk::Label::new(Some(label));
        lbl.set_hexpand(true);
        lbl.set_halign(gtk::Align::Start);
        let sw = gtk::Switch::new();
        sw.set_halign(gtk::Align::End);
        sw.set_active(
            state.config.game_value(game_name, key)
                .map(|v| v == "true" || v == "1")
                .unwrap_or(false),
        );
        row.append(&lbl);
        row.append(&sw);
        opts_inner.append(&row);
        save_switch(state, game_name, key, &sw);
    }
    cols.append(&opts_frame);

    // ---- Dependencies & DLL overrides card ----
    // Manual trigger for the plugin scans (they also run automatically on
    // add/import): fills Overrides live, asks before installing deps.
    {
        let (dep_frame, dep_inner) = make_frame("Dependencies & DLL overrides");
        let dep_status = gtk::Label::new(Some("Not scanned yet."));
        dep_status.set_halign(gtk::Align::Start);
        dep_status.set_wrap(true);
        dep_status.set_opacity(0.6);
        dep_status.add_css_class("time-label");
        dep_inner.append(&dep_status);
        let scan_btn = gtk::Button::with_label("Scan now");
        scan_btn.add_css_class("settings-btn");
        scan_btn.set_halign(gtk::Align::Start);
        scan_btn.set_width_request(140);
        {
            let state_c = state.clone();
            let game_c = game_name.to_string();
            let parent_c = parent.clone();
            let status_c = dep_status.clone();
            let entry_c = overrides_entry.clone();
            scan_btn.connect_clicked(move |b| {
                b.set_sensitive(false);
                status_c.set_text("Scanning…");
                let dir = state_c.config.game_value(&game_c, "MainPath").unwrap_or_default();
                if dir.trim().is_empty() {
                    status_c.set_text("No install path set for this game.");
                    b.set_sensitive(true);
                    return;
                }
                let prefix = state_c.proton.real_prefix_for(&game_c).display().to_string();
                let proton = state_c.config.game_value(&game_c, "Proton")
                    .and_then(|w| state_c.proton.resolve_proton(&w).ok())
                    .map(|(_, p)| p.display().to_string())
                    .unwrap_or_default();
                let plugins_dir = state_c.plugins.plugins_dir();
                let (tx, rx) = std::sync::mpsc::channel::<(String, Vec<(String, String)>)>();
                let dir_t = dir.trim().to_string();
                std::thread::spawn(move || {
                    let ov = crate::backend::plugins::PluginManager::dll_scan_in(&plugins_dir, &dir_t).unwrap_or_default();
                    let missing = crate::backend::plugins::PluginManager::dep_scan_in(&plugins_dir, &dir_t, &prefix, &proton)
                        .map(|(m, _)| m)
                        .unwrap_or_default();
                    let _ = tx.send((ov, missing));
                });
                let state_cc = state_c.clone();
                let parent_cc = parent_c.clone();
                let game_cc = game_c.clone();
                let status_cc = status_c.clone();
                let btn_c = b.clone();
                let entry_cc = entry_c.clone();
                glib::idle_add_local(move || match rx.try_recv() {
                    Ok((overrides, missing)) => {
                        btn_c.set_sensitive(true);
                        if !overrides.trim().is_empty() {
                            state_cc.game_model.set_overrides(&game_cc, overrides.trim());
                            if let Some(ref e) = *entry_cc.borrow() {
                                e.set_text(overrides.trim());
                            }
                            status_cc.set_text("Scan done: DLL overrides applied below.");
                        } else if missing.is_empty() {
                            status_cc.set_text("Scan done: nothing needed.");
                        }
                        if !missing.is_empty() {
                            let names: Vec<String> = missing.iter().map(|(id, _)| id.clone()).collect();
                            let descs: Vec<String> = missing.iter().map(|(id, d)| {
                                if d == id { id.clone() } else { format!("{} ({})", id, d) }
                            }).collect();
                            status_cc.set_text(&format!("Missing: {}.", descs.join(", ")));
                            crate::confirm_install_deps(&state_cc, &parent_cc, &game_cc, names);
                        }
                        glib::ControlFlow::Break
                    }
                    Err(std::sync::mpsc::TryRecvError::Empty) => glib::ControlFlow::Continue,
                    Err(_) => {
                        btn_c.set_sensitive(true);
                        glib::ControlFlow::Break
                    }
                });
            });
        }
        dep_inner.append(&scan_btn);
        cols.append(&dep_frame);
    }

    // ---- Proton and prefix card ----
    let (pp_frame, pp_inner) = make_frame("Proton and prefix");
    let plbl = gtk::Label::new(Some("Proton version"));
    plbl.set_halign(gtk::Align::Start);
    plbl.add_css_class("time-label");
    pp_inner.append(&plbl);
    // C++ parity: first entry is "GE-Proton Latest"
    let protons = state.proton.installed_protons();
    let mut proton_names = vec!["GE-Proton Latest".to_string()];
    proton_names.extend(protons.clone());
    let current_proton = state.config.game_value(game_name, "Proton").unwrap_or_default();
    let proton_dropdown = gtk::DropDown::new(None::<gtk::StringList>, None::<gtk::Expression>);
    let string_list = gtk::StringList::new(&[]);
    for p in &proton_names { string_list.append(p); }
    proton_dropdown.set_model(Some(&string_list.clone().upcast::<gio::ListModel>()));
    proton_dropdown.set_selected(
        proton_names.iter().position(|p| *p == current_proton).unwrap_or(0) as u32,
    );
    pp_inner.append(&proton_dropdown);

    // Shared prefix: switch + read-only resolved path + chooser (no text field)
    let original_prefix = state.config.game_value(game_name, "PrefixPath").unwrap_or_default();
    let shared_active: std::rc::Rc<std::cell::RefCell<bool>> =
        std::rc::Rc::new(std::cell::RefCell::new(false));
    let original_rc: std::rc::Rc<std::cell::RefCell<String>> =
        std::rc::Rc::new(std::cell::RefCell::new(original_prefix.clone()));
    let shared_box = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    shared_box.set_margin_top(6);
    let shared_sw = gtk::Switch::new();
    shared_sw.set_halign(gtk::Align::Start);
    shared_sw.set_valign(gtk::Align::Center);
    let shared_lbl = gtk::Label::new(Some("Shared prefix"));
    shared_lbl.set_halign(gtk::Align::Start);
    shared_box.append(&shared_sw);
    shared_box.append(&shared_lbl);
    pp_inner.append(&shared_box);
    let prefix_lbl = gtk::Label::new(Some("Prefix path"));
    prefix_lbl.set_halign(gtk::Align::Start);
    prefix_lbl.add_css_class("time-label");
    pp_inner.append(&prefix_lbl);
    let prefix_entry = gtk::Entry::new();
    prefix_entry.set_text(&original_prefix);
    prefix_entry.set_hexpand(true);
    pp_inner.append(&prefix_entry);
    save_entry(state, game_name, "PrefixPath", &prefix_entry);
    // Wine virtual desktop: borderless window instead of exclusive
    // fullscreen (mode switches crash some compositors, e.g. Muffin).
    {
        let geom = ProtonManager::primary_display_size();
        let vd_lbl = gtk::Label::new(Some("Display containment"));
        vd_lbl.set_halign(gtk::Align::Start);
        vd_lbl.set_margin_top(6);
        vd_lbl.add_css_class("time-label");
        pp_inner.append(&vd_lbl);
        let vd_check = gtk::CheckButton::with_label(&format!(
            "Wine virtual desktop {} (borderless, no mode switches)", geom));
        vd_check.set_active(!state.config.game_value(game_name, "WineVDesktop").unwrap_or_default().is_empty());
        pp_inner.append(&vd_check);
        pp_inner.append(&{
            let n = gtk::Label::new(Some("If launching kills your desktop, turn this on."));
            n.set_halign(gtk::Align::Start);
            n.set_opacity(0.6);
            n.add_css_class("time-label");
            n
        });
        let state_c = state.clone();
        let game_c = game_name.to_string();
        vd_check.connect_toggled(move |b| {
            let geom = ProtonManager::primary_display_size();
            state_c.config.set_game_value(&game_c, "WineVDesktop",
                if b.is_active() { &geom } else { "" });
        });
    }
    // Epic Online Services auth: EOS games (Fall Guys…) refuse to start
    // without an exchange code. Auto-on for Heroic Epic imports.
    {
        let src = state.config.game_value(game_name, "Source").unwrap_or_default();
        let heroic = state.config.game_value(game_name, "HeroicStore").unwrap_or_default();
        if src == "Epic" || heroic == "epic" {
            let eos_check = gtk::CheckButton::with_label("Epic Online Services auth (exchange code at launch)");
            let cur = state.config.game_value(game_name, "EosAuth");
            eos_check.set_active(match cur.as_deref() {
                Some("false") | Some("0") => false,
                Some(_) => true,
                None => true,
            });
            pp_inner.append(&eos_check);
            let state_c = state.clone();
            let game_c = game_name.to_string();
            eos_check.connect_toggled(move |b| {
                state_c.config.set_game_value(&game_c, "EosAuth", &b.is_active().to_string());
            });
        }
    }
    // Heroic anti-cheat runtimes: only for Epic/GOG games imported from Heroic.
    if !state.config.game_value(game_name, "HeroicStore").unwrap_or_default().is_empty() {
        let ac_lbl = gtk::Label::new(Some("Anti-cheat runtimes (Heroic)"));
        ac_lbl.set_halign(gtk::Align::Start);
        ac_lbl.set_margin_top(6);
        ac_lbl.add_css_class("time-label");
        pp_inner.append(&ac_lbl);
        for (key, label) in [("HeroicEac", "Easy Anti-Cheat runtime"), ("HeroicBattlEye", "BattlEye runtime")] {
            let row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
            let sw = gtk::Switch::new();
            sw.set_halign(gtk::Align::Start);
            sw.set_valign(gtk::Align::Center);
            sw.set_active(state.config.game_value(game_name, key).map(|v| v == "true").unwrap_or(false));
            let lb = gtk::Label::new(Some(label));
            lb.set_halign(gtk::Align::Start);
            row.append(&sw);
            row.append(&lb);
            pp_inner.append(&row);
            let state_c = state.clone();
            let game_c = game_name.to_string();
            let key_c = key.to_string();
            sw.connect_active_notify(move |s| {
                state_c.config.set_game_value(&game_c, &key_c, &s.is_active().to_string());
            });
        }
    }
    cols.append(&pp_frame);

    // Detect already-shared (prefix matches a registered shared prefix).
    // C++ parity: originalPrefixPath is only meaningful when NOT shared;
    // if the stored path is itself shared (or empty), the fallback
    // individual path is prefixesDir/gameName.
    {
        let cur = original_prefix.clone();
        let mut active = state.config.game_value(game_name, "UseSharedPrefix")
            .map(|v| v == "true" || v == "1")
            .unwrap_or(false);
        if !active {
            for sp in state.config.shared_prefixes() {
                if let Some(p) = state.config.shared_prefix_path(&sp) {
                    let ps = p.display().to_string();
                    if cur == ps || cur == format!("{}/pfx", ps) {
                        active = true;
                        break;
                    }
                }
            }
        }
        if active && is_shared_path(state, &cur) {
            let def = state.config.base_path_for("prefixes")
                .join(game_name).display().to_string();
            *original_rc.borrow_mut() = def;
        }
        *shared_active.borrow_mut() = active;
        shared_sw.set_active(active);
        proton_dropdown.set_sensitive(!active);
        prefix_entry.set_editable(!active);
    }

    // Proton selection saves through
    {
        let state_c = state.clone();
        let game_c = game_name.to_string();
        let names_c = proton_names.clone();
        proton_dropdown.connect_selected_notify(move |d| {
            let idx = d.selected() as usize;
            let val = names_c.get(idx).cloned().unwrap_or_default();
            state_c.config.set_game_value(&game_c, "Proton", &val);
        });
    }

    // Shared switch behavior (C++ parity)
    {
        let state_c = state.clone();
        let game_c = game_name.to_string();
        let active_c = shared_active.clone();
        let orig_c = original_rc.clone();
        let prefix_e = prefix_entry.clone();
        let proton_d = proton_dropdown.clone();
        let win_c = parent.clone();
        let names_c = proton_names.clone();
        shared_sw.connect_active_notify(move |sw| {
            let on = sw.is_active();
            *active_c.borrow_mut() = on;
            state_c.config.set_game_value(&game_c, "UseSharedPrefix", &on.to_string());
            if on {
                let names_cc = names_c.clone();
                let names_for_list = names_cc.clone();
                let sw_c = sw.clone();
                open_shared_picker(&state_c, &win_c, &names_for_list, {
                    let state_cc = state_c.clone();
                    let game_cc = game_c.clone();
                    let prefix_cc = prefix_e.clone();
                    let proton_dd = proton_d.clone();
                    let orig_cc = orig_c.clone();
                    move |proton_name: String| {
                        let sp = state_cc.config.add_shared_prefix(&proton_name);
                        let full = format!("{}/pfx", sp.display());
                        // Lock proton box to the shared proton
                        if let Some(idx) = names_cc.iter().position(|p| *p == proton_name) {
                            proton_dd.set_selected(idx as u32);
                        }
                        state_cc.config.set_game_value(&game_cc, "Proton", &proton_name);
                        prefix_cc.set_text(&full);
                        state_cc.config.set_game_value(&game_cc, "PrefixPath", &full);
                        prefix_cc.set_editable(false);
                        proton_dd.set_sensitive(false);
                        let _ = orig_cc;
                    }
                }, move || {
                    // C++ cancelSharedPrefix parity: revert the switch.
                    sw_c.set_active(false);
                });
            } else {
                let mut orig = orig_c.borrow().clone();
                // If the remembered path is itself shared (or empty),
                // rebuild the default individual path.
                if is_shared_path(&state_c, &orig) {
                    orig = state_c.config.base_path_for("prefixes")
                        .join(&game_c).display().to_string();
                    *orig_c.borrow_mut() = orig.clone();
                }
                prefix_e.set_text(&orig);
                state_c.config.set_game_value(&game_c, "PrefixPath", &orig);
                prefix_e.set_editable(true);
                proton_d.set_sensitive(true);
            }
        });
    }

    // C++ parity: refuse a manually typed path that belongs to a
    // registered shared prefix — restore the individual path.
    {
        let state_c = state.clone();
        let game_c = game_name.to_string();
        let active_c = shared_active.clone();
        let orig_c = original_rc.clone();
        let prefix_e = prefix_entry.clone();
        prefix_entry.connect_changed(move |e| {
            if *active_c.borrow() {
                return;
            }
            let text = e.text().to_string().trim().to_string();
            if text.is_empty() || !is_shared_path(&state_c, &text) {
                return;
            }
            let mut orig = orig_c.borrow().clone();
            if is_shared_path(&state_c, &orig) {
                orig = state_c.config.base_path_for("prefixes")
                    .join(&game_c).display().to_string();
                *orig_c.borrow_mut() = orig.clone();
            }
            if text != orig {
                prefix_e.set_text(&orig);
                state_c.config.set_game_value(&game_c, "PrefixPath", &orig);
            }
        });
    }

    wrap_scroll(page)
}

/// True when `path` is empty or matches a registered shared prefix
/// (with or without the /pfx suffix).
fn is_shared_path(state: &AppState, path: &str) -> bool {
    let p = path.trim();
    if p.is_empty() {
        return true;
    }
    for sp in state.config.shared_prefixes() {
        if let Some(dir) = state.config.shared_prefix_path(&sp) {
            let ps = dir.display().to_string();
            if p == ps || p == format!("{}/pfx", ps) {
                return true;
            }
        }
    }
    false
}

/// Shared-prefix picker dialog: existing shared prefixes plus installed
/// protons without one yet ("(new)"). C++ sharedPrefixPickerPopup parity.
fn open_shared_picker(
    state: &AppState,
    parent: &adw::ApplicationWindow,
    proton_names: &[String],
    on_pick: impl Fn(String) + 'static,
    on_cancel: impl Fn() + 'static,
) {
    let dialog = adw::Dialog::new();
    dialog.set_title("Shared Prefix");
    dialog.set_content_width(400);
    dialog.set_content_height(380);
    // Full-bleed: outer carries .modal-bg to the dialog edges (no
    // gray Adwaita frame), inner holds the padded content.
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

    let (picker_header, picker_x) = helpers::modal_header("Shared Prefix");
    {
        let dlg_c = dialog.clone();
        picker_x.connect_clicked(move |_| { dlg_c.close(); });
    }
    inner.append(&picker_header);
    let sub = gtk::Label::new(Some("Select an existing shared prefix or create a new one."));
    sub.set_halign(gtk::Align::Start);
    sub.set_wrap(true);
    sub.set_opacity(0.6);
    sub.add_css_class("time-label");
    inner.append(&sub);

    let list = gtk::Box::new(gtk::Orientation::Vertical, 6);
    let existing = state.config.shared_prefixes();
    let on_pick = std::rc::Rc::new(on_pick);
    let on_cancel = std::rc::Rc::new(on_cancel);
    let picked = std::rc::Rc::new(std::cell::Cell::new(false));
    // Skip the pseudo "GE-Proton Latest" entry for real dir mapping
    for name in proton_names.iter().filter(|n| n.as_str() != "GE-Proton Latest") {
        let already = existing.contains(name);
        let path = if already {
            state.config.shared_prefix_path(name)
                .map(|p| p.display().to_string())
                .unwrap_or_default()
        } else {
            state.config.prefixes_dir()
                .join(format!("shared-{}", name.replace('/', "_")))
                .display()
                .to_string()
        };
        let row_btn = gtk::Button::new();
        row_btn.add_css_class("filter-btn");
        row_btn.set_halign(gtk::Align::Fill);
        let col = gtk::Box::new(gtk::Orientation::Vertical, 2);
        let path_lbl = gtk::Label::new(Some(&path));
        path_lbl.set_halign(gtk::Align::Start);
        path_lbl.set_ellipsize(gtk::pango::EllipsizeMode::Middle);
        let sub_lbl = gtk::Label::new(Some(&if already {
            name.clone()
        } else {
            format!("{} (new)", name)
        }));
        sub_lbl.set_halign(gtk::Align::Start);
        sub_lbl.set_opacity(0.6);
        sub_lbl.add_css_class("time-label");
        col.append(&path_lbl);
        col.append(&sub_lbl);
        row_btn.set_child(Some(&col));
        let name_c = name.clone();
        let dlg_c = dialog.clone();
        let pick_c = on_pick.clone();
        let picked_c = picked.clone();
        row_btn.connect_clicked(move |_| {
            picked_c.set(true);
            dlg_c.close();
            pick_c(name_c.clone());
        });
        list.append(&row_btn);
    }
    let scroll = gtk::ScrolledWindow::new();
    scroll.set_vexpand(true);
    scroll.set_child(Some(&list));
    inner.append(&scroll);

    let cancel = gtk::Button::with_label("Cancel");
    cancel.add_css_class("settings-btn");
    cancel.set_halign(gtk::Align::Fill);
    let dlg_c = dialog.clone();
    let cancel_c = on_cancel.clone();
    let picked_c = picked.clone();
    cancel.connect_clicked(move |_| {
        picked_c.set(true);
        dlg_c.close();
        cancel_c();
    });
    inner.append(&cancel);

    // Dismiss via X/Escape counts as cancel (C++ cancelSharedPrefix).
    {
        let cancel_c = on_cancel.clone();
        let picked_c = picked.clone();
        dialog.connect_closed(move |_| {
            if !picked_c.get() {
                picked_c.set(true);
                cancel_c();
            }
        });
    }

    dialog.set_child(Some(&content));
    dialog.present(Some(parent));
}


/// Human labels for emulator setting ids (C++ emulatorSettingLabel parity).
fn emu_setting_label(id: &str) -> String {
    match id {
        "fullscreen" => "Fullscreen".into(),
        "show_osd" => "Show OSD overlay".into(),
        "jit_enable" => "Enable JIT (higher performance)".into(),
        "batch" => "Batch mode (run without opening the interface)".into(),
        "user_dir" => "Save and configuration folder".into(),
        "hide_osd" => "Hide OSD overlay".into(),
        "exit_on_pause" => "Exit when opening the pause menu".into(),
        "no_gui" => "No GUI (run the game directly)".into(),
        "full_boot" => "Full boot (do not skip the PS3 animation)".into(),
        "keys_path" => "Encryption keys path (prod.keys)".into(),
        "title_keys_path" => "Title keys path (title.keys)".into(),
        "config_path" => "Custom configuration path".into(),
        "load_config" => "Load configuration file".into(),
        "frameskip" => "Frame skip (higher speed, less smooth)".into(),
        "mlc_path" => "Custom mlc folder path".into(),
        other => other.replace('_', " "),
    }
}

fn build_emulator_tab(state: &AppState, game_name: &str) -> gtk::Box {
    let page = gtk::Box::new(gtk::Orientation::Vertical, 12);
    page.set_margin_top(12);
    page.set_margin_bottom(12);
    page.set_margin_start(20);
    page.set_margin_end(20);
    page.set_halign(gtk::Align::Fill);

    let executor = state.config.game_value(game_name, "Executor").unwrap_or_default();
    let emu_name = state.plugins.get_emulator_name(&executor);
    // Dynamic setting defs reported by emulator-manager (C++ parity)
    let defs: Vec<crate::backend::plugins::EmuSettingDef> = state
        .plugins
        .list_emulators()
        .into_iter()
        .find(|e| e.name == executor)
        .map(|e| e.settings)
        .unwrap_or_default();

    let (emu_frame, emu_inner) = make_frame(&format!("{} Settings", emu_name));

    if defs.is_empty() {
        let hint = gtk::Label::new(Some("This emulator reports no configurable settings."));
        hint.set_opacity(0.6);
        hint.set_wrap(true);
        hint.add_css_class("time-label");
        emu_inner.append(&hint);
    }
    for def in &defs {
        if def.stype == "path" {
            let lbl = gtk::Label::new(Some(&emu_setting_label(&def.id)));
            lbl.set_halign(gtk::Align::Start);
            lbl.add_css_class("time-label");
            emu_inner.append(&lbl);
            let row = gtk::Box::new(gtk::Orientation::Horizontal, 6);
            let entry = gtk::Entry::new();
            entry.set_text(
                &state.config.game_value(game_name, &format!("emu_{}", def.id)).unwrap_or_default(),
            );
            entry.set_hexpand(true);
            let key = format!("emu_{}", def.id);
            save_entry(state, game_name, &key, &entry);
            row.append(&entry);
            let browse = helpers::icon_button("folder", state.theme.is_dark(), "Choose folder");
            let state_c = state.clone();
            let entry_c = entry.clone();
            browse.connect_clicked(move |_| {
                if let Some(p) = state_c.config.pick_folder("Choose emulator path") {
                    entry_c.set_text(&p.display().to_string());
                }
            });
            row.append(&browse);
            let clear = gtk::Button::with_label("X");
            clear.set_width_request(36);
            let entry_c2 = entry.clone();
            clear.connect_clicked(move |_| {
                entry_c2.set_text("");
            });
            row.append(&clear);
            emu_inner.append(&row);
        } else {
            // bool (default type)
            let row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
            row.set_margin_top(4);
            let lbl = gtk::Label::new(Some(&emu_setting_label(&def.id)));
            lbl.set_halign(gtk::Align::Start);
            lbl.set_hexpand(true);
            lbl.set_wrap(true);
            let sw = gtk::Switch::new();
            sw.set_halign(gtk::Align::End);
            sw.set_valign(gtk::Align::Center);
            let cur = state.config.game_value(game_name, &format!("emu_{}", def.id));
            sw.set_active(match cur.as_deref() {
                Some("true") | Some("1") => true,
                Some("false") | Some("0") => false,
                _ => def.default_bool,
            });
            row.append(&lbl);
            row.append(&sw);
            emu_inner.append(&row);
            save_switch(state, game_name, &format!("emu_{}", def.id), &sw);
        }
    }

    page.append(&emu_frame);
    wrap_scroll(page)
}

fn build_rpg_tab(
    state: &AppState,
    game_name: &str,
    parent: &adw::ApplicationWindow,
) -> gtk::Box {
    let page = gtk::Box::new(gtk::Orientation::Vertical, 12);
    page.set_margin_top(12);
    page.set_margin_bottom(12);
    page.set_margin_start(20);
    page.set_margin_end(20);
    page.set_halign(gtk::Align::Fill);

    let exe = state.config.game_value(game_name, "Executable").unwrap_or_default();

    let (eng_frame, eng_inner) = make_frame("Engine");
    let eng_lbl = gtk::Label::new(Some("Detecting…"));
    eng_lbl.set_halign(gtk::Align::Start);
    eng_lbl.add_css_class("details-title");
    eng_inner.append(&eng_lbl);
    let dir_lbl = gtk::Label::new(Some(&exe));
    dir_lbl.set_halign(gtk::Align::Start);
    dir_lbl.set_ellipsize(gtk::pango::EllipsizeMode::Middle);
    dir_lbl.set_opacity(0.6);
    dir_lbl.add_css_class("time-label");
    eng_inner.append(&dir_lbl);
    page.append(&eng_frame);
    {
        let (tx, rx) = std::sync::mpsc::channel::<String>();
        let exe_c = exe.clone();
        std::thread::spawn(move || {
            let txt = match crate::backend::external::RpgMakerManager::scan(&exe_c) {
                Ok(doc) => {
                    let eng = doc.get("engine").and_then(|v| v.as_str()).unwrap_or("unknown");
                    let var = doc.get("variant").and_then(|v| v.as_str()).unwrap_or("");
                    let auth = doc.get("authoritative").and_then(|v| v.as_bool()).unwrap_or(false);
                    let base = match (eng, var) {
                        ("mv_mz", "mz") => "RPG Maker MZ",
                        ("mv_mz", _) => "RPG Maker MV",
                        ("2k3", _) => "RPG Maker 2000/2003",
                        ("unsupported", _) => "Unsupported (XP/VX/VX Ace)",
                        _ => "Unknown engine",
                    };
                    let why = doc.get("reason").and_then(|v| v.as_str()).unwrap_or("");
                    if why.is_empty() {
                        format!("{} {}", base, if auth { "(verified)" } else { "(hint)" })
                    } else {
                        format!("{} — {}", base, why)
                    }
                }
                Err(e) => format!("Detect failed: {}", e),
            };
            let _ = tx.send(txt);
        });
        let eng_c = eng_lbl.clone();
        let state_c = state.clone();
        let game_c = game_name.to_string();
        glib::idle_add_local(move || match rx.try_recv() {
            Ok(txt) => {
                eng_c.set_text(&txt);
                state_c.config.set_game_value(&game_c, "RpgEngine", &txt);
                glib::ControlFlow::Break
            }
            Err(std::sync::mpsc::TryRecvError::Empty) => glib::ControlFlow::Continue,
            Err(_) => glib::ControlFlow::Break,
        });
    }

    let (box_frame, box_inner) = make_frame("box-rpg");
    let box_lbl = gtk::Label::new(Some("Checking…"));
    box_lbl.set_halign(gtk::Align::Start);
    box_lbl.set_wrap(true);
    box_lbl.set_opacity(0.6);
    box_lbl.add_css_class("time-label");
    box_inner.append(&box_lbl);
    let box_btn = gtk::Button::with_label("Install box-rpg");
    box_btn.add_css_class("add-btn");
    box_btn.set_hexpand(true);
    {
        let parent_c = parent.clone();
        let box_c = box_lbl.clone();
        box_btn.connect_clicked(move |b| {
            b.set_sensitive(false);
            box_c.set_text("Fetching bundled box-rpg (git, quick)…");
            let (tx, rx) = std::sync::mpsc::channel::<Result<String, String>>();
            std::thread::spawn(move || {
                let r = crate::backend::external::RpgMakerManager::install_box();
                let _ = tx.send(r.map(|v| {
                    v.get("commit").and_then(|x| x.as_str()).unwrap_or("").to_string()
                }));
            });
            let parent_cc = parent_c.clone();
            let box_cc = box_c.clone();
            let b_c = b.clone();
            glib::idle_add_local(move || match rx.try_recv() {
                Ok(Ok(c)) => {
                    b_c.set_sensitive(true);
                    box_cc.set_text(&format!("Bundled copy ready ({})", c));
                    helpers::present_msg(&parent_cc, "box-rpg ready", &c);
                    glib::ControlFlow::Break
                }
                Ok(Err(e)) => {
                    b_c.set_sensitive(true);
                    box_cc.set_text("Install failed (see message).");
                    helpers::present_msg(&parent_cc, "Install failed", &e);
                    glib::ControlFlow::Break
                }
                Err(std::sync::mpsc::TryRecvError::Empty) => glib::ControlFlow::Continue,
                Err(_) => {
                    b_c.set_sensitive(true);
                    glib::ControlFlow::Break
                }
            });
        });
    }
    box_inner.append(&box_btn);
    page.append(&box_frame);
    {
        let (tx, rx) = std::sync::mpsc::channel::<String>();
        std::thread::spawn(move || {
            let txt = match crate::backend::external::RpgMakerManager::status() {
                Ok(doc) => {
                    if doc.get("box_rpg_installed").and_then(|v| v.as_bool()).unwrap_or(false) {
                        let nw = doc.get("nwjs_runtimes").and_then(|v| v.as_array())
                            .map(|a| a.len()).unwrap_or(0);
                        let er = doc.get("easyrpg_runtimes").and_then(|v| v.as_array())
                            .map(|a| a.len()).unwrap_or(0);
                        let mode = doc.get("box_rpg_mode").and_then(|v| v.as_str()).unwrap_or("?");
                        let commit = doc.get("box_rpg_commit").and_then(|v| v.as_str()).unwrap_or_default();
                        if commit.is_empty() {
                            format!("Ready ({}, NW.js: {}, EasyRPG: {})", mode, nw, er)
                        } else {
                            format!("Ready ({} {}, NW.js: {}, EasyRPG: {})", mode, commit, nw, er)
                        }
                    } else {
                        String::from("Missing — press Install box-rpg to fetch the bundled copy.")
                    }
                }
                Err(e) => format!("Status failed: {}", e),
            };
            let _ = tx.send(txt);
        });
        let box_c = box_lbl.clone();
        glib::idle_add_local(move || match rx.try_recv() {
            Ok(txt) => {
                box_c.set_text(&txt);
                glib::ControlFlow::Break
            }
            Err(std::sync::mpsc::TryRecvError::Empty) => glib::ControlFlow::Continue,
            Err(_) => glib::ControlFlow::Break,
        });
    }

    let (rt_frame, rt_inner) = make_frame("Runtime");
    let rt_entry = gtk::Entry::new();
    rt_entry.set_text(&state.config.game_value(game_name, "RpgRuntime").unwrap_or_default());
    rt_entry.set_hexpand(true);
    rt_entry.set_placeholder_text(Some("Pinned runtime version (empty = auto)"));
    save_entry(state, game_name, "RpgRuntime", &rt_entry);
    rt_inner.append(&rt_entry);
    let rt_row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    for kind in ["nwjs", "easyrpg"] {
        let btn = gtk::Button::with_label(&format!("Install latest {}", if kind == "nwjs" { "NW.js" } else { "EasyRPG" }));
        btn.add_css_class("settings-btn");
        btn.set_hexpand(true);
        let parent_c = parent.clone();
        let kind_c = kind.to_string();
        btn.connect_clicked(move |b| {
            b.set_sensitive(false);
            let (tx, rx) = std::sync::mpsc::channel::<Result<String, String>>();
            let kind_t = kind_c.clone();
            std::thread::spawn(move || {
                let r = crate::backend::external::RpgMakerManager::install_runtime(&kind_t, None);
                let _ = tx.send(r.map(|v| {
                    v.get("installed_version").and_then(|x| x.as_str()).unwrap_or("").to_string()
                }));
            });
            let parent_cc = parent_c.clone();
            let kind_cc = kind_c.clone();
            let b_c = b.clone();
            glib::idle_add_local(move || match rx.try_recv() {
                Ok(Ok(v)) => {
                    b_c.set_sensitive(true);
                    helpers::present_msg(&parent_cc, "Runtime installed", &format!("{} {}", kind_cc, v));
                    glib::ControlFlow::Break
                }
                Ok(Err(e)) => {
                    b_c.set_sensitive(true);
                    helpers::present_msg(&parent_cc, "Install failed", &e);
                    glib::ControlFlow::Break
                }
                Err(std::sync::mpsc::TryRecvError::Empty) => glib::ControlFlow::Continue,
                Err(_) => {
                    b_c.set_sensitive(true);
                    glib::ControlFlow::Break
                }
            });
        });
        rt_row.append(&btn);
    }
    rt_inner.append(&rt_row);
    page.append(&rt_frame);

    let (dg_frame, dg_inner) = make_frame("Diagnose");
    let dg_scroll = gtk::ScrolledWindow::new();
    dg_scroll.set_min_content_height(120);
    dg_scroll.set_hscrollbar_policy(gtk::PolicyType::Never);
    let dg_lbl = gtk::Label::new(Some("Run diagnose to see the box-rpg report."));
    dg_lbl.set_halign(gtk::Align::Start);
    dg_lbl.set_valign(gtk::Align::Start);
    dg_lbl.set_wrap(true);
    dg_lbl.set_selectable(true);
    dg_lbl.add_css_class("time-label");
    dg_scroll.set_child(Some(&dg_lbl));
    dg_inner.append(&dg_scroll);
    let dg_btn = gtk::Button::with_label("Run diagnose");
    dg_btn.add_css_class("settings-btn");
    dg_btn.set_hexpand(true);
    {
        let exe_c = exe.clone();
        let dg_c = dg_lbl.clone();
        dg_btn.connect_clicked(move |b| {
            b.set_sensitive(false);
            dg_c.set_text("Running diagnose…");
            let (tx, rx) = std::sync::mpsc::channel::<Result<String, String>>();
            let exe_t = exe_c.clone();
            std::thread::spawn(move || {
                let r = crate::backend::external::RpgMakerManager::diagnose(&exe_t, None);
                let _ = tx.send(r.map(|v| {
                    v.get("report").and_then(|x| x.as_str()).unwrap_or("").to_string()
                }));
            });
            let dg_cc = dg_c.clone();
            let b_c = b.clone();
            glib::idle_add_local(move || match rx.try_recv() {
                Ok(Ok(rep)) => {
                    b_c.set_sensitive(true);
                    dg_cc.set_text(if rep.is_empty() { "(empty report)" } else { &rep });
                    glib::ControlFlow::Break
                }
                Ok(Err(e)) => {
                    b_c.set_sensitive(true);
                    dg_cc.set_text(&format!("Diagnose failed: {}", e));
                    glib::ControlFlow::Break
                }
                Err(std::sync::mpsc::TryRecvError::Empty) => glib::ControlFlow::Continue,
                Err(_) => {
                    b_c.set_sensitive(true);
                    glib::ControlFlow::Break
                }
            });
        });
    }
    dg_inner.append(&dg_btn);
    page.append(&dg_frame);

    wrap_scroll(page)
}

fn build_graphics_tab(state: &AppState, game_name: &str) -> gtk::Box {
    let page = gtk::Box::new(gtk::Orientation::Vertical, 12);
    page.set_margin_top(12);
    page.set_margin_bottom(12);
    page.set_margin_start(20);
    page.set_margin_end(20);
    page.set_halign(gtk::Align::Fill);

    // Per-arch availability (C++ graphicsComponentStatus parity)
    let exe = state.config.game_value(game_name, "Executable").unwrap_or_default();
    let gm_status = state.proton.component_status("gamemode", &exe);
    let mh_status = state.proton.component_status("mangohud", &exe);
    let gamemode_avail = gm_status.available;
    let mangohud_avail = mh_status.available;

    // Rendering
    let (w3d_frame, w3d_inner) = make_frame("Rendering");
    let w3d_box = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    let w3d_sw = gtk::Switch::new();
    w3d_sw.set_halign(gtk::Align::End);
    w3d_sw.set_active(state.config.game_value(game_name, "UseWined3d")
        .map(|v| v == "true").unwrap_or(false));
    let w3d_lbl = gtk::Label::new(Some("Use wined3d instead of DXVK"));
    w3d_lbl.set_hexpand(true);
    w3d_box.append(&w3d_lbl);
    w3d_box.append(&w3d_sw);
    w3d_inner.append(&w3d_box);
    save_switch(state, game_name, "UseWined3d", &w3d_sw);
    page.append(&w3d_frame);

    // Display
    let (wl_frame, wl_inner) = make_frame("Display");
    let wl_box = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    let wl_sw = gtk::Switch::new();
    wl_sw.set_halign(gtk::Align::End);
    wl_sw.set_active(state.config.game_value(game_name, "NativeWayland")
        .map(|v| v == "true").unwrap_or(false));
    let wl_lbl = gtk::Label::new(Some("Native Wayland (disable XWayland)"));
    wl_lbl.set_hexpand(true);
    wl_box.append(&wl_lbl);
    wl_box.append(&wl_sw);
    wl_inner.append(&wl_box);
    save_switch(state, game_name, "NativeWayland", &wl_sw);
    page.append(&wl_frame);

    // Performance
    let (gm_frame, gm_inner) = make_frame("Performance");
    let gm_box = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    let gm_sw = gtk::Switch::new();
    gm_sw.set_halign(gtk::Align::End);
    gm_sw.set_active(state.config.game_value(game_name, "GameMode")
        .map(|v| v == "true").unwrap_or(false));
    gm_sw.set_sensitive(gamemode_avail);
    let gm_lbl = gtk::Label::new(Some(if gamemode_avail { "GameMode" } else { "GameMode (not installed)" }));
    gm_lbl.set_hexpand(true);
    gm_box.append(&gm_lbl);
    gm_box.append(&gm_sw);
    gm_inner.append(&gm_box);
    let gm_sub = gtk::Label::new(Some(
        &crate::backend::proton::ProtonManager::component_status_text(&gm_status, "GameMode"),
    ));
    gm_sub.set_halign(gtk::Align::Start);
    gm_sub.set_opacity(0.6);
    gm_sub.add_css_class("time-label");
    gm_inner.append(&gm_sub);
    save_switch(state, game_name, "GameMode", &gm_sw);

    let mh_box = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    let mh_sw = gtk::Switch::new();
    mh_sw.set_halign(gtk::Align::End);
    mh_sw.set_active(state.config.game_value(game_name, "MangoHud")
        .map(|v| v == "true").unwrap_or(false));
    mh_sw.set_sensitive(mangohud_avail);
    let mh_lbl = gtk::Label::new(Some(if mangohud_avail { "MangoHud" } else { "MangoHud (not installed)" }));
    mh_lbl.set_hexpand(true);
    mh_box.append(&mh_lbl);
    mh_box.append(&mh_sw);
    gm_inner.append(&mh_box);
    let mh_sub = gtk::Label::new(Some(
        &crate::backend::proton::ProtonManager::component_status_text(&mh_status, "MangoHud"),
    ));
    mh_sub.set_halign(gtk::Align::Start);
    mh_sub.set_opacity(0.6);
    mh_sub.add_css_class("time-label");
    gm_inner.append(&mh_sub);
    save_switch(state, game_name, "MangoHud", &mh_sw);

    page.append(&gm_frame);

    wrap_scroll(page)
}

fn expand_tilde(s: &str) -> String {
    if s.starts_with("~/") || s == "~" {
        if let Ok(home) = std::env::var("HOME") {
            return s.replacen("~", &home, 1);
        }
    }
    s.to_string()
}
