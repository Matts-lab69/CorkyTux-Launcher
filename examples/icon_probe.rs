use std::path::PathBuf;
use gtk::prelude::*;

fn main() {
    gtk::init().expect("gtk init");
    let home = std::env::var("HOME").expect("HOME");
    let mut theme = gtk::IconTheme::new();
    let bundled = PathBuf::from(&home).join(".local/share/corkytux/assets/icons");
    let mut paths = vec![bundled.clone()];
    paths.extend(theme.search_path());
    let refs: Vec<&std::path::Path> = paths.iter().map(|p| p.as_path()).collect();
    theme.set_search_path(&refs);

    println!("search_path[0] = {:?}", bundled);
    for name in ["epicgames-symbolic", "gogdotcom-symbolic", "applications-games-symbolic"] {
        let paintable = theme.lookup_icon(name, &[], 16, 1, gtk::TextDirection::None, gtk::IconLookupFlags::empty());
        match paintable.file().and_then(|f| f.path()) {
            Some(p) => println!("{name:24} -> Some({})", p.display()),
            None => println!("{name:24} -> Some(paintable) sin file"),
        }
    }
}