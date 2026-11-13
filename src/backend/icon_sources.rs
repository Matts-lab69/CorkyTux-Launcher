//! Shared local game-icon sources and shipped icon asset search paths.

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
        for up in [
            parent.to_path_buf(),
            parent.join(".."),
            parent.join("../.."),
        ] {
            let candidate = up.join("assets/icons");
            if candidate.is_dir() {
                push(candidate.canonicalize().unwrap_or(candidate));
            }
        }
    }
    paths
}

pub fn data_roots(home: &std::path::Path) -> Vec<std::path::PathBuf> {
    let data_home = std::env::var_os("XDG_DATA_HOME")
        .map(std::path::PathBuf::from)
        .filter(|path| path.is_absolute())
        .unwrap_or_else(|| home.join(".local/share"));
    let data_dirs = std::env::var_os("XDG_DATA_DIRS").unwrap_or_default();
    let mut system_roots: Vec<_> = std::env::split_paths(&data_dirs)
        .filter(|path| path.is_absolute())
        .collect();
    if system_roots.is_empty() {
        system_roots.extend(["/usr/local/share", "/usr/share"].map(std::path::PathBuf::from));
    }
    let mut roots = vec![data_home];
    roots.extend(system_roots);
    roots
}

/// Both the versioned game label and the stable AppImage application name.
fn game_icon_stems(name: &str, executable: &str) -> Vec<String> {
    let mut stems = Vec::new();
    let file_stem = std::path::Path::new(executable)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("");
    for raw in [name, file_stem] {
        let version = raw.char_indices().find_map(|(index, ch)| {
            let suffix = &raw[index + ch.len_utf8()..];
            (ch == '-'
                && suffix.split('-').next().unwrap_or("").contains('.')
                && suffix.as_bytes().first().is_some_and(u8::is_ascii_digit))
            .then_some(index)
        });
        for value in [raw, &raw[..version.unwrap_or(raw.len())]] {
            // Use the cache writer's exact normalization, including lowercase
            // expansion and collapsing consecutive separators.
            let slug = super::integration::slugify(value);
            if !slug.is_empty() && !stems.contains(&slug) {
                stems.push(slug);
            }
        }
    }
    stems
}

/// Local cache and matching desktop metadata, without scanning entire icon trees.
fn desktop_executable(app: &gio::DesktopAppInfo) -> Option<std::path::PathBuf> {
    use gio::prelude::AppInfoExt;
    // GDesktopAppInfo::executable may retain Exec quotes; parse, never execute.
    let args = glib::shell_parse_argv(app.commandline()?).ok()?;
    args.into_iter().next().map(std::path::PathBuf::from)
}

/// Parse only bytes read from a checked nonblocking regular-file descriptor.
/// Building from a key file avoids reopening a replaceable desktop pathname.
fn read_desktop_metadata(path: &std::path::Path) -> Option<gio::DesktopAppInfo> {
    let bytes = super::icon_io::read_icon_bytes(path)?;
    let text = std::str::from_utf8(bytes.as_ref()).ok()?;
    let key_file = glib::KeyFile::new();
    key_file
        .load_from_data(text, glib::KeyFileFlags::NONE)
        .ok()?;
    gio::DesktopAppInfo::from_keyfile(&key_file)
}

pub fn game_icon_candidates(
    home: &std::path::Path,
    data_dirs: &[std::path::PathBuf],
    name: &str,
    executable: &str,
) -> Vec<std::path::PathBuf> {
    let stems = game_icon_stems(name, executable);
    let mut paths = Vec::new();
    for stem in &stems {
        for suffix in [
            "-appimage.png",
            "-exe.png",
            "-sgdb.png",
            ".png",
            ".svg",
            ".ico",
            ".webp",
        ] {
            paths.push(
                home.join(".config/CorkyTux/icons")
                    .join(format!("{stem}{suffix}")),
            );
        }
    }
    for root in data_dirs {
        for stem in &stems {
            for desktop in [
                format!("{stem}.desktop"),
                format!("corkytux-{stem}.desktop"),
            ] {
                let Some(app) = read_desktop_metadata(&root.join("applications").join(desktop))
                else {
                    continue;
                };
                if desktop_executable(&app).as_deref() != Some(std::path::Path::new(executable))
                    && app.string("X-CorkyTux-Game").as_deref() != Some(name)
                {
                    continue;
                }
                let Some(icon) = app.string("Icon") else {
                    continue;
                };
                let path = std::path::Path::new(icon.as_str());
                if path.is_absolute() {
                    paths.push(path.to_path_buf());
                } else if !icon.contains('/') && !icon.is_empty() {
                    for icon_root in data_dirs {
                        for size in [
                            "512x512", "256x256", "128x128", "64x64", "48x48", "32x32", "scalable",
                        ] {
                            for ext in ["png", "svg", "xpm"] {
                                paths.push(
                                    icon_root
                                        .join("icons/hicolor")
                                        .join(size)
                                        .join("apps")
                                        .join(format!("{icon}.{ext}")),
                                );
                            }
                        }
                    }
                    paths.push(home.join(".icons").join(format!("{icon}.png")));
                }
            }
        }
    }
    paths
}

#[cfg(test)]
mod tests {
    use super::{game_icon_candidates, game_icon_stems, icon_search_paths};

    #[test]
    fn data_roots_ignore_empty_and_relative_xdg_paths() {
        let _lock = crate::test_env::lock();
        struct RestoreXdg(Vec<(&'static str, Option<std::ffi::OsString>)>);
        impl Drop for RestoreXdg {
            fn drop(&mut self) {
                for (key, value) in &self.0 {
                    match value {
                        Some(value) => std::env::set_var(key, value),
                        None => std::env::remove_var(key),
                    }
                }
            }
        }
        let _restore = RestoreXdg(
            ["XDG_DATA_HOME", "XDG_DATA_DIRS"]
                .into_iter()
                .map(|key| (key, std::env::var_os(key)))
                .collect(),
        );
        let home = std::path::Path::new("/home/example");
        for (data_home, data_dirs, expected) in [
            (
                None,
                None,
                vec![
                    "/home/example/.local/share",
                    "/usr/local/share",
                    "/usr/share",
                ],
            ),
            (
                Some(""),
                Some(""),
                vec![
                    "/home/example/.local/share",
                    "/usr/local/share",
                    "/usr/share",
                ],
            ),
            (
                Some("relative/home"),
                Some("relative/data:.:.."),
                vec![
                    "/home/example/.local/share",
                    "/usr/local/share",
                    "/usr/share",
                ],
            ),
            (
                Some("/custom/data"),
                Some(":"),
                vec!["/custom/data", "/usr/local/share", "/usr/share"],
            ),
            (
                Some("/custom/data"),
                Some("/opt/data::relative:/usr/share"),
                vec!["/custom/data", "/opt/data", "/usr/share"],
            ),
            (
                Some(""),
                Some("/first:/second"),
                vec!["/home/example/.local/share", "/first", "/second"],
            ),
        ] {
            for (key, value) in [("XDG_DATA_HOME", data_home), ("XDG_DATA_DIRS", data_dirs)] {
                match value {
                    Some(value) => std::env::set_var(key, value),
                    None => std::env::remove_var(key),
                }
            }
            let expected: Vec<std::path::PathBuf> = expected.into_iter().map(Into::into).collect();
            assert_eq!(
                super::data_roots(home),
                expected,
                "invalid XDG roots must never cause a current-directory metadata search"
            );
        }
    }

    #[test]
    fn desktop_metadata_fifo_and_symlink_never_wait_for_a_writer() {
        const CHILD_ROOT: &str = "CORKYTUX_DESKTOP_FIFO_REGRESSION_ROOT";
        const TEST: &str = "backend::icon_sources::tests::desktop_metadata_fifo_and_symlink_never_wait_for_a_writer";
        if let Some(root) = std::env::var_os(CHILD_ROOT) {
            let root = std::path::PathBuf::from(root);
            let paths = game_icon_candidates(&root, &[root.join("data")], "Pipe Game", "/bin/true");
            assert!(paths
                .iter()
                .all(|path| path.starts_with(root.join(".config"))));
            return;
        }
        use std::os::unix::ffi::OsStrExt;
        use std::time::{Duration, Instant};
        let root =
            std::env::temp_dir().join(format!("desktop-fifo-regression-{}", std::process::id()));
        let applications = root.join("data/applications");
        std::fs::create_dir_all(&applications).unwrap();
        let fifo = applications.join("pipe-game.desktop");
        let name = std::ffi::CString::new(fifo.as_os_str().as_bytes()).unwrap();
        // SAFETY: name is a live NUL-terminated pathname; mkfifo keeps no pointer.
        assert_eq!(unsafe { libc::mkfifo(name.as_ptr(), 0o600) }, 0);
        std::os::unix::fs::symlink(
            "pipe-game.desktop",
            applications.join("corkytux-pipe-game.desktop"),
        )
        .unwrap();
        let mut child = std::process::Command::new(std::env::current_exe().unwrap())
            .args(["--exact", TEST, "--nocapture"])
            .env(CHILD_ROOT, &root)
            .stdout(std::process::Stdio::null())
            .spawn()
            .unwrap();
        let deadline = Instant::now() + Duration::from_secs(3);
        let status = loop {
            if let Some(status) = child.try_wait().unwrap() {
                break Some(status);
            }
            if Instant::now() >= deadline {
                child.kill().unwrap();
                child.wait().unwrap();
                break None;
            }
            std::thread::sleep(Duration::from_millis(10));
        };
        std::fs::remove_dir_all(&root).unwrap();
        assert!(
            status.is_some_and(|status| status.success()),
            "desktop metadata lookup blocked on a FIFO with no writer or child failed"
        );
    }

    #[test]
    fn cache_aliases_match_writer_normalization() {
        let home = std::path::Path::new("/home/example");
        for (name, expected) in [
            ("Example: -- Game", "example-game-appimage.png"),
            ("İCON", "i-con-appimage.png"),
        ] {
            let paths = game_icon_candidates(home, &[], name, "/apps/launcher.AppImage");
            let cached = home.join(".config/CorkyTux/icons").join(expected);
            assert!(
                paths.contains(&cached),
                "missing writer-normalized alias: {cached:?}"
            );
        }
    }

    #[test]
    fn gamepad_openchamber_uses_stable_cache_and_matching_desktop_icon() {
        let root = std::env::temp_dir().join(format!("gamepad-icon-{}", std::process::id()));
        let home = root.join("home");
        let data = root.join("data");
        std::fs::create_dir_all(data.join("applications")).unwrap();
        let executable_path = root.join("OpenChamber-2.5.2-linux-x86_64.AppImage");
        std::fs::write(&executable_path, b"#!/bin/sh\nexit 99\n").unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&executable_path, std::fs::Permissions::from_mode(0o700))
                .unwrap();
        }
        let executable = executable_path.to_str().unwrap();
        let name = "OpenChamber-2.5.2-linux-x86_64";
        let icon = root.join("embedded.png");
        std::fs::write(data.join("applications/openchamber.desktop"), format!(
            "[Desktop Entry]\nType=Application\nName=OpenChamber\nExec=\"{executable}\" %U\nIcon={}\n", icon.display()
        )).unwrap();
        let desktop =
            gio::DesktopAppInfo::from_filename(data.join("applications/openchamber.desktop"))
                .expect("desktop fixture needs an existing executable; it is never run");
        assert_eq!(
            super::desktop_executable(&desktop).as_deref(),
            Some(executable_path.as_path())
        );
        let paths = game_icon_candidates(&home, &[data.clone()], name, executable);
        assert!(paths.contains(&home.join(".config/CorkyTux/icons/openchamber.png")));
        assert!(paths.contains(&icon));
        let wrong = game_icon_candidates(&home, &[data], name, "/different/OpenChamber.AppImage");
        assert!(!wrong.contains(&icon));
        assert!(game_icon_stems(name, executable).contains(&"openchamber".to_string()));
        std::fs::remove_dir_all(root).unwrap();
    }

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
        assert!(
            paths.contains(&src_assets),
            "missing {:?} in {:?}",
            src_assets,
            paths
        );
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
