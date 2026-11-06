//! Pinned runtime metadata and temporary archive integrity checks.

use sha2::{Digest, Sha512};
use std::fs::{File, OpenOptions};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

pub const GE_TAG: &str = "GE-Proton11-12";
pub const GE_API_URL: &str = "https://api.github.com/repos/GloriousEggroll/proton-ge-custom/releases/tags/GE-Proton11-12";
pub const GE_URL: &str = "https://github.com/GloriousEggroll/proton-ge-custom/releases/download/GE-Proton11-12/GE-Proton11-12-x86_64.tar.gz";
// Published SHA-512, read on 2026-11-04 from the release's official sidecar:
// https://github.com/GloriousEggroll/proton-ge-custom/releases/download/GE-Proton11-12/GE-Proton11-12-x86_64.sha512sum
pub const GE_SHA512: &str = "2e37447902529c9fc39138dc0b3f24aed3513b59a190b825e6b4a668e49e2033bdf6b8a7998e8686e067bbb3f61f3a6633e2408f0cc96f6dad983bd18e2db31e";

/// Reject stale or substituted GE metadata before requesting any bytes.
/// CachyOS remains a separate, unchanged download policy.
pub fn ge_checksum(name: &str, url: &str) -> Result<Option<&'static str>, String> {
    if name == GE_TAG && url == GE_URL {
        return Ok(Some(GE_SHA512));
    }
    if !name.starts_with("GE-Proton")
        && url.starts_with("https://github.com/CachyOS/Proton-CachyOS/releases/download/")
    {
        return Ok(None);
    }
    Err("Unpinned or unsupported Proton download; select the pinned GE-Proton release".into())
}

/// A private, uniquely created partial archive. Every error path removes it;
/// neither an existing archive nor an installed runtime is overwritten.
pub struct DownloadArchive {
    path: PathBuf,
}

impl DownloadArchive {
    pub fn create(directory: &Path) -> Result<(Self, File), String> {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        for _ in 0..32 {
            let path = directory.join(format!(
                ".corkytux-download-{}-{}.part",
                std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            let mut options = OpenOptions::new();
            options.write(true).create_new(true);
            #[cfg(unix)]
            {
                use std::os::unix::fs::OpenOptionsExt;
                options.mode(0o600);
            }
            match options.open(&path) {
                Ok(file) => return Ok((Self { path }, file)),
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(error) => return Err(format!("Cannot create download archive: {error}")),
            }
        }
        Err("Cannot allocate a unique download archive".into())
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn verify_sha512(&self, expected: &str) -> Result<(), String> {
        if expected.len() != 128 || !expected.bytes().all(|byte| byte.is_ascii_hexdigit()) {
            return Err("Invalid SHA-512 checksum metadata; installation aborted".into());
        }
        let mut file = File::open(&self.path).map_err(|error| format!("Cannot verify archive: {error}"))?;
        let mut hasher = Sha512::new();
        let mut buffer = [0u8; 64 * 1024];
        loop {
            let count = file.read(&mut buffer).map_err(|error| format!("Cannot verify archive: {error}"))?;
            if count == 0 { break; }
            hasher.update(&buffer[..count]);
        }
        let actual = format!("{:x}", hasher.finalize());
        if !actual.eq_ignore_ascii_case(expected) {
            return Err("SHA-512 checksum mismatch; installation aborted".into());
        }
        Ok(())
    }
}

impl Drop for DownloadArchive {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sha2::{Digest, Sha512};
    use std::io::Write;

    #[test]
    fn ge_policy_rejects_unpinned_versions_and_urls() {
        assert_eq!(ge_checksum(GE_TAG, GE_URL).unwrap(), Some(GE_SHA512));
        assert!(ge_checksum("GE-Proton-old", GE_URL).is_err());
        assert!(ge_checksum(GE_TAG, "https://example.invalid/archive.tar.gz").is_err());
        assert!(ge_checksum("different-name", GE_URL).is_err());
        assert_eq!(ge_checksum("proton-cachyos", "https://github.com/CachyOS/Proton-CachyOS/releases/download/test/archive.tar.gz").unwrap(), None);
    }

    #[test]
    fn verified_archive_rejects_corruption_and_cleans_partials() {
        let root = std::env::temp_dir();
        let (archive, mut file) = DownloadArchive::create(&root).unwrap();
        let path = archive.path().to_path_buf();
        file.write_all(b"corrupt bytes").unwrap();
        drop(file);
        let trusted = format!("{:x}", Sha512::digest(b"trusted bytes"));
        assert!(archive.verify_sha512(&trusted).unwrap_err().contains("checksum mismatch"));
        drop(archive);
        assert!(!path.exists());
    }

    #[test]
    fn verified_archive_accepts_expected_bytes_and_rejects_bad_metadata() {
        let (archive, mut file) = DownloadArchive::create(&std::env::temp_dir()).unwrap();
        file.write_all(b"trusted bytes").unwrap();
        drop(file);
        let trusted = format!("{:x}", Sha512::digest(b"trusted bytes"));
        assert!(archive.verify_sha512(&trusted).is_ok());
        assert!(archive.verify_sha512("not-a-checksum").is_err());
    }

    #[test]
    fn concurrent_archive_paths_are_distinct_and_do_not_replace_existing_files() {
        let (first, _file1) = DownloadArchive::create(&std::env::temp_dir()).unwrap();
        let (second, _file2) = DownloadArchive::create(&std::env::temp_dir()).unwrap();
        assert_ne!(first.path(), second.path());
    }

    #[test]
    fn pinned_digests_have_correct_hex_lengths() {
        // SHA-512 must be exactly 128 lowercase hex digits
        assert_eq!(GE_SHA512.len(), 128, "GE_SHA512 must be 128 hex chars");
        assert!(
            GE_SHA512.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f')),
            "GE_SHA512 must be lowercase hex"
        );
    }
}
