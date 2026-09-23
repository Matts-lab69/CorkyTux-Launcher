use adw::prelude::*;
use gtk::prelude::*;
use std::cell::RefCell;
use std::rc::Rc;

use crate::backend::config::ConfigManager;
use crate::backend::game_model::{GameEntry, GameSource};
use crate::backend::theme::ThemeManager;
use crate::ui::details_panel::DetailsPanel;
use crate::ui::helpers;

const CARD_W: i32 = 200;
const CARD_H: i32 = 140;

pub fn build_game_card(
    entry: &GameEntry,
    config: &ConfigManager,
    details: &Rc<RefCell<Option<DetailsPanel>>>,
    selected_game: &Rc<RefCell<String>>,
    badge: Option<String>,
    on_activate: Option<Rc<dyn Fn()>>,
    show_store_icon: bool,
) -> gtk::Button {
    let card = gtk::Button::new();
    card.add_css_class("game-card");
    card.set_size_request(CARD_W, CARD_H);
    card.set_tooltip_text(Some(&entry.name));
    card.set_overflow(gtk::Overflow::Hidden);

    let img = gtk::Picture::new();
    img.set_size_request(CARD_W, CARD_H);
    img.set_content_fit(gtk::ContentFit::Cover);
    img.set_can_shrink(true);
    img.set_halign(gtk::Align::Fill);
    img.set_valign(gtk::Align::Fill);
    img.set_hexpand(true);
    img.set_vexpand(true);
    if !entry.banner.is_empty() {
        // Route by local-existence, not URL prefix: load_card_banner is
        // local-files only and silently drops anything that is not an
        // existing path (Path::exists), so remote covers of any scheme
        // (https, protocol-relative, ...) must go to the shared download
        // + cache loader (load_cover_async uses download_icon_file, the
        // same cache/retry/webp handling load_mod_icon uses for thumbs).
        let home = std::env::var("HOME").unwrap_or_default();
        let local = entry.banner.replace("~", &home);
        if std::path::Path::new(&local).exists() {
            if let Some(tex) = helpers::load_card_banner(&entry.banner, CARD_W, CARD_H) {
                img.set_paintable(Some(&tex));
            }
        } else {
            crate::ui::minecraft_view::load_cover_async(&entry.banner, &entry.name, &img, CARD_W * 2);
        }
    }

    let strip = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    strip.add_css_class("accent-strip");
    strip.set_halign(gtk::Align::Fill);
    strip.set_valign(gtk::Align::End);

    let name_lbl = gtk::Label::new(Some(&entry.name));
    name_lbl.set_ellipsize(gtk::pango::EllipsizeMode::End);
    name_lbl.set_max_width_chars(22);
    if show_store_icon {
        name_lbl.set_halign(gtk::Align::Start);
        name_lbl.set_hexpand(true);
        let icon = gtk::Image::new();
        icon.set_pixel_size(22);
        // Store-aware fallback (no trademarked brand logos bundled: Epic/GOG
        // artwork is proprietary, unlike GPL Lutris/Heroic): distinct generic
        // symbolic per store instead of one repeated icon.
        let fallback = match entry.source {
            GameSource::Epic => "package-x-generic-symbolic",
            GameSource::Gog => "applications-games-symbolic",
            _ => "application-x-executable-symbolic",
        };
        if !entry.icon.is_empty() {
            if let Some(tex) = helpers::load_texture(&entry.icon) {
                icon.set_paintable(Some(&tex));
            } else {
                icon.set_icon_name(Some(fallback));
            }
        } else {
            icon.set_icon_name(Some(fallback));
        }
        strip.append(&icon);
    } else {
        name_lbl.set_halign(gtk::Align::Center);
        name_lbl.set_hexpand(true);
    }
    strip.append(&name_lbl);

    let overlay = gtk::Overlay::new();
    overlay.set_child(Some(&img));
    overlay.add_overlay(&strip);

    if let Some(text) = badge.as_deref() {
        let b = gtk::Label::new(Some(text));
        b.add_css_class("deal-badge");
        b.set_halign(gtk::Align::Start);
        b.set_valign(gtk::Align::Start);
        b.set_margin_top(8);
        b.set_margin_start(8);
        overlay.add_overlay(&b);
    }

    if let Some(cb) = on_activate {
        card.connect_clicked(move |_| cb());
    } else {
        let details_clone = details.clone();
        let sel = selected_game.clone();
        let config_clone = config.clone();
        let game_name = entry.name.clone();
        card.connect_clicked(move |_| {
            *sel.borrow_mut() = game_name.clone();
            if let Some(ref det) = *details_clone.borrow() {
                det.set_game(&game_name, &config_clone);
            }
        });
    }

    card.set_child(Some(&overlay));
    card
}
