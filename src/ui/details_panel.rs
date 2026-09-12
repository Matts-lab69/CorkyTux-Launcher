use gtk::prelude::*;
use gtk::{self, glib};

use crate::backend::config::ConfigManager;
use crate::backend::theme::ThemeManager;
use crate::ui::helpers;

#[derive(Clone)]
pub struct DetailsPanel {
    pub revealer: gtk::Revealer,
    root: gtk::Box,
    title_label: gtk::Label,
    star_button: gtk::ToggleButton,
    star_handler_id: std::rc::Rc<std::cell::RefCell<Option<glib::SignalHandlerId>>>,
    banner_image: gtk::Image,
    icon_image: gtk::Image,
    play_button: gtk::Button,
    play_icon: gtk::Image,
    play_label: gtk::Label,
    is_dark: bool,
    time_label: gtk::Label,
    install_size_label: gtk::Label,
    install_path_label: gtk::Label,
    prefix_label: gtk::Label,
    current_game: std::rc::Rc<std::cell::RefCell<Option<String>>>,
    pub action_buttons: std::rc::Rc<std::cell::RefCell<Vec<(String, gtk::Button, u8)>>>,
    actions_grid: gtk::Grid,
}

impl DetailsPanel {
    pub fn new<F: Fn(String) -> Result<(), String> + 'static + Clone, G: Fn(String) + 'static + Clone>(
        theme: &ThemeManager,
        on_play: F,
        on_favorite: G,
    ) -> Self {
        let is_dark = theme.is_dark();
        let revealer = gtk::Revealer::new();
        revealer.set_transition_type(gtk::RevealerTransitionType::SlideLeft);
        revealer.set_transition_duration(220);
        revealer.set_reveal_child(false);
        revealer.set_halign(gtk::Align::End);
        revealer.set_valign(gtk::Align::Fill);
        // No width_request here: a minimum width would keep a dead strip
        // visible on the right even while the panel is hidden. The child
        // root below already requests 380px when revealed.

        let root = gtk::Box::new(gtk::Orientation::Vertical, 0);
        root.set_width_request(270);
        root.add_css_class("details-panel");

        let scroll = gtk::ScrolledWindow::new();
        scroll.set_vexpand(true);
        scroll.set_hscrollbar_policy(gtk::PolicyType::Never);

        let content = gtk::Box::new(gtk::Orientation::Vertical, 16);
        content.set_margin_top(16);
        content.set_margin_bottom(16);
        content.set_margin_start(16);
        content.set_margin_end(16);

        let current_game_rc: std::rc::Rc<std::cell::RefCell<Option<String>>> =
            std::rc::Rc::new(std::cell::RefCell::new(None));

        let header_bar = gtk::Box::new(gtk::Orientation::Horizontal, 8);

        let title = gtk::Label::new(None);
        title.set_halign(gtk::Align::Start);
        title.set_hexpand(true);
        title.set_ellipsize(gtk::pango::EllipsizeMode::End);
        title.add_css_class("details-title");

        let star = gtk::ToggleButton::new();
        star.set_icon_name("starred-symbolic");
        star.set_tooltip_text(Some("Toggle Favorite"));
        star.add_css_class("star-btn");
        let of = on_favorite.clone();
        let cg_for_star = current_game_rc.clone();
        let star_handler_id = star.connect_toggled(move |b| {
            if let Some(ref name) = *cg_for_star.borrow() {
                let action = if b.is_active() { "fav_on" } else { "fav_off" };
                of(format!("{}:{}", action, name));
            }
        });

        let close_btn = gtk::Button::from_icon_name("window-close-symbolic");
        close_btn.set_tooltip_text(Some("Close"));
        close_btn.add_css_class("close-btn");

        let icon_img = gtk::Image::new();
        icon_img.set_pixel_size(32);
        icon_img.set_width_request(32);
        icon_img.set_height_request(32);
        icon_img.set_valign(gtk::Align::Center);
        header_bar.append(&icon_img);
        header_bar.append(&title);
        header_bar.append(&star);
        header_bar.append(&close_btn);
        content.append(&header_bar);

        let banner_frame = gtk::Frame::new(None);
        banner_frame.add_css_class("banner-frame");
        banner_frame.set_halign(gtk::Align::Fill);
        let banner = gtk::Image::new();
        banner.set_pixel_size(340);
        banner.set_height_request(192);
        banner.set_halign(gtk::Align::Fill);
        banner.set_valign(gtk::Align::Center);
        banner_frame.set_child(Some(&banner));
        content.append(&banner_frame);

        // C++ parity: play/stop glyph + label (was text-only).
        let play_btn = gtk::Button::new();
        play_btn.add_css_class("play-btn");
        play_btn.set_halign(gtk::Align::Fill);
        let play_box = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        play_box.set_halign(gtk::Align::Center);
        let play_icon = gtk::Image::new();
        play_icon.set_pixel_size(16);
        if let Some(tex) = helpers::load_themed_icon("play", is_dark) {
            play_icon.set_paintable(Some(&tex));
        } else {
            play_icon.set_icon_name(Some("media-playback-start-symbolic"));
        }
        let play_label = gtk::Label::new(Some("Play"));
        play_box.append(&play_icon);
        play_box.append(&play_label);
        play_btn.set_child(Some(&play_box));
        let op = on_play.clone();
        let cg_play = current_game_rc.clone();
        let btn_parent = play_btn.clone();
        play_btn.connect_clicked(move |_| {
            if let Some(ref name) = *cg_play.borrow() {
                // Launches used to fail silently (let _ = ...): surface the
                // reason (e.g. Proton not found) instead.
                if let Err(e) = op(name.clone()) {
                    helpers::present_msg(&btn_parent, "Failed to launch", &e);
                }
            }
        });
        content.append(&play_btn);

        let time = gtk::Label::new(None);
        time.set_halign(gtk::Align::Start);
        time.add_css_class("time-label");
        content.append(&time);

        let info_frame = gtk::Frame::new(None);
        info_frame.add_css_class("info-card");
        let info_box = gtk::Box::new(gtk::Orientation::Vertical, 8);
        info_box.set_margin_top(8);
        info_box.set_margin_bottom(8);
        info_box.set_margin_start(8);
        info_box.set_margin_end(8);

        let install_title = gtk::Label::new(Some("Install Info"));
        install_title.set_halign(gtk::Align::Start);
        install_title.add_css_class("info-label");
        info_box.append(&install_title);

        let size_label = gtk::Label::new(None);
        size_label.set_halign(gtk::Align::Start);
        size_label.add_css_class("info-value");
        let path_label = gtk::Label::new(None);
        path_label.set_halign(gtk::Align::Start);
        path_label.set_wrap(true);
        path_label.set_selectable(true);
        path_label.add_css_class("info-value");
        let prefix_label = gtk::Label::new(None);
        prefix_label.set_halign(gtk::Align::Start);
        prefix_label.set_wrap(true);
        prefix_label.set_selectable(true);
        prefix_label.add_css_class("info-value");

        info_box.append(&size_label);
        info_box.append(&path_label);
        info_box.append(&prefix_label);
        info_frame.set_child(Some(&info_box));
        content.append(&info_frame);

        let actions_frame = gtk::Frame::new(None);
        actions_frame.add_css_class("actions-frame");
        let actions_inner = gtk::Box::new(gtk::Orientation::Vertical, 6);
        actions_inner.set_margin_top(4);
        actions_inner.set_margin_bottom(4);
        actions_inner.set_margin_start(4);
        actions_inner.set_margin_end(4);
        let actions_title = gtk::Label::new(Some("Actions"));
        actions_title.set_halign(gtk::Align::Start);
        actions_title.add_css_class("frame-title");
        actions_inner.append(&actions_title);

        let actions_grid = gtk::Grid::new();
        actions_grid.set_column_homogeneous(true);
        actions_grid.set_row_homogeneous(true);
        actions_grid.set_column_spacing(4);
        actions_grid.set_row_spacing(4);

        // (label, icon_asset, visibility): 0 = both, 1 = proton-only, 2 = emu-only.
        // Emulator games get GameFAQs + MobyGames (console DBs) instead of the
        // Proton/Steam ones, so the grid never shows holes.
        let action_defs = [
            ("Settings", "settings", 0),
            ("Debug", "debug", 0),
            ("Remove", "remove", 0),
            ("Wine", "wine", 1),
            ("Run .exe", "run", 1),
            ("Folders", "folder", 0),
            ("SteamDB", "db", 1),
            ("ProtonDB", "protondb", 1),
            ("Steam", "steam", 1),
            ("GameFAQs", "search", 2),
            ("MobyGames", "openIn", 2),
        ];
        let action_buttons: std::rc::Rc<std::cell::RefCell<Vec<(String, gtk::Button, u8)>>> =
            std::rc::Rc::new(std::cell::RefCell::new(Vec::new()));
        for (i, (label, icon, vis)) in action_defs.iter().enumerate() {
            let btn = gtk::Button::new();
            btn.add_css_class("action-btn");
            let content = gtk::Box::new(gtk::Orientation::Vertical, 2);
            content.set_halign(gtk::Align::Center);
            let img = helpers::themed_image(icon, theme.is_dark(), 18);
            let lbl = gtk::Label::new(Some(label));
            content.append(&img);
            content.append(&lbl);
            btn.set_child(Some(&content));
            let _ = i;
            action_buttons.borrow_mut().push((label.to_string(), btn, *vis));
        }
        Self::layout_actions(&actions_grid, &action_buttons);

        actions_inner.append(&actions_grid);
        actions_frame.set_child(Some(&actions_inner));
        content.append(&actions_frame);

        scroll.set_child(Some(&content));
        root.append(&scroll);

        // Collapse layout space when closed (QML overlay parity:
        // hidden details must not reserve its 380px)
        let rev_notify = revealer.clone();
        revealer.connect_child_revealed_notify(move |r| {
            r.set_visible(r.is_child_revealed());
        });
        revealer.set_visible(false);

        let revealer_clone = revealer.clone();
        close_btn.connect_clicked(move |_| {
            revealer_clone.set_reveal_child(false);
        });

        let panel = Self {
            revealer,
            root,
            title_label: title,
            star_button: star,
            star_handler_id: std::rc::Rc::new(std::cell::RefCell::new(Some(star_handler_id))),
            banner_image: banner,
            icon_image: icon_img,
            play_button: play_btn,
            play_icon,
            play_label,
            is_dark,
            time_label: time,
            install_size_label: size_label,
            install_path_label: path_label,
            prefix_label,
            current_game: current_game_rc,
            action_buttons,
            actions_grid,
        };

        panel.revealer.set_child(Some(&panel.root));
        panel
    }

    pub fn show(&self) {
        self.revealer.set_visible(true);
        self.revealer.set_reveal_child(true);
    }

    pub fn hide(&self) {
        self.revealer.set_reveal_child(false);
    }

    pub fn set_title(&self, name: &str) {
        self.title_label.set_text(name);
    }

    pub fn set_banner(&self, path: Option<&str>) {
        if let Some(p) = path {
            if !p.is_empty() {
                // Cover-crop to the frame: aspect-fit leaves gray bands.
                if let Some(texture) = helpers::load_card_banner(&shellexpand_tilde(p), 380, 192) {
                    self.banner_image.set_paintable(Some(&texture));
                    return;
                }
                if let Some(texture) = helpers::load_texture(&shellexpand_tilde(p)) {
                    self.banner_image.set_paintable(Some(&texture));
                    return;
                }
            }
        }
        self.banner_image
            .set_icon_name(Some("image-x-generic-symbolic"));
    }

    pub fn set_playing(&self, playing: bool) {
        let name = if playing { "stop" } else { "play" };
        self.play_label.set_text(if playing { "Stop" } else { "Play" });
        if let Some(tex) = helpers::load_themed_icon(name, self.is_dark) {
            self.play_icon.set_paintable(Some(&tex));
        }
    }

    pub fn set_time_played(&self, seconds: u64) {
        if seconds < 3600 {
            let m = seconds / 60;
            self.time_label.set_text(&format!("Time played: {} min", m));
        } else {
            let h = seconds / 3600;
            self.time_label.set_text(&format!("Time played: {} h", h));
        }
    }

    pub fn set_favorited(&self, fav: bool) {
        self.star_button.set_active(fav);
    }

    pub fn set_install_info(&self, game_name: &str, cfg: &ConfigManager) {
        let size = cfg
            .game_value(game_name, "InstallSize")
            .unwrap_or_default();
        let main_path = cfg.game_value(game_name, "MainPath").unwrap_or_default();
        if size.is_empty() && !main_path.is_empty() {
            // Big installs take seconds to walk: compute off-thread, cache it.
            self.install_size_label.set_text("Size: …");
            let lbl = self.install_size_label.clone();
            let gname = game_name.to_string();
            let mpath = main_path.clone();
            let (tx, rx) = std::sync::mpsc::channel::<String>();
            std::thread::spawn(move || {
                let expanded = shellexpand_tilde(&mpath);
                let path = std::path::Path::new(&expanded);
                let text = if path.exists() {
                    format!("Size: {}", format_size(query_folder_size(path)))
                } else {
                    "Size: --".to_string()
                };
                let _ = tx.send(text);
            });
            glib::timeout_add_local(std::time::Duration::from_millis(100), move || match rx.try_recv() {
                Ok(text) => {
                    lbl.set_text(&text);
                    if let Some(val) = text.strip_prefix("Size: ") {
                        if val != "--" {
                            crate::backend::config::ConfigManager::new()
                                .set_game_value(&gname, "InstallSize", val);
                        }
                    }
                    glib::ControlFlow::Break
                }
                Err(std::sync::mpsc::TryRecvError::Empty) => glib::ControlFlow::Continue,
                Err(_) => glib::ControlFlow::Break,
            });
        } else if size.is_empty() {
            self.install_size_label.set_text("Size: --");
        } else {
            self.install_size_label.set_text(&format!("Size: {}", size));
        }

        let path_text = if main_path.is_empty() {
            "Path: --".to_string()
        } else {
            format!("Path: {}", main_path)
        };
        self.install_path_label.set_text(&path_text);

        let executor = cfg
            .game_value(game_name, "Executor")
            .unwrap_or_default();
        let source = cfg
            .game_value(game_name, "Source")
            .unwrap_or_default();
        let is_appimage = executor == "appimage-launcher" || source.eq_ignore_ascii_case("appimage");
        let is_rpg = executor == "rpgmaker-runtime" || source.eq_ignore_ascii_case("rpgmaker");
        let prefix_text = if is_appimage {
            String::from("Application: AppImage")
        } else if is_rpg {
            let eng = cfg.game_value(game_name, "RpgEngine").unwrap_or_default();
            let short = if eng.contains("MZ") {
                "MZ"
            } else if eng.contains("MV") {
                "MV"
            } else if eng.contains("2000") {
                "2k3"
            } else {
                ""
            };
            if short.is_empty() {
                String::from("RPG Maker: box-rpg")
            } else {
                format!("RPG Maker: {} (box-rpg)", short)
            }
        } else if !executor.is_empty() {
            format!("Emulator: {}", executor)
        } else {
            // Same resolution as launch: explicit field or default compatdata
            let prefix = cfg
                .game_value(game_name, "PrefixPath")
                .filter(|v| !v.is_empty())
                .unwrap_or_else(|| {
                    let home = std::env::var("HOME").unwrap_or_default();
                    format!("{}/.local/share/Steam/steamapps/compatdata/0/pfx", home)
                });
            format!("Prefix: {}", prefix)
        };
        self.prefix_label.set_text(&prefix_text);

        // Proton-only buttons hide on emulators; emu-only buttons
        // (GameFAQs/MobyGames) hide on Proton games — grid stays full.
        // AppImages and RPGs keep Settings, Debug, Remove only.
        let is_emulator = !executor.is_empty() && !is_appimage && !is_rpg;
        for (label, btn, vis) in self.action_buttons.borrow().iter() {
            btn.set_visible(if is_appimage || is_rpg {
                label == "Settings" || label == "Debug" || label == "Remove"
            } else {
                match vis {
                    1 => !is_emulator,
                    2 => is_emulator,
                    _ => true,
                }
            });
        }
        Self::layout_actions(&self.actions_grid, &self.action_buttons);
    }

    /// Re-attach only visible buttons left-to-right so hidden ones
    /// never leave holes or push the rest down.
    fn layout_actions(
        grid: &gtk::Grid,
        buttons: &std::rc::Rc<std::cell::RefCell<Vec<(String, gtk::Button, u8)>>>,
    ) {
        let mut pos = 0;
        for (_, btn, _) in buttons.borrow().iter() {
            // Detach first (no-op if not attached) then re-attach in order.
            grid.remove(btn);
            if btn.is_visible() {
                grid.attach(btn, (pos % 3) as i32, (pos / 3) as i32, 1, 1);
                pos += 1;
            }
        }
    }

    pub fn get_action_button(&self, name: &str) -> Option<gtk::Button> {
        self.action_buttons.borrow().iter()
            .find(|(n, _, _)| n == name)
            .map(|(_, btn, _)| btn.clone())
    }

    pub fn set_game(&self, name: &str, cfg: &ConfigManager) {
        *self.current_game.borrow_mut() = Some(name.to_string());

        self.title_label.set_text(name);

        // Load banner (tilde-aware: stored paths may use ~)
        let banner_path = cfg.game_value(name, "Banner").unwrap_or_default();
        if banner_path.is_empty() {
            self.banner_image.set_icon_name(Some("image-x-generic-symbolic"));
        } else if let Some(texture) = helpers::load_texture(&shellexpand_tilde(&banner_path)) {
            self.banner_image.set_paintable(Some(&texture));
        } else {
            self.banner_image.set_icon_name(Some("image-x-generic-symbolic"));
        }

        // Load game icon into the header (tilde-aware)
        let icon_path = cfg.game_value(name, "Icon").unwrap_or_default();
        if icon_path.is_empty() {
            self.icon_image.set_icon_name(Some("application-x-executable-symbolic"));
        } else if let Some(texture) = helpers::load_texture(&shellexpand_tilde(&icon_path)) {
            self.icon_image.set_paintable(Some(&texture));
        } else {
            self.icon_image.set_icon_name(Some("application-x-executable-symbolic"));
        }

        // Time played
        let time_spent: u64 = cfg
            .game_value(name, "TimeSpent")
            .and_then(|v| v.parse().ok())
            .unwrap_or(0);
        if time_spent < 3600 {
            let m = time_spent / 60;
            self.time_label.set_text(&format!("Time played: {} min", m));
        } else {
            let h = time_spent / 3600;
            self.time_label.set_text(&format!("Time played: {} h", h));
        }

        // Favorite — block signal to avoid triggering callback
        if let Some(ref id) = *self.star_handler_id.borrow() {
            self.star_button.block_signal(id);
        }
        let fav = cfg
            .game_value(name, "Favorite")
            .map(|v| v.eq_ignore_ascii_case("true"))
            .unwrap_or(false);
        self.star_button.set_active(fav);
        if let Some(ref id) = *self.star_handler_id.borrow() {
            self.star_button.unblock_signal(id);
        }

        // Install info
        self.set_install_info(name, cfg);

        // Reveal
        self.revealer.set_visible(true);
        self.revealer.set_reveal_child(true);
    }
}

fn shellexpand_tilde(s: &str) -> String {
    if s.starts_with("~/") || s == "~" {
        if let Ok(home) = std::env::var("HOME") {
            return s.replacen("~", &home, 1);
        }
    }
    s.to_string()
}

fn query_folder_size(path: &std::path::Path) -> u64 {
    let mut total = 0;
    if let Ok(entries) = std::fs::read_dir(path) {
        for entry in entries.flatten() {
            let p = entry.path();
            if p.is_file() {
                total += std::fs::metadata(&p).map(|m| m.len()).unwrap_or(0);
            } else if p.is_dir() {
                total += query_folder_size(&p);
            }
        }
    }
    total
}

fn format_size(bytes: u64) -> String {
    if bytes < 1024 { return format!("{} B", bytes); }
    if bytes < 1024 * 1024 { return format!("{:.1} KB", bytes as f64 / 1024.0); }
    if bytes < 1024 * 1024 * 1024 { return format!("{:.1} MB", bytes as f64 / (1024.0 * 1024.0)); }
    format!("{:.2} GB", bytes as f64 / (1024.0 * 1024.0 * 1024.0))
}
