use adw::prelude::*;
use gtk::prelude::*;
use std::cell::{Cell, RefCell};
use std::path::PathBuf;
use std::rc::Rc;

use crate::backend::game_model::GameSource;
use crate::backend::theme::{ThemeManager, ThemeMode, all_accents};
use crate::ui::helpers;
use crate::AppState;

fn make_settings_tab_icon(name: &str, theme: &ThemeManager) -> gtk::Image {
    helpers::themed_image(name, theme.is_dark(), 16)
}

fn capitalize(s: &str) -> String {
    let mut c = s.chars();
    match c.next() {
        None => String::new(),
        Some(f) => f.to_uppercase().to_string() + c.as_str(),
    }
}

pub fn show_settings_modal(
    state: &AppState,
    parent: &adw::ApplicationWindow,
    sidebar: &Rc<RefCell<Option<crate::ui::sidebar::Sidebar>>>,
    center: &Rc<RefCell<Option<crate::ui::center::CenterHandle>>>,
    details: &Rc<RefCell<Option<crate::ui::details_panel::DetailsPanel>>>,
) {
    // Fresh scan on open: external deletions (file manager) would otherwise
    // show stale builds.
    state.proton.refresh_installed();
    let dialog = adw::Dialog::new();
    dialog.set_title("Settings");
    dialog.set_content_width(720);
    dialog.set_content_height(520);

    let outer = gtk::Box::new(gtk::Orientation::Vertical, 0);
    outer.add_css_class("modal-bg");
    outer.set_margin_bottom(8);

    let (header_row, x_btn) = helpers::modal_header("Settings");
    header_row.set_margin_top(8);
    header_row.set_margin_start(16);
    header_row.set_margin_end(8);
    {
        let dlg = dialog.clone();
        x_btn.connect_clicked(move |_| { dlg.close(); });
    }
    // Pollers below stop when the dialog closes (no zombie 100ms wakeups).
    let dlg_alive: Rc<Cell<bool>> = Rc::new(Cell::new(true));
    {
        let alive = dlg_alive.clone();
        let closed_state = state.clone();
        let closed_sb = sidebar.clone();
        dialog.connect_closed(move |_| {
            alive.set(false);
            // Plugins may have been installed/removed/toggled: rescan and
            // update gated UI (sidebar Minecraft/Stores buttons).
            closed_state.plugins.refresh();
            if let Some(ref sb) = *closed_sb.borrow() {
                sb.refresh_plugin_buttons();
            }
        });
    }
    outer.append(&header_row);

    let page_stack = gtk::Stack::new();
    page_stack.set_vexpand(true);

    // Visuals page
    let visuals_page = gtk::Box::new(gtk::Orientation::Vertical, 12);
    visuals_page.set_margin_top(12);
    visuals_page.set_margin_bottom(12);
    visuals_page.set_margin_start(40);
    visuals_page.set_margin_end(40);
    visuals_page.set_halign(gtk::Align::Fill);

    let theme_label = gtk::Label::new(Some("Theme Mode"));
    theme_label.set_halign(gtk::Align::Center);
    theme_label.add_css_class("settings-title");
    visuals_page.append(&theme_label);

    let theme_sub = gtk::Label::new(Some("Dark is the default look. Light is easier on bright screens."));
    theme_sub.set_opacity(0.6);
    theme_sub.set_wrap(true);
    theme_sub.set_halign(gtk::Align::Start);
    visuals_page.append(&theme_sub);

    let theme_box = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    let dark_btn = gtk::ToggleButton::new();
    let light_btn = gtk::ToggleButton::new();
    for (btn, icon_name, label) in [(&dark_btn, "moon", "Dark"), (&light_btn, "sun", "Light")] {
        let content = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        content.set_halign(gtk::Align::Center);
        let img = helpers::themed_image(icon_name, state.theme.is_dark(), 16);
        content.append(&img);
        content.append(&gtk::Label::new(Some(label)));
        btn.set_child(Some(&content));
        btn.set_hexpand(true);
    }
    dark_btn.set_group(Some(&light_btn));
    dark_btn.set_active(state.theme.is_dark());
    light_btn.set_active(!state.theme.is_dark());

    let state_clone = state.clone();
    dark_btn.connect_clicked(move |_| {
        state_clone.theme.set_theme(ThemeMode::Dark);
        helpers::apply_theme_css(&state_clone.theme);
        helpers::init_accent_provider(&state_clone.theme);
        helpers::refresh_themed_icons(true);
    });
    let state_clone2 = state.clone();
    light_btn.connect_clicked(move |_| {
        state_clone2.theme.set_theme(ThemeMode::Light);
        helpers::apply_theme_css(&state_clone2.theme);
        helpers::init_accent_provider(&state_clone2.theme);
        helpers::refresh_themed_icons(false);
    });
    theme_box.append(&dark_btn);
    theme_box.append(&light_btn);
    visuals_page.append(&theme_box);

    let accent_label = gtk::Label::new(Some("Theme Colors"));
    accent_label.set_halign(gtk::Align::Center);
    accent_label.add_css_class("settings-title");
    visuals_page.append(&accent_label);

    let accent_sub = gtk::Label::new(Some("Choose an accent color for buttons, switches, and highlights"));
    accent_sub.set_opacity(0.6);
    accent_sub.set_wrap(true);
    accent_sub.set_halign(gtk::Align::Start);
    visuals_page.append(&accent_sub);

    let accent_grid = gtk::Grid::new();
    accent_grid.set_column_homogeneous(true);
    accent_grid.set_row_homogeneous(true);
    accent_grid.set_column_spacing(8);
    accent_grid.set_row_spacing(8);

    let accents = all_accents();
    let current_accent = state.theme.accent_id();
    let mut accent_btns: Vec<gtk::ToggleButton> = Vec::new();
    for (i, accent) in accents.iter().enumerate() {
        let display_name = capitalize(&accent.name);
        let btn = gtk::ToggleButton::with_label(&display_name);
        btn.add_css_class(&format!("accent-swatch-{}", accent.name));
        let accent_clone = accent.clone();
        let state_clone = state.clone();
        btn.connect_clicked(move |_| {
            state_clone.theme.set_accent_id(accent_clone.id);
            helpers::apply_theme_css(&state_clone.theme);
            helpers::init_accent_provider(&state_clone.theme);
        });
        if accent.id == current_accent { btn.set_active(true); }
        let col = (i % 5) as i32;
        let row = (i / 5) as i32;
        accent_grid.attach(&btn, col, row, 1, 1);
        accent_btns.push(btn);
    }
    for btn in &accent_btns[1..] { btn.set_group(Some(&accent_btns[0])); }
    visuals_page.append(&accent_grid);
    page_stack.add_titled(&visuals_page, Some("visuals"), "Visuals");

    // Paths page (C++ parity: installs/downloads/prefixes/protons/2/3 + shared)
    let paths_page = gtk::Box::new(gtk::Orientation::Vertical, 8);
    paths_page.set_margin_top(12);
    paths_page.set_margin_start(40);
    paths_page.set_margin_end(40);
    let paths_title = gtk::Label::new(Some("Paths"));
    paths_title.set_halign(gtk::Align::Center);
    paths_title.add_css_class("settings-title");
    paths_page.append(&paths_title);

    // Created early so path rows can trigger a proton rescan + UI rebuild
    // when a proton path changes (otherwise new paths stay undetected).
    let rebuild_proton_slot: Rc<RefCell<Option<Rc<dyn Fn()>>>> = Rc::new(RefCell::new(None));
    let make_editable_row = |label: &str, config_key: &str, base: &str, state: &AppState| {
        let col = gtk::Box::new(gtk::Orientation::Vertical, 4);
        col.set_margin_top(6);
        let lbl = gtk::Label::new(Some(label));
        lbl.set_halign(gtk::Align::Start);
        lbl.add_css_class("time-label");
        let row = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        let entry = gtk::Entry::new();
        let custom = state.config.launcher_value(config_key).unwrap_or_default();
        let shown = if !custom.trim().is_empty() {
            custom.trim().to_string()
        } else if !base.is_empty() {
            state.config.base_path_for(base).display().to_string()
        } else {
            String::new()
        };
        entry.set_text(&shown);
        entry.set_hexpand(true);
        let browse_btn = helpers::icon_button("folder", state.theme.is_dark(), "Choose folder");
        let state_c = state.clone();
        let entry_c = entry.clone();
        let key = config_key.to_string();
        let label_str = label.to_string();
        let slot_c = rebuild_proton_slot.clone();
        browse_btn.connect_clicked(move |_| {
            if let Some(path) = state_c.config.pick_folder(&format!("Select {}", label_str)) {
                entry_c.set_text(&path.display().to_string());
                state_c.config.set_launcher_value(&key, &path.display().to_string());
                if key.starts_with("protonsPath") {
                    state_c.proton.refresh_installed();
                    if let Some(r) = slot_c.borrow().clone() {
                        r();
                    }
                }
            }
        });
        // Save on Enter / focus-leave, NOT per keystroke: clearing the
        // field mid-edit used to persist the transient empty value and
        // wipe the stored path.
        let state_c2 = state.clone();
        let key2 = config_key.to_string();
        let slot_c2 = rebuild_proton_slot.clone();
        let save_path = Rc::new(move |text: &str| {
            state_c2.config.set_launcher_value(&key2, text.trim());
            if key2.starts_with("protonsPath") {
                state_c2.proton.refresh_installed();
                if let Some(r) = slot_c2.borrow().clone() {
                    r();
                }
            }
        });
        let save_c = save_path.clone();
        let entry_c2 = entry.clone();
        entry.connect_activate(move |_| {
            save_c(&entry_c2.text().to_string());
        });
        let save_c = save_path.clone();
        let entry_c3 = entry.clone();
        let focus = gtk::EventControllerFocus::new();
        focus.connect_leave(move |_| {
            save_c(&entry_c3.text().to_string());
        });
        entry.add_controller(focus);
        row.append(&entry);
        row.append(&browse_btn);
        col.append(&lbl);
        col.append(&row);
        col
    };

    paths_page.append(&make_editable_row("Installs path", "installsPath", "installs", state));
    paths_page.append(&make_editable_row("Downloads path", "downloadsPath", "downloads", state));
    paths_page.append(&make_editable_row("Prefixes path", "prefixesPath", "prefixes", state));
    paths_page.append(&make_editable_row("Proton Path 1", "protonsPath", "protons", state));
    paths_page.append(&make_editable_row("Proton Path 2 (optional)", "protonsPath2", "", state));
    paths_page.append(&make_editable_row("Proton Path 3 (optional)", "protonsPath3", "", state));

    // Shared Prefixes section
    let shared_title = gtk::Label::new(Some("Shared Prefixes"));
    shared_title.set_halign(gtk::Align::Center);
    shared_title.add_css_class("settings-title");
    shared_title.set_margin_top(12);
    paths_page.append(&shared_title);
    let shared_desc = gtk::Label::new(Some("Create shared Wine/Proton prefixes. Each is tied to a specific Proton version. Games using that Proton will share the same prefix."));
    shared_desc.set_wrap(true);
    shared_desc.set_halign(gtk::Align::Center);
    shared_desc.set_opacity(0.6);
    shared_desc.add_css_class("time-label");
    paths_page.append(&shared_desc);

    let shared_list_box = gtk::Box::new(gtk::Orientation::Vertical, 6);
    paths_page.append(&shared_list_box);

    refresh_shared_list(&shared_list_box, state);

    let add_shared_btn = gtk::Button::with_label("Add shared prefix");
    add_shared_btn.add_css_class("add-btn");
    add_shared_btn.set_halign(gtk::Align::Center);
    add_shared_btn.set_margin_top(4);
    {
        let state_c = state.clone();
        let win_c = parent.clone();
        let list_c = shared_list_box.clone();
        add_shared_btn.connect_clicked(move |_| {
            let dlg = adw::Dialog::new();
            dlg.set_title("Add shared prefix");
            dlg.set_content_width(380);
            dlg.set_content_height(240);
            // Full-bleed launcher style: outer fills, inner pads.
            let outer = gtk::Box::new(gtk::Orientation::Vertical, 0);
            outer.add_css_class("modal-bg");
            outer.set_hexpand(true);
            outer.set_vexpand(true);
            let col = gtk::Box::new(gtk::Orientation::Vertical, 10);
            col.set_hexpand(true);
            col.set_vexpand(true);
            col.set_margin_top(12);
            col.set_margin_bottom(12);
            col.set_margin_start(16);
            col.set_margin_end(16);
            outer.append(&col);
            let (header_row, x_btn) = helpers::modal_header("Add shared prefix");
            {
                let d0 = dlg.clone();
                x_btn.connect_clicked(move |_| { d0.close(); });
            }
            col.append(&header_row);
            let lbl = gtk::Label::new(Some("Proton version for this shared prefix:"));
            lbl.set_halign(gtk::Align::Start);
            col.append(&lbl);
            // Pick from installed builds instead of typing a version.
            state_c.proton.refresh_installed();
            let installed = state_c.proton.installed_protons();
            let drop = gtk::DropDown::new(None::<gtk::StringList>, None::<gtk::Expression>);
            let sl = gtk::StringList::new(&[]);
            for p in &installed {
                sl.append(p);
            }
            drop.set_model(Some(&sl.clone().upcast::<gio::ListModel>()));
            drop.set_hexpand(true);
            drop.set_selected(0);
            col.append(&drop);
            if installed.is_empty() {
                let empty = gtk::Label::new(Some("No Proton builds installed yet."));
                empty.set_halign(gtk::Align::Start);
                empty.set_opacity(0.6);
                empty.add_css_class("time-label");
                col.append(&empty);
            }
            let row = gtk::Box::new(gtk::Orientation::Horizontal, 10);
            let cancel = gtk::Button::with_label("Cancel");
            cancel.add_css_class("neon-red");
            cancel.set_hexpand(true);
            let add = gtk::Button::with_label("Add");
            add.add_css_class("neon-green");
            add.set_hexpand(true);
            add.set_sensitive(!installed.is_empty());
            let d1 = dlg.clone();
            cancel.connect_clicked(move |_| { d1.close(); });
            let d2 = dlg.clone();
            let state_cc = state_c.clone();
            let list_cc = list_c.clone();
            add.connect_clicked(move |_| {
                let idx = drop.selected() as usize;
                if idx < installed.len() {
                    state_cc.config.add_shared_prefix(&installed[idx]);
                    refresh_shared_list(&list_cc, &state_cc);
                }
                d2.close();
            });
            row.append(&cancel);
            row.append(&add);
            col.append(&row);
            dlg.set_child(Some(&outer));
            dlg.present(Some(&win_c));
        });
    }
    paths_page.append(&add_shared_btn);

    let paths_scroll = gtk::ScrolledWindow::new();
    paths_scroll.set_vexpand(true);
    paths_scroll.set_hscrollbar_policy(gtk::PolicyType::Never);
    paths_scroll.set_child(Some(&paths_page));
    page_stack.add_titled(&paths_scroll, Some("paths"), "Paths");

    // Protons page (C++ parity: default, filter, installed, available inline)
    let protons_page = gtk::Box::new(gtk::Orientation::Vertical, 8);
    protons_page.set_margin_top(12);
    protons_page.set_margin_start(40);
    protons_page.set_margin_end(40);
    let protons_title = gtk::Label::new(Some("Protons"));
    protons_title.set_halign(gtk::Align::Center);
    protons_title.add_css_class("settings-title");
    protons_page.append(&protons_title);

    // Default Proton for new games
    let def_row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    def_row.set_margin_top(4);
    let default_lbl = gtk::Label::new(Some("Default Proton for new games:"));
    default_lbl.set_halign(gtk::Align::Start);
    default_lbl.set_width_chars(26);
    let installed_names = state.proton.installed_protons();
    let default_proton = gtk::DropDown::new(None::<gtk::StringList>, None::<gtk::Expression>);
    let def_sl = gtk::StringList::new(&["(none)"]);
    for p in &installed_names { def_sl.append(p); }
    default_proton.set_model(Some(&def_sl.clone().upcast::<gio::ListModel>()));
    let current_default = state.config.launcher_value("defaultProton").unwrap_or_default();
    default_proton.set_selected(
        installed_names.iter().position(|p| *p == current_default)
            .map(|i| (i + 1) as u32)
            .unwrap_or(0),
    );
    default_proton.set_hexpand(true);
    {
        let state_c = state.clone();
        // Ignore the build-time set_selected emission so opening Settings
        // with an empty scan never wipes a stored default.
        let ready = Rc::new(Cell::new(false));
        let ready_c = ready.clone();
        default_proton.connect_selected_notify(move |d| {
            if !ready_c.get() {
                return;
            }
            let names = state_c.proton.installed_protons();
            let idx = d.selected() as usize;
            let val = if idx == 0 || idx > names.len() {
                String::new()
            } else {
                names[idx - 1].clone()
            };
            state_c.config.set_launcher_value("defaultProton", &val);
        });
        ready.set(true);
    }
    def_row.append(&default_lbl);
    def_row.append(&default_proton);
    protons_page.append(&def_row);

    // Source filter buttons: All | CachyOS | Proton-GE
    let filter_state: Rc<RefCell<String>> = Rc::new(RefCell::new("all".to_string()));
    let filter_box = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    filter_box.set_margin_top(6);
    let mut filter_btns: Vec<gtk::ToggleButton> = Vec::new();
    for (id, label) in [("all", "All"), ("cachy", "CachyOS"), ("ge", "Proton-GE")] {
        let b = gtk::ToggleButton::with_label(label);
        b.add_css_class("filter-btn");
        b.set_hexpand(true);
        if id == "all" { b.set_active(true); }
        filter_box.append(&b);
        filter_btns.push(b);
    }
    for b in &filter_btns[1..] { b.set_group(Some(&filter_btns[0])); }
    protons_page.append(&filter_box);

    // Status + progress
    let proton_status = gtk::Label::new(Some(""));
    proton_status.set_halign(gtk::Align::Start);
    proton_status.set_wrap(true);
    proton_status.add_css_class("time-label");
    protons_page.append(&proton_status);
    let proton_bar = gtk::ProgressBar::new();
    proton_bar.set_show_text(true);
    proton_bar.set_text(Some(""));
    protons_page.append(&proton_bar);

    // Installed builds
    let installed_title = gtk::Label::new(Some("Installed builds"));
    installed_title.set_halign(gtk::Align::Start);
    installed_title.add_css_class("details-title");
    installed_title.set_margin_top(6);
    protons_page.append(&installed_title);
    let installed_box = gtk::Box::new(gtk::Orientation::Vertical, 6);
    protons_page.append(&installed_box);

    // Available for download
    let avail_title_row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    avail_title_row.set_margin_top(6);
    let avail_title = gtk::Label::new(Some("Available for download"));
    avail_title.set_halign(gtk::Align::Start);
    avail_title.add_css_class("details-title");
    avail_title.set_hexpand(true);
    let refresh_btn = gtk::Button::with_label("Refresh");
    refresh_btn.add_css_class("settings-btn");
    avail_title_row.append(&avail_title);
    avail_title_row.append(&refresh_btn);
    protons_page.append(&avail_title_row);
    let avail_box = gtk::Box::new(gtk::Orientation::Vertical, 6);
    protons_page.append(&avail_box);

    // Path picker (shown when 2+ proton paths and a download starts)
    let path_picker = gtk::Box::new(gtk::Orientation::Vertical, 6);
    path_picker.set_visible(false);
    let path_picker_title = gtk::Label::new(Some("Install to:"));
    path_picker_title.set_halign(gtk::Align::Start);
    path_picker_title.add_css_class("info-label");
    path_picker.append(&path_picker_title);
    let path_picker_rows = gtk::Box::new(gtk::Orientation::Vertical, 4);
    path_picker.append(&path_picker_rows);
    let path_cancel = gtk::Button::with_label("Cancel");
    path_cancel.add_css_class("settings-btn");
    {
        let pp = path_picker.clone();
        path_cancel.connect_clicked(move |_| { pp.set_visible(false); });
    }
    path_picker.append(&path_cancel);
    protons_page.append(&path_picker);

    // Shared download machinery (thread + poller, like ProtonModal)
    let releases: Rc<RefCell<Vec<(String, String, String)>>> = Rc::new(RefCell::new(Vec::new()));
    let (ptx, prx) = std::sync::mpsc::channel::<ProtonTabMsg>();
    let prx = Rc::new(RefCell::new(prx));
    let pending_dl: Rc<RefCell<Option<(String, String)>>> = Rc::new(RefCell::new(None));

    // Forward slot so the installed-list Remove also refreshes Available.
    let rebuild_avail_early: Rc<RefCell<Option<Rc<dyn Fn()>>>> = Rc::new(RefCell::new(None));
    // Rebuild installed list (+ default dropdown, so new paths show up)
    let rebuild_installed = {
        let ibox = installed_box.clone();
        let state_c = state.clone();
        let proton_c = state.proton.clone();
        let slot_c = rebuild_proton_slot.clone();
        let avail_early_c = rebuild_avail_early.clone();
        let def_drop = default_proton.clone();
        let status_c = proton_status.clone();
        let win_c = parent.clone();
        move || {
            // Default dropdown follows the fresh scan (new paths appear).
            let fresh_names = proton_c.installed_protons();
            let def_sl = gtk::StringList::new(&["(none)"]);
            for p in &fresh_names {
                def_sl.append(p);
            }
            def_drop.set_model(Some(&def_sl.clone().upcast::<gio::ListModel>()));
            let cur_def = state_c.config.launcher_value("defaultProton").unwrap_or_default();
            def_drop.set_selected(
                fresh_names
                    .iter()
                    .position(|p| *p == cur_def)
                    .map(|i| (i + 1) as u32)
                    .unwrap_or(0),
            );
            while let Some(child) = ibox.first_child() {
                ibox.remove(&child);
            }
            let details = proton_c.installed_proton_details();
            if details.is_empty() {
                ibox.append(&gtk::Label::new(Some("No Proton builds installed")));
                // List just went empty (path removed/changed): offer the
                // automatic GE-Proton install.
                status_c.set_text("No builds found — downloading latest Proton-GE automatically…");
                crate::maybe_autoinstall_proton(&state_c, &win_c);
                return;
            }
            let npaths = proton_c.proton_paths().len();
            for (name, _version, path) in &details {
                let card = gtk::Frame::new(None);
                card.add_css_class("page-card");
                let inner = gtk::Box::new(gtk::Orientation::Horizontal, 8);
                inner.set_margin_top(4);
                inner.set_margin_bottom(4);
                inner.set_margin_start(8);
                inner.set_margin_end(8);
                let lbl = gtk::Label::new(Some(name));
                lbl.set_halign(gtk::Align::Start);
                lbl.set_hexpand(true);
                lbl.set_ellipsize(gtk::pango::EllipsizeMode::Middle);
                inner.append(&lbl);
                // Always show which configured path the build belongs to
                let proton_dir = PathBuf::from(path);
                let mut matched_path_idx: Option<usize> = None;
                for (i, cfg_path) in proton_c.proton_paths().iter().enumerate() {
                    // The proton's dir is a child of the configured path
                    if proton_dir.starts_with(cfg_path) || &proton_dir == cfg_path {
                        matched_path_idx = Some(i);
                        break;
                    }
                }
                if let Some(idx) = matched_path_idx {
                    let badge = gtk::Label::new(Some(&format!("Path {}", idx + 1)));
                    badge.add_css_class("proton-path-badge");
                    badge.set_valign(gtk::Align::Center);
                    inner.append(&badge);
                } else if npaths > 0 {
                    let badge = gtk::Label::new(Some("Unknown"));
                    badge.set_opacity(0.6);
                    badge.add_css_class("time-label");
                    inner.append(&badge);
                }
                let remove_btn = gtk::Button::with_label("Remove");
                remove_btn.add_css_class("destructive-action");
                let proton_cc = proton_c.clone();
                let name_c = name.clone();
                let state_cc = state_c.clone();
                let slot_cc = slot_c.clone();
                let avail_cc = avail_early_c.clone();
                remove_btn.connect_clicked(move |_| {
                    let _ = proton_cc.remove_proton(&name_c);
                    state_cc.proton.refresh_installed();
                    // remove_proton clears games pointing at it: reload model.
                    state_cc.game_model.load_from_disk();
                    if let Some(r) = slot_cc.borrow().clone() {
                        r();
                    }
                    if let Some(r) = avail_cc.borrow().clone() {
                        r();
                    }
                });
                inner.append(&remove_btn);
                card.set_child(Some(&inner));
                ibox.append(&card);
            }
        }
    };
    let rebuild_installed_rc = Rc::new(rebuild_installed);
    *rebuild_proton_slot.borrow_mut() = Some(rebuild_installed_rc.clone());
    rebuild_installed_rc();

    // Rebuild available list (filter-aware)
    let rebuild_avail_slot: Rc<RefCell<Option<Rc<dyn Fn()>>>> = Rc::new(RefCell::new(None));
    let rebuild_avail = {
        let abox = avail_box.clone();
        let releases_c = releases.clone();
        let filter_c = filter_state.clone();
        let state_c = state.clone();
        let status_c = proton_status.clone();
        let bar_c = proton_bar.clone();
        let picker_c = path_picker.clone();
        let picker_rows_c = path_picker_rows.clone();
        let pending_c = pending_dl.clone();
        let ptx_c = ptx.clone();
        let avail_slot_c = rebuild_avail_slot.clone();
        let proton_slot_c = rebuild_proton_slot.clone();
        move || {
            while let Some(child) = abox.first_child() {
                abox.remove(&child);
            }
            let filter = filter_c.borrow().clone();
            let installed = state_c.proton.installed_protons();
            let mut shown = 0;
            for (tag, url, source) in releases_c.borrow().iter() {
                if filter == "cachy" && source != "cachy" { continue; }
                if filter == "ge" && source != "ge" { continue; }
                shown += 1;
                let card = gtk::Frame::new(None);
                card.add_css_class("page-card");
                let inner = gtk::Box::new(gtk::Orientation::Horizontal, 8);
                inner.set_margin_top(4);
                inner.set_margin_bottom(4);
                inner.set_margin_start(8);
                inner.set_margin_end(8);
                let prefix = if source == "cachy" { "[CachyOS] " } else { "[GE] " };
                let lbl = gtk::Label::new(Some(&format!("{}{}", prefix, tag)));
                lbl.set_halign(gtk::Align::Start);
                lbl.set_hexpand(true);
                lbl.set_ellipsize(gtk::pango::EllipsizeMode::Middle);
                inner.append(&lbl);
                let already = installed.iter().any(|n| n == tag);
                let action = gtk::Button::with_label(if already { "Remove" } else { "Download" });
                if already {
                    action.add_css_class("destructive-action");
                } else {
                    action.add_css_class("add-btn");
                }
                let tag_c = tag.clone();
                let url_c = url.clone();
                let status_cc = status_c.clone();
                let bar_cc = bar_c.clone();
                let picker_cc = picker_c.clone();
                let picker_rows_cc = picker_rows_c.clone();
                let pending_cc = pending_c.clone();
                let ptx_cc = ptx_c.clone();
                let state_cc = state_c.clone();
                let avail_slot_cc = avail_slot_c.clone();
                let proton_slot_cc = proton_slot_c.clone();
                action.connect_clicked(move |_| {
                    if state_cc.proton.installed_protons().iter().any(|n| n == &tag_c) {
                        let _ = state_cc.proton.remove_proton(&tag_c);
                        state_cc.proton.refresh_installed();
                        state_cc.game_model.load_from_disk();
                        status_cc.set_text(&format!("Removed {}", tag_c));
                        if let Some(r) = avail_slot_cc.borrow().clone() {
                            r();
                        }
                        if let Some(r) = proton_slot_cc.borrow().clone() {
                            r();
                        }
                        return;
                    }
                    let paths = state_cc.proton.proton_paths();
                    if paths.len() > 1 {
                        // Ask destination first (C++ parity)
                        while let Some(child) = picker_rows_cc.first_child() {
                            picker_rows_cc.remove(&child);
                        }
                        *pending_cc.borrow_mut() = Some((tag_c.clone(), url_c.clone()));
                        for (i, p) in paths.iter().enumerate() {
                            let b = gtk::Button::with_label(
                                &format!("Path {} — {}", i + 1, p.display()),
                            );
                            b.set_halign(gtk::Align::Fill);
                            let tag_cc = tag_c.clone();
                            let url_cc = url_c.clone();
                            let status_ccc = status_cc.clone();
                            let bar_ccc = bar_cc.clone();
                            let picker_ccc = picker_cc.clone();
                            let ptx_ccc = ptx_cc.clone();
                            let dest = p.clone();
                            b.connect_clicked(move |_| {
                                picker_ccc.set_visible(false);
                                status_ccc.set_text(&format!("Downloading {}...", tag_cc));
                                bar_ccc.set_fraction(0.0);
                                start_proton_download(
                                    &ptx_ccc, &tag_cc, &url_cc, &dest,
                                );
                            });
                            picker_rows_cc.append(&b);
                        }
                        picker_cc.set_visible(true);
                    } else {
                        status_cc.set_text(&format!("Downloading {}...", tag_c));
                        bar_cc.set_fraction(0.0);
                        let dest = paths.first().cloned()
                            .unwrap_or_else(|| state_cc.proton.default_download_dir());
                        start_proton_download(&ptx_cc, &tag_c, &url_c, &dest);
                    }
                });
                inner.append(&action);
                card.set_child(Some(&inner));
                abox.append(&card);
            }
            if shown == 0 {
                let hint = gtk::Label::new(Some("Press Refresh to load releases."));
                hint.set_opacity(0.6);
                hint.add_css_class("time-label");
                abox.append(&hint);
            }
        }
    };
    let rebuild_avail_rc = Rc::new(rebuild_avail);
    *rebuild_avail_slot.borrow_mut() = Some(rebuild_avail_rc.clone());
    *rebuild_avail_early.borrow_mut() = Some(rebuild_avail_rc.clone());

    // Poller: releases / progress / finished
    {
        let status = proton_status.clone();
        let bar = proton_bar.clone();
        let releases_c = releases.clone();
        let rebuild_a = rebuild_avail_rc.clone();
        let rebuild_i = rebuild_installed_rc.clone();
        let state_c = state.clone();
        let alive_p = dlg_alive.clone();
        glib::timeout_add_local(std::time::Duration::from_millis(100), move || {
            if !alive_p.get() {
                return glib::ControlFlow::Break;
            }
            let msgs: Vec<ProtonTabMsg> = {
                let r = prx.borrow();
                let mut v = Vec::new();
                while let Ok(m) = r.try_recv() {
                    v.push(m);
                }
                v
            };
            for msg in msgs {
                match msg {
                    ProtonTabMsg::Releases(Ok(list)) => {
                        *releases_c.borrow_mut() = list;
                        let n = releases_c.borrow().len();
                        status.set_text(&format!("{} releases loaded", n));
                        rebuild_a();
                    }
                    ProtonTabMsg::Releases(Err(e)) => {
                        status.set_text(&format!("Failed to load releases: {}", e));
                    }
                    ProtonTabMsg::Progress(f, bps) => {
                        bar.set_fraction(f.clamp(0.0, 1.0));
                        bar.set_text(Some(&format!(
                            "{:.0}% · {}",
                            f * 100.0,
                            crate::backend::proton::ProtonManager::format_speed(bps)
                        )));
                    }
                    ProtonTabMsg::Downloaded(Ok(tag)) => {
                        state_c.proton.refresh_installed();
                        status.set_text(&format!("Installed {}", tag));
                        bar.set_fraction(0.0);
                        bar.set_text(Some(""));
                        rebuild_i();
                        rebuild_a();
                    }
                    ProtonTabMsg::Downloaded(Err(e)) => {
                        status.set_text(&format!("Download failed: {}", e));
                    }
                }
            }
            glib::ControlFlow::Continue
        });
    }

    // Refresh button: fetch both sources, merge with source tags
    {
        let ptx_c = ptx.clone();
        let status_c = proton_status.clone();
        refresh_btn.connect_clicked(move |_| {
            status_c.set_text("Loading releases...");
            let tx2 = ptx_c.clone();
            std::thread::spawn(move || {
                let mut all = Vec::new();
                let mut err = String::new();
                match crate::backend::proton::ProtonManager::fetch_releases("ge") {
                    Ok(list) => {
                        for (tag, url) in list {
                            all.push((tag, url, "ge".to_string()));
                        }
                    }
                    Err(e) => err = e,
                }
                match crate::backend::proton::ProtonManager::fetch_releases("cachyos") {
                    Ok(list) => {
                        for (tag, url) in list {
                            all.push((tag, url, "cachy".to_string()));
                        }
                    }
                    Err(e) => {
                        if err.is_empty() {
                            err = e;
                        }
                    }
                }
                if all.is_empty() && !err.is_empty() {
                    let _ = tx2.send(ProtonTabMsg::Releases(Err(err)));
                } else {
                    let _ = tx2.send(ProtonTabMsg::Releases(Ok(all)));
                }
            });
        });
    }

    // Filter buttons rebuild the available list
    for (btn, id) in filter_btns.iter().zip(["all", "cachy", "ge"]) {
        let filter_c = filter_state.clone();
        let rebuild_c = rebuild_avail_rc.clone();
        let id_owned = id.to_string();
        btn.connect_clicked(move |_| {
            *filter_c.borrow_mut() = id_owned.clone();
            rebuild_c();
        });
    }
    rebuild_avail_rc();

    let protons_scroll = gtk::ScrolledWindow::new();
    protons_scroll.set_vexpand(true);
    protons_scroll.set_hscrollbar_policy(gtk::PolicyType::Never);
    protons_scroll.set_child(Some(&protons_page));
    page_stack.add_titled(&protons_scroll, Some("protons"), "Protons");

    // Misc page (C++ parity: gamesUses* keys, "1"/"0", status texts)
    let misc_page = gtk::Box::new(gtk::Orientation::Vertical, 12);
    misc_page.set_margin_top(12);
    misc_page.set_margin_start(40);
    misc_page.set_margin_end(40);
    let misc_title = gtk::Label::new(Some("Miscellaneous"));
    misc_title.set_halign(gtk::Align::Center);
    misc_title.add_css_class("settings-title");
    misc_page.append(&misc_title);

    let general_lbl = gtk::Label::new(Some("General"));
    general_lbl.set_halign(gtk::Align::Center);
    general_lbl.add_css_class("time-label");
    misc_page.append(&general_lbl);

    // Gray bordered card around all options (QML parity)
    let misc_card = gtk::Frame::new(None);
    misc_card.add_css_class("page-card");
    let misc_inner = gtk::Box::new(gtk::Orientation::Vertical, 4);
    misc_inner.set_margin_top(4);
    misc_inner.set_margin_bottom(4);
    misc_inner.set_margin_start(6);
    misc_inner.set_margin_end(6);
    misc_card.set_child(Some(&misc_inner));
    misc_page.append(&misc_card);

    // Per-arch availability (C++ graphicsComponentStatus parity, no exe here)
    let gm_comp = state.proton.component_status("gamemode", "");
    let mh_comp = state.proton.component_status("mangohud", "");
    let gamemode_avail = gm_comp.available;
    let mangohud_avail = mh_comp.available;
    let umu_avail = state.proton.is_umu_available();

    let misc_switch = |key: &str, label: &str, default_on: bool, enabled: bool, state: &AppState| {
        let row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        row.set_margin_top(4);
        let lbl = gtk::Label::new(Some(label));
        lbl.set_halign(gtk::Align::Start);
        lbl.set_hexpand(true);
        lbl.set_wrap(true);
        let sw = gtk::Switch::new();
        sw.set_halign(gtk::Align::End);
        sw.set_sensitive(enabled);
        let current_val = state.config.launcher_value(key)
            .map(|v| v == "1")
            .unwrap_or(default_on);
        sw.set_active(current_val);
        let state_clone = state.clone();
        let key_owned = key.to_string();
        sw.connect_active_notify(move |switch| {
            state_clone.config.set_launcher_value(&key_owned, if switch.is_active() { "1" } else { "0" });
        });
        row.append(&lbl);
        row.append(&sw);
        row
    };

    misc_inner.append(&misc_switch("gamesUsesWined3d", "Use wined3d instead of DXVK", false, true, state));
    misc_inner.append(&misc_switch("gamesUsesWayland", "Prefer Wayland driver", false, true, state));
    misc_inner.append(&misc_switch("gamesUsesSteamRuntime", "Use Steam Runtime for Proton games", true, true, state));
    let runtime_note = gtk::Label::new(Some("Provides 32-bit audio/video libs on pure 64-bit systems"));
    runtime_note.set_halign(gtk::Align::Start);
    runtime_note.set_opacity(0.6);
    runtime_note.add_css_class("time-label");
    misc_inner.append(&runtime_note);
    misc_inner.append(&misc_switch("gamesUsesUmu", "Use umu-launcher (unified Proton runner)", false, true, state));
    let umu_status = gtk::Label::new(Some(if umu_avail {
        "umu-run: ready"
    } else {
        "umu-run: not installed (toggle stays available; install umu to use it)"
    }));
    umu_status.set_halign(gtk::Align::Start);
    umu_status.set_opacity(0.6);
    umu_status.set_wrap(true);
    umu_status.add_css_class("time-label");
    misc_inner.append(&umu_status);
    misc_inner.append(&misc_switch("gamesUsesGameMode", "Use GameMode for new games", false, gamemode_avail, state));
    let gm_status = gtk::Label::new(Some(
        &crate::backend::proton::ProtonManager::component_status_text(&gm_comp, "GameMode"),
    ));
    gm_status.set_halign(gtk::Align::Start);
    gm_status.set_opacity(0.6);
    gm_status.add_css_class("time-label");
    misc_inner.append(&gm_status);
    misc_inner.append(&misc_switch("gamesUsesMangoHud", "Use MangoHud for new games", false, mangohud_avail, state));
    let mh_status = gtk::Label::new(Some(
        &crate::backend::proton::ProtonManager::component_status_text(&mh_comp, "MangoHud"),
    ));
    mh_status.set_halign(gtk::Align::Start);
    mh_status.set_opacity(0.6);
    mh_status.add_css_class("time-label");
    misc_inner.append(&mh_status);

    let misc_scroll = gtk::ScrolledWindow::new();
    misc_scroll.set_vexpand(true);
    misc_scroll.set_hscrollbar_policy(gtk::PolicyType::Never);
    misc_scroll.set_child(Some(&misc_page));
    page_stack.add_titled(&misc_scroll, Some("misc"), "Misc");

    // Plugins page (C++ parity: installed w/ delete, emulator-manager, registry)
    let plugins_page = gtk::Box::new(gtk::Orientation::Vertical, 12);
    plugins_page.set_margin_top(12);
    plugins_page.set_margin_start(40);
    plugins_page.set_margin_end(40);
    let plugins_title = gtk::Label::new(Some("Plugins"));
    plugins_title.set_halign(gtk::Align::Center);
    plugins_title.add_css_class("settings-title");
    plugins_page.append(&plugins_title);

    let installed_title = gtk::Label::new(Some("Installed Plugins"));
    installed_title.set_halign(gtk::Align::Center);
    installed_title.add_css_class("details-title");
    plugins_page.append(&installed_title);
    let installed_box = gtk::Box::new(gtk::Orientation::Vertical, 8);
    plugins_page.append(&installed_box);

    // Emulator expand + installing statuses + registry state
    let emu_expanded: Rc<RefCell<bool>> = Rc::new(RefCell::new(false));
    let emu_list: Rc<RefCell<Vec<crate::backend::plugins::EmuInfo>>> =
        Rc::new(RefCell::new(Vec::new()));
    let emu_status: Rc<RefCell<std::collections::HashMap<String, String>>> =
        Rc::new(RefCell::new(std::collections::HashMap::new()));
    let registry: Rc<RefCell<Vec<crate::backend::plugins::RegistryEntry>>> =
        Rc::new(RefCell::new(Vec::new()));

    let (pltx, plrx) = std::sync::mpsc::channel::<PluginTabMsg>();
    let plrx = Rc::new(RefCell::new(plrx));

    // Slot so rows rebuilt by refresh can re-trigger a full rebuild
    // (e.g. the emulator-manager arrow toggle).
    let rebuild_slot: Rc<RefCell<Option<Rc<dyn Fn()>>>> = Rc::new(RefCell::new(None));
    // Slot for the Available-plugins (registry) list so delete handlers
    // can refresh it without a network round-trip.
    let rebuild_reg_slot: Rc<RefCell<Option<Rc<dyn Fn()>>>> = Rc::new(RefCell::new(None));
    // Rebuild installed plugin cards
    let rebuild_plugins = {
        let ibox = installed_box.clone();
        let state_c = state.clone();
        let emu_exp_c = emu_expanded.clone();
        let emu_list_c = emu_list.clone();
        let emu_status_c = emu_status.clone();
        let pltx_c = pltx.clone();
        let slot_c = rebuild_slot.clone();
        let reg_slot_c = rebuild_reg_slot.clone();
        move || {
            while let Some(child) = ibox.first_child() {
                ibox.remove(&child);
            }
            let list = state_c.plugins.list_plugins();
            if list.is_empty() {
                let empty = gtk::Label::new(Some("No plugins installed."));
                empty.set_opacity(0.6);
                empty.add_css_class("time-label");
                ibox.append(&empty);
                return;
            }
            for p in &list {
                let card = gtk::Frame::new(None);
                card.add_css_class("page-card");
                let inner = gtk::Box::new(gtk::Orientation::Vertical, 4);
                inner.set_margin_top(6);
                inner.set_margin_bottom(6);
                inner.set_margin_start(10);
                inner.set_margin_end(10);
                if p.plugin_type == crate::backend::plugins::PluginType::EmulatorManager {
                    // Expandable emulator-manager row (arrow IS the toggle button)
                    let row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
                    let arrow = gtk::Button::with_label(if *emu_exp_c.borrow() { "▼" } else { "▶" });
                    arrow.set_width_request(32);
                    arrow.add_css_class("settings-btn");
                    {
                        let exp_c = emu_exp_c.clone();
                        let slot_cc = slot_c.clone();
                        let state_cc = state_c.clone();
                        let pltx_cc = pltx_c.clone();
                        arrow.connect_clicked(move |_| {
                            let now = !*exp_c.borrow();
                            *exp_c.borrow_mut() = now;
                            if now {
                                let tx2 = pltx_cc.clone();
                                let dir = state_cc.plugins.plugins_dir();
                                std::thread::spawn(move || {
                                    let list = crate::backend::plugins::PluginManager::list_emulators_in(&dir);
                                    let _ = tx2.send(PluginTabMsg::Emus(list));
                                });
                            }
                            if let Some(r) = slot_cc.borrow().clone() {
                                r();
                            }
                        });
                    }
                    let name_lbl = gtk::Label::new(Some(&format!(
                        "{}{}", p.name,
                        if p.version.is_empty() { String::new() } else { format!("  v{}", p.version) }
                    )));
                    name_lbl.set_halign(gtk::Align::Start);
                    name_lbl.set_hexpand(true);
                    name_lbl.set_ellipsize(gtk::pango::EllipsizeMode::End);
                    let del = gtk::Button::with_label("✕");
                    del.add_css_class("destructive-action");
                    del.set_width_request(36);
                    let state_cc = state_c.clone();
                    let pid = p.id.clone();
                    let slot_cc = slot_c.clone();
                    let reg_cc = reg_slot_c.clone();
                    del.connect_clicked(move |_| {
                        state_cc.plugins.remove_plugin(&pid);
                        if let Some(r) = slot_cc.borrow().clone() {
                            r();
                        }
                        if let Some(rr) = reg_cc.borrow().clone() {
                            rr();
                        }
                    });
                    row.append(&arrow);
                    row.append(&name_lbl);
                    row.append(&del);
                    inner.append(&row);
                    if !p.description.is_empty() {
                        let desc = gtk::Label::new(Some(&p.description));
                        desc.set_halign(gtk::Align::Start);
                        desc.set_opacity(0.6);
                        desc.set_wrap(true);
                        desc.add_css_class("time-label");
                        inner.append(&desc);
                    }
                    card.set_child(Some(&inner));
                    ibox.append(&card);
                    // Expanded emulator list lives right after this card
                    if *emu_exp_c.borrow() {
                        let emu_box = gtk::Box::new(gtk::Orientation::Vertical, 4);
                        emu_box.set_margin_start(16);
                        ibox.append(&emu_box);
                        rebuild_emu_rows(&emu_box, &state_c, &emu_list_c, &emu_status_c, &pltx_c);
                    }
                } else {
                    let row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
                    let col = gtk::Box::new(gtk::Orientation::Vertical, 2);
                    col.set_hexpand(true);
                    let name_lbl = gtk::Label::new(Some(&format!(
                        "{}{}", p.name,
                        if p.version.is_empty() { String::new() } else { format!("  v{}", p.version) }
                    )));
                    name_lbl.set_halign(gtk::Align::Start);
                    name_lbl.set_ellipsize(gtk::pango::EllipsizeMode::End);
                    col.append(&name_lbl);
                    if !p.description.is_empty() {
                        let desc = gtk::Label::new(Some(&p.description));
                        desc.set_halign(gtk::Align::Start);
                        desc.set_opacity(0.6);
                        desc.set_wrap(true);
                        desc.add_css_class("time-label");
                        col.append(&desc);
                    }
                    if !p.capabilities.is_empty() {
                        let caps = gtk::Label::new(Some(&format!("Capabilities: {}", p.capabilities.join(", "))));
                        caps.set_halign(gtk::Align::Start);
                        caps.set_opacity(0.5);
                        caps.set_wrap(true);
                        caps.add_css_class("time-label");
                        col.append(&caps);
                    }
                    let sw = gtk::Switch::new();
                    sw.set_valign(gtk::Align::Center);
                    sw.set_active(p.enabled);
                    let state_cc = state_c.clone();
                    let pid = p.id.clone();
                    sw.connect_active_notify(move |s| {
                        state_cc.plugins.set_enabled(&pid, s.is_active());
                    });
                    let del = gtk::Button::with_label("✕");
                    del.add_css_class("destructive-action");
                    del.set_valign(gtk::Align::Center);
                    del.set_width_request(36);
                    let state_cc2 = state_c.clone();
                    let pid2 = p.id.clone();
                    let slot_cc2 = slot_c.clone();
                    let reg_cc2 = reg_slot_c.clone();
                    del.connect_clicked(move |_| {
                        state_cc2.plugins.remove_plugin(&pid2);
                        if let Some(r) = slot_cc2.borrow().clone() {
                            r();
                        }
                        if let Some(rr) = reg_cc2.borrow().clone() {
                            rr();
                        }
                    });
                    row.append(&col);
                    row.append(&sw);
                    row.append(&del);
                    inner.append(&row);
                    card.set_child(Some(&inner));
                    ibox.append(&card);
                }
            }
        }
    };
    let rebuild_plugins_rc = Rc::new(rebuild_plugins);
    *rebuild_slot.borrow_mut() = Some(rebuild_plugins_rc.clone());

    // Available plugins (remote registry)
    let avail_title = gtk::Label::new(Some("Available Plugins"));
    avail_title.set_halign(gtk::Align::Center);
    avail_title.add_css_class("details-title");
    avail_title.set_margin_top(8);
    plugins_page.append(&avail_title);
    let registry_status = gtk::Label::new(Some(""));
    registry_status.set_halign(gtk::Align::Start);
    registry_status.set_wrap(true);
    registry_status.add_css_class("time-label");
    plugins_page.append(&registry_status);
    let registry_bar = gtk::ProgressBar::new();
    registry_bar.set_show_text(true);
    registry_bar.set_text(Some(""));
    plugins_page.append(&registry_bar);
    let registry_box = gtk::Box::new(gtk::Orientation::Vertical, 8);
    plugins_page.append(&registry_box);

    // Rebuild registry rows (filters out already-installed)
    let rebuild_registry = {
        let rbox = registry_box.clone();
        let reg_c = registry.clone();
        let state_c = state.clone();
        let status_c = registry_status.clone();
        let pltx_c = pltx.clone();
        let bar_c = registry_bar.clone();
        move || {
            while let Some(child) = rbox.first_child() {
                rbox.remove(&child);
            }
            let installed = state_c.plugins.list_plugins();
            let installed_ids: Vec<String> = installed.iter().map(|p| p.id.to_lowercase()).collect();
            let installed_names: Vec<String> = installed.iter().map(|p| {
                let n = p.name.clone();
                let lower = n.to_lowercase();
                // strip trailing " v1.2" like C++
                match lower.rfind(" v") {
                    Some(i) if lower[i + 2..].chars().all(|c| c.is_ascii_digit() || c == '.') => {
                        lower[..i].trim().to_string()
                    }
                    _ => lower.trim().to_string(),
                }
            }).collect();
            let mut shown = 0;
            for e in reg_c.borrow().iter() {
                let rtag = e.tag.to_lowercase();
                let rname = {
                    let lower = e.name.to_lowercase();
                    match lower.rfind(" v") {
                        Some(i) if lower[i + 2..].chars().all(|c| c.is_ascii_digit() || c == '.') => {
                            lower[..i].trim().to_string()
                        }
                        _ => lower.trim().to_string(),
                    }
                };
                let rasset = e.asset_name.to_lowercase();
                let mut found = false;
                for (k, id) in installed_ids.iter().enumerate() {
                    if rtag.contains(id) || rasset.contains(id) || rname == installed_names[k] {
                        found = true;
                        break;
                    }
                }
                if found {
                    continue;
                }
                shown += 1;
                let card = gtk::Frame::new(None);
                card.add_css_class("page-card");
                let inner = gtk::Box::new(gtk::Orientation::Horizontal, 8);
                inner.set_margin_top(6);
                inner.set_margin_bottom(6);
                inner.set_margin_start(10);
                inner.set_margin_end(10);
                let col = gtk::Box::new(gtk::Orientation::Vertical, 2);
                col.set_hexpand(true);
                let name_lbl = gtk::Label::new(Some(if e.name.is_empty() { &e.tag } else { &e.name }));
                name_lbl.set_halign(gtk::Align::Start);
                name_lbl.set_ellipsize(gtk::pango::EllipsizeMode::End);
                col.append(&name_lbl);
                if !e.description.is_empty() {
                    let desc = gtk::Label::new(Some(&e.description));
                    desc.set_halign(gtk::Align::Start);
                    desc.set_opacity(0.6);
                    desc.set_wrap(true);
                    desc.add_css_class("time-label");
                    col.append(&desc);
                }
                if !e.date.is_empty() {
                    let date = gtk::Label::new(Some(&e.date));
                    date.set_halign(gtk::Align::Start);
                    date.set_opacity(0.5);
                    date.add_css_class("time-label");
                    col.append(&date);
                }
                let install = gtk::Button::with_label("Install");
                install.add_css_class("add-btn");
                install.set_valign(gtk::Align::Center);
                let tag_c = e.tag.clone();
                let url_c = e.url.clone();
                let status_cc = status_c.clone();
                let bar_cc = bar_c.clone();
                let pltx_cc = pltx_c.clone();
                let state_cc = state_c.clone();
                install.connect_clicked(move |_| {
                    status_cc.set_text(&format!("Installing {}...", tag_c));
                    bar_cc.set_fraction(0.0);
                    let tx2 = pltx_cc.clone();
                    let dir = state_cc.plugins.plugins_dir();
                    let tag2 = tag_c.clone();
                    let url2 = url_c.clone();
                    std::thread::spawn(move || {
                        let res = crate::backend::plugins::PluginManager::download_plugin_in(
                            &dir, &tag2, &url2,
                            |f, bps| {
                                let _ = tx2.send(PluginTabMsg::Progress(f, bps));
                            },
                        );
                        let _ = tx2.send(PluginTabMsg::PlugDownloaded(res));
                    });
                });
                inner.append(&col);
                inner.append(&install);
                card.set_child(Some(&inner));
                rbox.append(&card);
            }
            if shown == 0 {
                status_c.set_text("No new plugins available.");
            }
        }
    };
    let rebuild_registry_rc = Rc::new(rebuild_registry);
    *rebuild_reg_slot.borrow_mut() = Some(rebuild_registry_rc.clone());

    // Poller for registry / downloads / emulator ops
    let pltx_poll = pltx.clone();
    {
        let status = registry_status.clone();
        let bar = registry_bar.clone();
        let reg_c = registry.clone();
        let rebuild_r = rebuild_registry_rc.clone();
        let rebuild_p = rebuild_plugins_rc.clone();
        let emu_list_c = emu_list.clone();
        let emu_status_c = emu_status.clone();
        let state_c = state.clone();
        let alive_r = dlg_alive.clone();
        glib::timeout_add_local(std::time::Duration::from_millis(100), move || {
            if !alive_r.get() {
                return glib::ControlFlow::Break;
            }
            let msgs: Vec<PluginTabMsg> = {
                let r = plrx.borrow();
                let mut v = Vec::new();
                while let Ok(m) = r.try_recv() {
                    v.push(m);
                }
                v
            };
            for msg in msgs {
                match msg {
                    PluginTabMsg::Registry(Ok(list)) => {
                        *reg_c.borrow_mut() = list;
                        status.set_text("");
                        rebuild_r();
                    }
                    PluginTabMsg::Registry(Err(e)) => {
                        status.set_text(&format!("Registry failed: {}", e));
                    }
                    PluginTabMsg::Progress(f, bps) => {
                        bar.set_fraction(f.clamp(0.0, 1.0));
                        bar.set_text(Some(&format!(
                            "{:.0}% · {}",
                            f * 100.0,
                            crate::backend::proton::ProtonManager::format_speed(bps)
                        )));
                    }
                    PluginTabMsg::PlugDownloaded(Ok(tag)) => {
                        status.set_text(&format!("Installed {}", tag));
                        bar.set_fraction(0.0);
                        bar.set_text(Some(""));
                        state_c.plugins.refresh();
                        rebuild_p();
                        // refresh registry view (installed now filtered out)
                        let tx2 = pltx_poll.clone();
                        std::thread::spawn(move || {
                            if let Ok(list) = crate::backend::plugins::PluginManager::fetch_registry() {
                                let _ = tx2.send(PluginTabMsg::Registry(Ok(list)));
                            }
                        });
                    }
                    PluginTabMsg::PlugDownloaded(Err(e)) => {
                        status.set_text(&format!("Failed: {}", e));
                    }
                    PluginTabMsg::Emus(list) => {
                        *emu_list_c.borrow_mut() = list;
                        rebuild_p();
                    }
                    PluginTabMsg::EmuOp(Ok(msg)) => {
                        emu_status_c.borrow_mut().clear();
                        status.set_text(&msg);
                        state_c.plugins.refresh();
                        rebuild_p();
                        let tx2 = pltx_poll.clone();
                        let dir = state_c.plugins.plugins_dir();
                        std::thread::spawn(move || {
                            let list = crate::backend::plugins::PluginManager::list_emulators_in(&dir);
                            let _ = tx2.send(PluginTabMsg::Emus(list));
                        });
                    }
                    PluginTabMsg::EmuOp(Err(e)) => {
                        emu_status_c.borrow_mut().clear();
                        status.set_text(&format!("Emulator op failed: {}", e));
                        rebuild_p();
                    }
                }
            }
            glib::ControlFlow::Continue
        });
    }

    // Initial load: installed + registry fetch
    rebuild_plugins_rc();
    {
        let tx2 = pltx.clone();
        let status_c = registry_status.clone();
        status_c.set_text("Loading registry...");
        std::thread::spawn(move || {
            let res = crate::backend::plugins::PluginManager::fetch_registry();
            let _ = tx2.send(PluginTabMsg::Registry(res));
        });
    }

    let plugins_scroll = gtk::ScrolledWindow::new();
    plugins_scroll.set_vexpand(true);
    plugins_scroll.set_hscrollbar_policy(gtk::PolicyType::Never);
    plugins_scroll.set_child(Some(&plugins_page));
    page_stack.add_titled(&plugins_scroll, Some("plugins"), "Plugins");

    // Integrations page (C++ parity: descriptions + Steam prefix import)
    let integrations_page = gtk::Box::new(gtk::Orientation::Vertical, 12);
    integrations_page.set_margin_top(12);
    integrations_page.set_margin_start(40);
    integrations_page.set_margin_end(40);
    let int_title = gtk::Label::new(Some("Integrations"));
    int_title.set_halign(gtk::Align::Center);
    int_title.add_css_class("settings-title");
    integrations_page.append(&int_title);
    let int_sub = gtk::Label::new(Some("Connect external platforms. Local scans need no API key."));
    int_sub.set_halign(gtk::Align::Start);
    int_sub.set_opacity(0.6);
    int_sub.set_wrap(true);
    int_sub.add_css_class("time-label");
    integrations_page.append(&int_sub);

    let int_status = gtk::Label::new(Some(""));
    int_status.set_halign(gtk::Align::Start);
    int_status.set_wrap(true);
    int_status.add_css_class("time-label");
    integrations_page.append(&int_status);

    for (source_name, desc) in [
        ("Steam", "Import installed Steam games with their existing Proton prefixes"),
        ("Lutris", "Largest Linux gaming platform – scan installed Lutris games"),
    ] {
        let card = gtk::Frame::new(None);
        card.add_css_class("page-card");
        let col = gtk::Box::new(gtk::Orientation::Vertical, 6);
        col.set_margin_top(6);
        col.set_margin_bottom(6);
        col.set_margin_start(10);
        col.set_margin_end(10);
        let row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        let lbl = gtk::Label::new(Some(source_name));
        lbl.set_halign(gtk::Align::Start);
        lbl.set_hexpand(true);
        lbl.add_css_class("details-title");
        let scan = gtk::Button::with_label("Scan & Import");
        scan.add_css_class("add-btn");
        let state_clone = state.clone();
        let source = source_name.to_string();
        let status_c = int_status.clone();
        let parent_c = parent.clone();
        let sb_c = sidebar.clone();
        let ch_c = center.clone();
        let det_c = details.clone();
        scan.connect_clicked(move |_| {
            status_c.set_text(&format!("Scanning {}...", source));
            let entries = if source == "Steam" {
                state_clone.integration.scan_steam()
            } else {
                state_clone.integration.scan_lutris()
            };
            let mut imported = 0;
            let mut fresh: Vec<(String, String)> = Vec::new();
            let mut art_jobs: Vec<(String, String)> = Vec::new();
            let source_c = source.clone();
            let entries_c = entries.len();
            for entry in &entries {
                if state_clone.game_model.get_game(&entry.name).is_some() {
                    continue;
                }
                // C++ parity: lutris runner → executor, playtime → timeSpent
                let executor = if source == "Lutris" {
                    crate::backend::integration::lutris_runner_to_executor(&entry.runner)
                } else {
                    String::new()
                };
                let executable = if !entry.executable.is_empty() {
                    entry.executable.clone()
                } else {
                    entry.path.clone()
                };
                let game_entry = crate::backend::game_model::GameEntry {
                    name: entry.name.clone(),
                    executable,
                    main_path: entry.path.clone(),
                    prefix_path: entry.prefix.clone(),
                    proton: entry.proton.clone(),
                    overrides: String::new(),
                    steam_id: entry.appid.clone(),
                    banner: String::new(),
                    icon: String::new(),
                    time_spent: (entry.playtime_hours * 3600.0).round() as u64,
                    last_played: 0,
                    favorite: false,
                    source: GameSource::from_str(&source),
                    executor,
                    emu_settings: std::collections::HashMap::new(),
                    lutris_runner: entry.runner.clone(),
                    environment: String::new(),
                    args_before: String::new(),
                    args_after: String::new(),
                    steam_overlay: false,
                    steam_runtime: false,
                    use_umu: false,
                    use_shared_prefix: false,
                    shared_prefix_name: String::new(),
                    wined3d: false,
                    native_wayland: false,
                    game_mode: false,
                    mango_hud: false,
                    lutris_slug: entry.slug.clone(),
                    fake_steam_id: String::new(),
                    install_size: String::new(),
                    heroic_store: String::new(),
                    heroic_app_id: String::new(),
                    heroic_eac: false,
                    heroic_battleye: false,
                };
                state_clone.game_model.import_external_game(game_entry);
                // Artwork resolves in the background below (network on the
                // main thread froze the whole launcher).
                fresh.push((entry.name.clone(), entry.path.clone()));
                art_jobs.push((
                    entry.name.clone(),
                    if source == "Steam" {
                        entry.appid.clone()
                    } else {
                        String::new()
                    },
                ));
                imported += 1;
            }
            // Background artwork (Steam CDN, Lutris, SGDB), applied back on
            // the main thread like the AddGame flow.
            if !art_jobs.is_empty() {
                let (atx, arx) = std::sync::mpsc::channel::<Vec<(String, String, String)>>();
                std::thread::spawn(move || {
                    let fresh_mgr =
                        crate::backend::integration::IntegrationManager::new();
                    let mut done = Vec::new();
                    for (gname, gid) in art_jobs {
                        let (i, b) = fresh_mgr.resolve_artwork(&gname, &gid);
                        done.push((
                            gname,
                            i.unwrap_or_default(),
                            b.unwrap_or_default(),
                        ));
                    }
                    let _ = atx.send(done);
                });
                let gm_c = state_clone.game_model.clone();
                let sb_c = sb_c.clone();
                let ch_c = ch_c.clone();
                let det_c = det_c.clone();
                let state_cc = state_clone.clone();
                let status_cc = status_c.clone();
                let total = imported;
                glib::idle_add_local(move || match arx.try_recv() {
                    Ok(done) => {
                        for (gname, icon, banner) in &done {
                            if !icon.is_empty() || !banner.is_empty() {
                                gm_c.set_artwork(gname, banner, icon);
                            }
                        }
                        if let Some(ref sb) = *sb_c.borrow() {
                            sb.apply_current_filter();
                        }
                        if let Some(ref ch) = *ch_c.borrow() {
                            ch.rebuild(&state_cc, &det_c);
                        }
                        status_cc.set_text(&format!(
                            "{}: found {}, imported {} new (artwork done)",
                            source_c, entries_c, total
                        ));
                        glib::ControlFlow::Break
                    }
                    Err(std::sync::mpsc::TryRecvError::Empty) => glib::ControlFlow::Continue,
                    Err(_) => glib::ControlFlow::Break,
                });
            }
            // C++ applyScanPlugins parity: dep + dll scans for the fresh
            // imports (single summary dialog if deps are missing).
            if !fresh.is_empty() {
                crate::run_plugin_scans_batch(&state_clone, &parent_c, fresh);
            }
            state_clone.recent_model.refresh(30);
            // The library views must show the imports at once (no manual
            // filter switching needed).
            if let Some(ref sb) = *sb_c.borrow() {
                sb.apply_current_filter();
            }
            if let Some(ref ch) = *ch_c.borrow() {
                ch.rebuild(&state_clone, &det_c);
            }
            status_c.set_text(&format!(
                "{}: found {}, imported {} new",
                source,
                entries.len(),
                imported
            ));
        });
        row.append(&lbl);
        row.append(&scan);
        col.append(&row);
        let desc_lbl = gtk::Label::new(Some(desc));
        desc_lbl.set_halign(gtk::Align::Start);
        desc_lbl.set_opacity(0.6);
        desc_lbl.set_wrap(true);
        desc_lbl.add_css_class("time-label");
        col.append(&desc_lbl);
        card.set_child(Some(&col));
        integrations_page.append(&card);
    }
    // Heroic (Epic/GOG installs managed by Heroic) — plugin scan, native entries.
    {
        let card = gtk::Frame::new(None);
        card.add_css_class("page-card");
        let col = gtk::Box::new(gtk::Orientation::Vertical, 6);
        col.set_margin_top(6);
        col.set_margin_bottom(6);
        col.set_margin_start(10);
        col.set_margin_end(10);
        let row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        let lbl = gtk::Label::new(Some("Heroic"));
        lbl.set_halign(gtk::Align::Start);
        lbl.set_hexpand(true);
        lbl.add_css_class("details-title");
        let scan = gtk::Button::with_label("Scan & Import");
        scan.add_css_class("add-btn");
        let state_clone = state.clone();
        let status_c = int_status.clone();
        let sb_c = sidebar.clone();
        let ch_c = center.clone();
        let det_c = details.clone();
        scan.connect_clicked(move |_| {
            status_c.set_text("Scanning Heroic…");
            let (tx, rx) = std::sync::mpsc::channel::<Result<serde_json::Value, String>>();
            std::thread::spawn(move || {
                let _ = tx.send(crate::backend::external::StoreManager::heroic_scan().map_err(|e| e.to_string()));
            });
            let state_c = state_clone.clone();
            let status_cc = status_c.clone();
            let sb_cc = sb_c.clone();
            let ch_cc = ch_c.clone();
            let det_cc = det_c.clone();
            glib::idle_add_local(move || match rx.try_recv() {
                Ok(Ok(doc)) => {
                    let games = doc.get("games").and_then(|x| x.as_array()).cloned().unwrap_or_default();
                    let mut imported = 0;
                    let def_proton = state_c.config.launcher_value("defaultProton").unwrap_or_default();
                    for g in &games {
                        let title = g.get("title").and_then(|x| x.as_str()).unwrap_or("").to_string();
                        if title.is_empty() || state_c.game_model.get_game(&title).is_some() {
                            continue;
                        }
                        let store = g.get("store").and_then(|x| x.as_str()).unwrap_or("epic").to_string();
                        let app_id = g.get("app_id").and_then(|x| x.as_str()).unwrap_or("").to_string();
                        let ipath = g.get("install_path").and_then(|x| x.as_str()).unwrap_or("").to_string();
                        if ipath.is_empty() || !std::path::Path::new(&ipath).exists() {
                            continue;
                        }
                        let exe = g.get("executable").and_then(|x| x.as_str()).unwrap_or("").to_string();
                        let executable = if exe.is_empty() {
                            ipath.clone()
                        } else if exe.starts_with('/') {
                            exe.clone()
                        } else {
                            format!("{}/{}", ipath.trim_end_matches('/'), exe)
                        };
                        let prefix = state_c.config.base_path_for("prefixes").join(&title).display().to_string();
                        let entry = crate::backend::game_model::GameEntry {
                            name: title.clone(),
                            executable,
                            main_path: ipath,
                            prefix_path: prefix,
                            proton: def_proton.clone(),
                            overrides: String::new(),
                            steam_id: String::new(),
                            banner: String::new(),
                            icon: String::new(),
                            time_spent: 0,
                            last_played: 0,
                            favorite: false,
                            source: GameSource::from_str(if store == "gog" { "GOG" } else { "Epic" }),
                            executor: String::new(),
                            emu_settings: std::collections::HashMap::new(),
                            lutris_runner: String::new(),
                            environment: String::new(),
                            args_before: String::new(),
                            args_after: String::new(),
                            steam_overlay: false,
                            steam_runtime: false,
                            use_umu: false,
                            use_shared_prefix: false,
                            shared_prefix_name: String::new(),
                            wined3d: false,
                            native_wayland: false,
                            game_mode: false,
                            mango_hud: false,
                            lutris_slug: String::new(),
                            fake_steam_id: String::new(),
                            install_size: String::new(),
                            heroic_store: store,
                            heroic_app_id: app_id,
                            heroic_eac: g.get("eac").and_then(|x| x.as_bool()).unwrap_or(false),
                            heroic_battleye: g.get("battleye").and_then(|x| x.as_bool()).unwrap_or(false),
                        };
                        state_c.game_model.add_game(entry);
                        imported += 1;
                    }
                    state_c.recent_model.refresh(30);
                    if let Some(ref sb) = *sb_cc.borrow() {
                        sb.apply_current_filter();
                    }
                    if let Some(ref ch) = *ch_cc.borrow() {
                        ch.rebuild(&state_c, &det_cc);
                    }
                    status_cc.set_text(&format!("Heroic: imported {} new", imported));
                    glib::ControlFlow::Break
                }
                Ok(Err(e)) => {
                    status_cc.set_text(&format!("Heroic scan failed: {}", e));
                    glib::ControlFlow::Break
                }
                Err(std::sync::mpsc::TryRecvError::Empty) => glib::ControlFlow::Continue,
                Err(_) => glib::ControlFlow::Break,
            });
        });
        row.append(&lbl);
        row.append(&scan);
        col.append(&row);
        let desc_lbl = gtk::Label::new(Some("Import Epic/GOG games installed by Heroic, with their anti-cheat runtime flags"));
        desc_lbl.set_halign(gtk::Align::Start);
        desc_lbl.set_opacity(0.6);
        desc_lbl.set_wrap(true);
        desc_lbl.add_css_class("time-label");
        col.append(&desc_lbl);
        card.set_child(Some(&col));
        integrations_page.append(&card);
    }
    // umu-launcher runner (Heroic-style Windows launches outside Steam).
    {
        let card = gtk::Frame::new(None);
        card.add_css_class("page-card");
        let col = gtk::Box::new(gtk::Orientation::Vertical, 6);
        col.set_margin_top(6);
        col.set_margin_bottom(6);
        col.set_margin_start(10);
        col.set_margin_end(10);
        let row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        let lbl = gtk::Label::new(Some("umu-launcher"));
        lbl.set_halign(gtk::Align::Start);
        lbl.set_hexpand(true);
        lbl.add_css_class("details-title");
        let umu_btn = gtk::Button::with_label("Install");
        umu_btn.add_css_class("add-btn");
        let umu_status_lbl = gtk::Label::new(Some("Checking…"));
        umu_status_lbl.set_halign(gtk::Align::Start);
        umu_status_lbl.set_hexpand(true);
        umu_status_lbl.set_opacity(0.6);
        umu_status_lbl.add_css_class("time-label");
        row.append(&lbl);
        row.append(&umu_btn);
        col.append(&row);
        col.append(&umu_status_lbl);
        let desc_lbl = gtk::Label::new(Some("Unified Linux Wine runner used for Heroic-style launches"));
        desc_lbl.set_halign(gtk::Align::Start);
        desc_lbl.set_opacity(0.6);
        desc_lbl.set_wrap(true);
        desc_lbl.add_css_class("time-label");
        col.append(&desc_lbl);
        card.set_child(Some(&col));
        integrations_page.append(&card);
        {
            let (tx, rx) = std::sync::mpsc::channel::<bool>();
            std::thread::spawn(move || {
                let _ = tx.send(crate::backend::external::StoreManager::umu_status().ok()
                    .and_then(|d| d.get("available").and_then(|x| x.as_bool()))
                    .unwrap_or(false));
            });
            let btn = umu_btn.clone();
            let lbl = umu_status_lbl.clone();
            glib::timeout_add_local(std::time::Duration::from_millis(100), move || match rx.try_recv() {
                Ok(avail) => {
                    lbl.set_text(if avail { "Installed." } else { "Not installed." });
                    btn.set_label(if avail { "Reinstall" } else { "Install" });
                    glib::ControlFlow::Break
                }
                Err(std::sync::mpsc::TryRecvError::Empty) => glib::ControlFlow::Continue,
                Err(_) => glib::ControlFlow::Break,
            });
        }
        {
            umu_btn.connect_clicked(move |b| {
                b.set_sensitive(false);
                let lbl = umu_status_lbl.clone();
                lbl.set_text("Installing umu-launcher…");
                let rx = crate::backend::external::StoreManager::spawn_umu_setup();
                let btn = b.clone();
                crate::backend::plugin_process::pump_to_idle(rx, move |ev| {
                    match ev {
                        crate::backend::plugin_process::PluginEvent::Progress { stage, .. } => {
                            lbl.set_text(&stage.unwrap_or_default());
                            true
                        }
                        crate::backend::plugin_process::PluginEvent::Done(val) => {
                            let p = val.get("path").and_then(|x| x.as_str()).unwrap_or("");
                            lbl.set_text(&format!("Installed: {}", p));
                            btn.set_label("Reinstall");
                            btn.set_sensitive(true);
                            false
                        }
                        crate::backend::plugin_process::PluginEvent::Error { message, .. } => {
                            lbl.set_text(&format!("Failed: {}", message));
                            btn.set_sensitive(true);
                            false
                        }
                        _ => true,
                    }
                });
            });
        }
    }
    page_stack.add_titled(&integrations_page, Some("integrations"), "Integrations");

    // About page
    let about_page = gtk::Box::new(gtk::Orientation::Vertical, 12);
    about_page.set_margin_top(40);
    about_page.set_halign(gtk::Align::Center);
    let about_logo = gtk::Image::new();
    about_logo.set_pixel_size(120);
    let logo_path = helpers::asset_path("corkytux");
    if let Some(tex) = helpers::load_texture(&logo_path) { about_logo.set_paintable(Some(&tex)); }
    about_page.append(&about_logo);
    let about_name = gtk::Label::new(Some("CorkyTux"));
    about_name.add_css_class("details-title");
    about_page.append(&about_name);
    let about_ver = gtk::Label::new(Some("v3.0.9"));
    about_ver.set_opacity(0.6);
    about_page.append(&about_ver);
    let about_author = gtk::Label::new(Some("by Matts-lab69"));
    about_author.set_opacity(0.5);
    about_page.append(&about_author);

    let github_btn = gtk::Button::new();
    let gh_box = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    gh_box.set_halign(gtk::Align::Center);
    gh_box.append(&gtk::Image::from_icon_name("web-browser-symbolic"));
    let gh_label = gtk::Label::new(Some("GitHub"));
    gh_label.set_halign(gtk::Align::Center);
    gh_box.append(&gh_label);
    github_btn.set_child(Some(&gh_box));
    github_btn.add_css_class("settings-btn");
    github_btn.set_margin_top(12);
    github_btn.connect_clicked(|_| {
        let _ = std::process::Command::new("xdg-open")
            .arg("https://github.com/Matts-lab69/corkytux").spawn();
    });
    about_page.append(&github_btn);

    page_stack.add_titled(&about_page, Some("about"), "About");

    outer.append(&page_stack);

    // Tab bar with text labels + active indicator
    let tab_bar_frame = gtk::Frame::new(None);
    tab_bar_frame.add_css_class("settings-tab-bar");
    tab_bar_frame.set_halign(gtk::Align::Fill);
    tab_bar_frame.set_margin_start(8);
    tab_bar_frame.set_margin_end(8);

    let tab_bar = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    tab_bar.set_halign(gtk::Align::Fill);
    tab_bar.set_valign(gtk::Align::Center);

    let tabs = [
        ("palette", "Visuals"), ("fileview", "Paths"), ("proton17", "Protons"),
        ("settings", "Misc"), ("plugins", "Plugins"), ("openIn", "Integrations"), ("about", "About"),
    ];
    let tab_ids = ["visuals", "paths", "protons", "misc", "plugins", "integrations", "about"];
    let mut tab_buttons = Vec::new();
    let mut tab_indicators: Vec<gtk::Box> = Vec::new();

    for (i, ((icon_name, label), id)) in tabs.iter().zip(tab_ids.iter()).enumerate() {
        let btn_box = gtk::Box::new(gtk::Orientation::Vertical, 1);
        btn_box.set_halign(gtk::Align::Fill);
        btn_box.set_hexpand(true);

        let btn = gtk::ToggleButton::new();
        btn.add_css_class("settings-tab");
        btn.set_halign(gtk::Align::Fill);
        btn.set_hexpand(true);

        let col = gtk::Box::new(gtk::Orientation::Vertical, 2);
        col.set_halign(gtk::Align::Center);
        let icon = make_settings_tab_icon(icon_name, &state.theme);
        let lbl = gtk::Label::new(Some(label));
        lbl.add_css_class("time-label");
        col.append(&icon);
        col.append(&lbl);

        let indicator = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        indicator.add_css_class("settings-tab-indicator");
        indicator.set_visible(*id == "visuals");
        col.append(&indicator);

        btn.set_child(Some(&col));

        if *id == "visuals" { btn.set_active(true); }

        btn_box.append(&btn);
        if i == 0 { btn_box.set_margin_start(2); }
        if i == tabs.len() - 1 { btn_box.set_margin_end(2); }
        btn_box.set_margin_top(1);
        btn_box.set_margin_bottom(1);

        tab_bar.append(&btn_box);
        tab_buttons.push(btn);
        tab_indicators.push(indicator);
    }

    for btn in &tab_buttons[1..] { btn.set_group(Some(&tab_buttons[0])); }

    // Segundo pase: ocultar el indicador de cualquier otra pestaña
    // (antes o después), no solo las ya construidas.
    for (i, (_, id)) in tabs.iter().zip(tab_ids.iter()).enumerate() {
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

    tab_bar_frame.set_child(Some(&tab_bar));
    outer.append(&tab_bar_frame);

    dialog.set_child(Some(&outer));
    dialog.present(Some(parent));
}

enum ProtonTabMsg {
    Releases(Result<Vec<(String, String, String)>, String>),
    Progress(f64, f64),
    Downloaded(Result<String, String>),
}

enum PluginTabMsg {
    Registry(Result<Vec<crate::backend::plugins::RegistryEntry>, String>),
    Progress(f64, f64),
    PlugDownloaded(Result<String, String>),
    Emus(Vec<crate::backend::plugins::EmuInfo>),
    EmuOp(Result<String, String>),
}

fn rebuild_emu_rows(
    emu_box: &gtk::Box,
    state: &AppState,
    emu_list: &Rc<RefCell<Vec<crate::backend::plugins::EmuInfo>>>,
    emu_status: &Rc<RefCell<std::collections::HashMap<String, String>>>,
    tx: &std::sync::mpsc::Sender<PluginTabMsg>,
) {
    while let Some(child) = emu_box.first_child() {
        emu_box.remove(&child);
    }
    let list = emu_list.borrow().clone();
    if list.is_empty() {
        let hint = gtk::Label::new(Some("No emulators reported by emulator-manager."));
        hint.set_opacity(0.6);
        hint.add_css_class("time-label");
        emu_box.append(&hint);
        return;
    }
    for emu in &list {
        let row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        row.set_margin_top(2);
        row.set_margin_bottom(2);
        let dot = gtk::Label::new(Some("●"));
        if emu.installed {
            dot.add_css_class("emu-dot-on");
        } else {
            dot.add_css_class("emu-dot-off");
        }
        row.append(&dot);
        let name_lbl = gtk::Label::new(Some(&format!(
            "{}{} — {}",
            emu.name,
            if emu.native { " (native)" } else { "" },
            emu.description
        )));
        name_lbl.set_halign(gtk::Align::Start);
        name_lbl.set_hexpand(true);
        name_lbl.set_ellipsize(gtk::pango::EllipsizeMode::End);
        row.append(&name_lbl);
        if let Some(s) = emu_status.borrow().get(&emu.name) {
            let st = gtk::Label::new(Some(s));
            st.add_css_class("time-label");
            st.set_width_chars(12);
            row.append(&st);
        }
        let btn = gtk::Button::with_label(if emu.native {
            "Linked"
        } else if emu.installed {
            "Remove"
        } else {
            "Install"
        });
        if !emu.native && !emu.installed {
            btn.add_css_class("add-btn");
        }
        btn.set_sensitive(!emu.native);
        btn.set_width_request(80);
        let name_c = emu.name.clone();
        let installed_c = emu.installed;
        let state_c = state.clone();
        let status_c = emu_status.clone();
        let tx_c = tx.clone();
        btn.connect_clicked(move |_| {
            status_c.borrow_mut().insert(name_c.clone(), "Working…".to_string());
            let tx2 = tx_c.clone();
            let dir = state_c.plugins.plugins_dir();
            let name2 = name_c.clone();
            std::thread::spawn(move || {
                let res = if installed_c {
                    crate::backend::plugins::PluginManager::remove_emulator_in(&dir, &name2)
                } else {
                    crate::backend::plugins::PluginManager::install_emulator_in(&dir, &name2)
                };
                let _ = tx2.send(PluginTabMsg::EmuOp(res));
            });
        });
        row.append(&btn);
        emu_box.append(&row);
    }
}

fn start_proton_download(
    tx: &std::sync::mpsc::Sender<ProtonTabMsg>,
    tag: &str,
    url: &str,
    dest: &std::path::PathBuf,
) {
    let tx2 = tx.clone();
    let tag2 = tag.to_string();
    let url2 = url.to_string();
    let dest2 = dest.clone();
    std::thread::spawn(move || {
        let res = crate::backend::proton::ProtonManager::download_proton(
            &tag2,
            &url2,
            &dest2,
            |f, bps| {
                let _ = tx2.send(ProtonTabMsg::Progress(f, bps));
            },
        );
        let _ = tx2.send(ProtonTabMsg::Downloaded(res.map(|_| tag2).map_err(|e| e)));
    });
}

fn refresh_shared_list(list_box: &gtk::Box, state: &AppState) {
    while let Some(child) = list_box.first_child() {
        list_box.remove(&child);
    }
    let names = state.config.shared_prefixes();
    if names.is_empty() {
        let empty = gtk::Label::new(Some("No shared prefixes created yet."));
        empty.set_opacity(0.6);
        empty.add_css_class("time-label");
        list_box.append(&empty);
        return;
    }
    for name in &names {
        let card = gtk::Frame::new(None);
        card.add_css_class("page-card");
        let inner = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        inner.set_margin_top(4);
        inner.set_margin_bottom(4);
        inner.set_margin_start(8);
        inner.set_margin_end(8);
        let col = gtk::Box::new(gtk::Orientation::Vertical, 2);
        col.set_hexpand(true);
        let name_lbl = gtk::Label::new(Some(name));
        name_lbl.set_halign(gtk::Align::Start);
        name_lbl.add_css_class("info-label");
        let path_lbl = gtk::Label::new(Some(
            &state.config.shared_prefix_path(name)
                .map(|p| p.display().to_string())
                .unwrap_or_default(),
        ));
        path_lbl.set_halign(gtk::Align::Start);
        path_lbl.set_opacity(0.6);
        path_lbl.set_ellipsize(gtk::pango::EllipsizeMode::Middle);
        path_lbl.add_css_class("time-label");
        col.append(&name_lbl);
        col.append(&path_lbl);
        let remove_btn = gtk::Button::with_label("Remove");
        remove_btn.add_css_class("destructive-action");
        remove_btn.set_valign(gtk::Align::Center);
        let state_c = state.clone();
        let name_c = name.clone();
        let list_c = list_box.clone();
        remove_btn.connect_clicked(move |_| {
            state_c.config.remove_shared_prefix(&name_c);
            refresh_shared_list(&list_c, &state_c);
        });
        inner.append(&col);
        inner.append(&remove_btn);
        card.set_child(Some(&inner));
        list_box.append(&card);
    }
}
