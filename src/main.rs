mod backend;
mod ui;

use std::cell::RefCell;
use std::rc::Rc;

use adw::prelude::*;
use gtk::{self, glib};

use backend::config::ConfigManager;
use backend::game_model::{GameModel, FilterMode};
use backend::integration::IntegrationManager;
use backend::plugins::PluginManager;
use backend::proton::ProtonManager;
use backend::game_model::RecentModel;
use backend::theme::ThemeManager;
use ui::details_panel::DetailsPanel;
use ui::helpers;
use ui::sidebar::{Sidebar, GameCallback, FilterCallback};
use ui::center::CenterHandle;
use ui::log_modal::LogModal;
use ui::prefix_warning::PrefixWarningModal;
use ui::proton_modal::ProtonModal;

fn url_encode(s: &str) -> String {
    let mut out = String::new();
    for b in s.bytes() {
        if b.is_ascii_alphanumeric() || b"-_.~".contains(&b) {
            out.push(b as char);
        } else if b == b' ' {
            out.push('+');
        } else {
            out.push_str(&format!("%{:02X}", b));
        }
    }
    out
}

fn tilde(s: &str) -> String {
    if s.starts_with("~/") || s == "~" {
        if let Ok(home) = std::env::var("HOME") {
            return s.replacen("~", &home, 1);
        }
    }
    s.to_string()
}

#[derive(Clone)]
pub struct AppState {
    pub config: ConfigManager,
    pub theme: ThemeManager,
    pub game_model: GameModel,
    pub recent_model: RecentModel,
    pub proton: ProtonManager,
    pub plugins: PluginManager,
    pub integration: IntegrationManager,
    pub selected_game: Rc<RefCell<String>>,
}

impl AppState {
    fn new() -> Self {
        let config = ConfigManager::new();
        let theme = ThemeManager::new();
        let game_model = GameModel::new();
        let recent_model = RecentModel::new();
        let proton = ProtonManager::new();
        let plugins = PluginManager::new();
        let integration = IntegrationManager::new();
        recent_model.refresh(30);
        proton.detect_running_sessions();

        Self {
            config, theme, game_model, recent_model,
            proton, plugins, integration,
            selected_game: Rc::new(RefCell::new(String::new())),
        }
    }
}

fn build_details_panel(state: &AppState, window: &adw::ApplicationWindow) -> DetailsPanel {
    let state_clone = state.clone();
    let state_clone2 = state.clone();
    let win_clone = window.clone();
    DetailsPanel::new(&state.theme, move |name| {
        // QML parity: Play launches, Stop (same button) banks session time.
        // Returns Err so the panel can show launch failures (used to be
        // silent `let _ = ...`, leaving users with a dead button).
        if state_clone.proton.is_game_running()
            && state_clone.proton.session_game_name() == name
        {
            return match state_clone.proton.stop_game() {
                Ok((game, secs)) => {
                    if !game.is_empty() {
                        if secs > 0 {
                            state_clone.game_model.increment_time_spent(&game, secs);
                        }
                        state_clone.game_model.touch_last_played(&game);
                        state_clone.recent_model.refresh(30);
                    }
                    Ok(())
                }
                Err(e) => Err(e),
            };
        }
        state_clone.game_model.touch_last_played(&name);
        state_clone.recent_model.refresh(30);
        if state_clone.game_model.get_game(&name).is_some() {
            match state_clone.proton.run_game(&name) {
                Ok(()) => Ok(()),
                Err(e) => {
                    // Rescan first: the list may be stale (deleted externally).
                    state_clone.proton.refresh_installed();
                    if state_clone.proton.installed_protons().is_empty() {
                        // Ask before downloading: No (neon red) / Yes (neon green).
                        let cdlg = adw::Dialog::new();
                        cdlg.set_title("Proton missing");
                        cdlg.set_content_width(420);
                        cdlg.set_content_height(210);
                        let outer = gtk::Box::new(gtk::Orientation::Vertical, 0);
                        outer.add_css_class("modal-bg");
                        outer.set_hexpand(true);
                        outer.set_vexpand(true);
                        let inner = gtk::Box::new(gtk::Orientation::Vertical, 10);
                        inner.set_hexpand(true);
                        inner.set_vexpand(true);
                        inner.set_margin_top(12);
                        inner.set_margin_bottom(12);
                        inner.set_margin_start(16);
                        inner.set_margin_end(16);
                        outer.append(&inner);
                        let (header_row, x_btn) = helpers::modal_header("Proton missing");
                        {
                            let d0 = cdlg.clone();
                            x_btn.connect_clicked(move |_| { d0.close(); });
                        }
                        inner.append(&header_row);
                        let msg = gtk::Label::new(Some(
                            "The previous Proton was deleted. Continue with the automatic install of the latest Proton-GE?",
                        ));
                        msg.set_halign(gtk::Align::Start);
                        msg.set_wrap(true);
                        inner.append(&msg);
                        let row = gtk::Box::new(gtk::Orientation::Horizontal, 10);
                        let no_btn = gtk::Button::with_label("No");
                        no_btn.add_css_class("neon-red");
                        no_btn.set_hexpand(true);
                        let yes_btn = gtk::Button::with_label("Yes");
                        yes_btn.add_css_class("neon-green");
                        yes_btn.set_hexpand(true);
                        {
                            let d0 = cdlg.clone();
                            no_btn.connect_clicked(move |_| { d0.close(); });
                        }
                        {
                            let d0 = cdlg.clone();
                            let state_cc = state_clone.clone();
                            let win_cc = win_clone.clone();
                            yes_btn.connect_clicked(move |_| {
                                d0.close();
                                maybe_autoinstall_proton(&state_cc, &win_cc);
                            });
                        }
                        row.append(&no_btn);
                        row.append(&yes_btn);
                        inner.append(&row);
                        cdlg.set_child(Some(&outer));
                        cdlg.present(Some(&win_clone));
                        return Ok(());
                    }
                    Err(e)
                }
            }
        } else {
            Err("Game not found".into())
        }
    }, move |msg| {
        if let Some(name) = msg.strip_prefix("fav_on:") {
            state_clone2.game_model.set_favorite(name, true);
        } else if let Some(name) = msg.strip_prefix("fav_off:") {
            state_clone2.game_model.set_favorite(name, false);
        }
    })
}

fn build_window(app: &adw::Application) -> adw::ApplicationWindow {
    let state = AppState::new();
    helpers::apply_theme_css(&state.theme);
    helpers::init_accent_provider(&state.theme);

    // Restored size lets Cinnamon/Muffin tile freely; small minimum
    // so corner/side snapping is never blocked by our min size.
    let saved_w: i32 = state.config.launcher_value("CkWinW").and_then(|s| s.parse().ok()).unwrap_or(1200).clamp(640, 10000);
    let saved_h: i32 = state.config.launcher_value("CkWinH").and_then(|s| s.parse().ok()).unwrap_or(700).clamp(400, 10000);
    let window = adw::ApplicationWindow::builder()
        .application(app).title("CorkyTux")
        .default_width(saved_w).default_height(saved_h)
        .build();
    window.set_size_request(640, 400);
    {
        let state_c = state.clone();
        window.connect_close_request(move |win| {
            let a = win.allocation();
            state_c.config.set_launcher_value("CkWinW", &a.width().to_string());
            state_c.config.set_launcher_value("CkWinH", &a.height().to_string());
            glib::Propagation::Proceed
        });
    }

    let main_box = gtk::Box::new(gtk::Orientation::Vertical, 0);

    // Header bar with native window controls (buttons added after sidebar/center built)
    let header = adw::HeaderBar::new();
    let title_box = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    title_box.set_halign(gtk::Align::Center);
    let corky_icon = helpers::themed_image("corkytux", state.theme.is_dark(), 20);
    title_box.append(&corky_icon);
    title_box.append(&gtk::Label::new(Some("CorkyTux")));
    header.set_title_widget(Some(&title_box));
    main_box.append(&header);

    let body = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    body.set_vexpand(true);
    body.set_hexpand(true);
    body.set_halign(gtk::Align::Fill);

    let details = build_details_panel(&state, &window);
    let details_ref: Rc<RefCell<Option<DetailsPanel>>> = Rc::new(RefCell::new(Some(details.clone())));

    // Callback types
    let game_cb: GameCallback = Rc::new(RefCell::new(None));
    let filter_cb: FilterCallback = Rc::new(RefCell::new(None));

    // Build center
    let center_handle = Rc::new(RefCell::new(None::<CenterHandle>));
    let center_handle_for_filter = center_handle.clone();

    // Filter callback
    let sidebar_ref_for_filter: Rc<RefCell<Option<Rc<RefCell<Option<Sidebar>>>>>> =
        Rc::new(RefCell::new(None));
    {
        let state_c = state.clone();
        let details_c = details_ref.clone();
        let center_h = center_handle_for_filter;
        let sb_ref = sidebar_ref_for_filter.clone();
        *filter_cb.borrow_mut() = Some(Box::new(move |mode: String, search: String| {
            let filter_mode = match mode.as_str() {
                "favorites" => FilterMode::Favorites,
                "az" => FilterMode::Az,
                "most_played" => FilterMode::MostPlayed,
                "recent" => FilterMode::Recent,
                _ => FilterMode::All,
            };
            let games = state_c.game_model.filtered_games(filter_mode, &search);
            let names: Vec<String> = games.iter().map(|g| g.name.clone()).collect();
            if let Some(ref sb_inner) = *sb_ref.borrow() {
                if let Some(ref sidebar) = *sb_inner.borrow() {
                    sidebar.refresh_list(&names);
                }
            }
            if let Some(ref ch) = *center_h.borrow() {
                ch.rebuild(&state_c, &details_c);
            }
        }));
    }

    // Game click callback
    {
        let state_c = state.clone();
        let det = details_ref.clone();
        let center_h_for_art = center_handle.clone();
        let art_attempted: Rc<RefCell<std::collections::HashSet<String>>> =
            Rc::new(RefCell::new(std::collections::HashSet::new()));
        *game_cb.borrow_mut() = Some(Box::new(move |name: String| {
            *state_c.selected_game.borrow_mut() = name.clone();
            if let Some(ref d) = *det.borrow() {
                d.set_game(&name, &state_c.config);
            }
            // C++ artwork parity: games with no banner/icon resolve artwork
            // in the background when selected (Steam CDN/Lutris/SGDB).
            // AppImages use embedded .DirIcon extraction only, never online.
            let (needs, needs_appimage, needs_rpg) = match state_c.game_model.get_game(&name) {
                Some(g) => {
                    let app = g.source == crate::backend::game_model::GameSource::AppImage
                        || g.executor == "appimage-launcher";
                    let rpg = g.source == crate::backend::game_model::GameSource::RpgMaker
                        || g.executor == "rpgmaker-runtime";
                    (g.banner.is_empty() || g.icon.is_empty(), app, rpg)
                }
                None => (false, false, false),
            };
            if needs && !art_attempted.borrow().contains(&name) {
                art_attempted.borrow_mut().insert(name.clone());
                let art_name = name.clone();
                let gm_c = state_c.game_model.clone();
                let det_c = det.clone();
                let center_c = center_h_for_art.clone();
                let (art_tx, art_rx) =
                    std::sync::mpsc::channel::<(Option<String>, Option<String>)>();
                let send_name = art_name.clone();
                let send_exe = state_c.game_model.get_game(&art_name)
                    .map(|g| g.executable).unwrap_or_default();
                std::thread::spawn(move || {
                    if needs_appimage {
                        let fresh = IntegrationManager::new();
                        let i = fresh.extract_appimage_icon(&send_exe, &send_name);
                        let _ = art_tx.send((i, None));
                    } else if needs_rpg {
                        let fresh = IntegrationManager::new();
                        let i = fresh.extract_rpg_icon(&send_exe, &send_name);
                        let (_, b) = fresh.resolve_artwork(&send_name, "");
                        let _ = art_tx.send((i, b));
                    } else {
                        let fresh = IntegrationManager::new();
                        let (i, b) = fresh.resolve_artwork(&send_name, "");
                        let _ = art_tx.send((i, b));
                    }
                });
                let art_name3 = art_name.clone();
                let state_inner = state_c.clone();
                glib::idle_add_local(move || match art_rx.try_recv() {
                    Ok((i, b)) => {
                        let cur_icon = gm_c.get_game(&art_name3) .map(|g| g.icon).unwrap_or_default();
                        let icon = if cur_icon.contains("-exe.png")
                            || cur_icon.contains("-appimage.")
                            || cur_icon.contains("-rpg.png")
                        {
                            String::new()
                        } else {
                            i.unwrap_or_default()
                        };
                        let banner = b.unwrap_or_default();
                        if !icon.is_empty() || !banner.is_empty() {
                            gm_c.set_artwork(&art_name3, &banner, &icon);
                            if let Some(ref d2) = *det_c.borrow() {
                                d2.set_game(&art_name3, &state_inner.config);
                            }
                            if let Some(ref ch) = *center_c.borrow() {
                                ch.rebuild(&state_inner, &det_c);
                            }
                        }
                        glib::ControlFlow::Break
                    }
                    Err(std::sync::mpsc::TryRecvError::Empty) => glib::ControlFlow::Continue,
                    Err(_) => glib::ControlFlow::Break,
                });
            }
        }));
    }

    // Build sidebar
    let page_cb: ui::sidebar::PageCallback = Rc::new(RefCell::new(None));
    let sidebar = Sidebar::new_with_callbacks(&state.config, &state.theme, &game_cb, &filter_cb, &page_cb);
    // Gate Minecraft/Stores entry buttons on installed plugins.
    sidebar.set_plugin_manager(&state.plugins);
    let sidebar_ref: Rc<RefCell<Option<Sidebar>>> = Rc::new(RefCell::new(Some(sidebar.clone())));
    *sidebar_ref_for_filter.borrow_mut() = Some(sidebar_ref.clone());

    sidebar.widget.set_hexpand(false);
    sidebar.widget.set_halign(gtk::Align::Start);
    body.append(&sidebar.widget);

    // Build center
    let center = ui::center::build_center(&state, &details_ref);
    center_handle.borrow_mut().replace(center);

    // Games / Minecraft / Stores pages inside the launcher (sidebar buttons).
    let mc_view = ui::minecraft_view::MinecraftView::new(&state, &window);
    let stores_view = ui::stores_view::StoresView::new(
        &state,
        &window,
        &sidebar_ref,
        &center_handle,
        &details_ref,
    );
    let content_stack = gtk::Stack::new();
    content_stack.set_hexpand(true);
    content_stack.set_vexpand(true);
    content_stack.add_named(&center_handle.borrow().as_ref().unwrap().widget, Some("games"));
    content_stack.add_named(&mc_view.widget, Some("minecraft"));
    content_stack.add_named(&stores_view.widget, Some("stores"));
    content_stack.set_visible_child_name("games");
    {
        let stack_c = content_stack.clone();
        let det_c = details_ref.clone();
        *page_cb.borrow_mut() = Some(Box::new(move |page: String| {
            if page == "minecraft" || page == "stores" {
                if let Some(ref d) = *det_c.borrow() {
                    d.hide();
                }
            }
            stack_c.set_visible_child_name(&page);
        }));
    }

    // QML parity: details floats OVER the content (overlay), so opening
    // it never compresses RECENTLY PLAYED
    let content_overlay = gtk::Overlay::new();
    content_overlay.set_hexpand(true);
    content_overlay.set_vexpand(true);
    content_overlay.set_child(Some(&content_stack));
    details.revealer.set_halign(gtk::Align::End);
    details.revealer.set_valign(gtk::Align::Fill);
    content_overlay.add_overlay(&details.revealer);
    body.append(&content_overlay);

    // Add game button in header (left side)
    let add_btn = gtk::Button::with_label("+ Add game");
    add_btn.add_css_class("add-btn");
    let state_clone = state.clone();
    let win_clone = window.clone();
    let sb_clone = sidebar_ref.clone();
    let ch_clone = center_handle.clone();
    let det_clone = details_ref.clone();
    add_btn.connect_clicked(move |_| {
        ui::add_game::show_add_game_modal(&state_clone, &win_clone, &sb_clone, &ch_clone, &det_clone);
    });
    header.pack_start(&add_btn);

    // Settings button in header (right side) with label
    let settings_btn = gtk::Button::new();
    let settings_box = gtk::Box::new(gtk::Orientation::Horizontal, 4);
    settings_box.set_halign(gtk::Align::Center);
    let settings_icon = helpers::themed_image("settings-hires", state.theme.is_dark(), 16);
    settings_box.append(&settings_icon);
    settings_box.append(&gtk::Label::new(Some("Settings")));
    settings_btn.set_child(Some(&settings_box));
    settings_btn.add_css_class("settings-btn");
    let state_clone2 = state.clone();
    let win_clone2 = window.clone();
    let sb_clone2 = sidebar_ref.clone();
    let ch_clone2 = center_handle.clone();
    let det_clone2 = details_ref.clone();
    settings_btn.connect_clicked(move |_| {
        ui::settings::show_settings_modal(&state_clone2, &win_clone2, &sb_clone2, &ch_clone2, &det_clone2);
    });
    header.pack_end(&settings_btn);

    // === Connect all 9 action buttons ===
    {
        let state_c = state.clone();
        let win = window.clone();
        let sb = sidebar_ref.clone();
        let ch = center_handle.clone();
        let det = details_ref.clone();
        if let Some(btn) = details.get_action_button("Remove") {
            btn.connect_clicked(move |_| {
                let s = state_c.clone();
                let w = win.clone();
                let sb2 = sb.clone();
                let ch2 = ch.clone();
                let det2 = det.clone();
                // Need to borrow selected_game to pass to show_remove_modal
                ui::remove_modal::show_remove_modal(&s, &w, &sb2, &ch2, &det2);
            });
        }
    }
    {
        let state_c = state.clone();
        let win = window.clone();
        if let Some(btn) = details.get_action_button("Debug") {
            btn.connect_clicked(move |_| {
                let name = state_c.selected_game.borrow().clone();
                if name.is_empty() { return; }
                // Debug never launches. Live tail while running, snapshot
                // once stopped (a frozen shot is why logs "ended at fsync").
                let log_modal = LogModal::new(&state_c.theme, &name, &win);
                let header = state_c.proton.debug_header(&name);
                if state_c.proton.is_game_running()
                    && state_c.proton.session_game_name() == name
                {
                    log_modal.set_running(true);
                    log_modal.tail_game_log(&name, &state_c.proton, &header);
                } else {
                    log_modal.set_running(false);
                    let text = crate::backend::proton::ProtonManager::read_log(&name);
                    if text.trim().is_empty() {
                        log_modal.set_log("No logs recorded yet. Launch the game first.");
                    } else {
                        log_modal.set_log(&format!("{}\n{}", header, text));
                    }
                }
                log_modal.present(&win);
            });
        }
    }
    {
        let state_c = state.clone();
        let win = window.clone();
        if let Some(btn) = details.get_action_button("Wine") {
            btn.connect_clicked(move |b| {
                let name = state_c.selected_game.borrow().clone();
                if name.is_empty() { return; }
                let popover = gtk::Popover::new();
                let vbox = gtk::Box::new(gtk::Orientation::Vertical, 2);
                let tools = [
                    ("winecfg", "Wine Configuration"),
                    ("taskmgr", "Task Manager"),
                    ("control", "Control Panel"),
                    ("explorer", "File Explorer"),
                    ("cmd", "Command Prompt"),
                ];
                for (cmd, label) in &tools {
                    let item_btn = gtk::Button::with_label(label);
                    item_btn.set_has_frame(false);
                    item_btn.set_halign(gtk::Align::Fill);
                    let state_cc = state_c.clone();
                    let tool = cmd.to_string();
                    let game = name.clone();
                    let pop = popover.clone();
                    item_btn.connect_clicked(move |_| {
                        pop.popdown();
                        match state_cc.proton.run_wine_tool(&game, &tool) {
                            Ok(_) => {}
                            Err(e) => eprintln!("Wine tool failed: {}", e),
                        }
                    });
                    vbox.append(&item_btn);
                }
                popover.set_child(Some(&vbox));
                popover.set_parent(b);
                popover.popup();
            });
        }
    }
    {
        let state_c = state.clone();
        let win = window.clone();
        if let Some(btn) = details.get_action_button("Run .exe") {
            btn.connect_clicked(move |_| {
                let name = state_c.selected_game.borrow().clone();
                if name.is_empty() { return; }
                let dialog = adw::Dialog::new();
                dialog.set_title("Run .exe from game directory");
                dialog.set_content_width(420);
                dialog.set_content_height(360);
                let content = gtk::Box::new(gtk::Orientation::Vertical, 0);
                content.add_css_class("modal-bg");
                content.set_hexpand(true);
                content.set_vexpand(true);
                let inner = gtk::Box::new(gtk::Orientation::Vertical, 8);
                inner.set_hexpand(true);
                inner.set_vexpand(true);
                inner.set_margin_top(12);
                inner.set_margin_bottom(12);
                inner.set_margin_start(16);
                inner.set_margin_end(16);
                content.append(&inner);
                let (exe_header, exe_x) = ui::helpers::modal_header("Run .exe from game directory");
                {
                    let dlg = dialog.clone();
                    exe_x.connect_clicked(move |_| { dlg.close(); });
                }
                inner.append(&exe_header);
                let sub = gtk::Label::new(Some("Select an executable from the game directory"));
                sub.set_halign(gtk::Align::Start);
                sub.set_opacity(0.6);
                sub.add_css_class("time-label");
                sub.set_wrap(true);
                inner.append(&sub);
                // C++ parity: hover rows with bold filenames, click runs at once
                let has_dir = state_c.config.game_value(&name, "MainPath")
                    .map(|p| !p.trim().is_empty())
                    .unwrap_or(false);
                let exes = if has_dir {
                    state_c.proton.find_exe_in_game_dir(&name)
                } else {
                    Vec::new()
                };
                if exes.is_empty() {
                    let empty = gtk::Label::new(Some(if has_dir {
                        "No executables found in game directory"
                    } else {
                        "No game directory"
                    }));
                    empty.set_halign(gtk::Align::Center);
                    empty.set_opacity(0.6);
                    empty.add_css_class("time-label");
                    inner.append(&empty);
                }
                for exe in &exes {
                    let row_btn = gtk::Button::new();
                    row_btn.add_css_class("filter-btn");
                    row_btn.set_halign(gtk::Align::Fill);
                    let row_lbl = gtk::Label::new(Some(
                        std::path::Path::new(exe)
                            .file_name()
                            .map(|n| n.to_string_lossy().to_string())
                            .unwrap_or_else(|| exe.clone())
                            .as_str(),
                    ));
                    row_lbl.set_halign(gtk::Align::Start);
                    row_lbl.set_ellipsize(gtk::pango::EllipsizeMode::Middle);
                    row_lbl.add_css_class("details-title");
                    row_btn.set_child(Some(&row_lbl));
                    let state_cc = state_c.clone();
                    let game_c = name.clone();
                    let exe_c = exe.clone();
                    let dlg_c = dialog.clone();
                    row_btn.connect_clicked(move |_| {
                        dlg_c.close();
                        match state_cc.proton.run_custom_exe(&game_c, &exe_c) {
                            Ok(_) => {}
                            Err(e) => eprintln!("Run .exe failed: {}", e),
                        }
                    });
                    inner.append(&row_btn);
                }
                dialog.set_child(Some(&content));
                dialog.present(Some(&win));
            });
        }
    }
    {
        let state_c = state.clone();
        if let Some(btn) = details.get_action_button("Folders") {
            btn.connect_clicked(move |_| {
                let name = state_c.selected_game.borrow().clone();
                if name.is_empty() { return; }
                if let Some(path) = state_c.config.game_value(&name, "MainPath") {
                    if !path.is_empty() {
                        state_c.integration.open_folder(&path);
                    }
                }
            });
        }
    }
    {
        let state_c = state.clone();
        if let Some(btn) = details.get_action_button("SteamDB") {
            btn.connect_clicked(move |_| {
                let name = state_c.selected_game.borrow().clone();
                if name.is_empty() { return; }
                let sid = state_c.config.game_value(&name, "SteamID").unwrap_or_default();
                if sid.is_empty() {
                    // C++ parity: search fallback when there is no AppID
                    state_c.integration.open_url(&format!(
                        "https://steamdb.info/search/?q={}", url_encode(&name)));
                } else {
                    state_c.integration.open_url(&format!("https://steamdb.info/app/{}/", sid));
                }
            });
        }
    }
    {
        let state_c = state.clone();
        if let Some(btn) = details.get_action_button("ProtonDB") {
            btn.connect_clicked(move |_| {
                let name = state_c.selected_game.borrow().clone();
                if name.is_empty() { return; }
                let sid = state_c.config.game_value(&name, "SteamID").unwrap_or_default();
                if sid.is_empty() {
                    state_c.integration.open_url(&format!(
                        "https://www.protondb.com/search?q={}", url_encode(&name)));
                } else {
                    state_c.integration.open_url(&format!("https://www.protondb.com/app/{}", sid));
                }
            });
        }
    }
    {
        let state_c = state.clone();
        if let Some(btn) = details.get_action_button("Steam") {
            btn.connect_clicked(move |_| {
                let name = state_c.selected_game.borrow().clone();
                if name.is_empty() { return; }
                let sid = state_c.config.game_value(&name, "SteamID").unwrap_or_default();
                if sid.is_empty() {
                    state_c.integration.open_url(&format!(
                        "https://store.steampowered.com/search/?term={}", url_encode(&name)));
                } else {
                    state_c.integration.open_url(&format!("steam://store/{}", sid));
                }
            });
        }
    }
    {
        let state_c = state.clone();
        if let Some(btn) = details.get_action_button("GameFAQs") {
            btn.connect_clicked(move |_| {
                let name = state_c.selected_game.borrow().clone();
                if name.is_empty() { return; }
                // Console game database (safe, long-standing community site)
                state_c.integration.open_url(&format!(
                    "https://gamefaqs.com/search?game={}", url_encode(&name)));
            });
        }
    }
    {
        let state_c = state.clone();
        if let Some(btn) = details.get_action_button("MobyGames") {
            btn.connect_clicked(move |_| {
                let name = state_c.selected_game.borrow().clone();
                if name.is_empty() { return; }
                // Multi-platform database with covers/ratings (safe site)
                state_c.integration.open_url(&format!(
                    "https://www.mobygames.com/search/?q={}", url_encode(&name)));
            });
        }
    }
    {
        let state_c = state.clone();
        let win = window.clone();
        let sb_c = sidebar_ref.clone();
        let ch_c = center_handle.clone();
        let det_c = details_ref.clone();
        if let Some(btn) = details.get_action_button("Settings") {
            btn.connect_clicked(move |_| {
                let name = state_c.selected_game.borrow().clone();
                if name.is_empty() { return; }
                let state_cc = state_c.clone();
                let sb_cc = sb_c.clone();
                let ch_cc = ch_c.clone();
                let det_cc = det_c.clone();
                let is_app = state_c.game_model.get_game(&name)
                    .map(|g| g.source == crate::backend::game_model::GameSource::AppImage || g.executor == "appimage-launcher")
                    .unwrap_or(false);
                if is_app {
                    ui::apps_settings::show_apps_settings_modal(&state_c, &win, &name, move || {
                        let names = state_cc.game_model.ordered_names();
                        if let Some(ref sidebar) = *sb_cc.borrow() {
                            sidebar.refresh_list(&names);
                        }
                        if let Some(ref ch) = *ch_cc.borrow() {
                            ch.rebuild(&state_cc, &det_cc);
                        }
                        let current = state_cc.selected_game.borrow().clone();
                        if let Some(ref d) = *det_cc.borrow() {
                            if !current.is_empty() {
                                d.set_game(&current, &state_cc.config);
                            }
                        }
                    });
                } else {
                    ui::game_settings::show_game_settings_modal(&state_c, &win, &name, move || {
                        let names = state_cc.game_model.ordered_names();
                        if let Some(ref sidebar) = *sb_cc.borrow() {
                            sidebar.refresh_list(&names);
                        }
                        if let Some(ref ch) = *ch_cc.borrow() {
                            ch.rebuild(&state_cc, &det_cc);
                        }
                        let current = state_cc.selected_game.borrow().clone();
                        if let Some(ref d) = *det_cc.borrow() {
                            if !current.is_empty() {
                                d.set_game(&current, &state_cc.config);
                            }
                        }
                    });
                }
            });
        }
    }

    // === Connect close button to hide details ===
    // (handled inside DetailsPanel)

    main_box.append(&body);
    window.set_content(Some(&main_box));

    // Session poller (C++ onGameFinished parity): bank play time when the
    // child exits on its own, and keep the Play/Stop label in sync.
    {
        let state_c = state.clone();
        let det_c = details_ref.clone();
        let ch_c = center_handle.clone();
        glib::timeout_add_local(std::time::Duration::from_secs(2), move || {
            if let Some((game, secs)) = state_c.proton.check_finished() {
                if secs > 0 {
                    state_c.game_model.increment_time_spent(&game, secs);
                }
                state_c.game_model.touch_last_played(&game);
                state_c.recent_model.refresh(30);
                if let Some(ref ch) = *ch_c.borrow() {
                    ch.rebuild(&state_c, &det_c);
                }
                if let Some(ref d) = *det_c.borrow() {
                    d.set_playing(false);
                    d.set_game(&game, &state_c.config);
                }
            } else if let Some(ref d) = *det_c.borrow() {
                let selected = state_c.selected_game.borrow().clone();
                d.set_playing(
                    state_c.proton.is_game_running()
                        && state_c.proton.session_game_name() == selected
                        && !selected.is_empty(),
                );
            }
            glib::ControlFlow::Continue
        });
    }

    // Prefix warning on startup
    {
        let games = state.game_model.ordered_names();
        let mut missing: Vec<(String, String)> = Vec::new();
        for name in &games {
            if let Some(prefix) = state.config.game_value(name, "PrefixPath") {
                if !prefix.is_empty() {
                    let expanded = tilde(&prefix);
                    if !std::path::Path::new(&expanded).exists() {
                        missing.push((name.clone(), prefix));
                    }
                }
            }
        }
        if !missing.is_empty() {
            let pw = PrefixWarningModal::new();
            pw.show_missing(&missing);
            let win = window.clone();
            glib::idle_add_local(move || {
                pw.present(&win);
                glib::ControlFlow::Break
            });
        }
    }

    // Auto-install check at startup (also re-armed after list changes).
    maybe_autoinstall_proton(&state, &window);

    window
}

/// C++ applyScanPlugins parity: after a game is added/imported, scan its
/// directory with dependency-installer + dll-overrides-automator in the
/// background. DLL overrides apply straight to the game (visible in Game
/// Settings, used at launch); missing dependencies ask first (winetricks
/// into the prefix is slow and mutates it). Emulator games are skipped.
pub(crate) fn run_plugin_scans(
    state: &AppState,
    parent: &adw::ApplicationWindow,
    game_name: &str,
    game_dir: &str,
) {
    if game_dir.trim().is_empty() {
        return;
    }
    if let Some(exe) = state.config.game_value(game_name, "Executor") {
        if !exe.trim().is_empty() {
            return;
        }
    }
    let plugins_dir = state.plugins.plugins_dir();
    let dir_s = game_dir.trim().to_string();
    let prefix_s = state.proton.real_prefix_for(game_name).display().to_string();
    let proton_s = state
        .config
        .game_value(game_name, "Proton")
        .and_then(|w| state.proton.resolve_proton(&w).ok())
        .map(|(_, p)| p.display().to_string())
        .unwrap_or_default();
    let prefix_c = prefix_s.clone();
    let proton_c = proton_s.clone();
    let dir_c = plugins_dir.clone();
    let (tx, rx) = std::sync::mpsc::channel::<(String, Vec<(String, String)>)>();
    std::thread::spawn(move || {
        let overrides = PluginManager::dll_scan_in(&plugins_dir, &dir_s).unwrap_or_default();
        let missing = PluginManager::dep_scan_in(&plugins_dir, &dir_s, &prefix_s, &proton_s)
            .map(|(m, _)| m)
            .unwrap_or_default();
        let _ = tx.send((overrides, missing));
    });
    let state_c = state.clone();
    let parent_c = parent.clone();
    let game_c = game_name.to_string();
    glib::idle_add_local(move || match rx.try_recv() {
        Ok((overrides, missing)) => {
            // Apply DLL overrides unless the user already set custom ones.
            if !overrides.trim().is_empty() {
                let cur = state_c.config.game_value(&game_c, "Overrides").unwrap_or_default();
                if cur.trim().is_empty() {
                    state_c.game_model.set_overrides(&game_c, overrides.trim());
                }
            }
            if missing.is_empty() {
                glib::ControlFlow::Break
            } else {
                let names: Vec<String> = missing.iter().map(|(id, _)| id.clone()).collect();
                let descs: Vec<String> = missing
                    .iter()
                    .map(|(id, d)| {
                        if d == id {
                            id.clone()
                        } else {
                            format!("{} ({})", id, d)
                        }
                    })
                    .collect();
                let cdlg = adw::Dialog::new();
                cdlg.set_title("Missing dependencies");
                cdlg.set_content_width(440);
                cdlg.set_content_height(230);
                let outer = gtk::Box::new(gtk::Orientation::Vertical, 0);
                outer.add_css_class("modal-bg");
                outer.set_hexpand(true);
                outer.set_vexpand(true);
                let inner = gtk::Box::new(gtk::Orientation::Vertical, 10);
                inner.set_hexpand(true);
                inner.set_vexpand(true);
                inner.set_margin_top(12);
                inner.set_margin_bottom(12);
                inner.set_margin_start(16);
                inner.set_margin_end(16);
                outer.append(&inner);
                let (header_row, x_btn) = helpers::modal_header("Missing dependencies");
                {
                    let d0 = cdlg.clone();
                    x_btn.connect_clicked(move |_| { d0.close(); });
                }
                inner.append(&header_row);
                let msg = gtk::Label::new(Some(&format!(
                    "{} needs these Windows dependencies. Install them into the game prefix now?\n\n{}",
                    game_c,
                    descs.join(", ")
                )));
                msg.set_halign(gtk::Align::Start);
                msg.set_wrap(true);
                inner.append(&msg);
                let row = gtk::Box::new(gtk::Orientation::Horizontal, 10);
                let no_btn = gtk::Button::with_label("No");
                no_btn.add_css_class("neon-red");
                no_btn.set_hexpand(true);
                let yes_btn = gtk::Button::with_label("Yes");
                yes_btn.add_css_class("neon-green");
                yes_btn.set_hexpand(true);
                {
                    let d0 = cdlg.clone();
                    no_btn.connect_clicked(move |_| { d0.close(); });
                }
                {
                    let d0 = cdlg.clone();
                    let parent_cc = parent_c.clone();
                    let dir_d = dir_c.clone();
                    let prefix_d = prefix_c.clone();
                    let proton_d = proton_c.clone();
                    yes_btn.connect_clicked(move |_| {
                        d0.close();
                        let (tx2, rx2) = std::sync::mpsc::channel::<Result<String, String>>();
                        let dir_t = dir_d.clone();
                        let prefix_t = prefix_d.clone();
                        let proton_t = proton_d.clone();
                        let names_t = names.clone();
                        std::thread::spawn(move || {
                            // The real prefix may not exist yet on a fresh
                            // import (Proton creates it on first launch);
                            // the installer refuses missing prefixes.
                            std::fs::create_dir_all(&prefix_t).ok();
                            let r = PluginManager::dep_install_in(&dir_t, &prefix_t, &proton_t, &names_t);
                            let _ = tx2.send(r);
                        });
                        let parent_ci = parent_cc.clone();
                        glib::idle_add_local(move || match rx2.try_recv() {
                            Ok(Ok(m)) => {
                                helpers::present_msg(&parent_ci, "Dependencies installed", &m);
                                glib::ControlFlow::Break
                            }
                            Ok(Err(e)) => {
                                helpers::present_msg(&parent_ci, "Dependency install failed", &e);
                                glib::ControlFlow::Break
                            }
                            Err(std::sync::mpsc::TryRecvError::Empty) => glib::ControlFlow::Continue,
                            Err(_) => glib::ControlFlow::Break,
                        });
                    });
                }
                row.append(&no_btn);
                row.append(&yes_btn);
                inner.append(&row);
                cdlg.set_child(Some(&outer));
                cdlg.present(Some(&parent_c));
                glib::ControlFlow::Break
            }
        }
        Err(std::sync::mpsc::TryRecvError::Empty) => glib::ControlFlow::Continue,
        Err(_) => glib::ControlFlow::Break,
    });
}

/// Shared No (neon-red) / Yes (neon-green) confirm for dependency installs.
/// Yes runs winetricks in the background, then reports per game.
pub(crate) fn confirm_install_deps(
    state: &AppState,
    parent: &adw::ApplicationWindow,
    game_name: &str,
    dep_ids: Vec<String>,
) {
    if dep_ids.is_empty() {
        return;
    }
    let plugins_dir = state.plugins.plugins_dir();
    let prefix = state.proton.real_prefix_for(game_name).display().to_string();
    let proton = state
        .config
        .game_value(game_name, "Proton")
        .and_then(|w| state.proton.resolve_proton(&w).ok())
        .map(|(_, p)| p.display().to_string())
        .unwrap_or_default();
    let cdlg = adw::Dialog::new();
    cdlg.set_title("Missing dependencies");
    cdlg.set_content_width(440);
    cdlg.set_content_height(230);
    let outer = gtk::Box::new(gtk::Orientation::Vertical, 0);
    outer.add_css_class("modal-bg");
    outer.set_hexpand(true);
    outer.set_vexpand(true);
    let inner = gtk::Box::new(gtk::Orientation::Vertical, 10);
    inner.set_hexpand(true);
    inner.set_vexpand(true);
    inner.set_margin_top(12);
    inner.set_margin_bottom(12);
    inner.set_margin_start(16);
    inner.set_margin_end(16);
    outer.append(&inner);
    let (header_row, x_btn) = helpers::modal_header("Missing dependencies");
    {
        let d0 = cdlg.clone();
        x_btn.connect_clicked(move |_| { d0.close(); });
    }
    inner.append(&header_row);
    let msg = gtk::Label::new(Some(&format!(
        "{} needs these Windows dependencies. Install them into the game prefix now?\n\n{}",
        game_name,
        dep_ids.join(", ")
    )));
    msg.set_halign(gtk::Align::Start);
    msg.set_wrap(true);
    inner.append(&msg);
    let row = gtk::Box::new(gtk::Orientation::Horizontal, 10);
    let no_btn = gtk::Button::with_label("No");
    no_btn.add_css_class("neon-red");
    no_btn.set_hexpand(true);
    let yes_btn = gtk::Button::with_label("Yes");
    yes_btn.add_css_class("neon-green");
    yes_btn.set_hexpand(true);
    {
        let d0 = cdlg.clone();
        no_btn.connect_clicked(move |_| { d0.close(); });
    }
    {
        let d0 = cdlg.clone();
        let parent_c = parent.clone();
        let game_c = game_name.to_string();
        yes_btn.connect_clicked(move |_| {
            d0.close();
            let (tx2, rx2) = std::sync::mpsc::channel::<Result<String, String>>();
            let dir_t = plugins_dir.clone();
            let prefix_t = prefix.clone();
            let proton_t = proton.clone();
            let ids_t = dep_ids.clone();
            std::thread::spawn(move || {
                std::fs::create_dir_all(&prefix_t).ok();
                let r = PluginManager::dep_install_in(&dir_t, &prefix_t, &proton_t, &ids_t);
                let _ = tx2.send(r);
            });
            let parent_ci = parent_c.clone();
            let game_ci = game_c.clone();
            glib::idle_add_local(move || match rx2.try_recv() {
                Ok(Ok(m)) => {
                    helpers::present_msg(&parent_ci, "Dependencies installed", &format!("{}: {}", game_ci, m));
                    glib::ControlFlow::Break
                }
                    Ok(Err(e)) => {
                        helpers::present_msg(&parent_ci, "Dependency install failed", &format!("{}: {}", game_ci, e));
                        glib::ControlFlow::Break
                    }
                Err(std::sync::mpsc::TryRecvError::Empty) => glib::ControlFlow::Continue,
                Err(_) => glib::ControlFlow::Break,
            });
        });
    }
    row.append(&no_btn);
    row.append(&yes_btn);
    inner.append(&row);
    cdlg.set_child(Some(&outer));
    cdlg.present(Some(parent));
}

/// Batch twin of run_plugin_scans for Steam/Lutris imports: silent DLL
/// overrides per game, but a SINGLE summary dialog when any game needs
/// dependencies (one Yes installs into every affected prefix).
pub(crate) fn run_plugin_scans_batch(
    state: &AppState,
    parent: &adw::ApplicationWindow,
    games: Vec<(String, String)>,
) {
    // (name, dir, prefix, proton); GObject work stays on this thread.
    let mut jobs: Vec<(String, String, String, String)> = Vec::new();
    for (name, dir) in games {
        if dir.trim().is_empty() {
            continue;
        }
        if let Some(e) = state.config.game_value(&name, "Executor") {
            if !e.trim().is_empty() {
                continue;
            }
        }
        let prefix = state.proton.real_prefix_for(&name).display().to_string();
        let proton = state
            .config
            .game_value(&name, "Proton")
            .and_then(|w| state.proton.resolve_proton(&w).ok())
            .map(|(_, p)| p.display().to_string())
            .unwrap_or_default();
        jobs.push((name, dir.trim().to_string(), prefix, proton));
    }
    if jobs.is_empty() {
        return;
    }
    let plugins_dir = state.plugins.plugins_dir();
    let dir_c = plugins_dir.clone();
    // (name, overrides, missing, prefix, proton)
    let (tx, rx) = std::sync::mpsc::channel::<
        Vec<(String, String, Vec<(String, String)>, String, String)>,
    >();
    std::thread::spawn(move || {
        let mut out = Vec::new();
        for (name, dir, prefix, proton) in jobs {
            let ov = PluginManager::dll_scan_in(&plugins_dir, &dir).unwrap_or_default();
            let missing = PluginManager::dep_scan_in(&plugins_dir, &dir, &prefix, &proton)
                .map(|(m, _)| m)
                .unwrap_or_default();
            out.push((name, ov, missing, prefix, proton));
        }
        let _ = tx.send(out);
    });
    let state_c = state.clone();
    let parent_c = parent.clone();
    glib::idle_add_local(move || match rx.try_recv() {
        Ok(results) => {
            let mut needy: Vec<(String, String, String, Vec<String>)> = Vec::new();
            for (name, overrides, missing, prefix, proton) in &results {
                if !overrides.trim().is_empty() {
                    let cur = state_c.config.game_value(name, "Overrides").unwrap_or_default();
                    if cur.trim().is_empty() {
                        state_c.game_model.set_overrides(name, overrides.trim());
                    }
                }
                if !missing.is_empty() {
                    needy.push((
                        name.clone(),
                        prefix.clone(),
                        proton.clone(),
                        missing.iter().map(|(id, _)| id.clone()).collect(),
                    ));
                }
            }
            if needy.is_empty() {
                glib::ControlFlow::Break
            } else {
                let lines: Vec<String> = needy
                    .iter()
                    .map(|(n, _, _, ids)| format!("{} — {}", n, ids.join(", ")))
                    .collect();
                let cdlg = adw::Dialog::new();
                cdlg.set_title("Missing dependencies");
                cdlg.set_content_width(440);
                cdlg.set_content_height(250);
                let outer = gtk::Box::new(gtk::Orientation::Vertical, 0);
                outer.add_css_class("modal-bg");
                outer.set_hexpand(true);
                outer.set_vexpand(true);
                let inner = gtk::Box::new(gtk::Orientation::Vertical, 10);
                inner.set_hexpand(true);
                inner.set_vexpand(true);
                inner.set_margin_top(12);
                inner.set_margin_bottom(12);
                inner.set_margin_start(16);
                inner.set_margin_end(16);
                outer.append(&inner);
                let (header_row, x_btn) = helpers::modal_header("Missing dependencies");
                {
                    let d0 = cdlg.clone();
                    x_btn.connect_clicked(move |_| { d0.close(); });
                }
                inner.append(&header_row);
                let msg = gtk::Label::new(Some(&format!(
                    "These imported games need Windows dependencies. Install them into their prefixes now?\n\n{}",
                    lines.join("\n")
                )));
                msg.set_halign(gtk::Align::Start);
                msg.set_wrap(true);
                inner.append(&msg);
                let row = gtk::Box::new(gtk::Orientation::Horizontal, 10);
                let no_btn = gtk::Button::with_label("No");
                no_btn.add_css_class("neon-red");
                no_btn.set_hexpand(true);
                let yes_btn = gtk::Button::with_label("Yes");
                yes_btn.add_css_class("neon-green");
                yes_btn.set_hexpand(true);
                {
                    let d0 = cdlg.clone();
                    no_btn.connect_clicked(move |_| { d0.close(); });
                }
                {
                    let d0 = cdlg.clone();
                    let parent_cc = parent_c.clone();
                    let dir_d = dir_c.clone();
                    yes_btn.connect_clicked(move |_| {
                        d0.close();
                        let (tx2, rx2) = std::sync::mpsc::channel::<Vec<String>>();
                        let needy_t = needy.clone();
                        let dir_t = dir_d.clone();
                        std::thread::spawn(move || {
                            let mut done = Vec::new();
                            for (n, prefix, proton, ids) in needy_t {
                                std::fs::create_dir_all(&prefix).ok();
                                match PluginManager::dep_install_in(&dir_t, &prefix, &proton, &ids) {
                                    Ok(m) => done.push(format!("{}: {}", n, m)),
                                    Err(e) => done.push(format!("{} failed: {}", n, e)),
                                }
                            }
                            let _ = tx2.send(done);
                        });
                        let parent_ci = parent_cc.clone();
                        glib::idle_add_local(move || match rx2.try_recv() {
                            Ok(done) => {
                                helpers::present_msg(&parent_ci, "Dependencies installed", &done.join("\n"));
                                glib::ControlFlow::Break
                            }
                            Err(std::sync::mpsc::TryRecvError::Empty) => glib::ControlFlow::Continue,
                            Err(_) => glib::ControlFlow::Break,
                        });
                    });
                }
                row.append(&no_btn);
                row.append(&yes_btn);
                inner.append(&row);
                cdlg.set_child(Some(&outer));
                cdlg.present(Some(&parent_c));
                glib::ControlFlow::Break
            }
        }
        Err(std::sync::mpsc::TryRecvError::Empty) => glib::ControlFlow::Continue,
        Err(_) => glib::ControlFlow::Break,
    });
}

/// Install latest Proton-GE when no builds are detected in any configured
/// path. Target: custom Path1 -> Path2 -> Path3, else the default Path1
/// dir (created). Background thread does network + disk; the main thread
/// applies the result (GObjects never cross threads). Attempt-once per
/// empty episode: resets as soon as any build is detected again.
pub(crate) fn maybe_autoinstall_proton(state: &AppState, parent: &adw::ApplicationWindow) {
    static TRIED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
    use std::sync::atomic::Ordering;
    if !state.proton.installed_protons().is_empty() {
        TRIED.store(false, Ordering::SeqCst);
        return;
    }
    if TRIED.swap(true, Ordering::SeqCst) {
        return;
    }
    // Visible progress dialog: no silent background downloads. The user
    // sees what is being installed, how much is left and at what speed.
    let dlg = adw::Dialog::new();
    dlg.set_title("Downloading Proton");
    dlg.set_content_width(440);
    dlg.set_content_height(230);
    let outer = gtk::Box::new(gtk::Orientation::Vertical, 0);
    outer.add_css_class("modal-bg");
    outer.set_hexpand(true);
    outer.set_vexpand(true);
    let inner = gtk::Box::new(gtk::Orientation::Vertical, 10);
    inner.set_hexpand(true);
    inner.set_vexpand(true);
    inner.set_margin_top(12);
    inner.set_margin_bottom(12);
    inner.set_margin_start(16);
    inner.set_margin_end(16);
    outer.append(&inner);
    let (header_row, x_btn) = helpers::modal_header("Downloading Proton");
    {
        let d0 = dlg.clone();
        x_btn.connect_clicked(move |_| { d0.close(); });
    }
    inner.append(&header_row);
    let info_lbl = gtk::Label::new(Some(
        "No Proton builds found in the configured paths. Looking up the latest Proton-GE…",
    ));
    info_lbl.set_halign(gtk::Align::Start);
    info_lbl.set_wrap(true);
    info_lbl.set_opacity(0.8);
    inner.append(&info_lbl);
    let bar = gtk::ProgressBar::new();
    bar.set_show_text(true);
    bar.set_text(Some("Starting…"));
    bar.set_fraction(0.0);
    inner.append(&bar);
    let hide_btn = gtk::Button::with_label("Hide");
    hide_btn.add_css_class("settings-btn");
    hide_btn.set_halign(gtk::Align::Center);
    hide_btn.set_width_request(120);
    {
        let d0 = dlg.clone();
        hide_btn.connect_clicked(move |_| { d0.close(); });
    }
    inner.append(&hide_btn);
    dlg.set_child(Some(&outer));
    dlg.present(Some(parent));
    let parent_c = parent.clone();
    let dlg_c = dlg.clone();
    let bar_c = bar.clone();
    let info_c = info_lbl.clone();
    let hide_c = hide_btn.clone();
    let dlg_c = dlg.clone();

    let (tx, rx) = std::sync::mpsc::channel::<Result<(String, String), String>>();
    let (ptx, prx) = std::sync::mpsc::channel::<(f64, f64)>();
    let (ttx, trx) = std::sync::mpsc::channel::<String>();
    std::thread::spawn(move || {
        let cfg = ConfigManager::new();
        let custom = |k: &str| {
            cfg.launcher_value(k)
                .map(|v| v.trim().to_string())
                .filter(|v| !v.is_empty())
        };
        let target = custom("protonsPath")
            .or_else(|| custom("protonsPath2"))
            .or_else(|| custom("protonsPath3"))
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|| cfg.base_path_for("protons"));
        let result = (|| -> Result<(String, String), String> {
            let releases = ProtonManager::fetch_releases("ge")?;
            let (tag, url) = releases
                .into_iter()
                .next()
                .ok_or_else(|| "No GE-Proton releases found".to_string())?;
            let _ = ttx.send(tag.clone());
            let _ = ptx.send((0.0, 0.0));
            let dest = ProtonManager::download_proton(&tag, &url, &target, |f, bps| {
                let _ = ptx.send((f, bps));
            })?;
            Ok((tag, dest.display().to_string()))
        })();
        if let Err(e) = &result {
            eprintln!("[CorkyTux] Proton auto-install failed: {}", e);
        }
        let _ = tx.send(result);
    });
    let state_c = state.clone();
    glib::idle_add_local(move || {
        // Show which version is being fetched, then track progress.
        if let Ok(tag) = trx.try_recv() {
            info_c.set_text(&format!(
                "No Proton builds found in the configured paths. Downloading {} automatically…",
                tag
            ));
        }
        // Drain progress, keep the latest.
        let mut last: Option<(f64, f64)> = None;
        while let Ok(p) = prx.try_recv() {
            last = Some(p);
        }
        if let Some((f, bps)) = last {
            bar_c.set_fraction(f.clamp(0.0, 1.0));
            bar_c.set_text(Some(&format!(
                "{:.0}% · {}",
                f * 100.0,
                ProtonManager::format_speed(bps)
            )));
        }
        match rx.try_recv() {
            Ok(Ok((tag, dest))) => {
                state_c.proton.refresh_installed();
                let cur = state_c.config.launcher_value("defaultProton").unwrap_or_default();
                if cur.trim().is_empty() {
                    state_c.config.set_launcher_value("defaultProton", &tag);
                }
                // A build exists now: re-arm future empty episodes.
                TRIED.store(false, Ordering::SeqCst);
                dlg_c.close();
                helpers::present_msg(
                    &parent_c,
                    "Proton installed",
                    &format!("Proton-GE {} installed to {}", tag, dest),
                );
                glib::ControlFlow::Break
            }
            Ok(Err(e)) => {
                // Failed: allow a later trigger to retry.
                TRIED.store(false, Ordering::SeqCst);
                info_c.set_text(&format!("Download failed: {}", e));
                bar_c.set_fraction(0.0);
                bar_c.set_text(Some("Failed"));
                hide_c.set_label("Close");
                glib::ControlFlow::Break
            }
            Err(std::sync::mpsc::TryRecvError::Empty) => glib::ControlFlow::Continue,
            Err(_) => glib::ControlFlow::Break,
        }
    });
}

fn main() {
    let app = adw::Application::builder()
        .application_id("com.corkytux.CorkyTux").build();
    app.connect_activate(|app| {
        let window = build_window(app);
        window.present();
    });
    app.run();
}
