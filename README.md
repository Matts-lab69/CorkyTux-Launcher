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

CorkyTux began as a C++/Qt/QML application; the current launcher is the
Rust/GTK4 codebase in this repository.

---

## Features

### Library
Unified library, filters, search, recently played cards, favorites, playtime, per-game logs.

### Proton/Wine
Per-game Proton version and prefix, umu, EAC/BattlEye, EOS, Wine virtual desktop, environment variables, arguments, DLL overrides, Steam/Lutris/Heroic scans.

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

- GTK 4.12 or newer.
- libadwaita 1.5 or newer.

The official binary is built on Ubuntu 24.04 (glibc 2.39); an older glibc was
not tested. The build enables the GTK `v4_12` and libadwaita `v1_5` features,
so a newer distribution is fine and an older GTK or libadwaita is not.

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

For example, the current version produces
`corkytux-3.2.0-linux-x86_64.tar.gz` and extracts to `corkytux-3.2.0/`.

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

| File | Destination |
|------|-------------|
| `corkytux` | `~/.local/share/corkytux/corkytux` |
| `webdriver_login` | `~/.local/share/corkytux/webdriver_login` |
| `assets/` | `~/.local/share/corkytux/assets/` |
| `corkytux` (symlink) | `~/.local/bin/corkytux` |
| `corkytux.png` | `~/.local/share/icons/corkytux.png` |
| `corkytux.desktop` | `~/.local/share/applications/corkytux.desktop` |

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

## Self-update

- Opt-in: **Settings > Misc > Check for updates on startup** is off by default. Manual checks are in **Settings > About > Check for launcher updates**. Startup checks run at most once every six hours.
- Source: GitHub Releases at `Matts-lab69/CorkyTux-Launcher`.
- Integrity: the GitHub release API SHA256 digest is mandatory; `checksums.txt` is optional and must match when present.
- Hosts: HTTPS only; redirects only to `github.com`, `objects.githubusercontent.com`, and `release-assets.githubusercontent.com` for assets, and `api.github.com` and `github.com` for metadata.
- Limits: 200 MiB total, 50 MiB per file, 10,000 entries; no absolute paths, `..`, symlinks, hardlinks, or unexpected files.
- Rollback: every replaced file is backed up and restored on failure; backups are pruned after a successful install.
- Restart: the launcher quits and a detached sleeper starts the new binary after the old process exits.
- Scope: only copies installed by `install.sh` under `~/.local/share/corkytux` can self-update. Repository builds report that they must be updated manually.

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

- [@Cristioro](https://github.com/Cristioro) — UI ideas and suggestions
  to make the code more efficient.
- [@ZzEdovec](https://github.com/ZzEdovec) — some interfaces and icons
  used or inspired by this person's repositories.

---

## License

AGPL-3.0 — see [LICENSE](LICENSE).
