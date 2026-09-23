use std::path::PathBuf;

use gtk::prelude::*;

fn bundle_root() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".into());
    PathBuf::from(home).join(".local/share/corkytux/assets/icons")
}

fn main() {
    gtk::init().expect("gtk init");

    let bundle = bundle_root();
    let display = gtk::gdk::Display::default().expect("display");
    let it = gtk::IconTheme::for_display(&display);
    let mut paths = vec![bundle.clone()];
    paths.extend(it.search_path());
    let refs: Vec<&std::path::Path> = paths.iter().map(|p| p.as_path()).collect();
    it.set_search_path(&refs);

    let mut icons: Vec<String> = Vec::new();
    for ctx in ["actions", "categories", "status", "places", "mimetypes", "ui", "legacy"] {
        let dir = bundle.join("hicolor/symbolic").join(ctx);
        if let Ok(read) = std::fs::read_dir(&dir) {
            for e in read.flatten() {
                if let Some(n) = e.file_name().to_str() {
                    if let Some(stem) = n.strip_suffix(".svg") {
                        icons.push(stem.to_string());
                    }
                }
            }
        }
    }
    icons.sort();
    icons.dedup();

    for name in &icons {
        let paintable = it.lookup_icon(name, &[], 16, 1, gtk::TextDirection::None, gtk::IconLookupFlags::empty());
        let path = paintable
            .file()
            .and_then(|f| f.path())
            .map(|p| p.to_string_lossy().into_owned())
            .unwrap_or_else(|| "<sin archivo>".into());
        let resolved = std::fs::read(&path).unwrap_or_default();

        let mut bundle_bytes: Option<Vec<u8>> = None;
        for ctx in ["actions", "categories", "status", "places", "mimetypes", "ui", "legacy"] {
            let p = bundle.join("hicolor/symbolic").join(ctx).join(format!("{name}.svg"));
            if let Ok(b) = std::fs::read(&p) {
                bundle_bytes = Some(b);
                break;
            }
        }
        let from_bundle = bundle_bytes.is_some() && bundle_bytes.as_deref() == Some(resolved.as_slice());
        let owner = if from_bundle { "BUNDLE" } else if path.starts_with("/usr/share/") { "system" } else { "?" };
        println!("{name:40} -> {path}  [{owner}]");
    }
}