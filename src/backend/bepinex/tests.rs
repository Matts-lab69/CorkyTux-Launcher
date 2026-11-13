use super::*;
use std::io::{Cursor, Write};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use zip::write::SimpleFileOptions;

static NEXT: AtomicU64 = AtomicU64::new(0);

#[test]
fn bepinex_detection_does_not_claim_an_unrelated_winhttp_loader() {
    let root = Fixture::new();
    std::fs::create_dir_all(root.0.join("BepInEx/core")).unwrap();
    std::fs::write(root.0.join("winhttp.dll"), b"other loader").unwrap();
    let capability = fs::Root::open(&root.0).unwrap();
    assert!(!external_loader(&capability).unwrap());
}

#[test]
fn bepinex_manifest_update_is_atomic_when_directory_is_read_only() {
    use std::os::unix::fs::PermissionsExt;
    let root = Fixture::new();
    install_archive(&root.0, mono(), &archive(None), None, "").unwrap();
    let before = std::fs::read(root.0.join(MANIFEST)).unwrap();
    std::fs::set_permissions(&root.0, std::fs::Permissions::from_mode(0o500)).unwrap();
    let result = rebase_overrides(&root.0, "dxgi=b");
    std::fs::set_permissions(&root.0, std::fs::Permissions::from_mode(0o700)).unwrap();
    assert!(result.is_err());
    assert_eq!(std::fs::read(root.0.join(MANIFEST)).unwrap(), before);
}

#[test]
fn bepinex_activation_rejects_an_oversized_merged_value() {
    let value = format!("{}=b", "x".repeat(16 * 1024 - 2));
    assert!(combine_overrides(&value).is_err());
}

#[test]
fn bepinex_launch_environment_is_opt_in_and_preserves_other_overrides() {
    let mut game =
        std::collections::HashMap::from([("overrides".to_string(), "dxgi=b".to_string())]);
    let mut command = std::process::Command::new("never-executed");
    configure_launch(&mut command, &game).unwrap();
    assert!(command.get_envs().next().is_none());
    game.insert("bepinexenabled".into(), "true".into());
    configure_launch(&mut command, &game).unwrap();
    assert_eq!(
        command
            .get_envs()
            .find(|(key, _)| *key == "WINEDLLOVERRIDES")
            .unwrap()
            .1
            .unwrap(),
        "dxgi=b;winhttp=n,b"
    );
    game.insert("custom_dll_overrides".into(), String::new());
    configure_launch(&mut command, &game).unwrap();
    assert_eq!(
        command
            .get_envs()
            .find(|(key, _)| *key == "WINEDLLOVERRIDES")
            .unwrap()
            .1
            .unwrap(),
        "winhttp=n,b"
    );
}

#[test]
fn bepinex_manifest_cannot_claim_foreign_game_or_plugin_files() {
    for foreign in [
        "game.exe",
        "BepInEx/plugins/user.dll",
        "BepInEx/config/user.cfg",
    ] {
        let root = Fixture::new();
        install_archive(&root.0, mono(), &archive(None), None, "").unwrap();
        let path = root.0.join(foreign);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, b"user-owned").unwrap();
        let mut manifest: serde_json::Value =
            serde_json::from_slice(&std::fs::read(root.0.join(MANIFEST)).unwrap()).unwrap();
        manifest["files"]
            .as_array_mut()
            .unwrap()
            .push(serde_json::json!({"path":foreign,"sha256":package::digest(b"user-owned")}));
        std::fs::write(
            root.0.join(MANIFEST),
            serde_json::to_vec(&manifest).unwrap(),
        )
        .unwrap();
        assert!(disable(&root.0, "winhttp=n,b").is_err());
        assert_eq!(std::fs::read(path).unwrap(), b"user-owned");
        assert!(root.0.join("winhttp.dll").exists());
    }
}

#[test]
fn bepinex_case_colliding_directories_are_not_created() {
    let root = Fixture::new();
    std::fs::create_dir(root.0.join("bepinex")).unwrap();
    assert!(install_archive(&root.0, mono(), &archive(None), None, "").is_err());
    assert!(!root.0.join("BepInEx").exists());
    assert!(!root.0.join("winhttp.dll").exists());
}

#[test]
fn bepinex_zip_symlinks_are_rejected_before_writes() {
    let root = Fixture::new();
    let mut writer = zip::ZipWriter::new(Cursor::new(Vec::new()));
    writer
        .add_symlink(
            "BepInEx/core/escape.dll",
            "/outside-target",
            SimpleFileOptions::default(),
        )
        .unwrap();
    let zip = writer.finish().unwrap().into_inner();
    assert!(install_archive(&root.0, mono(), &zip, None, "").is_err());
    assert!(std::fs::read_dir(&root.0).unwrap().next().is_none());
}

struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "bepinex-fixture-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(&path).unwrap();
        Self(path)
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn archive(extra: Option<&str>) -> Vec<u8> {
    let mut zip = zip::ZipWriter::new(Cursor::new(Vec::new()));
    for name in [
        "winhttp.dll",
        "doorstop_config.ini",
        "BepInEx/core/BepInEx.dll",
    ]
    .into_iter()
    .chain(extra)
    {
        zip.start_file(name, SimpleFileOptions::default()).unwrap();
        zip.write_all(b"fixture data, never executed").unwrap();
    }
    zip.finish().unwrap().into_inner()
}

fn mono() -> Target {
    Target {
        runtime: Runtime::Mono,
        arch: Arch::X64,
    }
}

#[test]
fn bepinex_override_combination_preserves_user_values() {
    assert_eq!(combine_overrides("dxgi=b").unwrap(), "dxgi=b;winhttp=n,b");
    assert_eq!(
        combine_overrides("winhttp=n,b;dxgi=b").unwrap(),
        "winhttp=n,b;dxgi=b"
    );
    assert_eq!(
        combine_overrides("winhttp,version=n,b").unwrap(),
        "winhttp,version=n,b"
    );
    assert!(combine_overrides("winhttp=b").is_err());
    assert!(combine_overrides("winhttp=n\nPATH=x").is_err());
}

#[test]
fn bepinex_safe_extraction_rejects_paths_before_any_writes() {
    for bad in [
        "../escape.dll",
        "/escape.dll",
        "C:/escape.dll",
        "BepInEx/../escape.dll",
        "BepInEx\\escape.dll",
        "BepInEx/plugins/user.dll",
    ] {
        let root = Fixture::new();
        assert!(
            install_archive(&root.0, mono(), &archive(Some(bad)), None, "").is_err(),
            "accepted {bad}"
        );
        assert!(
            std::fs::read_dir(&root.0).unwrap().next().is_none(),
            "partial files for {bad}"
        );
    }
}

#[test]
fn bepinex_manifest_uninstall_preserves_mods_config_and_foreign_files() {
    let root = Fixture::new();
    std::fs::create_dir_all(root.0.join("BepInEx/plugins")).unwrap();
    std::fs::create_dir_all(root.0.join("BepInEx/config")).unwrap();
    std::fs::write(root.0.join("BepInEx/plugins/user.dll"), b"mod").unwrap();
    std::fs::write(root.0.join("BepInEx/config/user.cfg"), b"config").unwrap();
    std::fs::write(root.0.join("game.exe"), b"game").unwrap();
    let installed = install_archive(&root.0, mono(), &archive(None), None, "dxgi=b").unwrap();
    assert!(root.0.join(MANIFEST).is_file());
    assert_eq!(installed.overrides, "dxgi=b;winhttp=n,b");
    let removed = disable(&root.0, &installed.overrides).unwrap();
    assert_eq!(removed.overrides, "dxgi=b");
    assert!(!root.0.join("winhttp.dll").exists());
    assert!(!root.0.join(MANIFEST).exists());
    for path in [
        "game.exe",
        "BepInEx/plugins/user.dll",
        "BepInEx/config/user.cfg",
    ] {
        assert!(root.0.join(path).is_file());
    }
}

#[test]
fn bepinex_uninstall_keeps_modified_files_and_overrides() {
    let root = Fixture::new();
    install_archive(&root.0, mono(), &archive(None), None, "").unwrap();
    std::fs::write(root.0.join("BepInEx/core/BepInEx.dll"), b"user replacement").unwrap();
    let change = disable(&root.0, "winhttp=n,b;dxgi=n").unwrap();
    assert_eq!(change.overrides, "winhttp=n,b;dxgi=n");
    assert_eq!(
        std::fs::read(root.0.join("BepInEx/core/BepInEx.dll")).unwrap(),
        b"user replacement"
    );
    assert!(change.warning.is_some());
}

#[test]
fn bepinex_checksum_and_existing_file_conflicts_make_no_changes() {
    let root = Fixture::new();
    assert!(install_archive(&root.0, mono(), &archive(None), Some(&"0".repeat(64)), "").is_err());
    assert!(std::fs::read_dir(&root.0).unwrap().next().is_none());
    std::fs::write(root.0.join("winhttp.dll"), b"existing loader").unwrap();
    assert!(install_archive(&root.0, mono(), &archive(None), None, "").is_err());
    assert_eq!(
        std::fs::read(root.0.join("winhttp.dll")).unwrap(),
        b"existing loader"
    );
    assert!(!root.0.join(MANIFEST).exists());
}

#[test]
fn bepinex_symlink_parent_never_writes_outside_game() {
    let root = Fixture::new();
    let outside = Fixture::new();
    std::os::unix::fs::symlink(&outside.0, root.0.join("BepInEx")).unwrap();
    assert!(install_archive(&root.0, mono(), &archive(None), None, "").is_err());
    assert!(std::fs::read_dir(&outside.0).unwrap().next().is_none());
    assert!(!root.0.join("winhttp.dll").exists());
}

#[test]
fn bepinex_detects_pe_architecture_and_unity_runtime() {
    for (machine, arch) in [(0x14cu16, Arch::X86), (0x8664u16, Arch::X64)] {
        let root = Fixture::new();
        let mut pe = vec![0; 256];
        pe[..2].copy_from_slice(b"MZ");
        pe[0x3c..0x40].copy_from_slice(&128u32.to_le_bytes());
        pe[128..132].copy_from_slice(b"PE\0\0");
        pe[132..134].copy_from_slice(&machine.to_le_bytes());
        std::fs::write(root.0.join("Game.exe"), pe).unwrap();
        std::fs::create_dir_all(root.0.join("Game_Data/Managed")).unwrap();
        std::fs::write(
            root.0.join("Game_Data/Managed/Assembly-CSharp.dll"),
            b"managed",
        )
        .unwrap();
        assert_eq!(
            detect(&root.0, std::path::Path::new("Game.exe")).unwrap(),
            Target {
                runtime: Runtime::Mono,
                arch
            }
        );
        std::fs::create_dir_all(root.0.join("Game_Data/il2cpp_data")).unwrap();
        std::fs::write(root.0.join("GameAssembly.dll"), b"il2cpp").unwrap();
        assert_eq!(
            detect(&root.0, std::path::Path::new("Game.exe"))
                .unwrap()
                .runtime,
            Runtime::Il2Cpp
        );
    }
}
