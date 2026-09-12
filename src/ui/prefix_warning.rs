use adw::prelude::*;
use gtk::prelude::*;

pub struct PrefixWarningModal {
    dialog: adw::Dialog,
    games_list: gtk::Box,
}

impl PrefixWarningModal {
    pub fn new() -> Self {
        let dialog = adw::Dialog::new();
        dialog.set_title("Warning");
        dialog.set_content_width(520);
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

        let (header_row, x_btn) = crate::ui::helpers::modal_header("Warning");
        header_row.set_margin_top(4);
        {
            let dlg = dialog.clone();
            x_btn.connect_clicked(move |_| { dlg.close(); });
        }
        inner.append(&header_row);

        let title = gtk::Label::new(Some("The following games have prefix paths that no longer exist:"));
        title.set_halign(gtk::Align::Start);
        title.set_wrap(true);
        title.add_css_class("details-title");
        inner.append(&title);

        // C++ parity: list capped at 220px with scroll (no yellow box,
        // only the small orange dot per row).
        let games_list = gtk::Box::new(gtk::Orientation::Vertical, 6);
        let games_scroll = gtk::ScrolledWindow::new();
        games_scroll.set_min_content_height(80);
        games_scroll.set_max_content_height(220);
        games_scroll.set_vexpand(true);
        games_scroll.set_child(Some(&games_list));
        inner.append(&games_scroll);

        let hint = gtk::Label::new(Some("These games may not launch until you create the prefix manually or update the prefix path in Game Settings."));
        hint.set_halign(gtk::Align::Center);
        hint.set_justify(gtk::Justification::Center);
        hint.set_wrap(true);
        hint.set_opacity(0.6);
        hint.add_css_class("time-label");
        inner.append(&hint);

        let ok_btn = gtk::Button::with_label("OK");
        ok_btn.add_css_class("add-btn");
        ok_btn.set_halign(gtk::Align::Center);
        ok_btn.set_width_request(100);
        let dlg = dialog.clone();
        ok_btn.connect_clicked(move |_| { dlg.close(); });
        inner.append(&ok_btn);

        dialog.set_child(Some(&content));

        Self { dialog, games_list }
    }

    pub fn show_missing(&self, games: &[(String, String)]) {
        // Remove old entries
        while let Some(child) = self.games_list.first_child() {
            self.games_list.remove(&child);
        }
        for (name, prefix) in games {
            let row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
            let dot = gtk::Label::new(Some("●"));
            dot.set_css_classes(&["warn-dot"]);
            dot.set_valign(gtk::Align::Center);
            let col = gtk::Box::new(gtk::Orientation::Vertical, 2);
            let name_lbl = gtk::Label::new(Some(name));
            name_lbl.set_halign(gtk::Align::Start);
            name_lbl.add_css_class("warn-game");
            let prefix_lbl = gtk::Label::new(Some(if prefix.is_empty() { "(no prefix configured)" } else { prefix }));
            prefix_lbl.set_halign(gtk::Align::Start);
            prefix_lbl.set_ellipsize(gtk::pango::EllipsizeMode::Middle);
            prefix_lbl.set_opacity(0.6);
            prefix_lbl.add_css_class("time-label");
            col.append(&name_lbl);
            col.append(&prefix_lbl);
            row.append(&dot);
            row.append(&col);
            self.games_list.append(&row);
        }
    }

    pub fn present(&self, parent: &impl IsA<gtk::Widget>) {
        self.dialog.present(Some(parent));
    }
}
