//! Self-update from GitHub Releases.
//!
//! Trust model: GitHub + HTTPS. Artifacts are verified by SHA256
//! against the digest published by the release API (and against
//! `checksums.txt` when the release carries one). There are no
//! artifact signatures yet: `verify_artifact` is the single seam
//! where minisign/GPG verification plugs in later without changing
//! the download/install flow.
//!
//! Safety rules enforced here, not in the UI:
//! - HTTPS only, redirects followed solely to an allowlist of hosts.
//! - No install when the published version is <= the running one.
//! - Tarball entries are allowlisted: no absolute paths, no `..`,
//!   no symlinks/hardlinks, no unexpected files.
//! - The new binary must answer `--version` with the tagged version
//!   before anything installed is touched.
//! - Installed files are replaced atomically with timestamped backups.

use std::io::Read;
use std::path::{Component, Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

const API_LATEST_URL: &str =
    "https://api.github.com/repos/Matts-lab69/CorkyTux-Launcher/releases/latest";
/// Test-only API override (loopback servers in tests). Release builds
/// always use API_LATEST_URL; there is no runtime knob.
fn api_latest_url() -> String {
    #[cfg(test)]
    if let Some(u) = test_api_override() {
        return u;
    }
    API_LATEST_URL.to_string()
}
const USER_AGENT: &str = concat!("CorkyTux/", env!("CARGO_PKG_VERSION"));
/// Closed redirect target list, evidenced 2026-10-14: release assets
/// redirect github.com -> release-assets.githubusercontent.com.
/// api.github.com is allowed ONLY for the release-metadata query,
/// never for artifact downloads (see API_HOSTS below).
const ASSET_HOSTS: &[&str] = &[
    "github.com",
    "objects.githubusercontent.com",
    "release-assets.githubusercontent.com",
];
const API_HOSTS: &[&str] = &["api.github.com", "github.com"];
const MAX_REDIRECTS: u32 = 5;
const CONNECT_TIMEOUT: Duration = Duration::from_secs(15);
/// Total cap for one request including the body stream. Honest note:
/// the 30 s inactivity guard below is evaluated between reads only;
/// a single blocked read is cut by THIS timeout (reqwest exposes no
/// per-read timeout on the blocking client), never by the inactivity
/// counter. Stall detection therefore means "no chunk completed in
/// 30 s while reads keep returning", and the hard ceiling is 3 min.
const TOTAL_TIMEOUT: Duration = Duration::from_secs(180);
const INACTIVITY_TIMEOUT: Duration = Duration::from_secs(30);
const HEALTH_TIMEOUT: Duration = Duration::from_secs(5);
const AUTO_CHECK_INTERVAL: Duration = Duration::from_secs(6 * 3600);
/// Hard ceiling for a single downloaded file. Applied even when the
/// API omits the size: the space check and the byte cutoff both use
/// `download_cap`, which never exceeds this.
pub const MAX_DOWNLOAD_BYTES: u64 = 256 * 1024 * 1024;
/// Hard ceiling for the optional `checksums.txt`. It is a small text
/// file, so its download, its on-disk read and the space check share
/// this cap instead of the much larger `MAX_DOWNLOAD_BYTES`.
pub const CHECKSUMS_MAX_BYTES: u64 = 1024 * 1024;
/// Backups retained per target after a fully successful install: the
/// oldest (the pre-update original, so even a bad first install can be
/// walked back) plus the `BACKUP_KEEP_NEWEST` most recent. Pruning runs
/// only after an install lands, so a failed install never deletes one.
const BACKUP_KEEP_NEWEST: usize = 2;
/// Backup-name prefix for the aside-renamed assets dir.
const ASSETS_BAK_PREFIX: &str = "assets.bak-";

/// Age threshold used by the worker when sweeping updater leftovers
/// (see `sweep_stale_downloads`). Shared so the test and the caller
/// cannot drift apart.
pub const SWEEP_MAX_AGE_SECS: u64 = 3600;

/// Partial-download name for an asset: `<asset>.part`.
pub fn partial_name(asset_name: &str) -> String {
    format!("{}.part", asset_name)
}

/// Staging-directory name for a release tag: `staging-<tag>`.
pub fn staging_name(tag: &str) -> String {
    format!("staging-{}", tag)
}

/// Unique tmp name used by `store_cached_latest` (pid + counter so
/// concurrent writers never share a name).
pub fn cached_latest_tmp_name(pid: u32, seq: u64) -> String {
    format!("{}.{}.{}.tmp", CACHED_LATEST_FILE, pid, seq)
}

/// Unique aside name for the assets dir: `assets.bak-<secs>-<counter>`.
/// The counter keeps two installs in the same second from colliding.
pub fn assets_backup_name(secs: u64, seq: u64) -> String {
    format!("{}{}-{:06}", ASSETS_BAK_PREFIX, secs, seq)
}

/// True when a downloads-dir entry is an updater leftover that may be
/// swept: `*.part`, a `staging-*` directory, or `latest.json.*.tmp`.
/// The verified tarball and `latest.json` itself never match.
pub fn is_sweepable_leftover(name: &str, is_dir: bool) -> bool {
    name.ends_with(".part")
        || (name.starts_with("staging-") && is_dir)
        || (name.starts_with("latest.json.") && name.ends_with(".tmp"))
}

/// A parsed `releases/latest` response.
#[derive(Clone, Debug)]
pub struct ReleaseInfo {
    pub tag: String,
    pub version: semver::Version,
    pub name: String,
    pub notes: String,
    pub assets: Vec<AssetInfo>,
}

/// One release asset we care about.
#[derive(Clone, Debug)]
pub struct AssetInfo {
    pub name: String,
    pub browser_download_url: String,
    pub digest_sha256: Option<String>,
    pub size: u64,
}

/// Strict `vX.Y.Z` tag, no prerelease, no build metadata, no `qt-*`.
/// Rejects anything `semver` would otherwise accept loosely.
pub fn parse_tag_version(tag: &str) -> Result<semver::Version, String> {
    let rest = tag.strip_prefix('v').ok_or_else(|| format!("bad tag (need vX.Y.Z): {}", tag))?;
    let parts: Vec<&str> = rest.split('.').collect();
    if parts.len() != 3 || parts.iter().any(|p| p.is_empty() || !p.bytes().all(|b| b.is_ascii_digit())) {
        return Err(format!("bad tag (need vX.Y.Z): {}", tag));
    }
    semver::Version::parse(rest).map_err(|e| format!("bad tag {}: {}", tag, e))
}

/// Running version, from the same source as the About page.
pub fn current_version() -> Result<semver::Version, String> {
    semver::Version::parse(env!("CARGO_PKG_VERSION"))
        .map_err(|e| format!("bad Cargo version: {}", e))
}

/// This machine's tarball arch (`uname -m` naming).
pub fn tarball_arch() -> String {
    std::env::consts::ARCH.to_string()
}

/// Exact asset name a release must carry for this machine.
pub fn expected_asset_name(version: &semver::Version) -> String {
    format!("corkytux-{}-linux-{}.tar.gz", version, tarball_arch())
}

/// Parse `releases/latest` JSON. Drafts and prereleases are refused
/// defensively (the endpoint never returns them, but be explicit).
pub fn parse_latest_release(json: &str) -> Result<ReleaseInfo, String> {
    let v: serde_json::Value =
        serde_json::from_str(json).map_err(|e| format!("bad release JSON: {}", e))?;
    if v.get("draft").and_then(|d| d.as_bool()).unwrap_or(false) {
        return Err("release is a draft".into());
    }
    if v.get("prerelease").and_then(|d| d.as_bool()).unwrap_or(false) {
        return Err("release is a prerelease".into());
    }
    let tag = v
        .get("tag_name")
        .and_then(|t| t.as_str())
        .ok_or("release has no tag_name")?;
    let version = parse_tag_version(tag)?;
    let mut assets = Vec::new();
    let list = v.get("assets").and_then(|a| a.as_array()).cloned().unwrap_or_default();
    for a in &list {
        let name = a.get("name").and_then(|n| n.as_str()).unwrap_or("").to_string();
        if name.is_empty() {
            continue;
        }
        let url = a
            .get("browser_download_url")
            .and_then(|u| u.as_str())
            .unwrap_or("")
            .to_string();
        let digest = a
            .get("digest")
            .and_then(|d| d.as_str())
            .and_then(|d| d.strip_prefix("sha256:"))
            .map(|h| h.to_string());
        let size = a.get("size").and_then(|s| s.as_u64()).unwrap_or(0);
        assets.push(AssetInfo {
            name,
            browser_download_url: url,
            digest_sha256: digest,
            size,
        });
    }
    Ok(ReleaseInfo {
        tag: tag.to_string(),
        version,
        name: v.get("name").and_then(|n| n.as_str()).unwrap_or("").to_string(),
        notes: v.get("body").and_then(|b| b.as_str()).unwrap_or("").to_string(),
        assets,
    })
}

/// The one asset this machine may install: exact name match, which
/// also pins it to the validated tag version and current arch.
pub fn select_asset<'a>(info: &'a ReleaseInfo) -> Result<&'a AssetInfo, String> {
    let want = expected_asset_name(&info.version);
    let asset = info
        .assets
        .iter()
        .find(|a| a.name == want)
        .ok_or_else(|| format!("no asset {} in {}", want, info.tag))?;
    // A published size of 0 would disable both download caps (space check
    // and byte ceiling), so an asset that does not state its size is
    // refused instead of being fetched unbounded.
    if asset.size == 0 {
        return Err(format!("asset {} has no published size. Aborted.", want));
    }
    Ok(asset)
}

/// The optional `checksums.txt` asset. When the release carries one,
/// its published size must be non-zero and at most `CHECKSUMS_MAX_BYTES`
/// so the download, the space check and the on-disk read share one
/// small ceiling; a lying size is refused before anything is fetched.
pub fn select_checksums_asset<'a>(
    info: &'a ReleaseInfo,
) -> Result<Option<&'a AssetInfo>, String> {
    let Some(asset) = info.assets.iter().find(|a| a.name == "checksums.txt") else {
        return Ok(None);
    };
    if asset.size == 0 || asset.size > CHECKSUMS_MAX_BYTES {
        return Err(format!(
            "checksums.txt size {} is out of range (0, {}]. Aborted.",
            asset.size, CHECKSUMS_MAX_BYTES
        ));
    }
    Ok(Some(asset))
}

/// True only when the published version is strictly newer.
/// Equal means "up to date"; older is never a downgrade candidate.
pub fn needs_update(current: &semver::Version, latest: &semver::Version) -> bool {
    latest > current
}

/// Pure redirect-target check, no I/O: parsed with the `url` crate,
/// not string prefixes. Requires https, an exact allowlisted host
/// (no suffix tricks, trailing dots rejected by parsing), no
/// username/password, no non-443 port, and no backslash anywhere.
pub fn host_allowed(url: &str) -> bool {
    host_allowed_in(url, ASSET_HOSTS)
}

fn host_allowed_in(url: &str, allowed: &[&str]) -> bool {
    if url.contains('\\') {
        return false;
    }
    // Tests serve fixtures over loopback; release builds never see this:
    // the override lives in a mutex-guarded static under cfg(test), so
    // parallel tests never race on process-global environment.
    #[cfg(test)]
    if let Some(hook) = test_host_override() {
        if let Ok(u) = url::Url::parse(url) {
            if u.host_str() == Some(hook.as_str()) {
                return true;
            }
        }
    }
    let parsed = match url::Url::parse(url) {
        Ok(u) => u,
        Err(_) => return false,
    };
    if parsed.scheme() != "https" {
        return false;
    }
    if !parsed.username().is_empty() || parsed.password().is_some() {
        return false;
    }
    if parsed.port().is_some_and(|p| p != 443) {
        return false;
    }
    match parsed.host_str() {
        Some(h) => allowed.iter().any(|a| h == *a),
        None => false,
    }
}

/// Origin pinning for artifacts: parsed with `url`, never prefix
/// strings. Requires https, host exactly `github.com`, no
/// username/password, no non-443 port, no query, no fragment, and
/// path segments exactly
/// `Matts-lab69/CorkyTux-Launcher/releases/download/<tag>/<asset>`.
/// A same-host URL from another repo, a traversal, or an encoded
/// dot/slash segment is refused.
pub fn asset_origin_ok(asset_url: &str, tag: &str, asset_name: &str) -> bool {
    if asset_url.contains('\\') {
        return false;
    }
    let parsed = match url::Url::parse(asset_url) {
        Ok(u) => u,
        Err(_) => return false,
    };
    if parsed.scheme() != "https" {
        return false;
    }
    if parsed.host_str() != Some("github.com") {
        return false;
    }
    if !parsed.username().is_empty() || parsed.password().is_some() {
        return false;
    }
    if parsed.port().is_some_and(|p| p != 443) {
        return false;
    }
    if parsed.query().is_some() || parsed.fragment().is_some() {
        return false;
    }
    let segs: Vec<String> = parsed
        .path_segments()
        .map(|it| it.map(|s| s.to_string()).collect())
        .unwrap_or_default();
    if segs.len() != 6 {
        return false;
    }
    for s in &segs {
        if s.is_empty() || s == "." || s == ".." {
            return false;
        }
        let low = s.to_lowercase();
        if low.contains("%2e") || low.contains("%2f") {
            return false;
        }
    }
    segs[0] == "Matts-lab69"
        && segs[1] == "CorkyTux-Launcher"
        && segs[2] == "releases"
        && segs[3] == "download"
        && segs[4] == tag
        && segs[5] == asset_name
}

fn http_client() -> Result<reqwest::blocking::Client, String> {
    reqwest::blocking::Client::builder()
        .user_agent(USER_AGENT)
        .connect_timeout(CONNECT_TIMEOUT)
        .timeout(TOTAL_TIMEOUT)
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .map_err(|e| format!("HTTP client failed: {}", e))
}

/// GET with manual redirect handling: at most MAX_REDIRECTS hops,
/// each target validated by the allowlist before following.
fn get_guarded(
    client: &reqwest::blocking::Client,
    url: &str,
    etag: Option<&str>,
    allowed: &[&str],
) -> Result<reqwest::blocking::Response, String> {
    let mut current = url.to_string();
    for _ in 0..=MAX_REDIRECTS {
        if !host_allowed_in(&current, allowed) {
            return Err(format!("redirect target not allowed: {}", current));
        }
        let mut req = client.get(current.clone());
        if let Some(tag) = etag {
            req = req.header("If-None-Match", tag);
        }
        let resp = req.send().map_err(|e| format!("request failed: {}", e))?;
        let status = resp.status().as_u16();
        if (300..400).contains(&status) {
            let loc = resp
                .headers()
                .get("location")
                .and_then(|h| h.to_str().ok())
                .ok_or("redirect without location")?
                .to_string();
            // Resolve relative locations with Url::join, not format!.
            let joined = url::Url::parse(&current)
                .map_err(|_| format!("redirect target not allowed: {}", current))?
                .join(&loc)
                .map_err(|_| format!("redirect target not allowed: {}", loc))?;
            current = joined.to_string();
            continue;
        }
        return Ok(resp);
    }
    Err("too many redirects".into())
}

/// Fresh API body plus the response ETag, or NotModified on 304.
#[derive(Debug)]
pub enum ApiOutcome {
    Fresh(String, Option<String>),
    NotModified,
}

/// Hard cap for release metadata: a `releases/latest` body is JSON
/// of a few KiB. Anything over 1 MiB is refused before parsing.
pub const MAX_API_BODY: u64 = 1024 * 1024;

pub fn api_get_latest(etag: Option<&str>) -> Result<ApiOutcome, String> {
    let client = http_client()?;
    let resp = get_guarded(&client, &api_latest_url(), etag, API_HOSTS)?;
    match resp.status().as_u16() {
        304 => Ok(ApiOutcome::NotModified),
        200 => {
            let tag = resp
                .headers()
                .get("etag")
                .and_then(|h| h.to_str().ok())
                .map(|s| s.to_string());
            let body = read_capped(resp, MAX_API_BODY)?;
            Ok(ApiOutcome::Fresh(body, tag))
        }
        403 => Err("GitHub API rate limit reached (403). Try again later.".into()),
        s => Err(format!("release check failed: HTTP {}", s)),
    }
}

/// Read a response body up to `cap` bytes; error when it is larger.
/// Guards both the live API and any future large-body reads.
fn read_capped(resp: reqwest::blocking::Response, cap: u64) -> Result<String, String> {
    let mut buf = Vec::new();
    resp.take(cap + 1)
        .read_to_end(&mut buf)
        .map_err(|e| format!("read failed: {}", e))?;
    if buf.len() as u64 > cap {
        return Err("release metadata exceeds 1 MiB. Aborted.".into());
    }
    String::from_utf8(buf).map_err(|_| "release metadata is not UTF-8".into())
}

/// Read a whole file up to `cap` bytes and require UTF-8. Anything
/// larger is refused before the bytes are used: same discipline as
/// `read_capped`, for files fetched to disk (e.g. `checksums.txt`).
pub fn read_file_capped(path: &Path, cap: u64) -> Result<String, String> {
    let handle = std::fs::File::open(path).map_err(|e| format!("read failed: {}", e))?;
    let mut buf = Vec::new();
    handle
        .take(cap + 1)
        .read_to_end(&mut buf)
        .map_err(|e| format!("read failed: {}", e))?;
    if buf.len() as u64 > cap {
        return Err("file exceeds its size cap. Aborted.".into());
    }
    String::from_utf8(buf).map_err(|_| "file is not UTF-8".into())
}

fn free_bytes(dir: &Path) -> Option<u64> {
    use std::os::unix::ffi::OsStrExt;
    let c = std::ffi::CString::new(dir.as_os_str().as_bytes()).ok()?;
    let mut st: libc::statvfs = unsafe { std::mem::zeroed() };
    if unsafe { libc::statvfs(c.as_ptr(), &mut st) } != 0 {
        return None;
    }
    Some(st.f_bavail as u64 * st.f_frsize as u64)
}

/// Byte ceiling for one download: the published size when it is set,
/// else `hard_cap`; never above `hard_cap`.
pub fn download_cap(expected_size: u64, hard_cap: u64) -> u64 {
    if expected_size == 0 {
        hard_cap
    } else {
        expected_size.min(hard_cap)
    }
}

/// Download to `dest_part` with the process-wide `MAX_DOWNLOAD_BYTES`
/// ceiling. See `download_asset_limited`.
pub fn download_asset(
    url: &str,
    dest_part: &Path,
    expected_size: u64,
    progress: &dyn Fn(u64, u64),
    cancel: &Arc<AtomicBool>,
) -> Result<u64, String> {
    download_asset_limited(
        url,
        dest_part,
        expected_size,
        MAX_DOWNLOAD_BYTES,
        progress,
        cancel,
    )
}

/// Download to `dest_part` with progress, cancel, and stall guards.
/// Removes the partial file on cancel or error. Never accepts more
/// than `download_cap(expected_size, hard_cap)` bytes, so an absent or
/// undersized published size still cannot exceed `hard_cap`. Returns
/// byte count.
pub fn download_asset_limited(
    url: &str,
    dest_part: &Path,
    expected_size: u64,
    hard_cap: u64,
    progress: &dyn Fn(u64, u64),
    cancel: &Arc<AtomicBool>,
) -> Result<u64, String> {
    use std::io::Write;
    if cancel.load(Ordering::Relaxed) {
        return Err("download cancelled".into());
    }
    // Fail fast on a disallowed origin before creating anything.
    if !host_allowed(url) {
        return Err(format!("redirect target not allowed: {}", url));
    }
    let cap = download_cap(expected_size, hard_cap);
    if let Some(parent) = dest_part.parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("cannot create download dir: {}", e))?;
        if let Some(free) = free_bytes(parent) {
            if free < cap + 8 * 1024 * 1024 {
                return Err("not enough disk space for the download".into());
            }
        }
    }
    let client = http_client()?;
    let mut resp = get_guarded(&client, url, None, ASSET_HOSTS)?;
    if resp.status().as_u16() != 200 {
        return Err(format!("download failed: HTTP {}", resp.status()));
    }
    let total = resp.content_length().unwrap_or(expected_size);
    // A declared length above the cap is refused before the partial file
    // is touched, so an oversized body never writes anything.
    if total > cap {
        return Err("download exceeds size limit".into());
    }
    // Unlink any stale `.part` first, then create it with O_EXCL: a
    // pre-planted symlink is removed (never followed) and a re-planted
    // one makes the create fail instead of writing through it.
    let _ = std::fs::remove_file(dest_part);
    let mut out = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(dest_part)
        .map_err(|e| format!("cannot write download: {}", e))?;
    let started = Instant::now();
    let mut last_progress = Instant::now();
    let mut done: u64 = 0;
    let mut buf = [0u8; 64 * 1024];
    loop {
        if cancel.load(Ordering::Relaxed) {
            drop(out);
            let _ = std::fs::remove_file(dest_part);
            return Err("download cancelled".into());
        }
        if started.elapsed() > TOTAL_TIMEOUT {
            drop(out);
            let _ = std::fs::remove_file(dest_part);
            return Err("download timed out".into());
        }
        if last_progress.elapsed() > INACTIVITY_TIMEOUT {
            drop(out);
            let _ = std::fs::remove_file(dest_part);
            return Err("download stalled".into());
        }
        let n: usize = resp
            .read(&mut buf)
            .map_err(|e| format!("download interrupted: {}", e))?;
        if n == 0 {
            break;
        }
        out.write_all(&buf[..n])
            .map_err(|e| format!("cannot write download: {}", e))?;
        done += n as u64;
        // Hard stop: never accept more than the computed cap.
        if done > cap {
            drop(out);
            let _ = std::fs::remove_file(dest_part);
            return Err("download exceeds size limit".into());
        }
        last_progress = Instant::now();
        progress(done, total);
    }
    out.flush().map_err(|e| format!("cannot write download: {}", e))?;
    drop(out);
    Ok(done)
}

/// SHA256 of a file, lowercase hex. The single seam for future
/// minisign/GPG verification: that check plugs in here (or beside
/// it) without touching download/install callers.
pub fn verify_artifact(path: &Path, expected_hex: &str) -> Result<(), String> {
    use sha2::Digest;
    let mut file =
        std::fs::File::open(path).map_err(|e| format!("cannot read artifact: {}", e))?;
    let mut hasher = sha2::Sha256::new();
    let mut buf = [0u8; 64 * 1024];
    loop {
        let n: usize = file.read(&mut buf).map_err(|e| format!("cannot read artifact: {}", e))?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    let got = hex_encode(&hasher.finalize());
    let want = expected_hex
        .strip_prefix("sha256:")
        .unwrap_or(expected_hex)
        .to_lowercase();
    if got == want {
        Ok(())
    } else {
        Err("checksum mismatch: download is corrupt or tampered".into())
    }
}

fn hex_encode(bytes: &[u8]) -> String {
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        s.push_str(&format!("{:02x}", b));
    }
    s
}

/// Mandatory API digest, or abort: a release without a published
/// checksum is never installed.
pub fn require_digest(asset: &AssetInfo) -> Result<String, String> {
    match asset.digest_sha256.as_deref().unwrap_or("").trim() {
        "" => Err("release has no published checksum. Aborted.".into()),
        h => Ok(h.to_string()),
    }
}

/// checksums.txt agreement: the file's line for `asset_name` must
/// equal the API digest. Mismatch, bad line, or absence aborts.
pub fn checksums_agree(content: &str, asset_name: &str, api_digest: &str) -> Result<(), String> {
    match parse_checksums_txt(content, asset_name) {
        Ok(hex) if hex == api_digest.to_lowercase() => Ok(()),
        _ => Err("checksums.txt disagrees with the release digest. Aborted.".into()),
    }
}

/// Find the `<hex>  <asset>` line for our tarball in a checksums.txt
/// body (`sha256sum` format, `*` binary marker tolerated).
pub fn parse_checksums_txt(content: &str, asset_name: &str) -> Result<String, String> {
    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let mut parts = line.split_whitespace();
        let hex = parts.next().unwrap_or("");
        let name = parts.next().unwrap_or("").trim_start_matches('*');
        let name = name.rsplit('/').next().unwrap_or(name);
        if name == asset_name {
            if hex.len() == 64 && hex.bytes().all(|b| b.is_ascii_hexdigit()) {
                return Ok(hex.to_lowercase());
            }
            return Err(format!("bad checksum line for {}", asset_name));
        }
    }
    Err(format!("{} not listed in checksums.txt", asset_name))
}

/// Files extracted from a release tarball into staging.
#[derive(Clone, Debug, Default)]
pub struct Staged {
    pub binary: PathBuf,
    pub helper: Option<PathBuf>,
    pub assets_dir: Option<PathBuf>,
    pub icon: Option<PathBuf>,
}

/// Extraction bomb guards.
#[derive(Clone, Debug)]
pub struct ExtractLimits {
    pub max_total_bytes: u64,
    pub max_file_bytes: u64,
    pub max_entries: usize,
}

impl Default for ExtractLimits {
    fn default() -> Self {
        ExtractLimits {
            max_total_bytes: 200 * 1024 * 1024,
            max_file_bytes: 50 * 1024 * 1024,
            max_entries: 10_000,
        }
    }
}

/// Unpack a release tarball into `staging` (created fresh).
/// Only expected entries land on disk; install scripts and the
/// desktop template are skipped, everything else aborts the run.
pub fn extract_tarball(archive: &Path, staging: &Path) -> Result<Staged, String> {
    extract_tarball_limited(archive, staging, &ExtractLimits::default())
}

/// Same, with explicit bomb guards (tests use tiny limits).
/// Requires max_total + 32 MiB free before writing anything.
pub fn extract_tarball_limited(
    archive: &Path,
    staging: &Path,
    limits: &ExtractLimits,
) -> Result<Staged, String> {
    use std::io::Write;
    if staging.exists() {
        std::fs::remove_dir_all(staging).map_err(|e| format!("cannot clear staging: {}", e))?;
    }
    std::fs::create_dir_all(staging).map_err(|e| format!("cannot create staging: {}", e))?;
    if let Some(parent) = staging.parent() {
        if let Some(free) = free_bytes(parent) {
            if free < limits.max_total_bytes + 32 * 1024 * 1024 {
                return Err("not enough disk space to extract the update".into());
            }
        }
    }
    let file =
        std::fs::File::open(archive).map_err(|e| format!("cannot read tarball: {}", e))?;
    let gz = flate2::read::GzDecoder::new(file);
    let mut ar = tar::Archive::new(gz);
    let entries = ar.entries().map_err(|e| format!("bad tarball: {}", e))?;
    let mut out = Staged::default();
    let mut total: u64 = 0;
    let mut count: usize = 0;
    let mut buf = [0u8; 64 * 1024];
    for entry in entries {
        count += 1;
        if count > limits.max_entries {
            return Err("tarball has too many entries: refused".into());
        }
        let mut entry = entry.map_err(|e| format!("bad tarball entry: {}", e))?;
        let kind = entry.header().entry_type();
        if kind.is_symlink() || kind.is_hard_link() {
            return Err("tarball contains a link: refused".into());
        }
        if !kind.is_file() && !kind.is_dir() {
            return Err("tarball contains a special file: refused".into());
        }
        let rel = entry.path().map_err(|e| format!("bad tarball path: {}", e))?.into_owned();
        // Strip the single top-level dir (corkytux-X.Y.Z/).
        let mut comps = rel.components().peekable();
        match comps.next() {
            Some(Component::Normal(_)) => {}
            _ => return Err("tarball has an absolute or empty top entry: refused".into()),
        }
        let mut clean = PathBuf::new();
        for c in comps {
            match c {
                Component::Normal(name) => clean.push(name),
                _ => return Err("tarball path escapes its directory: refused".into()),
            }
        }
        if clean.as_os_str().is_empty() {
            continue;
        }
        let target = staging.join(&clean);
        let name = clean.to_string_lossy().to_string();
        if name == "install.sh" || name == "uninstall.sh" || name == "corkytux.desktop.in" {
            continue;
        }
        if kind.is_dir() {
            if name != "assets" && !name.starts_with("assets/") {
                return Err(format!("unexpected directory in tarball: {}", name));
            }
            std::fs::create_dir_all(&target).map_err(|e| format!("cannot stage {}: {}", name, e))?;
            continue;
        }
        // Regular file: allowlisted name, then a counted copy so a
        // lying header or a gzip bomb cannot fill the disk.
        let staged_kind: Option<fn(&mut Staged, PathBuf)> = match name.as_str() {
            "corkytux" => Some(|o, p| o.binary = p),
            "webdriver_login" => Some(|o, p| o.helper = Some(p)),
            "corkytux.png" => Some(|o, p| o.icon = Some(p)),
            _ if name == "assets" || name.starts_with("assets/") => None,
            _ => return Err(format!("unexpected file in tarball: {}", name)),
        };
        let declared = entry
            .header()
            .size()
            .map_err(|e| format!("bad tarball entry: {}", e))?;
        if declared > limits.max_file_bytes {
            return Err(format!("tarball file too large: {}", name));
        }
        if let Some(parent) = target.parent() {
            std::fs::create_dir_all(parent).map_err(|e| format!("cannot stage {}: {}", name, e))?;
        }
        let mut dst =
            std::fs::File::create(&target).map_err(|e| format!("cannot stage {}: {}", name, e))?;
        let mut written: u64 = 0;
        loop {
            let n: usize = entry
                .read(&mut buf)
                .map_err(|e| format!("cannot stage {}: {}", name, e))?;
            if n == 0 {
                break;
            }
            written += n as u64;
            if written > limits.max_file_bytes {
                drop(dst);
                let _ = std::fs::remove_file(&target);
                return Err(format!("tarball file too large: {}", name));
            }
            dst.write_all(&buf[..n])
                .map_err(|e| format!("cannot stage {}: {}", name, e))?;
        }
        total += written;
        if total > limits.max_total_bytes {
            let _ = std::fs::remove_file(&target);
            return Err("tarball too large when unpacked: refused".into());
        }
        match staged_kind {
            Some(set) => {
                // Only runnable artifacts get the exec bit: the icon is
                // staged (and installed) as a plain 0o644 file.
                if name == "corkytux.png" {
                    set_plain(&target)?;
                } else {
                    set_exec(&target)?;
                }
                set(&mut out, target);
            }
            None => {
                if out.assets_dir.is_none() {
                    out.assets_dir = Some(staging.join("assets"));
                }
            }
        }
    }
    if out.binary.as_os_str().is_empty() {
        return Err("tarball has no corkytux binary".into());
    }
    Ok(out)
}

#[cfg(unix)]
fn set_exec(path: &Path) -> Result<(), String> {
    use std::os::unix::fs::PermissionsExt;
    let mut perms =
        std::fs::metadata(path).map_err(|e| format!("cannot stat staged file: {}", e))?.permissions();
    perms.set_mode(0o755);
    std::fs::set_permissions(path, perms).map_err(|e| format!("cannot chmod staged file: {}", e))
}

#[cfg(not(unix))]
fn set_exec(_path: &Path) -> Result<(), String> {
    Ok(())
}

/// Staged icon: plain 0o644, never executable.
#[cfg(unix)]
fn set_plain(path: &Path) -> Result<(), String> {
    use std::os::unix::fs::PermissionsExt;
    let mut perms =
        std::fs::metadata(path).map_err(|e| format!("cannot stat staged file: {}", e))?.permissions();
    perms.set_mode(0o644);
    std::fs::set_permissions(path, perms).map_err(|e| format!("cannot chmod staged file: {}", e))
}

#[cfg(not(unix))]
fn set_plain(_path: &Path) -> Result<(), String> {
    Ok(())
}

/// Run the staged binary with `--version` and require exactly
/// `corkytux <expected>`. Timeout kills the child. The new binary
/// exits before GTK init, so no display is opened.
pub fn health_check(bin: &Path, expected: &semver::Version) -> Result<(), String> {
    use std::process::Stdio;
    // Spawn retries solely on ETXTBSY (errno 26): a just-written
    // file can briefly refuse exec under parallel I/O (observed in
    // tests on tmpfs). Any other spawn error fails at once; a
    // persistent busy file still surfaces after 3 attempts.
    let mut child = None;
    let mut last_err = String::new();
    for _ in 0..3 {
        match std::process::Command::new(bin)
            .arg("--version")
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
        {
            Ok(c) => {
                child = Some(c);
                break;
            }
            Err(e) => {
                if e.raw_os_error() != Some(libc::ETXTBSY) {
                    return Err(format!("new binary would not start: {}", e));
                }
                last_err = format!("new binary would not start: {}", e);
                std::thread::sleep(Duration::from_millis(100));
            }
        }
    }
    let mut child = child.ok_or(last_err)?;
    let deadline = Instant::now() + HEALTH_TIMEOUT;
    loop {
        match child.try_wait().map_err(|e| format!("health check failed: {}", e))? {
            Some(status) => {
                let mut text = String::new();
                if let Some(mut o) = child.stdout.take() {
                    let _ = o.read_to_string(&mut text);
                }
                if !status.success() {
                    let _ = child.wait();
                    return Err("new binary failed its health check".into());
                }
                let want = format!("corkytux {}", expected);
                if text.trim() == want {
                    return Ok(());
                }
                return Err(format!(
                    "new binary reports a wrong version (want {})",
                    want
                ));
            }
            None => {
                if Instant::now() > deadline {
                    let _ = child.kill();
                    let _ = child.wait();
                    return Err("new binary hung on --version".into());
                }
                std::thread::sleep(Duration::from_millis(50));
            }
        }
    }
}

/// Timestamped copy-aside of `path` inside its own dir
/// (`name.bak-<unix-secs>-<counter>`, same scheme as install.sh plus a
/// process-wide counter). The counter makes names unique even when two
/// installs land in the same second. Retention is applied separately,
/// only after an install lands (see `prune_install_backups`).
pub fn backup_file(path: &Path) -> Result<PathBuf, String> {
    static BACKUP_SEQ: AtomicU64 = AtomicU64::new(0);
    let name = path
        .file_name()
        .and_then(|n| n.to_str())
        .ok_or("bad backup path")?
        .to_string();
    let dir = path.parent().ok_or("bad backup path")?.to_path_buf();
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|e| format!("clock failed: {}", e))?
        .as_secs();
    let seq = BACKUP_SEQ.fetch_add(1, Ordering::Relaxed);
    // Zero-padded so prune_backups' lexical sort stays chronological.
    let bak = dir.join(format!("{}.bak-{}-{:06}", name, secs, seq));
    if path.is_dir() {
        copy_dir(path, &bak)?;
    } else {
        std::fs::copy(path, &bak).map_err(|e| format!("backup failed: {}", e))?;
    }
    Ok(bak)
}

fn copy_dir(src: &Path, dst: &Path) -> Result<(), String> {
    // Follows symlinks (`is_dir` / `copy`): safe today because both
    // callers pass trees the updater created (tar extraction rejects
    // links). A symlink cycle in a user-provided tree would recurse
    // without bound; a `symlink_metadata` walk would close that.
    std::fs::create_dir_all(dst).map_err(|e| format!("backup failed: {}", e))?;
    let entries =
        std::fs::read_dir(src).map_err(|e| format!("backup failed: {}", e))?;
    for e in entries {
        let e = e.map_err(|e| format!("backup failed: {}", e))?;
        let (s, d) = (e.path(), dst.join(e.file_name()));
        if s.is_dir() {
            copy_dir(&s, &d)?;
        } else {
            std::fs::copy(&s, &d).map_err(|e| format!("backup failed: {}", e))?;
        }
    }
    Ok(())
}

/// True when `name` is a backup this updater wrote: the given prefix
/// followed by `<digits>-<6 digits>`. Anything that merely shares the
/// prefix is somebody else's file and is never a prune candidate.
fn is_own_backup_name(name: &str, prefix: &str) -> bool {
    let Some(rest) = name.strip_prefix(prefix) else {
        return false;
    };
    let Some((secs, seq)) = rest.split_once('-') else {
        return false;
    };
    !secs.is_empty()
        && secs.bytes().all(|b| b.is_ascii_digit())
        && seq.len() == 6
        && seq.bytes().all(|b| b.is_ascii_digit())
}

fn prune_backups(dir: &Path, prefix: &str, keep_newest: usize) {
    let mut found: Vec<PathBuf> = Vec::new();
    if let Ok(entries) = std::fs::read_dir(dir) {
        for e in entries.flatten() {
            let n = e.file_name().to_string_lossy().to_string();
            // Only names this updater produced: a foreign file that only
            // shares the prefix is never collected or deleted.
            if is_own_backup_name(&n, prefix) {
                found.push(e.path());
            }
        }
    }
    // Oldest first: names are `<secs>-<counter>`, both fixed width.
    found.sort();
    if found.len() <= keep_newest + 1 {
        return;
    }
    let oldest = found.first().cloned();
    let keep_from = found.len() - keep_newest;
    for (i, old) in found.iter().enumerate() {
        // Keep the oldest (the pre-update original) and the newest
        // `keep_newest`; drop everything in between.
        if i >= keep_from || Some(old) == oldest.as_ref() {
            continue;
        }
        if old.is_dir() {
            let _ = std::fs::remove_dir_all(old);
        } else {
            let _ = std::fs::remove_file(old);
        }
    }
}

/// Apply backup retention across every install target: the oldest plus
/// the newest `BACKUP_KEEP_NEWEST`. Called only after an install lands,
/// so a failed install never deletes an existing backup. Prefixes are
/// derived from the same names `backup_file` / `assets_backup_name`
/// write.
fn prune_install_backups(dirs: &InstallDirs) {
    let bin = dirs.install_dir.join("corkytux");
    let helper = dirs.install_dir.join("webdriver_login");
    let assets = dirs.install_dir.join("assets");
    let icon = dirs.icon_dir.join("corkytux.png");
    for p in [&bin, &helper, &assets, &icon, &dirs.desktop_file] {
        if let (Some(parent), Some(name)) =
            (p.parent(), p.file_name().and_then(|n| n.to_str()))
        {
            prune_backups(parent, &format!("{}.bak-", name), BACKUP_KEEP_NEWEST);
        }
    }
}

/// Atomic file replace inside one directory: temp + rename. The final
/// mode is the explicit `mode` argument, not the source's: the tmp is
/// created with the umask default and chmodded before the rename, so a
/// setuid/setgid bit on the source can never survive into the install.
fn atomic_replace_mode(src: &Path, dst: &Path, mode: u32) -> Result<(), String> {
    use std::io::Write as _;
    let tmp = dst.with_extension("new");
    // Unlink any stale tmp first, then create it with O_EXCL: a
    // pre-planted symlink is removed (never followed) and a re-planted
    // one makes the create fail instead of writing through it.
    let _ = std::fs::remove_file(&tmp);
    let mut out = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&tmp)
        .map_err(|e| format!("install failed: {}", e))?;
    let mut input = std::fs::File::open(src).map_err(|e| format!("install failed: {}", e))?;
    if let Err(e) = std::io::copy(&mut input, &mut out) {
        drop(out);
        let _ = std::fs::remove_file(&tmp);
        return Err(format!("install failed: {}", e));
    }
    if let Err(e) = out.flush() {
        drop(out);
        let _ = std::fs::remove_file(&tmp);
        return Err(format!("install failed: {}", e));
    }
    // Durability before the rename: a crash must not leave a truncated
    // destination that looks complete.
    if let Err(e) = out.sync_all() {
        drop(out);
        let _ = std::fs::remove_file(&tmp);
        return Err(format!("install failed: {}", e));
    }
    drop(out);
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = std::fs::metadata(&tmp)
            .map_err(|e| format!("install failed: {}", e))?
            .permissions();
        perms.set_mode(mode);
        std::fs::set_permissions(&tmp, perms).map_err(|e| format!("install failed: {}", e))?;
    }
    #[cfg(not(unix))]
    let _ = mode;
    std::fs::rename(&tmp, dst).map_err(|e| format!("install failed: {}", e))?;
    Ok(())
}

/// Atomic replace that installs an executable (binary/helper): mode
/// 0o755. See `atomic_replace_mode`.
fn atomic_replace(src: &Path, dst: &Path) -> Result<(), String> {
    atomic_replace_mode(src, dst, 0o755)
}

/// Install targets: mirrors install.sh locations.
pub struct InstallDirs {
    pub install_dir: PathBuf,
    pub icon_dir: PathBuf,
    pub desktop_file: PathBuf,
    pub symlink: PathBuf,
}

impl InstallDirs {
    /// Fixed destination: $HOME/.local/share/corkytux, the exact dir
    /// install.sh writes. install.sh does not honor XDG_DATA_HOME,
    /// so neither does the updater (same location, no surprises).
    pub fn fixed(home: &Path) -> Self {
        InstallDirs {
            install_dir: home.join(".local/share/corkytux"),
            icon_dir: home.join(".local/share/icons"),
            desktop_file: home.join(".local/share/applications/corkytux.desktop"),
            symlink: home.join(".local/bin/corkytux"),
        }
    }
}

/// Whether `exe` runs from inside `install_dir`. Tolerates a kernel
/// " (deleted)" suffix on the exe path. Both sides are canonicalized
/// (symlinks resolved) so a HOME reached through a symlink still
/// counts; when the exe itself does not exist, its parent is
/// canonicalized and the file name re-attached; anything still
/// unresolvable falls back to lexical comparison. Anything else
/// (repo builds, /tmp copies, other prefixes) is not an installed copy.
pub fn installed_here(exe: &Path, install_dir: &Path) -> bool {
    let s = exe.to_string_lossy();
    let s = s.strip_suffix(" (deleted)").unwrap_or(&s);
    let exe_path = Path::new(s);
    let canon_exe = std::fs::canonicalize(exe_path).unwrap_or_else(|_| {
        match (
            exe_path.parent().and_then(|p| std::fs::canonicalize(p).ok()),
            exe_path.file_name(),
        ) {
            (Some(dir), Some(file)) => dir.join(file),
            _ => exe_path.to_path_buf(),
        }
    });
    let canon_dir =
        std::fs::canonicalize(install_dir).unwrap_or_else(|_| install_dir.to_path_buf());
    canon_exe.starts_with(canon_dir)
}

/// Tracks the backups created during install pre-flight so a failure
/// *before* the replace block removes exactly what this run created:
/// a pre-outcome `?` must not leave `.bak` residue. Disarmed once the
/// replace block owns the backups (rollback restores them, a landed
/// install prunes them).
struct PreOutcomeBackups(Vec<PathBuf>);

impl PreOutcomeBackups {
    fn new() -> Self {
        PreOutcomeBackups(Vec::new())
    }
    fn keep(&mut self, path: PathBuf) -> PathBuf {
        self.0.push(path.clone());
        path
    }
    fn disarm(mut self) {
        self.0.clear();
    }
}

impl Drop for PreOutcomeBackups {
    fn drop(&mut self) {
        for b in &self.0 {
            if b.is_dir() {
                let _ = std::fs::remove_dir_all(b);
            } else {
                let _ = std::fs::remove_file(b);
            }
        }
    }
}

/// Health-check the staged binary, then install binary/helper/assets/
/// icon with backups and atomic replaces. A missing current binary is
/// not an update: refuse before touching anything. Any failure after
/// the first replace restores binary, helper, icon and assets, and
/// reports the original error plus any rollback errors (rollback
/// failures never hide the original). Returns log lines for UI.
pub fn install_if_healthy(
    staged: &Staged,
    dirs: &InstallDirs,
    expected: &semver::Version,
) -> Result<Vec<String>, String> {
    let mut log: Vec<String> = Vec::new();
    // Pre-flight: newlines in the binary/icon paths would break out
    // of the .desktop Exec/Icon lines and inject desktop entries.
    // Refuse before health checks, backups, or any install step, so
    // a refusal touches nothing.
    {
        let icon = dirs.icon_dir.join("corkytux.png");
        for p in [dirs.install_dir.join("corkytux"), icon] {
            let s = p.display().to_string();
            if s.contains('\n') || s.contains('\r') {
                return Err("refusing to write .desktop entry with newline in path".into());
            }
        }
    }
    // TOCTOU note: the staged binary is health-checked here and copied
    // later by `atomic_replace`; a same-user writer of downloads_dir
    // could swap the file in between. Checking and copying the same fd
    // would close that window.
    health_check(&staged.binary, expected)?;
    log.push("Health check passed.".into());
    let current_bin = dirs.install_dir.join("corkytux");
    if !current_bin.is_file() {
        return Err("no installed binary to update; install manually".into());
    }
    let helper_path = dirs.install_dir.join("webdriver_login");
    let icon_path = dirs.icon_dir.join("corkytux.png");
    let assets_target = dirs.install_dir.join("assets");
    let mut pre = PreOutcomeBackups::new();
    let bin_bak = pre.keep(backup_file(&current_bin)?);
    log.push("Backed up current binary.".into());
    let helper_bak = if staged.helper.is_some() && helper_path.is_file() {
        Some(pre.keep(backup_file(&helper_path)?))
    } else {
        None
    };
    let helper_existed = helper_path.exists();
    let icon_bak = if staged.icon.is_some() && icon_path.is_file() {
        Some(pre.keep(backup_file(&icon_path)?))
    } else {
        None
    };
    let icon_existed = icon_path.exists();
    let assets_existed = assets_target.exists();
    let assets_bak: Option<PathBuf> = if staged.assets_dir.is_some() && assets_target.is_dir() {
        static ASSETS_BAK_SEQ: AtomicU64 = AtomicU64::new(0);
        let secs = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|e| format!("clock failed: {}", e))?
            .as_secs();
        let bak = dirs.install_dir.join(assets_backup_name(
            secs,
            ASSETS_BAK_SEQ.fetch_add(1, Ordering::Relaxed),
        ));
        std::fs::rename(&assets_target, &bak).map_err(|e| format!("install failed: {}", e))?;
        // Retention runs only once the install lands (post-outcome).
        Some(pre.keep(bak))
    } else {
        None
    };
    // The replace block now owns the backups: dispose of the tracking so
    // the guard does not delete them on the way out.
    pre.disarm();
    let outcome: Result<(), String> = (|| {
        atomic_replace(&staged.binary, &current_bin)?;
        log.push("Binary installed.".into());
        if let Some(helper) = &staged.helper {
            atomic_replace(helper, &helper_path)?;
            log.push("Helper installed.".into());
        }
        if let Some(assets) = &staged.assets_dir {
            copy_dir(assets, &assets_target)?;
            log.push("Assets installed.".into());
        }
        if let Some(icon) = &staged.icon {
            std::fs::create_dir_all(&dirs.icon_dir)
                .map_err(|e| format!("install failed: {}", e))?;
            // The icon is data, not an executable: install it 0o644.
            atomic_replace_mode(icon, &icon_path, 0o644)?;
            log.push("Icon installed.".into());
        }
        // Cosmetic tail, deliberately non-fatal: a desktop/symlink
        // hiccup is logged, never rolls back a good binary install.
        for w in ensure_desktop_and_link(dirs) {
            log.push(w);
        }
        Ok(())
    })();
    if let Err(e) = outcome {
        // Direct renames, not fresh temp copies: the temp path itself
        // may be what failed (poisoned .new dir), so rollback must not
        // depend on it. Backups are consumed by a successful rollback.
        let mut rb: Vec<String> = Vec::new();
        if let Err(r) = std::fs::rename(&bin_bak, &current_bin) {
            rb.push(format!("binary: {}", r));
        }
        // Files that did not exist before the update are removed
        // instead of restored: rollback leaves no trace of the
        // failed install.
        if helper_bak.is_some() {
            if let Some(hb) = &helper_bak {
                if let Err(r) = std::fs::rename(hb, &helper_path) {
                    rb.push(format!("helper: {}", r));
                }
            }
        } else if staged.helper.is_some() && !helper_existed {
            // Cleanup failures join the rollback report: a half-removed
            // trace must not be reported as "restored".
            if helper_path.is_symlink() || helper_path.is_file() {
                if let Err(r) = std::fs::remove_file(&helper_path) {
                    rb.push(format!("helper cleanup: {}", r));
                }
            } else if helper_path.is_dir() {
                if let Err(r) = std::fs::remove_dir_all(&helper_path) {
                    rb.push(format!("helper cleanup: {}", r));
                }
            }
        }
        if icon_bak.is_some() {
            if let Some(ib) = &icon_bak {
                if let Err(r) = std::fs::rename(ib, &icon_path) {
                    rb.push(format!("icon: {}", r));
                }
            }
        } else if staged.icon.is_some() && !icon_existed {
            if let Err(r) = std::fs::remove_file(&icon_path) {
                rb.push(format!("icon cleanup: {}", r));
            }
        }
        if let Some(ab) = &assets_bak {
            if assets_target.exists() {
                let cleared = if assets_target.is_dir() {
                    std::fs::remove_dir_all(&assets_target)
                } else {
                    std::fs::remove_file(&assets_target)
                };
                if let Err(r) = cleared {
                    rb.push(format!("assets cleanup: {}", r));
                }
            }
            if let Err(r) = std::fs::rename(ab, &assets_target) {
                rb.push(format!("assets: {}", r));
            }
        } else if staged.assets_dir.is_some() && !assets_existed {
            let cleared = if assets_target.is_dir() {
                std::fs::remove_dir_all(&assets_target).map_err(|e| e.to_string())
            } else if assets_target.is_file() || assets_target.is_symlink() {
                std::fs::remove_file(&assets_target).map_err(|e| e.to_string())
            } else {
                Ok(())
            };
            if let Err(r) = cleared {
                rb.push(format!("assets cleanup: {}", r));
            }
        }
        if rb.is_empty() {
            return Err(format!("{}\nPrevious version restored.", e));
        }
        return Err(format!("{}\nRollback also failed: {}", e, rb.join("; ")));
    }
    // The install landed: apply backup retention. Any failure returned
    // above before this point, so a failed install never prunes.
    prune_install_backups(dirs);
    Ok(log)
}

/// Value for Icon=: layer-2 string escaping only (a literal
/// backslash doubles), never quoted. Field codes do not apply to
/// Icon, and quoting it would change its meaning.
fn desktop_icon_value(path: &Path) -> String {
    path.display().to_string().replace('\\', "\\\\")
}

/// Whether the desktop entry already launches `bin`: the first
/// whitespace-separated token of the Exec line, with both spec
/// layers undone in reverse (string-value `\\` collapse first, then
/// quoting escapes, `%%`, and field codes). Only double quotes group;
/// single quotes are literal. An unterminated quote or a trailing
/// lone backslash never matches (fail closed: we rewrite it).
pub fn desktop_exec_matches(content: &str, bin: &Path) -> bool {
    for line in content.lines() {
        let line = line.trim();
        let rest = match line.strip_prefix("Exec=") {
            Some(r) => r.trim(),
            None => continue,
        };
        // Layer 2 undo: string-value escapes. \\ collapses, \s \n
        // \t \r map to space/newline/tab/CR. Any other escape or a
        // trailing lone backslash is malformed: no match (fail closed).
        let mut flat = String::with_capacity(rest.len());
        let mut it = rest.chars();
        let mut bad = false;
        while let Some(c) = it.next() {
            if c != '\\' {
                flat.push(c);
                continue;
            }
            match it.next() {
                Some('\\') => flat.push('\\'),
                Some('s') => flat.push(' '),
                Some('n') => flat.push('\n'),
                Some('t') => flat.push('\t'),
                Some('r') => flat.push('\r'),
                _ => {
                    bad = true;
                    break;
                }
            }
        }
        if bad {
            continue;
        }
        // Layer 1 undo: quoting, escapes, %%, field codes.
        let mut token = String::new();
        let mut chars = flat.chars();
        let mut quoted = false;
        let mut malformed = false;
        while let Some(c) = chars.next() {
            if quoted {
                if c == '"' {
                    quoted = false;
                } else if c == '\\' {
                    match chars.next() {
                        Some(n) => token.push(n),
                        None => {
                            malformed = true;
                            break;
                        }
                    }
                } else if c == '%' {
                    match chars.next() {
                        Some('%') => token.push('%'),
                        Some(other) => {
                            token.push('%');
                            token.push(other);
                        }
                        None => token.push('%'),
                    }
                } else {
                    token.push(c);
                }
            } else if c == '"' {
                quoted = true;
            } else if c == '\\' {
                match chars.next() {
                    Some(n) => token.push(n),
                    None => {
                        malformed = true;
                        break;
                    }
                }
            } else if c.is_whitespace() {
                break;
            } else if c == '%' {
                match chars.next() {
                    Some('%') => token.push('%'),
                    Some(other) => {
                        token.push('%');
                        token.push(other);
                    }
                    None => token.push('%'),
                }
            } else {
                token.push(c);
            }
        }
        if malformed || quoted {
            continue;
        }
        if Path::new(&token) == bin {
            return true;
        }
    }
    false
}

/// Quote for a .desktop Exec value, freedesktop double layer.
/// Layer 1 (quoting): the value is quoted when the path holds any
/// reserved character (space, tab, `" ' \ > < ~ | & ; $ * ? # ( )`
/// `` ` ``); inside quotes, `"`, `` ` ``, `$` and `\` take a
/// backslash escape. Layer 2 (string-value escape): every `\` of
/// that result is doubled. In the file this reads as: literal `\`
/// is `\\\\`, `$` is `\\$`, `"` is `\\"`, `` ` `` is `` \\` ``.
/// A literal `%` is always doubled (`%%`), quoted or not.
/// Icon= is never quoted (spec: only Exec takes quoting).
fn desktop_quote_exec(path: &Path) -> String {
    let s = path.display().to_string();
    // Field codes first: a literal % never reaches the file single.
    let s = s.replace('%', "%%");
    if !s.bytes().any(|b| matches!(
        b,
        b' '
            | b'\t'
            | b'"'
            | b'\''
            | b'\\'
            | b'>'
            | b'<'
            | b'~'
            | b'|'
            | b'&'
            | b';'
            | b'$'
            | b'*'
            | b'?'
            | b'#'
            | b'('
            | b')'
            | b'`'
    )) {
        return s;
    }
    // Layer 1: quoting escapes.
    let mut quoted = String::with_capacity(s.len() + 2);
    quoted.push('"');
    for c in s.chars() {
        if matches!(c, '"' | '`' | '$' | '\\') {
            quoted.push('\\');
        }
        quoted.push(c);
    }
    quoted.push('"');
    // Layer 2: string-value escape doubles every backslash.
    quoted.replace('\\', "\\\\")
}

/// Leave the desktop entry and symlink alone when they already point
/// at the installed binary; otherwise write them (desktop backed up).
fn ensure_desktop_and_link(dirs: &InstallDirs) -> Vec<String> {
    // Fully non-fatal: every hiccup becomes a warning line, never an
    // install failure. The binary/helper/assets/icon above already
    // landed; cosmetics must not roll them back.
    let mut warnings: Vec<String> = Vec::new();
    let bin = dirs.install_dir.join("corkytux");
    let link_ok = std::fs::read_link(&dirs.symlink).map(|t| t == bin).unwrap_or(false);
    if !link_ok {
        // Atomic swap through a tmp link, and only when the target is
        // missing or already a symlink: a real file or directory is
        // never touched.
        let mut proceed = true;
        match std::fs::symlink_metadata(&dirs.symlink) {
            Err(_) => {}
            Ok(m) if m.file_type().is_symlink() => {}
            Ok(_) => {
                warnings.push(format!(
                    "Symlink skipped: {} is not a symlink",
                    dirs.symlink.display()
                ));
                proceed = false;
            }
        }
        if proceed {
            let mut failed: Option<String> = None;
            if let Some(parent) = dirs.symlink.parent() {
                if let Err(e) = std::fs::create_dir_all(parent) {
                    failed = Some(format!("install failed: {}", e));
                }
            }
            if failed.is_none() {
                let tmp = dirs.symlink.with_extension("link.new");
                let _ = std::fs::remove_file(&tmp);
                #[cfg(unix)]
                match std::os::unix::fs::symlink(&bin, &tmp) {
                    Ok(()) => match std::fs::rename(&tmp, &dirs.symlink) {
                        Ok(()) => {}
                        Err(e) => failed = Some(format!("install failed: {}", e)),
                    },
                    Err(e) => failed = Some(format!("install failed: {}", e)),
                }
                let _ = std::fs::remove_file(&tmp);
            }
            if let Some(e) = failed {
                warnings.push(format!("Desktop entry skipped: {}", e));
            }
        }
    }
    let rewrite = match std::fs::read_to_string(&dirs.desktop_file) {
        Ok(content) => !desktop_exec_matches(&content, &bin),
        Err(_) => true,
    };
    if rewrite {
        if dirs.desktop_file.is_file() {
            if let Err(e) = backup_file(&dirs.desktop_file) {
                warnings.push(format!("Desktop entry skipped: {}", e));
                return warnings;
            }
        }
        if let Some(parent) = dirs.desktop_file.parent() {
            if let Err(e) = std::fs::create_dir_all(parent) {
                warnings.push(format!("Desktop entry skipped: {}", e));
                return warnings;
            }
        }
        // Same bytes install.sh writes (its heredoc), quoting only
        // when the paths require it. Atomic tmp + rename so a crash
        // never leaves a partial entry; create_new refuses to clobber.
        let icon = dirs.icon_dir.join("corkytux.png");
        let content = format!(
            "[Desktop Entry]\nVersion=1.0\nName=CorkyTux\nComment=Linux game launcher\n\
             Exec={bin}\nIcon={icon}\nTerminal=false\nType=Application\n\
             Categories=Game;\nStartupWMClass=corkytux\n",
            bin = desktop_quote_exec(&bin),
            icon = desktop_icon_value(&icon)
        );
        let tmp = dirs.desktop_file.with_extension("desktop.new");
        let _ = std::fs::remove_file(&tmp);
        let mut failed: Option<String> = None;
        match std::fs::File::create_new(&tmp) {
            Ok(mut f) => {
                use std::io::Write;
                if let Err(e) = f.write_all(content.as_bytes()) {
                    failed = Some(format!("install failed: {}", e));
                } else {
                    drop(f);
                    if let Err(e) = std::fs::rename(&tmp, &dirs.desktop_file) {
                        failed = Some(format!("install failed: {}", e));
                    }
                }
            }
            Err(e) => failed = Some(format!("install failed: {}", e)),
        }
        let _ = std::fs::remove_file(&tmp);
        if let Some(e) = failed {
            warnings.push(format!("Desktop entry skipped: {}", e));
        }
    }
    warnings
}

/// Remove staging and partial downloads, best effort.
pub fn cleanup(paths: &[&Path]) {
    for p in paths {
        if p.is_dir() {
            let _ = std::fs::remove_dir_all(p);
        } else {
            let _ = std::fs::remove_file(p);
        }
    }
}

/// True when some other process cmdline contains `marker`.
/// /proc scan, no pgrep dependency. Empty markers never match.
/// Exercised by tests; reserved for restart-flow diagnostics.
#[allow(dead_code)]
pub fn process_with_cmdline_running(marker: &str) -> bool {
    if marker.is_empty() {
        return false;
    }
    let me = std::process::id().to_string();
    let Ok(proc) = std::fs::read_dir("/proc") else {
        return false;
    };
    for e in proc.flatten() {
        let name = e.file_name().to_string_lossy().to_string();
        if name.parse::<u32>().is_err() || name == me {
            continue;
        }
        let cmd = std::fs::read(e.path().join("cmdline")).unwrap_or_default();
        if cmd.windows(marker.len()).any(|w| w == marker.as_bytes()) {
            return true;
        }
    }
    false
}

/// Relaunch script with zero interpolation: the binary path and the
/// parent PID travel as $1 and $2 (never spliced into the text), so
/// odd install paths cannot inject shell. Waits (up to ~10s) for the
/// parent PID to vanish, then execs the new binary. The parent quits
/// first, so a single-instance GTK app never sees two owners of its
/// bus name: spawn-then-quit would make the new copy rendezvous with
/// the still alive primary and exit windowless.
///
/// PID-reuse note: the wait is a bounded ~10s `kill -0` poll on `$2`;
/// a reused PID only delays the relaunch and never targets the wrong
/// process (nothing is signaled, only polled).
pub fn restart_script_text() -> &'static str {
    "i=0; while [ $i -lt 50 ]; do kill -0 \"$2\" 2>/dev/null || break; i=$((i+1)); sleep 0.2; done; exec \"$1\""
}

/// Spawn the restart sleeper detached (own session, null stdio): it
/// execs `bin` once this process is gone, then the caller quits.
pub fn spawn_restart(bin: &Path) -> Result<(), String> {
    use std::os::unix::process::CommandExt;
    use std::process::Stdio;
    // SAFETY: pre_exec only calls setsid(), async-signal-safe.
    unsafe {
        std::process::Command::new("sh")
            .arg("-c")
            .arg(restart_script_text())
            .arg("sh")
            .arg(bin)
            .arg(std::process::id().to_string())
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .current_dir("/")
            .pre_exec(|| {
                libc::setsid();
                Ok(())
            })
            .spawn()
            .map_err(|e| format!("cannot schedule restart: {}", e))?;
    }
    Ok(())
}

/// Set by the install worker as soon as a new version landed, before
/// it releases `UpdateRunGuard`. Process-wide: the pending restart is
/// a property of this run, not of one window.
static INSTALL_PENDING_RESTART: AtomicBool = AtomicBool::new(false);

/// True once an install landed in this process and waits for restart.
pub fn install_pending_restart() -> bool {
    INSTALL_PENDING_RESTART.load(Ordering::SeqCst)
}

/// Called by the worker right after a successful install and before
/// the guard is dropped, so a second attempt cannot slip into the
/// window between guard release and UI notification.
pub fn mark_install_pending_restart() {
    INSTALL_PENDING_RESTART.store(true, Ordering::SeqCst);
}

#[cfg(test)]
pub fn clear_install_pending_restart() {
    INSTALL_PENDING_RESTART.store(false, Ordering::SeqCst);
}

/// Gate for starting a download+install run. A landed install that
/// waits for restart wins over everything; an active run blocks
/// newcomers. Reads the process-wide restart flag, so the ordering
/// cannot be raced by the UI thread. Pure and unit-tested; the UI
/// enforces the answer and shows the message.
pub fn start_run_decision(already_running: bool) -> Result<(), String> {
    if install_pending_restart() {
        return Err("update already installed; restart to finish".into());
    }
    if already_running {
        return Err("update already in progress".into());
    }
    Ok(())
}

/// What to show when a download/install worker ends with an error: a
/// user cancellation is not a failure, so it closes silently (`None`);
/// anything else is handed back for the error dialog. Pure so the UI
/// branch is testable without a display.
pub fn failure_to_show(cancelled: bool, err: &str) -> Option<String> {
    if cancelled {
        None
    } else {
        Some(err.to_string())
    }
}

/// Single-flight guard for download+install runs across all
/// windows. `try_acquire` fails while a run is active; the holder
/// releases on Drop: success, error, cancel, and panic (unwind) all
/// clear it. Fixed staging/.part names are safe because only one run
/// exists at a time.
static UPDATE_RUNNING: AtomicBool = AtomicBool::new(false);

pub struct UpdateRunGuard {
    _private: (),
}

impl UpdateRunGuard {
    pub fn try_acquire() -> Option<Self> {
        UPDATE_RUNNING
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .ok()
            .map(|_| UpdateRunGuard { _private: () })
    }

    /// The install landed: raise the process-wide restart flag and only
    /// then release the guard. The worker calls this instead of a bare
    /// drop, so there is no window where a second attempt could acquire
    /// the guard before the flag is visible.
    pub fn finish_installed(self) {
        mark_install_pending_restart();
        drop(self);
    }
}

impl Drop for UpdateRunGuard {
    fn drop(&mut self) {
        UPDATE_RUNNING.store(false, Ordering::SeqCst);
    }
}

/// Single-flight gate for release checks, so a second check while one
/// is in flight does not spawn a thread or open a dialog. Released on
/// Drop: a success, an error and a panic (unwind) all free it.
static CHECK_RUNNING: AtomicBool = AtomicBool::new(false);

pub struct CheckGuard {
    _private: (),
}

impl CheckGuard {
    /// `Some(guard)` when no check is in flight; `None` otherwise.
    pub fn acquire() -> Option<Self> {
        CHECK_RUNNING
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .ok()
            .map(|_| CheckGuard { _private: () })
    }
}

impl Drop for CheckGuard {
    fn drop(&mut self) {
        CHECK_RUNNING.store(false, Ordering::Release);
    }
}

/// Remove stale updater leftovers in `dir`: `*.part` partials,
/// `staging-*` dirs and `latest.json.*.tmp` files older than
/// `max_age_secs`. Names are matched through `is_sweepable_leftover`
/// and generated by `partial_name` / `staging_name` /
/// `cached_latest_tmp_name` in production. The verified tarball and
/// `latest.json` itself are never touched. Returns the removed count.
///
/// Classification uses `symlink_metadata` (lstat), so a symlink is
/// never taken for the directory it points to: a `staging-*` link to a
/// foreign dir is not a sweep candidate at all, and any leftover link
/// is unlinked in place instead of being followed.
pub fn sweep_stale_downloads(dir: &Path, max_age_secs: u64) -> usize {
    let now = now_unix();
    let mut removed = 0;
    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return 0,
    };
    for e in entries.flatten() {
        let name = e.file_name().to_string_lossy().to_string();
        let path = e.path();
        let meta = match path.symlink_metadata() {
            Ok(m) => m,
            Err(_) => continue,
        };
        // lstat: false for a symlink-to-dir, so the sweep never walks
        // into a linked directory.
        let is_dir = meta.file_type().is_dir();
        if !is_sweepable_leftover(&name, is_dir) {
            continue;
        }
        let old = meta
            .modified()
            .map(|t| {
                now.saturating_sub(
                    t.duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(now),
                )
            })
            .unwrap_or(u64::MAX);
        if old < max_age_secs {
            continue;
        }
        // Only a real directory is removed recursively; everything else
        // (a file or a symlink) is unlinked in place.
        let ok = if is_dir {
            std::fs::remove_dir_all(&path).is_ok()
        } else {
            std::fs::remove_file(&path).is_ok()
        };
        if ok {
            removed += 1;
        }
    }
    removed
}

/// Whether the automatic startup check is due (manual checks bypass).
pub fn should_auto_check(
    enabled: bool,
    last_check_unix: Option<u64>,
    now_unix: u64,
) -> bool {
    if !enabled {
        return false;
    }
    match last_check_unix {
        None => true,
        Some(t) => now_unix.saturating_sub(t) >= AUTO_CHECK_INTERVAL.as_secs(),
    }
}

pub fn now_unix() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

/// Cached `releases/latest` body, so a 304 can still be decided.
/// Written on every 200, read when the API answers NotModified.
pub const CACHED_LATEST_FILE: &str = "latest.json";

pub fn store_cached_latest(dir: &Path, body: &str) -> Result<(), String> {
    static SEQ: AtomicU64 = AtomicU64::new(0);
    std::fs::create_dir_all(dir).map_err(|e| format!("cannot cache release info: {}", e))?;
    // Unique tmp per call (pid + counter): concurrent writers never
    // share a tmp name, and readers only ever see latest.json.
    let tmp = dir.join(cached_latest_tmp_name(
        std::process::id(),
        SEQ.fetch_add(1, Ordering::Relaxed),
    ));
    if let Err(e) = std::fs::write(&tmp, body) {
        let _ = std::fs::remove_file(&tmp);
        return Err(format!("cannot cache release info: {}", e));
    }
    if let Err(e) = std::fs::rename(&tmp, dir.join(CACHED_LATEST_FILE)) {
        let _ = std::fs::remove_file(&tmp);
        return Err(format!("cannot cache release info: {}", e));
    }
    Ok(())
}

pub fn read_cached_latest(dir: &Path) -> Option<ReleaseInfo> {
    let path = dir.join(CACHED_LATEST_FILE);
    // Same 1 MiB cap as the live API: a bloated cache is corrupt.
    if std::fs::metadata(&path).map(|m| m.len()).unwrap_or(0) > MAX_API_BODY {
        return None;
    }
    let body = std::fs::read_to_string(&path).ok()?;
    parse_latest_release(&body).ok()
}

/// Check outcome without any I/O: a newer, non-skipped release is
/// Available; anything else (equal, older, skipped) is UpToDate.
#[derive(Debug, PartialEq, Eq)]
pub enum UpdateDecision {
    Available,
    UpToDate,
}

pub fn decide_update(
    current: &semver::Version,
    info: &ReleaseInfo,
    skip_tag: &str,
) -> UpdateDecision {
    if !needs_update(current, &info.version) {
        return UpdateDecision::UpToDate;
    }
    if !skip_tag.trim().is_empty() && skip_tag.trim() == info.tag {
        return UpdateDecision::UpToDate;
    }
    UpdateDecision::Available
}

#[cfg(test)]
fn test_host_override() -> Option<String> {
    static OVERRIDE: std::sync::Mutex<Option<String>> = std::sync::Mutex::new(None);
    // Initialized once per process: every loopback test uses 127.0.0.1,
    // so sharing one value across parallel tests is race-free.
    let mut guard = OVERRIDE.lock().ok()?;
    if guard.is_none() {
        *guard = Some("127.0.0.1".to_string());
    }
    guard.clone()
}

/// Same mutex-guarded pattern for the API base URL: no set_var races
/// between parallel tests. Only the oversized-body test sets it, and
/// only through the guard below, which restores None on Drop.
#[cfg(test)]
static API_TEST_STATE: std::sync::Mutex<Option<String>> = std::sync::Mutex::new(None);

#[cfg(test)]
fn test_api_override() -> Option<String> {
    match API_TEST_STATE.lock().ok()?.clone() {
        Some(u) if !u.trim().is_empty() => Some(u),
        _ => None,
    }
}

/// RAII holder for the API override: clears it on Drop, panics
/// included. The lock is only ever held for the brief set/clear, so
/// readers inside `api_get_latest` can never deadlock on it. Only one
/// test ever sets an override, which rules out cross-test reads.
#[cfg(test)]
struct ApiUrlGuard;

#[cfg(test)]
impl ApiUrlGuard {
    fn set(url: String) -> Self {
        let mut lock = API_TEST_STATE.lock().unwrap_or_else(|e| e.into_inner());
        *lock = Some(url);
        drop(lock);
        ApiUrlGuard
    }
}

#[cfg(test)]
impl Drop for ApiUrlGuard {
    fn drop(&mut self) {
        let mut lock = API_TEST_STATE.lock().unwrap_or_else(|e| e.into_inner());
        *lock = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE_JSON: &str = r###"{
        "tag_name": "v3.10.0",
        "name": "v3.10.0: test",
        "body": "## Fixed\n\n- something\n",
        "draft": false,
        "prerelease": false,
        "assets": [
            {"name": "corkytux-3.10.0-linux-x86_64.tar.gz",
             "browser_download_url": "https://github.com/x/y/releases/download/v3.10.0/corkytux-3.10.0-linux-x86_64.tar.gz",
             "digest": "sha256:44e1e1f83346a61c953c464b7fdbc6af17aa621052d71f5dcddbedb6bad7f38e",
             "size": 5903577},
            {"name": "corkytux-3.10.0-linux-aarch64.tar.gz",
             "browser_download_url": "https://github.com/x/y/releases/download/v3.10.0/corkytux-3.10.0-linux-aarch64.tar.gz",
             "size": 1234}
        ]
    }"###;

    #[test]
    fn tag_parses_strictly_and_compares() {
        assert_eq!(parse_tag_version("v3.10.0").unwrap().to_string(), "3.10.0");
        assert!(parse_tag_version("v3.10.0").unwrap() > parse_tag_version("v3.9.0").unwrap());
        assert!(parse_tag_version("3.1.2").is_err());
        assert!(parse_tag_version("v3.1").is_err());
        assert!(parse_tag_version("qt-v2.12.1").is_err());
        assert!(parse_tag_version("v3.2.0-beta").is_err());
        assert!(parse_tag_version("v3.1.2+build").is_err());
        assert!(parse_tag_version("").is_err());
    }

    #[test]
    fn parses_latest_json_and_selects_arch_asset() {
        let info = parse_latest_release(SAMPLE_JSON).unwrap();
        assert_eq!(info.tag, "v3.10.0");
        assert_eq!(info.notes, "## Fixed\n\n- something\n");
        let asset = select_asset(&info).unwrap();
        assert_eq!(asset.name, format!("corkytux-3.10.0-linux-{}.tar.gz", tarball_arch()));
        assert_eq!(
            asset.digest_sha256.as_deref(),
            Some("44e1e1f83346a61c953c464b7fdbc6af17aa621052d71f5dcddbedb6bad7f38e")
        );
        assert_eq!(asset.size, 5903577);
    }

    #[test]
    fn asset_selection_rejects_wrong_names() {
        let mut info = parse_latest_release(SAMPLE_JSON).unwrap();
        info.assets.clear();
        assert!(select_asset(&info).is_err());
        info.assets.push(AssetInfo {
            name: "corkytux-3.10.0-linux-other.tar.gz".into(),
            browser_download_url: "https://github.com/x".into(),
            digest_sha256: None,
            size: 1,
        });
        assert!(select_asset(&info).is_err());
    }

    #[test]
    fn release_parse_rejects_drafts_and_prereleases() {
        let draft = SAMPLE_JSON.replace("\"draft\": false", "\"draft\": true");
        assert!(parse_latest_release(&draft).is_err());
        let pre = SAMPLE_JSON.replace("\"prerelease\": false", "\"prerelease\": true");
        assert!(parse_latest_release(&pre).is_err());
        assert!(parse_latest_release("{}").is_err());
    }

    #[test]
    fn update_decision_covers_equal_and_downgrade() {
        let cur = semver::Version::parse("3.1.2").unwrap();
        assert!(!needs_update(&cur, &semver::Version::parse("3.1.2").unwrap()));
        assert!(!needs_update(&cur, &semver::Version::parse("3.0.0").unwrap()));
        assert!(needs_update(&cur, &semver::Version::parse("3.10.0").unwrap()));
    }

    #[test]
    fn decide_update_covers_newer_equal_older_skipped() {
        let cur = semver::Version::parse("3.1.2").unwrap();
        let mut info = parse_latest_release(SAMPLE_JSON).unwrap();
        assert_eq!(decide_update(&cur, &info, ""), UpdateDecision::Available);
        assert_eq!(decide_update(&cur, &info, "v3.10.0"), UpdateDecision::UpToDate);
        info.version = semver::Version::parse("3.1.2").unwrap();
        assert_eq!(decide_update(&cur, &info, ""), UpdateDecision::UpToDate);
        info.version = semver::Version::parse("3.0.0").unwrap();
        assert_eq!(decide_update(&cur, &info, ""), UpdateDecision::UpToDate);
    }

    #[test]
    fn redirect_hosts_use_allowlist() {
        assert!(host_allowed("https://github.com/a/b.tar.gz"));
        assert!(host_allowed(
            "https://release-assets.githubusercontent.com/x?sig=abc"
        ));
        assert!(host_allowed("https://objects.githubusercontent.com/x"));
        assert!(!host_allowed("http://github.com/a"));
        assert!(!host_allowed("https://evilgithub.com/a"));
        assert!(!host_allowed("https://github.com.evil.com/a"));
        assert!(!host_allowed("https://evil.com/a"));
        assert!(!host_allowed("not a url"));
        assert!(!host_allowed("https://github.com@evil.com/a"));
        assert!(!host_allowed("https://user:pass@evil.com/a"));
        assert!(!host_allowed("https://user:pass@github.com/a"));
    }

    #[test]
    fn host_parsing_covers_case_dots_ports_paths() {
        assert!(host_allowed("https://GITHUB.COM/a"));
        assert!(!host_allowed("https://github.com./a"));
        assert!(!host_allowed("https://github.com:444/a"));
        assert!(host_allowed("https://github.com:443/a"));
        assert!(!host_allowed("https://user@github.com/a"));
        assert!(!host_allowed("//evil.com/x"));
        assert!(!host_allowed("https://github.com/a\\b"));
    }

    #[test]
    fn origin_pinning_rejects_foreign_repos() {
        let good = "https://github.com/Matts-lab69/CorkyTux-Launcher/releases/download/v3.10.0/corkytux-3.10.0-linux-x86_64.tar.gz";
        assert!(asset_origin_ok(good, "v3.10.0", "corkytux-3.10.0-linux-x86_64.tar.gz"));
        assert!(!asset_origin_ok(
            "https://github.com/Other/Repo/releases/download/v3.10.0/corkytux-3.10.0-linux-x86_64.tar.gz",
            "v3.10.0",
            "corkytux-3.10.0-linux-x86_64.tar.gz"
        ));
        assert!(!asset_origin_ok(
            "https://github.com/Matts-lab69/CorkyTux-Launcher/releases/download/v9.9.9/corkytux-3.10.0-linux-x86_64.tar.gz",
            "v3.10.0",
            "corkytux-3.10.0-linux-x86_64.tar.gz"
        ));
        assert!(!asset_origin_ok(good, "v3.10.0", "other.tar.gz"));
        // Traversal, encoded dots, query/fragment smuggling, slash.
        assert!(!asset_origin_ok(
            "https://github.com/Matts-lab69/CorkyTux-Launcher/releases/download/v3.10.0/../../../../Other/Repo/x",
            "v3.10.0",
            "x"
        ));
        assert!(!asset_origin_ok(
            "https://github.com/Matts-lab69/CorkyTux-Launcher/releases/download/v3.10.0/%2e%2e",
            "v3.10.0",
            "%2e%2e"
        ));
        assert!(!asset_origin_ok(
            "https://github.com/Matts-lab69/CorkyTux-Launcher/releases/download/v3.10.0/corkytux-3.10.0-linux-x86_64.tar.gz?x=/corkytux-3.10.0-linux-x86_64.tar.gz",
            "v3.10.0",
            "corkytux-3.10.0-linux-x86_64.tar.gz"
        ));
        assert!(!asset_origin_ok(
            "https://github.com/Matts-lab69/CorkyTux-Launcher/releases/download/v3.10.0/corkytux-3.10.0-linux-x86_64.tar.gz#/corkytux-3.10.0-linux-x86_64.tar.gz",
            "v3.10.0",
            "corkytux-3.10.0-linux-x86_64.tar.gz"
        ));
        assert!(!asset_origin_ok(
            "https://github.com/Matts-lab69/CorkyTux-Launcher/releases/download/v3.10.0/corkytux-3.10.0-linux-x86_64.tar.gz/",
            "v3.10.0",
            "corkytux-3.10.0-linux-x86_64.tar.gz"
        ));
        assert!(!asset_origin_ok(
            "https://github.com/Matts-LAB69/CorkyTux-Launcher/releases/download/v3.10.0/corkytux-3.10.0-linux-x86_64.tar.gz",
            "v3.10.0",
            "corkytux-3.10.0-linux-x86_64.tar.gz"
        ));
        assert!(!asset_origin_ok(
            "https://github.com/Matts-lab69/CorkyTux-Launcher/releases/download/v3.10.0/a%2fb",
            "v3.10.0",
            "a%2fb"
        ));
    }

    #[test]
    fn sha_verifies_and_rejects() {
        let dir = std::env::temp_dir().join("corky_upd_sha");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let f = dir.join("data.bin");
        std::fs::write(&f, b"hello updater").unwrap();
        use sha2::Digest;
        let mut h = sha2::Sha256::new();
        h.update(b"hello updater");
        let hex = hex_encode(&h.finalize());
        assert!(verify_artifact(&f, &hex).is_ok());
        assert!(verify_artifact(&f, &format!("sha256:{}", hex)).is_ok());
        assert!(verify_artifact(&f, &"0".repeat(64)).is_err());
        assert!(verify_artifact(&dir.join("missing"), &hex).is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn checksums_txt_matching() {
        let body = "# comment\n\
            44e1e1f83346a61c953c464b7fdbc6af17aa621052d71f5dcddbedb6bad7f38e  \
            corkytux-3.10.0-linux-x86_64.tar.gz\n";
        let got = parse_checksums_txt(body, "corkytux-3.10.0-linux-x86_64.tar.gz").unwrap();
        assert_eq!(got, "44e1e1f83346a61c953c464b7fdbc6af17aa621052d71f5dcddbedb6bad7f38e");
        assert!(parse_checksums_txt(body, "corkytux-9.9.9-linux-x86_64.tar.gz").is_err());
        assert!(parse_checksums_txt("not a checksum line\n", "corkytux-3.10.0-linux-x86_64.tar.gz").is_err());
    }

    #[test]
    fn digest_and_checksums_gates() {
        let asset = AssetInfo {
            name: "corkytux-3.10.0-linux-x86_64.tar.gz".into(),
            browser_download_url: "https://github.com/x".into(),
            digest_sha256: Some("44e1e1f83346a61c953c464b7fdbc6af17aa621052d71f5dcddbedb6bad7f38e".into()),
            size: 1,
        };
        assert_eq!(
            require_digest(&asset).unwrap(),
            "44e1e1f83346a61c953c464b7fdbc6af17aa621052d71f5dcddbedb6bad7f38e"
        );
        let bare = AssetInfo { digest_sha256: None, ..asset.clone() };
        assert!(require_digest(&bare).is_err());
        let body = "44e1e1f83346a61c953c464b7fdbc6af17aa621052d71f5dcddbedb6bad7f38e  corkytux-3.10.0-linux-x86_64.tar.gz\n";
        assert!(checksums_agree(body, "corkytux-3.10.0-linux-x86_64.tar.gz", "44e1e1f83346a61c953c464b7fdbc6af17aa621052d71f5dcddbedb6bad7f38e").is_ok());
        assert!(checksums_agree(body, "corkytux-3.10.0-linux-x86_64.tar.gz", &"0".repeat(64)).is_err());
        assert!(checksums_agree("nope\n", "corkytux-3.10.0-linux-x86_64.tar.gz", "44e1e1f83346a61c953c464b7fdbc6af17aa621052d71f5dcddbedb6bad7f38e").is_err());
    }

    #[test]
    fn checksums_origin_is_pinned_too() {
        assert!(asset_origin_ok(
            "https://github.com/Matts-lab69/CorkyTux-Launcher/releases/download/v3.10.0/checksums.txt",
            "v3.10.0",
            "checksums.txt"
        ));
        assert!(!asset_origin_ok(
            "https://github.com/Other/Repo/releases/download/v3.10.0/checksums.txt",
            "v3.10.0",
            "checksums.txt"
        ));
    }

    #[test]
    fn install_layout_and_gate() {
        let home = PathBuf::from("/home/u");
        let dirs = InstallDirs::fixed(&home);
        assert_eq!(dirs.install_dir, PathBuf::from("/home/u/.local/share/corkytux"));
        assert_eq!(dirs.symlink, PathBuf::from("/home/u/.local/bin/corkytux"));
        assert!(installed_here(
            &PathBuf::from("/home/u/.local/share/corkytux/corkytux"),
            &dirs.install_dir
        ));
        assert!(installed_here(
            &PathBuf::from("/home/u/.local/share/corkytux/corkytux (deleted)"),
            &dirs.install_dir
        ));
        assert!(!installed_here(
            &PathBuf::from("/home/u/src/target/release/corkytux"),
            &dirs.install_dir
        ));
        assert!(!installed_here(&PathBuf::from("/tmp/fake/corkytux"), &dirs.install_dir));
    }

    #[test]
    fn installed_here_resolves_symlinked_home() {
        // HOME reached through a symlink counts as installed; a
        // sibling reached through the same link does not.
        let base = std::env::temp_dir().join("corky_upd_canon");
        let _ = std::fs::remove_dir_all(&base);
        let real = base.join("real-home/.local/share/corkytux");
        std::fs::create_dir_all(&real).unwrap();
        #[cfg(unix)]
        std::os::unix::fs::symlink(base.join("real-home"), base.join("link-home")).unwrap();
        let via_link = base.join("link-home/.local/share/corkytux");
        // A real file forces the canonicalized (not fallback) path.
        std::fs::write(real.join("corkytux"), b"fake").unwrap();
        // A missing exe under the same symlinked parent resolves too.
        let missing = via_link.join("corkytux-missing");
        assert!(!missing.exists());
        let dirs = InstallDirs::fixed(&base.join("link-home"));
        assert!(installed_here(&via_link.join("corkytux"), &dirs.install_dir));
        assert!(installed_here(&missing, &dirs.install_dir));
        assert!(!installed_here(&base.join("link-home/other/corkytux"), &dirs.install_dir));
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn installed_here_deleted_with_existing_target() {
        // The file exists at the plain path and the exe string carries
        // " (deleted)", under a symlinked HOME: still installed.
        let base = std::env::temp_dir().join("corky_upd_canon_both");
        let _ = std::fs::remove_dir_all(&base);
        let real = base.join("real-home/.local/share/corkytux");
        std::fs::create_dir_all(&real).unwrap();
        #[cfg(unix)]
        std::os::unix::fs::symlink(base.join("real-home"), base.join("link-home")).unwrap();
        std::fs::write(real.join("corkytux"), b"fake").unwrap();
        let dirs = InstallDirs::fixed(&base.join("link-home"));
        let plain = base.join("link-home/.local/share/corkytux/corkytux");
        assert!(plain.exists(), "sees the file through the symlink");
        assert!(installed_here(&plain, &dirs.install_dir));
        let gone = base.join("link-home/.local/share/corkytux/corkytux (deleted)");
        assert!(installed_here(&gone, &dirs.install_dir));
        assert!(!installed_here(
            &base.join("link-home/other/corkytux (deleted)"),
            &dirs.install_dir
        ));
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn installed_here_deleted_under_symlink() {
        // Deleted-suffix exe that never existed, under a symlinked HOME.
        let base = std::env::temp_dir().join("corky_upd_canon_del");
        let _ = std::fs::remove_dir_all(&base);
        let real = base.join("real-home/.local/share/corkytux");
        std::fs::create_dir_all(&real).unwrap();
        #[cfg(unix)]
        std::os::unix::fs::symlink(base.join("real-home"), base.join("link-home")).unwrap();
        let dirs = InstallDirs::fixed(&base.join("link-home"));
        let gone = base.join("link-home/.local/share/corkytux/corkytux (deleted)");
        assert!(!gone.exists());
        assert!(installed_here(&gone, &dirs.install_dir));
        assert!(!installed_here(
            &base.join("link-home/other/corkytux (deleted)"),
            &dirs.install_dir
        ));
        let _ = std::fs::remove_dir_all(&base);
    }

    /// Raw tar entry: bypasses the `tar` builder, which itself refuses
    /// to archive `..` paths. Exactly what an attacker ships.
    fn raw_entry(name: &str, link: Option<&str>, content: &[u8]) -> Vec<u8> {
        let mut h = [0u8; 512];
        let nb = name.as_bytes();
        assert!(nb.len() < 100);
        h[..nb.len()].copy_from_slice(nb);
        h[100..108].copy_from_slice(b"0000755\0");
        let size_str = format!("{:011o}\0", content.len());
        h[124..136].copy_from_slice(size_str.as_bytes());
        h[156] = if link.is_some() { b'2' } else { b'0' };
        if let Some(t) = link {
            let tb = t.as_bytes();
            h[157..157 + tb.len()].copy_from_slice(tb);
        }
        h[257..263].copy_from_slice(b"ustar\0");
        h[263..265].copy_from_slice(b"00");
        for b in h.iter_mut().take(156).skip(148) {
            *b = b' ';
        }
        let sum: u32 = h.iter().map(|b| *b as u32).sum();
        let cs = format!("{:06o}\0 ", sum);
        h[148..156].copy_from_slice(cs.as_bytes());
        let mut out = h.to_vec();
        out.extend_from_slice(content);
        while out.len() % 512 != 0 {
            out.push(0);
        }
        out
    }

    fn raw_tar_gz(dir: &Path, name: &str, entries: &[(&str, Option<&str>, &[u8])]) -> PathBuf {
        use std::io::Write;
        let tar_path = dir.join(name);
        let f = std::fs::File::create(&tar_path).unwrap();
        let mut gz = flate2::write::GzEncoder::new(f, flate2::Compression::default());
        for (n, l, c) in entries {
            gz.write_all(&raw_entry(n, *l, c)).unwrap();
        }
        gz.write_all(&[0u8; 1024]).unwrap();
        gz.finish().unwrap();
        tar_path
    }

    #[test]
    fn malicious_tarball_is_rejected() {
        let dir = std::env::temp_dir().join("corky_upd_evil");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        // (entry name, link target, content)
        let evil: Vec<(&str, Option<&str>, &[u8])> = vec![
            ("pkg/../../escape", None, b"x"),
            ("/abs/path", None, b"x"),
            ("pkg/link", Some("corkytux"), b""),
            ("pkg/surprise.sh", None, b"evil"),
        ];
        for (i, e) in evil.iter().enumerate() {
            let tar_path = raw_tar_gz(&dir, &format!("evil{}.tar.gz", i), &[*e]);
            let staging = dir.join(format!("stage{}", i));
            assert!(extract_tarball(&tar_path, &staging).is_err(), "case {} accepted", e.0);
            assert!(!staging.join("escape").exists());
        }
        // A clean tarball with just the binary is accepted.
        let good = raw_tar_gz(&dir, "good.tar.gz", &[("pkg/corkytux", None, b"bin")]);
        let staged = extract_tarball(&good, &dir.join("goodstage")).unwrap();
        assert_eq!(std::fs::read(&staged.binary).unwrap(), b"bin");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn gzip_bomb_is_capped() {
        let dir = std::env::temp_dir().join("corky_upd_bomb");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        // 4 MiB of zeros compresses to ~4 KiB; limits are 1 MiB/file.
        let zeros = vec![0u8; 4 * 1024 * 1024];
        let tar_path = raw_tar_gz(&dir, "bomb.tar.gz", &[("pkg/corkytux", None, &zeros)]);
        let tiny = ExtractLimits { max_total_bytes: 1024 * 1024, max_file_bytes: 1024 * 1024, max_entries: 100 };
        assert!(extract_tarball_limited(&tar_path, &dir.join("stage"), &tiny).is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[cfg(unix)]
    fn chmod_exec(path: &std::path::Path) {
        use std::os::unix::fs::PermissionsExt;
        let mut perm = std::fs::metadata(path).unwrap().permissions();
        perm.set_mode(0o755);
        std::fs::set_permissions(path, perm).unwrap();
    }

    #[cfg(unix)]
    fn assert_exec(path: &std::path::Path) {
        use std::os::unix::fs::PermissionsExt;
        let mode = std::fs::metadata(path).unwrap().permissions().mode();
        assert!(mode & 0o111 != 0, "not executable: {}", path.display());
    }

    fn healthy_fake_binary(dir: &Path, name: &str) -> PathBuf {
        let p = dir.join(name);
        std::fs::write(&p, "#!/bin/sh\necho corkytux 9.9.9\n").unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perm = std::fs::metadata(&p).unwrap().permissions();
            perm.set_mode(0o755);
            std::fs::set_permissions(&p, perm).unwrap();
        }
        p
    }

    #[test]
    fn failed_health_check_keeps_original() {
        let dir = std::env::temp_dir().join("corky_upd_rollback");
        let _ = std::fs::remove_dir_all(&dir);
        let install = dir.join("install");
        std::fs::create_dir_all(&install).unwrap();
        std::fs::write(install.join("corkytux"), b"original-binary").unwrap();
        let staging = dir.join("staging");
        std::fs::create_dir_all(&staging).unwrap();
        // A "binary" that fails --version (false exits 1 with no output).
        std::fs::copy("/bin/false", staging.join("corkytux")).unwrap();
        let staged = Staged {
            binary: staging.join("corkytux"),
            ..Default::default()
        };
        let dirs = InstallDirs {
            install_dir: install.clone(),
            icon_dir: dir.join("icons"),
            desktop_file: dir.join("app.desktop"),
            symlink: dir.join("link"),
        };
        let ver = semver::Version::parse("9.9.9").unwrap();
        assert!(install_if_healthy(&staged, &dirs, &ver).is_err());
        assert_eq!(std::fs::read(install.join("corkytux")).unwrap(), b"original-binary");
        assert!(!install.join("corkytux.new").exists());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn wrong_version_reports_and_installs_nothing() {
        let dir = std::env::temp_dir().join("corky_upd_wrongver");
        let _ = std::fs::remove_dir_all(&dir);
        let install = dir.join("install");
        std::fs::create_dir_all(&install).unwrap();
        std::fs::write(install.join("corkytux"), b"original-binary").unwrap();
        let staging = dir.join("staging");
        std::fs::create_dir_all(&staging).unwrap();
        std::fs::write(staging.join("corkytux"), "#!/bin/sh\necho corkytux 1.0.0\n").unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perm = std::fs::metadata(staging.join("corkytux")).unwrap().permissions();
            perm.set_mode(0o755);
            std::fs::set_permissions(staging.join("corkytux"), perm).unwrap();
        }
        let staged = Staged { binary: staging.join("corkytux"), ..Default::default() };
        let dirs = InstallDirs {
            install_dir: install.clone(),
            icon_dir: dir.join("icons"),
            desktop_file: dir.join("app.desktop"),
            symlink: dir.join("link"),
        };
        let ver = semver::Version::parse("9.9.9").unwrap();
        let err = install_if_healthy(&staged, &dirs, &ver).expect_err("wrong version must fail");
        assert!(err.contains("wrong version"), "got: {}", err);
        assert_eq!(std::fs::read(install.join("corkytux")).unwrap(), b"original-binary");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn failed_helper_install_restores_everything() {
        let dir = std::env::temp_dir().join("corky_upd_rbhelp");
        let _ = std::fs::remove_dir_all(&dir);
        let install = dir.join("install");
        std::fs::create_dir_all(&install).expect("rb-scaffold");
        std::fs::write(install.join("corkytux"), b"orig-bin").expect("rb-scaffold");
        #[cfg(unix)]
        chmod_exec(&install.join("corkytux"));
        std::fs::write(install.join("webdriver_login"), b"orig-helper").expect("rb-scaffold");
        let assets = install.join("assets");
        std::fs::create_dir_all(&assets).expect("rb-scaffold");
        std::fs::write(assets.join("a.txt"), b"orig-asset").expect("rb-scaffold");
        let staging = dir.join("staging");
        std::fs::create_dir_all(&staging).expect("rb-scaffold");
        let bin = healthy_fake_binary(&staging, "corkytux");
        std::fs::write(staging.join("webdriver_login"), b"new-helper").expect("rb-scaffold");
        let assets_new = staging.join("assets");
        std::fs::create_dir_all(&assets_new).expect("rb-scaffold");
        std::fs::write(assets_new.join("a.txt"), b"new-asset").expect("rb-scaffold");
        // Sabotage the atomic temp path, not the destination: the copy
        // to `webdriver_login.new` lands inside this directory and the
        // rename fails, while the helper backup stays valid so rollback
        // restores it with no manual repair.
        std::fs::create_dir_all(install.join("webdriver_login.new")).expect("rb-scaffold");
        let staged = Staged {
            binary: bin,
            helper: Some(staging.join("webdriver_login")),
            assets_dir: Some(assets_new),
            icon: None,
        };
        let dirs = InstallDirs {
            install_dir: install.clone(),
            icon_dir: dir.join("icons"),
            desktop_file: dir.join("app.desktop"),
            symlink: dir.join("link"),
        };
        let ver = semver::Version::parse("9.9.9").unwrap();
        let err = install_if_healthy(&staged, &dirs, &ver).expect_err("helper install must fail");
        assert!(err.contains("Previous version restored"), "got: {}", err);
        assert_eq!(std::fs::read(install.join("corkytux")).expect("rb-scaffold"), b"orig-bin");
        #[cfg(unix)]
        assert_exec(&install.join("corkytux"));
        #[cfg(unix)]
        assert_exec(&install.join("corkytux"));
        assert_eq!(std::fs::read(install.join("webdriver_login")).expect("rb-scaffold"), b"orig-helper");
        assert_eq!(std::fs::read(assets.join("a.txt")).expect("rb-scaffold"), b"orig-asset");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn rollback_combined_error_path() {
        // Destination sabotaged AND no backup possible (original was
        // already a directory at backup time): install fails, binary
        // is still restored, the foreign directory is left alone, and
        // the original error is reported without a phantom rollback
        // failure.
        let dir = std::env::temp_dir().join("corky_upd_rbboth");
        let _ = std::fs::remove_dir_all(&dir);
        let install = dir.join("install");
        std::fs::create_dir_all(&install).expect("rb-scaffold");
        std::fs::write(install.join("corkytux"), b"orig-bin").expect("rb-scaffold");
        std::fs::write(install.join("webdriver_login"), b"orig-helper").expect("rb-scaffold");
        let staging = dir.join("staging");
        std::fs::create_dir_all(&staging).expect("rb-scaffold");
        let bin = healthy_fake_binary(&staging, "corkytux");
        std::fs::write(staging.join("webdriver_login"), b"new-helper").expect("rb-scaffold");
        std::fs::remove_file(install.join("webdriver_login")).expect("rb-scaffold");
        std::fs::create_dir_all(install.join("webdriver_login")).expect("rb-scaffold");
        let staged = Staged {
            binary: bin,
            helper: Some(staging.join("webdriver_login")),
            assets_dir: None,
            icon: None,
        };
        let dirs = InstallDirs {
            install_dir: install.clone(),
            icon_dir: dir.join("icons"),
            desktop_file: dir.join("app.desktop"),
            symlink: dir.join("link"),
        };
        let ver = semver::Version::parse("9.9.9").unwrap();
        let err = install_if_healthy(&staged, &dirs, &ver).expect_err("must fail");
        assert!(err.contains("install failed"), "got: {}", err);
        assert!(err.contains("Previous version restored"), "got: {}", err);
        assert_eq!(std::fs::read(install.join("corkytux")).expect("rb-scaffold"), b"orig-bin");
        assert!(install.join("webdriver_login").is_dir());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn failed_assets_install_restores_everything() {
        let dir = std::env::temp_dir().join("corky_upd_rbassets");
        let _ = std::fs::remove_dir_all(&dir);
        let install = dir.join("install");
        std::fs::create_dir_all(&install).expect("rb-scaffold");
        std::fs::write(install.join("corkytux"), b"orig-bin").expect("rb-scaffold");
        #[cfg(unix)]
        chmod_exec(&install.join("corkytux"));
        std::fs::write(install.join("webdriver_login"), b"orig-helper").expect("rb-scaffold");
        let assets = install.join("assets");
        std::fs::write(&assets, b"blocker").expect("rb-scaffold");
        let staging = dir.join("staging");
        std::fs::create_dir_all(&staging).expect("rb-scaffold");
        let bin = healthy_fake_binary(&staging, "corkytux");
        std::fs::write(staging.join("webdriver_login"), b"new-helper").expect("rb-scaffold");
        let assets_new = staging.join("assets");
        std::fs::create_dir_all(&assets_new).expect("rb-scaffold");
        std::fs::write(assets_new.join("a.txt"), b"new-asset").expect("rb-scaffold");
        // assets_target is a file: no aside-rename, copy_dir fails
        // after binary AND helper were already replaced.
        let staged = Staged {
            binary: bin,
            helper: Some(staging.join("webdriver_login")),
            assets_dir: Some(assets_new),
            icon: None,
        };
        let dirs = InstallDirs {
            install_dir: install.clone(),
            icon_dir: dir.join("icons"),
            desktop_file: dir.join("app.desktop"),
            symlink: dir.join("link"),
        };
        let ver = semver::Version::parse("9.9.9").unwrap();
        let err = install_if_healthy(&staged, &dirs, &ver).expect_err("assets install must fail");
        assert!(err.contains("Previous version restored"), "got: {}", err);
        assert_eq!(std::fs::read(install.join("corkytux")).expect("rb-scaffold"), b"orig-bin");
        #[cfg(unix)]
        assert_exec(&install.join("corkytux"));
        assert_eq!(std::fs::read(install.join("webdriver_login")).expect("rb-scaffold"), b"orig-helper");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn rollback_removes_newly_installed_files() {
        // Helper did not exist before: installed, then assets fail.
        // Rollback must delete the new helper, not just restore old.
        let dir = std::env::temp_dir().join("corky_upd_rbnew");
        let _ = std::fs::remove_dir_all(&dir);
        let install = dir.join("install");
        std::fs::create_dir_all(&install).expect("rb-scaffold");
        std::fs::write(install.join("corkytux"), b"orig-bin").expect("rb-scaffold");
        #[cfg(unix)]
        chmod_exec(&install.join("corkytux"));
        assert!(!install.join("webdriver_login").exists());
        let staging = dir.join("staging");
        std::fs::create_dir_all(&staging).expect("rb-scaffold");
        let bin = healthy_fake_binary(&staging, "corkytux");
        std::fs::write(staging.join("webdriver_login"), b"new-helper").expect("rb-scaffold");
        let assets_new = staging.join("assets");
        std::fs::create_dir_all(&assets_new).expect("rb-scaffold");
        std::fs::write(assets_new.join("a.txt"), b"new-asset").expect("rb-scaffold");
        // Blocker where the fresh assets dir must land.
        std::fs::write(install.join("assets"), b"blocker").expect("rb-scaffold");
        let staged = Staged {
            binary: bin,
            helper: Some(staging.join("webdriver_login")),
            assets_dir: Some(assets_new),
            icon: None,
        };
        let dirs = InstallDirs {
            install_dir: install.clone(),
            icon_dir: dir.join("icons"),
            desktop_file: dir.join("app.desktop"),
            symlink: dir.join("link"),
        };
        let ver = semver::Version::parse("9.9.9").unwrap();
        let err = install_if_healthy(&staged, &dirs, &ver).expect_err("assets install must fail");
        assert!(err.contains("Previous version restored"), "got: {}", err);
        assert_eq!(std::fs::read(install.join("corkytux")).expect("rb-scaffold"), b"orig-bin");
        #[cfg(unix)]
        assert_exec(&install.join("corkytux"));
        assert!(
            !install.join("webdriver_login").exists(),
            "new helper must be gone after rollback"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn missing_binary_is_not_an_update() {
        let dir = std::env::temp_dir().join("corky_upd_nobin");
        let _ = std::fs::remove_dir_all(&dir);
        let install = dir.join("install");
        std::fs::create_dir_all(&install).expect("rb-scaffold");
        let staging = dir.join("staging");
        std::fs::create_dir_all(&staging).expect("rb-scaffold");
        let bin = healthy_fake_binary(&staging, "corkytux");
        let staged = Staged { binary: bin, ..Default::default() };
        let dirs = InstallDirs {
            install_dir: install.clone(),
            icon_dir: dir.join("icons"),
            desktop_file: dir.join("app.desktop"),
            symlink: dir.join("link"),
        };
        let ver = semver::Version::parse("9.9.9").unwrap();
        let err = install_if_healthy(&staged, &dirs, &ver).expect_err("missing binary must fail");
        assert!(err.contains("install manually"), "got: {}", err);
        assert!(!install.join("corkytux").exists());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn symlink_swap_is_atomic_and_selective() {
        let dir = std::env::temp_dir().join("corky_upd_link");
        let _ = std::fs::remove_dir_all(&dir);
        let install = dir.join("install");
        std::fs::create_dir_all(&install).unwrap();
        std::fs::write(install.join("corkytux"), b"orig-bin").unwrap();
        let staging = dir.join("staging");
        std::fs::create_dir_all(&staging).unwrap();
        let bin = healthy_fake_binary(&staging, "corkytux");
        let staged = Staged { binary: bin, ..Default::default() };
        // Case 1: existing symlink elsewhere is swapped atomically.
        let link = dir.join("link");
        #[cfg(unix)]
        std::os::unix::fs::symlink("/nowhere/old", &link).unwrap();
        let dirs = InstallDirs {
            install_dir: install.clone(),
            icon_dir: dir.join("icons"),
            desktop_file: dir.join("app.desktop"),
            symlink: link.clone(),
        };
        let ver = semver::Version::parse("9.9.9").unwrap();
        assert!(install_if_healthy(&staged, &dirs, &ver).is_ok());
        #[cfg(unix)]
        assert_eq!(std::fs::read_link(&link).unwrap(), install.join("corkytux"));
        assert!(!dir.join("link.new").exists(), "no orphan tmp link");
        // Case 2: a real file at the link path is never touched.
        let plain = dir.join("plainlink");
        std::fs::write(&plain, b"do not touch").unwrap();
        let dirs2 = InstallDirs {
            install_dir: install.clone(),
            icon_dir: dir.join("icons"),
            desktop_file: dir.join("app2.desktop"),
            symlink: plain.clone(),
        };
        let log = install_if_healthy(&staged, &dirs2, &ver).expect("real file must not fail install");
        assert!(log.iter().any(|l| l.contains("Symlink skipped")), "log: {:?}", log);
        assert_eq!(std::fs::read(&plain).unwrap(), b"do not touch");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn healthy_install_keeps_link_and_desktop() {
        let dir = std::env::temp_dir().join("corky_upd_keep");
        let _ = std::fs::remove_dir_all(&dir);
        let install = dir.join("install");
        std::fs::create_dir_all(&install).unwrap();
        std::fs::write(install.join("corkytux"), b"orig-bin").unwrap();
        #[cfg(unix)]
        chmod_exec(&install.join("corkytux"));
        let link = dir.join("link");
        #[cfg(unix)]
        std::os::unix::fs::symlink(install.join("corkytux"), &link).unwrap();
        let desk = dir.join("app.desktop");
        let desk_content = format!("[Desktop Entry]\nExec={}\n", install.join("corkytux").display());
        std::fs::write(&desk, &desk_content).unwrap();
        let staging = dir.join("staging");
        std::fs::create_dir_all(&staging).unwrap();
        let bin = healthy_fake_binary(&staging, "corkytux");
        let staged = Staged { binary: bin, ..Default::default() };
        let dirs = InstallDirs {
            install_dir: install.clone(),
            icon_dir: dir.join("icons"),
            desktop_file: desk.clone(),
            symlink: link.clone(),
        };
        let ver = semver::Version::parse("9.9.9").unwrap();
        assert!(install_if_healthy(&staged, &dirs, &ver).is_ok());
        #[cfg(unix)]
        assert_exec(&install.join("corkytux"));
        #[cfg(unix)]
        assert_eq!(std::fs::read_link(&link).unwrap(), install.join("corkytux"));
        assert_eq!(std::fs::read_to_string(&desk).unwrap(), desk_content);
        assert!(install.join("corkytux").exists());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn restart_parts_are_sound() {
        let script = restart_script_text();
        assert!(!script.contains("seq"), "no seq dependency: {}", script);
        assert!(script.contains("while"), "counter loop: {}", script);
        assert!(script.contains("kill -0 \"$2\""), "waits on $2: {}", script);
        assert!(script.contains("exec \"$1\""), "execs $1: {}", script);
        assert!(!script.contains("/h/"), "no interpolated paths");
        assert!(!process_with_cmdline_running(""));
        // Unique marker per run: stale sleepers from killed runs must
        // never match (substring scan is process-global).
        static MARKER_SEQ: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let marker = format!(
            "CORKYTEST-{}-{}",
            std::process::id(),
            MARKER_SEQ.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        );
        // Absolute paths: the test PATH is not guaranteed to
        // contain /usr/bin, so never rely on exec search here. No
        // `exec -a` either: not portable to dash. The marker lives in
        // the `sh -c` script text, which vanishes with the parent.
        let mut child = std::process::Command::new("/bin/sh")
            .args(["-c", &format!("/bin/sleep 5; : {}", marker)])
            .spawn()
            .expect("sh available");
        let mut seen = false;
        for _ in 0..160 {
            if process_with_cmdline_running(&marker) {
                seen = true;
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(25));
        }
        assert!(seen, "marker process never appeared");
        child.kill().ok();
        let _ = child.wait();
        let mut seen_gone = false;
        for _ in 0..50 {
            if !process_with_cmdline_running(&marker) {
                seen_gone = true;
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(50));
        }
        assert!(seen_gone);
    }

    fn serve_once(response: Vec<u8>) -> String {
        let ln = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = ln.local_addr().unwrap();
        std::thread::spawn(move || {
            if let Ok((mut s, _)) = ln.accept() {
                use std::io::{Read, Write};
                let mut buf = [0u8; 4096];
                let _ = s.read(&mut buf);
                let _ = s.write_all(&response);
                let _ = s.flush();
                std::thread::sleep(std::time::Duration::from_millis(400));
            }
        });
        format!("http://{}:{}/", addr.ip(), addr.port())
    }

    #[test]
    fn download_guards_against_loopback_fixtures() {
        // Loopback fixtures: real HTTP through the real guarded stack,
        // served by a one-shot thread-local server. No outside network.
        // The 127.0.0.1 override is a mutex-guarded static (see
        // test_host_override), shared read-only by parallel tests.
        // 1. Oversized body: server declares 64 bytes, sends 4 KiB.
        let big = format!(
            "HTTP/1.1 200 OK\r\nContent-Length: 4096\r\nConnection: close\r\n\r\n{}",
            "B".repeat(4096)
        );
        let url = serve_once(big.into_bytes());
        let dir = std::env::temp_dir().join("corky_upd_cap");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let cancel = Arc::new(AtomicBool::new(false));
        let err = download_asset(&url, &dir.join("f.part"), 64, &|_, _| {}, &cancel)
            .expect_err("oversize body must abort");
        assert!(err.contains("exceeds size limit"), "got: {}", err);
        assert!(!dir.join("f.part").exists());
        // 2. Evil redirect: 302 to https://127.0.0.2 (not allowlisted).
        let redir = "HTTP/1.1 302 Found\r\nLocation: https://127.0.0.2:9/x\r\nContent-Length: 0\r\nConnection: close\r\n\r\n".to_string();
        let url2 = serve_once(redir.into_bytes());
        let err2 = download_asset(&url2, &dir.join("g.part"), 64, &|_, _| {}, &cancel)
            .expect_err("evil redirect must abort");
        assert!(err2.contains("not allowed"), "got: {}", err2);
        // 3. Pre-cancelled: nothing is written.
        cancel.store(true, Ordering::Relaxed);
        let err3 = download_asset(&url, &dir.join("h.part"), 64, &|_, _| {}, &cancel)
            .expect_err("cancelled download must abort");
        assert!(err3.contains("cancelled"), "got: {}", err3);
        assert!(!dir.join("h.part").exists());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn cached_latest_decides_a_304() {
        // 304 scenario from the report: cache holds v3.10.0 while the
        // running version is 3.1.2 -> update available, not up to date.
        let dir = std::env::temp_dir().join("corky_upd_cache");
        let _ = std::fs::remove_dir_all(&dir);
        assert!(read_cached_latest(&dir).is_none());
        store_cached_latest(&dir, SAMPLE_JSON).unwrap();
        let info = read_cached_latest(&dir).expect("cached release parses");
        assert_eq!(info.tag, "v3.10.0");
        let cur = semver::Version::parse("3.1.2").unwrap();
        assert!(needs_update(&cur, &info.version));
        assert_eq!(decide_update(&cur, &info, ""), UpdateDecision::Available);
        std::fs::write(dir.join(CACHED_LATEST_FILE), "not json").unwrap();
        assert!(read_cached_latest(&dir).is_none());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn desktop_dir_means_skipped_not_failed() {
        // desktop_file is a directory: entry work fails, the binary
        // install still lands, the log carries the skip, and no
        // partial entry or orphan tmp remains.
        let dir = std::env::temp_dir().join("corky_upd_deskdir");
        let _ = std::fs::remove_dir_all(&dir);
        let install = dir.join("install");
        std::fs::create_dir_all(&install).unwrap();
        std::fs::write(install.join("corkytux"), b"orig-bin").unwrap();
        let staging = dir.join("staging");
        std::fs::create_dir_all(&staging).unwrap();
        let bin = healthy_fake_binary(&staging, "corkytux");
        let staged = Staged { binary: bin, ..Default::default() };
        let deskdir = dir.join("app.desktop");
        std::fs::create_dir_all(&deskdir).unwrap();
        let dirs = InstallDirs {
            install_dir: install.clone(),
            icon_dir: dir.join("icons"),
            desktop_file: deskdir.clone(),
            symlink: dir.join("link"),
        };
        let ver = semver::Version::parse("9.9.9").unwrap();
        let log = install_if_healthy(&staged, &dirs, &ver).expect("desktop failure is not fatal");
        assert!(log.iter().any(|l| l.contains("Desktop entry skipped")), "log: {:?}", log);
        let content = std::fs::read(install.join("corkytux")).unwrap();
        assert!(
            content.windows(b"corkytux 9.9.9".len()).any(|w| w == b"corkytux 9.9.9"),
            "new binary in place"
        );
        #[cfg(unix)]
        assert_exec(&install.join("corkytux"));
        assert!(deskdir.is_dir(), "sabotage dir untouched");
        let orphans: Vec<_> = std::fs::read_dir(&dir)
            .unwrap()
            .filter_map(|e| e.ok())
            .filter(|e| {
                let n = e.file_name().to_string_lossy().to_string();
                n.ends_with(".new") || n.ends_with(".part")
            })
            .collect();
        assert!(orphans.is_empty(), "orphan tmp files: {:?}", orphans);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn desktop_newline_paths_are_refused() {
        let dir = std::env::temp_dir().join("corky_upd_newline");
        let _ = std::fs::remove_dir_all(&dir);
        let install = dir.join("install");
        std::fs::create_dir_all(&install).unwrap();
        std::fs::write(install.join("corkytux"), b"orig-bin").unwrap();
        let evil_icons = dir.join("ico\nns");
        std::fs::create_dir_all(&evil_icons).unwrap();
        let staging = dir.join("staging");
        std::fs::create_dir_all(&staging).unwrap();
        let bin = healthy_fake_binary(&staging, "corkytux");
        std::fs::write(staging.join("corkytux.png"), b"icon").unwrap();
        let staged = Staged {
            binary: bin,
            helper: None,
            assets_dir: None,
            icon: Some(staging.join("corkytux.png")),
        };
        let dirs = InstallDirs {
            install_dir: install.clone(),
            icon_dir: evil_icons,
            desktop_file: dir.join("app.desktop"),
            symlink: dir.join("link"),
        };
        let ver = semver::Version::parse("9.9.9").unwrap();
        let err = install_if_healthy(&staged, &dirs, &ver).expect_err("newline path must fail");
        assert!(err.contains("newline"), "got: {}", err);
        // Pre-flight refusal: original intact, no backups taken.
        assert_eq!(std::fs::read(install.join("corkytux")).unwrap(), b"orig-bin");
        assert!(!dir.join("app.desktop").exists());
        let leftovers: Vec<_> = std::fs::read_dir(&install)
            .unwrap()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_name().to_string_lossy().contains(".bak-"))
            .collect();
        assert!(leftovers.is_empty(), "no backups on pre-flight refusal");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn layer_two_escapes_match_literally() {
        // Layer-2 string escapes with hand-written file bytes (Rust
        // doubles each file backslash). Unquoted, a decoded space/tab
        // newline/CR ends the token (INI semantics), so only the
        // truncated path matches; quoted, the full value matches.
        let bin_nl = PathBuf::from("/h/a\nb");
        assert!(!desktop_exec_matches("Exec=/h/a\\nb\n", &bin_nl));
        assert!(desktop_exec_matches("Exec=/h/a\\nb\n", &PathBuf::from("/h/a")));
        assert!(!desktop_exec_matches("Exec=/h/a\\tb\n", &PathBuf::from("/h/a\tb")));
        assert!(!desktop_exec_matches("Exec=/h/a\\rb\n", &PathBuf::from("/h/a\rb")));
        assert!(!desktop_exec_matches("Exec=/h/a\\sb\n", &PathBuf::from("/h/a b")));
        assert!(!desktop_exec_matches("Exec=/h/a\\qb\n", &PathBuf::from("/h/aqb")));
        assert!(!desktop_exec_matches("Exec=/h/ab\\\n", &PathBuf::from("/h/ab\\")));
        // Quoted layer-2 still applies inside quotes.
        assert!(desktop_exec_matches("Exec=\"/h/a\\nb\"\n", &bin_nl));
    }

    #[test]
    fn desktop_exec_matching() {
        let bin = PathBuf::from("/h/.local/share/corkytux/corkytux");
        assert!(desktop_exec_matches("Exec=/h/.local/share/corkytux/corkytux\n", &bin));
        assert!(desktop_exec_matches("Exec=/h/.local/share/corkytux/corkytux %U\n", &bin));
        assert!(desktop_exec_matches("Exec=\"/h/.local/share/corkytux/corkytux\" %F\n", &bin));
        assert!(!desktop_exec_matches("Exec=/other/corkytux\n", &bin));
        assert!(!desktop_exec_matches("Name=CorkyTux\n", &bin));
        assert_eq!(desktop_quote_exec(&bin), "/h/.local/share/corkytux/corkytux");
        let spaced = PathBuf::from("/h/my games/corkytux");
        assert_eq!(desktop_quote_exec(&spaced), "\"/h/my games/corkytux\"");
        let tricky = PathBuf::from("/h/a$b`c\\d e/x");
        assert_eq!(
            desktop_quote_exec(&tricky),
            "\"/h/a\\\\$b\\\\`c\\\\\\\\d e/x\""
        );
        assert_eq!(
            desktop_icon_value(&PathBuf::from("/h/ic\\ons")),
            "/h/ic\\\\ons"
        );
        // Literal vectors straight from the spec layers (Rust doubles
        // each file backslash): literal \ is \\\\ in file, $ is \\$,
        // " is \\", ` is \\`, % is %%.
        assert_eq!(
            desktop_quote_exec(&PathBuf::from("/h/a\\b c/x")),
            "\"/h/a\\\\\\\\b c/x\""
        );
        assert_eq!(
            desktop_quote_exec(&PathBuf::from("/h/a$b c/x")),
            "\"/h/a\\\\$b c/x\""
        );
        assert_eq!(
            desktop_quote_exec(&PathBuf::from("/h/100% c/x")),
            "\"/h/100%% c/x\""
        );
        assert_eq!(
            desktop_quote_exec(&PathBuf::from("/h/a\"b c/x")),
            "\"/h/a\\\\\"b c/x\""
        );
        assert_eq!(
            desktop_quote_exec(&PathBuf::from("/h/a`b c/x")),
            "\"/h/a\\\\`b c/x\""
        );
        // Every reserved character forces quoting.
        for special in ["a b", "a\tb", "a&b", "a;b", "a'b", "a(b)", "a*b", "a?b", "a#b"] {
            let p = PathBuf::from(format!("/h/{}/x", special));
            let q = desktop_quote_exec(&p);
            assert!(q.starts_with('"') && q.ends_with('"'), "unquoted: {}", q);
        }
        // Round-trip: write -> matches, for hostile names.
        for raw in [
            "/h/my games/corkytux",
            "/h/a$b/corkytux",
            "/h/a\\b/corkytux",
            "/h/100%/corkytux",
            "/h/a`b&c/corkytux",
            "/h/a b%c/d$e\\f/corkytux",
            "/h/a \"b/corkytux",
            "/h/a `b/corkytux",
        ] {
            let p = PathBuf::from(raw);
            let line = format!("Exec={}\n", desktop_quote_exec(&p));
            assert!(desktop_exec_matches(&line, &p), "round-trip failed: {}", line);
        }
    }

    #[test]
    fn api_body_and_cache_capped() {
        // Live-shaped oversized body through loopback: refused past 1 MiB.
        let big = format!(
            "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
            MAX_API_BODY + 16,
            "J".repeat((MAX_API_BODY + 16) as usize)
        );
        let _guard = ApiUrlGuard::set(serve_once(big.into_bytes()));
        let err = api_get_latest(None).expect_err("oversized API body must abort");
        assert!(err.contains("1 MiB"), "got: {}", err);
        // Oversized cache file reads as corrupt, never parses.
        let dir = std::env::temp_dir().join("corky_upd_bigcache");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join(CACHED_LATEST_FILE), "J".repeat((MAX_API_BODY + 16) as usize)).unwrap();
        assert!(read_cached_latest(&dir).is_none());
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Tests that touch the process-wide update flags share this lock:
    /// `cargo test` runs them in parallel.
    static UPDATE_STATE_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

    fn update_state_lock() -> std::sync::MutexGuard<'static, ()> {
        UPDATE_STATE_LOCK.lock().unwrap_or_else(|e| e.into_inner())
    }

    #[test]
    fn start_run_decision_orders_pending_over_running() {
        let _lock = update_state_lock();
        clear_install_pending_restart();
        assert!(start_run_decision(false).is_ok());
        mark_install_pending_restart();
        assert_eq!(
            start_run_decision(false).unwrap_err(),
            "update already installed; restart to finish"
        );
        assert_eq!(
            start_run_decision(true).unwrap_err(),
            "update already installed; restart to finish"
        );
        clear_install_pending_restart();
        assert_eq!(
            start_run_decision(true).unwrap_err(),
            "update already in progress"
        );
    }

    #[test]
    fn worker_marks_restart_before_guard_release() {
        let _lock = update_state_lock();
        clear_install_pending_restart();
        let guard = UpdateRunGuard::try_acquire().expect("no run active");
        // Worker path: an install landed, so the guard's own
        // finish_installed() raises the flag and then releases.
        guard.finish_installed();
        // Guard is gone, but the new attempt still sees the pending restart.
        assert_eq!(
            start_run_decision(false).unwrap_err(),
            "update already installed; restart to finish"
        );
        assert!(UpdateRunGuard::try_acquire().is_some(), "guard released");
        clear_install_pending_restart();
    }

    #[test]
    fn update_guard_is_single_flight() {
        let _lock = update_state_lock();
        assert!(UpdateRunGuard::try_acquire().is_some());
        drop(UpdateRunGuard::try_acquire().expect("released after drop"));
        // Second holder while the first lives: refused.
        let g = UpdateRunGuard::try_acquire().expect("first acquires");
        assert!(UpdateRunGuard::try_acquire().is_none());
        drop(g);
        assert!(UpdateRunGuard::try_acquire().is_some());
        // Panic releases too.
        let r = std::panic::catch_unwind(|| {
            let _h = UpdateRunGuard::try_acquire().expect("re-acquires");
            panic!("boom");
        });
        assert!(r.is_err());
        assert!(UpdateRunGuard::try_acquire().is_some());
    }

    #[test]
    fn cached_latest_concurrent_stores_stay_clean() {
        // 8 threads store at once: the final file parses and no tmp
        // remains. Last writer wins; every body is valid JSON.
        let dir = std::env::temp_dir().join("corky_upd_conc");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let mut handles = Vec::new();
        for _ in 0..8 {
            handles.push(std::thread::spawn(|| {
                store_cached_latest(
                    &std::env::temp_dir().join("corky_upd_conc"),
                    SAMPLE_JSON,
                )
                .unwrap();
            }));
        }
        for h in handles {
            h.join().unwrap();
        }
        assert!(read_cached_latest(&dir).is_some());
        let tmps: Vec<_> = std::fs::read_dir(&dir)
            .unwrap()
            .filter_map(|e| e.ok())
            .filter(|e| {
                let n = e.file_name().to_string_lossy().to_string();
                n.ends_with(".tmp") || n.ends_with(".new")
            })
            .collect();
        assert!(tmps.is_empty(), "leftover tmps: {:?}", tmps);
        let _ = std::fs::remove_dir_all(&dir);
    }

    fn set_age_hours(path: &std::path::Path, hours: u64, flags: i32) {
        use std::os::unix::ffi::OsStrExt;
        let c = std::ffi::CString::new(path.as_os_str().as_bytes()).unwrap();
        let old = libc::timespec {
            tv_sec: (self_update_now().saturating_sub(hours * 3600)) as libc::time_t,
            tv_nsec: 0,
        };
        let times = [old, old];
        let r = unsafe { libc::utimensat(libc::AT_FDCWD, c.as_ptr(), times.as_ptr(), flags) };
        assert_eq!(r, 0, "utimensat failed");
    }

    fn age_hours(path: &std::path::Path, hours: u64) {
        set_age_hours(path, hours, 0);
    }

    /// Age the link itself, never its target (AT_SYMLINK_NOFOLLOW).
    #[cfg(unix)]
    fn age_link_hours(path: &std::path::Path, hours: u64) {
        set_age_hours(path, hours, libc::AT_SYMLINK_NOFOLLOW);
    }

    fn self_update_now() -> u64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0)
    }

    #[test]
    fn backups_are_unique_within_one_second() {
        let dir = std::env::temp_dir().join("corky_upd_backup_seq");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let bin = dir.join("corkytux");
        std::fs::write(&bin, b"orig").unwrap();
        // Two installs in the same wall-clock second must not collide.
        let a = backup_file(&bin).unwrap();
        let b = backup_file(&bin).unwrap();
        assert_ne!(a, b, "backup names collided: {:?}", a);
        assert_eq!(std::fs::read(&a).unwrap(), b"orig");
        assert_eq!(std::fs::read(&b).unwrap(), b"orig");
        let n = std::fs::read_dir(&dir)
            .unwrap()
            .flatten()
            .filter(|e| e.file_name().to_string_lossy().contains(".bak-"))
            .count();
        assert_eq!(n, 2);
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Install fixture with a healthy staged binary and a staged assets
    /// dir. `assets_blocker` writes a regular file where the assets dir
    /// must land, so the assets copy fails after the binary replaced.
    fn assets_fixture(
        name: &str,
        assets_blocker: bool,
    ) -> (PathBuf, InstallDirs, Staged, semver::Version) {
        let dir = std::env::temp_dir().join(name);
        let _ = std::fs::remove_dir_all(&dir);
        let install = dir.join("install");
        std::fs::create_dir_all(&install).unwrap();
        std::fs::write(install.join("corkytux"), b"orig-bin").unwrap();
        let assets = install.join("assets");
        if assets_blocker {
            std::fs::write(&assets, b"blocker").unwrap();
        } else {
            std::fs::create_dir_all(&assets).unwrap();
            std::fs::write(assets.join("a.txt"), b"v0").unwrap();
        }
        let staging = dir.join("staging");
        std::fs::create_dir_all(&staging).unwrap();
        let bin = healthy_fake_binary(&staging, "corkytux");
        let assets_new = staging.join("assets");
        std::fs::create_dir_all(&assets_new).unwrap();
        std::fs::write(assets_new.join("a.txt"), b"new").unwrap();
        let staged = Staged { binary: bin, assets_dir: Some(assets_new), ..Default::default() };
        let dirs = InstallDirs {
            install_dir: install,
            icon_dir: dir.join("icons"),
            desktop_file: dir.join("app.desktop"),
            symlink: dir.join("link"),
        };
        let ver = semver::Version::parse("9.9.9").unwrap();
        (dir, dirs, staged, ver)
    }

    fn assets_bak_names(install: &Path) -> Vec<String> {
        let mut names: Vec<String> = std::fs::read_dir(install)
            .unwrap()
            .flatten()
            .map(|e| e.file_name().to_string_lossy().to_string())
            .filter(|n| n.starts_with(ASSETS_BAK_PREFIX))
            .collect();
        names.sort();
        names
    }

    #[test]
    fn assets_backups_are_unique_within_one_second() {
        let (dir, dirs, staged, ver) = assets_fixture("corky_upd_assets_seq", false);
        // Two installs back to back: the aside name is `assets.bak-<secs>-
        // <counter>`, so even two installs in the same second differ.
        install_if_healthy(&staged, &dirs, &ver).expect("first install");
        install_if_healthy(&staged, &dirs, &ver).expect("second install");
        let names = assets_bak_names(&dirs.install_dir);
        assert_eq!(names.len(), 2, "assets backups: {:?}", names);
        assert_ne!(names[0], names[1], "aside names collided");
        for n in &names {
            // Built by the production constructor, not a literal.
            assert!(n.starts_with(ASSETS_BAK_PREFIX), "unexpected name: {}", n);
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn prune_keeps_oldest_and_newest_after_five_installs() {
        let (dir, dirs, staged, ver) = assets_fixture("corky_upd_assets_prune", false);
        install_if_healthy(&staged, &dirs, &ver).expect("install 1");
        let first = assets_bak_names(&dirs.install_dir);
        assert_eq!(first.len(), 1);
        let original = first[0].clone();
        for _ in 0..4 {
            install_if_healthy(&staged, &dirs, &ver).expect("install");
        }
        let names = assets_bak_names(&dirs.install_dir);
        // Oldest plus the two newest survive; the two in between are gone.
        assert_eq!(names.len(), BACKUP_KEEP_NEWEST + 1, "kept: {:?}", names);
        assert_eq!(names[0], original, "oldest (the original) must survive");
        assert!(dirs.install_dir.join(&original).exists(), "original pruned");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn failed_install_does_not_prune_backups() {
        let (dir, dirs, staged, ver) = assets_fixture("corky_upd_assets_noprune", true);
        // Five pre-existing backups: a prune would drop the middle two.
        let mut seeded: Vec<PathBuf> = Vec::new();
        for i in 0..5u64 {
            let p = dirs.install_dir.join(format!("corkytux.bak-1000-{:06}", i));
            std::fs::write(&p, b"old").unwrap();
            seeded.push(p);
        }
        let err = install_if_healthy(&staged, &dirs, &ver).expect_err("assets blocker fails");
        assert!(err.contains("Previous version restored"), "got: {}", err);
        for p in &seeded {
            assert!(p.exists(), "failed install pruned {}", p.display());
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn cache_errors_keep_the_original_io_error() {
        let path = std::env::temp_dir().join("corky_upd_cache_err");
        let _ = std::fs::remove_dir_all(&path);
        // A regular file where the cache dir should be: create_dir_all
        // fails with AlreadyExists, and that cause must survive.
        std::fs::write(&path, b"not a dir").unwrap();
        let err = store_cached_latest(&path, SAMPLE_JSON).unwrap_err();
        assert!(err.contains("cannot cache release info"), "got: {}", err);
        assert!(
            err.contains("os error 17"),
            "lost io cause: {}",
            err
        );
        let _ = std::fs::remove_file(&path);
    }

    #[cfg(unix)]
    #[test]
    fn cache_write_reports_permission_denied() {
        use std::os::unix::fs::PermissionsExt;
        if unsafe { libc::geteuid() } == 0 {
            return; // root ignores mode bits
        }
        let dir = std::env::temp_dir().join("corky_upd_cache_ro");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let mut perm = std::fs::metadata(&dir).unwrap().permissions();
        perm.set_mode(0o555);
        std::fs::set_permissions(&dir, perm).unwrap();
        let err = store_cached_latest(&dir, SAMPLE_JSON).unwrap_err();
        let mut perm = std::fs::metadata(&dir).unwrap().permissions();
        perm.set_mode(0o755);
        std::fs::set_permissions(&dir, perm).unwrap();
        assert!(err.contains("cannot cache release info"), "got: {}", err);
        assert!(
            err.contains("os error 13"),
            "lost io cause: {}",
            err
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn sweep_removes_only_stale_orphans() {
        let dir = std::env::temp_dir().join("corky_upd_sweep");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        // Names come from the production constructors, never literals.
        let old_part = partial_name("corkytux-9.9.9-linux-x86_64.tar.gz");
        let old_staging = staging_name("v9.9.9");
        let old_tmp = cached_latest_tmp_name(std::process::id(), 7);
        let fresh_part = partial_name("fresh.tar.gz");
        let fresh_staging = staging_name("v9.9.10");
        std::fs::write(dir.join(&old_part), b"p").unwrap();
        std::fs::create_dir_all(dir.join(&old_staging)).unwrap();
        std::fs::write(dir.join(&old_tmp), b"t").unwrap();
        std::fs::write(dir.join(&fresh_part), b"f").unwrap();
        std::fs::create_dir_all(dir.join(&fresh_staging)).unwrap();
        std::fs::write(dir.join("keep.tar.gz"), b"k").unwrap();
        std::fs::write(dir.join(CACHED_LATEST_FILE), b"{}").unwrap();
        // Age the orphans 2h (mtime is what counts; threshold is 1h).
        age_hours(&dir.join(&old_part), 2);
        age_hours(&dir.join(&old_staging), 2);
        age_hours(&dir.join(&old_tmp), 2);
        assert_eq!(sweep_stale_downloads(&dir, SWEEP_MAX_AGE_SECS), 3);
        assert!(!dir.join(&old_part).exists());
        assert!(!dir.join(&old_staging).exists());
        assert!(!dir.join(&old_tmp).exists());
        assert!(dir.join(&fresh_part).exists());
        assert!(dir.join(&fresh_staging).exists());
        assert!(dir.join("keep.tar.gz").exists());
        assert!(dir.join(CACHED_LATEST_FILE).exists());
        assert_eq!(sweep_stale_downloads(&dir, SWEEP_MAX_AGE_SECS), 0);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[cfg(unix)]
    #[test]
    fn sweep_never_follows_symlink_targets() {
        let dir = std::env::temp_dir().join("corky_upd_sweep_link");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        // Foreign tree the sweep must never walk into or delete.
        let foreign = dir.join("foreign");
        std::fs::create_dir_all(&foreign).unwrap();
        std::fs::write(foreign.join("keep.txt"), b"precious").unwrap();
        let link_dir = dir.join(staging_name("v9.9.9"));
        let link_part = dir.join(partial_name("evil.tar.gz"));
        std::os::unix::fs::symlink(&foreign, &link_dir).unwrap();
        std::os::unix::fs::symlink(foreign.join("keep.txt"), &link_part).unwrap();
        // Age the links themselves, never their targets.
        age_link_hours(&link_dir, 2);
        age_link_hours(&link_part, 2);
        sweep_stale_downloads(&dir, SWEEP_MAX_AGE_SECS);
        assert!(foreign.is_dir(), "foreign dir removed via symlink");
        assert_eq!(
            std::fs::read(foreign.join("keep.txt")).unwrap(),
            b"precious",
            "foreign file followed and clobbered"
        );
        // The dir link is not a sweep candidate (lstat says symlink,
        // not dir), so it is left untouched and never recursed into.
        assert!(std::fs::symlink_metadata(&link_dir).unwrap().file_type().is_symlink());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn select_asset_rejects_zero_size() {
        let mut info = parse_latest_release(SAMPLE_JSON).unwrap();
        info.assets.clear();
        info.assets.push(AssetInfo {
            name: expected_asset_name(&info.version),
            browser_download_url: "https://github.com/x".into(),
            digest_sha256: Some("ab".repeat(32)),
            size: 0,
        });
        let err = select_asset(&info).expect_err("size 0 must be refused");
        assert!(err.contains("published size"), "got: {}", err);
    }

    #[test]
    fn download_cap_is_hard_bounded() {
        assert_eq!(download_cap(0, MAX_DOWNLOAD_BYTES), MAX_DOWNLOAD_BYTES);
        assert_eq!(download_cap(1234, MAX_DOWNLOAD_BYTES), 1234);
        assert_eq!(
            download_cap(MAX_DOWNLOAD_BYTES, MAX_DOWNLOAD_BYTES),
            MAX_DOWNLOAD_BYTES
        );
        assert_eq!(
            download_cap(MAX_DOWNLOAD_BYTES + 1, MAX_DOWNLOAD_BYTES),
            MAX_DOWNLOAD_BYTES
        );
        // A smaller per-call hard cap wins even against a larger or
        // absent published size (checksums.txt uses CHECKSUMS_MAX_BYTES).
        assert_eq!(download_cap(0, CHECKSUMS_MAX_BYTES), CHECKSUMS_MAX_BYTES);
        assert_eq!(
            download_cap(MAX_DOWNLOAD_BYTES, CHECKSUMS_MAX_BYTES),
            CHECKSUMS_MAX_BYTES
        );
        assert_eq!(download_cap(4096, CHECKSUMS_MAX_BYTES), 4096);
    }

    #[test]
    fn download_size_zero_uses_the_hard_cap() {
        // Published size 0: the cap falls back to the 1 MiB hard cap, so a
        // small body still lands (and the byte ceiling is not disabled)
        // without requiring hundreds of MiB of free space.
        let body = b"hello".to_vec();
        let mut resp = format!(
            "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
            body.len()
        )
        .into_bytes();
        resp.extend_from_slice(&body);
        let url = serve_once(resp);
        let dir = std::env::temp_dir().join("corky_upd_cap_zero");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let cancel = Arc::new(AtomicBool::new(false));
        let n = download_asset_limited(
            &url,
            &dir.join("z.part"),
            0,
            CHECKSUMS_MAX_BYTES,
            &|_, _| {},
            &cancel,
        )
        .expect("size 0 must not abort a small body");
        assert_eq!(n, 5);
        assert_eq!(std::fs::read(dir.join("z.part")).unwrap(), b"hello");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn read_file_capped_refuses_oversize() {
        let dir = std::env::temp_dir().join("corky_upd_readcap");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let f = dir.join("checksums.txt");
        std::fs::write(&f, b"abcd").unwrap();
        assert_eq!(read_file_capped(&f, 4).unwrap(), "abcd");
        assert!(read_file_capped(&f, 3).is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn prune_never_deletes_foreign_names() {
        let dir = std::env::temp_dir().join("corky_upd_prune_foreign");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        // Names that merely share the prefix are not the updater's.
        let foreign = [
            "corkytux.bak-manual-1",
            "corkytux.bak-notaseq",
            "corkytux.bak-123-4",
            "corkytux.bak-12ab-000001",
            "corkytux.bak-123-00000a",
        ];
        for n in foreign {
            std::fs::write(dir.join(n), b"x").unwrap();
        }
        for i in 0..5u64 {
            std::fs::write(dir.join(format!("corkytux.bak-1000-{:06}", i)), b"x").unwrap();
        }
        prune_backups(&dir, "corkytux.bak-", BACKUP_KEEP_NEWEST);
        for n in foreign {
            assert!(dir.join(n).exists(), "foreign file pruned: {}", n);
        }
        let mut kept: Vec<String> = std::fs::read_dir(&dir)
            .unwrap()
            .flatten()
            .map(|e| e.file_name().to_string_lossy().to_string())
            .filter(|n| is_own_backup_name(n, "corkytux.bak-"))
            .collect();
        kept.sort();
        assert_eq!(
            kept,
            vec![
                "corkytux.bak-1000-000000".to_string(),
                "corkytux.bak-1000-000003".to_string(),
                "corkytux.bak-1000-000004".to_string(),
            ],
            "own retention changed"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[cfg(unix)]
    #[test]
    fn part_symlink_is_not_followed_on_write() {
        let body = b"hello".to_vec();
        let mut resp = format!(
            "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
            body.len()
        )
        .into_bytes();
        resp.extend_from_slice(&body);
        let url = serve_once(resp);
        let dir = std::env::temp_dir().join("corky_upd_part_sym");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let victim = dir.join("victim.txt");
        std::fs::write(&victim, b"ORIGINAL").unwrap();
        let part = dir.join(partial_name("corkytux-9.9.9-linux-x86_64.tar.gz"));
        std::os::unix::fs::symlink(&victim, &part).unwrap();
        let cancel = Arc::new(AtomicBool::new(false));
        let n = download_asset(&url, &part, 5, &|_, _| {}, &cancel).expect("download ok");
        assert_eq!(n, 5);
        assert_eq!(
            std::fs::read(&victim).unwrap(),
            b"ORIGINAL",
            "victim rewritten through the .part symlink"
        );
        assert!(
            !std::fs::symlink_metadata(&part).unwrap().file_type().is_symlink(),
            ".part stayed a symlink"
        );
        assert_eq!(std::fs::read(&part).unwrap(), b"hello");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[cfg(unix)]
    #[test]
    fn atomic_replace_does_not_follow_tmp_symlink() {
        let dir = std::env::temp_dir().join("corky_upd_new_sym");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let src = dir.join("src.bin");
        std::fs::write(&src, b"NEW").unwrap();
        let dst = dir.join("corkytux");
        let victim = dir.join("victim.txt");
        std::fs::write(&victim, b"ORIGINAL").unwrap();
        let tmp = dst.with_extension("new");
        std::os::unix::fs::symlink(&victim, &tmp).unwrap();
        atomic_replace(&src, &dst).expect("replace ok");
        assert_eq!(std::fs::read(&dst).unwrap(), b"NEW");
        assert_eq!(
            std::fs::read(&victim).unwrap(),
            b"ORIGINAL",
            "victim rewritten through the .new symlink"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn pre_outcome_backups_are_removed_unless_disarmed() {
        let dir = std::env::temp_dir().join("corky_upd_prebak");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let keep = dir.join("kept.bak-1-000001");
        let drop_me = dir.join("dropped.bak-1-000002");
        std::fs::write(&keep, b"k").unwrap();
        std::fs::write(&drop_me, b"d").unwrap();
        {
            let mut pre = PreOutcomeBackups::new();
            pre.keep(keep.clone());
            pre.keep(drop_me.clone());
            // Not disarmed: both are removed when the guard drops.
        }
        assert!(!keep.exists() && !drop_me.exists(), "guard did not clean up");
        std::fs::write(&keep, b"k").unwrap();
        {
            let mut pre = PreOutcomeBackups::new();
            pre.keep(keep.clone());
            pre.disarm();
        }
        assert!(keep.exists(), "disarmed guard deleted the backup");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[cfg(unix)]
    #[test]
    fn pre_outcome_failure_leaves_no_backups() {
        use std::os::unix::fs::PermissionsExt;
        if unsafe { libc::geteuid() } == 0 {
            return; // root ignores mode bits
        }
        let dir = std::env::temp_dir().join("corky_upd_prefail");
        let _ = std::fs::remove_dir_all(&dir);
        let install = dir.join("install");
        std::fs::create_dir_all(&install).unwrap();
        std::fs::write(install.join("corkytux"), b"orig-bin").unwrap();
        let icons = dir.join("icons");
        std::fs::create_dir_all(&icons).unwrap();
        std::fs::write(icons.join("corkytux.png"), b"png").unwrap();
        let staging = dir.join("staging");
        std::fs::create_dir_all(&staging).unwrap();
        let bin = healthy_fake_binary(&staging, "corkytux");
        std::fs::write(staging.join("corkytux.png"), b"new-png").unwrap();
        let staged = Staged {
            binary: bin,
            icon: Some(staging.join("corkytux.png")),
            ..Default::default()
        };
        let dirs = InstallDirs {
            install_dir: install.clone(),
            icon_dir: icons.clone(),
            desktop_file: dir.join("app.desktop"),
            symlink: dir.join("link"),
        };
        // Read-only icon dir: the icon backup fails after the binary
        // backup already landed, i.e. a pre-outcome `?`.
        let mut perm = std::fs::metadata(&icons).unwrap().permissions();
        perm.set_mode(0o555);
        std::fs::set_permissions(&icons, perm).unwrap();
        let ver = semver::Version::parse("9.9.9").unwrap();
        let err = install_if_healthy(&staged, &dirs, &ver).expect_err("icon backup must fail");
        let mut perm = std::fs::metadata(&icons).unwrap().permissions();
        perm.set_mode(0o755);
        std::fs::set_permissions(&icons, perm).unwrap();
        assert!(err.contains("backup failed"), "got: {}", err);
        let residue: Vec<String> = std::fs::read_dir(&install)
            .unwrap()
            .flatten()
            .map(|e| e.file_name().to_string_lossy().to_string())
            .filter(|n| n.contains(".bak-"))
            .collect();
        assert!(
            residue.is_empty(),
            "pre-outcome failure left backups: {:?}",
            residue
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn select_checksums_asset_validates_size() {
        let mut info = parse_latest_release(SAMPLE_JSON).unwrap();
        // Absent: no checksums.txt is fine.
        assert!(select_checksums_asset(&info).unwrap().is_none());
        // A sane size is accepted.
        info.assets.push(AssetInfo {
            name: "checksums.txt".into(),
            browser_download_url: "https://github.com/x/checksums.txt".into(),
            digest_sha256: None,
            size: 4096,
        });
        assert_eq!(select_checksums_asset(&info).unwrap().unwrap().size, 4096);
        // Zero is refused.
        info.assets.last_mut().unwrap().size = 0;
        assert!(select_checksums_asset(&info).is_err());
        // One byte over the 1 MiB ceiling is refused.
        info.assets.last_mut().unwrap().size = CHECKSUMS_MAX_BYTES + 1;
        assert!(select_checksums_asset(&info).is_err());
        // The ceiling itself is allowed.
        info.assets.last_mut().unwrap().size = CHECKSUMS_MAX_BYTES;
        assert!(select_checksums_asset(&info).is_ok());
    }

    #[test]
    fn download_limited_enforces_the_small_hard_cap() {
        // No Content-Length, so the streaming guard is the one that stops
        // the 5-byte body at a 4-byte cap.
        let body = b"hello".to_vec();
        let mut resp = b"HTTP/1.1 200 OK\r\nConnection: close\r\n\r\n".to_vec();
        resp.extend_from_slice(&body);
        let url = serve_once(resp);
        let dir = std::env::temp_dir().join("corky_upd_cap_limited");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let cancel = Arc::new(AtomicBool::new(false));
        let err = download_asset_limited(&url, &dir.join("l.part"), 0, 4, &|_, _| {}, &cancel)
            .expect_err("must abort past the hard cap");
        assert!(err.contains("exceeds size limit"), "got: {}", err);
        assert!(!dir.join("l.part").exists(), "partial left behind");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn content_length_over_cap_aborts_before_writing() {
        // The declared length alone is over the cap: fail before the
        // partial file is created, so nothing is written.
        let body = b"hello".to_vec();
        let mut resp = b"HTTP/1.1 200 OK\r\nContent-Length: 100\r\nConnection: close\r\n\r\n".to_vec();
        resp.extend_from_slice(&body);
        let url = serve_once(resp);
        let dir = std::env::temp_dir().join("corky_upd_cl_cap");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let part = dir.join("cl.part");
        let cancel = Arc::new(AtomicBool::new(false));
        let err = download_asset_limited(&url, &part, 0, 8, &|_, _| {}, &cancel)
            .expect_err("declared length over cap must abort");
        assert!(err.contains("exceeds size limit"), "got: {}", err);
        assert!(!part.exists(), "partial written despite the header check");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn cancellation_is_not_shown_as_an_error() {
        assert_eq!(failure_to_show(true, "download cancelled"), None);
        assert_eq!(failure_to_show(true, "boom"), None);
        assert_eq!(failure_to_show(false, "boom").as_deref(), Some("boom"));
    }

    #[test]
    fn check_guard_is_exclusive_and_released_on_panic() {
        let _lock = update_state_lock();
        let first = CheckGuard::acquire().expect("first acquire");
        assert!(
            CheckGuard::acquire().is_none(),
            "second acquire must be refused"
        );
        drop(first);
        drop(CheckGuard::acquire().expect("released after drop"));
        // Panic path: unwinding drops the guard and frees the gate.
        let panicked = std::panic::catch_unwind(|| {
            let _g = CheckGuard::acquire().expect("acquire before panic");
            panic!("boom");
        });
        assert!(panicked.is_err());
        assert!(CheckGuard::acquire().is_some(), "gate leaked after a panic");
    }

    #[cfg(unix)]
    #[test]
    fn icon_installs_without_exec_bit() {
        use std::os::unix::fs::PermissionsExt;
        let dir = std::env::temp_dir().join("corky_upd_icon_mode");
        let _ = std::fs::remove_dir_all(&dir);
        let install = dir.join("install");
        std::fs::create_dir_all(&install).unwrap();
        std::fs::write(install.join("corkytux"), b"orig-bin").unwrap();
        let icons = dir.join("icons");
        std::fs::create_dir_all(&icons).unwrap();
        let staging = dir.join("staging");
        std::fs::create_dir_all(&staging).unwrap();
        let bin = healthy_fake_binary(&staging, "corkytux");
        let icon_src = staging.join("corkytux.png");
        std::fs::write(&icon_src, b"png").unwrap();
        // Stage it the way extraction does: plain, no exec bit.
        set_plain(&icon_src).unwrap();
        assert_eq!(
            std::fs::metadata(&icon_src).unwrap().permissions().mode() & 0o111,
            0,
            "staged icon is executable"
        );
        let staged = Staged {
            binary: bin,
            icon: Some(icon_src),
            ..Default::default()
        };
        let dirs = InstallDirs {
            install_dir: install.clone(),
            icon_dir: icons.clone(),
            desktop_file: dir.join("app.desktop"),
            symlink: dir.join("link"),
        };
        let ver = semver::Version::parse("9.9.9").unwrap();
        install_if_healthy(&staged, &dirs, &ver).expect("install ok");
        let icon_mode = std::fs::metadata(icons.join("corkytux.png"))
            .unwrap()
            .permissions()
            .mode();
        assert_eq!(icon_mode & 0o777, 0o644, "icon mode is {:o}", icon_mode & 0o777);
        // The binary, by contrast, stays executable.
        assert_ne!(
            std::fs::metadata(install.join("corkytux"))
                .unwrap()
                .permissions()
                .mode()
                & 0o111,
            0
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn auto_check_gate() {
        assert!(!should_auto_check(false, None, 1000));
        assert!(should_auto_check(true, None, 1000));
        assert!(should_auto_check(true, Some(0), 6 * 3600));
        assert!(!should_auto_check(true, Some(1000), 1000 + 6 * 3600 - 1));
    }
}
