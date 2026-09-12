use adw::prelude::*;
use gtk::prelude::*;
use std::rc::Rc;

use crate::AppState;
use crate::ui::helpers;

fn card(title: &str) -> (gtk::Box, gtk::Box) {
    let frame = gtk::Box::new(gtk::Orientation::Vertical, 6);
    frame.add_css_class("page-card");
    frame.set_margin_top(8);
    frame.set_margin_bottom(8);
    frame.set_margin_start(10);
    frame.set_margin_end(10);
    let lbl = gtk::Label::new(Some(title));
    lbl.set_halign(gtk::Align::Start);
    lbl.add_css_class("frame-title");
    frame.append(&lbl);
    let inner = gtk::Box::new(gtk::Orientation::Vertical, 6);
    frame.append(&inner);
    (frame, inner)
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

fn switch_row(title: &str, sub: &str, active: bool) -> (gtk::Box, gtk::Switch) {
    let row = gtk::Box::new(gtk::Orientation::Horizontal, 10);
    row.set_hexpand(true);
    let vbox = gtk::Box::new(gtk::Orientation::Vertical, 2);
    vbox.set_hexpand(true);
    let lbl = gtk::Label::new(Some(title));
    lbl.set_halign(gtk::Align::Start);
    let s = gtk::Label::new(Some(sub));
    s.set_halign(gtk::Align::Start);
    s.set_opacity(0.6);
    s.add_css_class("time-label");
    vbox.append(&lbl);
    vbox.append(&s);
    let sw = gtk::Switch::new();
    sw.set_active(active);
    sw.set_valign(gtk::Align::Center);
    row.append(&vbox);
    row.append(&sw);
    (row, sw)
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

fn expand_tilde(s: &str) -> String {
    if s.starts_with("~/") || s == "~" {
        if let Ok(home) = std::env::var("HOME") {
            return s.replacen("~", &home, 1);
        }
    }
    s.to_string()
}

fn build_view_tab(state: &AppState, game_name: &str, name_holder: &Rc<std::cell::RefCell<String>>) -> gtk::Box {
    let page = gtk::Box::new(gtk::Orientation::Vertical, 12);
    page.set_margin_top(12);
    page.set_margin_bottom(12);
    page.set_margin_start(20);
    page.set_margin_end(20);
    let (name_frame, name_inner) = card("App Name");
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
    let (icon_frame, icon_inner) = card("Icon");
    let icon_box = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    let icon_entry = gtk::Entry::new();
    icon_entry.set_text(&state.config.game_value(game_name, "Icon").unwrap_or_default());
    icon_entry.set_hexpand(true);
    icon_entry.set_placeholder_text(Some("Path to icon image"));
    let browse = helpers::icon_button("folder", state.theme.is_dark(), "Choose icon image");
    {
        let state_c = state.clone();
        let e = icon_entry.clone();
        browse.connect_clicked(move |_| {
            if let Some(p) = state_c.config.pick_file("Select icon", &["png", "jpg", "svg"]) {
                e.set_text(&p.display().to_string());
            }
        });
    }
    save_entry(state, game_name, "Icon", &icon_entry);
    icon_box.append(&icon_entry);
    icon_box.append(&browse);
    icon_inner.append(&icon_box);
    page.append(&icon_frame);
    let (banner_frame, banner_inner) = card("Banner / Cover");
    let banner_entry = gtk::Entry::new();
    banner_entry.set_text(&state.config.game_value(game_name, "Banner").unwrap_or_default());
    banner_entry.set_hexpand(true);
    banner_entry.set_placeholder_text(Some("Path to banner image"));
    save_entry(state, game_name, "Banner", &banner_entry);
    banner_inner.append(&banner_entry);
    page.append(&banner_frame);
    wrap_scroll(page)
}

fn build_app_tab(state: &AppState, game_name: &str, parent: &adw::ApplicationWindow) -> gtk::Box {
    let page = gtk::Box::new(gtk::Orientation::Vertical, 12);
    page.set_margin_top(12);
    page.set_margin_bottom(12);
    page.set_margin_start(20);
    page.set_margin_end(20);
    let (file_frame, file_inner) = card("AppImage File");
    let path_row = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    let path_entry = gtk::Entry::new();
    path_entry.set_text(&state.config.game_value(game_name, "Executable").unwrap_or_default());
    path_entry.set_hexpand(true);
    path_entry.set_editable(false);
    path_entry.add_css_class("path-entry");
    let change_btn = gtk::Button::with_label("Change…");
    change_btn.add_css_class("settings-btn");
    {
        let state_c = state.clone();
        let e = path_entry.clone();
        let game_c = game_name.to_string();
        change_btn.connect_clicked(move |_| {
            if let Some(p) = state_c.config.pick_file("Select AppImage", &["AppImage", "appimage"]) {
                let s = p.display().to_string();
                e.set_text(&s);
                state_c.config.set_game_value(&game_c, "Executable", &s);
                if let Some(parent) = p.parent() {
                    state_c.config.set_game_value(&game_c, "MainPath", &parent.display().to_string());
                }
            }
        });
    }
    path_row.append(&path_entry);
    path_row.append(&change_btn);
    file_inner.append(&path_row);
    let exe_lbl = gtk::Label::new(None);
    exe_lbl.set_halign(gtk::Align::Start);
    exe_lbl.add_css_class("time-label");
    exe_lbl.set_opacity(0.6);
    let update_exe = {
        let e = path_entry.clone();
        let l = exe_lbl.clone();
        Rc::new(move || {
            let p = expand_tilde(&e.text().to_string());
            let path = std::path::Path::new(&p);
            if !path.is_file() {
                l.set_text("File not found");
                return;
            }
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                if let Ok(m) = std::fs::metadata(path) {
                    if m.permissions().mode() & 0o111 == 0 {
                        l.set_text("Not executable — use Fix below");
                    } else {
                        l.set_text("Executable, ready to run");
                    }
                }
            }
        })
    };
    {
        let u = update_exe.clone();
        path_entry.connect_changed(move |_| u());
    }
    update_exe();
    file_inner.append(&exe_lbl);
    let fix_btn = gtk::Button::with_label("Fix: chmod +x");
    fix_btn.add_css_class("settings-btn");
    {
        let e = path_entry.clone();
        let u = update_exe.clone();
        fix_btn.connect_clicked(move |_| {
            let p = expand_tilde(&e.text().to_string());
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                if let Ok(m) = std::fs::metadata(&p) {
                    let mut perm = m.permissions();
                    perm.set_mode(perm.mode() | 0o755);
                    std::fs::set_permissions(&p, perm).ok();
                }
            }
            u();
        });
    }
    file_inner.append(&fix_btn);
    page.append(&file_frame);
    let (args_frame, args_inner) = card("Arguments & Environment");
    let args_entry = gtk::Entry::new();
    args_entry.set_text(&state.config.game_value(game_name, "LaunchArgs").unwrap_or_default());
    args_entry.set_hexpand(true);
    args_entry.set_placeholder_text(Some("Extra args, e.g. --no-sandbox"));
    save_entry(state, game_name, "LaunchArgs", &args_entry);
    args_inner.append(&args_entry);
    let env_entry = gtk::Entry::new();
    env_entry.set_text(&state.config.game_value(game_name, "Environment").unwrap_or_default());
    env_entry.set_hexpand(true);
    env_entry.set_placeholder_text(Some("KEY=VAL KEY2=VAL2"));
    save_entry(state, game_name, "Environment", &env_entry);
    args_inner.append(&env_entry);
    page.append(&args_frame);
    let (icon_frame, icon_inner) = card("Icon");
    let icon_note = gtk::Label::new(Some("Embedded .DirIcon only — no online lookup."));
    icon_note.set_halign(gtk::Align::Start);
    icon_note.set_opacity(0.6);
    icon_note.add_css_class("time-label");
    icon_inner.append(&icon_note);
    let extract_btn = gtk::Button::with_label("Extract embedded icon");
    extract_btn.add_css_class("add-btn");
    extract_btn.set_hexpand(true);
    {
        let state_c = state.clone();
        let e = path_entry.clone();
        let parent_c = parent.clone();
        let game_c = game_name.to_string();
        extract_btn.connect_clicked(move |btn| {
            btn.set_sensitive(false);
            let exe = e.text().to_string();
            let (tx, rx) = std::sync::mpsc::channel::<Option<String>>();
            let game_t = game_c.clone();
            std::thread::spawn(move || {
                let integ = crate::backend::integration::IntegrationManager::new();
                let got = integ.extract_appimage_icon(&exe, &game_t);
                let _ = tx.send(got);
            });
            let state_cc = state_c.clone();
            let parent_cc = parent_c.clone();
            let game_cc = game_c.clone();
            let btn_c = btn.clone();
            glib::idle_add_local(move || match rx.try_recv() {
                Ok(got) => {
                    btn_c.set_sensitive(true);
                    match got {
                        Some(p) => {
                            state_cc.config.set_game_value(&game_cc, "Icon", &p);
                            helpers::present_msg(&parent_cc, "Icon extracted", &p);
                        }
                        None => helpers::present_msg(
                            &parent_cc,
                            "No icon",
                            "Could not extract .DirIcon from this AppImage.",
                        ),
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
    icon_inner.append(&extract_btn);
    page.append(&icon_frame);
    wrap_scroll(page)
}

fn build_system_tab(state: &AppState, game_name: &str) -> gtk::Box {
    let page = gtk::Box::new(gtk::Orientation::Vertical, 12);
    page.set_margin_top(12);
    page.set_margin_bottom(12);
    page.set_margin_start(20);
    page.set_margin_end(20);
    let (sys_frame, sys_inner) = card("System");
    let get_flag = |k: &str| state.config.game_value(game_name, k).map(|v| v == "true").unwrap_or(false);
    let (gm_row, gm_sw) = switch_row("GameMode", "gamemoderun wrapper", get_flag("GameMode"));
    save_switch(state, game_name, "GameMode", &gm_sw);
    sys_inner.append(&gm_row);
    let (mh_row, mh_sw) = switch_row("MangoHud", "MANGOHUD=1 overlay", get_flag("MangoHud"));
    save_switch(state, game_name, "MangoHud", &mh_sw);
    sys_inner.append(&mh_row);
    let note = gtk::Label::new(Some("AppImages run natively: no Proton, no prefix, no Wine tools."));
    note.set_halign(gtk::Align::Start);
    note.set_wrap(true);
    note.set_opacity(0.6);
    note.add_css_class("time-label");
    sys_inner.append(&note);
    page.append(&sys_frame);
    wrap_scroll(page)
}

pub fn show_apps_settings_modal(
    state: &AppState,
    parent: &adw::ApplicationWindow,
    game_name: &str,
    on_close: impl Fn() + 'static,
) {
    let dialog = adw::Dialog::new();
    dialog.set_title(&format!("{} — Apps Settings", game_name));
    dialog.set_content_width(580);
    dialog.set_content_height(600);
    let (header_row, x_btn) = helpers::modal_header(&format!("{} — Apps Settings", game_name));
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
    let name_holder: Rc<std::cell::RefCell<String>> = Rc::new(std::cell::RefCell::new(game_name.to_string()));
    let view_page = build_view_tab(state, game_name, &name_holder);
    page_stack.add_titled(&view_page, Some("view"), "View");
    let app_page = build_app_tab(state, game_name, parent);
    page_stack.add_titled(&app_page, Some("app"), "AppImage");
    let sys_page = build_system_tab(state, game_name);
    page_stack.add_titled(&sys_page, Some("system"), "System");
    content.append(&page_stack);
    let tab_bar = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    tab_bar.set_halign(gtk::Align::Fill);
    let tabs: Vec<(&str, &str, &str)> = vec![
        ("View", "view", "palette"),
        ("AppImage", "app", "startup"),
        ("System", "system", "graphics"),
    ];
    let mut tab_buttons: Vec<gtk::ToggleButton> = Vec::new();
    let mut tab_indicators: Vec<gtk::Box> = Vec::new();
    for (i, (label, id, icon_name)) in tabs.iter().enumerate() {
        let btn_box = gtk::Box::new(gtk::Orientation::Horizontal, 0);
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
        let _ = id;
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
    let tab_bar_frame = gtk::Box::new(gtk::Orientation::Vertical, 0);
    tab_bar_frame.add_css_class("settings-tab-bar");
    tab_bar_frame.set_margin_start(8);
    tab_bar_frame.set_margin_end(8);
    tab_bar_frame.set_margin_bottom(8);
    tab_bar_frame.append(&tab_bar);
    content.append(&tab_bar_frame);
    let state_c = state.clone();
    let holder_c = name_holder.clone();
    let orig_c = game_name.to_string();
    dialog.connect_closed(move |_| {
        let new_name = holder_c.borrow().trim().to_string();
        if !new_name.is_empty() && new_name != orig_c {
            if state_c.game_model.get_game(&new_name).is_none() {
                state_c.game_model.rename_game(&orig_c, &new_name);
                *state_c.selected_game.borrow_mut() = new_name;
            }
        }
        state_c.recent_model.refresh(30);
        on_close();
    });
    dialog.set_child(Some(&content));
    dialog.present(Some(parent));
}
