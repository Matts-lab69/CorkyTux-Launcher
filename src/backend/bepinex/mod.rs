//! Opt-in BepInEx deployment; downloaded payloads are never executed here.

mod fs;
mod package;

use std::collections::{HashMap, HashSet};
use std::path::Path;

use serde::{Deserialize, Serialize};

pub const MANIFEST: &str = ".corkytux-bepinex.json";
const MAX_MANIFEST: u64 = 1024 * 1024;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Runtime {
    Mono,
    Il2Cpp,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Arch {
    X86,
    X64,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Target {
    pub runtime: Runtime,
    pub arch: Arch,
}

impl Target {
    pub fn installation_available(self) -> bool {
        package::select(self).checksum.is_some()
    }

    pub fn description(self) -> String {
        let package = package::select(self);
        if package.checksum.is_some() {
            format!("{}: published SHA256 verification required.", package.name)
        } else {
            format!(
                "{}: installation disabled until an official SHA256 is available.",
                package.name
            )
        }
    }
}

pub struct Inspection {
    pub target: Target,
    pub managed: bool,
    pub external: bool,
}
pub struct Change {
    pub overrides: String,
    pub warning: Option<String>,
}

#[derive(Clone, Copy)]
pub struct Authorization {
    pub target: Target,
    pub allow_unverified: bool,
}

fn authorize(actual: Target, authorization: Authorization) -> Result<(), String> {
    if actual != authorization.target {
        return Err("Game runtime or architecture changed; reopen settings to review the correct BepInEx package".into());
    }
    if package::select(actual).checksum.is_none() && !authorization.allow_unverified {
        return Err(
            "This package has no published SHA256 and requires explicit confirmation".into(),
        );
    }
    Ok(())
}

pub fn override_key(game: &HashMap<String, String>) -> &'static str {
    if game.contains_key("custom_dll_overrides") {
        "custom_dll_overrides"
    } else {
        "Overrides"
    }
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct OwnedFile {
    path: String,
    sha256: String,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Manifest {
    schema: u32,
    root: (u64, u64),
    target: Target,
    source: String,
    archive_sha256: String,
    checksum_verified: bool,
    ready: bool,
    files: Vec<OwnedFile>,
    directories: Vec<String>,
    overrides_before: String,
    overrides_after: String,
}

pub fn combine_overrides(current: &str) -> Result<String, String> {
    if current.len() > 16 * 1024 || current.chars().any(char::is_control) {
        return Err(
            "Edit the DLL overrides before enabling BepInEx: expected one bounded line".into(),
        );
    }
    let mut found = false;
    for clause in current
        .split(';')
        .map(str::trim)
        .filter(|clause| !clause.is_empty())
    {
        let (names, order) = clause
            .split_once('=')
            .ok_or("Edit the DLL overrides: expected DLL=load-order groups")?;
        if names.split(',').any(|name| {
            let name = name.trim();
            name.is_empty()
                || matches!(name, "." | "..")
                || !name
                    .chars()
                    .all(|c| c.is_alphanumeric() || matches!(c, '_' | '-' | '.' | '*'))
        }) || (!order.trim().is_empty()
            && order.split(',').any(|mode| {
                !matches!(
                    mode.trim().to_ascii_lowercase().as_str(),
                    "n" | "b" | "native" | "builtin"
                )
            }))
        {
            return Err("DLL overrides must contain DLL names and native/builtin load orders, without paths or empty list items".into());
        }
        if names.split(',').any(|name| {
            let name = name.trim().to_ascii_lowercase();
            matches!(name.trim_start_matches('*'), "winhttp" | "winhttp.dll")
        }) {
            if !matches!(
                order
                    .split(',')
                    .next()
                    .unwrap_or("")
                    .trim()
                    .to_ascii_lowercase()
                    .as_str(),
                "n" | "native"
            ) {
                return Err("Existing winhttp overrides do not prefer native loading; edit them before enabling BepInEx".into());
            }
            found = true;
        }
    }
    let merged = if found {
        current.to_string()
    } else if current.is_empty() {
        "winhttp=n,b".into()
    } else {
        format!("{current};winhttp=n,b")
    };
    if merged.len() > 16 * 1024 {
        return Err("Merged DLL overrides exceed 16384 bytes".into());
    }
    Ok(merged)
}

/// Apply opt-in activation to all launcher-controlled Wine routes.
pub fn configure_launch(
    command: &mut std::process::Command,
    game: &HashMap<String, String>,
) -> Result<(), String> {
    if game
        .get("bepinexenabled")
        .is_some_and(|value| value == "true")
    {
        let value = game
            .get(&override_key(game).to_ascii_lowercase())
            .map(String::as_str)
            .unwrap_or("");
        command.env("WINEDLLOVERRIDES", combine_overrides(value)?);
    }
    Ok(())
}

pub fn detect(game_dir: &Path, executable: &Path) -> Result<Target, String> {
    let root = fs::Root::open(game_dir)?;
    let relative = if executable.is_absolute() {
        executable
            .strip_prefix(game_dir)
            .map_err(|_| "The executable must be inside the game directory")?
    } else {
        executable
    };
    let relative = relative
        .to_str()
        .ok_or("The executable path must be UTF-8")?;
    fs::parts(relative)?;
    let header = root
        .prefix(relative, 1024 * 1024)?
        .ok_or("Game executable not found")?;
    if header.get(..2) != Some(b"MZ") {
        return Err(
            "Automatic BepInEx installation supports Windows Unity games under Proton/Wine only"
                .into(),
        );
    }
    let pointer = header.get(0x3c..0x40).ok_or("Truncated PE header")?;
    let offset = u32::from_le_bytes(pointer.try_into().map_err(|_| "Invalid PE header")?) as usize;
    let pe = header
        .get(offset..offset.checked_add(6).ok_or("Invalid PE offset")?)
        .ok_or("Invalid PE offset")?;
    if pe.get(..4) != Some(b"PE\0\0") {
        return Err("Invalid Windows executable signature".into());
    }
    let arch = match u16::from_le_bytes([pe[4], pe[5]]) {
        0x14c => Arch::X86,
        0x8664 => Arch::X64,
        _ => return Err("Unsupported Windows executable architecture".into()),
    };
    let executable = Path::new(relative);
    let stem = executable
        .file_stem()
        .and_then(|s| s.to_str())
        .ok_or("Missing game executable name")?;
    let parent = executable.parent().filter(|p| !p.as_os_str().is_empty());
    if parent.is_some() {
        return Err(
            "Select the Unity executable in the game directory, not a launcher in a subdirectory"
                .into(),
        );
    }
    let data = format!("{stem}_Data");
    let runtime = if root.prefix("GameAssembly.dll", 1)?.is_some()
        && root.is_directory(&format!("{data}/il2cpp_data"))?
    {
        Runtime::Il2Cpp
    } else if root.is_directory(&format!("{data}/Managed"))? {
        Runtime::Mono
    } else {
        return Err(
            "Unity Mono/IL2CPP runtime could not be identified; no files were changed".into(),
        );
    };
    Ok(Target { runtime, arch })
}

fn load_manifest(root: &fs::Root) -> Result<Option<Manifest>, String> {
    let Some(bytes) = root.read(MANIFEST, MAX_MANIFEST)? else {
        return Ok(None);
    };
    let manifest: Manifest = serde_json::from_slice(&bytes)
        .map_err(|_| "The BepInEx ownership manifest is invalid; no files were removed")?;
    if manifest.schema != 1
        || manifest.root != root.identity()?
        || manifest.files.len() > package::MAX_ENTRIES
        || manifest.directories.len() > package::MAX_ENTRIES * 32
        || manifest.overrides_before.len() > 16 * 1024
        || manifest.overrides_after.len() > 16 * 1024
    {
        return Err("BepInEx manifest does not match this game or exceeds its limits".into());
    }
    if combine_overrides(&manifest.overrides_before)? != manifest.overrides_after {
        return Err("BepInEx manifest has an invalid DLL override ownership record".into());
    }
    let mut seen = HashSet::new();
    for file in &manifest.files {
        fs::parts(&file.path)?;
        if !package::owned_path(&file.path)
            || !seen.insert(file.path.to_lowercase())
            || file.sha256.len() != 64
            || !file.sha256.bytes().all(|c| c.is_ascii_hexdigit())
        {
            return Err("BepInEx manifest contains an unsafe ownership entry".into());
        }
    }
    for directory in &manifest.directories {
        fs::parts(directory)?;
        if !package::owned_directory(directory) {
            return Err("BepInEx manifest refers to an unmanaged directory".into());
        }
    }
    Ok(Some(manifest))
}

fn save_manifest(root: &fs::Root, manifest: &Manifest, new: bool) -> Result<(), String> {
    let bytes = serde_json::to_vec(manifest).map_err(|e| e.to_string())?;
    if bytes.len() as u64 > MAX_MANIFEST {
        return Err("BepInEx ownership manifest exceeds 1 MiB".into());
    }
    if new {
        root.create(MANIFEST, &bytes, &mut Vec::new())
    } else {
        root.update(MANIFEST, &bytes)
    }
}

fn external_loader(root: &fs::Root) -> Result<bool, String> {
    Ok(root.prefix("winhttp.dll", 1)?.is_some()
        && (root.prefix("BepInEx/core/BepInEx.dll", 1)?.is_some()
            || root
                .prefix("BepInEx/core/BepInEx.Unity.IL2CPP.dll", 1)?
                .is_some()))
}

pub fn inspect(game_dir: &Path, executable: &Path) -> Result<Inspection, String> {
    let root = fs::Root::open(game_dir)?;
    if let Some(manifest) = load_manifest(&root)? {
        return Ok(Inspection {
            target: manifest.target,
            managed: true,
            external: false,
        });
    }
    let target = detect(game_dir, executable)?;
    Ok(Inspection {
        target,
        managed: false,
        external: external_loader(&root)?,
    })
}

/// Refresh only activation metadata; the GTK caller supplies its latest value.
pub fn rebase_overrides(game_dir: &Path, current: &str) -> Result<String, String> {
    let root = fs::Root::open(game_dir)?;
    root.lock()?;
    let mut manifest = load_manifest(&root)?.ok_or("Managed BepInEx installation not found")?;
    if !manifest.ready {
        return Err(
            "Incomplete BepInEx installation; turn Mods off to remove its managed files first"
                .into(),
        );
    }
    if current != manifest.overrides_after {
        manifest.overrides_after = combine_overrides(current)?;
        manifest.overrides_before = current.to_string();
        save_manifest(&root, &manifest, false)?;
    }
    Ok(manifest.overrides_after)
}

pub fn enable(
    game_dir: &Path,
    executable: &Path,
    current: &str,
    authorization: Authorization,
) -> Result<Change, String> {
    enable_with_fetch(
        game_dir,
        executable,
        current,
        authorization,
        package::download,
    )
}

fn enable_with_fetch(
    game_dir: &Path,
    executable: &Path,
    current: &str,
    authorization: Authorization,
    fetch: impl FnOnce(&package::Package) -> Result<Vec<u8>, String>,
) -> Result<Change, String> {
    let target = detect(game_dir, executable)?;
    if !target.installation_available() {
        return Err(
            "BepInEx installation is disabled until an official SHA256 is available".into(),
        );
    }
    authorize(target, authorization)?;
    let root = fs::Root::open(game_dir)?;
    root.lock()?;
    let merged = combine_overrides(current)?;
    if let Some(mut manifest) = load_manifest(&root)? {
        if manifest.target != target {
            return Err("The Unity runtime or architecture changed; disable the managed installation before reinstalling".into());
        }
        if !manifest.ready {
            return Err("Incomplete BepInEx installation; turn Mods off before retrying".into());
        }
        if !external_loader(&root)? {
            return Err(
                "Managed BepInEx loader files are missing; disable it before reinstalling".into(),
            );
        }
        if current != manifest.overrides_after {
            manifest.overrides_before = current.to_string();
            manifest.overrides_after = merged;
            save_manifest(&root, &manifest, false)?;
        }
        return Ok(Change {
            overrides: manifest.overrides_after,
            warning: (!manifest.checksum_verified)
                .then(|| "Existing installation has no verified published SHA256.".into()),
        });
    }
    if external_loader(&root)? {
        let manifest = Manifest {
            schema: 1,
            root: root.identity()?,
            target,
            source: "external".into(),
            archive_sha256: String::new(),
            checksum_verified: false,
            ready: true,
            files: Vec::new(),
            directories: Vec::new(),
            overrides_before: current.into(),
            overrides_after: merged.clone(),
        };
        save_manifest(&root, &manifest, true)?;
        return Ok(Change { overrides: merged, warning: Some("Existing BepInEx detected. Its files are unchanged and will not be uninstalled by CorkyTux.".into()) });
    }
    let package = package::select(target);
    let bytes = fetch(&package)?;
    if root.identity()? != fs::Root::open(game_dir)?.identity()? {
        return Err("The game directory changed during download; no package was installed".into());
    }
    authorize(detect(game_dir, executable)?, authorization)?;
    install_root(&root, target, &bytes, package.checksum, current)
}

#[cfg(test)]
fn install_archive(
    game_dir: &Path,
    target: Target,
    bytes: &[u8],
    checksum: Option<&str>,
    current: &str,
) -> Result<Change, String> {
    let root = fs::Root::open(game_dir)?;
    root.lock()?;
    install_root(&root, target, bytes, checksum, current)
}

fn install_root(
    root: &fs::Root,
    target: Target,
    bytes: &[u8],
    checksum: Option<&str>,
    current: &str,
) -> Result<Change, String> {
    let merged = combine_overrides(current)?;
    let files = package::unpack(bytes, checksum, target)?;
    root.vacant(MANIFEST)?;
    for file in &files {
        root.vacant(&file.path)?;
    }
    let spec = package::select(target);
    let mut manifest = Manifest {
        schema: 1,
        root: root.identity()?,
        target,
        source: format!(
            "https://github.com/BepInEx/BepInEx/releases/download/{}/{}",
            spec.tag, spec.name
        ),
        archive_sha256: package::digest(bytes),
        checksum_verified: checksum.is_some(),
        ready: false,
        files: files
            .iter()
            .map(|file| OwnedFile {
                path: file.path.clone(),
                sha256: package::digest(&file.bytes),
            })
            .collect(),
        directories: Vec::new(),
        overrides_before: current.into(),
        overrides_after: merged.clone(),
    };
    save_manifest(root, &manifest, true)?;
    let mut written = Vec::new();
    let result = (|| {
        for (file, owned) in files.iter().zip(&manifest.files) {
            root.create(&file.path, &file.bytes, &mut manifest.directories)?;
            written.push(owned.clone());
        }
        manifest.ready = true;
        save_manifest(root, &manifest, false)
    })();
    if let Err(error) = result {
        for file in written.into_iter().rev() {
            let _ = remove_unchanged(root, &file);
        }
        for path in manifest.directories.iter().rev() {
            root.remove_empty_directory(path);
        }
        let _ = root.remove(MANIFEST);
        return Err(error);
    }
    Ok(Change { overrides: merged, warning: checksum.is_none().then(|| "No published SHA256: downloaded only over official GitHub HTTPS. IL2CPP is experimental.".into()) })
}

fn remove_unchanged(root: &fs::Root, file: &OwnedFile) -> Result<bool, String> {
    match root.capture(&file.path)? {
        Some(captured) => captured.remove_if_matches(&file.sha256, package::MAX_FILE),
        None => Ok(true),
    }
}

pub fn disable(game_dir: &Path, current: &str) -> Result<Change, String> {
    let root = fs::Root::open(game_dir)?;
    root.lock()?;
    let Some(manifest) = load_manifest(&root)? else {
        return Ok(Change {
            overrides: current.into(),
            warning: None,
        });
    };
    let mut unchanged = Vec::new();
    let mut preserved = false;
    for file in &manifest.files {
        match root.read(&file.path, package::MAX_FILE) {
            Ok(Some(bytes)) if package::digest(&bytes) == file.sha256 => unchanged.push(file),
            Ok(None) => {}
            _ => preserved = true,
        }
    }
    for file in unchanged {
        // Recheck immediately before unlinking; modified files are user-owned.
        if !remove_unchanged(&root, file)? {
            preserved = true;
        }
    }
    for directory in manifest.directories.iter().rev() {
        root.remove_empty_directory(directory);
    }
    root.remove(MANIFEST)?;
    let overrides = if current == manifest.overrides_after {
        manifest.overrides_before
    } else {
        preserved = true;
        current.into()
    };
    Ok(Change { overrides, warning: preserved.then(|| "Modified files or DLL overrides were retained. User mods, plugins and configuration were not removed.".into()) })
}

#[cfg(test)]
mod tests;
