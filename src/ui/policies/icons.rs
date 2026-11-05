//! Where to look for the shipped icon assets: the user data dir, every XDG
//! data dir and, when running from the source tree, the repo assets.

/// Icon theme search paths for a game launcher: the installed assets, every
/// XDG data dir, and the source tree next to the executable (2 levels up),
/// only when those asset folders exist.
pub fn icon_search_paths(
    exe: &std::path::Path,
    home: &std::path::Path,
    xdg_data_dirs: &[std::path::PathBuf],
) -> Vec<std::path::PathBuf> {
    let mut paths = Vec::new();
    let mut push = |p: std::path::PathBuf| {
        if !paths.contains(&p) {
            paths.push(p);
        }
    };
    push(home.join(".local/share/corkytux/assets/icons"));
    for dir in xdg_data_dirs {
        push(dir.join("corkytux/assets/icons"));
    }
    if let Some(parent) = exe.parent() {
        for up in [parent.to_path_buf(), parent.join(".."), parent.join("../..")] {
            let candidate = up.join("assets/icons");
            if candidate.is_dir() {
                push(candidate.canonicalize().unwrap_or(candidate));
            }
        }
    }
    paths
}

#[cfg(test)]
mod tests {
    use super::icon_search_paths;

    #[test]
    fn icon_search_paths_cover_install_xdg_and_source_tree() {
        let base = std::env::temp_dir().join("corky-icon-paths");
        std::fs::create_dir_all(&base).unwrap();
        let base = base.canonicalize().unwrap();
        let home = base.join("home");
        let exe = base.join("repo/target/release/corkytux");
        let src_assets = base.join("repo/assets/icons");
        std::fs::create_dir_all(&src_assets).unwrap();
        std::fs::create_dir_all(&home).unwrap();
        // The binary's own directory must exist for `..` to resolve.
        std::fs::create_dir_all(base.join("repo/target/release")).unwrap();
        let xdg = vec![base.join("xdg1"), base.join("xdg2")];
        let paths = icon_search_paths(&exe, &home, &xdg);
        assert_eq!(paths[0], home.join(".local/share/corkytux/assets/icons"));
        assert!(paths.contains(&base.join("xdg1/corkytux/assets/icons")));
        assert!(paths.contains(&base.join("xdg2/corkytux/assets/icons")));
        assert!(paths.contains(&src_assets), "missing {:?} in {:?}", src_assets, paths);
        // No duplicates and no missing-relative entries.
        let mut unique = paths.clone();
        unique.sort();
        unique.dedup();
        assert_eq!(unique.len(), paths.len());
        // A build under target/ without assets next to it finds nothing extra.
        let bare = base.join("elsewhere/target/release/corkytux");
        let paths = icon_search_paths(&bare, &home, &xdg);
        assert!(!paths.iter().any(|p| p.ends_with("elsewhere/assets/icons")));
        let _ = std::fs::remove_dir_all(&base);
    }
}
