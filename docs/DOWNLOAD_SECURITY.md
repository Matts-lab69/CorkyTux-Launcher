# Runtime download integrity

## Pinned releases

The following Linux x86-64 installers use fixed versions. Checksums were read
from the linked official release sources on **2026-11-04**; none was generated
as a substitute for a publisher checksum.

| Component | Pinned tag / asset | Official checksum source |
|---|---|---|
| Legendary | `0.21.1` / `legendary_linux_x64` | SHA-256 in the asset's `digest` field in the [GitHub release API](https://api.github.com/repos/legendary-gl/legendary/releases/tags/0.21.1). |
| gogdl | `v1.4.0` / `gogdl_linux_x86_64` | SHA-256 in the asset's `digest` field in the [GitHub release API](https://api.github.com/repos/Heroic-Games-Launcher/heroic-gogdl/releases/tags/v1.4.0). |
| umu-launcher | `1.4.4` / `umu-launcher-1.4.4-zipapp.tar` | SHA-256 in the asset's `digest` field in the [GitHub release API](https://api.github.com/repos/Open-Wine-Components/umu-launcher/releases/tags/1.4.4). |
| GE-Proton | `GE-Proton11-12` / `GE-Proton11-12-x86_64.tar.gz` | SHA-512 in the publisher's [checksum sidecar](https://github.com/GloriousEggroll/proton-ge-custom/releases/download/GE-Proton11-12/GE-Proton11-12-x86_64.sha512sum). |

Legendary, gogdl and umu checksums are official GitHub per-asset metadata, **not**
standalone checksum files or publisher signatures. GE-Proton additionally has a
publisher-provided sidecar. The exact expected values are kept with the pinned
installer metadata in source. Updating a pin requires checking its official
asset name, URL and checksum together; a tag alone is insufficient.

Verification happens before installing a binary or extracting an archive. A
checksum mismatch aborts with an error. GE-Proton ignores old latest-release
catalog caches and rejects downloads outside its pinned tag/URL. Existing
separately installed or user-selected runners are not retroactively attested by
these installer checks. CachyOS downloads retain their separate existing policy;
this change does not claim checksum verification for CachyOS.

## Emulator AppImages

AppImage selection still uses the catalog publisher's latest release. When its
release API supplies a supported SHA-256/SHA-512 asset digest, the installer checks
the bytes against that digest before replacing the managed image. Digest and URL
must come from the same release response. A missing digest is reported as an
unverified download, never as successful checksum verification. Malformed digest
metadata or a mismatch is an installation error.

At the source check on 2026-11-04, release API digests were available for melonDS,
Dolphin, PCSX2, PPSSPP, RPCS3, Vita3K, Azahar, DuckStation, Cemu and DeSmuME assets.
The catalog's original Ryujinx and Mupen64Plus repositories returned no accessible
latest release (HTTP 404); no current asset or checksum is claimed for them.
Availability can change, so the actual selected asset metadata is checked on
each installation. Assets that omit a digest remain explicitly unverified.

Several catalog repositories package AppImages independently of the upstream
emulator project. Their release digests attest to the **catalog publisher's**
bytes, not an upstream emulator signature. A `.zsync` asset is not treated as a
SHA-256/SHA-512 checksum file.

## SteamGridDB credentials

SteamGridDB artwork is optional. Builds contain no shared credential and accept
no compile-time credential input. User credentials are resolved at runtime in
this order:

1. `CORKYTUX_STEAMGRIDDB_API_KEY` environment variable, if nonblank.
2. The private file named by `CORKYTUX_STEAMGRIDDB_KEY_FILE`, or by default
   `~/.config/CorkyTux/steamgriddb.key`. Store only the key in this file, keep it
   outside the repository, and restrict it to the account owner (`0600`).
3. The existing `IntegrationApiKey` setting in the user's private `Launcher.ini`,
   for compatibility when no environment value or private key file exists.

A missing or blank private file falls through to the next source, so an empty
file cannot hide a saved setting. The reader requires a user-owned regular file
with one link and does not follow symlinks. It sets permissions to `0600` before
reading; failures warn without printing contents and allow the saved setting
to answer. Reads are limited to 4096 UTF-8 bytes. Whichever value is selected is
registered with the redactor before use, so it cannot appear in logs or error
messages. No credential belongs in source, build flags, commits or logs. Rotate
credentials through the user's private configuration, not by changing a source
constant.
