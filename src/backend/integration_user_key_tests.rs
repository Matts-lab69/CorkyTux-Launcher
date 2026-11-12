//! Runtime-only artwork credentials and private-file permissions.

use super::IntegrationManager;
use std::fs;
use std::os::unix::fs::{symlink, PermissionsExt};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

struct Fixture(PathBuf);

impl Fixture {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let path = std::env::temp_dir().join(format!(
            "corkytux-user-key-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o700)).unwrap();
        Self(path)
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn priority_uses_environment_file_and_saved_setting_only() {
    let fixture = Fixture::new();
    let path = fixture.0.join("key");
    fs::write(&path, " FILE_TEST_VALUE\n").unwrap();
    assert_eq!(
        IntegrationManager::sgdb_key_from_sources(
            Some(" ENV_TEST_VALUE "),
            Some(&path),
            "SETTING_TEST_VALUE",
            Some("SHARED_TEST_VALUE"),
        ),
        Some("ENV_TEST_VALUE".into())
    );
    assert_eq!(
        IntegrationManager::sgdb_key_from_sources(
            Some("  "),
            Some(&path),
            "SETTING_TEST_VALUE",
            Some("SHARED_TEST_VALUE"),
        ),
        Some("FILE_TEST_VALUE".into())
    );
    fs::write(&path, " \n").unwrap();
    assert_eq!(
        IntegrationManager::sgdb_key_from_sources(
            None,
            Some(&path),
            " SETTING_TEST_VALUE ",
            Some("SHARED_TEST_VALUE"),
        ),
        Some("SETTING_TEST_VALUE".into())
    );
    fs::remove_file(&path).unwrap();
    assert_eq!(
        IntegrationManager::sgdb_key_from_sources(None, Some(&path), "", Some("SHARED_TEST_VALUE")),
        None,
        "no user credential must resolve to no key"
    );
}

#[test]
fn private_file_permissions_are_hardened_before_reading() {
    let fixture = Fixture::new();
    let path = fixture.0.join("key");
    fs::write(&path, "PRIVATE_FILE_TEST_VALUE").unwrap();
    fs::set_permissions(&path, fs::Permissions::from_mode(0o664)).unwrap();
    assert_eq!(
        IntegrationManager::key_from_private_file(&path),
        Some("PRIVATE_FILE_TEST_VALUE".into())
    );
    assert_eq!(
        fs::metadata(&path).unwrap().permissions().mode() & 0o7777,
        0o600,
        "private credential permissions must be exactly 0600"
    );
}

#[test]
fn private_file_symlinks_are_not_read_or_modified() {
    let fixture = Fixture::new();
    let target = fixture.0.join("target");
    let link = fixture.0.join("key");
    fs::write(&target, "SYMLINK_TEST_VALUE").unwrap();
    fs::set_permissions(&target, fs::Permissions::from_mode(0o640)).unwrap();
    symlink(&target, &link).unwrap();
    assert_eq!(IntegrationManager::key_from_private_file(&link), None);
    assert_eq!(
        fs::metadata(&target).unwrap().permissions().mode() & 0o7777,
        0o640
    );
}

#[test]
fn production_sources_have_no_compile_time_credential_route() {
    let integration = include_str!("integration.rs");
    let redact = include_str!("redact.rs");
    let workflow = include_str!("../../.github/workflows/release.yml");
    let packaging = include_str!("../../release/build-release.sh");
    assert!(
        !integration.contains("fn builtin_sgdb_key("),
        "the compiled credential resolver must be absent"
    );
    let legacy = concat!("CORKYTUX_SGDB_", "BUILTIN_KEY");
    for source in [integration, redact] {
        assert!(!source.contains(&format!("option_env!(\"{legacy}\")")));
        assert!(!source.contains(&format!("env!(\"{legacy}\")")));
    }
    assert!(!workflow.contains(legacy));
    assert!(!packaging.contains(legacy));
}
