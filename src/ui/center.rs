use adw::prelude::*;
use gtk::prelude::*;
use std::cell::RefCell;
use std::rc::Rc;

use crate::AppState;
use crate::ui::details_panel::DetailsPanel;
use crate::ui::game_card::build_game_card;

pub struct CenterHandle {
    pub widget: gtk::Box,
    flow: gtk::FlowBox,
    recent_inner: gtk::Box,
    empty_label: gtk::Label,
}

impl Clone for CenterHandle {
    fn clone(&self) -> Self {
        Self {
            widget: self.widget.clone(),
            flow: self.flow.clone(),
            recent_inner: self.recent_inner.clone(),
            empty_label: self.empty_label.clone(),
        }
    }
}

impl CenterHandle {
    pub fn rebuild(&self, state: &AppState, details: &Rc<RefCell<Option<DetailsPanel>>>) {
        self.flow.remove_all();
        let entries = state.recent_model.entries();
        self.empty_label.set_visible(entries.is_empty());
        for entry in &entries {
            let card = build_game_card(entry, &state.config, details, &state.selected_game);
            self.flow.append(&card);
        }
    }
}

pub fn build_center(
    state: &AppState,
    details: &Rc<RefCell<Option<DetailsPanel>>>,
) -> CenterHandle {
    let vbox = gtk::Box::new(gtk::Orientation::Vertical, 0);
    vbox.set_hexpand(true);
    vbox.set_vexpand(true);

    let scroll = gtk::ScrolledWindow::new();
    scroll.set_hscrollbar_policy(gtk::PolicyType::Never);
    scroll.set_has_frame(false);
    scroll.set_overlay_scrolling(true);
    scroll.set_hexpand(true);
    scroll.set_vexpand(true);

    let center_col = gtk::Box::new(gtk::Orientation::Vertical, 24);
    center_col.set_margin_top(24);
    center_col.set_margin_bottom(24);
    center_col.set_margin_start(24);
    center_col.set_margin_end(24);
    center_col.set_halign(gtk::Align::Fill);
    center_col.set_valign(gtk::Align::Fill);
    center_col.set_hexpand(true);
    center_col.set_vexpand(true);

    let recent_frame = gtk::Box::new(gtk::Orientation::Vertical, 0);
    recent_frame.add_css_class("recent-frame");
    recent_frame.set_hexpand(true);
    recent_frame.set_vexpand(true);
    recent_frame.set_halign(gtk::Align::Fill);
    recent_frame.set_valign(gtk::Align::Fill);
    // QML parity: height = max(360, window space) — always full static area
    recent_frame.set_size_request(-1, 360);

    let recent_inner = gtk::Box::new(gtk::Orientation::Vertical, 12);
    recent_inner.set_margin_top(14);
    recent_inner.set_halign(gtk::Align::Fill);
    recent_inner.set_hexpand(true);

    let header = gtk::Label::new(Some("RECENTLY PLAYED"));
    header.add_css_class("recent-label");
    header.set_halign(gtk::Align::Center);
    recent_inner.append(&header);

    let sep = gtk::Separator::new(gtk::Orientation::Horizontal);
    sep.set_margin_top(4);
    sep.set_margin_bottom(4);
    recent_inner.append(&sep);

    let flow = gtk::FlowBox::new();
    flow.set_orientation(gtk::Orientation::Horizontal);
    flow.set_column_spacing(16);
    flow.set_row_spacing(16);
    flow.set_homogeneous(false);
    flow.set_min_children_per_line(1);
    flow.set_max_children_per_line(12);
    flow.set_selection_mode(gtk::SelectionMode::None);
    flow.set_halign(gtk::Align::Center);
    flow.set_valign(gtk::Align::Start);
    flow.set_hexpand(true);

    let entries = state.recent_model.entries();
    let empty_label = gtk::Label::new(Some("Play a game and it will show up here"));
    empty_label.add_css_class("recent-empty");
    empty_label.set_halign(gtk::Align::Center);
    empty_label.set_visible(entries.is_empty());
    recent_inner.append(&empty_label);
    if !entries.is_empty() {
        for entry in &entries {
            let card = build_game_card(entry, &state.config, details, &state.selected_game);
            flow.append(&card);
        }
    }

    recent_inner.append(&flow);
    recent_frame.append(&recent_inner);
    center_col.append(&recent_frame);

    scroll.set_child(Some(&center_col));
    vbox.append(&scroll);

    CenterHandle {
        widget: vbox,
        flow,
        recent_inner,
        empty_label,
    }
}
