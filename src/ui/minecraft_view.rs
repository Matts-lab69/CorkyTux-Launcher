use adw::prelude::*;
use gtk::prelude::*;
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use crate::AppState;
use crate::ui::helpers;
use crate::backend::external::MinecraftManager;

// GDLauncher-Carbon order, CorkyTux style, PineconeMC/mll logic.
// Library (tiles grid) <-> Detail (Overview/Addons/Logs/Settings).
// Instances are isolated dirs; legacy global installs show as Shared.

#[derive(Clone, Default)]
struct Inst {
    id: String,
    mc_ver: String,
    loader: String,
    loader_ver: String,
    isolated: bool,
    fav: bool,
    secs: u64,
    last: String,
    disp: String,
}

#[derive(Clone, Default)]
struct McData {
    accounts: Vec<(String, String, bool, bool)>,
    versions: Vec<(String, String)>,
    installed: Vec<Inst>,
    java_found: Vec<(String, String, String)>,
    java_selected: String,
    deps_ok: bool,
    azure_configured: bool,
}

#[derive(Clone, Default)]
struct AddonRow {
    file: String,
    title: String,
    version: String,
    platform: String,
    enabled: bool,
    icon_url: String,
    project_id: String,
}

fn mc_root() -> std::path::PathBuf {
    std::path::PathBuf::from(std::env::var("HOME").unwrap_or_default())
        .join(".config/CorkyTux/plugins/minecraft-launcher")
}

fn instances_root() -> std::path::PathBuf {
    mc_root().join("mc-instances")
}

fn legacy_dir() -> std::path::PathBuf {
    mc_root().join("instances")
}

fn inst_dir(id: &str, isolated: bool) -> std::path::PathBuf {
    if isolated {
        instances_root().join(safe_id(id))
    } else {
        legacy_dir()
    }
}

fn safe_id(id: &str) -> String {
    id.chars().map(|c| if c.is_ascii_alphanumeric() || c == '.' || c == '-' || c == '_' { c } else { '_' }).collect()
}

fn is_mc_ver(s: &str) -> bool {
    let mut parts = s.split('.');
    match (parts.next(), parts.next(), parts.next(), parts.next()) {
        (Some(a), Some(b), None, None) | (Some(a), Some(b), Some(_), None) => {
            a.chars().all(|c| c.is_ascii_digit()) && b.chars().all(|c| c.is_ascii_digit()) && !a.is_empty() && !b.is_empty()
        }
        _ => false,
    }
}

fn parse_inst_id(id: &str) -> (String, String, String) {
    // scheme A (mll fabric-style): "<loader>-loader-<lver>-<mc>"
    for loader in ["neoforge", "fabric", "forge", "quilt"] {
        let prefix = format!("{}-loader-", loader);
        if let Some(rest) = id.strip_prefix(&prefix) {
            // mc is the trailing version: split off trailing numeric runs
            if let Some(pos) = rest.rfind('-') {
                let (lver, mc) = (&rest[..pos], &rest[pos + 1..]);
                if is_mc_ver(mc) && !lver.is_empty() {
                    return (mc.to_string(), loader.to_string(), lver.to_string());
                }
            }
        }
    }
    // scheme B (mod_loader-style): "<mc>-<loader>-<lver>"
    for loader in ["neoforge", "fabric", "forge", "quilt"] {
        let marker = format!("-{}-", loader);
        if let Some(pos) = id.find(&marker) {
            let (mc, lver) = (&id[..pos], &id[pos + marker.len()..]);
            if is_mc_ver(mc) && !lver.is_empty() {
                return (mc.to_string(), loader.to_string(), lver.to_string());
            }
        }
    }
    (id.to_string(), "vanilla".to_string(), String::new())
}

fn required_java_major(ver: &str) -> u32 {
    let mut parts = ver.split('.');
    let major: u32 = parts.next().and_then(|s| s.parse().ok()).unwrap_or(0);
    let minor: u32 = parts.next().and_then(|s| s.parse().ok()).unwrap_or(0);
    let patch: u32 = parts.next().and_then(|s| s.parse().ok()).unwrap_or(0);
    if major != 1 {
        return 21;
    }
    if minor < 17 {
        return 8;
    }
    if minor == 17 {
        return 17;
    }
    if minor <= 20 {
        return 17;
    }
    21
}

fn max_java_major(ver: &str) -> u32 {
    let mut parts = ver.split('.');
    let major: u32 = parts.next().and_then(|s| s.parse().ok()).unwrap_or(0);
    let minor: u32 = parts.next().and_then(|s| s.parse().ok()).unwrap_or(0);
    if major == 1 && minor <= 16 {
        return 17;
    }
    99
}

fn java_range_text(ver: &str) -> String {
    if ver.is_empty() {
        return String::new();
    }
    let mx = max_java_major(ver);
    if mx >= 99 {
        format!("Needs Java {}+ (auto-selected on launch).", required_java_major(ver))
    } else {
        format!("Needs Java {}–{} (auto-selected on launch).", required_java_major(ver), mx)
    }
}

fn fmt_playtime(secs: u64) -> String {
    if secs < 60 {
        return format!("{}s", secs);
    }
    if secs < 3600 {
        return format!("{}m", secs / 60);
    }
    format!("{}h {}m", secs / 3600, (secs % 3600) / 60)
}

fn fmt_date(epoch: &str) -> String {
    epoch.to_string()
}

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

fn sym(name: &str, size: i32) -> gtk::Image {
    let img = gtk::Image::from_icon_name(name);
    img.set_pixel_size(size);
    img
}

fn btn_with_icon(icon: &str, label: &str) -> gtk::Button {
    let b = gtk::Button::new();
    let row = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    row.set_halign(gtk::Align::Center);
    row.append(&sym(icon, 16));
    row.append(&gtk::Label::new(Some(label)));
    b.set_child(Some(&row));
    b
}

fn set_btn_icon_label(btn: &gtk::Button, icon: &str, label: &str) {
    let row = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    row.set_halign(gtk::Align::Center);
    row.append(&sym(icon, 16));
    row.append(&gtk::Label::new(Some(label)));
    btn.set_child(Some(&row));
}

fn set_btn_icon(btn: &gtk::Button, icon: &str, size: i32) {
    btn.set_child(Some(&sym(icon, size)));
}

/// Force the current theme accent on a primary button, bypassing any
/// cascade issue (USER priority always wins).
fn paint_accent(btn: &gtk::Button, theme: &crate::backend::theme::ThemeManager) {
    let css = format!("button {{ background-color: {}; color: #FFFFFF; }}", theme.accent_color());
    let provider = gtk::CssProvider::new();
    provider.load_from_string(&css);
    btn.style_context().add_provider(&provider, gtk::STYLE_PROVIDER_PRIORITY_USER);
}

fn menu_btn(icon: &str, label: &str) -> gtk::Button {
    let b = btn_with_icon(icon, label);
    b.add_css_class("settings-btn");
    b.set_halign(gtk::Align::Fill);
    b
}

/// Button with a launcher-style themed PNG glyph + label.
fn themed_btn(asset: &str, label: &str, is_dark: bool, size: i32) -> gtk::Button {
    let b = gtk::Button::new();
    let row = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    row.set_halign(gtk::Align::Center);
    row.append(&helpers::themed_image(asset, is_dark, size));
    row.append(&gtk::Label::new(Some(label)));
    b.set_child(Some(&row));
    b
}

fn set_themed_btn(btn: &gtk::Button, asset: &str, label: &str, is_dark: bool, size: i32) {
    let row = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    row.set_halign(gtk::Align::Center);
    row.append(&helpers::themed_image(asset, is_dark, size));
    row.append(&gtk::Label::new(Some(label)));
    btn.set_child(Some(&row));
}

fn menu_btn_png(asset: &str, label: &str, is_dark: bool) -> gtk::Button {
    let b = themed_btn(asset, label, is_dark, 16);
    b.add_css_class("settings-btn");
    b.set_halign(gtk::Align::Fill);
    b
}

fn paint_btn(btn: &gtk::Button, hex: &str) {
    let css = format!("button {{ background-color: {}; color: #FFFFFF; }}", hex);
    let provider = gtk::CssProvider::new();
    provider.load_from_string(&css);
    btn.style_context().add_provider(&provider, gtk::STYLE_PROVIDER_PRIORITY_USER);
}

fn note(text: &str) -> gtk::Label {
    let l = gtk::Label::new(Some(text));
    l.set_halign(gtk::Align::Start);
    l.set_wrap(true);
    l.set_opacity(0.6);
    l.add_css_class("time-label");
    l
}

fn info_card(title: &str) -> (gtk::Box, gtk::Label) {
    let frame = gtk::Box::new(gtk::Orientation::Vertical, 2);
    frame.add_css_class("page-card");
    frame.set_margin_top(4);
    frame.set_margin_bottom(4);
    frame.set_margin_start(4);
    frame.set_margin_end(4);
    frame.set_hexpand(true);
    let t = gtk::Label::new(Some(title));
    t.set_halign(gtk::Align::Start);
    t.set_opacity(0.6);
    t.add_css_class("time-label");
    frame.append(&t);
    let v = gtk::Label::new(Some("—"));
    v.set_halign(gtk::Align::Start);
    v.set_ellipsize(gtk::pango::EllipsizeMode::Middle);
    v.add_css_class("details-title");
    frame.append(&v);
    (frame, v)
}

fn clean_md(s: &str) -> String {
    // Modrinth bodies are markdown/HTML: render as plain readable text.
    let mut out = s.replace("\r\n", "\n");
    if let Ok(re) = regex::Regex::new(r"!\[[^\]]*\]\([^\)]*\)") {
        out = re.replace_all(&out, "").to_string();
    }
    if let Ok(re) = regex::Regex::new(r"\[([^\]]+)\]\([^\)]*\)") {
        out = re.replace_all(&out, "$1").to_string();
    }
    // empty links `[](url)` carry nothing readable: drop them entirely
    if let Ok(re) = regex::Regex::new(r"\[\]\([^\)]*\)") {
        out = re.replace_all(&out, "").to_string();
    }
    // footnote refs like [16] and definitions like `[1]: https://…`
    if let Ok(re) = regex::Regex::new(r"\[[0-9]+\](:[^\n]*)?") {
        out = re.replace_all(&out, "").to_string();
    }
    // lone brackets without a link target: `[Post it to our GitHub.]`
    if let Ok(re) = regex::Regex::new(r"\[([^\[\]]+)\]") {
        out = re.replace_all(&out, "$1").to_string();
    }
    if let Ok(re) = regex::Regex::new(r"<[^>]*>") {
        out = re.replace_all(&out, "").to_string();
    }
    let mut lines = Vec::new();
    for line in out.lines() {
        let mut l = line.trim().trim_start_matches(|c| c == '#' || c == '>' || c == '-' || c == '*' || c == '=').trim().to_string();
        l = l.replace('|', " ");
        while l.contains("  ") {
            l = l.replace("  ", " ");
        }
        let l = l.trim().to_string();
        if l.is_empty() || l.chars().all(|c| c == '-' || c == '_' || c == '*' || c == '=' || c == ':') {
            continue;
        }
        lines.push(l.replace("**", "").replace("__", "").replace('`', "").replace('*', ""));
    }
    // collapse 3+ blank gaps
    let mut compact = Vec::new();
    let mut blanks = 0;
    for l in lines {
        if l.is_empty() {
            blanks += 1;
            if blanks <= 1 {
                compact.push(String::new());
            }
        } else {
            blanks = 0;
            compact.push(l.to_string());
        }
    }
    compact.join("\n")
}

enum MdSeg {
    Text(String),
    Link(String, String),
}

fn md_segments(s: &str) -> Vec<MdSeg> {
    let mut out = s.replace("\r\n", "\n");
    if let Ok(re) = regex::Regex::new(r"!\[[^\]]*\]\([^\)]*\)") {
        out = re.replace_all(&out, "").to_string();
    }
    // collect links first, replacing them with placeholders
    let mut links: Vec<(String, String)> = Vec::new();
    if let Ok(re) = regex::Regex::new(r"\[([^\]]*)\]\(([^\)]+)\)") {
        out = re.replace_all(&out, |caps: &regex::Captures| {
            let label = caps.get(1).map(|m| m.as_str().trim().to_string()).unwrap_or_default();
            let url = caps.get(2).map(|m| m.as_str().trim().to_string()).unwrap_or_default();
            let label = if label.is_empty() { url.clone() } else { label };
            links.push((label.clone(), url.clone()));
            format!("@@LINK{}@@", links.len() - 1)
        }).to_string();
    }
    if let Ok(re) = regex::Regex::new(r"\[[0-9]+\]:\s*(https?://[^\s]+)") {
        out = re.replace_all(&out, |caps: &regex::Captures| {
            let url = caps.get(1).map(|m| m.as_str().to_string()).unwrap_or_default();
            links.push((url.clone(), url.clone()));
            format!("@@LINK{}@@", links.len() - 1)
        }).to_string();
    }
    if let Ok(re) = regex::Regex::new(r"<[^>]*>") {
        out = re.replace_all(&out, "").to_string();
    }
    // bare urls last
    if let Ok(re) = regex::Regex::new(r"https?://[^\s\)\]]+") {
        out = re.replace_all(&out, |caps: &regex::Captures| {
            let mut url = caps.get(0).map(|m| m.as_str().to_string()).unwrap_or_default();
            while url.ends_with(|c| ".,;:!?)".contains(c)) {
                url.pop();
            }
            links.push((url.clone(), url.clone()));
            format!("@@LINK{}@@", links.len() - 1)
        }).to_string();
    }
    // footnote refs now meaningless
    if let Ok(re) = regex::Regex::new(r"\[[0-9]+\]") {
        out = re.replace_all(&out, "").to_string();
    }
    // split into clean lines, re-emitting link segments
    let mut segs = Vec::new();
    let link_re = regex::Regex::new(r"@@LINK(\d+)@@").unwrap();
    for line in out.lines() {
        let mut l = line.trim().trim_start_matches(|c| c == '#' || c == '>' || c == '-' || c == '*' || c == '=').trim().to_string();
        l = l.replace('|', " ");
        while l.contains("  ") {
            l = l.replace("  ", " ");
        }
        let l = l.trim().to_string();
        if l.is_empty() || l.chars().all(|c| c == '-' || c == '_' || c == '*' || c == '=' || c == ':') {
            continue;
        }
        let mut last = 0;
        for m in link_re.find_iter(&l) {
            if m.start() > last {
                segs.push(MdSeg::Text(l[last..m.start()].to_string()));
            }
            if let Some(idx) = link_re.captures(&l[m.start()..m.end()]).and_then(|c| c.get(1)).and_then(|x| x.as_str().parse::<usize>().ok()) {
                if let Some((label, url)) = links.get(idx) {
                    segs.push(MdSeg::Link(label.clone(), url.clone()));
                }
            }
            last = m.end();
        }
        if last < l.len() {
            segs.push(MdSeg::Text(l[last..].to_string()));
        }
        segs.push(MdSeg::Text("\n".to_string()));
    }
    // collapse 3+ blank gaps
    let mut compact = Vec::new();
    let mut blanks = 0;
    for s in segs {
        match &s {
            MdSeg::Text(x) if x.trim().is_empty() => {
                blanks += 1;
                if blanks <= 1 {
                    compact.push(s);
                }
            }
            _ => {
                blanks = 0;
                compact.push(s);
            }
        }
    }
    // strip leftover markdown emphasis inside text runs
    compact.into_iter().map(|s| match s {
        MdSeg::Text(x) => MdSeg::Text(x.replace("**", "").replace("__", "").replace('`', "").replace('*', "")),
        other => other,
    }).collect()
}

fn serde_json_to_py_str(s: &str) -> String {
    format!("'{}'", s.replace('\\', "\\\\").replace('\'', "\\'"))
}

fn icon_cache_path(filename: &str) -> std::path::PathBuf {
    std::path::PathBuf::from(std::env::var("HOME").unwrap_or_default())
        .join(".cache/CorkyTux/modicons")
        .join(filename)
}

pub(crate) fn load_mod_icon(url: &str, project_id: &str, img: &gtk::Image, size: i32) {
    if url.is_empty() || project_id.is_empty() {
        return;
    }
    // Thumbnails are decoded bounded (tiny RAM) instead of full-res.
    let max_px = (size * 2).max(64);
    // Modrinth serves webp; gdk-pixbuf here has no webp loader, so the
    // worker normalizes to PNG (PIL, ffmpeg fallback) before GTK loads it.
    let png = icon_cache_path(&format!("{}-icon.png", project_id));
    if png.exists() {
        let path = png.display().to_string();
        if let Some(tex) = helpers::load_thumb(&path, max_px) {
            img.set_paintable(Some(&tex));
            img.set_pixel_size(size);
        } else {
            // Corrupt cache entry: drop it and fetch fresh below.
            std::fs::remove_file(&png).ok();
            load_mod_icon_uncached(url, project_id, img, size, max_px);
        }
        return;
    }
    load_mod_icon_uncached(url, project_id, img, size, max_px);
}

fn load_mod_icon_uncached(url: &str, project_id: &str, img: &gtk::Image, size: i32, max_px: i32) {
    // Cap concurrent downloads: when scrolling fast, extra rows retry
    // shortly instead of spawning hundreds of threads/curl processes.
    // Weak refs everywhere: rows removed while loading stop cleanly.
    if !helpers::img_slot_try_acquire() {
        let url_c = url.to_string();
        let pid_c = project_id.to_string();
        let imgw = img.downgrade();
        glib::timeout_add_local_once(std::time::Duration::from_millis(300), move || {
            if let Some(im) = imgw.upgrade() {
                load_mod_icon(&url_c, &pid_c, &im, size);
            }
        });
        return;
    }
    let png = icon_cache_path(&format!("{}-icon.png", project_id));
    let url = url.to_string();
    let pid = project_id.to_string();
    let imgw = img.downgrade();
    let (tx, rx) = std::sync::mpsc::channel::<String>();
    std::thread::spawn(move || {
        struct Release;
        impl Drop for Release {
            fn drop(&mut self) {
                helpers::img_slot_release();
            }
        }
        let _guard = Release;
        let raw = icon_cache_path(&format!("{}-raw.bin", pid));
        if let Some(parent) = raw.parent() {
            std::fs::create_dir_all(parent).ok();
        }
        let mut dl = false;
        for _ in 0..2 {
            dl = std::process::Command::new("curl").args(["-sL", "--max-time", "20", "-o", raw.to_str().unwrap_or(""), &url]).output().is_ok() && raw.exists();
            if dl {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(400));
        }
        if !dl {
            return;
        }
        let bytes = std::fs::read(&raw).unwrap_or_default();
        if bytes.len() <= 100 {
            return;
        }
        let is_webp = bytes.len() > 12 && &bytes[8..12] == b"WEBP";
        let needs_convert = is_webp || url.ends_with(".webp");
        if needs_convert {
            let py = std::process::Command::new("python3").args(["-c",
                &format!("from PIL import Image; Image.open({}).convert('RGBA').save({})",
                    serde_json_to_py_str(raw.to_str().unwrap_or("")),
                    serde_json_to_py_str(png.to_str().unwrap_or("")))]).output();
            let converted = py.is_ok() && png.exists() && png.metadata().map(|m| m.len()).unwrap_or(0) > 0;
            if !converted {
                let out = std::process::Command::new("ffmpeg").args(["-y", "-v", "error", "-i", raw.to_str().unwrap_or(""), png.to_str().unwrap_or("")]).output();
                if !(out.is_ok() && png.exists() && png.metadata().map(|m| m.len()).unwrap_or(0) > 0) {
                    return;
                }
            }
            std::fs::remove_file(&raw).ok();
        } else {
            std::fs::rename(&raw, &png).ok();
        }
        let _ = tx.send(png.display().to_string());
    });
    glib::idle_add_local(move || match rx.try_recv() {
        Ok(path) => {
            if let Some(im) = imgw.upgrade() {
                if let Some(tex) = helpers::load_thumb(&path, max_px) {
                    im.set_paintable(Some(&tex));
                    im.set_pixel_size(size);
                }
            }
            glib::ControlFlow::Break
        }
        Err(std::sync::mpsc::TryRecvError::Empty) => {
            // Row scrolled away: stop polling instead of spinning forever.
            if imgw.upgrade().is_none() {
                glib::ControlFlow::Break
            } else {
                glib::ControlFlow::Continue
            }
        }
        Err(_) => glib::ControlFlow::Break,
    });
}

fn download_pack_icon(icon_url: &str, launch_version: &str) -> Option<String> {
    if icon_url.is_empty() {
        return None;
    }
    let png = icon_cache_path(&format!("{}-pack-icon.png", safe_id(launch_version)));
    if png.exists() && png.metadata().map(|m| m.len()).unwrap_or(0) > 100 {
        return Some(png.display().to_string());
    }
    if let Some(parent) = png.parent() {
        std::fs::create_dir_all(parent).ok();
    }
    let raw = icon_cache_path(&format!("{}-pack-raw.bin", safe_id(launch_version)));
    let dl = std::process::Command::new("curl").args(["-sL", "--max-time", "25", "-o", raw.to_str().unwrap_or(""), icon_url]).output().is_ok() && raw.exists();
    if !dl {
        return None;
    }
    let bytes = std::fs::read(&raw).unwrap_or_default();
    if bytes.len() <= 100 {
        return None;
    }
    let is_webp = bytes.len() > 12 && bytes.len() >= 12 && &bytes[8..12] == b"WEBP";
    if is_webp || icon_url.ends_with(".webp") {
        let py = std::process::Command::new("python3").args(["-c",
            &format!("from PIL import Image; Image.open({}).convert('RGBA').save({})",
                serde_json_to_py_str(raw.to_str().unwrap_or("")),
                serde_json_to_py_str(png.to_str().unwrap_or("")))]).output();
        let ok = py.is_ok() && png.exists() && png.metadata().map(|m| m.len()).unwrap_or(0) > 0;
        if !ok {
            let out = std::process::Command::new("ffmpeg").args(["-y", "-v", "error", "-i", raw.to_str().unwrap_or(""), png.to_str().unwrap_or("")]).output();
            if !(out.is_ok() && png.exists() && png.metadata().map(|m| m.len()).unwrap_or(0) > 0) {
                return None;
            }
        }
        std::fs::remove_file(&raw).ok();
    } else {
        std::fs::rename(&raw, &png).ok();
    }
    if png.exists() { Some(png.display().to_string()) } else { None }
}

fn find_local_pack_icon(dir: &std::path::Path) -> Option<String> {
    for name in ["icon.png", "pack.png", "logo.png", "pack-icon.png", "modpack.png"] {
        let p = dir.join(name);
        if p.is_file() {
            return Some(p.display().to_string());
        }
        let p2 = dir.join("overrides").join(name);
        if p2.is_file() {
            return Some(p2.display().to_string());
        }
    }
    for base in [dir.to_path_buf(), dir.join("overrides")] {
        if let Ok(entries) = std::fs::read_dir(&base) {
            for e in entries.flatten() {
                let p = e.path();
                if p.is_file() {
                    if let Some(ext) = p.extension().and_then(|x| x.to_str()) {
                        if ["png", "jpg", "jpeg", "webp"].contains(&ext.to_lowercase().as_str()) {
                            if let Some(fname) = p.file_name().and_then(|x| x.to_str()) {
                                let fl = fname.to_lowercase();
                                if fl.contains("icon") || fl.contains("logo") || fl.contains("pack") {
                                    return Some(p.display().to_string());
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    None
}

fn block_fallback_image(is_dark: bool, size: i32) -> gtk::Image {
    let home = std::env::var("HOME").unwrap_or_default();
    let path = format!("{}/.local/share/corkytux/assets/{}",
        home, if is_dark { "inst_block_white.png" } else { "inst_block_black.png" });
    if let Some(tex) = helpers::load_texture(&path) {
        let img = gtk::Image::new();
        img.set_paintable(Some(&tex));
        img.set_pixel_size(size);
        return img;
    }
    helpers::themed_image("minecraft", is_dark, size)
}

fn inst_icon_path(id: &str, kind: &str, is_dark: bool) -> Option<String> {
    let home = std::env::var("HOME").unwrap_or_default();
    if let Some(custom) = kind.strip_prefix("custom:") {
        if !custom.is_empty() {
            return Some(custom.to_string());
        }
    }
    Some(format!("{}/.local/share/corkytux/assets/{}",
        home,
        match kind {
            "ely" => if is_dark { "minecraft.png" } else { "minecraft_dark.png" },
            "block" => if is_dark { "inst_block_white.png" } else { "inst_block_black.png" },
            _ => "inst_grass.png",
        }))
}

fn inst_icon_image(id: &str, kind: &str, is_dark: bool, size: i32) -> gtk::Image {
    if let Some(path) = inst_icon_path(id, kind, is_dark) {
        let mut load = path.clone();
        // webp customs: normalize once via ffmpeg (sync is fine, local file)
        if load.ends_with(".webp") {
            let cached = icon_cache_path(&format!("inst-{}-icon.png", safe_id(id)));
            if !cached.exists() {
                let _ = std::process::Command::new("ffmpeg").args(["-y", "-v", "error", "-i", &load, cached.to_str().unwrap_or("")]).output();
            }
            if cached.exists() {
                load = cached.display().to_string();
            }
        }
        if let Some(tex) = helpers::load_texture(&load) {
            let img = gtk::Image::new();
            img.set_paintable(Some(&tex));
            img.set_pixel_size(size);
            return img;
        }
    }
    helpers::themed_image("minecraft", is_dark, size)
}

fn scan_instance_dirs() -> Vec<(String, String)> {
    let mut out = Vec::new();
    if let Ok(entries) = std::fs::read_dir(instances_root()) {
        for e in entries.flatten() {
            let p = e.path();
            if !p.is_dir() {
                continue;
            }
            let vers_dir = p.join("versions");
            let ids: Vec<String> = std::fs::read_dir(&vers_dir).map(|it| {
                it.flatten().filter(|x| x.path().is_dir())
                    .map(|x| x.file_name().to_string_lossy().to_string()).collect()
            }).unwrap_or_default();
            for id in ids {
                if id.is_empty() {
                    continue;
                }
                out.push((p.display().to_string(), id));
            }
        }
    }
    out
}

/// Drop base vanilla copies living inside a loader install's dir
/// (e.g. versions/26.2 next to fabric-loader-…): they are dependencies,
/// not instances. Same-dir grouping only, so real standalone copies stay.
fn drop_base_copies(ids: &[String]) -> Vec<String> {
    let loader_mcs: Vec<String> = ids.iter().filter_map(|id| {
        let (mc, loader, _) = parse_inst_id(id);
        if loader != "vanilla" { Some(mc) } else { None }
    }).collect();
    ids.iter().filter(|id| {
        let (mc, loader, _) = parse_inst_id(id);
        !(loader == "vanilla" && loader_mcs.iter().any(|m| m == &mc))
    }).cloned().collect()
}

pub struct MinecraftView {
    pub widget: gtk::ScrolledWindow,
    data: Rc<RefCell<McData>>,
    main_stack: gtk::Stack,
    // library
    flow: gtk::FlowBox,
    search_entry: gtk::SearchEntry,
    sort_drop: gtk::DropDown,
    lib_status: gtk::Label,
    account_btn: gtk::MenuButton,
    account_head: gtk::Image,
    account_label: gtk::Label,
    account_pop: gtk::Popover,
    install_prog: Rc<RefCell<HashMap<String, f64>>>,
    sessions: Rc<RefCell<HashMap<String, (String, i64)>>>,
    // detail
    detail_id: Rc<RefCell<String>>,
    detail_icon: gtk::Image,
    detail_name: gtk::Label,
    detail_sub: gtk::Label,
    detail_play: gtk::Button,
    detail_star: gtk::Button,
    addons_tab_wrap: gtk::Box,
    det_overview_btn: gtk::ToggleButton,
    detail_tabs: gtk::Stack,
    ov_mc: gtk::Label,
    ov_loader: gtk::Label,
    ov_addons: gtk::Label,
    ov_played: gtk::Label,
    ov_last: gtk::Label,
    ov_icon_btn: gtk::Button,
    ov_name_entry: gtk::Entry,
    // addons tab
    addons_search: gtk::SearchEntry,
    addons_type: Rc<RefCell<String>>,
    addons_box: gtk::Box,
    addons_count: gtk::Label,
    addons_dir_lbl: gtk::Label,
    plat_drop: gtk::DropDown,
    update_all_btn: gtk::Button,
    // logs tab
    logs_view: gtk::TextView,
    log_lbl: gtk::Label,
    // per-instance settings tab
    set_ram: gtk::Scale,
    set_ram_lbl: gtk::Label,
    set_ram_entry: gtk::Entry,
    set_jvm: gtk::Entry,
    set_res_w: gtk::Entry,
    set_res_h: gtk::Entry,
    set_wrapper: gtk::Entry,
    set_prehook: gtk::Entry,
    set_java: gtk::Entry,
    state: AppState,
    parent: adw::ApplicationWindow,
}

impl Clone for MinecraftView {
    fn clone(&self) -> Self {
        Self {
            widget: self.widget.clone(),
            data: self.data.clone(),
            main_stack: self.main_stack.clone(),
            flow: self.flow.clone(),
            search_entry: self.search_entry.clone(),
            sort_drop: self.sort_drop.clone(),
            lib_status: self.lib_status.clone(),
            account_btn: self.account_btn.clone(),
            account_head: self.account_head.clone(),
            account_label: self.account_label.clone(),
            account_pop: self.account_pop.clone(),
            install_prog: self.install_prog.clone(),
            sessions: self.sessions.clone(),
            detail_id: self.detail_id.clone(),
            detail_icon: self.detail_icon.clone(),
            detail_name: self.detail_name.clone(),
            detail_sub: self.detail_sub.clone(),
            detail_play: self.detail_play.clone(),
            detail_star: self.detail_star.clone(),
            addons_tab_wrap: self.addons_tab_wrap.clone(),
            det_overview_btn: self.det_overview_btn.clone(),
            detail_tabs: self.detail_tabs.clone(),
            ov_mc: self.ov_mc.clone(),
            ov_loader: self.ov_loader.clone(),
            ov_addons: self.ov_addons.clone(),
            ov_played: self.ov_played.clone(),
            ov_last: self.ov_last.clone(),
            ov_icon_btn: self.ov_icon_btn.clone(),
            ov_name_entry: self.ov_name_entry.clone(),
            addons_search: self.addons_search.clone(),
            addons_type: self.addons_type.clone(),
            addons_box: self.addons_box.clone(),
            addons_count: self.addons_count.clone(),
            addons_dir_lbl: self.addons_dir_lbl.clone(),
            plat_drop: self.plat_drop.clone(),
            update_all_btn: self.update_all_btn.clone(),
            logs_view: self.logs_view.clone(),
            log_lbl: self.log_lbl.clone(),
            set_ram: self.set_ram.clone(),
            set_ram_lbl: self.set_ram_lbl.clone(),
            set_ram_entry: self.set_ram_entry.clone(),
            set_jvm: self.set_jvm.clone(),
            set_res_w: self.set_res_w.clone(),
            set_res_h: self.set_res_h.clone(),
            set_wrapper: self.set_wrapper.clone(),
            set_prehook: self.set_prehook.clone(),
            set_java: self.set_java.clone(),
            state: self.state.clone(),
            parent: self.parent.clone(),
        }
    }
}

impl MinecraftView {
    fn cfg(&self, key: &str) -> String {
        self.state.config.launcher_value(key).unwrap_or_default()
    }

    fn set_cfg(&self, key: &str, val: &str) {
        self.state.config.set_launcher_value(key, val);
    }

    fn inst_cfg(&self, id: &str, key: &str) -> String {
        self.cfg(&format!("Mc{}_{}", key, safe_id(id)))
    }

    fn set_inst_cfg(&self, id: &str, key: &str, val: &str) {
        self.set_cfg(&format!("Mc{}_{}", key, safe_id(id)), val);
    }

    pub fn new(state: &AppState, parent: &adw::ApplicationWindow) -> Self {
        let scroll = gtk::ScrolledWindow::new();
        scroll.set_hexpand(true);
        scroll.set_vexpand(true);
        scroll.set_hscrollbar_policy(gtk::PolicyType::Never);
        scroll.set_has_frame(false);
        scroll.set_overlay_scrolling(true);
        let col = gtk::Box::new(gtk::Orientation::Vertical, 12);
        col.set_margin_top(24);
        col.set_margin_bottom(24);
        col.set_margin_start(24);
        col.set_margin_end(24);
        col.set_hexpand(true);
        scroll.set_child(Some(&col));

        let data = Rc::new(RefCell::new(McData::default()));
        let install_prog: Rc<RefCell<HashMap<String, f64>>> = Rc::new(RefCell::new(HashMap::new()));
        let sessions: Rc<RefCell<HashMap<String, (String, i64)>>> = Rc::new(RefCell::new(HashMap::new()));
        let detail_id: Rc<RefCell<String>> = Rc::new(RefCell::new(String::new()));
        let addons_type: Rc<RefCell<String>> = Rc::new(RefCell::new("mods".to_string()));

        let main_stack = gtk::Stack::new();
        main_stack.set_vexpand(false);
        main_stack.set_hexpand(true);

        // ============ LIBRARY PAGE (Carbon HomeGrid) ============
        let lib_page = gtk::Box::new(gtk::Orientation::Vertical, 10);
        let head = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        head.set_valign(gtk::Align::Start);
        let account_head = gtk::Image::new();
        account_head.set_pixel_size(24);
        account_head.set_icon_name(Some("avatar-default-symbolic"));
        let account_label = gtk::Label::new(Some("Inicia sesión"));
        account_label.set_halign(gtk::Align::Start);
        account_label.add_css_class("mc-account-label");
        account_label.set_ellipsize(gtk::pango::EllipsizeMode::End);
        account_label.set_max_width_chars(14);
        let account_arrow = gtk::Image::from_icon_name("pan-down-symbolic");
        account_arrow.set_pixel_size(14);
        account_arrow.set_opacity(0.6);
        let account_inner = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        account_inner.append(&account_head);
        account_inner.append(&account_label);
        account_inner.append(&account_arrow);
        let account_btn = gtk::MenuButton::new();
        account_btn.set_child(Some(&account_inner));
        account_btn.add_css_class("mc-account");
        account_btn.set_tooltip_text(Some("Cuenta activa — clic para cambiar"));
        let account_pop = gtk::Popover::new();
        account_pop.set_autohide(true);
        account_btn.set_popover(Some(&account_pop));
        head.append(&account_btn);
        let search_entry = gtk::SearchEntry::new();
        search_entry.set_placeholder_text(Some("Search instances…"));
        search_entry.set_hexpand(true);
        search_entry.set_width_request(160);
        head.append(&search_entry);
        let sort_store = gtk::StringList::new(&["Name", "Most played", "Last played", "Game version"]);
        let sort_drop = gtk::DropDown::new(Some(sort_store), gtk::Expression::NONE);
        sort_drop.set_tooltip_text(Some("Sort by"));
        sort_drop.set_valign(gtk::Align::Center);
        head.append(&sort_drop);
        let settings_btn = gtk::Button::new();
        settings_btn.set_child(Some(&helpers::themed_image("settings", state.theme.is_dark(), 16)));
        settings_btn.set_tooltip_text(Some("Minecraft settings"));
        settings_btn.set_valign(gtk::Align::Center);
        head.append(&settings_btn);
        let add_btn = themed_btn("download", "ADD INSTANCE", state.theme.is_dark(), 18);
        add_btn.add_css_class("add-btn");
        paint_accent(&add_btn, &state.theme);
        head.append(&add_btn);
        head.add_css_class("mc-head");
        lib_page.append(&head);

        let lib_status = note("");
        lib_page.append(&lib_status);

        let flow = gtk::FlowBox::new();
        flow.set_max_children_per_line(20);
        flow.set_min_children_per_line(2);
        flow.set_selection_mode(gtk::SelectionMode::None);
        flow.set_row_spacing(10);
        flow.set_column_spacing(10);
        flow.set_halign(gtk::Align::Fill);
        flow.set_hexpand(true);
        lib_page.append(&flow);
        main_stack.add_named(&lib_page, Some("library"));

        // ============ DETAIL PAGE (Carbon DetailPageLayout) ============
        let det_page = gtk::Box::new(gtk::Orientation::Vertical, 10);
        let det_head = gtk::Box::new(gtk::Orientation::Horizontal, 10);
        let back_btn = gtk::Button::new();
        back_btn.add_css_class("icon-ghost");
        set_btn_icon(&back_btn, "go-previous-symbolic", 18);
        back_btn.set_tooltip_text(Some("Back to library"));
        det_head.append(&back_btn);
        let detail_icon = helpers::themed_image("minecraft", state.theme.is_dark(), 44);
        det_head.append(&detail_icon);
        let name_box = gtk::Box::new(gtk::Orientation::Vertical, 0);
        name_box.set_hexpand(true);
        let detail_name = gtk::Label::new(Some("—"));
        detail_name.set_halign(gtk::Align::Start);
        detail_name.add_css_class("details-title");
        name_box.append(&detail_name);
        let detail_sub = gtk::Label::new(Some(""));
        detail_sub.set_halign(gtk::Align::Start);
        detail_sub.set_opacity(0.6);
        detail_sub.add_css_class("time-label");
        name_box.append(&detail_sub);
        det_head.append(&name_box);
        let detail_play = themed_btn("play", "PLAY", state.theme.is_dark(), 20);
        detail_play.add_css_class("add-btn");
        detail_play.set_width_request(150);
        paint_accent(&detail_play, &state.theme);
        det_head.append(&detail_play);
        let detail_star = gtk::Button::new();
        detail_star.add_css_class("icon-ghost");
        detail_star.set_child(Some(&helpers::themed_image("star_gray", state.theme.is_dark(), 20)));
        detail_star.set_tooltip_text(Some("Favorite"));
        det_head.append(&detail_star);

        det_head.add_css_class("mc-head");
        det_page.append(&det_head);

        // detail tabs (Carbon: Overview/Addons/Settings/Logs)
        let det_tabbar = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        let detail_tabs = gtk::Stack::new();
        detail_tabs.set_vexpand(false);
        let det_ids = ["overview", "addons", "logs", "isettings"];
        let det_labels = ["Overview", "Addons", "Logs", "Settings"];
        let mut det_btns: Vec<gtk::ToggleButton> = Vec::new();
        let mut det_inds: Vec<gtk::Box> = Vec::new();
        let addons_tab_wrap_holder: Rc<RefCell<Option<gtk::Box>>> = Rc::new(RefCell::new(None));
        let det_icons = ["view-grid-symbolic", "application-x-addon-symbolic", "text-x-generic-symbolic", "preferences-system-symbolic"];
        for ((label, id), tab_icon) in det_labels.iter().zip(det_ids.iter()).zip(det_icons.iter()) {
            let wrap = gtk::Box::new(gtk::Orientation::Vertical, 1);
            wrap.set_hexpand(true);
            let btn = gtk::ToggleButton::new();
            btn.add_css_class("settings-tab");
            btn.set_hexpand(true);
            let c = gtk::Box::new(gtk::Orientation::Vertical, 2);
            c.set_halign(gtk::Align::Center);
            c.append(&sym(tab_icon, 18));
            let lbl = gtk::Label::new(Some(label));
            lbl.add_css_class("time-label");
            c.append(&lbl);
            let ind = gtk::Box::new(gtk::Orientation::Horizontal, 0);
            ind.add_css_class("settings-tab-indicator");
            ind.set_visible(*id == "overview");
            c.append(&ind);
            btn.set_child(Some(&c));
            if *id == "overview" {
                btn.set_active(true);
            }
            wrap.append(&btn);
            det_tabbar.append(&wrap);
            det_btns.push(btn);
            det_inds.push(ind);
            if *id == "addons" {
                addons_tab_wrap_holder.borrow_mut().replace(wrap);
            }
        }
        for b in &det_btns[1..] {
            b.set_group(Some(&det_btns[0]));
        }
        let det_overview_btn = det_btns[0].clone();
        for (i, id) in det_ids.iter().enumerate() {
            let stack = detail_tabs.clone();
            let tid = id.to_string();
            let all = det_inds.clone();
            let mine = det_inds[i].clone();
            det_btns[i].connect_toggled(move |b| {
                if b.is_active() {
                    stack.set_visible_child_name(&tid);
                    for ind in &all {
                        ind.set_visible(false);
                    }
                    mine.set_visible(true);
                }
            });
        }
        det_page.append(&det_tabbar);

        // Overview tab: Carbon cards
        let ov_page = gtk::Box::new(gtk::Orientation::Vertical, 8);
        let (ov_id_frame, ov_id_inner) = card("Instance");
        let ov_id_row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        let ov_icon_btn = gtk::Button::new();
        ov_icon_btn.set_tooltip_text(Some("Change icon"));
        let ov_icon_overlay = gtk::Overlay::new();
        ov_icon_overlay.set_child(Some(&ov_icon_btn));
        let ov_pencil = gtk::Button::new();
        ov_pencil.add_css_class("icon-ghost");
        ov_pencil.set_child(Some(&sym("document-edit-symbolic", 11)));
        ov_pencil.set_halign(gtk::Align::End);
        ov_pencil.set_valign(gtk::Align::End);
        ov_pencil.set_width_request(20);
        ov_pencil.set_height_request(20);
        ov_pencil.set_tooltip_text(Some("Change icon"));
        ov_id_row.append(&ov_icon_overlay);
        {
            let ov_icon_btn_c = ov_icon_btn.clone();
            ov_pencil.connect_clicked(move |_| ov_icon_btn_c.emit_clicked());
        }
        ov_icon_overlay.add_overlay(&ov_pencil);
        let ov_id_mid = gtk::Box::new(gtk::Orientation::Vertical, 4);
        ov_id_mid.set_hexpand(true);
        let ov_name_entry = gtk::Entry::new();
        ov_id_mid.append(&note("Display name"));
        ov_id_mid.append(&ov_name_entry);
        ov_id_row.append(&ov_id_mid);
        let ov_name_save = btn_with_icon("view-refresh-symbolic", "Save");
        ov_name_save.add_css_class("settings-btn");
        ov_name_save.set_valign(gtk::Align::Center);
        ov_id_row.append(&ov_name_save);
        ov_id_inner.append(&ov_id_row);
        ov_page.append(&ov_id_frame);
        ov_page.set_margin_top(12);
        ov_page.set_margin_bottom(12);
        ov_page.set_margin_start(16);
        ov_page.set_margin_end(16);
        let cards_row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        cards_row.set_homogeneous(true);
        let (c1, ov_mc) = info_card("Minecraft version");
        let (c2, ov_loader) = info_card("Modloader");
        let (c3, ov_addons) = info_card("Addons");
        cards_row.append(&c1);
        cards_row.append(&c2);
        cards_row.append(&c3);
        ov_page.append(&cards_row);
        let cards_row2 = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        cards_row2.set_homogeneous(true);
        let (c4, ov_played) = info_card("Time played");
        let (c5, ov_last) = info_card("Last played");
        cards_row2.append(&c4);
        cards_row2.append(&c5);
        ov_page.append(&cards_row2);
        detail_tabs.add_titled(&ov_page, Some("overview"), "Overview");

        // Addons tab: Carbon AddonsPageLayout toolbar
        let ad_page = gtk::Box::new(gtk::Orientation::Vertical, 8);
        ad_page.set_margin_top(12);
        ad_page.set_margin_bottom(12);
        ad_page.set_margin_start(16);
        ad_page.set_margin_end(16);
        let ad_toolbar = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        let addons_search = gtk::SearchEntry::new();
        addons_search.set_placeholder_text(Some("Search installed…"));
        addons_search.set_hexpand(true);
        ad_toolbar.append(&addons_search);
        let plat_store = gtk::StringList::new(&["All", "Modrinth", "Local"]);
        let plat_drop = gtk::DropDown::new(Some(plat_store), gtk::Expression::NONE);
        ad_toolbar.append(&plat_drop);
        let update_all_btn = btn_with_icon("software-update-available-symbolic", "Update all");
        update_all_btn.add_css_class("settings-btn");
        update_all_btn.set_visible(false);
        ad_toolbar.append(&update_all_btn);
        let rescan_btn = themed_btn("search", "Rescan", state.theme.is_dark(), 14);
        rescan_btn.add_css_class("settings-btn");
        ad_toolbar.append(&rescan_btn);
        let browse_btn = themed_btn("download", "ADD", state.theme.is_dark(), 16);
        browse_btn.add_css_class("add-btn");
        ad_toolbar.append(&browse_btn);
        let ad_folder_btn = gtk::Button::with_label("Open folder");
        ad_folder_btn.add_css_class("settings-btn");
        ad_toolbar.append(&ad_folder_btn);
        ad_page.append(&ad_toolbar);
        let chips_row = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        let chip_mods = gtk::ToggleButton::with_label("Mods");
        let chip_shaders = gtk::ToggleButton::with_label("Shaders");
        let chip_res = gtk::ToggleButton::with_label("Resource Packs");
        chip_mods.set_active(true);
        chip_shaders.set_group(Some(&chip_mods));
        chip_res.set_group(Some(&chip_mods));
        chips_row.append(&chip_mods);
        chips_row.append(&chip_shaders);
        chips_row.append(&chip_res);
        ad_page.append(&chips_row);
        let addons_count = note("");
        ad_page.append(&addons_count);
        let addons_dir_lbl = note("");
        addons_dir_lbl.set_selectable(true);
        ad_page.append(&addons_dir_lbl);
        let addons_box = gtk::Box::new(gtk::Orientation::Vertical, 4);
        ad_page.append(&addons_box);
        detail_tabs.add_titled(&ad_page, Some("addons"), "Addons");

        // Logs tab
        let lg_page = gtk::Box::new(gtk::Orientation::Vertical, 8);
        lg_page.set_margin_top(12);
        lg_page.set_margin_bottom(12);
        lg_page.set_margin_start(16);
        lg_page.set_margin_end(16);
        let (lg_frame, lg_inner) = card("Logs");
        let log_lbl = note("");
        log_lbl.set_selectable(true);
        lg_inner.append(&log_lbl);
        let lg_scroll = gtk::ScrolledWindow::new();
        lg_scroll.set_min_content_height(220);
        lg_scroll.set_max_content_height(320);
        lg_scroll.set_vexpand(false);
        let logs_view = gtk::TextView::new();
        logs_view.set_editable(false);
        logs_view.set_cursor_visible(false);
        logs_view.set_monospace(true);
        lg_scroll.set_child(Some(&logs_view));
        lg_inner.append(&lg_scroll);
        let lg_row = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        let lg_refresh = btn_with_icon("view-refresh-symbolic", "Refresh");
        lg_refresh.add_css_class("settings-btn");
        lg_refresh.set_hexpand(true);
        let lg_folder = themed_btn("folder", "Open logs folder", state.theme.is_dark(), 16);
        lg_folder.add_css_class("settings-btn");
        lg_folder.set_hexpand(true);
        lg_row.append(&lg_refresh);
        lg_row.append(&lg_folder);
        lg_inner.append(&lg_row);
        lg_page.append(&lg_frame);
        detail_tabs.add_titled(&lg_page, Some("logs"), "Logs");

        // Per-instance Settings tab
        let st_page = gtk::Box::new(gtk::Orientation::Vertical, 8);
        st_page.set_margin_top(12);
        st_page.set_margin_bottom(12);
        st_page.set_margin_start(16);
        st_page.set_margin_end(16);
        let (st_frame, st_inner) = card("Instance settings");
        st_inner.append(&note("Memory (RAM)"));
        let set_ram_row = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        let set_ram_lbl = gtk::Label::new(Some("2048 MB"));
        let set_ram = gtk::Scale::with_range(gtk::Orientation::Horizontal, 512.0, 16384.0, 256.0);
        set_ram.set_hexpand(true);
        set_ram.set_draw_value(false);
        let set_ram_entry = gtk::Entry::new();
        set_ram_entry.set_width_request(90);
        set_ram_entry.set_placeholder_text(Some("MB"));
        set_ram_row.append(&set_ram_lbl);
        set_ram_row.append(&set_ram);
        set_ram_row.append(&set_ram_entry);
        st_inner.append(&set_ram_row);

        st_inner.append(&note("Java binary (empty = auto)"));
        let set_java = gtk::Entry::new();
        set_java.set_placeholder_text(Some("auto"));
        st_inner.append(&set_java);
        st_inner.append(&note("Extra JVM arguments"));
        let set_jvm = gtk::Entry::new();
        set_jvm.set_placeholder_text(Some("-XX:+UseG1GC …"));
        st_inner.append(&set_jvm);
        st_inner.append(&note("Game resolution (empty = default)"));
        let set_res_row = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        let set_res_w = gtk::Entry::new();
        set_res_w.set_placeholder_text(Some("width"));
        set_res_w.set_hexpand(true);
        let set_res_h = gtk::Entry::new();
        set_res_h.set_placeholder_text(Some("height"));
        set_res_h.set_hexpand(true);
        set_res_row.append(&set_res_w);
        set_res_row.append(&gtk::Label::new(Some("×")));
        set_res_row.append(&set_res_h);
        st_inner.append(&set_res_row);
        st_inner.append(&note("Wrapper command (e.g. gamemoderun)"));
        let set_wrapper = gtk::Entry::new();
        set_wrapper.set_placeholder_text(Some("empty = none"));
        st_inner.append(&set_wrapper);
        st_inner.append(&note("Pre-launch hook (shell command)"));
        let set_prehook = gtk::Entry::new();
        set_prehook.set_placeholder_text(Some("empty = none"));
        st_inner.append(&set_prehook);
        let set_repair = btn_with_icon("view-refresh-symbolic", "Repair (reinstall files)");
        set_repair.add_css_class("settings-btn");
        st_inner.append(&set_repair);
        let set_danger_row = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        set_danger_row.set_homogeneous(true);
        let set_folder_btn = themed_btn("folder", "Open folder", state.theme.is_dark(), 16);
        set_folder_btn.add_css_class("settings-btn");
        let set_delete_btn = themed_btn("remove", "Delete instance", state.theme.is_dark(), 16);
        set_delete_btn.add_css_class("settings-btn");
        set_danger_row.append(&set_folder_btn);
        set_danger_row.append(&set_delete_btn);
        st_inner.append(&set_danger_row);
        let set_save = gtk::Button::with_label("Save");
        set_save.add_css_class("add-btn");
        paint_accent(&set_save, &state.theme);
        st_inner.append(&set_save);
        st_page.append(&st_frame);
        detail_tabs.add_titled(&st_page, Some("isettings"), "Settings");
        det_page.append(&detail_tabs);
        main_stack.add_named(&det_page, Some("detail"));

        col.append(&main_stack);

        let view = Self {
            widget: scroll,
            data,
            main_stack,
            flow,
            search_entry,
            sort_drop,
            lib_status,
            account_btn: account_btn.clone(),
            account_head: account_head.clone(),
            account_label: account_label.clone(),
            account_pop: account_pop.clone(),
            install_prog,
            sessions,
            detail_id,
            detail_icon: detail_icon.clone(),
            detail_name,
            detail_sub,
            detail_play,
            detail_star,
            addons_tab_wrap: addons_tab_wrap_holder.borrow().clone().unwrap_or_else(|| gtk::Box::new(gtk::Orientation::Vertical, 1)),
            det_overview_btn: det_overview_btn.clone(),
            detail_tabs,
            ov_mc,
            ov_loader,
            ov_addons,
            ov_played,
            ov_last,
            ov_icon_btn: ov_icon_btn.clone(),
            ov_name_entry: ov_name_entry.clone(),
            addons_search,
            addons_type,
            addons_box,
            addons_count,
            addons_dir_lbl: addons_dir_lbl.clone(),
            plat_drop,
            update_all_btn: update_all_btn.clone(),
            logs_view,
            log_lbl,
            set_ram,
            set_ram_lbl,
            set_ram_entry: set_ram_entry.clone(),
            set_jvm: set_jvm.clone(),
            set_res_w: set_res_w.clone(),
            set_res_h: set_res_h.clone(),
            set_wrapper: set_wrapper.clone(),
            set_prehook: set_prehook.clone(),
            set_java,
            state: state.clone(),
            parent: parent.clone(),
        };

        // ---- wiring ----
        {
            let v = view.clone();
            view.search_entry.connect_search_changed(move |_| v.render_library());
        }
        {
            let v = view.clone();
            view.sort_drop.connect_selected_notify(move |d| {
                let names = ["Name", "Most played", "Last played", "Game version"];
                let sel = names.get(d.selected() as usize).unwrap_or(&"Name").to_string();
                v.set_cfg("McSort", &sel);
                v.render_library();
            });
        }
        {
            let v = view.clone();
            add_btn.connect_clicked(move |_| v.show_add_instance());
        }
        {
            let v = view.clone();
            settings_btn.connect_clicked(move |_| v.show_mc_settings());
        }
        {
            let v = view.clone();
            back_btn.connect_clicked(move |_| {
                v.main_stack.set_visible_child_name("library");
                v.render_library();
            });
        }
        {
            let v = view.clone();
            view.detail_play.connect_clicked(move |_| v.toggle_play_selected());
        }
        {
            let v = view.clone();
            view.detail_star.connect_clicked(move |_| v.toggle_fav_selected());
        }
        {
            let v = view.clone();
            view.ov_icon_btn.connect_clicked(move |_| v.show_icon_popover());
        }
        {
            let v = view.clone();
            let save = move || {
                let name = v.ov_name_entry.text().to_string().trim().to_string();
                if !name.is_empty() {
                    let id = v.detail_id.borrow().clone();
                    if !id.is_empty() {
                        v.set_inst_cfg(&id, "Name", &name);
                        v.reload_silent();
                        v.render_detail();
                    }
                }
            };
            let s2 = save.clone();
            ov_name_save.connect_clicked(move |_| save());
            let vv = view.clone();
            view.ov_name_entry.connect_activate(move |_| {
                s2();
                let _ = &vv;
            });
        }
        {
            let v = view.clone();
            let gesture = gtk::GestureClick::new();
            gesture.connect_pressed(move |g, _, _, _| {
                if g.current_button() == 1 {
                    v.show_icon_popover();
                }
            });
            view.detail_icon.add_controller(gesture);
        }

        {
            let v = view.clone();
            view.addons_search.connect_search_changed(move |_| v.render_addons());
        }
        {
            let v = view.clone();
            let t = view.addons_type.clone();
            chip_mods.connect_toggled(move |b| {
                if b.is_active() {
                    *t.borrow_mut() = "mods".to_string();
                    v.render_addons();
                }
            });
        }
        {
            let v = view.clone();
            let t = view.addons_type.clone();
            chip_shaders.connect_toggled(move |b| {
                if b.is_active() {
                    *t.borrow_mut() = "shaders".to_string();
                    v.render_addons();
                }
            });
        }
        {
            let v = view.clone();
            let t = view.addons_type.clone();
            chip_res.connect_toggled(move |b| {
                if b.is_active() {
                    *t.borrow_mut() = "resourcepacks".to_string();
                    v.render_addons();
                }
            });
        }
        {
            let v = view.clone();
            view.plat_drop.connect_selected_notify(move |_| v.render_addons());
        }
        {
            let v = view.clone();
            browse_btn.connect_clicked(move |_| v.show_browse());
        }
        {
            let v = view.clone();
            update_all_btn.connect_clicked(move |_| v.do_update_all());
        }
        {
            let v = view.clone();
            rescan_btn.connect_clicked(move |_| {
                v.rescan_addons();
            });
        }
        {
            let st = state.clone();
            let vv = view.clone();
            ad_folder_btn.connect_clicked(move |_| {
                let dir = vv.addon_dir();
                st.integration.open_url(&format!("file://{}", dir.display()));
            });
        }
        {
            let v = view.clone();
            lg_refresh.connect_clicked(move |_| v.do_refresh_logs());
        }
        {
            let st = state.clone();
            lg_folder.connect_clicked(move |_| {
                let dir = std::env::var("HOME").unwrap_or_default() + "/.local/share/CorkyTux/logs";
                st.integration.open_url(&format!("file://{}", dir));
            });
        }
        {
            let v = view.clone();
            let l = view.set_ram_lbl.clone();
            let e = view.set_ram_entry.clone();
            view.set_ram.connect_value_changed(move |s| {
                let mb = s.value() as u32;
                l.set_text(&format!("{} MB", mb));
                e.set_text(&mb.to_string());
                let id = v.detail_id.borrow().clone();
                if !id.is_empty() {
                    v.set_inst_cfg(&id, "Ram", &mb.to_string());
                }
            });
        }
        {
            let v = view.clone();
            // typed MB (Enter or focus-leave) <-> slider, saved live
            v.set_ram_entry.connect_activate(move |e| {
                let raw = e.text().to_string().trim().to_string();
                if let Ok(mb) = raw.parse::<f64>() {
                    v.set_ram.set_value(mb.clamp(512.0, 16384.0));
                } else {
                    e.set_text(&format!("{}", v.set_ram.value() as u32));
                }
            });
            let v2 = view.clone();
            let focus = gtk::EventControllerFocus::new();
            focus.connect_leave(move |_| {
                let raw = v2.set_ram_entry.text().to_string().trim().to_string();
                if let Ok(mb) = raw.parse::<f64>() {
                    v2.set_ram.set_value(mb.clamp(512.0, 16384.0));
                }
            });
            view.set_ram_entry.add_controller(focus);
        }
        {
            let v = view.clone();
            set_save.connect_clicked(move |_| v.save_instance_settings());
        }
        {
            let v = view.clone();
            set_repair.connect_clicked(move |_| v.do_repair());
        }
        {
            let v = view.clone();
            set_folder_btn.connect_clicked(move |_| {
                let id = v.detail_id.borrow().clone();
                v.open_inst_folder(&id);
            });
        }
        {
            let v = view.clone();
            set_delete_btn.connect_clicked(move |_| {
                let id = v.detail_id.borrow().clone();
                if !id.is_empty() {
                    v.confirm_delete(&id);
                }
            });
        }
        // playtime ticker: finalize sessions whose pid died
        {
            let v = view.clone();
            glib::timeout_add_local(std::time::Duration::from_secs(15), move || {
                v.tick_sessions();
                glib::ControlFlow::Continue
            });
        }
        {
            let names = ["Name", "Most played", "Last played", "Game version"];
            let saved = view.cfg_sort();
            if let Some(pos) = names.iter().position(|n| *n == saved) {
                view.sort_drop.set_selected(pos as u32);
            }
        }
        {
            let v = view.clone();
            view.account_pop.connect_show(move |_| v.rebuild_account_popover());
        }
        view.refresh_account_header();
        view.refresh_all();
        view
    }

    // ============ data ============

    fn cfg_sort(&self) -> String {
        let s = self.cfg("McSort");
        if s.is_empty() { "Name".to_string() } else { s }
    }

    fn load_installed(&self) -> Vec<Inst> {
        let mut out = Vec::new();
        let mut by_dir: std::collections::HashMap<String, Vec<String>> = std::collections::HashMap::new();
        for (dir, id) in scan_instance_dirs() {
            by_dir.entry(dir).or_default().push(id);
        }
        for ids in by_dir.values() {
            for id in drop_base_copies(ids) {
                out.push(self.make_inst(&id, true));
            }
        }
        // legacy global installs (same base-copy rule in the shared dir)
        let st = MinecraftManager::status().unwrap_or_default();
        let legacy: Vec<String> = st.get("installed_versions").and_then(|a| a.as_array()).cloned().unwrap_or_default()
            .into_iter().filter_map(|x| x.as_str().map(str::to_string)).collect();
        for id in drop_base_copies(&legacy) {
            if !out.iter().any(|i: &Inst| i.id == id) {
                out.push(self.make_inst(&id, false));
            }
        }
        out
    }

    fn make_inst(&self, id: &str, isolated: bool) -> Inst {
        let (mc_ver, loader, loader_ver) = parse_inst_id(id);
        let secs: u64 = self.inst_cfg(id, "Time").parse().unwrap_or(0);
        Inst {
            id: id.to_string(),
            mc_ver,
            loader,
            loader_ver,
            isolated,
            fav: self.inst_cfg(id, "Fav") == "1",
            secs,
            last: self.inst_cfg(id, "Last"),
            disp: {
                let n = self.inst_cfg(id, "Name");
                if n.is_empty() { id.to_string() } else { n }
            },
        }
    }

    fn refresh_all(&self) {
        self.lib_status.set_text("Loading…");
        let (tx, rx) = std::sync::mpsc::channel::<McData>();
        std::thread::spawn(move || {
            let st = MinecraftManager::status().unwrap_or_default();
            let mut data = McData::default();
            data.deps_ok = st.get("deps").and_then(|d| d.get("minecraft_launcher_lib"))
                .and_then(|x| x.as_bool()).unwrap_or(false);
            data.accounts = st.get("accounts").and_then(|a| a.as_array()).cloned().unwrap_or_default()
                .into_iter().map(|a| (
                    a.get("id").and_then(|x| x.as_str()).unwrap_or("").to_string(),
                    a.get("name").and_then(|x| x.as_str()).unwrap_or("").to_string(),
                    a.get("offline").and_then(|x| x.as_bool()).unwrap_or(false),
                    a.get("ely").and_then(|x| x.as_bool()).unwrap_or(false),
                )).collect::<Vec<_>>();
            data.java_selected = st.get("java").and_then(|j| j.get("selected")).and_then(|x| x.as_str()).unwrap_or("").to_string();
            data.java_found = MinecraftManager::java_detect().ok()
                .and_then(|d| d.get("found").cloned()).and_then(|v| v.as_array().cloned()).unwrap_or_default()
                .into_iter().map(|j| (
                    j.get("path").and_then(|x| x.as_str()).unwrap_or("").to_string(),
                    j.get("version").and_then(|x| x.as_str()).unwrap_or("").to_string(),
                    j.get("origin").and_then(|x| x.as_str()).unwrap_or("").to_string(),
                )).collect::<Vec<_>>();
            data.azure_configured = st.get("azure_configured").and_then(|x| x.as_bool()).unwrap_or(false);
            data.versions = MinecraftManager::versions(false).ok()
                .and_then(|d| d.get("versions").cloned()).and_then(|v| v.as_array().cloned()).unwrap_or_default()
                .into_iter().map(|x| (
                    x.get("id").and_then(|i| i.as_str()).unwrap_or("").to_string(),
                    x.get("type").and_then(|i| i.as_str()).unwrap_or("").to_string(),
                )).collect::<Vec<_>>();
            let _ = tx.send(data);
        });
        let v = self.clone();
        glib::idle_add_local(move || match rx.try_recv() {
            Ok(mut data) => {
                data.installed = v.load_installed();
                *v.data.borrow_mut() = data;
                v.render_library();
                v.refresh_account_header();
                if !v.detail_id.borrow().is_empty() {
                    v.render_detail();
                }
                glib::ControlFlow::Break
            }
            Err(std::sync::mpsc::TryRecvError::Empty) => glib::ControlFlow::Continue,
            Err(_) => glib::ControlFlow::Break,
        });
    }

    fn reload_silent(&self) {
        let (tx, rx) = std::sync::mpsc::channel::<(Vec<(String, String)>, serde_json::Value)>();
        std::thread::spawn(move || {
            let dirs = scan_instance_dirs();
            let st = MinecraftManager::status().unwrap_or_default();
            let _ = tx.send((dirs, st));
        });
        let vv = self.clone();
        glib::idle_add_local(move || match rx.try_recv() {
            Ok((dirs, st)) => {
                let mut list = Vec::new();
                let mut by_dir: std::collections::HashMap<String, Vec<String>> = std::collections::HashMap::new();
                for (dir, id) in &dirs {
                    by_dir.entry(dir.clone()).or_default().push(id.clone());
                }
                for ids in by_dir.values() {
                    for id in drop_base_copies(ids) {
                        list.push(vv.make_inst(&id, true));
                    }
                }
                let legacy: Vec<String> = st.get("installed_versions").and_then(|a| a.as_array()).cloned().unwrap_or_default()
                    .into_iter().filter_map(|x| x.as_str().map(str::to_string)).collect();
                for id in drop_base_copies(&legacy) {
                    if !list.iter().any(|i: &Inst| i.id == id) {
                        list.push(vv.make_inst(&id, false));
                    }
                }
                vv.data.borrow_mut().installed = list;
                vv.render_library();
                if !vv.detail_id.borrow().is_empty() {
                    vv.render_detail();
                }
                glib::ControlFlow::Break
            }
            Err(std::sync::mpsc::TryRecvError::Empty) => glib::ControlFlow::Continue,
            Err(_) => glib::ControlFlow::Break,
        });
    }

    // ============ library (Carbon HomeGrid) ============

    fn sorted_filtered(&self) -> Vec<Inst> {
        let data = self.data.borrow();
        let q = self.search_entry.text().to_string().to_lowercase();
        let mut list: Vec<Inst> = data.installed.iter()
            .filter(|i| q.is_empty() || i.disp.to_lowercase().contains(&q) || i.id.to_lowercase().contains(&q))
            .cloned().collect();
        match self.cfg_sort().as_str() {
            "Most played" => list.sort_by(|a, b| b.secs.cmp(&a.secs)),
            "Last played" => list.sort_by(|a, b| b.last.cmp(&a.last)),
            "Game version" => list.sort_by(|a, b| b.mc_ver.cmp(&a.mc_ver)),
            _ => list.sort_by(|a, b| a.disp.to_lowercase().cmp(&b.disp.to_lowercase())),
        }
        list.sort_by(|a, b| b.fav.cmp(&a.fav));
        list
    }

    fn render_library(&self) {
        while let Some(c) = self.flow.first_child() {
            self.flow.remove(&c);
        }
        let list = self.sorted_filtered();
        let running = MinecraftManager::running_pids();
        let running_any = !running.is_empty();
        let total = self.data.borrow().installed.len();
        let msg = if list.is_empty() {
            if total == 0 {
                "No instances — press ADD INSTANCE.".to_string()
            } else {
                "No matches.".to_string()
            }
        } else if list.len() == total {
            format!("{} instance(s)", total)
        } else {
            format!("{} of {} shown", list.len(), total)
        };
        self.lib_status.set_text(&msg);
        let compact = self.cfg("McCompact") == "1";
        for inst in &list {
            let tile = gtk::FlowBoxChild::new();
            tile.set_width_request((self.cfg("McTileSize").parse().unwrap_or(64) + if compact { 110 } else { 126 }).max(150));
            let inner = gtk::Box::new(gtk::Orientation::Vertical, if compact { 2 } else { 4 });
            let pad = if compact { 4 } else { 8 };
            inner.set_margin_top(pad);
            inner.set_margin_bottom(pad);
            inner.set_margin_start(pad);
            inner.set_margin_end(pad);
            inner.add_css_class("page-card");
            inner.add_css_class("mc-tile");
            let kind = self.inst_cfg(&inst.id, "Icon");
            let tsize: i32 = self.cfg("McTileSize").parse().unwrap_or(64);
            let img = inst_icon_image(&inst.id, &kind, self.state.theme.is_dark(), tsize);
            img.set_halign(gtk::Align::Center);
            inner.append(&img);
            let name = gtk::Label::new(Some(&inst.disp));
            name.set_halign(gtk::Align::Center);
            name.set_ellipsize(gtk::pango::EllipsizeMode::Middle);
            name.add_css_class("details-title");
            inner.append(&name);
            let sub = gtk::Label::new(Some(&format!("{} • {}", inst.mc_ver,
                if inst.loader == "vanilla" { "Vanilla".to_string() } else { inst.loader.clone() })));
            sub.set_halign(gtk::Align::Center);
            sub.set_opacity(0.6);
            sub.add_css_class("time-label");
            inner.append(&sub);
            let badge_row = gtk::Box::new(gtk::Orientation::Horizontal, 4);
            badge_row.set_halign(gtk::Align::Center);
            if inst.fav {
                badge_row.append(&helpers::themed_image("star_gold", self.state.theme.is_dark(), 14));
            }
            if running_any {
                let dot = gtk::Label::new(Some("● running"));
                dot.add_css_class("time-label");
                badge_row.append(&dot);
            }
            if !inst.isolated {
                let sh = gtk::Label::new(Some("shared"));
                sh.set_opacity(0.55);
                sh.add_css_class("time-label");
                badge_row.append(&sh);
            }
            inner.append(&badge_row);
            // install progress overlay
            let prog = self.install_prog.borrow().get(&inst.id).cloned();
            if let Some(p) = prog {
                let bar = gtk::ProgressBar::new();
                if p < 0.0 {
                    bar.pulse();
                } else {
                    bar.set_fraction(p.clamp(0.0, 1.0));
                }
                inner.append(&bar);
            }
            tile.set_child(Some(&inner));
            let v = self.clone();
            let id = inst.id.clone();
            let gesture = gtk::GestureClick::new();
            gesture.set_button(0);
            gesture.connect_pressed(move |g, _n, x, y| {
                if g.current_button() == 3 {
                    v.show_tile_menu(&id, x, y);
                } else if g.current_button() == 1 {
                    v.open_detail(&id);
                }
            });
            tile.add_controller(gesture);
            self.flow.insert(&tile, -1);
        }
    }

    fn show_tile_menu(&self, id: &str, x: f64, y: f64) {
        let is_vanilla = self.get_inst(id).map(|i| i.loader == "vanilla").unwrap_or(false);
        let pop = gtk::Popover::new();
        let box_ = gtk::Box::new(gtk::Orientation::Vertical, 2);
        box_.set_margin_top(6);
        box_.set_margin_bottom(6);
        box_.set_margin_start(6);
        box_.set_margin_end(6);
        let running = !MinecraftManager::running_pids().is_empty();
        let is_dark = self.state.theme.is_dark();
        for (micon, label, action) in [
            ("media-playback-stop-symbolic", "Stop", "play"),
            ("application-x-addon-symbolic", "Addons", "addons"),
            ("folder-symbolic", "Open folder", "folder"),
            ("document-edit-symbolic", "Rename", "rename"),
            ("non-starred-symbolic", "Favorite", "fav"),
            ("user-trash-symbolic", "Delete", "delete"),
        ] {
            if action == "addons" && is_vanilla {
                continue;
            }
            let b = if action == "play" && !running {
                menu_btn_png("play", label, is_dark)
            } else if action == "fav" {
                menu_btn_png("star_gray", label, is_dark)
            } else if action == "delete" {
                menu_btn_png("remove", label, is_dark)
            } else if action == "folder" {
                menu_btn_png("folder", label, is_dark)
            } else {
                menu_btn(micon, label)
            };
            let v = self.clone();
            let idc = id.to_string();
            let act = action.to_string();
            let popc = pop.clone();
            b.connect_clicked(move |_| {
                popc.popdown();
                match act.as_str() {
                    "play" => v.quick_play(&idc),
                    "addons" => {
                        v.open_detail(&idc);
                        v.detail_tabs.set_visible_child_name("addons");
                    }
                    "folder" => v.open_inst_folder(&idc),
                    "rename" => v.show_rename(&idc),
                    "fav" => v.toggle_fav(&idc),
                    "delete" => v.confirm_delete(&idc),
                    _ => {}
                }
            });
            box_.append(&b);
        }
        pop.set_child(Some(&box_));
        if let Some(child) = self.flow.first_child() {
            let rect = gdk::Rectangle::new(x as i32, y as i32, 1, 1);
            pop.set_parent(&child);
            pop.set_pointing_to(Some(&rect));
            let popc = pop.clone();
            pop.connect_closed(move |_| {
                popc.unparent();
            });
            pop.popup();
        }
    }

    fn get_inst(&self, id: &str) -> Option<Inst> {
        self.data.borrow().installed.iter().find(|i| i.id == id).cloned()
    }

    fn open_detail(&self, id: &str) {
        *self.detail_id.borrow_mut() = id.to_string();
        self.detail_tabs.set_visible_child_name("overview");
        self.det_overview_btn.set_active(true);
        self.render_detail();
        self.main_stack.set_visible_child_name("detail");
    }

    fn open_inst_folder(&self, id: &str) {
        if let Some(inst) = self.get_inst(id) {
            let dir = inst_dir(&inst.id, inst.isolated);
            self.state.integration.open_url(&format!("file://{}", dir.display()));
        }
    }

    fn toggle_fav(&self, id: &str) {
        let cur = self.inst_cfg(id, "Fav") == "1";
        self.set_inst_cfg(id, "Fav", if cur { "0" } else { "1" });
        self.reload_silent();
    }

    fn toggle_fav_selected(&self) {
        let id = self.detail_id.borrow().clone();
        if !id.is_empty() {
            self.toggle_fav(&id);
        }
    }

    fn show_rename(&self, id: &str) {
        let dlg = adw::Dialog::new();
        dlg.set_title("Rename instance");
        dlg.set_content_width(380);
        let (header, x_btn) = helpers::modal_header("Rename instance");
        let content = gtk::Box::new(gtk::Orientation::Vertical, 8);
        content.add_css_class("modal-bg");
        content.append(&header);
        let entry = gtk::Entry::new();
        entry.set_text(&self.get_inst(id).map(|i| i.disp).unwrap_or_default());
        entry.set_margin_start(16);
        entry.set_margin_end(16);
        content.append(&entry);
        let save = gtk::Button::with_label("Save");
        save.add_css_class("add-btn");
        save.set_margin_start(16);
        save.set_margin_end(16);
        save.set_margin_bottom(16);
        content.append(&save);
        dlg.set_child(Some(&content));
        {
            let d = dlg.clone();
            x_btn.connect_clicked(move |_| { d.close(); });
        }
        let v = self.clone();
        let idc = id.to_string();
        let d = dlg.clone();
        save.connect_clicked(move |_| {
            let name = entry.text().to_string().trim().to_string();
            if !name.is_empty() {
                v.set_inst_cfg(&idc, "Name", &name);
                v.reload_silent();
                if *v.detail_id.borrow() == idc {
                    v.render_detail();
                }
            }
            d.close();
        });
        dlg.present(Some(&self.parent));
    }

    fn confirm_delete(&self, id: &str) {
        let dlg = adw::MessageDialog::new(Some(&self.parent), Some("Delete instance?"), Some(&format!("Remove {} and its files?", id)));
        if let Some(root) = self.parent.root() {
            if let Ok(win) = root.downcast::<gtk::Window>() {
                dlg.set_transient_for(Some(&win));
                dlg.set_modal(true);
            }
        }
        dlg.add_response("cancel", "Cancel");
        dlg.add_response("delete", "Delete");
        dlg.set_response_appearance("delete", adw::ResponseAppearance::Destructive);
        dlg.set_default_response(Some("cancel"));
        dlg.set_close_response("cancel");
        let v = self.clone();
        let idc = id.to_string();
        dlg.connect_response(None, move |d, resp| {
            if resp == "delete" {
                v.do_delete(&idc);
            }
            d.close();
        });
        dlg.present();
    }

    fn do_delete(&self, id: &str) {
        let inst = match self.get_inst(id) {
            Some(i) => i,
            None => return,
        };
        let dir = inst_dir(&inst.id, inst.isolated);
        let mut errs = Vec::new();
        if inst.isolated {
            if dir.exists() {
                if let Err(e) = std::fs::remove_dir_all(&dir) {
                    errs.push(format!("{}: {}", dir.display(), e));
                }
            }
        }
        // legacy copy may exist alongside (stale duplicate installs)
        let legacy_ver = legacy_dir().join("versions").join(&inst.id);
        if legacy_ver.exists() {
            if let Err(e) = std::fs::remove_dir_all(&legacy_ver) {
                errs.push(format!("{}: {}", legacy_ver.display(), e));
            }
        }
        // verify: rescan dirs — id must be gone
        let still_there = (inst.isolated && dir.exists()) || legacy_ver.exists();
        for k in ["Fav", "Time", "Last", "Name", "Ram", "Java", "Account"] {
            self.state.config.remove_launcher_value_in("User Settings", &format!("Mc{}_{}", k, safe_id(id)));
        }
        if *self.detail_id.borrow() == id {
            *self.detail_id.borrow_mut() = String::new();
            self.main_stack.set_visible_child_name("library");
        }
        if errs.is_empty() && !still_there {
            self.toast("Deleted", id);
        } else {
            errs.push(if still_there { "files remain on disk".to_string() } else { String::new() });
            let msg: Vec<String> = errs.into_iter().filter(|s| !s.is_empty()).collect();
            self.toast("Delete incomplete", &msg.join("\n"));
        }
        self.reload_silent();
    }

    fn toast(&self, heading: &str, body: &str) {
        helpers::present_msg(&self.parent, heading, body);
    }

    // ============ detail ============

    fn selected_account(&self) -> (String, String) {
        let data = self.data.borrow();
        let names: Vec<(String, String)> = data.accounts.iter().map(|(i, n, _, _)| (i.clone(), n.clone())).collect();
        let saved = self.state.config.launcher_value("McAccount").unwrap_or_default();
        if let Some((aid, name)) = names.iter().find(|(_, n)| n == &saved) {
            return (aid.clone(), name.clone());
        }
        names.first().cloned().unwrap_or_default()
    }

    fn refresh_account_header(&self) {
        let (aid, name) = self.selected_account();
        if aid.is_empty() {
            self.account_label.set_text("Inicia sesión");
            self.account_head.set_pixel_size(24);
            self.account_head.set_icon_name(Some("avatar-default-symbolic"));
            self.account_btn.set_tooltip_text(Some("Sin cuenta — clic para añadir"));
            return;
        }
        self.account_label.set_text(&name);
        self.account_btn.set_tooltip_text(Some(&format!("{} — clic para cambiar", name)));
        self.account_head.set_pixel_size(24);
        self.account_head.set_icon_name(Some("avatar-default-symbolic"));
        let img = self.account_head.clone();
        let (tx, rx) = std::sync::mpsc::channel::<Option<String>>();
        std::thread::spawn(move || {
            let head = MinecraftManager::ely_skin(&name, &aid, false).ok()
                .and_then(|d| d.get("head").and_then(|x| x.as_str()).map(str::to_string));
            let _ = tx.send(head);
        });
        glib::idle_add_local(move || match rx.try_recv() {
            Ok(Some(path)) => {
                if let Some(tex) = helpers::load_texture(&path) {
                    img.set_paintable(Some(&tex));
                    img.set_pixel_size(24);
                }
                glib::ControlFlow::Break
            }
            Ok(None) => glib::ControlFlow::Break,
            Err(std::sync::mpsc::TryRecvError::Empty) => glib::ControlFlow::Continue,
            Err(_) => glib::ControlFlow::Break,
        });
    }

    fn rebuild_account_popover(&self) {
        self.account_pop.set_child(None::<&gtk::Box>);
        let list = gtk::Box::new(gtk::Orientation::Vertical, 4);
        list.set_margin_top(8);
        list.set_margin_bottom(8);
        list.set_margin_start(8);
        list.set_margin_end(8);
        list.set_size_request(260, -1);
        let (cur_aid, cur_name) = self.selected_account();
        let accounts = self.data.borrow().accounts.clone();
        if accounts.is_empty() {
            list.append(&note("Sin cuentas. Jugarás offline."));
        }
        for (aid, name, offline, ely) in &accounts {
            let row_btn = gtk::Button::new();
            row_btn.add_css_class("mc-row");
            let row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
            row.set_hexpand(true);
            let head_img = gtk::Image::new();
            head_img.set_pixel_size(24);
            head_img.set_icon_name(Some("avatar-default-symbolic"));
            row.append(&head_img);
            let tag = if *ely { "Ely.by" } else if *offline { "Offline" } else { "MS" };
            let lbl = gtk::Label::new(Some(&format!("{}  ·  {}", name, tag)));
            lbl.set_halign(gtk::Align::Start);
            lbl.set_hexpand(true);
            lbl.set_ellipsize(gtk::pango::EllipsizeMode::End);
            row.append(&lbl);
            if *aid == cur_aid || *name == cur_name {
                let check = gtk::Image::from_icon_name("emblem-ok-symbolic");
                check.set_pixel_size(16);
                row.append(&check);
            }
            row_btn.set_child(Some(&row));
            {
                let nm = name.clone();
                let aidc = aid.clone();
                let (tx, rx) = std::sync::mpsc::channel::<Option<String>>();
                std::thread::spawn(move || {
                    let head = MinecraftManager::ely_skin(&nm, &aidc, false).ok()
                        .and_then(|d| d.get("head").and_then(|x| x.as_str()).map(str::to_string));
                    let _ = tx.send(head);
                });
                glib::idle_add_local(move || match rx.try_recv() {
                    Ok(Some(path)) => {
                        if let Some(tex) = helpers::load_texture(&path) {
                            head_img.set_paintable(Some(&tex));
                            head_img.set_pixel_size(24);
                        }
                        glib::ControlFlow::Break
                    }
                    Ok(None) => glib::ControlFlow::Break,
                    Err(std::sync::mpsc::TryRecvError::Empty) => glib::ControlFlow::Continue,
                    Err(_) => glib::ControlFlow::Break,
                });
            }
            {
                let v = self.clone();
                let pop = self.account_pop.clone();
                let nm = name.clone();
                row_btn.connect_clicked(move |_| {
                    v.set_cfg("McAccount", &nm);
                    v.refresh_account_header();
                    v.rebuild_account_popover();
                    pop.popdown();
                });
            }
            list.append(&row_btn);
        }
        let sep = gtk::Separator::new(gtk::Orientation::Horizontal);
        list.append(&sep);
        let add_btn = btn_with_icon("list-add-symbolic", "Añadir cuenta…");
        add_btn.add_css_class("settings-btn");
        {
            let v = self.clone();
            let pop = self.account_pop.clone();
            add_btn.connect_clicked(move |_| {
                pop.popdown();
                v.show_mc_settings_tab("accounts");
            });
        }
        list.append(&add_btn);
        self.account_pop.set_child(Some(&list));
    }

    fn inst_ram(&self, id: &str) -> u32 {
        if let Ok(v) = self.inst_cfg(id, "Ram").parse::<u32>() {
            if v >= 512 {
                return v;
            }
        }
        self.cfg("McRam").parse().unwrap_or(2048)
    }

    fn inst_java(&self, id: &str) -> String {
        let j = self.inst_cfg(id, "Java");
        if !j.is_empty() {
            return j;
        }
        self.data.borrow().java_selected.clone()
    }

    fn render_detail(&self) {
        let id = self.detail_id.borrow().clone();
        let inst = match self.get_inst(&id) {
            Some(i) => i,
            None => return,
        };
        self.detail_name.set_text(&inst.disp);
        {
            let kind = self.inst_cfg(&inst.id, "Icon");
            let fresh = inst_icon_image(&inst.id, &kind, self.state.theme.is_dark(), 44);
            if let Some(p) = fresh.paintable() {
                self.detail_icon.set_paintable(Some(&p));
                self.detail_icon.set_pixel_size(44);
            }
            let ov_img = inst_icon_image(&inst.id, &kind, self.state.theme.is_dark(), 40);
            if let Some(p) = ov_img.paintable() {
                self.ov_icon_btn.set_child(Some(&{
                    let img = gtk::Image::new();
                    img.set_paintable(Some(&p));
                    img.set_pixel_size(40);
                    img
                }));
            }
            self.ov_name_entry.set_text(&inst.disp);
        }
        let running = !MinecraftManager::running_pids().is_empty();
        let loader_txt = if inst.loader == "vanilla" { "Vanilla".to_string() } else { format!("{} {}", inst.loader, inst.loader_ver) };
        let mut sub = format!("{} • {} • {}", inst.mc_ver, loader_txt, fmt_playtime(inst.secs));
        if !inst.last.is_empty() {
            sub.push_str(&format!(" • last {}", fmt_date(&inst.last)));
        }
        self.detail_sub.set_text(&sub);
        if running {
            set_btn_icon_label(&self.detail_play, "media-playback-stop-symbolic", "STOP");
            paint_btn(&self.detail_play, "#E53935");
        } else {
            set_themed_btn(&self.detail_play, "play", "PLAY", self.state.theme.is_dark(), 20);
            paint_accent(&self.detail_play, &self.state.theme);
        }
        self.detail_star.set_child(Some(&helpers::themed_image(
            if inst.fav { "star_gold" } else { "star_gray" }, self.state.theme.is_dark(), 20)));
        // vanilla has no addons: hide the whole category
        let is_vanilla = inst.loader == "vanilla";
        self.addons_tab_wrap.set_visible(!is_vanilla);
        if is_vanilla {
            self.detail_tabs.set_visible_child_name("overview");
            self.det_overview_btn.set_active(true);
        }
        // overview cards
        self.ov_mc.set_text(&inst.mc_ver);
        let loader_txt = if inst.loader == "vanilla" { "Vanilla".to_string() } else { format!("{} {}", inst.loader, inst.loader_ver) };
        self.ov_loader.set_text(&loader_txt);
        let (nd, _items) = self.addon_dir_items("mods");
        let _ = nd;
        let count = self.count_addons();
        self.ov_addons.set_text(&format!("{}", count));
        self.ov_played.set_text(&fmt_playtime(inst.secs));
        let last_txt = if inst.last.is_empty() { "—".to_string() } else { fmt_date(&inst.last) };
        self.ov_last.set_text(&last_txt);
        self.set_jvm.set_text(&self.inst_cfg(&inst.id, "Jvm"));
        self.set_res_w.set_text(&self.inst_cfg(&inst.id, "ResW"));
        self.set_res_h.set_text(&self.inst_cfg(&inst.id, "ResH"));
        self.set_wrapper.set_text(&self.inst_cfg(&inst.id, "Wrapper"));
        self.set_prehook.set_text(&self.inst_cfg(&inst.id, "PreHook"));
        let ram = self.inst_ram(&inst.id);
        self.set_ram.set_value(ram as f64);
        self.set_ram_lbl.set_text(&format!("{} MB", ram));
        self.set_java.set_text(&self.inst_cfg(&inst.id, "Java"));
        self.render_addons();
        self.do_refresh_logs();
    }

    fn count_addons(&self) -> usize {
        let id = self.detail_id.borrow().clone();
        let inst = match self.get_inst(&id) {
            Some(i) => i,
            None => return 0,
        };
        let dir = inst_dir(&inst.id, inst.isolated);
        let mut n = 0;
        for sub in ["mods", "resourcepacks", "shaderpacks"] {
            if let Ok(entries) = std::fs::read_dir(dir.join(sub)) {
                n += entries.flatten().filter(|e| {
                    e.path().is_file() && !e.file_name().to_string_lossy().ends_with(".corky.json")
                }).count();
            }
        }
        n
    }

    fn toggle_play_selected(&self) {
        if !MinecraftManager::running_pids().is_empty() {
            self.do_stop();
            return;
        }
        let id = self.detail_id.borrow().clone();
        if id.is_empty() {
            return;
        }
        self.do_launch_inst(&id);
    }

    fn quick_play(&self, id: &str) {
        if !MinecraftManager::running_pids().is_empty() {
            self.do_stop();
            return;
        }
        self.do_launch_inst(id);
    }

    fn do_launch_inst(&self, id: &str) {
        let inst = match self.get_inst(id) {
            Some(i) => i,
            None => return,
        };
        // stale tile guard: files deleted outside the app
        let dir = inst_dir(&inst.id, inst.isolated);
        if !dir.join("versions").join(&inst.id).join(format!("{}.json", inst.id)).exists() {
            self.toast("Instance files missing",
                &format!("{} has no game files on disk (deleted outside the app?). Remove the tile.", inst.disp));
            self.confirm_delete(&inst.id);
            return;
        }
        // pre-flight: warn when no compatible Java exists ("te falta Java X")
        let dir_s = inst_dir(&inst.id, inst.isolated).display().to_string();
        let req = MinecraftManager::java_required(&inst.id, &dir_s).ok();
        let satisfied = req.as_ref().and_then(|d| d.get("satisfied")).and_then(|x| x.as_bool()).unwrap_or(true);
        if !satisfied {
            let want = req.as_ref().map(|d| {
                let r = d.get("required_java").and_then(|x| x.as_u64()).unwrap_or(17);
                let m = d.get("max_java").and_then(|x| x.as_u64()).unwrap_or(99);
                if m >= 99 { format!("Java {}", r) } else { format!("Java {}-{}", r, m) }
            }).unwrap_or("Java".to_string());
            self.toast(&format!("Te falta {}", want),
                &format!("{} needs {}. Install it in Settings → Java.", inst.disp, want));
            self.show_mc_settings();
            return;
        }
        let (acc_id, acc_name) = self.selected_account();
        if acc_id.is_empty() {
            self.toast("No account", "Add an account in Settings first.");
            return;
        }
        // per-instance dir: temporarily point plugin via env? The plugin uses
        // fixed MC_DIR. For isolated instances we pass --mc-dir override.
        let dir = inst_dir(&inst.id, inst.isolated);
        std::fs::create_dir_all(&dir).ok();
        let ram = self.inst_ram(&inst.id);
        let ram_min: u32 = 0;
        let java = self.inst_java(&inst.id);
        let jvm_extra: Vec<String> = [self.inst_cfg(&inst.id, "Jvm"), self.cfg("McJvmArgs")]
            .iter().flat_map(|s| s.split_whitespace().map(str::to_string)).collect();
        let res = {
            let mut w = self.inst_cfg(&inst.id, "ResW");
            let mut h = self.inst_cfg(&inst.id, "ResH");
            if w.is_empty() { w = self.cfg("McResW"); }
            if h.is_empty() { h = self.cfg("McResH"); }
            if w.is_empty() || h.is_empty() { String::new() } else { format!("{}x{}", w, h) }
        };
        let opts = crate::backend::external::LaunchOpts {
            max_mb: ram,
            min_mb: if ram_min >= ram { 0 } else { ram_min },
            jvm_args: jvm_extra,
            resolution: res,
            wrapper: self.inst_cfg(&inst.id, "Wrapper"),
            pre_hook: self.inst_cfg(&inst.id, "PreHook"),
        };
        let rx = MinecraftManager::spawn_launch_isolated(
            inst.id.clone(), Some(acc_id.clone()), ram, java, dir.display().to_string(), Some(opts));
        self.lib_status.set_text(&format!("Launching {} as {}…", inst.disp, acc_name));
        let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0);
        self.sessions.borrow_mut().insert(acc_id.clone(), (inst.id.clone(), now));
        let v = self.clone();
        let idc = inst.id.clone();
        crate::backend::plugin_process::pump_to_idle(rx, move |ev| {
            match ev {
                crate::backend::plugin_process::PluginEvent::Done(val) => {
                    let pid = val.get("pid").and_then(|x| x.as_i64()).unwrap_or(0);
                    v.lib_status.set_text(&format!("{} running (pid {}).", idc, pid));
                    v.render_library();
                    v.render_detail();
                    false
                }
                crate::backend::plugin_process::PluginEvent::Error { message, .. } => {
                    v.lib_status.set_text(&format!("Launch failed: {}", message));
                    v.sessions.borrow_mut().remove(&acc_id);
                    v.toast("Launch failed", &message);
                    false
                }
                _ => true,
            }
        });
    }

    fn do_stop(&self) {
        let (tx, rx) = std::sync::mpsc::channel::<Result<String, String>>();
        std::thread::spawn(move || {
            let _ = tx.send(MinecraftManager::stop(None).map(|_| String::new()).map_err(|e| e.to_string()));
        });
        let v = self.clone();
        glib::idle_add_local(move || match rx.try_recv() {
            Ok(Ok(_)) => {
                v.lib_status.set_text("Stopped.");
                v.tick_sessions();
                v.render_library();
                v.render_detail();
                glib::ControlFlow::Break
            }
            Ok(Err(e)) => {
                v.toast("Stop failed", &e);
                glib::ControlFlow::Break
            }
            Err(std::sync::mpsc::TryRecvError::Empty) => glib::ControlFlow::Continue,
            Err(_) => glib::ControlFlow::Break,
        });
    }

    fn tick_sessions(&self) {
        let running = MinecraftManager::running_pids();
        let alive: std::collections::HashSet<String> = running.into_iter().map(|(a, _)| a).collect();
        let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0);
        let mut done = Vec::new();
        for (acc, (inst, start)) in self.sessions.borrow().iter() {
            if !alive.contains(acc) {
                let elapsed = (now - start).max(0) as u64;
                let cur: u64 = self.inst_cfg(inst, "Time").parse().unwrap_or(0);
                self.set_inst_cfg(inst, "Time", &(cur + elapsed).to_string());
                let date = chrono_date(now);
                self.set_inst_cfg(inst, "Last", &date);
                done.push(acc.clone());
            }
        }
        for acc in done {
            self.sessions.borrow_mut().remove(&acc);
        }
        if !self.sessions.borrow().is_empty() {
            return;
        }
    }

    fn show_icon_popover(&self) {
        let id = self.detail_id.borrow().clone();
        if id.is_empty() {
            return;
        }
        let pop = gtk::Popover::new();
        let box_ = gtk::Box::new(gtk::Orientation::Vertical, 2);
        box_.set_margin_top(6);
        box_.set_margin_bottom(6);
        box_.set_margin_start(6);
        box_.set_margin_end(6);
        let is_dark = self.state.theme.is_dark();
        for (kind, label, asset) in [
            ("grass", "Grass (GDL default)", "inst_grass.png"),
            ("ely", "Ely", if is_dark { "minecraft.png" } else { "minecraft_dark.png" }),
            ("block", "Block", if is_dark { "inst_block_white.png" } else { "inst_block_black.png" }),
            ("custom:", "Custom file…", ""),
        ] {
            let b = gtk::Button::new();
            let row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
            if asset.is_empty() {
                row.append(&sym("image-x-generic-symbolic", 32));
            } else if let Some(tex) = helpers::load_texture(&format!("{}/.local/share/corkytux/assets/{}", std::env::var("HOME").unwrap_or_default(), asset)) {
                let img = gtk::Image::new();
                img.set_paintable(Some(&tex));
                img.set_pixel_size(32);
                row.append(&img);
            }
            row.append(&gtk::Label::new(Some(label)));
            b.set_child(Some(&row));
            b.add_css_class("settings-btn");
            let v = self.clone();
            let kindc = kind.to_string();
            let popc = pop.clone();
            b.connect_clicked(move |_| {
                popc.popdown();
                if kindc == "custom:" {
                    v.pick_custom_icon();
                } else {
                    v.set_icon_kind(&kindc);
                }
            });
            box_.append(&b);
        }
        pop.set_child(Some(&box_));
        pop.set_parent(&self.detail_icon);
        let popc = pop.clone();
        pop.connect_closed(move |_| {
            popc.unparent();
        });
        pop.popup();
    }

    fn set_icon_kind(&self, kind: &str) {
        let id = self.detail_id.borrow().clone();
        if id.is_empty() {
            return;
        }
        self.set_inst_cfg(&id, "Icon", kind);
        self.render_detail();
        self.render_library();
    }

    fn pick_custom_icon(&self) {
        let dlg = gtk::FileDialog::new();
        dlg.set_title("Choose instance icon");
        let filter = gtk::FileFilter::new();
        filter.set_name(Some("Images (png, jpg, svg, webp)"));
        filter.add_pattern("*.png");
        filter.add_pattern("*.jpg");
        filter.add_pattern("*.jpeg");
        filter.add_pattern("*.svg");
        filter.add_pattern("*.webp");
        let filters = gio::ListStore::new::<gtk::FileFilter>();
        filters.append(&filter);
        dlg.set_filters(Some(&filters));
        let v = self.clone();
        dlg.open(Some(&self.parent), gio::Cancellable::NONE, move |res| {
            let file = match res {
                Ok(f) => f,
                Err(_) => {
                    v.render_detail();
                    return;
                }
            };
            let path = file.path().map(|p| p.display().to_string()).unwrap_or_default();
            if path.is_empty() {
                v.render_detail();
                return;
            }
            let id = v.detail_id.borrow().clone();
            if !id.is_empty() {
                v.set_inst_cfg(&id, "Icon", &format!("custom:{}", path));
            }
            v.render_detail();
            v.render_library();
        });
    }

    fn do_repair(&self) {
        let id = self.detail_id.borrow().clone();
        let inst = match self.get_inst(&id) {
            Some(i) => i,
            None => return,
        };
        let dir = inst_dir(&inst.id, inst.isolated).display().to_string();
        self.lib_status.set_text(&format!("Repairing {}…", inst.disp));
        self.main_stack.set_visible_child_name("library");
        let rx = MinecraftManager::spawn_install_isolated(
            inst.mc_ver.clone(), inst.loader.clone(), inst.loader_ver.clone(), dir);
        let v = self.clone();
        let idc = inst.id.clone();
        crate::backend::plugin_process::pump_to_idle(rx, move |ev| {
            match ev {
                crate::backend::plugin_process::PluginEvent::Done(_) => {
                    v.lib_status.set_text(&format!("{} repaired.", idc));
                    v.toast("Repaired", &idc);
                    v.reload_silent();
                    false
                }
                crate::backend::plugin_process::PluginEvent::Error { message, .. } => {
                    v.lib_status.set_text(&format!("Repair failed: {}", message));
                    false
                }
                _ => true,
            }
        });
    }

    fn save_instance_settings(&self) {
        let id = self.detail_id.borrow().clone();
        if id.is_empty() {
            return;
        }
        self.set_inst_cfg(&id, "Ram", &(self.set_ram.value() as u32).to_string());
        self.set_inst_cfg(&id, "Java", self.set_java.text().trim());
        self.set_inst_cfg(&id, "Jvm", self.set_jvm.text().trim());
        self.set_inst_cfg(&id, "ResW", self.set_res_w.text().trim().chars().filter(|c| c.is_ascii_digit()).collect::<String>().as_str());
        self.set_inst_cfg(&id, "ResH", self.set_res_h.text().trim().chars().filter(|c| c.is_ascii_digit()).collect::<String>().as_str());
        self.set_inst_cfg(&id, "Wrapper", self.set_wrapper.text().trim());
        self.set_inst_cfg(&id, "PreHook", self.set_prehook.text().trim());
        self.toast("Saved", "Instance settings saved.");
        self.reload_silent();
        self.render_detail();
    }

    // ============ addons tab ============

    fn addon_dir(&self) -> std::path::PathBuf {
        let id = self.detail_id.borrow().clone();
        let (dir, isolated) = match self.get_inst(&id) {
            Some(i) => (inst_dir(&i.id, i.isolated), i.isolated),
            None => (legacy_dir(), false),
        };
        let sub = match self.addons_type.borrow().as_str() {
            "shaders" => "shaderpacks",
            "resourcepacks" => "resourcepacks",
            _ => "mods",
        };
        let d = dir.join(sub);
        if isolated {
            std::fs::create_dir_all(&d).ok();
        }
        d
    }

    fn addon_dir_items(&self, _which: &str) -> (std::path::PathBuf, Vec<AddonRow>) {
        (self.addon_dir(), Vec::new())
    }

    fn inst_base_dir(&self) -> std::path::PathBuf {
        let id = self.detail_id.borrow().clone();
        match self.get_inst(&id) {
            Some(i) => inst_dir(&i.id, i.isolated),
            None => legacy_dir(),
        }
    }

    fn load_addon_rows(&self) -> Vec<AddonRow> {
        let id = self.detail_id.borrow().clone();
        let inst = match self.get_inst(&id) {
            Some(i) => i,
            None => return Vec::new(),
        };
        let dir = inst_dir(&inst.id, inst.isolated);
        let sub = match self.addons_type.borrow().as_str() {
            "shaders" => "shaderpacks",
            "resourcepacks" => "resourcepacks",
            _ => "mods",
        };
        let target = dir.join(sub);
        let mut rows = Vec::new();
        let at = self.addons_type.borrow().clone();
        if let Ok(entries) = std::fs::read_dir(&target) {
            for e in entries.flatten() {
                let p = e.path();
                if !p.is_file() {
                    continue;
                }
                let name = p.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
                if name.ends_with(".corky.json") {
                    continue;
                }
                let lower = name.to_lowercase();
                let wanted = if at == "mods" {
                    lower.ends_with(".jar") || lower.ends_with(".jar.disabled")
                } else {
                    lower.ends_with(".zip") || lower.ends_with(".zip.disabled")
                };
                if !wanted {
                    continue;
                }
                let enabled = !name.ends_with(".disabled");
                let mut row = AddonRow {
                    file: name.clone(),
                    title: name.clone(),
                    version: String::new(),
                    platform: "local".to_string(),
                    enabled,
                    icon_url: String::new(),
                    project_id: String::new(),
                };
                let sc = target.join(format!("{}.corky.json", name));
                if sc.is_file() {
                    if let Ok(txt) = std::fs::read_to_string(&sc) {
                        if let Ok(meta) = serde_json::from_str::<serde_json::Value>(&txt) {
                            row.title = meta.get("title").and_then(|x| x.as_str()).unwrap_or(&name).to_string();
                            row.version = meta.get("version_number").and_then(|x| x.as_str()).unwrap_or("").to_string();
                            row.project_id = meta.get("project_id").and_then(|x| x.as_str()).unwrap_or("").to_string();
                            row.icon_url = meta.get("icon_url").and_then(|x| x.as_str()).unwrap_or("").to_string();
                            if !row.project_id.is_empty() {
                                row.platform = "modrinth".to_string();
                            }
                        }
                    }
                }
                rows.push(row);
            }
        }
        rows.sort_by(|a, b| a.title.to_lowercase().cmp(&b.title.to_lowercase()));
        rows
    }

    fn render_addons(&self) {
        while let Some(c) = self.addons_box.first_child() {
            self.addons_box.remove(&c);
        }
        let q = self.addons_search.text().to_string().to_lowercase();
        let plat = match self.plat_drop.selected() {
            1 => "modrinth",
            2 => "local",
            _ => "all",
        };
        let rows: Vec<AddonRow> = self.load_addon_rows().into_iter()
            .filter(|r| (q.is_empty() || r.title.to_lowercase().contains(&q) || r.file.to_lowercase().contains(&q))
                && (plat == "all" || r.platform == plat))
            .collect();
        self.addons_count.set_text(&format!("{} {}", rows.len(), self.addons_type.borrow().as_str()));
        self.addons_dir_lbl.set_text(&format!("{}", self.addon_dir().display()));
        if rows.is_empty() {
            self.addons_box.append(&note("Nothing here — press ADD to browse Modrinth."));
        }
        for r in &rows {
            let row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
            if !r.icon_url.is_empty() && !r.project_id.is_empty() {
                let img = gtk::Image::new();
                img.set_pixel_size(40);
                load_mod_icon(&r.icon_url, &r.project_id, &img, 40);
                row.append(&img);
            } else {
                row.append(&block_fallback_image(self.state.theme.is_dark(), 40));
            }
            let mid = gtk::Box::new(gtk::Orientation::Vertical, 0);
            mid.set_hexpand(true);
            let t = gtk::Label::new(Some(&r.title));
            t.set_halign(gtk::Align::Start);
            t.set_ellipsize(gtk::pango::EllipsizeMode::Middle);
            mid.append(&t);
            let sub = gtk::Label::new(Some(&format!("{}  •  {}  •  {}",
                if r.version.is_empty() { r.file.clone() } else { r.version.clone() },
                r.platform,
                if r.enabled { "enabled" } else { "disabled" })));
            sub.set_halign(gtk::Align::Start);
            sub.set_opacity(0.55);
            sub.add_css_class("time-label");
            mid.append(&sub);
            row.append(&mid);
            let badge = gtk::Label::new(Some(if r.platform == "modrinth" { "Modrinth" } else { "Local" }));
            badge.add_css_class("proton-path-badge");
            badge.set_valign(gtk::Align::Center);
            row.append(&badge);
            let tgl = gtk::Switch::new();
            tgl.set_valign(gtk::Align::Center);
            tgl.set_tooltip_text(Some(if r.enabled { "Enabled — click to disable" } else { "Disabled — click to enable" }));
            tgl.set_active(r.enabled);
            let v = self.clone();
            let fc = r.file.clone();
            tgl.connect_state_set(move |_, _| {
                v.do_addon_toggle(&fc);
                glib::Propagation::Proceed
            });
            row.append(&tgl);
            if !r.project_id.is_empty() {
                let upd = btn_with_icon("software-update-available-symbolic", "Update");
                upd.add_css_class("settings-btn");
                upd.set_valign(gtk::Align::Center);
                let vv = self.clone();
                let fc2 = r.file.clone();
                upd.connect_clicked(move |_| vv.do_addon_update(&fc2));
                row.append(&upd);
            }
            if !r.project_id.is_empty() {
                let vb = gtk::Button::with_label("View");
                vb.add_css_class("settings-btn");
                vb.set_valign(gtk::Align::Center);
                let vv = self.clone();
                let pidv = r.project_id.clone();
                vb.connect_clicked(move |_| {
                    vv.show_project_view(&pidv, None);
                });
                row.append(&vb);
            }
            let menu = gtk::Button::new();
            set_btn_icon(&menu, "view-more-symbolic", 16);
            menu.set_valign(gtk::Align::Center);
            let vv = self.clone();
            let fc3 = r.file.clone();
            menu.connect_clicked(move |b| vv.show_addon_menu(b, &fc3));
            row.append(&menu);
            self.addons_box.append(&row);
        }
        // update-all visibility
        let ctx = self.inst_addon_ctx();
        let (tx, rx) = std::sync::mpsc::channel::<bool>();
        std::thread::spawn(move || {
            let has = ctx.map(|(mdir, mc, loader, at)| {
                MinecraftManager::mod_check_updates(&at, &mc, &loader, &mdir)
                    .ok().and_then(|d| d.get("updates").and_then(|u| u.as_array()).map(|a| !a.is_empty()))
                    .unwrap_or(false)
            }).unwrap_or(false);
            let _ = tx.send(has);
        });
        let v = self.clone();
        glib::idle_add_local(move || match rx.try_recv() {
            Ok(has) => {
                v.update_all_btn.set_visible(has);
                glib::ControlFlow::Break
            }
            Err(std::sync::mpsc::TryRecvError::Empty) => glib::ControlFlow::Continue,
            Err(_) => glib::ControlFlow::Break,
        });
    }

    fn do_addon_toggle(&self, file: &str) {
        let id = self.detail_id.borrow().clone();
        let inst = match self.get_inst(&id) {
            Some(i) => i,
            None => return,
        };
        let dir = inst_dir(&inst.id, inst.isolated);
        let sub = match self.addons_type.borrow().as_str() {
            "shaders" => "shaderpacks",
            "resourcepacks" => "resourcepacks",
            _ => "mods",
        };
        let target = dir.join(sub).join(file);
        let disabled = dir.join(sub).join(format!("{}.disabled", file));
        if target.is_file() {
            std::fs::rename(&target, &disabled).ok();
            let sc_old = dir.join(sub).join(format!("{}.corky.json", file));
            let sc_new = dir.join(sub).join(format!("{}.disabled.corky.json", file));
            if sc_old.is_file() {
                std::fs::rename(&sc_old, &sc_new).ok();
            }
        } else if disabled.is_file() {
            let back = if file.ends_with(".disabled") { file.trim_end_matches(".disabled").to_string() } else { file.to_string() };
            std::fs::rename(&disabled, dir.join(sub).join(&back)).ok();
            let sc_old = dir.join(sub).join(format!("{}.corky.json", file));
            let sc_new = dir.join(sub).join(format!("{}.corky.json", back));
            if sc_old.is_file() {
                std::fs::rename(&sc_old, &sc_new).ok();
            }
        }
        self.render_addons();
        self.render_detail_cards();
    }

    fn inst_addon_ctx(&self) -> Option<(String, String, String, String)> {
        let id = self.detail_id.borrow().clone();
        let inst = self.get_inst(&id)?;
        let dir = inst_dir(&inst.id, inst.isolated);
        let at = self.addons_type.borrow().clone();
        Some((dir.display().to_string(), inst.mc_ver, inst.loader, at))
    }

    fn do_addon_update(&self, file: &str) {
        let fc = file.to_string();
        let (mdir, _mc, _loader, at) = match self.inst_addon_ctx() {
            Some(c) => c,
            None => return,
        };
        let (tx, rx) = std::sync::mpsc::channel::<Result<String, String>>();
        std::thread::spawn(move || {
            let _ = tx.send(MinecraftManager::mod_update(&fc, &at, &mdir).map(|d| {
                d.get("file").and_then(|x| x.as_str()).unwrap_or("").to_string()
            }).map_err(|e| e.to_string()));
        });
        let v = self.clone();
        glib::idle_add_local(move || match rx.try_recv() {
            Ok(Ok(f)) => {
                v.toast("Updated", &f);
                v.render_addons();
                glib::ControlFlow::Break
            }
            Ok(Err(e)) => {
                v.toast("Update failed", &e);
                glib::ControlFlow::Break
            }
            Err(std::sync::mpsc::TryRecvError::Empty) => glib::ControlFlow::Continue,
            Err(_) => glib::ControlFlow::Break,
        });
    }

    fn show_addon_menu(&self, btn: &gtk::Button, file: &str) {
        let pop = gtk::Popover::new();
        let box_ = gtk::Box::new(gtk::Orientation::Vertical, 2);
        box_.set_margin_top(6);
        box_.set_margin_bottom(6);
        box_.set_margin_start(6);
        box_.set_margin_end(6);
        let is_dark = self.state.theme.is_dark();
        for (micon, label, action) in [("folder-symbolic", "Open folder", "folder"), ("user-trash-symbolic", "Delete", "delete")] {
            let b = if action == "folder" {
                menu_btn_png("folder", label, is_dark)
            } else if action == "delete" {
                menu_btn_png("remove", label, is_dark)
            } else {
                menu_btn(micon, label)
            };
            let v = self.clone();
            let fc = file.to_string();
            let act = action.to_string();
            let popc = pop.clone();
            b.connect_clicked(move |_| {
                popc.popdown();
                match act.as_str() {
                    "folder" => {
                        let dir = v.addon_dir();
                        v.state.integration.open_url(&format!("file://{}", dir.display()));
                    }
                    "delete" => v.do_addon_delete(&fc),
                    _ => {}
                }
            });
            box_.append(&b);
        }
        pop.set_child(Some(&box_));
        pop.set_parent(btn);
        let popc = pop.clone();
        pop.connect_closed(move |_| {
            popc.unparent();
        });
        pop.popup();
    }

    fn do_addon_delete(&self, file: &str) {
        let id = self.detail_id.borrow().clone();
        let inst = match self.get_inst(&id) {
            Some(i) => i,
            None => return,
        };
        let dir = inst_dir(&inst.id, inst.isolated);
        let sub = match self.addons_type.borrow().as_str() {
            "shaders" => "shaderpacks",
            "resourcepacks" => "resourcepacks",
            _ => "mods",
        };
        std::fs::remove_file(dir.join(sub).join(file)).ok();
        std::fs::remove_file(dir.join(sub).join(format!("{}.corky.json", file))).ok();
        self.render_addons();
        self.render_detail_cards();
    }

    fn do_update_all(&self) {
        let (mdir, mc, loader, at) = match self.inst_addon_ctx() {
            Some(c) => c,
            None => return,
        };
        let (tx, rx) = std::sync::mpsc::channel::<Result<Vec<String>, String>>();
        std::thread::spawn(move || {
            let res = MinecraftManager::mod_check_updates(&at, &mc, &loader, &mdir)
                .map(|d| d.get("updates").and_then(|u| u.as_array()).cloned().unwrap_or_default()
                    .into_iter().filter_map(|u| u.get("file").and_then(|x| x.as_str()).map(str::to_string)).collect::<Vec<_>>())
                .map_err(|e| e.to_string());
            let _ = tx.send(res);
        });
        let v = self.clone();
        glib::idle_add_local(move || match rx.try_recv() {
            Ok(Ok(files)) => {
                let (mdir2, _, _, at2) = v.inst_addon_ctx().unwrap_or_default();
                for f in &files {
                    let _ = MinecraftManager::mod_update(f, &at2, &mdir2);
                }
                v.toast("Updated", &format!("{} addon(s) updated.", files.len()));
                v.render_addons();
                glib::ControlFlow::Break
            }
            Ok(Err(e)) => {
                v.toast("Check failed", &e);
                glib::ControlFlow::Break
            }
            Err(std::sync::mpsc::TryRecvError::Empty) => glib::ControlFlow::Continue,
            Err(_) => glib::ControlFlow::Break,
        });
    }

    fn rescan_addons(&self) {
        // re-read folders + hash-match missing metadata (icons/titles)
        self.render_addons();
        let ctx = match self.inst_addon_ctx() {
            Some(c) => c,
            None => return,
        };
        let (mdir, _mc, _loader, _at) = ctx;
        self.addons_count.set_text("Matching metadata…");
        let all_types = ["mods".to_string(), "resourcepacks".to_string(), "shaders".to_string()];
        let (tx, rx) = std::sync::mpsc::channel::<bool>();
        std::thread::spawn(move || {
            for at in &all_types {
                let _ = MinecraftManager::mod_match(at, &mdir);
            }
            let _ = tx.send(true);
        });
        let v = self.clone();
        glib::idle_add_local(move || match rx.try_recv() {
            Ok(true) => {
                v.render_addons();
                v.render_detail_cards();
                v.toast("Rescanned", "Folders re-read, metadata matched.");
                glib::ControlFlow::Break
            }
            Ok(false) => {
                v.render_addons();
                v.toast("Rescan done", "Folders re-read (match skipped).");
                glib::ControlFlow::Break
            }
            Err(std::sync::mpsc::TryRecvError::Empty) => {
                v.addons_count.set_text("Matching metadata…");
                glib::ControlFlow::Continue
            }
            Err(_) => glib::ControlFlow::Break,
        });
    }

    fn render_detail_cards(&self) {
        let count = self.count_addons();
        self.ov_addons.set_text(&format!("{}", count));
    }

    // ============ logs tab ============

    fn latest_mc_log() -> Option<std::path::PathBuf> {
        let dir = std::path::PathBuf::from(std::env::var("HOME").unwrap_or_default())
            .join(".local/share/CorkyTux/logs");
        let entries = std::fs::read_dir(&dir).ok()?;
        let mut best: Option<(std::time::SystemTime, std::path::PathBuf)> = None;
        for e in entries.flatten() {
            let name = e.file_name().to_string_lossy().to_string();
            if !name.starts_with("Minecraft-") {
                continue;
            }
            let modified = e.metadata().and_then(|m| m.modified()).ok()?;
            if best.as_ref().map(|(t, _)| &modified > t).unwrap_or(true) {
                best = Some((modified, e.path()));
            }
        }
        best.map(|(_, p)| p)
    }

    fn do_refresh_logs(&self) {
        match Self::latest_mc_log() {
            Some(p) => {
                let text = std::fs::read_to_string(&p).unwrap_or_default();
                let lines: Vec<&str> = text.lines().collect();
                let tail = if lines.len() > 200 { &lines[lines.len() - 200..] } else { &lines[..] };
                self.logs_view.buffer().set_text(&format!("{}:\n{}", p.display(), tail.join("\n")));
                self.log_lbl.set_text(&format!("Showing: {}", p.display()));
            }
            None => {
                self.logs_view.buffer().set_text("No Minecraft logs yet — launch an instance first.");
                self.log_lbl.set_text("");
            }
        }
    }

    // ============ Add Instance dialog (Carbon Custom tab) ============

    fn show_add_instance(&self) {
        let dlg = adw::Dialog::new();
        dlg.set_title("Add instance");
        dlg.set_content_width(520);
        let (header, x_btn) = helpers::modal_header("Add instance");
        let content = gtk::Box::new(gtk::Orientation::Vertical, 8);
        content.add_css_class("modal-bg");
        content.append(&header);
        let body = gtk::Box::new(gtk::Orientation::Vertical, 8);
        body.set_margin_start(16);
        body.set_margin_end(16);
        body.set_margin_bottom(16);

        // Carbon-style tabs: Custom | Modpack | Import
        let add_stack = gtk::Stack::new();
        add_stack.set_vexpand(true);
        let custom_page = gtk::Box::new(gtk::Orientation::Vertical, 8);
        let pack_page = gtk::Box::new(gtk::Orientation::Vertical, 8);
        let import_page = gtk::Box::new(gtk::Orientation::Vertical, 8);
        add_stack.add_titled(&custom_page, Some("custom"), "Custom");
        add_stack.add_titled(&pack_page, Some("modpack"), "Modpack");
        add_stack.add_titled(&import_page, Some("import"), "Import");
        let add_tabbar = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        let add_ids = ["custom", "modpack", "import"];
        let add_labels = ["Custom", "Modpack", "Import"];
        let mut add_btns: Vec<gtk::ToggleButton> = Vec::new();
        let mut add_inds: Vec<gtk::Box> = Vec::new();
        for (label, id) in add_labels.iter().zip(add_ids.iter()) {
            let wrap = gtk::Box::new(gtk::Orientation::Vertical, 1);
            wrap.set_hexpand(true);
            let btn = gtk::ToggleButton::new();
            btn.add_css_class("settings-tab");
            btn.set_hexpand(true);
            let c = gtk::Box::new(gtk::Orientation::Vertical, 2);
            c.set_halign(gtk::Align::Center);
            let lbl = gtk::Label::new(Some(label));
            lbl.add_css_class("time-label");
            c.append(&lbl);
            let ind = gtk::Box::new(gtk::Orientation::Horizontal, 0);
            ind.add_css_class("settings-tab-indicator");
            ind.set_visible(*id == "custom");
            c.append(&ind);
            btn.set_child(Some(&c));
            if *id == "custom" {
                btn.set_active(true);
            }
            wrap.append(&btn);
            add_tabbar.append(&wrap);
            add_btns.push(btn);
            add_inds.push(ind);
        }
        for b in &add_btns[1..] {
            b.set_group(Some(&add_btns[0]));
        }
        for (i, id) in add_ids.iter().enumerate() {
            let st = add_stack.clone();
            let tid = id.to_string();
            let all = add_inds.clone();
            let mine = add_inds[i].clone();
            add_btns[i].connect_toggled(move |b| {
                if b.is_active() {
                    st.set_visible_child_name(&tid);
                    for ind in &all {
                        ind.set_visible(false);
                    }
                    mine.set_visible(true);
                }
            });
        }
        body.append(&add_tabbar);
        body.append(&add_stack);

        custom_page.append(&note("Name (auto: Loader + version)"));
        let name_entry = gtk::Entry::new();
        custom_page.append(&name_entry);

        custom_page.append(&note("Minecraft version"));
        let avail: Vec<String> = self.data.borrow().versions.iter().map(|(id, _)| id.clone()).collect();
        let ver_store = gtk::StringList::new(&[] as &[&str]);
        let refs: Vec<&str> = avail.iter().map(|s| s.as_str()).collect();
        ver_store.splice(0, 0, &refs);
        let ver_drop = gtk::DropDown::new(Some(ver_store), gtk::Expression::NONE);
        custom_page.append(&ver_drop);
        let add_java_req = note("");
        custom_page.append(&add_java_req);

        let filt_row = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        let snap_sw = gtk::Switch::new();
        snap_sw.set_valign(gtk::Align::Center);
        filt_row.append(&gtk::Label::new(Some("Snapshots")));
        filt_row.append(&snap_sw);
        let ver_refresh = btn_with_icon("view-refresh-symbolic", "Refresh list");
        ver_refresh.add_css_class("settings-btn");
        filt_row.append(&ver_refresh);
        custom_page.append(&filt_row);

        custom_page.append(&note("Modloader (auto-detected for this version)"));
        let loader_ids: Rc<RefCell<Vec<String>>> = Rc::new(RefCell::new(vec!["vanilla".to_string()]));
        let loader_vers: Rc<RefCell<HashMap<String, Vec<String>>>> = Rc::new(RefCell::new(HashMap::new()));
        let loader_store = gtk::StringList::new(&["Vanilla"]);
        let loader_drop = gtk::DropDown::new(Some(loader_store.clone()), gtk::Expression::NONE);
        custom_page.append(&loader_drop);

        let lv_wrap = gtk::Box::new(gtk::Orientation::Vertical, 8);
        lv_wrap.append(&note("Loader version (auto: latest)"));
        let lv_store = gtk::StringList::new(&["latest"]);
        let lv_drop = gtk::DropDown::new(Some(lv_store.clone()), gtk::Expression::NONE);
        lv_wrap.append(&lv_drop);
        lv_wrap.set_visible(false);
        custom_page.append(&lv_wrap);

        pack_page.append(&note("Modpacks — version, mods and configs install as a new isolated instance."));
        let pack_search_row = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        let pack_q = gtk::SearchEntry::new();
        pack_q.set_placeholder_text(Some("Search modpacks…"));
        pack_q.set_hexpand(true);
        pack_search_row.append(&pack_q);
        let pack_src_store = gtk::StringList::new(&["Modrinth", "CurseForge"]);
        let pack_src = gtk::DropDown::new(Some(pack_src_store), gtk::Expression::NONE);
        pack_src.set_tooltip_text(Some("Modpack source"));
        pack_src.set_selected(if self.cfg("McPackSrc") == "cf" { 1 } else { 0 });
        pack_search_row.append(&pack_src);
        let pack_go = gtk::Button::with_label("Search");
        pack_go.add_css_class("settings-btn");
        pack_search_row.append(&pack_go);
        pack_page.append(&pack_search_row);
        let pack_results = gtk::Box::new(gtk::Orientation::Vertical, 6);
        let pack_scroll = gtk::ScrolledWindow::new();
        pack_scroll.set_vexpand(true);
        pack_scroll.set_min_content_height(220);
        pack_scroll.set_child(Some(&pack_results));
        pack_page.append(&pack_scroll);
        import_page.append(&note("Import a Modrinth .mrpack (or compatible zip) from disk."));
        let import_pick = btn_with_icon("folder-symbolic", "Choose .mrpack file…");
        import_pick.add_css_class("settings-btn");
        import_page.append(&import_pick);
        let import_lbl = note("");
        import_page.append(&import_lbl);

        let status = note("");
        body.append(&status);
        let bar = gtk::ProgressBar::new();
        bar.set_visible(false);
        body.append(&bar);
        let create = gtk::Button::with_label("Create");
        create.add_css_class("add-btn");
        paint_accent(&create, &self.state.theme);
        body.append(&create);
        content.append(&body);
        dlg.set_child(Some(&content));
        {
            let d = dlg.clone();
            x_btn.connect_clicked(move |_| { d.close(); });
        }
        // auto name: refreshes while the user hasn't typed their own
        // (a stale auto value like "Vanilla X" is replaced, manual text kept)
        let last_auto: Rc<RefCell<String>> = Rc::new(RefCell::new(String::new()));
        let update_name = {
            let name_entry = name_entry.clone();
            let avail = avail.clone();
            let ver_drop = ver_drop.clone();
            let loader_ids = loader_ids.clone();
            let loader_drop = loader_drop.clone();
            let last_auto = last_auto.clone();
            move || {
                let cur = name_entry.text().to_string();
                if !cur.trim().is_empty() && cur != *last_auto.borrow() {
                    return;
                }
                let ver = avail.get(ver_drop.selected() as usize).cloned().unwrap_or_default();
                let loader = loader_ids.borrow().get(loader_drop.selected() as usize).cloned().unwrap_or("vanilla".to_string());
                let disp = if loader == "vanilla" { "Vanilla".to_string() } else {
                    let mut s = loader.clone();
                    s[..1].make_ascii_uppercase();
                    s
                };
                let auto = format!("{} {}", disp, ver);
                *last_auto.borrow_mut() = auto.clone();
                name_entry.set_text(&auto);
            }
        };
        {
            let u = update_name.clone();
            let avail2 = avail.clone();
            let req2 = add_java_req.clone();
            ver_drop.connect_selected_notify(move |d| {
                u();
                let ver = avail2.get(d.selected() as usize).cloned().unwrap_or_default();
                req2.set_text(&java_range_text(&ver));
            });
        }
        {
            let avail3 = avail.clone();
            add_java_req.set_text(&java_range_text(&avail3.get(ver_drop.selected() as usize).cloned().unwrap_or_default()));
        }
        {
            let u = update_name.clone();
            let ids = loader_ids.clone();
            let vers = loader_vers.clone();
            let lv_store_c = lv_store.clone();
            let lv_wrap_c = lv_wrap.clone();
            loader_drop.connect_selected_notify(move |d| {
                u();
                let loader = ids.borrow().get(d.selected() as usize).cloned().unwrap_or("vanilla".to_string());
                lv_wrap_c.set_visible(loader != "vanilla");
                lv_store_c.splice(0, lv_store_c.n_items(), &[] as &[&str]);
                lv_store_c.append("latest");
                if loader != "vanilla" {
                    if let Some(list) = vers.borrow().get(&loader) {
                        let refs: Vec<&str> = list.iter().map(|s| s.as_str()).collect();
                        lv_store_c.splice(1, 0, &refs);
                    }
                }
            });
        }
        update_name();
        // loader auto-detect: probe support per MC version, fill dropdowns
        {
            let ver_drop_c = ver_drop.clone();
            let avail_c = avail.clone();
            let loader_store_c = loader_store.clone();
            let loader_drop_c = loader_drop.clone();
            let loader_ids_c = loader_ids.clone();
            let loader_vers_c = loader_vers.clone();
            let lv_store_c = lv_store.clone();
            let lv_wrap_c = lv_wrap.clone();
            let status_c = status.clone();
            let uname = update_name.clone();
            let last_loader = self.cfg("McLastLoader");
            let do_probe = Rc::new(RefCell::new(Box::new(move || {}) as Box<dyn Fn()>));
            *do_probe.borrow_mut() = Box::new(move || {
                let ver = avail_c.get(ver_drop_c.selected() as usize).cloned().unwrap_or_default();
                if ver.is_empty() {
                    return;
                }
                let prev_loader = loader_ids_c.borrow().get(loader_drop_c.selected() as usize).cloned().unwrap_or("vanilla".to_string());
                let loader_store_b = loader_store_c.clone();
                let loader_drop_b = loader_drop_c.clone();
                let loader_ids_b = loader_ids_c.clone();
                let loader_vers_b = loader_vers_c.clone();
                let prev_loader_b = prev_loader.clone();
                let last_loader_b = last_loader.clone();
                let lv_store_b = lv_store_c.clone();
                let lv_wrap_b = lv_wrap_c.clone();
                let status_b = status_c.clone();
                let req_b = add_java_req.clone();
                let uname_b = uname.clone();
                status_b.set_text("Detecting modloader support…");
                let (tx, rx) = std::sync::mpsc::channel::<(Vec<(String, bool, Vec<String>)>, Option<u64>)>();
                    let ver_s = ver.clone();
                    std::thread::spawn(move || {
                    let mut out = Vec::new();
                    for l in ["fabric", "forge", "neoforge", "quilt"] {
                        match MinecraftManager::loader_versions(l, &ver_s) {
                            Ok(d) => {
                                let ok = d.get("supported").and_then(|x| x.as_bool()).unwrap_or(false);
                                let list: Vec<String> = d.get("versions").and_then(|x| x.as_array()).cloned().unwrap_or_default()
                                    .into_iter().filter_map(|x| x.as_str().map(str::to_string)).collect();
                                out.push((l.to_string(), ok, list));
                            }
                            Err(_) => out.push((l.to_string(), false, Vec::new())),
                        }
                    }
                    let java = MinecraftManager::manifest_java(&ver_s).ok()
                        .and_then(|d| d.get("required_java").and_then(|x| x.as_u64()));
                    let _ = tx.send((out, java));
                });
                glib::idle_add_local(move || match rx.try_recv() {
                    Ok((probe, java)) => {
                        let loader_ids_c = loader_ids_b.clone();
                        let loader_vers_c = loader_vers_b.clone();
                        let mut ids = vec!["vanilla".to_string()];
                        let mut names: Vec<String> = vec!["Vanilla".to_string()];
                        let mut vers = HashMap::new();
                        for (l, ok, list) in &probe {
                            if *ok {
                                let mut disp = l.clone();
                                disp[..1].make_ascii_uppercase();
                                names.push(disp);
                                ids.push(l.clone());
                                vers.insert(l.clone(), list.clone());
                            }
                        }
                        // never clobber the user's pick: keep current loader if
                        // still supported, else last-used, else vanilla
                        let keep = ids.iter().position(|x| x == &prev_loader_b)
                            .or_else(|| ids.iter().position(|x| x == &last_loader_b))
                            .unwrap_or(0);
                        let keep_vers: Vec<String> = vers.get(&ids[keep]).cloned().unwrap_or_default();
                        *loader_ids_c.borrow_mut() = ids;
                        *loader_vers_c.borrow_mut() = vers;
                        loader_store_b.splice(0, loader_store_b.n_items(), &[] as &[&str]);
                        let refs: Vec<&str> = names.iter().map(|s| s.as_str()).collect();
                        loader_store_b.splice(0, 0, &refs);
                        lv_store_b.splice(0, lv_store_b.n_items(), &[] as &[&str]);
                        lv_store_b.append("latest");
                        if keep != 0 {
                            let refs2: Vec<&str> = keep_vers.iter().map(|s| s.as_str()).collect();
                            lv_store_b.splice(1, 0, &refs2);
                            lv_wrap_b.set_visible(true);
                        } else {
                            lv_wrap_b.set_visible(false);
                        }
                        loader_drop_b.set_selected(keep as u32);
                        status_b.set_text("");
                        if let Some(j) = java {
                            req_b.set_text(&format!("Needs Java {} (auto-selected on launch).", j));
                        }
                        uname_b();
                        glib::ControlFlow::Break
                    }
                    Err(std::sync::mpsc::TryRecvError::Empty) => glib::ControlFlow::Continue,
                    Err(_) => glib::ControlFlow::Break,
                });
            });
            let dr = do_probe.clone();
            ver_drop.connect_selected_notify(move |_| dr.borrow()());
            do_probe.borrow()();
        }
        {
            // ---- Modpack tab: search + install (PineconeMC mrpack flow, GDL order)
            let v = self.clone();
            let d = dlg.clone();
            let do_pack_search = Rc::new(RefCell::new(Box::new(move || {}) as Box<dyn Fn()>));
            *do_pack_search.borrow_mut() = {
                let results = pack_results.clone();
                let q = pack_q.clone();
                let d = d.clone();
                let src = pack_src.clone();
                let vv0 = v.clone();
                Box::new(move || {
                    let d = d.clone();
                    let results_c = results.clone();
                    let q_c = q.clone();
                    let src_c = src.clone();
                    let is_cf = src_c.selected() == 1;
                    vv0.set_cfg("McPackSrc", if is_cf { "cf" } else { "mr" });
                    while let Some(c) = results_c.first_child() {
                        results_c.remove(&c);
                    }
                    results_c.append(&note(if is_cf { "Searching CurseForge…" } else { "Searching Modrinth…" }));
                    let query = q_c.text().to_string();
                    let (tx, rx) = std::sync::mpsc::channel::<Result<Vec<(String, String, String, String, String)>, String>>();
                    std::thread::spawn(move || {
                        let _ = tx.send(if is_cf {
                            MinecraftManager::cf_search(&query, "modpack", "", "", 20).map(|doc| {
                                doc.get("hits").and_then(|x| x.as_array()).cloned().unwrap_or_default().into_iter().map(|h| (
                                    h.get("mod_id").and_then(|x| x.as_str()).unwrap_or("").to_string(),
                                    h.get("title").and_then(|x| x.as_str()).unwrap_or("").to_string(),
                                    h.get("author").and_then(|x| x.as_str()).unwrap_or("").to_string(),
                                    h.get("description").and_then(|x| x.as_str()).unwrap_or("").to_string(),
                                    h.get("icon_url").and_then(|x| x.as_str()).unwrap_or("").to_string(),
                                )).collect::<Vec<_>>()
                            }).map_err(|e| e.to_string())
                        } else {
                            MinecraftManager::mod_search(&query, "modpack", "", "", 20).map(|doc| {
                                doc.get("hits").and_then(|x| x.as_array()).cloned().unwrap_or_default().into_iter().map(|h| (
                                    h.get("project_id").and_then(|x| x.as_str()).unwrap_or("").to_string(),
                                    h.get("title").and_then(|x| x.as_str()).unwrap_or("").to_string(),
                                    h.get("author").and_then(|x| x.as_str()).unwrap_or("").to_string(),
                                    h.get("description").and_then(|x| x.as_str()).unwrap_or("").to_string(),
                                    h.get("icon_url").and_then(|x| x.as_str()).unwrap_or("").to_string(),
                                )).collect::<Vec<_>>()
                            }).map_err(|e| e.to_string())
                        });
                    });
                    let vv = v.clone();
                    glib::idle_add_local(move || match rx.try_recv() {
                        Ok(Ok(hits)) => {
                            while let Some(c) = results_c.first_child() {
                                results_c.remove(&c);
                            }
                            if hits.is_empty() {
                                results_c.append(&note("No modpacks found."));
                            }
                            for (pid, title, author, desc, icon) in hits {
                                let row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
                                if !icon.is_empty() {
                                    let img = gtk::Image::new();
                                    img.set_pixel_size(40);
                                    load_mod_icon(&icon, &pid, &img, 40);
                                    row.append(&img);
                                } else {
                                    row.append(&block_fallback_image(vv.state.theme.is_dark(), 40));
                                }
                                let mid = gtk::Box::new(gtk::Orientation::Vertical, 0);
                                mid.set_hexpand(true);
                                let tt = gtk::Label::new(Some(&format!("{}  ·  {}", title, author)));
                                tt.set_halign(gtk::Align::Start);
                                tt.set_ellipsize(gtk::pango::EllipsizeMode::Middle);
                                mid.append(&tt);
                                let dd2 = gtk::Label::new(Some(&clean_md(&desc).lines().next().unwrap_or("").to_string()));
                                dd2.set_halign(gtk::Align::Start);
                                dd2.set_ellipsize(gtk::pango::EllipsizeMode::End);
                                dd2.set_opacity(0.6);
                                dd2.add_css_class("time-label");
                                mid.append(&dd2);
                                row.append(&mid);
                                let ib = themed_btn("download", "Install", vv.state.theme.is_dark(), 14);
                                ib.add_css_class("add-btn");
                                ib.set_valign(gtk::Align::Center);
                                paint_accent(&ib, &vv.state.theme);
                                let vv2 = vv.clone();
                                let pidc = pid.clone();
                                let iconc = icon.clone();
                                let dd = d.clone();
                                ib.connect_clicked(move |b| {
                                    b.set_sensitive(false);
                                    b.set_label("Installing…");
                                    if is_cf {
                                        vv2.install_cf_modpack(&pidc, "", &iconc, &dd);
                                    } else {
                                        vv2.install_modpack(&pidc, "", &dd);
                                    }
                                });
                                row.append(&ib);
                                let vwb = gtk::Button::with_label("View");
                                vwb.add_css_class("settings-btn");
                                vwb.set_valign(gtk::Align::Center);
                                let vvw = vv.clone();
                                let pidw = pid.clone();
                                let iconw = icon.clone();
                                let ddw = d.clone();
                                vwb.connect_clicked(move |_| {
                                    let pidw2 = pidw.clone();
                                    let iconw2 = iconw.clone();
                                    let vvw2 = vvw.clone();
                                    let ddw2 = ddw.clone();
                                    if is_cf {
                                        let cb: Box<dyn Fn()> = Box::new(move || {
                                            vvw2.install_cf_modpack(&pidw2, "", &iconw2, &ddw2);
                                        });
                                        vvw.show_cf_project_view(&pidw, Some(("Install".to_string(), cb)));
                                    } else {
                                        let cb: Box<dyn Fn()> = Box::new(move || {
                                            vvw2.install_modpack(&pidw2, "", &ddw2);
                                        });
                                        vvw.show_project_view(&pidw, Some(("Install".to_string(), cb)));
                                    }
                                });
                                row.append(&vwb);
                                results_c.append(&row);
                            }
                            glib::ControlFlow::Break
                        }
                        Ok(Err(e)) => {
                            while let Some(c) = results_c.first_child() {
                                results_c.remove(&c);
                            }
                            results_c.append(&note(&format!("Search failed: {}", e)));
                            glib::ControlFlow::Break
                        }
                        Err(std::sync::mpsc::TryRecvError::Empty) => glib::ControlFlow::Continue,
                        Err(_) => glib::ControlFlow::Break,
                    });
                })
            };
            {
                let ds = do_pack_search.clone();
                pack_go.connect_clicked(move |_| ds.borrow()());
            }
            {
                let ds = do_pack_search.clone();
                pack_q.connect_activate(move |_| ds.borrow()());
            }
            {
                let ds = do_pack_search.clone();
                pack_src.connect_selected_notify(move |_| ds.borrow()());
            }
            {
                let pending: Rc<RefCell<Option<glib::SourceId>>> = Rc::new(RefCell::new(None));
                let ds = do_pack_search.clone();
                let pend = pending.clone();
                pack_q.connect_search_changed(move |_| {
                    if let Some(id) = pend.borrow_mut().take() {
                        id.remove();
                    }
                    let ds2 = ds.clone();
                    let pend2 = pend.clone();
                    *pend.borrow_mut() = Some(glib::timeout_add_local_once(
                        std::time::Duration::from_millis(600),
                        move || {
                            *pend2.borrow_mut() = None;
                            ds2.borrow()();
                        },
                    ));
                });
            }
            do_pack_search.borrow()();
        }
        {
            // ---- Import tab
            let v = self.clone();
            let d = dlg.clone();
            import_pick.connect_clicked(move |_| {
                let fd = gtk::FileDialog::new();
                fd.set_title("Choose modpack file");
                let filt = gtk::FileFilter::new();
                filt.set_name(Some("Modrinth pack (*.mrpack, *.zip)"));
                filt.add_pattern("*.mrpack");
                filt.add_pattern("*.zip");
                let store = gio::ListStore::new::<gtk::FileFilter>();
                store.append(&filt);
                fd.set_filters(Some(&store));
                let vv = v.clone();
                let dd = d.clone();
                let par = vv.parent.clone();
                fd.open(Some(&par), gio::Cancellable::NONE, move |res| {
                    let file = match res {
                        Ok(f) => f,
                        Err(_) => return,
                    };
                    let path = file.path().map(|x| x.display().to_string()).unwrap_or_default();
                    if path.is_empty() {
                        return;
                    }
                    vv.import_modpack(&path, &dd);
                });
            });
        }
        {
            let v = self.clone();
            let d = dlg.clone();
            ver_refresh.connect_clicked(move |_| {
                let v = v.clone();
                let d = d.clone();
                let all = snap_sw.is_active();
                let (tx, rx) = std::sync::mpsc::channel::<Result<Vec<String>, String>>();
                std::thread::spawn(move || {
                    let _ = tx.send(MinecraftManager::versions(all).map(|doc| {
                        doc.get("versions").and_then(|x| x.as_array()).cloned().unwrap_or_default()
                            .into_iter().filter_map(|x| x.get("id").and_then(|i| i.as_str()).map(str::to_string)).collect::<Vec<_>>()
                    }).map_err(|e| e.to_string()));
                });
                glib::idle_add_local(move || match rx.try_recv() {
                    Ok(Ok(list)) => {
                        v.data.borrow_mut().versions = list.into_iter().map(|id| (id, String::new())).collect();
                        d.close();
                        v.show_add_instance();
                        glib::ControlFlow::Break
                    }
                    Ok(Err(e)) => {
                        v.toast("Versions failed", &e);
                        glib::ControlFlow::Break
                    }
                    Err(std::sync::mpsc::TryRecvError::Empty) => glib::ControlFlow::Continue,
                    Err(_) => glib::ControlFlow::Break,
                });
            });
        }
        {
            let v = self.clone();
            let d = dlg.clone();
            let create_h = create.clone();
            create.connect_clicked(move |_| {
                let v = v.clone();
                let d = d.clone();
                let create_h = create_h.clone();
                let bar = bar.clone();
                let status = status.clone();
                let avail: Vec<String> = v.data.borrow().versions.iter().map(|(id, _)| id.clone()).collect();
                let mc = avail.get(ver_drop.selected() as usize).cloned().unwrap_or_default();
                if mc.is_empty() {
                    status.set_text("Pick a Minecraft version first.");
                    return;
                }
                let loader = loader_ids.borrow().get(loader_drop.selected() as usize).cloned().unwrap_or("vanilla".to_string());
                v.set_cfg("McLastLoader", &loader);
                let lv_idx = lv_drop.selected();
                let lv_text = lv_drop.selected_item().and_then(|o| o.downcast::<gtk::StringObject>().ok()).map(|o| o.string().to_string()).unwrap_or_default();
                let lv_pick = if lv_idx == 0 || lv_text == "latest" { String::new() } else { lv_text };
                let name = name_entry.text().to_string().trim().to_string();
                bar.set_visible(true);
                bar.pulse();
                status.set_text(&format!("Installing {}…", mc));
                create_h.set_sensitive(false);
                // resolve loader version + target dir off-thread, then install
                let (tx2, rx2) = std::sync::mpsc::channel::<Result<(String, String, String), String>>();
                let mc2 = mc.clone();
                let loader2 = loader.clone();
                std::thread::spawn(move || {
                    let mut lv = lv_pick;
                    if loader2 != "vanilla" {
                        match MinecraftManager::loader_versions(&loader2, &mc2) {
                            Ok(doc) => {
                                if doc.get("supported").and_then(|x| x.as_bool()).unwrap_or(false) != true {
                                    let _ = tx2.send(Err(format!("{} has no builds for Minecraft {} — pick another loader.", loader2, mc2)));
                                    return;
                                }
                                if lv.is_empty() {
                                    lv = doc.get("latest").and_then(|x| x.as_str()).unwrap_or("").to_string();
                                    if lv.is_empty() {
                                        let _ = tx2.send(Err(format!("No {} builds for {}", loader2, mc2)));
                                        return;
                                    }
                                }
                            }
                            Err(e) => {
                                let _ = tx2.send(Err(e));
                                return;
                            }
                        }
                    }
                    let vid = if loader2 == "vanilla" { mc2.clone() } else { format!("{}-{}-{}", mc2, loader2, lv) };
                    let dir = instances_root().join(safe_id(&vid)).display().to_string();
                    let _ = tx2.send(Ok((lv, vid, dir)));
                });
                let vv = v.clone();
                let dd = d.clone();
                let mc3 = mc.clone();
                let loader3 = loader.clone();
                let create_h2 = create_h.clone();
                let status_c = status.clone();
                let bar_c = bar.clone();
                let create_c = create_h.clone();
                glib::idle_add_local(move || match rx2.try_recv() {
                    Ok(Ok((lv, vid_pred, dir))) => {
                        let loader3c = loader3.clone();
                        let dir_c = dir.clone();
                        let bar = bar.clone();
                        let status = status.clone();
                        let create = create_h2.clone();
                        let rx = MinecraftManager::spawn_install_isolated(mc3.clone(), loader3c.clone(), lv, dir);
                        let vv2 = vv.clone();
                        let dd2 = dd.clone();
                        let name2 = name.clone();
                        let vid_pred2 = vid_pred.clone();
                        crate::backend::plugin_process::pump_to_idle(rx, move |ev| {
                            match ev {
                                crate::backend::plugin_process::PluginEvent::Progress { percent, .. } => {
                                    if let Some(p) = percent {
                                        bar.set_fraction((p / 100.0).clamp(0.0, 1.0));
                                    } else {
                                        bar.pulse();
                                    }
                                    true
                                }
                                crate::backend::plugin_process::PluginEvent::Done(val) => {
                                    let vid = val.get("version").and_then(|x| x.as_str()).unwrap_or(&vid_pred2).to_string();
                                    let got_loader = parse_inst_id(&vid).1;
                                    if loader3c != "vanilla" && got_loader == "vanilla" {
                                        vv2.toast("Installed as Vanilla",
                                            &format!("{} did not produce a modded install (got {}). The loader installer may have failed — check the version list.", loader3c, vid));
                                    }
                                    if vid != vid_pred2 {
                                        let old = instances_root().join(safe_id(&vid_pred2));
                                        let new = instances_root().join(safe_id(&vid));
                                        if old.exists() && !new.exists() {
                                            std::fs::rename(&old, &new).ok();
                                        }
                                    }
                                    if !name2.is_empty() {
                                        vv2.set_inst_cfg(&vid, "Name", &name2);
                                    }
                                    vv2.toast("Installed", &vid);
                                    dd2.close();
                                    vv2.reload_silent();
                                    false
                                }
                                crate::backend::plugin_process::PluginEvent::Error { message, .. } => {
                                    status.set_text(&format!("Failed: {}", message));
                                    bar.set_visible(false);
                                    create.set_sensitive(true);
                                    // no ghost instances: drop the fresh dir unless the
                                    // expected version id landed in it (a lone base
                                    // version would show up as a phantom vanilla tile)
                                    let vd = std::path::PathBuf::from(&dir_c).join("versions").join(&vid_pred2);
                                    if !vd.exists() {
                                        std::fs::remove_dir_all(&dir_c).ok();
                                    }
                                    false
                                }
                                _ => true,
                            }
                        });
                        glib::ControlFlow::Break
                    }
                    Ok(Err(e)) => {
                        status_c.set_text(&format!("Failed: {}", e));
                        bar_c.set_visible(false);
                        create_c.set_sensitive(true);
                        glib::ControlFlow::Break
                    }
                    Err(std::sync::mpsc::TryRecvError::Empty) => glib::ControlFlow::Continue,
                    Err(_) => glib::ControlFlow::Break,
                });
            });
        }
        dlg.present(Some(&self.parent));
    }

    fn show_curse_key_dialog(&self) {
        let dlg = adw::Dialog::new();
        dlg.set_title("CurseForge API key");
        dlg.set_content_width(420);
        let (header, x_btn) = helpers::modal_header("CurseForge API key");
        let content = gtk::Box::new(gtk::Orientation::Vertical, 0);
        content.add_css_class("modal-bg");
        content.append(&header);
        let body = gtk::Box::new(gtk::Orientation::Vertical, 8);
        body.set_margin_top(12);
        body.set_margin_bottom(16);
        body.set_margin_start(16);
        body.set_margin_end(16);
        body.append(&note("Paste your key (console.curseforge.com → API key). Stored with private permissions, never shown again."));
        let entry = gtk::Entry::new();
        entry.set_placeholder_text(Some("$2a$10$…"));
        entry.set_visibility(false);
        body.append(&entry);
        let status = note("");
        status.set_visible(false);
        body.append(&status);
        let save = gtk::Button::with_label("Save key");
        save.add_css_class("add-btn");
        body.append(&save);
        content.append(&body);
        dlg.set_child(Some(&content));
        {
            let d = dlg.clone();
            x_btn.connect_clicked(move |_| { d.close(); });
        }
        {
            let d = dlg.clone();
            let e = entry.clone();
            let s = status.clone();
            save.connect_clicked(move |_| {
                let key = e.text().to_string().trim().to_string();
                if key.len() < 20 {
                    s.set_visible(true);
                    s.set_text("That key looks too short — paste the full key.");
                    return;
                }
                e.set_text("");
                s.set_visible(true);
                s.set_text("Saving…");
                let (tx, rx) = std::sync::mpsc::channel::<Result<String, String>>();
                std::thread::spawn(move || {
                    let saved = MinecraftManager::curse_save_key(&key).map(|_| ()).map_err(|e| e.to_string());
                    let msg = match saved {
                        Err(e) => Err(e),
                        Ok(_) => match MinecraftManager::curse_test() {
                            Ok(doc) => Ok(format!("Saved. API OK ({}). Reopen Browse → CurseForge.",
                                doc.get("game").and_then(|x| x.as_str()).unwrap_or("reachable"))),
                            Err(e) => Err(format!("Saved, but API test failed: {}", e.lines().next().unwrap_or("").to_string())),
                        },
                    };
                    let _ = tx.send(msg);
                });
                let sc = s.clone();
                glib::idle_add_local(move || match rx.try_recv() {
                    Ok(Ok(msg)) => {
                        sc.set_text(&msg);
                        glib::ControlFlow::Break
                    }
                    Ok(Err(err)) => {
                        sc.set_text(&err);
                        glib::ControlFlow::Break
                    }
                    Err(std::sync::mpsc::TryRecvError::Empty) => glib::ControlFlow::Continue,
                    Err(_) => glib::ControlFlow::Break,
                });
            });
        }
        dlg.present(Some(&self.parent));
    }

    fn show_cf_versions(&self, mod_id: &str, title: &str, mc: &str, loader: &str, kind: &str) {
        let dlg = adw::Dialog::new();
        dlg.set_title(&format!("{} — versions (CurseForge)", title));
        dlg.set_content_width(560);
        dlg.set_content_height(480);
        let (header, x_btn) = helpers::modal_header(&format!("{} — versions (CurseForge)", title));
        let content = gtk::Box::new(gtk::Orientation::Vertical, 8);
        content.add_css_class("modal-bg");
        content.append(&header);
        let body = gtk::Box::new(gtk::Orientation::Vertical, 6);
        body.set_margin_start(16);
        body.set_margin_end(16);
        body.set_margin_bottom(16);
        body.append(&note(&format!("For Minecraft {} • {}", mc, if loader == "vanilla" { "Vanilla".to_string() } else { loader.to_string() })));
        let list = gtk::Box::new(gtk::Orientation::Vertical, 6);
        let scroll = gtk::ScrolledWindow::new();
        scroll.set_vexpand(true);
        scroll.set_min_content_height(300);
        scroll.set_child(Some(&list));
        body.append(&scroll);
        content.append(&body);
        dlg.set_child(Some(&content));
        {
            let d = dlg.clone();
            x_btn.connect_clicked(move |_| { d.close(); });
        }
        list.append(&note("Loading versions…"));
        let (tx, rx) = std::sync::mpsc::channel::<Result<Vec<(String, String, String, String)>, String>>();
        let mid = mod_id.to_string();
        let mc_s = mc.to_string();
        let loader_s = loader.to_string();
        let kind_s = kind.to_string();
        std::thread::spawn(move || {
            let _ = tx.send(MinecraftManager::cf_versions(&mid, &mc_s, &loader_s, &kind_s).map(|d| {
                d.get("versions").and_then(|x| x.as_array()).cloned().unwrap_or_default().into_iter().map(|ver| (
                    ver.get("version_id").and_then(|x| x.as_str()).unwrap_or("").to_string(),
                    ver.get("version_number").and_then(|x| x.as_str()).unwrap_or("").to_string(),
                    ver.get("date").and_then(|x| x.as_str()).unwrap_or("").to_string(),
                    ver.get("game_versions").and_then(|x| x.as_array()).map(|a| a.iter().filter_map(|g| g.as_str()).collect::<Vec<_>>().join(", ")).unwrap_or_default(),
                )).collect::<Vec<_>>()
            }).map_err(|e| e.to_string()));
        });
        let v = self.clone();
        let mid2 = mod_id.to_string();
        let mc2 = mc.to_string();
        let loader2 = loader.to_string();
        let kind2 = kind.to_string();
        let title2 = title.to_string();
        glib::idle_add_local(move || match rx.try_recv() {
            Ok(Ok(vers)) => {
                while let Some(c) = list.first_child() {
                    list.remove(&c);
                }
                if vers.is_empty() {
                    list.append(&note("No files for this Minecraft/loader on CurseForge."));
                }
                for (fid, fname, date, games) in vers {
                    let row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
                    let mid_box = gtk::Box::new(gtk::Orientation::Vertical, 0);
                    mid_box.set_hexpand(true);
                    let t = gtk::Label::new(Some(&fname));
                    t.set_halign(gtk::Align::Start);
                    t.set_ellipsize(gtk::pango::EllipsizeMode::Middle);
                    mid_box.append(&t);
                    let sub = gtk::Label::new(Some(&format!("{}  •  {}", date, games)));
                    sub.set_halign(gtk::Align::Start);
                    sub.set_opacity(0.6);
                    sub.add_css_class("time-label");
                    mid_box.append(&sub);
                    row.append(&mid_box);
                    let ib = themed_btn("download", "Install", v.state.theme.is_dark(), 14);
                    ib.add_css_class("add-btn");
                    paint_accent(&ib, &v.state.theme);
                    let vv = v.clone();
                    let midc = mid2.clone();
                    let mc3 = mc2.clone();
                    let ld3 = loader2.clone();
                    let kind3 = kind2.clone();
                    let at = v.addons_type.borrow().clone();
                    let ttl = title2.clone();
                    ib.connect_clicked(move |_| {
                        vv.install_cf_with_progress(&midc, &fid, &mc3, &ld3, &kind3, &at, &ttl);
                    });
                    row.append(&ib);
                    list.append(&row);
                }
                glib::ControlFlow::Break
            }
            Ok(Err(e)) => {
                while let Some(c) = list.first_child() {
                    list.remove(&c);
                }
                list.append(&note(&format!("Failed: {}", e)));
                glib::ControlFlow::Break
            }
            Err(std::sync::mpsc::TryRecvError::Empty) => glib::ControlFlow::Continue,
            Err(_) => glib::ControlFlow::Break,
        });
        dlg.present(Some(&self.parent));
    }

    fn show_cf_project_view(&self, mod_id: &str, install: Option<(String, Box<dyn Fn()>)>) {
        let dlg = adw::Dialog::new();
        dlg.set_title("Project (CurseForge)");
        dlg.set_content_width(560);
        dlg.set_content_height(600);
        let (header, x_btn) = helpers::modal_header("Project (CurseForge)");
        let content = gtk::Box::new(gtk::Orientation::Vertical, 8);
        content.add_css_class("modal-bg");
        content.append(&header);
        let body = gtk::Box::new(gtk::Orientation::Vertical, 8);
        body.set_margin_start(16);
        body.set_margin_end(16);
        body.set_margin_bottom(16);
        body.append(&note("Loading project info…"));
        content.append(&body);
        dlg.set_child(Some(&content));
        {
            let d = dlg.clone();
            x_btn.connect_clicked(move |_| { d.close(); });
        }
        let (tx, rx) = std::sync::mpsc::channel::<Result<serde_json::Value, String>>();
        let mid = mod_id.to_string();
        std::thread::spawn(move || {
            let _ = tx.send(MinecraftManager::cf_project(&mid).map_err(|e| e.to_string()));
        });
        let v = self.clone();
        let install_slot: Rc<RefCell<Option<(String, Box<dyn Fn()>)>>> = Rc::new(RefCell::new(install));
        glib::idle_add_local(move || match rx.try_recv() {
            Ok(Ok(doc)) => {
                while let Some(c) = body.first_child() {
                    body.remove(&c);
                }
                let pr = doc.get("project").cloned().unwrap_or_default();
                let title = pr.get("title").and_then(|x| x.as_str()).unwrap_or("?");
                let desc = pr.get("description").and_then(|x| x.as_str()).unwrap_or("");
                let icon = pr.get("icon_url").and_then(|x| x.as_str()).unwrap_or("");
                let mid2 = pr.get("mod_id").and_then(|x| x.as_str()).unwrap_or("").to_string();
                let head = gtk::Box::new(gtk::Orientation::Horizontal, 10);
                if !icon.is_empty() {
                    let img = gtk::Image::new();
                    img.set_pixel_size(64);
                    load_mod_icon(icon, &format!("cf-{}", mid2), &img, 64);
                    head.append(&img);
                } else {
                    head.append(&block_fallback_image(v.state.theme.is_dark(), 64));
                }
                let tbox = gtk::Box::new(gtk::Orientation::Vertical, 2);
                tbox.set_hexpand(true);
                let tt = gtk::Label::new(Some(title));
                tt.set_halign(gtk::Align::Start);
                tt.add_css_class("details-title");
                tbox.append(&tt);
                let stats = gtk::Label::new(Some(&format!("{} downloads",
                    pr.get("downloads").and_then(|x| x.as_u64()).unwrap_or(0))));
                stats.set_halign(gtk::Align::Start);
                stats.set_opacity(0.6);
                stats.add_css_class("time-label");
                tbox.append(&stats);
                head.append(&tbox);
                body.append(&head);
                let gallery: Vec<String> = pr.get("gallery").and_then(|x| x.as_array()).cloned().unwrap_or_default()
                    .into_iter().filter_map(|g| g.as_str().map(str::to_string)).collect();
                if let Some(first) = gallery.first() {
                    let gimg = gtk::Image::new();
                    gimg.set_pixel_size(320);
                    gimg.set_halign(gtk::Align::Center);
                    load_mod_icon(first, &format!("cf-{}-gallery", mid2), &gimg, 320);
                    body.append(&gimg);
                }
                if !desc.is_empty() {
                    let dl = gtk::Label::new(Some(desc));
                    dl.set_halign(gtk::Align::Start);
                    dl.set_wrap(true);
                    body.append(&dl);
                }
                if let Some((label, cb)) = install_slot.borrow_mut().take() {
                    let ib = themed_btn("download", &label, v.state.theme.is_dark(), 16);
                    ib.add_css_class("add-btn");
                    paint_accent(&ib, &v.state.theme);
                    ib.connect_clicked(move |_| cb());
                    body.append(&ib);
                }
                glib::ControlFlow::Break
            }
            Ok(Err(e)) => {
                while let Some(c) = body.first_child() {
                    body.remove(&c);
                }
                body.append(&note(&format!("Failed: {}", e)));
                glib::ControlFlow::Break
            }
            Err(std::sync::mpsc::TryRecvError::Empty) => glib::ControlFlow::Continue,
            Err(_) => glib::ControlFlow::Break,
        });
        dlg.present(Some(&self.parent));
    }

    fn show_mod_versions(&self, project_id: &str, title: &str, mc: &str, loader: &str) {
        let ptype = MinecraftManager::addon_project_type(&self.addons_type.borrow()).to_string();
        let dlg = adw::Dialog::new();
        dlg.set_title(&format!("{} — versions", title));
        dlg.set_content_width(560);
        dlg.set_content_height(480);
        let (header, x_btn) = helpers::modal_header(&format!("{} — versions", title));
        let content = gtk::Box::new(gtk::Orientation::Vertical, 8);
        content.add_css_class("modal-bg");
        content.append(&header);
        let body = gtk::Box::new(gtk::Orientation::Vertical, 6);
        body.set_margin_start(16);
        body.set_margin_end(16);
        body.set_margin_bottom(16);
        body.append(&note(&format!("For Minecraft {} • {}", mc, if loader == "vanilla" { "Vanilla".to_string() } else { loader.to_string() })));
        let list = gtk::Box::new(gtk::Orientation::Vertical, 6);
        let scroll = gtk::ScrolledWindow::new();
        scroll.set_vexpand(true);
        scroll.set_min_content_height(300);
        scroll.set_child(Some(&list));
        body.append(&scroll);
        content.append(&body);
        dlg.set_child(Some(&content));
        {
            let d = dlg.clone();
            x_btn.connect_clicked(move |_| { d.close(); });
        }
        list.append(&note("Loading versions…"));
        let (tx, rx) = std::sync::mpsc::channel::<Result<Vec<(String, String, String, String)>, String>>();
        let pid = project_id.to_string();
        let mc_s = mc.to_string();
        let loader_s = loader.to_string();
        std::thread::spawn(move || {
            let _ = tx.send(MinecraftManager::mod_versions(&pid, &mc_s, &loader_s, &ptype).map(|d| {
                d.get("versions").and_then(|x| x.as_array()).cloned().unwrap_or_default().into_iter().map(|ver| (
                    ver.get("version_id").and_then(|x| x.as_str()).unwrap_or("").to_string(),
                    ver.get("version_number").and_then(|x| x.as_str()).unwrap_or("").to_string(),
                    ver.get("date").and_then(|x| x.as_str()).unwrap_or("").to_string(),
                    ver.get("game_versions").and_then(|x| x.as_array()).map(|a| a.iter().filter_map(|g| g.as_str()).collect::<Vec<_>>().join(", ")).unwrap_or_default(),
                )).collect::<Vec<_>>()
            }).map_err(|e| e.to_string()));
        });
        let v = self.clone();
        let pid2 = project_id.to_string();
        let mc2 = mc.to_string();
        let loader2 = loader.to_string();
        glib::idle_add_local(move || match rx.try_recv() {
            Ok(Ok(vers)) => {
                while let Some(c) = list.first_child() {
                    list.remove(&c);
                }
                if vers.is_empty() {
                    list.append(&note("No versions for this Minecraft/loader."));
                }
                for (vid, vnum, date, games) in vers {
                    let row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
                    let mid = gtk::Box::new(gtk::Orientation::Vertical, 0);
                    mid.set_hexpand(true);
                    let t = gtk::Label::new(Some(&vnum));
                    t.set_halign(gtk::Align::Start);
                    mid.append(&t);
                    let sub = gtk::Label::new(Some(&format!("{}  •  {}", &date[..10.min(date.len())], games)));
                    sub.set_halign(gtk::Align::Start);
                    sub.set_opacity(0.6);
                    sub.add_css_class("time-label");
                    mid.append(&sub);
                    row.append(&mid);
                    let ib = themed_btn("download", "Install", v.state.theme.is_dark(), 14);
                    ib.add_css_class("add-btn");
                    paint_accent(&ib, &v.state.theme);
                    let vv = v.clone();
                    let pidc = pid2.clone();
                    let mc3 = mc2.clone();
                    let ld3 = loader2.clone();
                    let at = v.addons_type.borrow().clone();
                    ib.connect_clicked(move |_| {
                        vv.install_addon_with_progress(&pidc, &vid, &mc3, &ld3, &at, &vnum);
                    });
                    row.append(&ib);
                    list.append(&row);
                }
                glib::ControlFlow::Break
            }
            Ok(Err(e)) => {
                while let Some(c) = list.first_child() {
                    list.remove(&c);
                }
                list.append(&note(&format!("Failed: {}", e)));
                glib::ControlFlow::Break
            }
            Err(std::sync::mpsc::TryRecvError::Empty) => glib::ControlFlow::Continue,
            Err(_) => glib::ControlFlow::Break,
        });
        dlg.present(Some(&self.parent));
    }

    fn staging_dir() -> std::path::PathBuf {
        let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
        instances_root().join(format!(".staging-{}-{}", std::process::id(), now))
    }

    fn match_then_finish(&self, staging: &std::path::Path, launch_version: &str, title: &str, project_id: &str, icon_url: &str, dlg: Option<&adw::Dialog>) {
        // identify pack-provided jars so addon rows show real icons/titles
        let staging_owned = staging.to_path_buf();
        let dir = staging_owned.display().to_string();
        let vid = launch_version.to_string();
        let ttl = title.to_string();
        let pid = project_id.to_string();
        let ico = icon_url.to_string();
        self.lib_status.set_text("Matching addon metadata…");
        let rx = MinecraftManager::spawn_mod_match("mods".to_string(), dir.clone());
        let v = self.clone();
        let dd: Option<adw::Dialog> = dlg.map(|d| d.clone());
        crate::backend::plugin_process::pump_to_idle(rx, move |ev| {
            match ev {
                crate::backend::plugin_process::PluginEvent::Done(_) => {
                    let rx2 = MinecraftManager::spawn_mod_match("resourcepacks".to_string(), dir.clone());
                    let (v2, dd2, vid2, ttl2, dir2, pid2, ico2) = (v.clone(), dd.clone(), vid.clone(), ttl.clone(), dir.clone(), pid.clone(), ico.clone());
                    crate::backend::plugin_process::pump_to_idle(rx2, move |ev2| {
                        match ev2 {
                            crate::backend::plugin_process::PluginEvent::Done(_) => {
                                let (vid3, ttl3, dir3, pid3, ico3) = (vid2.clone(), ttl2.clone(), dir2.clone(), pid2.clone(), ico2.clone());
                                let rx3 = MinecraftManager::spawn_mod_match("shaders".to_string(), dir3.clone());
                                let v3 = v2.clone();
                                let dd3 = dd2.clone();
                                crate::backend::plugin_process::pump_to_idle(rx3, move |ev3| {
                                    match ev3 {
                                        crate::backend::plugin_process::PluginEvent::Done(_) => {
                                            v3.finish_staging(std::path::Path::new(&dir3), &vid3, &ttl3, &pid3, &ico3, dd3.as_ref());
                                            false
                                        }
                                        _ => true,
                                    }
                                });
                                false
                            }
                            _ => true,
                        }
                    });
                    false
                }
                crate::backend::plugin_process::PluginEvent::Error { message, .. } => {
                    v.lib_status.set_text(&format!("Match failed (continuing): {}", message));
                    v.finish_staging(&staging_owned, &vid, &ttl, &pid, &ico, dd.as_ref());
                    false
                }
                _ => true,
            }
        });
    }

    fn finish_staging(&self, staging: &std::path::Path, launch_version: &str, title: &str, project_id: &str, icon_url: &str, dlg: Option<&adw::Dialog>) {
        let target = instances_root().join(safe_id(launch_version));
        if target.exists() {
            std::fs::remove_dir_all(&target).ok();
        }
        if std::fs::rename(staging, &target).is_err() {
            self.toast("Install failed", "Could not move instance into place.");
            std::fs::remove_dir_all(staging).ok();
            return;
        }
        if !title.is_empty() {
            self.set_inst_cfg(launch_version, "Name", title);
        }
        // pack icon: Modrinth project icon for installs, direct URL (CurseForge
        // search hit) or embedded file for imports
        if !project_id.is_empty() || !icon_url.is_empty() {
            let vid = launch_version.to_string();
            let pid = project_id.to_string();
            let direct = icon_url.to_string();
            let (tx, rx) = std::sync::mpsc::channel::<Option<(String, String)>>();
            std::thread::spawn(move || {
                let icon_url = if direct.is_empty() {
                    MinecraftManager::mod_project(&pid).ok()
                        .and_then(|d| d.get("project").and_then(|p| p.get("icon_url")).and_then(|x| x.as_str()).map(str::to_string))
                        .unwrap_or_default()
                } else {
                    direct
                };
                let got = download_pack_icon(&icon_url, &vid).map(|p| (vid.clone(), p));
                let _ = tx.send(got);
            });
            let v = self.clone();
            glib::idle_add_local(move || match rx.try_recv() {
                Ok(Some((vidc, pathc))) => {
                    v.set_inst_cfg(&vidc, "Icon", &format!("custom:{}", pathc));
                    v.reload_silent();
                    glib::ControlFlow::Break
                }
                Ok(None) => glib::ControlFlow::Break,
                Err(std::sync::mpsc::TryRecvError::Empty) => glib::ControlFlow::Continue,
                Err(_) => glib::ControlFlow::Break,
            });
        } else if let Some(path) = find_local_pack_icon(&target) {
            self.set_inst_cfg(launch_version, "Icon", &format!("custom:{}", path));
        }
        self.toast("Installed", launch_version);
        if let Some(d) = dlg {
            d.close();
        }
        self.reload_silent();
    }

    fn install_modpack(&self, project_id: &str, version_id: &str, dlg: &adw::Dialog) {
        let staging = Self::staging_dir();
        std::fs::create_dir_all(&staging).ok();
        self.lib_status.set_text("Installing modpack…");
        let (bar, status) = self.show_progress("Installing modpack");
        let pid0 = project_id.to_string();
        let rx = MinecraftManager::spawn_modpack_install(
            project_id.to_string(), version_id.to_string(), staging.display().to_string());
        let v = self.clone();
        let dd = dlg.clone();
        crate::backend::plugin_process::pump_to_idle(rx, move |ev| {
            match ev {
                crate::backend::plugin_process::PluginEvent::Progress { stage, percent, .. } => {
                    Self::drive_progress(&bar, &status, stage, percent, "Installing modpack");
                    v.lib_status.set_text(&status.text().to_string());
                    true
                }
                crate::backend::plugin_process::PluginEvent::Done(val) => {
                    let vid = val.get("version").and_then(|x| x.as_str()).unwrap_or("").to_string();
                    let title = val.get("title").and_then(|x| x.as_str()).unwrap_or("").to_string();
                    let pid = val.get("project_id").and_then(|x| x.as_str()).unwrap_or("").to_string();
                    let pid = if pid.is_empty() { pid0.clone() } else { pid };
                    if vid.is_empty() {
                        status.set_text("Install failed: no launch version reported.");
                        v.toast("Install failed", "No launch version reported.");
                        std::fs::remove_dir_all(&staging).ok();
                    } else {
                        bar.set_fraction(1.0);
                        status.set_text("Matching addon metadata…");
                        v.match_then_finish(&staging, &vid, &title, &pid, "", Some(&dd));
                    }
                    false
                }
                crate::backend::plugin_process::PluginEvent::Error { message, .. } => {
                    bar.set_fraction(0.0);
                    bar.set_text(Some("Failed"));
                    status.set_text(&message);
                    v.lib_status.set_text(&format!("Modpack failed: {}", message));
                    v.toast("Modpack failed", &message);
                    std::fs::remove_dir_all(&staging).ok();
                    false
                }
                _ => true,
            }
        });
    }

    fn install_cf_modpack(&self, pack_id: &str, file_id: &str, icon_url: &str, dlg: &adw::Dialog) {
        let staging = Self::staging_dir();
        std::fs::create_dir_all(&staging).ok();
        self.lib_status.set_text("Installing modpack (CurseForge)…");
        let (bar, status) = self.show_progress("Installing modpack (CurseForge)");
        let rx = MinecraftManager::spawn_cf_modpack_install(
            pack_id.to_string(), file_id.to_string(), staging.display().to_string());
        let v = self.clone();
        let dd = dlg.clone();
        let ico = icon_url.to_string();
        crate::backend::plugin_process::pump_to_idle(rx, move |ev| {
            match ev {
                crate::backend::plugin_process::PluginEvent::Progress { stage, percent, .. } => {
                    Self::drive_progress(&bar, &status, stage, percent, "Installing modpack");
                    v.lib_status.set_text(&status.text().to_string());
                    true
                }
                crate::backend::plugin_process::PluginEvent::Done(val) => {
                    let vid = val.get("version").and_then(|x| x.as_str()).unwrap_or("").to_string();
                    let title = val.get("title").and_then(|x| x.as_str()).unwrap_or("").to_string();
                    if vid.is_empty() {
                        status.set_text("Install failed: no launch version reported.");
                        v.toast("Install failed", "No launch version reported.");
                        std::fs::remove_dir_all(&staging).ok();
                    } else {
                        bar.set_fraction(1.0);
                        status.set_text("Matching addon metadata…");
                        v.match_then_finish(&staging, &vid, &title, "", &ico, Some(&dd));
                    }
                    false
                }
                crate::backend::plugin_process::PluginEvent::Error { message, .. } => {
                    bar.set_fraction(0.0);
                    bar.set_text(Some("Failed"));
                    status.set_text(&message);
                    v.lib_status.set_text(&format!("Modpack failed: {}", message));
                    v.toast("Modpack failed", &message);
                    std::fs::remove_dir_all(&staging).ok();
                    false
                }
                _ => true,
            }
        });
    }

    fn import_modpack(&self, path: &str, dlg: &adw::Dialog) {
        let staging = Self::staging_dir();
        std::fs::create_dir_all(&staging).ok();
        self.lib_status.set_text("Importing modpack…");
        let (bar, status) = self.show_progress("Importing modpack");
        let rx = MinecraftManager::spawn_modpack_import(path.to_string(), staging.display().to_string());
        let v = self.clone();
        let dd = dlg.clone();
        crate::backend::plugin_process::pump_to_idle(rx, move |ev| {
            match ev {
                crate::backend::plugin_process::PluginEvent::Progress { stage, percent, .. } => {
                    Self::drive_progress(&bar, &status, stage, percent, "Importing modpack");
                    v.lib_status.set_text(&status.text().to_string());
                    true
                }
                crate::backend::plugin_process::PluginEvent::Done(val) => {
                    let vid = val.get("version").and_then(|x| x.as_str()).unwrap_or("").to_string();
                    let title = val.get("title").and_then(|x| x.as_str()).unwrap_or("").to_string();
                    if vid.is_empty() {
                        status.set_text("Import failed: no launch version reported.");
                        v.toast("Import failed", "No launch version reported.");
                        std::fs::remove_dir_all(&staging).ok();
                    } else {
                        bar.set_fraction(1.0);
                        status.set_text("Matching addon metadata…");
                        v.match_then_finish(&staging, &vid, &title, "", "", Some(&dd));
                    }
                    false
                }
                crate::backend::plugin_process::PluginEvent::Error { message, .. } => {
                    bar.set_fraction(0.0);
                    bar.set_text(Some("Failed"));
                    status.set_text(&message);
                    v.lib_status.set_text(&format!("Import failed: {}", message));
                    v.toast("Import failed", &message);
                    std::fs::remove_dir_all(&staging).ok();
                    false
                }
                _ => true,
            }
        });
    }

    fn show_progress(&self, title: &str) -> (gtk::ProgressBar, gtk::Label) {
        let dlg = adw::Dialog::new();
        dlg.set_title(title);
        dlg.set_content_width(400);
        let (header, x_btn) = helpers::modal_header(title);
        let content = gtk::Box::new(gtk::Orientation::Vertical, 0);
        content.add_css_class("modal-bg");
        content.append(&header);
        let body = gtk::Box::new(gtk::Orientation::Vertical, 8);
        body.set_margin_top(12);
        body.set_margin_bottom(16);
        body.set_margin_start(16);
        body.set_margin_end(16);
        let status = note("Starting…");
        body.append(&status);
        let bar = gtk::ProgressBar::new();
        bar.set_show_text(true);
        bar.set_text(Some("…"));
        body.append(&bar);
        let hide = gtk::Button::with_label("Hide");
        hide.add_css_class("settings-btn");
        body.append(&hide);
        content.append(&body);
        dlg.set_child(Some(&content));
        {
            let d = dlg.clone();
            x_btn.connect_clicked(move |_| { d.close(); });
        }
        {
            let d = dlg.clone();
            hide.connect_clicked(move |_| { d.close(); });
        }
        dlg.present(Some(&self.parent));
        (bar, status)
    }

    fn drive_progress(bar: &gtk::ProgressBar, status: &gtk::Label, stage: Option<String>, percent: Option<f64>, prefix: &str) {
        let st = stage.unwrap_or_default();
        match percent {
            Some(pc) => {
                let frac = (pc / 100.0).clamp(0.0, 1.0);
                bar.set_fraction(frac);
                bar.set_text(Some(&format!("{}%", pc as u32)));
                status.set_text(&format!("{}{}", prefix, if st.is_empty() { String::new() } else { format!(" — {}", st) }));
            }
            None => {
                bar.pulse();
                bar.set_text(Some(&st));
                status.set_text(&format!("{}{}", prefix, st));
            }
        }
    }

    fn install_addon_with_progress(&self, pid: &str, version_id: &str, mc: &str, ld: &str, at: &str, title: &str) {
        if ld == "vanilla" && at != "resourcepacks" {
            self.toast("No modloader", "This instance is Vanilla — create a Fabric/Forge/NeoForge/Quilt instance to use mods or shaders. Resource Packs work on Vanilla.");
            return;
        }
        let (bar, status) = self.show_progress(&format!("Installing {}", title));
        let mdir = self.inst_base_dir().display().to_string();
        let rx = MinecraftManager::spawn_mod_install(
            pid.to_string(), version_id.to_string(), mc.to_string(), ld.to_string(), at.to_string(), mdir);
        let v = self.clone();
        crate::backend::plugin_process::pump_to_idle(rx, move |ev| {
            match ev {
                crate::backend::plugin_process::PluginEvent::Progress { stage, percent, .. } => {
                    Self::drive_progress(&bar, &status, stage, percent, "");
                    v.lib_status.set_text(&status.text().to_string());
                    true
                }
                crate::backend::plugin_process::PluginEvent::Done(val) => {
                    let f = val.get("file").and_then(|x| x.as_str()).unwrap_or("");
                    bar.set_fraction(1.0);
                    bar.set_text(Some("Done"));
                    status.set_text(&format!("Installed {}", f));
                    v.lib_status.set_text(&format!("Installed {}", f));
                    v.toast("Installed", f);
                    v.render_addons();
                    v.render_detail_cards();
                    false
                }
                crate::backend::plugin_process::PluginEvent::Error { message, .. } => {
                    bar.set_fraction(0.0);
                    bar.set_text(Some("Failed"));
                    status.set_text(&message);
                    v.lib_status.set_text(&format!("Install failed: {}", message));
                    v.toast("Install failed", &message);
                    false
                }
                _ => true,
            }
        });
    }

    fn install_cf_with_progress(&self, mod_id: &str, file_id: &str, mc: &str, ld: &str, kind: &str, at: &str, title: &str) {
        if ld == "vanilla" && at != "resourcepacks" {
            self.toast("No modloader", "This instance is Vanilla — create a Fabric/Forge/NeoForge/Quilt instance to use mods or shaders. Resource Packs work on Vanilla.");
            return;
        }
        let (bar, status) = self.show_progress(&format!("Installing {} (CurseForge)", title));
        let mdir = self.inst_base_dir().display().to_string();
        let rx = MinecraftManager::spawn_cf_install(
            mod_id.to_string(), file_id.to_string(), mc.to_string(), ld.to_string(), kind.to_string(), at.to_string(), mdir);
        let v = self.clone();
        crate::backend::plugin_process::pump_to_idle(rx, move |ev| {
            match ev {
                crate::backend::plugin_process::PluginEvent::Progress { stage, percent, .. } => {
                    Self::drive_progress(&bar, &status, stage, percent, "");
                    v.lib_status.set_text(&status.text().to_string());
                    true
                }
                crate::backend::plugin_process::PluginEvent::Done(val) => {
                    let f = val.get("file").and_then(|x| x.as_str()).unwrap_or("");
                    bar.set_fraction(1.0);
                    bar.set_text(Some("Done"));
                    status.set_text(&format!("Installed {}", f));
                    v.lib_status.set_text(&format!("Installed {}", f));
                    v.toast("Installed", f);
                    v.render_addons();
                    v.render_detail_cards();
                    false
                }
                crate::backend::plugin_process::PluginEvent::Error { message, .. } => {
                    bar.set_fraction(0.0);
                    bar.set_text(Some("Failed"));
                    status.set_text(&message);
                    v.lib_status.set_text(&format!("Install failed: {}", message));
                    v.toast("Install failed", &message);
                    false
                }
                _ => true,
            }
        });
    }

    fn install_addon_queue(&self, items: Vec<(String, String)>, mc: &str, ld: &str, at: &str, source: &str, kind: &str) {
        if ld == "vanilla" && at != "resourcepacks" {
            self.toast("No modloader", "This instance is Vanilla — create a Fabric/Forge/NeoForge/Quilt instance to use mods or shaders. Resource Packs work on Vanilla.");
            return;
        }
        if items.is_empty() {
            return;
        }
        let total = items.len();
        let (bar, status) = self.show_progress(&format!("Installing {} item(s)", total));
        let v = self.clone();
        let mc = mc.to_string();
        let ld = ld.to_string();
        let at = at.to_string();
        let source = source.to_string();
        let kind = kind.to_string();
        let queue: Rc<RefCell<Vec<(String, String)>>> = Rc::new(RefCell::new(items));
        let failed: Rc<RefCell<Vec<String>>> = Rc::new(RefCell::new(Vec::new()));
        let step: Rc<RefCell<Box<dyn Fn(usize)>>> = Rc::new(RefCell::new(Box::new(|_| {}) as Box<dyn Fn(usize)>));
        *step.borrow_mut() = {
            let queue = queue.clone();
            let failed = failed.clone();
            let step = step.clone();
            let bar = bar.clone();
            let status = status.clone();
            let source = source.clone();
            let kind = kind.clone();
            Box::new(move |i: usize| {
                let (pid, title) = match queue.borrow().get(i).cloned() {
                    Some(x) => x,
                    None => return,
                };
                status.set_text(&format!("{}/{} — {}", i + 1, total, title));
                let mdir = v.inst_base_dir().display().to_string();
                let rx = if source == "curseforge" {
                    MinecraftManager::spawn_cf_install(
                        pid, String::new(), mc.clone(), ld.clone(), kind.clone(), at.clone(), mdir)
                } else {
                    MinecraftManager::spawn_mod_install(
                        pid, String::new(), mc.clone(), ld.clone(), at.clone(), mdir)
                };
                let (vv, barc, statusc, queuec, failedc, stepc) =
                    (v.clone(), bar.clone(), status.clone(), queue.clone(), failed.clone(), step.clone());
                crate::backend::plugin_process::pump_to_idle(rx, move |ev| {
                    match ev {
                        crate::backend::plugin_process::PluginEvent::Progress { percent, .. } => {
                            let frac = ((i as f64) + percent.unwrap_or(0.0) / 100.0) / (total as f64);
                            barc.set_fraction(frac.clamp(0.0, 1.0));
                            barc.set_text(Some(&format!("{}/{}", i + 1, total)));
                            true
                        }
                        crate::backend::plugin_process::PluginEvent::Done(val) => {
                            let f = val.get("file").and_then(|x| x.as_str()).unwrap_or("").to_string();
                            vv.lib_status.set_text(&format!("Installed {} ({}/{})", f, i + 1, total));
                            if i + 1 >= total {
                                barc.set_fraction(1.0);
                                barc.set_text(Some("Done"));
                                let fails = failedc.borrow().clone();
                                if fails.is_empty() {
                                    statusc.set_text(&format!("Installed {} item(s).", total));
                                    vv.toast("Installed", &format!("{} item(s) installed.", total));
                                } else {
                                    statusc.set_text(&format!("Done with {} failure(s): {}", fails.len(), fails.join(", ")));
                                    vv.toast("Installed with errors", &fails.join("\n"));
                                }
                                vv.render_addons();
                                vv.render_detail_cards();
                            } else {
                                stepc.borrow()(i + 1);
                            }
                            let _ = queuec;
                            false
                        }
                        crate::backend::plugin_process::PluginEvent::Error { message, .. } => {
                            let title = queuec.borrow().get(i).map(|(_, t)| t.clone()).unwrap_or_default();
                            failedc.borrow_mut().push(format!("{}: {}", title, message.lines().next().unwrap_or("").to_string()));
                            if i + 1 >= total {
                                barc.set_fraction(1.0);
                                barc.set_text(Some("Done"));
                                let fails = failedc.borrow().clone();
                                statusc.set_text(&format!("Done with {} failure(s): {}", fails.len(), fails.join(", ")));
                                vv.toast("Installed with errors", &fails.join("\n"));
                                vv.render_addons();
                                vv.render_detail_cards();
                            } else {
                                stepc.borrow()(i + 1);
                            }
                            false
                        }
                        _ => true,
                    }
                });
            })
        };
        step.borrow()(0);
    }

    fn do_browse_install_simple(&self, pid: &str, mc: &str, ld: &str, at: &str) {
        self.install_addon_with_progress(pid, "", mc, ld, at, pid);
    }

    fn show_project_view(&self, project_id: &str, install: Option<(String, Box<dyn Fn()>)>) {
        let dlg = adw::Dialog::new();
        dlg.set_title("Project");
        dlg.set_content_width(560);
        dlg.set_content_height(600);
        let (header, x_btn) = helpers::modal_header("Project");
        let content = gtk::Box::new(gtk::Orientation::Vertical, 8);
        content.add_css_class("modal-bg");
        content.append(&header);
        let body = gtk::Box::new(gtk::Orientation::Vertical, 8);
        body.set_margin_start(16);
        body.set_margin_end(16);
        body.set_margin_bottom(16);
        body.append(&note("Loading project info…"));
        content.append(&body);
        dlg.set_child(Some(&content));
        {
            let d = dlg.clone();
            x_btn.connect_clicked(move |_| { d.close(); });
        }
        let (tx, rx) = std::sync::mpsc::channel::<Result<serde_json::Value, String>>();
        let pid = project_id.to_string();
        std::thread::spawn(move || {
            let _ = tx.send(MinecraftManager::mod_project(&pid).map_err(|e| e.to_string()));
        });
        let v = self.clone();
        let install_slot: Rc<RefCell<Option<(String, Box<dyn Fn()>)>>> = Rc::new(RefCell::new(install));
        glib::idle_add_local(move || match rx.try_recv() {
            Ok(Ok(doc)) => {
                while let Some(c) = body.first_child() {
                    body.remove(&c);
                }
                let pr = doc.get("project").cloned().unwrap_or_default();
                let title = pr.get("title").and_then(|x| x.as_str()).unwrap_or("?");
                let desc = pr.get("description").and_then(|x| x.as_str()).unwrap_or("");
                let body_txt = pr.get("body").and_then(|x| x.as_str()).unwrap_or("");
                let icon = pr.get("icon_url").and_then(|x| x.as_str()).unwrap_or("");
                let pid2 = pr.get("project_id").and_then(|x| x.as_str()).unwrap_or("").to_string();
                let head = gtk::Box::new(gtk::Orientation::Horizontal, 10);
                if !icon.is_empty() {
                    let img = gtk::Image::new();
                    img.set_pixel_size(64);
                    load_mod_icon(icon, &pid2, &img, 64);
                    head.append(&img);
                } else {
                    head.append(&block_fallback_image(v.state.theme.is_dark(), 64));
                }
                let tbox = gtk::Box::new(gtk::Orientation::Vertical, 2);
                tbox.set_hexpand(true);
                let tt = gtk::Label::new(Some(title));
                tt.set_halign(gtk::Align::Start);
                tt.add_css_class("details-title");
                tbox.append(&tt);
                let stats = gtk::Label::new(Some(&format!("{} downloads • {} followers",
                    pr.get("downloads").and_then(|x| x.as_u64()).unwrap_or(0),
                    pr.get("followers").and_then(|x| x.as_u64()).unwrap_or(0))));
                stats.set_halign(gtk::Align::Start);
                stats.set_opacity(0.6);
                stats.add_css_class("time-label");
                tbox.append(&stats);
                head.append(&tbox);
                body.append(&head);
                // gallery preview
                let gallery: Vec<String> = pr.get("gallery").and_then(|x| x.as_array()).cloned().unwrap_or_default()
                    .into_iter().filter_map(|g| g.as_str().map(str::to_string)).collect();
                if let Some(first) = gallery.first() {
                    let gimg = gtk::Image::new();
                    gimg.set_pixel_size(320);
                    gimg.set_halign(gtk::Align::Center);
                    load_mod_icon(first, &format!("{}-gallery", pid2), &gimg, 320);
                    body.append(&gimg);
                }
                // loader + version chips
                let chips = gtk::Box::new(gtk::Orientation::Horizontal, 4);
                for c in pr.get("loaders").and_then(|x| x.as_array()).cloned().unwrap_or_default()
                    .into_iter().filter_map(|x| x.as_str().map(str::to_string)) {
                    let b = gtk::Label::new(Some(&c));
                    b.add_css_class("proton-path-badge");
                    chips.append(&b);
                }
                for g in pr.get("game_versions").and_then(|x| x.as_array()).cloned().unwrap_or_default()
                    .into_iter().filter_map(|x| x.as_str().map(str::to_string)) {
                    let b = gtk::Label::new(Some(&g));
                    b.add_css_class("proton-path-badge");
                    chips.append(&b);
                }
                body.append(&chips);
                if !desc.is_empty() {
                    let dl = gtk::Label::new(Some(desc));
                    dl.set_halign(gtk::Align::Start);
                    dl.set_wrap(true);
                    body.append(&dl);
                }
                if !body_txt.is_empty() {
                    let scroll = gtk::ScrolledWindow::new();
                    scroll.set_vexpand(true);
                    scroll.set_min_content_height(160);
                    let tv = gtk::TextView::new();
                    tv.set_editable(false);
                    tv.set_cursor_visible(false);
                    tv.set_wrap_mode(gtk::WrapMode::Word);
                    let buf = tv.buffer();
                    let accent = v.state.theme.accent_color();
                    for seg in md_segments(&body_txt) {
                        match seg {
                            MdSeg::Text(x) => {
                                buf.insert(&mut buf.end_iter(), &x);
                            }
                            MdSeg::Link(label, url) => {
                                let tag_name = format!("link::{}", url);
                                let tag = match buf.tag_table().lookup(&tag_name) {
                                    Some(tag) => tag,
                                    None => {
                                        let tag = gtk::TextTag::new(Some(&tag_name));
                                        tag.set_underline(pango::Underline::Single);
                                        if let Ok(rgba) = accent.parse() {
                                            tag.set_foreground_rgba(Some(&rgba));
                                        }
                                        buf.tag_table().add(&tag);
                                        tag
                                    }
                                };
                                buf.insert_with_tags(&mut buf.end_iter(), &label, &[&tag]);
                            }
                        }
                    }
                    // click a link -> browser; hover -> pointer cursor
                    let click = gtk::GestureClick::new();
                    let tv_c = tv.clone();
                    let open = v.state.clone();
                    click.connect_pressed(move |g, _, x, y| {
                        if g.current_button() != 1 {
                            return;
                        }
                        let (ix, iy) = tv_c.window_to_buffer_coords(gtk::TextWindowType::Text, x as i32, y as i32);
                        let Some(iter) = tv_c.iter_at_location(ix, iy) else {
                            return;
                        };
                        for tag in iter.tags() {
                            if let Some(name) = tag.name() {
                                if let Some(url) = name.strip_prefix("link::") {
                                    open.integration.open_url(url);
                                    break;
                                }
                            }
                        }
                    });
                    tv.add_controller(click);
                    let motion = gtk::EventControllerMotion::new();
                    let tv_m = tv.clone();
                    motion.connect_motion(move |_, x, y| {
                        let (ix, iy) = tv_m.window_to_buffer_coords(gtk::TextWindowType::Text, x as i32, y as i32);
                        let over = tv_m.iter_at_location(ix, iy).map(|iter| {
                            iter.tags().iter().any(|tag| {
                                tag.name().map(|n| n.starts_with("link::")).unwrap_or(false)
                            })
                        }).unwrap_or(false);
                        tv_m.set_cursor_from_name(if over { Some("pointer") } else { None });
                    });
                    tv.add_controller(motion);
                    scroll.set_child(Some(&tv));
                    body.append(&scroll);
                }
                if let Some((label, cb)) = install_slot.borrow_mut().take() {
                    let ib = themed_btn("download", &label, v.state.theme.is_dark(), 16);
                    ib.add_css_class("add-btn");
                    paint_accent(&ib, &v.state.theme);
                    ib.connect_clicked(move |_| cb());
                    body.append(&ib);
                }
                glib::ControlFlow::Break
            }
            Ok(Err(e)) => {
                while let Some(c) = body.first_child() {
                    body.remove(&c);
                }
                body.append(&note(&format!("Failed: {}", e)));
                glib::ControlFlow::Break
            }
            Err(std::sync::mpsc::TryRecvError::Empty) => glib::ControlFlow::Continue,
            Err(_) => glib::ControlFlow::Break,
        });
        dlg.present(Some(&self.parent));
    }

    // ============ Browse Modrinth dialog (Carbon Search page) ============

    fn show_browse(&self) {
        let id = self.detail_id.borrow().clone();
        let inst = match self.get_inst(&id) {
            Some(i) => i,
            None => return,
        };
        let dlg = adw::Dialog::new();
        dlg.set_title("Browse Modrinth");
        dlg.set_content_width(640);
        dlg.set_content_height(560);
        let (header, x_btn) = helpers::modal_header("Browse addons");
        let content = gtk::Box::new(gtk::Orientation::Vertical, 8);
        content.add_css_class("modal-bg");
        content.append(&header);
        let body = gtk::Box::new(gtk::Orientation::Vertical, 8);
        body.set_margin_start(16);
        body.set_margin_end(16);
        body.set_margin_bottom(16);
        let search_row = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        let q_entry = gtk::SearchEntry::new();
        q_entry.set_placeholder_text(Some("Search mods, shaders, resource packs…"));
        q_entry.set_hexpand(true);
        search_row.append(&q_entry);
        let type_store = gtk::StringList::new(&["Mods", "Resource Packs", "Shaders"]);
        let type_drop = gtk::DropDown::new(Some(type_store), gtk::Expression::NONE);
        let cur = self.addons_type.borrow().clone();
        type_drop.set_selected(match cur.as_str() {
            "resourcepacks" => 1,
            "shaders" => 2,
            _ => 0,
        });
        search_row.append(&type_drop);
        let src_store = gtk::StringList::new(&["Modrinth", "CurseForge"]);
        let src_drop = gtk::DropDown::new(Some(src_store), gtk::Expression::NONE);
        src_drop.set_tooltip_text(Some("Addon source"));
        src_drop.set_selected(if self.cfg("McBrowseSrc") == "cf" { 1 } else { 0 });
        search_row.append(&src_drop);
        let go = gtk::Button::with_label("Search");
        go.add_css_class("settings-btn");
        search_row.append(&go);
        body.append(&search_row);
        let inst_loader_lbl = note(&format!("Instance: {} • {}",
            inst.mc_ver,
            if inst.loader == "vanilla" { "Vanilla".to_string() } else { inst.loader.clone() }));
        body.append(&inst_loader_lbl);
        let vanilla_warn = note("Vanilla instances can't load mods or shaders — create a Fabric/Forge/NeoForge/Quilt instance, or browse Resource Packs.");
        vanilla_warn.set_visible(inst.loader == "vanilla");
        body.append(&vanilla_warn);
        let cf_row = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        let cf_warn = note("CurseForge needs an API key to search here.");
        cf_warn.set_hexpand(true);
        cf_warn.set_halign(gtk::Align::Start);
        cf_row.append(&cf_warn);
        let cf_key_btn = gtk::Button::with_label("Set API key…");
        cf_key_btn.add_css_class("settings-btn");
        cf_row.append(&cf_key_btn);
        cf_row.set_visible(false);
        body.append(&cf_row);
        let cf_ok: Rc<RefCell<bool>> = Rc::new(RefCell::new(false));
        {
            let row = cf_row.clone();
            let flag = cf_ok.clone();
            let src = src_drop.clone();
            let (tx, rx) = std::sync::mpsc::channel::<bool>();
            std::thread::spawn(move || {
                let _ = tx.send(MinecraftManager::curse_configured());
            });
            glib::idle_add_local(move || match rx.try_recv() {
                Ok(ok) => {
                    *flag.borrow_mut() = ok;
                    row.set_visible(src.selected() == 1 && !ok);
                    glib::ControlFlow::Break
                }
                Err(std::sync::mpsc::TryRecvError::Empty) => glib::ControlFlow::Continue,
                Err(_) => glib::ControlFlow::Break,
            });
        }
        {
            let v = self.clone();
            cf_key_btn.connect_clicked(move |_| v.show_curse_key_dialog());
        }
        let sel_row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        let sel_all = gtk::CheckButton::new();
        sel_all.set_tooltip_text(Some("Select all visible"));
        sel_row.append(&sel_all);
        let sel_count = gtk::Label::new(Some("0 selected"));
        sel_count.set_halign(gtk::Align::Start);
        sel_count.set_hexpand(true);
        sel_count.add_css_class("time-label");
        sel_row.append(&sel_count);
        let sel_btn = gtk::Button::with_label("Install selected");
        sel_btn.add_css_class("add-btn");
        sel_btn.set_sensitive(false);
        sel_btn.set_valign(gtk::Align::Center);
        sel_row.append(&sel_btn);
        body.append(&sel_row);
        let sel_checks: Rc<RefCell<Vec<(String, String, gtk::CheckButton)>>> = Rc::new(RefCell::new(Vec::new()));
        let refresh_sel: Rc<RefCell<Box<dyn Fn()>>> = Rc::new(RefCell::new(Box::new(|| {}) as Box<dyn Fn()>));
        *refresh_sel.borrow_mut() = {
            let checks = sel_checks.clone();
            let lbl = sel_count.clone();
            let btn = sel_btn.clone();
            let all = sel_all.clone();
            Box::new(move || {
                let n = checks.borrow().iter().filter(|(_, _, c)| c.is_active()).count();
                lbl.set_text(&format!("{} selected", n));
                btn.set_sensitive(n > 0);
                btn.set_label(&format!("Install selected ({})", n));
                let total = checks.borrow().len();
                all.set_active(n > 0 && n == total);
            })
        };
        {
            let checks = sel_checks.clone();
            let refresh = refresh_sel.clone();
            sel_all.connect_toggled(move |b| {
                let on = b.is_active();
                for (_, _, c) in checks.borrow().iter() {
                    c.set_active(on);
                }
                refresh.borrow()();
            });
        }
        {
            let vv = self.clone();
            let checks = sel_checks.clone();
            let mcq = inst.mc_ver.clone();
            let ldq = inst.loader.clone();
            let typed = type_drop.clone();
            let srcd = src_drop.clone();
            sel_btn.connect_clicked(move |_| {
                let items: Vec<(String, String)> = checks.borrow().iter()
                    .filter(|(_, _, c)| c.is_active())
                    .map(|(p, t, _)| (p.clone(), t.clone()))
                    .collect();
                if items.is_empty() {
                    return;
                }
                let at = match typed.selected() {
                    1 => "resourcepacks",
                    2 => "shaders",
                    _ => "mods",
                }.to_string();
                let kind = match typed.selected() {
                    1 => "resourcepack",
                    2 => "shader",
                    _ => "mod",
                }.to_string();
                if srcd.selected() == 1 {
                    vv.install_addon_queue(items, &mcq, &ldq, &at, "curseforge", &kind);
                } else {
                    vv.install_addon_queue(items, &mcq, &ldq, &at, "modrinth", "");
                }
            });
        }
        let results = gtk::Box::new(gtk::Orientation::Vertical, 6);
        let res_scroll = gtk::ScrolledWindow::new();
        res_scroll.set_vexpand(true);
        res_scroll.set_min_content_height(320);
        res_scroll.set_child(Some(&results));
        body.append(&res_scroll);
        content.append(&body);
        dlg.set_child(Some(&content));
        {
            let d = dlg.clone();
            x_btn.connect_clicked(move |_| { d.close(); });
        }
        let v = self.clone();
        let mc = inst.mc_ver.clone();
        let loader = inst.loader.clone();
        let do_search = Rc::new(RefCell::new(Box::new(move || {}) as Box<dyn Fn()>));
        *do_search.borrow_mut() = {
            let results = results.clone();
            let q_entry = q_entry.clone();
            let type_drop = type_drop.clone();
            let src_drop = src_drop.clone();
            let mc = mc.clone();
            let loader = loader.clone();
            let vv = v.clone();
            let checks0 = sel_checks.clone();
            let refresh0 = refresh_sel.clone();
            let cf_row0 = cf_row.clone();
            let cf_ok0 = cf_ok.clone();
            Box::new(move || {
                let results_c = results.clone();
                let q_entry_c = q_entry.clone();
                let type_drop_c = type_drop.clone();
                let src_drop_c = src_drop.clone();
                let warn_c = vanilla_warn.clone();
                let mc_c = mc.clone();
                let vv_c = vv.clone();
                let loader_c = loader.clone();
                let checks_c = checks0.clone();
                let refresh_c = refresh0.clone();
                checks_c.borrow_mut().clear();
                refresh_c.borrow()();
                vv_c.set_cfg("McBrowseSrc", if src_drop_c.selected() == 1 { "cf" } else { "mr" });
                cf_row0.set_visible(src_drop_c.selected() == 1 && !*cf_ok0.borrow());
                let is_cf = src_drop_c.selected() == 1;
                let is_mod_kind = type_drop_c.selected() != 1;
                warn_c.set_visible(loader_c == "vanilla" && is_mod_kind);
                while let Some(c) = results_c.first_child() {
                    results_c.remove(&c);
                }
                results_c.append(&note(if is_cf { "Searching CurseForge…" } else { "Searching Modrinth…" }));
                let q = q_entry_c.text().to_string();
                let ptype = match type_drop_c.selected() {
                    1 => "resourcepack",
                    2 => "shader",
                    _ => "mod",
                }.to_string();
                let kind_rows = ptype.clone();
                // PineconeMC-style: no loader facet — every build for this MC
                // version is listed, badges show each project's loaders.
                let (tx, rx) = std::sync::mpsc::channel::<Result<Vec<(String, String, String, String, String, Vec<String>)>, String>>();
                let mc_s = mc_c.clone();
                let loader_s = loader_c.clone();
                std::thread::spawn(move || {
                    let _ = tx.send(if is_cf {
                        MinecraftManager::cf_search(&q, &ptype, &mc_s, &loader_s, 20).map(|d| {
                            d.get("hits").and_then(|x| x.as_array()).cloned().unwrap_or_default().into_iter().map(|h| (
                                h.get("mod_id").and_then(|x| x.as_str()).unwrap_or("").to_string(),
                                h.get("title").and_then(|x| x.as_str()).unwrap_or("").to_string(),
                                h.get("author").and_then(|x| x.as_str()).unwrap_or("").to_string(),
                                h.get("description").and_then(|x| x.as_str()).unwrap_or("").to_string(),
                                h.get("icon_url").and_then(|x| x.as_str()).unwrap_or("").to_string(),
                                h.get("categories").and_then(|x| x.as_array()).map(|a| a.iter().filter_map(|c| c.as_str()).map(str::to_string).collect()).unwrap_or_default(),
                            )).collect::<Vec<_>>()
                        }).map_err(|e| e.to_string())
                    } else {
                        MinecraftManager::mod_search(&q, &ptype, &mc_s, "", 20).map(|d| {
                            d.get("hits").and_then(|x| x.as_array()).cloned().unwrap_or_default().into_iter().map(|h| (
                                h.get("project_id").and_then(|x| x.as_str()).unwrap_or("").to_string(),
                                h.get("title").and_then(|x| x.as_str()).unwrap_or("").to_string(),
                                h.get("author").and_then(|x| x.as_str()).unwrap_or("").to_string(),
                                h.get("description").and_then(|x| x.as_str()).unwrap_or("").to_string(),
                                h.get("icon_url").and_then(|x| x.as_str()).unwrap_or("").to_string(),
                                h.get("categories").and_then(|x| x.as_array()).map(|a| a.iter().filter_map(|c| c.as_str()).map(str::to_string).collect()).unwrap_or_default(),
                            )).collect::<Vec<_>>()
                        }).map_err(|e| e.to_string())
                    });
                });
                glib::idle_add_local(move || match rx.try_recv() {
                    Ok(Ok(hits)) => {
                        while let Some(c) = results_c.first_child() {
                            results_c.remove(&c);
                        }
                        if hits.is_empty() {
                            results_c.append(&note("No results."));
                        }
                        for (pid, title, author, desc, icon, cats) in hits {
                            let row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
                            let check = gtk::CheckButton::new();
                            check.set_valign(gtk::Align::Center);
                            check.set_tooltip_text(Some("Select for batch install"));
                            row.append(&check);
                            checks_c.borrow_mut().push((pid.clone(), title.clone(), check.clone()));
                            {
                                let r = refresh_c.clone();
                                check.connect_toggled(move |_| r.borrow()());
                            }
                            if !icon.is_empty() {
                                let img = gtk::Image::new();
                                img.set_pixel_size(40);
                                load_mod_icon(&icon, &pid, &img, 40);
                                row.append(&img);
                            } else {
                                row.append(&block_fallback_image(vv_c.state.theme.is_dark(), 40));
                            }
                            let mid = gtk::Box::new(gtk::Orientation::Vertical, 0);
                            mid.set_hexpand(true);
                            let t = gtk::Label::new(Some(&format!("{}  ·  {}", title, author)));
                            t.set_halign(gtk::Align::Start);
                            t.set_ellipsize(gtk::pango::EllipsizeMode::Middle);
                            mid.append(&t);
                            let dd = gtk::Label::new(Some(&clean_md(&desc).lines().next().unwrap_or("").to_string()));
                            dd.set_halign(gtk::Align::Start);
                            dd.set_ellipsize(gtk::pango::EllipsizeMode::End);
                            dd.set_opacity(0.6);
                            dd.add_css_class("time-label");
                            mid.append(&dd);
                            let badges = gtk::Box::new(gtk::Orientation::Horizontal, 4);
                            for bl in ["fabric", "forge", "neoforge", "quilt"] {
                                if cats.iter().any(|c| c == bl) {
                                    let b = gtk::Label::new(Some(bl));
                                    b.add_css_class("proton-path-badge");
                                    badges.append(&b);
                                }
                            }
                            mid.append(&badges);
                            row.append(&mid);
                            let inst_btn = themed_btn("download", "Install", vv_c.state.theme.is_dark(), 16);
                            inst_btn.add_css_class("add-btn");
                            inst_btn.set_valign(gtk::Align::Center);
                            paint_accent(&inst_btn, &vv_c.state.theme);
                            let vv2 = vv_c.clone();
                            let pidc = pid.clone();
                            let titlec = title.clone();
                            let mc2 = mc_c.clone();
                            let ldinst = loader_c.clone();
                            let type_drop2 = type_drop_c.clone();
                            let view_btn = gtk::Button::with_label("View");
                            view_btn.add_css_class("settings-btn");
                            view_btn.set_valign(gtk::Align::Center);
                            let vvw = vv_c.clone();
                            let pidw = pid.clone();
                            let mcw = mc_c.clone();
                            let ldw = loader_c.clone();
                            let atw = match type_drop_c.selected() {
                                1 => "resourcepacks",
                                2 => "shaders",
                                _ => "mods",
                            }.to_string();
                            let kind_btn = kind_rows.clone();
                            view_btn.connect_clicked(move |_| {
                                let pidc2 = pidw.clone();
                                let mcw2 = mcw.clone();
                                let ldw2 = ldw.clone();
                                let atw2 = atw.clone();
                                let kind2 = kind_btn.clone();
                                let vvw2 = vvw.clone();
                                if is_cf {
                                    let pidv2 = pidw.clone();
                                    let vvw3 = vvw.clone();
                                    let cb: Box<dyn Fn()> = Box::new(move || {
                                        vvw3.install_cf_with_progress(&pidc2, "", &mcw2, &ldw2, &kind2, &atw2, &pidc2);
                                    });
                                    vvw.show_cf_project_view(&pidv2, Some(("Install".to_string(), cb)));
                                } else {
                                    let cb: Box<dyn Fn()> = Box::new(move || {
                                        vvw2.do_browse_install_simple(&pidc2, &mcw2, &ldw2, &atw2);
                                    });
                                    vvw.show_project_view(&pidw, Some(("Install".to_string(), cb)));
                                }
                            });
                            row.append(&view_btn);
                            let vers_btn = gtk::Button::with_label("Versions");
                            vers_btn.add_css_class("settings-btn");
                            vers_btn.set_valign(gtk::Align::Center);
                            let vv4 = vv_c.clone();
                            let pidv = pid.clone();
                            let ldv = loader_c.clone();
                            let mcv = mc_c.clone();
                            let kindv = kind_rows.clone();
                            vers_btn.connect_clicked(move |_| {
                                if is_cf {
                                    vv4.show_cf_versions(&pidv, &titlec, &mcv, &ldv, &kindv);
                                } else {
                                    vv4.show_mod_versions(&pidv, &titlec, &mcv, &ldv);
                                }
                            });
                            row.append(&vers_btn);
                            let kind_inst = kind_rows.clone();
                            inst_btn.connect_clicked(move |_| {
                                let at = match type_drop2.selected() {
                                    1 => "resourcepacks",
                                    2 => "shaders",
                                    _ => "mods",
                                }.to_string();
                                if is_cf {
                                    vv2.install_cf_with_progress(&pidc, "", &mc2, &ldinst, &kind_inst, &at, &title);
                                } else {
                                    vv2.install_addon_with_progress(&pidc, "", &mc2, &ldinst, &at, &title);
                                }
                            });
                            row.append(&inst_btn);
                            results_c.append(&row);
                        }
                        glib::ControlFlow::Break
                    }
                    Ok(Err(e)) => {
                        while let Some(c) = results_c.first_child() {
                            results_c.remove(&c);
                        }
                        results_c.append(&note(&format!("Search failed: {}", e)));
                        glib::ControlFlow::Break
                    }
                    Err(std::sync::mpsc::TryRecvError::Empty) => glib::ControlFlow::Continue,
                    Err(_) => glib::ControlFlow::Break,
                });
            })
        };
        {
            let ds = do_search.clone();
            go.connect_clicked(move |_| ds.borrow()());
        }
        {
            let ds = do_search.clone();
            q_entry.connect_activate(move |_| ds.borrow()());
        }
        {
            // live search while typing (debounced) + reload on type change
            let pending: Rc<RefCell<Option<glib::SourceId>>> = Rc::new(RefCell::new(None));
            let ds = do_search.clone();
            let pend = pending.clone();
            q_entry.connect_search_changed(move |_| {
                if let Some(id) = pend.borrow_mut().take() {
                    id.remove();
                }
                let ds2 = ds.clone();
                let pend2 = pend.clone();
                *pend.borrow_mut() = Some(glib::timeout_add_local_once(
                    std::time::Duration::from_millis(600),
                    move || {
                        *pend2.borrow_mut() = None;
                        ds2.borrow()();
                    },
                ));
            });
            let ds2 = do_search.clone();
            type_drop.connect_selected_notify(move |_| ds2.borrow()());
            let ds3 = do_search.clone();
            src_drop.connect_selected_notify(move |_| ds3.borrow()());

        }
        do_search.borrow()();
        dlg.present(Some(&self.parent));
    }

    // ============ global MC settings dialog (Carbon Settings page) ============

    fn render_java_card(&self, java_box: &gtk::Box, java_lbl: &gtk::Label) {
        while let Some(c) = java_box.first_child() {
            java_box.remove(&c);
        }
        java_lbl.set_text("Detecting Java…");
        let (tx, rx) = std::sync::mpsc::channel::<(Vec<(String, String, String)>, String)>();
        std::thread::spawn(move || {
            let found: Vec<(String, String, String)> = MinecraftManager::java_detect().ok()
                .and_then(|d| d.get("found").cloned()).and_then(|x| x.as_array().cloned()).unwrap_or_default()
                .into_iter().map(|j| (
                    j.get("path").and_then(|x| x.as_str()).unwrap_or("").to_string(),
                    j.get("version").and_then(|x| x.as_str()).unwrap_or("").to_string(),
                    j.get("origin").and_then(|x| x.as_str()).unwrap_or("").to_string(),
                )).collect();
            let sel = MinecraftManager::status().ok()
                .and_then(|d| d.get("java").and_then(|j| j.get("selected")).and_then(|x| x.as_str()).map(str::to_string))
                .unwrap_or_default();
            let _ = tx.send((found, sel));
        });
        let v = self.clone();
        let box_c = java_box.clone();
        let lbl_c = java_lbl.clone();
        glib::idle_add_local(move || match rx.try_recv() {
            Ok((found, sel)) => {
                v.data.borrow_mut().java_found = found.clone();
                v.data.borrow_mut().java_selected = sel.clone();
                lbl_c.set_text(&format!("Selected: {}",
                    if sel.is_empty() { "(auto)".to_string() } else { sel.clone() }));
                if found.is_empty() {
                    box_c.append(&note("No Java found — install 8/17/21/25 below."));
                }
                for (path, version, origin) in &found {
                    let bc = box_c.clone();
                    let lc = lbl_c.clone();
                    let row = gtk::Box::new(gtk::Orientation::Horizontal, 6);
                    let current = *path == sel;
                    let lbl = gtk::Label::new(Some(&format!("{}{} ({}, {})",
                        path, if current { " ✓" } else { "" }, version, origin)));
                    lbl.set_halign(gtk::Align::Start);
                    lbl.set_hexpand(true);
                    lbl.set_ellipsize(gtk::pango::EllipsizeMode::Middle);
                    let use_btn = gtk::Button::with_label(if current { "Current" } else { "Use" });
                    use_btn.add_css_class("settings-btn");
                    use_btn.set_sensitive(!current);
                    let vv = v.clone();
                    let pc = path.clone();
                    use_btn.connect_clicked(move |_| {
                        let vv = vv.clone();
                        let bc = bc.clone();
                        let lc = lc.clone();
                        let (tx2, rx2) = std::sync::mpsc::channel::<Result<(), String>>();
                        let pcc = pc.clone();
                        std::thread::spawn(move || {
                            let _ = tx2.send(MinecraftManager::java_use(&pcc).map(|_| ()).map_err(|e| e.to_string()));
                        });
                        glib::idle_add_local(move || match rx2.try_recv() {
                            Ok(Ok(_)) => {
                                vv.toast("Java selected", "Default Java updated.");
                                vv.refresh_all();
                                vv.render_java_card(&bc, &lc);
                                glib::ControlFlow::Break
                            }
                            Ok(Err(e)) => {
                                vv.toast("Failed", &e);
                                glib::ControlFlow::Break
                            }
                            Err(std::sync::mpsc::TryRecvError::Empty) => glib::ControlFlow::Continue,
                            Err(_) => glib::ControlFlow::Break,
                        });
                    });
                    row.append(&lbl);
                    row.append(&use_btn);
                    box_c.append(&row);
                }
                glib::ControlFlow::Break
            }
            Err(std::sync::mpsc::TryRecvError::Empty) => glib::ControlFlow::Continue,
            Err(_) => glib::ControlFlow::Break,
        });
    }

    fn show_mc_settings(&self) {
        self.show_mc_settings_tab("general");
    }

    fn show_mc_settings_tab(&self, initial: &str) {
        let dlg = adw::Dialog::new();
        dlg.set_title("Minecraft settings");
        dlg.set_content_width(560);
        dlg.set_content_height(600);
        let (header, x_btn) = helpers::modal_header("Minecraft settings");
        let content = gtk::Box::new(gtk::Orientation::Vertical, 0);
        content.add_css_class("modal-bg");
        content.append(&header);
        let stack = gtk::Stack::new();
        stack.set_vexpand(true);

        // General
        let gen_page = gtk::Box::new(gtk::Orientation::Vertical, 8);
        gen_page.set_margin_top(12);
        gen_page.set_margin_start(16);
        gen_page.set_margin_end(16);
        let (setup_frame, setup_inner) = card("Setup");
        let setup_lbl = note(if self.data.borrow().deps_ok { "Dependencies ready." } else { "Missing deps — press Install." });
        setup_inner.append(&setup_lbl);
        let setup_btn = gtk::Button::with_label("Install Python deps");
        setup_btn.add_css_class("add-btn");
        setup_btn.set_visible(!self.data.borrow().deps_ok);
        setup_inner.append(&setup_btn);
        gen_page.append(&setup_frame);
        gen_page.append(&note("Default memory for new instances (per-instance override in its Settings tab)"));
        let ram_row = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        let ram_lbl = gtk::Label::new(Some(&format!("{} MB", self.cfg("McRam"))));
        let ram = gtk::Scale::with_range(gtk::Orientation::Horizontal, 512.0, 16384.0, 256.0);
        ram.set_value(self.cfg("McRam").parse().unwrap_or(2048.0));
        ram.set_hexpand(true);
        ram.set_draw_value(false);
        let ram_entry = gtk::Entry::new();
        ram_entry.set_width_request(90);
        ram_entry.set_text(&format!("{}", ram.value() as u32));
        ram_row.append(&ram_lbl);
        ram_row.append(&ram);
        ram_row.append(&ram_entry);
        gen_page.append(&ram_row);
        gen_page.append(&note("Default extra JVM arguments"));
        let gjvm = gtk::Entry::new();
        gjvm.set_text(&self.cfg("McJvmArgs"));
        gen_page.append(&gjvm);
        gen_page.append(&note("Default resolution (empty = game default)"));
        let gres_row = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        let gres_w = gtk::Entry::new();
        gres_w.set_placeholder_text(Some("width"));
        gres_w.set_text(&self.cfg("McResW"));
        gres_w.set_hexpand(true);
        let gres_h = gtk::Entry::new();
        gres_h.set_placeholder_text(Some("height"));
        gres_h.set_text(&self.cfg("McResH"));
        gres_h.set_hexpand(true);
        gres_row.append(&gres_w);
        gres_row.append(&gtk::Label::new(Some("×")));
        gres_row.append(&gres_h);
        gen_page.append(&gres_row);
        let gen_save = gtk::Button::with_label("Save defaults");
        gen_save.add_css_class("add-btn");
        gen_page.append(&gen_save);
        stack.add_titled(&gen_page, Some("general"), "General");

        // Accounts
        let acc_page = gtk::Box::new(gtk::Orientation::Vertical, 8);
        acc_page.set_margin_top(12);
        acc_page.set_margin_start(16);
        acc_page.set_margin_end(16);
        let (acc_frame, acc_inner) = card("Accounts");
        let accounts_box = gtk::Box::new(gtk::Orientation::Vertical, 4);
        acc_inner.append(&accounts_box);
        for (aid, name, offline, ely) in self.data.borrow().accounts.clone() {
            let row = gtk::Box::new(gtk::Orientation::Horizontal, 6);
            let tag = if ely { "(Ely.by)" } else if offline { "(offline)" } else { "(MS)" };
            let lbl = gtk::Label::new(Some(&format!("{} {}", name, tag)));
            lbl.set_halign(gtk::Align::Start);
            lbl.set_hexpand(true);
            let rm = gtk::Button::with_label("Remove");
            rm.add_css_class("settings-btn");
            let v = self.clone();
            let idc = aid.clone();
            rm.connect_clicked(move |_| {
                let v = v.clone();
                let idc = idc.clone();
                let (tx, rx) = std::sync::mpsc::channel::<Result<(), String>>();
                std::thread::spawn(move || {
                    let _ = tx.send(MinecraftManager::unaccount(&idc).map(|_| ()).map_err(|e| e.to_string()));
                });
                glib::idle_add_local(move || match rx.try_recv() {
                    Ok(Ok(_)) => {
                        v.toast("Removed", "Account removed.");
                        v.refresh_all();
                        glib::ControlFlow::Break
                    }
                    Ok(Err(e)) => {
                        v.toast("Failed", &e);
                        glib::ControlFlow::Break
                    }
                    Err(std::sync::mpsc::TryRecvError::Empty) => glib::ControlFlow::Continue,
                    Err(_) => glib::ControlFlow::Break,
                });
            });
            row.append(&lbl);
            // GDL-style account head (async, cached PNGs)
            let head_img = gtk::Image::new();
            head_img.set_pixel_size(32);
            head_img.set_valign(gtk::Align::Center);
            head_img.set_tooltip_text(Some(&name));
            row.append(&head_img);
            {
                let nm = name.clone();
                let aidc = aid.clone();
                let (tx, rx) = std::sync::mpsc::channel::<Option<String>>();
                std::thread::spawn(move || {
                    let doc = MinecraftManager::ely_skin(&nm, &aidc, false).ok();
                    let head = doc.as_ref().and_then(|d| d.get("head")).and_then(|x| x.as_str()).map(str::to_string);
                    let _ = tx.send(head);
                });
                glib::idle_add_local(move || match rx.try_recv() {
                    Ok(Some(path)) => {
                        if let Some(tex) = helpers::load_texture(&path) {
                            head_img.set_paintable(Some(&tex));
                            head_img.set_pixel_size(32);
                        }
                        glib::ControlFlow::Break
                    }
                    Ok(None) => glib::ControlFlow::Break,
                    Err(std::sync::mpsc::TryRecvError::Empty) => glib::ControlFlow::Continue,
                    Err(_) => glib::ControlFlow::Break,
                });
            }
            row.append(&rm);
            accounts_box.append(&row);
        }
        if self.data.borrow().accounts.is_empty() {
            acc_inner.append(&note("No accounts yet."));
        }
        let acc_row = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        let ms_btn = gtk::Button::with_label("Add Microsoft");
        ms_btn.add_css_class("settings-btn");
        ms_btn.set_hexpand(true);
        let off_entry = gtk::Entry::new();
        off_entry.set_placeholder_text(Some("offline name"));
        off_entry.set_hexpand(true);
        let off_btn = gtk::Button::with_label("Add offline");
        off_btn.add_css_class("settings-btn");
        acc_row.append(&ms_btn);
        acc_row.append(&off_entry);
        acc_row.append(&off_btn);
        acc_inner.append(&acc_row);
        let ms_lbl = note("");
        ms_lbl.set_visible(false);
        acc_inner.append(&ms_lbl);
        let ely_row = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        let ely_user = gtk::Entry::new();
        ely_user.set_placeholder_text(Some("Ely.by username or e-mail"));
        ely_user.set_hexpand(true);
        let ely_pass = gtk::Entry::new();
        ely_pass.set_placeholder_text(Some("password (use password:token with 2FA)"));
        ely_pass.set_visibility(false);
        ely_pass.set_hexpand(true);
        let ely_btn = gtk::Button::with_label("Add Ely.by");
        ely_btn.add_css_class("settings-btn");
        let ely_reg = gtk::Button::with_label("Register");
        ely_reg.add_css_class("settings-btn");
        ely_reg.set_tooltip_text(Some("Open Ely.by to create an account"));
        ely_row.append(&ely_user);
        ely_row.append(&ely_pass);
        ely_row.append(&ely_btn);
        ely_row.append(&ely_reg);
        acc_inner.append(&ely_row);
        let ely_lbl = note("");
        ely_lbl.set_visible(false);
        acc_inner.append(&ely_lbl);

        acc_page.append(&acc_frame);
        // Skins (Ely.by skins system + Mojang proxy, per account type)
        let (skin_frame, skin_inner) = card("Skins");
        let skin_row = gtk::Box::new(gtk::Orientation::Horizontal, 12);
        let skin_prev = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        skin_prev.set_halign(gtk::Align::Center);
        skin_prev.add_css_class("mc-head");
        let skin_img = gtk::Image::new();
        skin_img.set_pixel_size(128);
        skin_img.set_valign(gtk::Align::Center);
        skin_prev.append(&skin_img);
        let skin_head_img = gtk::Image::new();
        skin_head_img.set_pixel_size(64);
        skin_head_img.set_valign(gtk::Align::Center);
        skin_prev.append(&skin_head_img);
        skin_row.append(&skin_prev);
        let skin_mid = gtk::Box::new(gtk::Orientation::Vertical, 4);
        skin_mid.set_hexpand(true);
        skin_mid.set_valign(gtk::Align::Center);
        let skin_for = gtk::Label::new(Some(""));
        skin_for.set_halign(gtk::Align::Start);
        skin_for.add_css_class("details-title");
        skin_mid.append(&skin_for);
        let skin_model = gtk::Label::new(Some(""));
        skin_model.set_halign(gtk::Align::Start);
        skin_model.add_css_class("proton-path-badge");
        skin_model.set_visible(false);
        skin_mid.append(&skin_model);
        skin_mid.append(&note("Ely.by accounts use the Ely wardrobe. Microsoft skins come from Mojang. Offline names show a match when one exists."));
        skin_row.append(&skin_mid);
        let skin_btns = gtk::Box::new(gtk::Orientation::Vertical, 4);
        let skin_refresh = gtk::Button::with_label("Refresh");
        skin_refresh.add_css_class("settings-btn");
        let skin_change = gtk::Button::with_label("Change skin…");
        skin_change.add_css_class("settings-btn");
        skin_btns.append(&skin_refresh);
        skin_btns.append(&skin_change);
        skin_row.append(&skin_btns);
        skin_inner.append(&skin_row);
        let skin_lbl = note("");
        skin_lbl.set_visible(false);
        skin_inner.append(&skin_lbl);
        acc_page.append(&skin_frame);
        stack.add_titled(&acc_page, Some("accounts"), "Accounts");

        // Java
        let java_page = gtk::Box::new(gtk::Orientation::Vertical, 8);
        java_page.set_margin_top(12);
        java_page.set_margin_start(16);
        java_page.set_margin_end(16);
        let (java_frame, java_inner) = card("Java");
        let java_lbl = note(&format!("Selected: {}",
            {
                let s = &self.data.borrow().java_selected;
                if s.is_empty() { "(auto)".to_string() } else { s.clone() }
            }));
        java_inner.append(&java_lbl);
        let java_box = gtk::Box::new(gtk::Orientation::Vertical, 4);
        java_inner.append(&java_box);
        self.render_java_card(&java_box, &java_lbl);
        let java_row = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        for ver in ["8", "17", "21", "25"] {
            let b = gtk::Button::with_label(&format!("Install {}", ver));
            b.add_css_class("settings-btn");
            b.set_hexpand(true);
            let v = self.clone();
            let verc = ver.to_string();
            let d = dlg.clone();
            let java_box_c = java_box.clone();
            let java_lbl_c = java_lbl.clone();
            b.connect_clicked(move |_| {
                let rx = MinecraftManager::spawn_java_install(verc.clone());
                let vv = v.clone();
                let verc2 = verc.clone();
                let java_box_c = java_box_c.clone();
                let java_lbl_c = java_lbl_c.clone();
                crate::backend::plugin_process::pump_to_idle(rx, move |ev| {
                    match ev {
                        crate::backend::plugin_process::PluginEvent::Done(val) => {
                            let p = val.get("path").and_then(|x| x.as_str()).unwrap_or("");
                            vv.toast("Java installed", &format!("{}: {}", verc2, p));
                            vv.refresh_all();
                            vv.render_java_card(&java_box_c.clone(), &java_lbl_c.clone());
                            false
                        }
                        crate::backend::plugin_process::PluginEvent::Error { message, .. } => {
                            vv.toast("Java install failed", &message);
                            false
                        }
                        _ => true,
                    }
                });
                let _ = d;
            });
            java_row.append(&b);
        }
        java_inner.append(&java_row);
        java_page.append(&java_frame);
        stack.add_titled(&java_page, Some("java"), "Java");
        let ap_page = gtk::Box::new(gtk::Orientation::Vertical, 8);
        ap_page.set_margin_top(12);
        ap_page.set_margin_start(16);
        ap_page.set_margin_end(16);
        let (ap_frame, ap_inner) = card("Appearance");
        ap_inner.append(&note("Instance tile size (preview below, library updates live)"));
        let tile_store = gtk::StringList::new(&["Small (48px)", "Medium (64px)", "Large (96px)"]);
        let tile_drop = gtk::DropDown::new(Some(tile_store), gtk::Expression::NONE);
        tile_drop.set_selected(match self.cfg("McTileSize").as_str() {
            "48" => 0,
            "96" => 2,
            _ => 1,
        });
        ap_inner.append(&tile_drop);
        let compact_check = gtk::CheckButton::with_label("Compact tiles (less padding, narrower cards)");
        compact_check.set_active(self.cfg("McCompact") == "1");
        ap_inner.append(&compact_check);
        let (prev_frame, prev_inner) = card("Preview");
        let prev_tile = gtk::Box::new(gtk::Orientation::Vertical, 4);
        prev_tile.set_halign(gtk::Align::Center);
        prev_tile.add_css_class("page-card");
        prev_tile.add_css_class("mc-tile");
        let prev_img = helpers::themed_image("minecraft", self.state.theme.is_dark(), 64);
        prev_img.set_halign(gtk::Align::Center);
        prev_tile.append(&prev_img);
        let prev_lbl = gtk::Label::new(Some("Example instance"));
        prev_lbl.set_halign(gtk::Align::Center);
        prev_lbl.add_css_class("details-title");
        prev_tile.append(&prev_lbl);
        prev_inner.append(&prev_tile);
        ap_inner.append(&prev_frame);
        ap_page.append(&ap_frame);
        stack.add_titled(&ap_page, Some("appearance"), "Appearance");
        content.append(&stack);

        // tab bar
        let tab_bar = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        let ids = ["general", "accounts", "java", "appearance"];
        let labels = ["General", "Accounts", "Java", "Appearance"];
        let mut btns: Vec<gtk::ToggleButton> = Vec::new();
        let mut inds: Vec<gtk::Box> = Vec::new();
        let init_tab = initial.to_string();
        stack.set_visible_child_name(&init_tab);
        let set_icons = ["preferences-other-symbolic", "system-users-symbolic", "application-x-executable-symbolic", "view-grid-symbolic"];
        for ((label, id), tab_icon) in labels.iter().zip(ids.iter()).zip(set_icons.iter()) {
            let wrap = gtk::Box::new(gtk::Orientation::Vertical, 1);
            wrap.set_hexpand(true);
            let btn = gtk::ToggleButton::new();
            btn.add_css_class("settings-tab");
            btn.set_hexpand(true);
            let c = gtk::Box::new(gtk::Orientation::Vertical, 2);
            c.set_halign(gtk::Align::Center);
            c.append(&sym(tab_icon, 18));
            let lbl = gtk::Label::new(Some(label));
            lbl.add_css_class("time-label");
            c.append(&lbl);
            let ind = gtk::Box::new(gtk::Orientation::Horizontal, 0);
            ind.add_css_class("settings-tab-indicator");
            ind.set_visible(*id == init_tab);
            c.append(&ind);
            btn.set_child(Some(&c));
            if *id == init_tab {
                btn.set_active(true);
            }
            wrap.append(&btn);
            tab_bar.append(&wrap);
            btns.push(btn);
            inds.push(ind);
        }
        for b in &btns[1..] {
            b.set_group(Some(&btns[0]));
        }
        for (i, id) in ids.iter().enumerate() {
            let st = stack.clone();
            let tid = id.to_string();
            let all = inds.clone();
            let mine = inds[i].clone();
            btns[i].connect_toggled(move |b| {
                if b.is_active() {
                    st.set_visible_child_name(&tid);
                    for ind in &all {
                        ind.set_visible(false);
                    }
                    mine.set_visible(true);
                }
            });
        }
        content.append(&tab_bar);
        dlg.set_child(Some(&content));
        {
            let d = dlg.clone();
            x_btn.connect_clicked(move |_| { d.close(); });
        }
        {
            let v = self.clone();
            setup_btn.connect_clicked(move |_| {
                let v = v.clone();
                let (tx, rx) = std::sync::mpsc::channel::<Result<(), String>>();
                std::thread::spawn(move || {
                    let _ = tx.send(MinecraftManager::setup().map(|_| ()).map_err(|e| e.to_string()));
                });
                glib::idle_add_local(move || match rx.try_recv() {
                    Ok(Ok(_)) => {
                        v.toast("Setup done", "Dependencies installed.");
                        v.refresh_all();
                        glib::ControlFlow::Break
                    }
                    Ok(Err(e)) => {
                        v.toast("Setup failed", &e);
                        glib::ControlFlow::Break
                    }
                    Err(std::sync::mpsc::TryRecvError::Empty) => glib::ControlFlow::Continue,
                    Err(_) => glib::ControlFlow::Break,
                });
            });
        }
        {
            let v = self.clone();
            let l = ram_lbl.clone();
            let re = ram_entry.clone();
            ram.connect_value_changed(move |s| {
                let mb = s.value() as u32;
                l.set_text(&format!("{} MB", mb));
                re.set_text(&mb.to_string());
                v.set_cfg("McRam", &mb.to_string());
            });
            let rr = ram.clone();
            ram_entry.connect_activate(move |e| {
                let raw = e.text().to_string().trim().to_string();
                if let Ok(mb) = raw.parse::<f64>() {
                    rr.set_value(mb.clamp(512.0, 16384.0));
                } else {
                    e.set_text(&format!("{}", rr.value() as u32));
                }
            });
            let rr2 = ram.clone();
            let re2 = ram_entry.clone();
            let rfocus = gtk::EventControllerFocus::new();
            rfocus.connect_leave(move |_| {
                let raw = re2.text().to_string().trim().to_string();
                if let Ok(mb) = raw.parse::<f64>() {
                    rr2.set_value(mb.clamp(512.0, 16384.0));
                }
            });
            ram_entry.add_controller(rfocus);
        }
        {
            let v = self.clone();
            gen_save.connect_clicked(move |_| {
                v.set_cfg("McJvmArgs", &gjvm.text().to_string().trim());
                v.set_cfg("McResW", &gres_w.text().to_string().trim().chars().filter(|c| c.is_ascii_digit()).collect::<String>());
                v.set_cfg("McResH", &gres_h.text().to_string().trim().chars().filter(|c| c.is_ascii_digit()).collect::<String>());
                v.toast("Saved", "Defaults saved.");
            });
        }
        {
            let v = self.clone();
            let img = prev_img.clone();
            let tile = prev_tile.clone();
            let upd = Rc::new(RefCell::new(Box::new(move || {}) as Box<dyn Fn()>));
            *upd.borrow_mut() = {
                let v = v.clone();
                Box::new(move || {
                    let size: i32 = v.cfg("McTileSize").parse().unwrap_or(64);
                    img.set_pixel_size(size);
                    let compact = v.cfg("McCompact") == "1";
                    tile.set_margin_top(if compact { 4 } else { 8 });
                    tile.set_margin_bottom(if compact { 4 } else { 8 });
                    tile.set_margin_start(if compact { 4 } else { 8 });
                    tile.set_margin_end(if compact { 4 } else { 8 });
                })
            };
            {
                let u = upd.clone();
                let vv = v.clone();
                tile_drop.connect_selected_notify(move |d| {
                    vv.set_cfg("McTileSize", match d.selected() {
                        0 => "48",
                        2 => "96",
                        _ => "64",
                    });
                    u.borrow()();
                    vv.render_library();
                });
            }
            {
                let u = upd.clone();
                let vv = v.clone();
                compact_check.connect_toggled(move |b| {
                    vv.set_cfg("McCompact", if b.is_active() { "1" } else { "0" });
                    u.borrow()();
                    vv.render_library();
                });
            }
            upd.borrow()();
        }
        {
            // skin preview loader (selected account, cached PNGs)
            let v = self.clone();
            let img = skin_img.clone();
            let lbl = skin_lbl.clone();
            let who = skin_for.clone();
            let load = Rc::new(RefCell::new(Box::new(move |force: bool| {}) as Box<dyn Fn(bool)>));
            let vv0 = v.clone();
            *load.borrow_mut() = {
                Box::new(move |force: bool| {
                    let head_img = skin_head_img.clone();
                    let model_lbl = skin_model.clone();
                    let img = img.clone();
                    let lbl = lbl.clone();
                    let who = who.clone();
                    let vv = vv0.clone();
                    let (aid, name) = vv.selected_account();
                    if aid.is_empty() {
                        who.set_text("No account selected.");
                        return;
                    }
                    who.set_text(&format!("Skin of {}", name));
                    lbl.set_visible(false);
                    let uuid = aid.clone();
                    let (tx, rx) = std::sync::mpsc::channel::<Result<(String, String, String), String>>();
                    std::thread::spawn(move || {
                        let _ = tx.send(MinecraftManager::ely_skin(&name, &uuid, force).map(|d| (
                            d.get("skin").and_then(|x| x.as_str()).unwrap_or("").to_string(),
                            d.get("head").and_then(|x| x.as_str()).unwrap_or("").to_string(),
                            d.get("model").and_then(|x| x.as_str()).unwrap_or("classic").to_string(),
                        )).map_err(|e| e.to_string()));
                    });
                    glib::idle_add_local(move || match rx.try_recv() {
                        Ok(Ok((skin, head, model))) => {
                            if let Some(tex) = helpers::load_texture(&skin) {
                                img.set_paintable(Some(&tex));
                                img.set_pixel_size(128);
                            }
                            if let Some(tex) = helpers::load_texture(&head) {
                                head_img.set_paintable(Some(&tex));
                                head_img.set_pixel_size(64);
                            }
                            model_lbl.set_text(if model == "slim" { "Slim (Alex)" } else { "Classic (Steve)" });
                            model_lbl.set_visible(true);
                            glib::ControlFlow::Break
                        }
                        Ok(Err(e)) => {
                            lbl.set_visible(true);
                            lbl.set_text(&format!("No skin found — upload one in the wardrobe, then press Refresh. ({})", e.lines().next().unwrap_or("")));
                            glib::ControlFlow::Break
                        }
                        Err(std::sync::mpsc::TryRecvError::Empty) => glib::ControlFlow::Continue,
                        Err(_) => glib::ControlFlow::Break,
                    });
                })
            };
            let ld = load.clone();
            skin_refresh.connect_clicked(move |_| ld.borrow()(true));
            let vv = v.clone();
            skin_change.connect_clicked(move |_| {
                let (aid, name) = vv.selected_account();
                let ely_acc = vv.data.borrow().accounts.iter().any(|(i, _, _, e)| i == &aid && *e);
                if ely_acc {
                    vv.state.integration.open_url("https://ely.by/skins");
                    vv.toast("Change skin", "Upload it in your Ely.by wardrobe, then press Refresh.");
                } else {
                    vv.state.integration.open_url("https://www.minecraft.net/msaprofile/mygames/editskin");
                    vv.toast("Change skin", "Change it on minecraft.net (Microsoft accounts), then press Refresh.");
                }
                let _ = name;
            });
            load.borrow()(false);
        }
        {
            let st = self.state.clone();
            ely_reg.connect_clicked(move |_| {
                st.integration.open_url("https://ely.by");
            });
        }
        {
            let v = self.clone();
            let eu = ely_user.clone();
            let ep = ely_pass.clone();
            let el = ely_lbl.clone();
            ely_btn.connect_clicked(move |_| {
                let v = v.clone();
                let el = el.clone();
                let user = eu.text().to_string().trim().to_string();
                let pwd = ep.text().to_string();
                if user.is_empty() || pwd.is_empty() {
                    v.toast("Missing data", "Type your Ely.by username and password first.");
                    return;
                }
                eu.set_text("");
                ep.set_text("");
                el.set_visible(true);
                el.set_text("Signing in to Ely.by…");
                let (tx, rx) = std::sync::mpsc::channel::<Result<String, String>>();
                std::thread::spawn(move || {
                    let _ = tx.send(MinecraftManager::ely_auth(&user, &pwd).map(|d| {
                        d.get("username").and_then(|x| x.as_str()).unwrap_or("").to_string()
                    }).map_err(|e| e.to_string()));
                });
                glib::idle_add_local(move || match rx.try_recv() {
                    Ok(Ok(username)) => {
                        el.set_text(&format!("Signed in as {}", username));
                        v.toast("Ely.by account added", &username);
                        v.refresh_all();
                        glib::ControlFlow::Break
                    }
                    Ok(Err(e)) => {
                        el.set_text(&format!("Sign in failed: {}", e));
                        glib::ControlFlow::Break
                    }
                    Err(std::sync::mpsc::TryRecvError::Empty) => glib::ControlFlow::Continue,
                    Err(_) => glib::ControlFlow::Break,
                });
            });
        }
        {
            let v = self.clone();
            ms_btn.connect_clicked(move |_| {
                if !v.data.borrow().azure_configured {
                    ms_lbl.set_visible(true);
                    ms_lbl.set_text("Microsoft login needs a registered Azure app (portal.azure.com → App registrations). Offline accounts work without it.");
                    v.state.integration.open_url("https://portal.azure.com/#view/Microsoft_AAD_RegisteredApps/ApplicationsListBlade");
                    return;
                }
                ms_lbl.set_visible(true);
                ms_lbl.set_text("Waiting for browser login…");
                let rx = MinecraftManager::spawn_auth();
                let lbl = ms_lbl.clone();
                let vv = v.clone();
                crate::backend::plugin_process::pump_to_idle(rx, move |ev| {
                    match ev {
                        crate::backend::plugin_process::PluginEvent::Custom(val) => {
                            let code = val.get("user_code").and_then(|x| x.as_str()).unwrap_or("");
                            let uri = val.get("verification_uri").and_then(|x| x.as_str()).unwrap_or("");
                            lbl.set_text(&format!("Code: {} — open {}", code, uri));
                            if let Some(u) = val.get("verification_uri").and_then(|x| x.as_str()) {
                                vv.state.integration.open_url(u);
                            }
                            true
                        }
                        crate::backend::plugin_process::PluginEvent::Done(val) => {
                            let user = val.get("username").and_then(|x| x.as_str()).unwrap_or("?");
                            lbl.set_text(&format!("Logged in as {}", user));
                            vv.toast("Account added", user);
                            vv.refresh_all();
                            false
                        }
                        crate::backend::plugin_process::PluginEvent::Error { message, .. } => {
                            lbl.set_text(&format!("Auth failed: {}", message));
                            false
                        }
                        _ => true,
                    }
                });
            });
        }
        {
            let v = self.clone();
            let e = off_entry.clone();
            off_btn.connect_clicked(move |_| {
                let name = e.text().to_string().trim().to_string();
                e.set_text("");
                if name.is_empty() {
                    v.toast("Name required", "Type an offline name first.");
                    return;
                }
                let v = v.clone();
                let (tx, rx) = std::sync::mpsc::channel::<Result<String, String>>();
                std::thread::spawn(move || {
                    let _ = tx.send(MinecraftManager::offline(&name).map(|_| name.clone()).map_err(|e| e.to_string()));
                });
                glib::idle_add_local(move || match rx.try_recv() {
                    Ok(Ok(n)) => {
                        v.toast("Account added", &n);
                        v.refresh_all();
                        glib::ControlFlow::Break
                    }
                    Ok(Err(e)) => {
                        v.toast("Failed", &e);
                        glib::ControlFlow::Break
                    }
                    Err(std::sync::mpsc::TryRecvError::Empty) => glib::ControlFlow::Continue,
                    Err(_) => glib::ControlFlow::Break,
                });
            });
        }
        dlg.present(Some(&self.parent));
    }
}

fn chrono_date(epoch: i64) -> String {
    // days since epoch -> YYYY-MM-DD via libc-less math
    let days = epoch.div_euclid(86400);
    let (mut y, mut m, mut d) = (1970i64, 1i64, 1i64);
    let mut rem = days;
    loop {
        let leap = (y % 4 == 0 && y % 100 != 0) || y % 400 == 0;
        let yd = if leap { 366 } else { 365 };
        if rem < yd {
            break;
        }
        rem -= yd;
        y += 1;
    }
    let months = [31, if (y % 4 == 0 && y % 100 != 0) || y % 400 == 0 { 29 } else { 28 }, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];
    for (i, dim) in months.iter().enumerate() {
        if rem < *dim {
            m = i as i64 + 1;
            d = rem + 1;
            break;
        }
        rem -= dim;
    }
    format!("{:04}-{:02}-{:02}", y, m, d)
}
