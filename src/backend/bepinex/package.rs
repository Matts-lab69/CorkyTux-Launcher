use std::collections::HashSet;
use std::io::{Cursor, Read};

use sha2::{Digest, Sha256};

use super::{Arch, Runtime, Target};

pub(super) const MAX_DOWNLOAD: u64 = 128 * 1024 * 1024;
pub(super) const MAX_FILE: u64 = 64 * 1024 * 1024;
const MAX_EXPANDED: u64 = 256 * 1024 * 1024;
pub(super) const MAX_ENTRIES: usize = 2048;

pub(super) struct Package {
    pub tag: &'static str,
    pub name: String,
    pub checksum: Option<&'static str>,
}

pub(super) fn select(target: Target) -> Package {
    let arch = match target.arch {
        Arch::X86 => "x86",
        Arch::X64 => "x64",
    };
    match target.runtime {
        Runtime::Mono => Package {
            tag: "v5.4.23.6",
            name: format!("BepInEx_win_{arch}_5.4.23.6.zip"),
            // Official GitHub asset SHA256 digests for v5.4.23.6.
            // https://api.github.com/repos/BepInEx/BepInEx/releases/tags/v5.4.23.6
            checksum: Some(match target.arch {
                Arch::X64 => "6c57a714a892efb4e9e1fef63d608a3391f30ec984031c465404e4ee381a0755",
                Arch::X86 => "a4024dafaca5d3784097e34b3c2921415030c0b461e52f30053ed8914be9622d",
            }),
        },
        Runtime::Il2Cpp => Package {
            tag: "v6.0.0-pre.2",
            name: format!("BepInEx-Unity.IL2CPP-win-{arch}-6.0.0-pre.2.zip"),
            checksum: None,
        },
    }
}

pub(super) fn digest(data: &[u8]) -> String {
    format!("{:x}", Sha256::digest(data))
}

pub(super) fn owned_path(path: &str) -> bool {
    matches!(
        path,
        "winhttp.dll" | "doorstop_config.ini" | ".doorstop_version"
    ) || path.starts_with("BepInEx/core/")
        || path.starts_with("dotnet/")
}

pub(super) fn owned_directory(path: &str) -> bool {
    matches!(path, "BepInEx" | "BepInEx/core" | "dotnet")
        || path.starts_with("BepInEx/core/")
        || path.starts_with("dotnet/")
}

pub(super) struct Payload {
    pub path: String,
    pub bytes: Vec<u8>,
}

pub(super) fn unpack(
    bytes: &[u8],
    expected: Option<&str>,
    target: Target,
) -> Result<Vec<Payload>, String> {
    if bytes.len() as u64 > MAX_DOWNLOAD {
        return Err("BepInEx download exceeds 128 MiB".into());
    }
    if let Some(expected) = expected {
        if digest(bytes) != expected.to_ascii_lowercase() {
            return Err("BepInEx SHA256 does not match the published checksum".into());
        }
    }
    let mut archive = zip::ZipArchive::new(Cursor::new(bytes)).map_err(|e| e.to_string())?;
    if archive.len() > MAX_ENTRIES {
        return Err("BepInEx archive has too many entries".into());
    }
    let mut seen = HashSet::new();
    let mut total = 0u64;
    let mut files = Vec::new();
    for index in 0..archive.len() {
        let mut file = archive.by_index(index).map_err(|e| e.to_string())?;
        let path = file.name().trim_end_matches('/').to_string();
        super::fs::parts(&path)?;
        if !seen.insert(path.to_lowercase()) {
            return Err("Duplicate or case-colliding archive paths".into());
        }
        let kind = file.unix_mode().unwrap_or(0) & 0o170000;
        if !matches!(kind, 0 | 0o100000 | 0o040000) {
            return Err("Archive links and special files are not allowed".into());
        }
        if file.is_dir() {
            continue;
        }
        if matches!(path.as_str(), "changelog.txt" | "README.md" | "LICENSE") {
            continue;
        }
        if !owned_path(&path) {
            return Err(
                "Archive contains files outside the BepInEx loader/core directories".into(),
            );
        }
        if file.size() > MAX_FILE {
            return Err("BepInEx archive member exceeds 64 MiB".into());
        }
        let mut data = Vec::new();
        (&mut file)
            .take(MAX_FILE + 1)
            .read_to_end(&mut data)
            .map_err(|e| e.to_string())?;
        if data.len() as u64 > MAX_FILE {
            return Err("Expanded archive member exceeds 64 MiB".into());
        }
        total = total
            .checked_add(data.len() as u64)
            .ok_or("Archive size overflow")?;
        if total > MAX_EXPANDED {
            return Err("Expanded BepInEx archive exceeds 256 MiB".into());
        }
        files.push(Payload { path, bytes: data });
    }
    let core = match target.runtime {
        Runtime::Mono => "BepInEx/core/BepInEx.dll",
        Runtime::Il2Cpp => "BepInEx/core/BepInEx.Unity.IL2CPP.dll",
    };
    for required in ["winhttp.dll", "doorstop_config.ini", core] {
        if !files.iter().any(|file| file.path == required) {
            return Err("BepInEx archive is missing a required loader/core file".into());
        }
    }
    Ok(files)
}

pub(super) fn download(package: &Package) -> Result<Vec<u8>, String> {
    let url = format!(
        "https://github.com/BepInEx/BepInEx/releases/download/{}/{}",
        package.tag, package.name
    );
    let client = reqwest::blocking::Client::builder()
        .user_agent("CorkyTux-BepInEx")
        .https_only(true)
        .timeout(std::time::Duration::from_secs(120))
        .redirect(reqwest::redirect::Policy::custom(|attempt| {
            let allowed = attempt.url().scheme() == "https"
                && matches!(
                    attempt.url().host_str(),
                    Some(
                        "github.com"
                            | "release-assets.githubusercontent.com"
                            | "objects.githubusercontent.com"
                    )
                );
            if allowed && attempt.previous().len() < 5 {
                attempt.follow()
            } else {
                attempt.error("BepInEx redirects must remain on official HTTPS download hosts")
            }
        }))
        .build()
        .map_err(|e| e.to_string())?;
    let response = client
        .get(url)
        .send()
        .and_then(|r| r.error_for_status())
        .map_err(|e| e.to_string())?;
    if response
        .content_length()
        .is_some_and(|size| size > MAX_DOWNLOAD)
    {
        return Err("BepInEx download exceeds 128 MiB".into());
    }
    let mut bytes = Vec::new();
    response
        .take(MAX_DOWNLOAD + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    if bytes.len() as u64 > MAX_DOWNLOAD {
        return Err("BepInEx download exceeds 128 MiB".into());
    }
    Ok(bytes)
}
