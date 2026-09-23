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

    let icons = [
        "epicgames-symbolic",
        "gogdotcom-symbolic",
        "applications-games-symbolic",
        "applications-engineering-symbolic",
        "preferences-other-symbolic",
        "preferences-system-symbolic",
        "folder-symbolic",
        "user-trash-symbolic",
        "system-run-symbolic",
        "system-users-symbolic",
        "view-grid-symbolic",
        "view-more-symbolic",
        "view-refresh-symbolic",
        "view-sort-ascending-symbolic",
        "document-edit-symbolic",
        "document-open-symbolic",
        "edit-copy-symbolic",
        "list-add-symbolic",
        "media-playback-start-symbolic",
        "media-playback-stop-symbolic",
        "go-previous-symbolic",
        "emblem-ok-symbolic",
        "window-close-symbolic",
        "pan-down-symbolic",
        "system-software-install-symbolic",
        "web-browser-symbolic",
        "alarm-symbolic",
        "avatar-default-symbolic",
        "display-brightness-symbolic",
        "non-starred-symbolic",
        "starred-symbolic",
        "software-update-available-symbolic",
        "application-x-addon-symbolic",
        "application-x-executable-symbolic",
        "image-x-generic-symbolic",
        "package-x-generic-symbolic",
        "text-x-generic-symbolic",
    ];

    for name in icons {
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