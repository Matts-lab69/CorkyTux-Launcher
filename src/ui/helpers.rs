use adw::prelude::*;
use gtk::gdk;
use gtk::glib;
use gtk::prelude::*;
use std::cell::RefCell;

use crate::backend::theme::ThemeManager;

thread_local! {
    static MAIN_CSS_PROVIDER: RefCell<Option<gtk::CssProvider>> = RefCell::new(None);
    static ACCENT_CSS_PROVIDER: RefCell<Option<gtk::CssProvider>> = RefCell::new(None);
    static THEMED_IMAGES: RefCell<Vec<(glib::WeakRef<gtk::Image>, String)>> = RefCell::new(Vec::new());
    // Bounded RAM cache: every library re-render used to re-decode the same
    // PNGs from disk. Key includes mtime+size so replaced files never go stale.
    static TEX_CACHE: RefCell<(std::collections::HashMap<String, gdk::Texture>, Vec<String>)> =
        RefCell::new((std::collections::HashMap::new(), Vec::new()));
}

const TEX_CACHE_MAX: usize = 64;

fn tex_cache_key(path: &str) -> Option<String> {
    let m = std::fs::metadata(path).ok()?;
    let mt = m.modified().ok()?.duration_since(std::time::UNIX_EPOCH).ok()?.as_secs();
    Some(format!("{}:{}:{}", path, mt, m.len()))
}

fn tex_cache_get(key: &str) -> Option<gdk::Texture> {
    TEX_CACHE.with(|c| c.borrow().0.get(key).cloned())
}

/// Already-decoded texture without touching disk decoders again.
/// Returns None when not cached (caller falls back to load_texture).
pub fn texture_if_cached(path: &str) -> Option<gdk::Texture> {
    let key = tex_cache_key(path)?;
    tex_cache_get(&key)
}

fn tex_cache_put(key: String, tex: &gdk::Texture) {
    TEX_CACHE.with(|c| {
        let (map, order) = &mut *c.borrow_mut();
        if map.len() >= TEX_CACHE_MAX {
            map.clear();
            order.clear();
        }
        order.push(key.clone());
        map.insert(key, tex.clone());
    });
}

pub fn parse_rgba(color: &str) -> gdk::RGBA {
    let color = color.trim();
    if color.starts_with('#') {
        let hex = color.trim_start_matches('#');
        let (r, g, b) = match hex.len() {
            6 => (
                u8::from_str_radix(&hex[0..2], 16).unwrap_or(0),
                u8::from_str_radix(&hex[2..4], 16).unwrap_or(0),
                u8::from_str_radix(&hex[4..6], 16).unwrap_or(0),
            ),
            _ => (0, 0, 0),
        };
        return gdk::RGBA::new(r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0, 1.0);
    }
    gdk::RGBA::new(0.0, 0.0, 0.0, 1.0)
}

pub fn load_texture(path: &str) -> Option<gdk::Texture> {
    let path = path.replace("~", &std::env::var("HOME").unwrap_or_default());
    if !std::path::Path::new(&path).exists() {
        return None;
    }
    if let Some(key) = tex_cache_key(&path) {
        if let Some(tex) = tex_cache_get(&key) {
            return Some(tex);
        }
        // Fast path: PNG/JPEG/etc. straight to texture.
        if let Ok(tex) = gdk::Texture::from_filename(&path) {
            tex_cache_put(key, &tex);
            return Some(tex);
        }
        // Fallback chain: pixbuf loaders, then manual ICO extraction.
        if let Some(pb) = load_pixbuf(&path) {
            let tex = gdk::Texture::for_pixbuf(&pb);
            tex_cache_put(key, &tex);
            return Some(tex);
        }
        return None;
    }
    if let Ok(tex) = gdk::Texture::from_filename(&path) {
        return Some(tex);
    }
    load_pixbuf(&path).map(|pb| gdk::Texture::for_pixbuf(&pb))
}

/// Full pixbuf load chain: gdk-pixbuf loaders (PNG, JPEG, ICO, BMP,
/// GIF, WebP...) plus manual extraction of PNG-compressed entries out
/// of Windows .ico containers (pixbuf's ico loader rejects those with
/// "Compressed icons are not supported").
fn load_pixbuf(path: &str) -> Option<gdk_pixbuf::Pixbuf> {
    if let Ok(file) = std::fs::File::open(path) {
        if let Ok(pb) = gdk_pixbuf::Pixbuf::from_read(file) {
            return Some(pb);
        }
    }
    load_ico_pixbuf(path)
}

/// Scaled-down texture for small previews (game settings icon/banner).
/// Never returns an oversized image: everything is scaled to fit the box.
pub fn load_preview(path: &str, max_w: i32, max_h: i32) -> Option<gdk::Texture> {
    if !std::path::Path::new(path).exists() {
        return None;
    }
    let pb = load_pixbuf(path)?;
    let (w, h) = (pb.width(), pb.height());
    if w <= max_w && h <= max_h {
        return Some(gdk::Texture::for_pixbuf(&pb));
    }
    let scale = (max_w as f32 / w as f32).min(max_h as f32 / h as f32);
    let nw = ((w as f32 * scale) as i32).max(1);
    let nh = ((h as f32 * scale) as i32).max(1);
    pb.scale_simple(nw, nh, gdk_pixbuf::InterpType::Bilinear)
        .map(|s| gdk::Texture::for_pixbuf(&s))
}

/// Load a banner image with QML PreserveAspectCrop parity: cover-scale
/// preserving aspect ratio, then center-crop to exactly target_w x target_h.
pub fn load_card_banner(path: &str, target_w: i32, target_h: i32) -> Option<gdk::Texture> {
    let home = std::env::var("HOME").unwrap_or_default();
    let path = path.replace("~", &home);
    if !std::path::Path::new(&path).exists() {
        return None;
    }
    let pb = load_pixbuf(&path)?;
    let (w, h) = (pb.width(), pb.height());
    if w <= 0 || h <= 0 {
        return None;
    }
    // Cover scale: smallest scale that fills the whole target box.
    let scale = (target_w as f32 / w as f32).max(target_h as f32 / h as f32);
    let nw = ((w as f32 * scale).round() as i32).max(1);
    let nh = ((h as f32 * scale).round() as i32).max(1);
    let scaled = if nw == w && nh == h {
        pb
    } else {
        pb.scale_simple(nw, nh, gdk_pixbuf::InterpType::Bilinear)?
    };
    // Center-crop to the exact target size.
    if scaled.width() == target_w && scaled.height() == target_h {
        return Some(gdk::Texture::for_pixbuf(&scaled));
    }
    let dest = gdk_pixbuf::Pixbuf::new(
        scaled.colorspace(),
        scaled.has_alpha(),
        scaled.bits_per_sample(),
        target_w,
        target_h,
    )?;
    let ox = ((scaled.width() - target_w) / 2).max(0);
    let oy = ((scaled.height() - target_h) / 2).max(0);
    scaled.copy_area(ox, oy, target_w, target_h, &dest, 0, 0);
    Some(gdk::Texture::for_pixbuf(&dest))
}

/// Parse a Windows .ico container and decode its largest PNG entry.
fn load_ico_pixbuf(path: &str) -> Option<gdk_pixbuf::Pixbuf> {
    let data = std::fs::read(path).ok()?;
    if data.len() < 6 || data[0..4] != [0, 0, 1, 0] {
        return None;
    }
    let count = u16::from_le_bytes([data[4], data[5]]) as usize;
    let mut best: Option<(u32, usize, usize)> = None;
    for i in 0..count.min(64) {
        let off = 6 + i * 16;
        if data.len() < off + 16 {
            break;
        }
        let w = data[off] as u32;
        let h = data[off + 1] as u32;
        let size = u32::from_le_bytes([
            data[off + 8],
            data[off + 9],
            data[off + 10],
            data[off + 11],
        ]) as usize;
        let ofs = u32::from_le_bytes([
            data[off + 12],
            data[off + 13],
            data[off + 14],
            data[off + 15],
        ]) as usize;
        if ofs + size > data.len() || size < 8 {
            continue;
        }
        if data[ofs..].starts_with(&[0x89, b'P', b'N', b'G']) {
            let wv = if w == 0 { 256 } else { w };
            let hv = if h == 0 { 256 } else { h };
            let area = wv * hv;
            if best.map(|(a, _, _)| area > a).unwrap_or(true) {
                best = Some((area, ofs, size));
            }
        }
    }
    let (_, ofs, len) = best?;
    let loader = gdk_pixbuf::PixbufLoader::with_type("png").ok()?;
    loader.write(&data[ofs..ofs + len]).ok()?;
    loader.close().ok()?;
    loader.pixbuf()
}

pub fn load_themed_icon(name: &str, is_dark: bool) -> Option<gdk::Texture> {
    let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".to_string());
    let suffix = if is_dark { ".png" } else { "_dark.png" };
    let path = format!("{}/.local/share/corkytux/assets/{}{}", home, name, suffix);
    // Fall back to the base asset when the theme variant is missing
    // (e.g. corkytux has no _dark.png), then to a symbolic icon.
    load_texture(&path).or_else(|| load_texture(&asset_path(name)))
}

/// Build a theme-aware image: its paintable follows Dark/Light switches
/// via refresh_themed_icons() (C++ Theme.icon binding parity).
pub fn themed_image(name: &str, is_dark: bool, pixel_size: i32) -> gtk::Image {
    let img = gtk::Image::new();
    img.set_pixel_size(pixel_size);
    set_themed_paintable(&img, name, is_dark);
    THEMED_IMAGES.with(|v| v.borrow_mut().push((img.downgrade(), name.to_string())));
    img
}

fn set_themed_paintable(img: &gtk::Image, name: &str, is_dark: bool) {
    if let Some(tex) = load_themed_icon(name, is_dark) {
        img.set_paintable(Some(&tex));
    } else {
        img.set_icon_name(Some("image-x-generic-symbolic"));
    }
}

/// Reload every registered themed image after a Dark/Light switch.
pub fn refresh_themed_icons(is_dark: bool) {
    THEMED_IMAGES.with(|v| {
        let mut live = Vec::new();
        for (w, name) in v.borrow().iter() {
            if let Some(img) = w.upgrade() {
                set_themed_paintable(&img, name, is_dark);
                live.push((w.clone(), name.clone()));
            }
        }
        *v.borrow_mut() = live;
    });
}

pub fn asset_path(name: &str) -> String {
    let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".to_string());
    format!("{}/.local/share/corkytux/assets/{}.png", home, name)
}

pub fn accent_strip_color(theme: &ThemeManager) -> String {
    let hex = theme.accent_color();
    let hex = hex.trim_start_matches('#');
    if hex.len() < 6 {
        return "rgba(0,0,0,0.6)".to_string();
    }
    let r = u8::from_str_radix(&hex[0..2], 16).unwrap_or(0) as f32;
    let g = u8::from_str_radix(&hex[2..4], 16).unwrap_or(0) as f32;
    let b = u8::from_str_radix(&hex[4..6], 16).unwrap_or(0) as f32;
    let luminance = (0.299 * r + 0.587 * g + 0.114 * b) / 255.0;
    let alpha = (0.75 - luminance * 0.30).clamp(0.3, 0.8);
    format!("rgba({},{},{},{:.2})", r as u8, g as u8, b as u8, alpha)
}

pub fn init_accent_provider(theme: &ThemeManager) {
    let display = match gdk::Display::default() {
        Some(d) => d,
        None => return,
    };

    ACCENT_CSS_PROVIDER.with(|cell| {
        let mut borrow = cell.borrow_mut();
        if let Some(provider) = borrow.take() {
            gtk::style_context_remove_provider_for_display(&display, &provider);
        }

        let provider = gtk::CssProvider::new();
        let accents = crate::backend::theme::all_accents();
        let current_id = theme.accent_id();
        let mut css = String::new();
        for accent in &accents {
            let checked_border = if accent.id == current_id { "3px" } else { "2px" };
            css.push_str(&format!(
                ".accent-swatch-{} {{ background-color: {}; color: #FFFFFF; border-radius: 8px; min-width: 84px; min-height: 28px; font-weight: bold; font-size: 11px; border: {} solid transparent; }}\
                 .accent-swatch-{}:checked {{ border-color: #FFFFFF; border-width: 3px; }}",
                accent.name, accent.hex, checked_border, accent.name,
            ));
        }
        provider.load_from_string(&css);
        gtk::style_context_add_provider_for_display(
            &display,
            &provider,
            gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
        );
        *borrow = Some(provider);
    });
}

/// X close button for dialogs (accent colored via .close-btn).
/// The caller must connect the actual close action.
/// Centered modal message, transient to the launcher window: fixes
/// parentless popups appearing as separate background windows, and wires
/// OK to actually close (MessageDialog does not auto-close on response).
pub fn present_msg(parent_widget: &impl IsA<gtk::Widget>, heading: &str, body: &str) {
    let dlg = adw::MessageDialog::new(None::<&adw::ApplicationWindow>, Some(heading), Some(body));
    if let Some(root) = parent_widget.root() {
        if let Ok(win) = root.downcast::<gtk::Window>() {
            dlg.set_transient_for(Some(&win));
            dlg.set_modal(true);
        }
    }
    dlg.add_response("ok", "OK");
    dlg.connect_response(None, move |d, _| {
        d.close();
    });
    dlg.present();
}

pub fn dialog_x_button() -> gtk::Button {
    let btn = gtk::Button::with_label("✕");
    btn.add_css_class("close-btn");
    btn.set_halign(gtk::Align::End);
    btn.set_valign(gtk::Align::Start);
    btn.set_width_request(32);
    btn.set_height_request(32);
    btn.set_tooltip_text(Some("Close"));
    btn
}

/// Modal title header matching the launcher reference style:
/// bold title on the left, accent X on the right, gray divider below.
/// Returns the container and the X button (caller connects close).
pub fn modal_header(title: &str) -> (gtk::Box, gtk::Button) {
    let outer = gtk::Box::new(gtk::Orientation::Vertical, 6);
    outer.set_halign(gtk::Align::Fill);
    let row = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    row.set_halign(gtk::Align::Fill);
    row.set_valign(gtk::Align::Start);
    let lbl = gtk::Label::new(Some(title));
    lbl.add_css_class("modal-title");
    lbl.set_halign(gtk::Align::Start);
    lbl.set_valign(gtk::Align::Center);
    lbl.set_hexpand(true);
    row.append(&lbl);
    let x = dialog_x_button();
    x.set_valign(gtk::Align::Center);
    row.append(&x);
    outer.append(&row);
    let sep = gtk::Separator::new(gtk::Orientation::Horizontal);
    sep.set_halign(gtk::Align::Fill);
    outer.append(&sep);
    (outer, x)
}

/// Small icon button (folder, etc.) using themed PNG assets.
pub fn icon_button(name: &str, is_dark: bool, tooltip: &str) -> gtk::Button {
    let btn = gtk::Button::new();
    btn.set_tooltip_text(Some(tooltip));
    btn.set_width_request(36);
    let img = themed_image(name, is_dark, 16);
    btn.set_child(Some(&img));
    btn
}

pub fn apply_theme_css(theme: &ThemeManager) {
    let display = match gdk::Display::default() {
        Some(d) => d,
        None => return,
    };

    // Force Adwaita color scheme so adw::Dialog gets dark surface
    let sm = adw::StyleManager::for_display(&display);
    if theme.is_dark() {
        sm.set_color_scheme(adw::ColorScheme::PreferDark);
    } else {
        sm.set_color_scheme(adw::ColorScheme::PreferLight);
    }

    MAIN_CSS_PROVIDER.with(|cell| {
        let mut borrow = cell.borrow_mut();
        if let Some(provider) = borrow.take() {
            gtk::style_context_remove_provider_for_display(&display, &provider);
        }

        let provider = gtk::CssProvider::new();
        let accent = theme.accent_color();
        let bg = theme.bg();
        let panel = theme.panel();
        let card = theme.card();
        let well = theme.well();
        let border = theme.border();
        let hover = theme.hover();
        let text_main = theme.text_main();
        let text_sec = theme.text_sec();
        let text_muted = theme.text_muted();
        let is_dark = theme.is_dark();

        let strip_color = accent_strip_color(theme);
        let banner_bg = if is_dark { "#282828" } else { "#E9ECEF" };
        let tab_bar_bg = if is_dark { "#181818" } else { "#FFFFFF" };
        let search_bg = if is_dark { "#242424" } else { "#F1F3F5" };
        let game_border = if is_dark { "transparent" } else { &border };
        let game_border_width = if is_dark { "0" } else { "1" };
        let action_bg = if is_dark { "#242424" } else { "#F1F3F5" };
        let dialog_bg = if is_dark { "#1E1E1E" } else { "#FFFFFF" };
        let warn_dot = if is_dark { "#FFA726" } else { "#FB8C00" };

        let css = format!(
            "window.background {{ background-color: {bg}; color: {text_main}; }}\
             dialog.background {{ background-color: {dialog_bg}; color: {text_main}; border-radius: 12px; border: 1px solid {border}; }}\
             .top-bar {{ background-color: {bg}; min-height: 52px; padding: 0 16px; }}\
             .sidebar {{ background-color: {bg}; min-width: 230px; }}\
            .sidebar-card {{ border: 1px solid {accent}; border-radius: 8px; background-color: transparent; padding: 8px; }}\
            .sidebar-frame {{ border: 1px solid {accent}; border-radius: 10px; background-color: transparent; padding: 4px; }}\
             .filter-btn {{ background-color: {well}; color: {text_main}; border: 1px solid transparent; border-radius: 8px; padding: 6px 12px; font-weight: bold; font-size: 12px; min-height: 30px; }}\
             .filter-btn:hover {{ background-color: {hover}; }}\
             .filter-btn:checked {{ background-color: {hover}; color: {accent}; border-color: {accent}; font-weight: bold; }}\
             .search-entry {{ background-color: {search_bg}; border: 1px solid transparent; border-radius: 16px; padding: 4px 12px; font-size: 12px; min-height: 32px; color: {text_main}; }}\
             .search-entry placeholder {{ color: {text_muted}; }}\
             .game-row {{ padding: 4px 8px; border-radius: 6px; min-height: 32px; }}\
             .game-row:hover {{ background-color: {accent}; }}\
             .game-row:hover label {{ color: #000000; }}\
             .game-row label {{ color: {text_main}; font-size: 12px; }}\
             .details-panel {{ background-color: {panel}; border-left: 1px solid {accent}; padding: 12px; min-width: 270px; border-radius: 0; }}\
             .banner-frame {{ background-color: {banner_bg}; border-radius: 8px; min-height: 200px; }}\
             .details-title {{ font-weight: bold; font-size: 16px; color: {text_main}; }}\
             .play-btn {{ background-color: {accent}; color: #FFFFFF; font-weight: bold; border-radius: 8px; min-height: 44px; font-size: 14px; border: none; }}\
             .play-btn:hover {{ opacity: 0.85; }}\
             .info-card {{ background-color: {panel}; border: 1px solid {border}; border-radius: 6px; padding: 8px; }}\
             .info-label {{ color: {text_muted}; font-size: 11px; font-weight: bold; }}\
            .frame-title {{ color: {text_main}; font-size: 12px; font-weight: bold; }}\
            .path-entry {{ min-height: 28px; font-size: 12px; padding: 2px 8px; }}\
            .log-view, .log-view text {{ background-color: {well}; color: {text_main}; caret-color: {accent}; }}\
            .log-view {{ border: 1px solid {border}; border-radius: 8px; }}\
             .info-value {{ color: {text_main}; font-size: 13px; }}\
            .action-btn {{ background-color: {action_bg}; color: {text_main}; border: 1px solid transparent; border-radius: 18px; min-height: 36px; font-size: 11px; font-weight: bold; }}\
            .action-btn:hover {{ background-color: {hover}; }}\
            .actions-frame {{ border: 1px solid {accent}; border-radius: 8px; background-color: transparent; padding: 8px; }}\
            .neon-red {{ background-color: #FF0040; color: #FFFFFF; font-weight: bold; border-radius: 20px; min-height: 36px; font-size: 14px; padding: 0 16px; border: none; }}\
            .neon-green {{ background-color: #00E639; color: #FFFFFF; font-weight: bold; border-radius: 20px; min-height: 36px; font-size: 14px; padding: 0 16px; border: none; }}\
            switch {{ background-color: {well}; border: 1px solid {border}; border-radius: 16px; }}\
            switch:checked {{ background-color: {accent}; border-color: {accent}; }}\
            switch:checked > slider {{ background-color: #FFFFFF; }}\
            .emu-dot-on {{ color: #00E639; font-size: 14px; }}\
            .emu-dot-off {{ color: {text_muted}; font-size: 14px; }}\
            .warn-dot {{ color: {warn_dot}; font-size: 14px; }}\
            .warn-game {{ color: {text_main}; font-weight: bold; font-size: 13px; }}\
             .recent-frame {{ border: 1px solid {accent}; border-radius: 10px; background-color: transparent; padding: 16px; }}\
             .recent-label {{ color: {text_main}; font-weight: bold; font-size: 24px; }}\
             .recent-empty {{ color: {text_sec}; font-size: 12px; }}\
             scrollbar.vertical trough {{ background: none; background-color: transparent; background-image: none; border: none; box-shadow: none; outline: none; }}\
             scrollbar.horizontal trough {{ background: none; background-color: transparent; background-image: none; border: none; box-shadow: none; outline: none; }}\
             .game-card {{ background-color: {card}; border-radius: 22px; min-width: 200px; min-height: 140px; border: {game_border_width}px solid {game_border}; padding: 0; }}\
             .game-card:hover {{ background-color: {hover}; }}\
             .accent-strip {{ background-color: {strip_color}; border-radius: 0 0 20px 20px; padding: 6px 10px; }}\
             .accent-strip label {{ color: #FFFFFF; font-weight: bold; font-size: 12px; }}\
             .star-btn {{ color: {text_muted}; background: transparent; border: none; font-size: 20px; }}\
             .star-btn:checked {{ color: #FFD700; }}\
             .close-btn {{ background: transparent; border: none; color: {accent}; font-size: 16px; font-weight: bold; }}\
             .close-btn:hover {{ opacity: 0.75; }}\
             .proton-path-badge {{ color: {accent}; border: 1px solid {accent}; border-radius: 10px; padding: 2px 8px; font-size: 11px; font-weight: bold; }}\
             .modal-title {{ color: {text_main}; font-size: 16px; font-weight: bold; }}\
             .cand-list {{ background-color: {well}; border-radius: 8px; padding: 6px; }}\
             .card-selected {{ background-color: {accent}; }}\
             .card-selected label {{ color: #000000; }}\
             .destructive-action {{ border-radius: 20px; min-height: 36px; font-size: 14px; padding: 0 16px; font-weight: bold; }}
             .mc-tile {{ border-radius: 14px; }}
             .mc-tile:hover {{ border-color: {accent}; }}
             .mc-head {{ background-color: {panel}; border: 1px solid {border}; border-radius: 14px; padding: 6px 10px; }}
             .mc-account {{ background-color: transparent; border: none; padding: 4px 8px; border-radius: 10px; }}
             .mc-account:hover {{ background-color: {hover}; }}
             .mc-account-label {{ font-weight: bold; font-size: 13px; color: {text_main}; }}
             .mc-row {{ background-color: {panel}; border: 1px solid {border}; border-radius: 10px; padding: 8px 10px; }}
             .mc-row:hover {{ border-color: {accent}; }}
             .icon-ghost {{ background: transparent; border: none; padding: 0; min-height: 0; min-width: 0; }}
             .icon-ghost:hover {{ opacity: 0.7; }}\
             .dark-btn {{ background-color: #000000; color: #FFFFFF; border: 1.5px solid #FFFFFF; border-radius: 20px; min-height: 36px; font-size: 14px; padding: 0 16px; font-weight: bold; }}\
            .title-label {{ font-weight: bold; font-size: 16px; color: {text_main}; }}\
             .time-label {{ color: {text_sec}; font-size: 12px; }}\
             .add-btn {{ font-size: 14px; min-height: 36px; padding: 0 16px; border-radius: 20px; background-color: {accent}; color: #FFFFFF; border: none; font-weight: bold; }}\
             .add-btn:hover {{ opacity: 0.85; }}\
             .settings-btn {{ font-size: 14px; min-height: 36px; padding: 0 16px; border-radius: 20px; background-color: transparent; color: {text_main}; border: 1.5px solid {accent}; font-weight: bold; }}\
             .settings-btn:hover {{ background-color: {hover}; }}\
            .settings-title {{ color: {text_main}; font-size: 18px; font-weight: bold; }}\
            .settings-tab-bar {{ background-color: {tab_bar_bg}; border: 1px solid {border}; border-radius: 14px; min-height: 44px; }}\
            .settings-tab {{ background-color: transparent; color: {text_sec}; border: none; border-radius: 0; padding: 4px 6px; font-size: 11px; font-weight: bold; min-height: 38px; min-width: 64px; }}\
             .settings-tab:hover {{ color: {text_main}; }}\
             .settings-tab:checked {{ color: {accent}; background-color: transparent; }}\
             .settings-tab-indicator {{ background-color: {accent}; border-radius: 2px; min-height: 3px; min-width: 24px; }}\
             .page-card {{ background-color: {panel}; border: 1px solid {border}; border-radius: 8px; padding: 14px; }}\
             .modal-bg {{ background-color: {panel}; border-radius: 12px; border: 1px solid {border}; }}\
             tooltip.background {{ background-color: {tab_bar_bg}; border-radius: 8px; border: 1px solid {border}; padding: 12px; }}\
             tooltip label {{ color: {text_main}; font-size: 13px; }}"
        );

        provider.load_from_string(&css);
        gtk::style_context_add_provider_for_display(
            &display,
            &provider,
            gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
        );
        *borrow = Some(provider);
    });
}
