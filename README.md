<p align="center">
  <img src="assets/corkytux.png" width="120" alt="CorkyTux Logo">
</p>

<h1 align="center">CorkyTux</h1>

<p align="center">
  <strong>Linux Game Launcher (Rust + GTK4/libadwaita)</strong><br>
  Preserve your video games: unified library + multi-purpose plugins.
</p>

> **Alpha** — CorkyTux is under active development. If anything breaks,
> report it in [Issues](https://github.com/Matts-lab69/CorkyTux-Launcher/issues).

<p align="center">
  <img src="https://img.shields.io/badge/Rust-recent%20stable-orange?logo=rust&logoColor=white" alt="Rust">
  <img src="https://img.shields.io/badge/GTK-4.12%2B-blue?logo=gtk&logoColor=white" alt="GTK4">
  <img src="https://img.shields.io/badge/libadwaita-1.5%2B-green" alt="libadwaita">
  <img src="https://img.shields.io/badge/Platform-Linux-orange?logo=linux&logoColor=white" alt="Linux">
  <img src="https://img.shields.io/badge/License-AGPL--3.0-purple" alt="License">
</p>

<p align="center">
  <a href="#what-is-corkytux">What is</a> •
  <a href="#features">Features</a> •
  <a href="#quick-start">Quick start</a> •
  <a href="#build-from-source">Build</a> •
  <a href="#installation">Installation</a> •
  <a href="#configuration-and-data-paths">Paths</a> •
  <a href="#self-update">Self-update</a> •
  <a href="#architecture">Architecture</a> •
  <a href="#credits">Credits</a> •
  <a href="#license">License</a>
</p>

---
## What is CorkyTux?

CorkyTux is a Linux launcher built with **Rust + GTK4/libadwaita** to
**preserve video games**: a unified library for Windows games through
Proton/Wine and umu, Steam/Lutris/Heroic scans, and emulators, plus
multi-purpose plugins.

---

## Features

### Library
Unified library, filters, search, recently played cards, favorites, playtime, per-game logs.

### Proton/Wine
Per-game Proton version and prefix, umu, EAC/BattlEye, EOS, Wine virtual desktop, environment variables, arguments, DLL overrides, Steam/Lutris/Heroic scans.

**Custom DLL overrides:** edit the per-game field under Launch options using
`WINEDLLOVERRIDES` syntax, for example `winhttp=n,b;version=b`. Separate DLL
groups with semicolons; use `n`/`native`, `b`/`builtin`, or an empty load order
to disable a DLL. Invalid edits are marked and are not saved. Clear the field
to remove its overrides. The setting uses `custom_dll_overrides`; existing
`Overrides` values migrate once without removing the old value or other keys.
The explicit field takes precedence over the same variable in Environment
for direct Proton, umu and Legendary launches. Steam-managed launches continue
to use Steam's own launch options. No presets or DLL files are installed.

### Mods (BepInEx)

The per-game Launch options switch is off by default. It detects Windows Unity
Mono/IL2CPP and x86/x64 from the executable and game data. Native Linux and
unidentified layouts are not changed. Mono uses BepInEx 5.4.23.6 with published
SHA256 verification. IL2CPP installation is disabled until an official checksum
is available; existing managed installations can still be removed.

Existing files are never overwritten. A private `.corkytux-bepinex.json`
ownership manifest records installed files and their hashes. Turning the switch
off removes only unchanged owned files; user mods, plugins, patchers and config
directories remain. Modified files and edited overrides are preserved with a
notice. Existing unmanaged BepInEx files can be activated but are never removed.

Activation adds `winhttp=n,b` without replacing other DLL overrides; conflicting
explicit winhttp values must be edited first. Direct Proton, umu and Legendary
launches receive the variable. Steam-managed games additionally need
`WINEDLLOVERRIDES="winhttp=n,b" %command%` in Steam's own launch options; the
launcher does not modify Steam configuration. Compatibility with individual
games is not guaranteed. No downloaded payload is executed during installation.

---
## Plugins

Plugins are installed separately. See
[CorkyTux-Plugins](https://github.com/Matts-lab69/CorkyTux-Plugins) for
binaries and guides.

- `minecraft-launcher`: Minecraft Java, accounts, loaders, Modrinth/CurseForge.
- `heroic-store`: Epic/GOG stores, library, promos, deals.
- `emulator-manager`: retro emulator install and ROM detection.
- `appimage-launcher`: scan and integrate AppImages.
- `rpgmaker-runtime`: RPG Maker MV/MZ and 2000/2003 runtimes.
- `dependency-installer`: detect and install missing Windows components.
- `dll-overrides-automator`: manage `WINEDLLOVERRIDES`.

---
## Requirements

The prebuilt tarball needs a 64-bit Linux system with:

- **glibc 2.39 or newer.** The official binary is built on Ubuntu 24.04 and
  links against that glibc. Older releases cannot run it: Ubuntu 22.04
  (glibc 2.35), Debian 12 (2.36), Fedora 39 (2.38) and Linux Mint 21 (2.35)
  are not supported by the prebuilt tarball.
- GTK 4.12 or newer.
- libadwaita 1.5 or newer.
- libudev (`libudev.so.1`).

Distributions that meet that floor, and where the tarball is expected to run:
Ubuntu 24.04 and newer, Debian 13 and newer, Fedora 40 and newer, Linux Mint 22
and newer, Arch Linux, openSUSE Tumbleweed and NixOS (`install.sh` looks for
GTK and glibc in `/nix/store`). Other distributions with the same library
versions should work, but they are not tested per release.

The build enables the GTK `v4_12` and libadwaita `v1_5` features, so a newer
distribution is fine and an older GTK or libadwaita is not.

The launcher itself needs no Python or Java. The bundled plugins use these
optional runtime dependencies; `install.sh` warns about the ones it can check:

| Dependency | Needed for | Without it |
|---|---|---|
| `python3` | every bundled plugin | plugins do not start |
| `python3-requests` | plugins that call HTTP APIs | network-backed actions fail |
| `minecraft_launcher_lib`, `psutil` | Minecraft plugin | Minecraft install and launch fail |
| a Java runtime (8, 17, 21 or 25, matching the game version) | Minecraft plugin | Minecraft cannot launch |
| `ffmpeg` or Pillow (`python3-PIL`) | WebP mod icons | a generic icon is shown |
| `icoextract` | icons extracted from Windows executables | a generic icon is shown |
| `sqlite3` | Lutris library scan | the scan is skipped |
| `curl` | Proton downloads | Proton downloads fail |

---
## Quick start

### Install from tarball

Download the tarball from
[Releases](https://github.com/Matts-lab69/CorkyTux-Launcher/releases).

```bash
tar -xzf corkytux-*-linux-*.tar.gz
cd corkytux-*/
./install.sh
```

For example, the current source version produces
`corkytux-3.2.2-linux-x86_64.tar.gz` and extracts to `corkytux-3.2.2/`.

### Build from source

```bash
git clone https://github.com/Matts-lab69/CorkyTux-Launcher.git
cd CorkyTux-Launcher
cargo build --release --bins
```

### Run

```bash
corkytux
# or, without installing:
./target/release/corkytux
```

---
## Build from source

### Prerequisites

| Distro | Rust | pkg-config | gcc | GTK4 dev | libadwaita dev |
|--------|------|------------|-----|----------|----------------|
| Debian/Ubuntu | `cargo rustc` | `pkg-config` | `build-essential` | `libgtk-4-dev` | `libadwaita-1-dev` |
| Fedora/RHEL | `cargo rust` | `pkg-config` | `gcc` (+ `base-devel`) | `gtk4-devel` | `libadwaita-devel` |
| Arch | `rust` | `pkgconf` | `base-devel` | `gtk4` | `libadwaita` |
| openSUSE | `cargo rust` | `pkgconf` | `patterns-devel-base` | `gtk4-devel` | `libadwaita-devel` |
| Gentoo | `dev-lang/rust` | `dev-util/pkgconfig` | `sys-devel/gcc` | `gui-libs/gtk:4` | `gui-libs/libadwaita` |

`Cargo.toml` enables the GTK `v4_12` and libadwaita `v1_5` features, so
GTK 4.12+ and libadwaita 1.5+ are required. `Cargo.toml` does not declare
`rust-version`; use a recent stable Rust toolchain (edition 2021).

### Install Rust

If Rust is not installed, use rustup:

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source "$HOME/.cargo/env"
```

### Verify GTK and libadwaita

```bash
pkg-config --modversion gtk4 libadwaita-1
```

The output must be 4.12 or newer for `gtk4` and 1.5 or newer for
`libadwaita-1`. If a module is missing, install the development package
listed in the table above.

### Clone and build

```bash
git clone https://github.com/Matts-lab69/CorkyTux-Launcher.git
cd CorkyTux-Launcher
cargo build --release --bins
```

The build produces `target/release/corkytux` and
`target/release/webdriver_login`. The second binary is the Epic/GOG
WebDriver login helper. The release profile enables LTO and strip.

### Run without installing

```bash
./target/release/corkytux
```

### Create the tarball

```bash
./release/build-release.sh vX.Y.Z
```

Replace `vX.Y.Z` with the `version` in `Cargo.toml`; the version argument
is mandatory and must match it. The script does not check that the git tag
exists. The script creates
`corkytux-<version>-linux-<arch>.tar.gz` and `checksums.txt`. The tarball
contains `corkytux`, `webdriver_login`, `install.sh`, `uninstall.sh`,
`corkytux.png`, `corkytux.desktop.in`, and `assets/`.

### Install the tarball

```bash
tar -xzf corkytux-*-linux-*.tar.gz
cd corkytux-*/
./install.sh
```

Do not use `sudo`; `install.sh` writes under `$HOME`. Paths are listed in
[Installation](#installation).

### Troubleshooting

| Symptom | Cause | Fix | Basis |
|---------|-------|-----|-------|
| `pkg-config` cannot find `gtk4` or `libadwaita-1` | Development packages are missing or `PKG_CONFIG_PATH` is incomplete | Install the GTK4 and libadwaita development packages for your distribution | Deduced |
| GTK or libadwaita is too old | Installed version is below 4.12 or 1.5 | Upgrade the distribution packages or build newer libraries | Deduced; minimum comes from `Cargo.toml` |
| Linker or `gcc` is missing | No C toolchain | Install `build-essential`, `gcc`, or `base-devel` | Deduced |
| `rustc` is too old | `Cargo.toml` does not declare `rust-version`; the crate uses edition 2021 | Run `rustup update stable`; no numeric minimum is declared | Deduced; missing `rust-version` is verified |
| `version argument does not match Cargo.toml` | The version argument differs from `Cargo.toml` | Pass the exact version from `Cargo.toml` | Verified by `release/build-release.sh` |
| `corkytux` is not in `PATH` | `~/.local/bin` is not on `PATH` | Add `export PATH="$HOME/.local/bin:$PATH"` to your shell profile | Verified by `release/install.sh` |

---
## Installation

### Install from a release tarball

```bash
tar -xzf corkytux-*-linux-*.tar.gz
cd corkytux-*/
./install.sh
```

`install.sh` does not use `sudo`.
If the tarball contains `plugins/`, the installer also installs its bundled
plugin code into `${XDG_DATA_HOME}/CorkyTux/plugins` when `XDG_DATA_HOME` is
absolute, otherwise `~/.local/share/CorkyTux/plugins` — the same path the launcher
uses. Run `./install.sh --no-plugins` to leave plugins untouched. Reinstalling
updates bundled code without deleting runtime files or other plugins. Optional
dependencies and game runtimes are not downloaded by the installer.

| File | Destination |
|------|-------------|
| `corkytux` | `~/.local/share/corkytux/corkytux` |
| `webdriver_login` | `~/.local/share/corkytux/webdriver_login` |
| `assets/` | `~/.local/share/corkytux/assets/` |
| `plugins/` (when bundled) | `~/.local/share/CorkyTux/plugins/` or absolute `XDG_DATA_HOME` equivalent |
| `corkytux` (symlink) | `~/.local/bin/corkytux` |
| `corkytux.png` | `~/.local/share/icons/hicolor/128x128/apps/com.corkytux.CorkyTux.png` (plus legacy aliases) |
| `com.corkytux.CorkyTux.desktop` | `~/.local/share/applications/com.corkytux.CorkyTux.desktop` |

The desktop filename, `Icon`, `StartupWMClass` and GTK application ID all use
`com.corkytux.CorkyTux`, so menus and window grouping can resolve the installed
hicolor icon. With absolute `XDG_DATA_HOME`, icons and the desktop entry use
that data directory instead. Reinstalling migrates the old `corkytux.desktop`
entry only when it points to this launcher, preserving a backup. Installation
fails before replacing anything if the package is missing `corkytux.png`.

### Check the installation

```bash
corkytux --version     # prints: corkytux X.Y.Z
command -v corkytux    # must print ~/.local/bin/corkytux
```

If `command -v corkytux` prints nothing, `~/.local/bin` is not on your
`PATH`: add `export PATH="$HOME/.local/bin:$PATH"` to your shell profile
(`~/.bashrc`, `~/.zshrc`) and open a new shell. If the path is printed and
the version is not, re-run `./install.sh`.

### Uninstall

```bash
cd corkytux-*/
./uninstall.sh
```

Uninstall removes the bundled plugin code recorded by the installer, including
after a reinstall. It leaves unrelated plugins and plugin-created files alone;
deleting game data, cache and configuration still requires explicit confirmation.

---
## Configuration and data paths

| Path | Contents |
|------|----------|
| `~/.config/CorkyTux/Launcher.ini` | Launcher settings |
| `~/.config/CorkyTux/Games.ini` | Game library |
| `~/.config/CorkyTux/banners/` | Game banners |
| `~/.config/CorkyTux/icons/` | Game icons |
| `~/.local/share/CorkyTux/plugins/` | Installed plugins |
| `~/.local/share/CorkyTux/prefixes/` | Game prefixes |
| `~/.local/share/CorkyTux/downloads/` | Self-update staging |
| `~/.cache/CorkyTux/modicons/` | Minecraft mod icon cache |

---
## Optional runtime tools

| Tool | Feature |
|------|---------|
| `python3` + `requests` (+ `minecraft_launcher_lib`, `psutil` for MC) | all Python plugins |
| `java` 8/17/21/25 | Minecraft plugin |
| `steam` | Proton games / Steam scan |
| `umu-run` / `umu-launcher` | umu Proton (auto-installed by heroic-store to `tools/umu`, v1.4.4) |
| `lutris`, `heroic`, `legendary`, `gogdl` | scans / Epic+GOG (legendary/gogdl auto-downloaded by plugin setup) |
| `sqlite3` | Lutris `pga.db` scan |
| `curl`, `tar`, `unzip`/`unrar`, `ffmpeg` | Proton downloads, artwork |
| `gamemode`, `mangohud`, `zenity`/`kdialog`, `glxinfo` | optional launch helpers |

---
## SteamGridDB artwork

Cover, banner and icon art fall back to SteamGridDB when a key is available.
CorkyTux resolves it in this order and stops at the first non-blank value:

1. `CORKYTUX_STEAMGRIDDB_API_KEY` environment variable.
2. The file named by `CORKYTUX_STEAMGRIDDB_KEY_FILE`, or
   `~/.config/CorkyTux/steamgriddb.key` by default. Store only the key and keep
   the file readable by its owner alone (`0600`).
3. `IntegrationApiKey` in `~/.config/CorkyTux/Launcher.ini`.

A missing or blank file moves on to the next source. Without any key the
launcher skips SteamGridDB, keeps the other artwork sources, and shows a
one-time dismissible notice explaining where to add one. Game launching does
not require SteamGridDB. No shared key or build-time credential is included.

Obtain a personal key from your
[SteamGridDB API preferences](https://www.steamgriddb.com/profile/preferences/api),
then save it in **Settings > Integrations > SteamGridDB artwork**, or configure
the environment/private-file source above. The saved field remains
`IntegrationApiKey` in the private user configuration.

The optional standalone file must be a regular file owned by the current user,
not a symlink or hard link. Create it privately (for example, with `umask 077`)
and store only the key. Before reading, CorkyTux corrects its permissions to
`0600`; if that fails, it warns without printing the key and tries the saved
setting. Missing files are not created automatically. Empty or invalid files
do not prevent the rest of the application from working.

---
## Game icons

Windows `.exe` icons are extracted with `icoextract` and `ffmpeg`. When either
is missing, CorkyTux keeps the generic cover and logs one warning per run:
"icon extraction needs icoextract or ffmpeg; the generic icon is used".

## Self-update

- Opt-in: **Settings > Misc > Check for updates on startup** is off by default. Manual checks are in **Settings > About > Check for launcher updates**. Startup checks run at most once every six hours.
- Source: GitHub Releases at `Matts-lab69/CorkyTux-Launcher`.
- Integrity: the GitHub release API SHA256 digest is mandatory; `checksums.txt` is optional and must match when present.
- Hosts: HTTPS only; redirects only to `github.com`, `objects.githubusercontent.com`, and `release-assets.githubusercontent.com` for assets, and `api.github.com` and `github.com` for metadata.
- Limits: 200 MiB total, 50 MiB per file, 10,000 entries; no absolute paths, `..`, symlinks, hardlinks, or unexpected files.
- Payload: the binary, `webdriver_login`, the icon and `assets/` are replaced. The bundled plugins and the docs in the tarball are skipped by exact name and the install log says so; re-run `install.sh` to refresh plugins. Any other unexpected entry aborts the update.
- Rollback: every replaced file is backed up and restored on failure; backups are pruned after a successful install.
- Restart: the launcher quits and a detached sleeper starts the new binary after the old process exits.
- Scope: only copies installed by `install.sh` under `~/.local/share/corkytux` can self-update. Repository builds report that they must be updated manually.
- From 3.2.0 or 3.2.1: install 3.2.2 manually once. Both older versions misclassify HTTP 304 responses during update checks; 3.2.0 also rejects the release tarball with its stricter extractor. The 3.2.2 updater handles cached metadata correctly.

---
## --version

```bash
corkytux --version
```

Prints `corkytux X.Y.Z` and exits without opening a window. The flag is
recognized only when it is the sole argument.

---
## Release

1. Bump `version` in `Cargo.toml`.
2. Run `cargo check`; it refreshes the version entry in `Cargo.lock` without upgrading dependencies.
3. Commit `Cargo.toml` and `Cargo.lock` together.
4. Build the release and check the tarball:

```bash
./release/build-release.sh vX.Y.Z
VERSION=$(grep -m1 '^version' Cargo.toml | grep -oE '[0-9]+\.[0-9]+\.[0-9]+')
tar tzf "corkytux-${VERSION}-linux-$(uname -m).tar.gz"
```

5. Create and push the tag:

```bash
git tag vX.Y.Z
git push origin vX.Y.Z
```

6. Create the GitHub Release and upload the tarball and `checksums.txt`.

`vX.Y.Z` is the `Cargo.toml` version; the version argument is mandatory
and must match it. The script does not check that the git tag exists.
The versioned tarball example above uses the same value. The script
builds with `cargo build --release --bins`, creates
`corkytux-<version>-linux-<arch>.tar.gz`, and writes `checksums.txt`.

---
## Security

- Epic Online Services exchange codes: games that use EOS authentication
  receive the exchange code through their launch arguments
  (`-auth_password=...`), because the Epic client reads it from argv and
  offers no environment variable or file alternative. The code is therefore
  visible in `/proc/<pid>/cmdline` while the game runs. The launcher
  mitigates what it can: the game log is created with mode 0600 and every
  copy the launcher makes (the log view and the saved log) is redacted.
  Do not share game logs or crash reports from an EOS game without
  reviewing them first.


## Architecture

```text
src/main.rs                     Adw window, pages games/minecraft/stores
src/bin/webdriver_login.rs      WebDriver login helper for Epic/GOG
src/ui/sidebar.rs               library, filters, search, Minecraft/Store toggles
src/ui/center.rs                Recently Played cards
src/ui/details_panel.rs         banner, Play/Stop, async cached install size
src/ui/game_settings.rs         Proton/prefix, EAC/BE, EOS, WineVDesktop, env, args, isolation
src/ui/settings.rs              visuals, paths, protons, plugins, integrations, about
src/ui/minecraft_view.rs        MC library/detail/browse (Modrinth/CurseForge)
src/ui/stores_view.rs           Epic/GOG tabs, session, library, promos, deals
src/ui/update_dialog.rs         self-update check, progress, restart prompt
src/backend/proton.rs           Proton/umu launch, EAC/BE envs, legendary launch, playtime/logs
src/backend/isolation.rs        bubblewrap sandbox (private HOME, prefix+game rw, runners ro)
src/backend/plugin_process.rs   JSON-lines spawn + 50ms pump
src/backend/self_update.rs      GitHub Releases check, download, verify, install, restart
src/backend/game_model.rs       GameEntry <-> Games.ini
src/backend/config.rs           Launcher.ini + Games.ini
src/backend/external.rs         plugin command wrappers (Minecraft, Heroic, RPG, dependency installer)
src/backend/import_move.rs      permanent import, prefix moves, Steam library import
src/backend/integration.rs      Lutris/Steam scans, artwork, SGDB
src/backend/plugins.rs          plugin registry, download, install, update
src/backend/shortcuts.rs        .desktop entries and app menu
src/backend/theme.rs            GTK theme helpers
docs/                           BUILD, audits (multidistro, perf), wine-isolation design
assets/                         bundled icons (light + dark)
release/                        install.sh, uninstall.sh, build-release.sh, corkytux.png, corkytux.desktop.in
plugins/<name>/                 separate repo (CorkyTux-Plugins): plugin binary + plugin.json
```

---
## Credits

See [CREDITS.md](CREDITS.md) for included code and artwork, optional runtimes,
upstream license references, and project inspirations.
Existing third-party copyright and license notices remain authoritative.

- [@Cristioro](https://github.com/Cristioro) — UI ideas and suggestions
  to make the code more efficient.
- [@ZzEdovec](https://github.com/ZzEdovec) — some interfaces and icons
  used or inspired by this person's repositories.

---
## Gamepad

Any controller with an SDL gamepad mapping works: DualShock 4, DualSense,
Xbox (including the newer Bluetooth pads), Switch Pro and most 8BitDo pads.
Wheels, flight sticks and devices without an SDL mapping are ignored.

- Linux needs read access to `/dev/input/event*`. On most distributions the
  user in the `input` group is enough; a udev rule can grant it to a single
  vendor:

  ```
  SUBSYSTEM=="input", ATTRS{idVendor}=="054c", MODE="0660", GROUP="input"
  ```

- `SDL_GAMECONTROLLERCONFIG` is honored: export a mapping line for a pad that
  SDL does not know and it will be used instead of the built-in database.
- With Steam running, Steam Input exposes a virtual pad. CorkyTux keeps the
  virtual pad and drops the physical duplicate, so one press is one action.
- Repeat timing can be tuned in `~/.config/CorkyTux/Launcher.ini` under
  `[User Settings]` with `gamepadInitialRepeatDelay` (default `400` ms) and
  `gamepadRepeatDelay` (default `120` ms). Values must be positive integers;
  invalid or zero values use the defaults.
- There is no on-screen keyboard yet, so search fields and paths cannot be
  typed with a controller (#88).
- Guide/PS remains unbound. Steam/Game Bar or the desktop compositor may intercept
  it before the application receives it; CorkyTux does not grab this button or
  compete with host shortcuts. This conservative policy can be revisited later
  without changing existing controller mappings or user configuration.

---

## First-use tutorial

The first visible window offers up to six short, widget-relative hints. Every
step has **Skip**; Escape, controller Back or hiding the window also dismisses
the tour. Completed, skipped and interrupted tours do not appear automatically
again. Reopen it with **Settings → View tutorial**. Missing or hidden targets
are skipped. Progress is stored as `FirstUseTutorialState` in `Launcher.ini`.
Hints use standard GTK buttons and the existing controller navigation; no
full-window overlay or fixed screen coordinates are used.

## License

AGPL-3.0 — see [LICENSE](LICENSE).
