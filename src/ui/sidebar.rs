use gtk::prelude::*;
use gtk::{self, pango};
use std::cell::RefCell;
use std::rc::Rc;

use crate::backend::config::ConfigManager;
use crate::backend::plugins::PluginManager;
use crate::backend::theme::ThemeManager;
use crate::ui::helpers;

pub type GameCallback = Rc<RefCell<Option<Box<dyn Fn(String)>>>>;
pub type FilterCallback = Rc<RefCell<Option<Box<dyn Fn(String, String)>>>>;
pub type PageCallback = Rc<RefCell<Option<Box<dyn Fn(String)>>>>;

#[derive(Clone)]
pub struct Sidebar {
    pub widget: gtk::Box,
    pub list_box: Rc<gtk::ListBox>,
    pub search_entry: gtk::SearchEntry,
    current_filter: Rc<RefCell<String>>,
    game_names: Rc<RefCell<Vec<String>>>,
    pub on_game_clicked: GameCallback,
    pub on_filter_changed: FilterCallback,
    config: ConfigManager,
    theme: ThemeManager,
    mc_btn: gtk::Button,
    store_btn: gtk::Button,
    plugins: Rc<RefCell<Option<PluginManager>>>,
}

impl Sidebar {
    pub fn new(cfg: &ConfigManager, theme: &ThemeManager) -> Self {
        return Self::new_with_callbacks(cfg, theme, &Rc::new(RefCell::new(None)), &Rc::new(RefCell::new(None)), &Rc::new(RefCell::new(None)));
    }

    pub fn new_with_callbacks(cfg: &ConfigManager, theme: &ThemeManager, game_cb: &GameCallback, filter_cb: &FilterCallback, page_cb: &PageCallback) -> Self {
        let root = gtk::Box::new(gtk::Orientation::Vertical, 8);
        root.set_width_request(230);
        // Match the center column margins (24/24) so both panels end
        // on the same bottom edge.
        root.set_margin_top(24);
        root.set_margin_start(8);
        root.set_margin_end(8);
        root.set_margin_bottom(24);
        root.add_css_class("sidebar");

        // Plain Box instead of Frame: GtkFrame's own chrome painted a
        // stray gray edge next to our accent border.
        let header_card = gtk::Box::new(gtk::Orientation::Vertical, 0);
        header_card.add_css_class("sidebar-card");
        header_card.set_hexpand(true);
        header_card.set_halign(gtk::Align::Fill);
        let header_card_inner = gtk::Box::new(gtk::Orientation::Vertical, 12);

        let header_label = gtk::Label::new(Some("Your Library"));
        header_label.add_css_class("title-label");
        header_label.set_halign(gtk::Align::Start);
        header_label.set_hexpand(true);
        header_label.set_margin_top(2);
        header_label.set_margin_bottom(2);
        let header_row = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        header_row.append(&header_label);
        // Shared page toggle: mc <-> games, store <-> games, never desynced.
        let page_state = Rc::new(RefCell::new("games".to_string()));
        let mc_btn = gtk::Button::new();
        mc_btn.set_tooltip_text(Some("Minecraft"));
        mc_btn.add_css_class("mc-btn");
        mc_btn.set_valign(gtk::Align::Center);
        let mc_icon = helpers::themed_image("minecraft", theme.is_dark(), 22);
        mc_btn.set_child(Some(&mc_icon));
        {
            let pc = page_cb.clone();
            let st = page_state.clone();
            mc_btn.connect_clicked(move |_| {
                let next = if *st.borrow() == "minecraft" { "games" } else { "minecraft" }.to_string();
                *st.borrow_mut() = next.clone();
                if let Some(f) = pc.borrow().as_ref() {
                    f(next);
                }
            });
        }
        // Entry buttons stay hidden until a plugin manager confirms the
        // matching plugin is installed (see refresh_plugin_buttons).
        mc_btn.set_visible(false);
        header_row.append(&mc_btn);
        let store_btn = gtk::Button::new();
        store_btn.set_tooltip_text(Some("Stores (Epic / GOG)"));
        store_btn.add_css_class("mc-btn");
        store_btn.set_valign(gtk::Align::Center);
        let store_icon = gtk::Image::from_icon_name("system-software-install-symbolic");
        store_icon.set_pixel_size(22);
        store_btn.set_child(Some(&store_icon));
        {
            let pc = page_cb.clone();
            let st = page_state.clone();
            store_btn.connect_clicked(move |_| {
                let next = if *st.borrow() == "stores" { "games" } else { "stores" }.to_string();
                *st.borrow_mut() = next.clone();
                if let Some(f) = pc.borrow().as_ref() {
                    f(next);
                }
            });
        }
        store_btn.set_visible(false);
        header_row.append(&store_btn);
        header_card_inner.append(&header_row);
        let title_sep = gtk::Separator::new(gtk::Orientation::Horizontal);
        title_sep.set_halign(gtk::Align::Fill);
        header_card_inner.append(&title_sep);

        let grid = gtk::Grid::new();
        grid.set_column_homogeneous(true);
        grid.set_row_homogeneous(true);
        grid.set_column_spacing(6);
        grid.set_row_spacing(6);

        let modes = [
            ("All", "all"),
            ("Favorites", "favorites"),
            ("A-Z", "az"),
            ("Most Played", "most_played"),
            ("Recently Added", "recent"),
        ];

        let current_filter = Rc::new(RefCell::new("all".to_string()));
        let mut toggle_buttons = Vec::new();

        for (i, (label, _mode)) in modes.iter().enumerate() {
            let btn = gtk::ToggleButton::with_label(label);
            btn.add_css_class("filter-btn");
            if i < 2 {
                grid.attach(&btn, i as i32, 0, 1, 1);
            } else if i == 2 {
                grid.attach(&btn, 0, 1, 1, 1);
            } else if i == 3 {
                grid.attach(&btn, 1, 1, 1, 1);
            } else {
                grid.attach(&btn, 0, 2, 2, 1);
            }
            toggle_buttons.push(btn);
        }

        for i in 1..toggle_buttons.len() {
            toggle_buttons[i].set_group(Some(&toggle_buttons[0]));
        }

        toggle_buttons[0].set_active(true);

        header_card_inner.append(&grid);

        let search_row = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        let search = gtk::SearchEntry::new();
        search.set_placeholder_text(Some("Search in your library"));
        search.add_css_class("search-entry");
        search.set_hexpand(true);
        search_row.append(&search);
        header_card_inner.append(&search_row);

        header_card.append(&header_card_inner);
        root.append(&header_card);

        let game_list_frame = gtk::Box::new(gtk::Orientation::Vertical, 0);
        game_list_frame.set_vexpand(true);
        game_list_frame.set_hexpand(true);
        game_list_frame.set_halign(gtk::Align::Fill);
        game_list_frame.add_css_class("sidebar-frame");

        let scrolled = gtk::ScrolledWindow::new();
        scrolled.set_vexpand(true);
        scrolled.set_hscrollbar_policy(gtk::PolicyType::Never);
        // No default gray frame around the viewport (that was the stray bar).
        scrolled.set_has_frame(false);
        // Overlay slider instead of the classic gray scrollbar track.
        scrolled.set_overlay_scrolling(true);

        let list = Rc::new(gtk::ListBox::new());
        list.set_selection_mode(gtk::SelectionMode::Single);

        let game_names: Rc<RefCell<Vec<String>>> = Rc::new(RefCell::new(Vec::new()));

        let names = cfg.game_names();

        scrolled.set_child(Some(&*list));
        game_list_frame.append(&scrolled);
        root.append(&game_list_frame);

        let on_game_clicked = game_cb.clone();
        let on_filter_changed = filter_cb.clone();

        // Connect row-selected on the ListBox (single click!)
        {
            let cb = on_game_clicked.clone();
            let gn = game_names.clone();
            list.connect_row_selected(move |_, row_opt| {
                if let Some(row) = row_opt {
                    let idx = row.index() as usize;
                    let names = gn.borrow();
                    if let Some(name) = names.get(idx) {
                        if let Some(f) = cb.borrow().as_ref() {
                            f(name.clone());
                        }
                    }
                }
            });
        }

        // Populate initial rows
        for name in &names {
            let row = Self::build_game_row(name, cfg, theme);
            list.append(&row);
            game_names.borrow_mut().push(name.clone());
        }

        // Connect filter buttons
        let cf = current_filter.clone();
        let ofc = on_filter_changed.clone();
        for (i, mode) in modes.iter().map(|m| m.1).enumerate() {
            let cf2 = cf.clone();
            let ofc2 = ofc.clone();
            toggle_buttons[i].connect_toggled(move |b| {
                if b.is_active() {
                    *cf2.borrow_mut() = mode.to_string();
                    let search = String::new();
                    if let Some(f) = ofc2.borrow().as_ref() {
                        f(mode.to_string(), search);
                    }
                }
            });
        }

        // Connect search — delegate entirely to filter callback
        let on_filter_changed_final = on_filter_changed.clone();
        let cf_final = current_filter.clone();
        search.connect_search_changed(move |entry| {
            let query = entry.text().to_string();
            let filter = cf_final.borrow().clone();
            if let Some(f) = on_filter_changed_final.borrow().as_ref() {
                f(filter, query);
            }
        });

        Self {
            widget: root,
            list_box: list,
            search_entry: search,
            current_filter,
            game_names,
            on_game_clicked,
            on_filter_changed,
            config: cfg.clone(),
            theme: theme.clone(),
            mc_btn: mc_btn.clone(),
            store_btn: store_btn.clone(),
            plugins: Rc::new(RefCell::new(None)),
        }
    }

    /// Attach the plugin manager and show/hide the Minecraft + Stores
    /// entry buttons depending on installed plugins.
    pub fn set_plugin_manager(&self, pm: &PluginManager) {
        *self.plugins.borrow_mut() = Some(pm.clone());
        self.refresh_plugin_buttons();
    }

    /// Show each entry button only when its plugin is installed
    /// (minecraft-launcher / heroic-store) and enabled.
    pub fn refresh_plugin_buttons(&self) {
        let pm = self.plugins.borrow().clone();
        match pm {
            Some(p) => {
                p.refresh();
                self.mc_btn.set_visible(p.is_available("minecraft-launcher"));
                self.store_btn.set_visible(p.is_available("heroic-store"));
            }
            None => {
                self.mc_btn.set_visible(false);
                self.store_btn.set_visible(false);
            }
        }
    }

    pub fn set_callbacks(&self, on_game_clicked: GameCallback, on_filter_changed: FilterCallback) {
        *self.on_game_clicked.borrow_mut() = on_game_clicked.borrow_mut().take();
        *self.on_filter_changed.borrow_mut() = on_filter_changed.borrow_mut().take();
    }

    fn build_game_row(name: &str, cfg: &ConfigManager, _theme: &ThemeManager) -> gtk::ListBoxRow {
        let row = gtk::ListBoxRow::new();
        row.add_css_class("game-row");
        let hbox = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        hbox.set_margin_top(4);
        hbox.set_margin_bottom(4);
        hbox.set_margin_start(8);
        hbox.set_margin_end(8);

        let icon = gtk::Image::new();
        icon.set_pixel_size(24);
        if let Some(icon_path) = cfg.game_value(name, "Icon") {
            if !icon_path.is_empty() {
                if let Some(texture) = helpers::load_texture(&icon_path) {
                    icon.set_paintable(Some(&texture));
                } else {
                    icon.set_icon_name(Some("application-x-executable-symbolic"));
                }
            } else {
                icon.set_icon_name(Some("application-x-executable-symbolic"));
            }
        } else {
            icon.set_icon_name(Some("application-x-executable-symbolic"));
        }

        let label = gtk::Label::new(Some(name));
        label.set_halign(gtk::Align::Start);
        label.set_ellipsize(pango::EllipsizeMode::End);
        label.set_max_width_chars(20);

        hbox.append(&icon);
        hbox.append(&label);
        row.set_child(Some(&hbox));
        row
    }

    pub fn get_filter(&self) -> String {
        self.current_filter.borrow().clone()
    }

    pub fn get_search_query(&self) -> String {
        self.search_entry.text().to_string()
    }

    pub fn refresh_list(&self, names: &[String]) {
        self.refresh_plugin_buttons();
        self.list_box.remove_all();
        self.game_names.borrow_mut().clear();
        for name in names {
            let row = Self::build_game_row(name, &self.config, &self.theme);
            self.list_box.append(&row);
            self.game_names.borrow_mut().push(name.clone());
        }
    }

    /// Re-apply the active filter + search text (imports/rebuilds must not
    /// silently reset the list to "All").
    pub fn apply_current_filter(&self) {
        if let Some(f) = self.on_filter_changed.borrow().as_ref() {
            f(self.current_filter.borrow().clone(), self.search_entry.text().to_string());
        }
    }

    pub fn select_game(&self, name: &str) {
        let names = self.game_names.borrow();
        if let Some(idx) = names.iter().position(|n| n == name) {
            self.list_box
                .select_row(self.list_box.row_at_index(idx as i32).as_ref());
        }
    }
}
