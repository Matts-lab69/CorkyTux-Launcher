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
  <img src="https://img.shields.io/badge/Rust-1.70%2B-orange?logo=rust&logoColor=white" alt="Rust">
  <img src="https://img.shields.io/badge/GTK-4.12%2B-blue?logo=gtk&logoColor=white" alt="GTK4">
  <img src="https://img.shields.io/badge/libadwaita-1.4%2B-green" alt="libadwaita">
  <img src="https://img.shields.io/badge/Platform-Linux-orange?logo=linux&logoColor=white" alt="Linux">
  <img src="https://img.shields.io/badge/License-AGPL--3.0-purple" alt="License">
</p>

<p align="center">
  <a href="#features">Features</a> •
  <a href="#installation">Installation</a> •
  <a href="#dependencies">Dependencies</a> •
  <a href="docs/BUILD.md">Build from source</a> •
  <a href="#plugins">Plugins</a> •
  <a href="#architecture">Architecture</a> •
  <a href="#credits">Credits</a> •
  <a href="#license">License</a>
</p>

---

## What is CorkyTux?

CorkyTux is a Linux launcher built with **Rust + GTK4/libadwaita** to
**preserve video games**: your unified library in one place
(Windows games via Proton/Wine + umu, Steam/Lutris/Heroic scans,
emulators) plus **multi-purpose plugins** — Minecraft Java,
Epic/GOG stores, AppImages, RPG Maker, dependencies, DLLs and more
(via a streaming JSON-lines protocol).

---

## Features

### Library
- Unified library, filters (All / Favorites / A-Z / Most played / Recent), search
- Recently Played cards, details overlay (banner 380x192, Play/Stop, install size)
- Favorites, playtime tracking, per-game logs, persistent window size

### Proton / Wine
- Per-game Proton version + prefix, `waitforexitandrun`, umu (`umu-run`) support
- EAC / BattlEye runtimes (Heroic), EOS auth (Epic), Wine virtual desktop (`user.reg`)
- Custom env vars, launch args, DLL overrides, Steam/Lutris/Heroic scan + import

### Minecraft (plugin) — [guide](https://github.com/Matts-lab69/CorkyTux-Plugins/blob/main/minecraft-launcher/README.md)
- Offline / Microsoft / Ely.by accounts, Java 8/17/21/25 auto-detect
  (`~/jdk`, `/opt/jvm`, `/usr/lib/jvm`, `update-alternatives`), isolated instances
- Fabric/Forge/Quilt/NeoForge loaders, Modrinth (search/install/update + sidecars),
  `.mrpack` install/import, CurseForge (integrated public key)

### Stores (plugin) — [guide](https://github.com/Matts-lab69/CorkyTux-Plugins/blob/main/heroic-store/README.md)
- Epic + GOG via own `legendary`/`gogdl` binaries (`setup` downloads to
  `plugins/heroic-store/bin/`), own session
- Embedded WebKit Epic login with auto-capture, GOG token auth
- Library with covers, detail view, free promos (100% only), Epic deals
  (%/price/end verified), GOG store search + buy with prices
- Installs go to native library (`~/Games/Heroic`) with launcher Proton/prefix

### Emulators & tools (plugins)
- **Emulator manager** (`emulator-manager`) — 12+ retro emulators
  (Dolphin, PCSX2, PPSSPP, RPCS3, Ryujinx, melonDS, Mupen64Plus, DuckStation,
  Cemu, Vita3K, Azahar…): AppImage install or link existing (native/Flatpak),
  ROM auto-detect, per-emulator executors in Add Game
- **AppImage launcher** (`appimage-launcher`) — [guide](https://github.com/Matts-lab69/CorkyTux-Plugins/blob/main/appimage-launcher/README.md):
  scan, integrate into `~/Applications`, detached launch
- **RPG Maker runtime** (`rpgmaker-runtime`) — [guide](https://github.com/Matts-lab69/CorkyTux-Plugins/blob/main/rpgmaker-runtime/README.md):
  MV/MZ (NW.js) and 2000/2003 (EasyRPG) via bundled box-rpg
- **Dependency installer** (`dependency-installer`) — smart detection of missing
  Windows components (VC++, DirectX, .NET…): DLL-import scan, only what Wine/Proton lacks
- **DLL overrides automator** (`dll-overrides-automator`) — scans game folders,
  generates `WINEDLLOVERRIDES`, patch/unpatch with undo

### Plugin system
- Streaming JSON-lines protocol, 50ms pump (fixes 100% CPU idle busy-loop)
- Settings > Plugins manager. Entry points (Minecraft/Store buttons,
  AppImage/RPG Maker cards) only appear when the plugin is installed.
  Plugins install **separately** — see
  [CorkyTux-Plugins](https://github.com/Matts-lab69/CorkyTux-Plugins)

---

## Installation

> **Do NOT use `sudo`** — installs to `~/.local/`.

```bash
# 1. Download corkytux-3.0.6-linux-x86_64.tar.gz from Releases
tar -xzf corkytux-3.0.6-linux-x86_64.tar.gz
cd corkytux-3.0.6 && ./install.sh
# Run: corkytux  (or app menu > CorkyTux)
# Uninstall: ./uninstall.sh
```

Binary: `~/.local/share/corkytux/corkytux`
Config: `~/.config/CorkyTux/` (`Launcher.ini`, `Games.ini`, `prefixes/`, `banners/`, `icons/`)
Plugins (installed by launcher): `~/.local/share/CorkyTux/plugins/`

---

## Dependencies

### Runtime (required)

| Lib | Gentoo | Debian/Ubuntu | Fedora | Arch | openSUSE |
|-----|--------|---------------|--------|------|----------|
| GTK 4.12+ | `gui-libs/gtk:4` | `libgtk-4-1` | `gtk4` | `gtk4` | `gtk4` |
| libadwaita 1.4+ | `gui-libs/libadwaita` | `libadwaita-1-0` | `libadwaita` | `libadwaita` | `libadwaita` |

`install.sh` checks `libgtk-4.so.1`, `libadwaita-1.so.0` (+ pango/cairo/pixbuf/glib) and
prints the exact install command for your distro.

### Runtime (optional, by feature)

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

### Build (see [docs/BUILD.md](docs/BUILD.md))

`cargo` + `rustc` 1.70+, `pkg-config`, `gcc`, `libgtk-4-dev` + `libadwaita-1-dev`
(names vary per distro — full table in BUILD.md).

---

## Architecture

```
src/main.rs             Adw window (1200x700, min 640x400), pages games/minecraft/stores
src/ui/sidebar.rs       library, filters, search, Minecraft/Store toggles
src/ui/center.rs        Recently Played cards
src/ui/details_panel.rs banner, Play/Stop, async cached install size
src/ui/game_settings.rs Proton/prefix, EAC/BE, EOS, WineVDesktop, env, args
src/ui/settings.rs      visuals, paths, protons, plugins, integrations, about
src/ui/minecraft_view.rs MC library/detail/browse (Modrinth/CurseForge)
src/ui/stores_view.rs   Epic/GOG tabs, session, library, promos, deals
src/backend/proton.rs   Proton/umu launch, EAC/BE envs, legendary launch, playtime/logs
src/backend/plugin_process.rs  JSON-lines spawn + 50ms pump
src/backend/game_model.rs GameEntry <-> Games.ini
src/backend/config.rs   Launcher.ini + Games.ini
```

---

## Credits

- [@Cristioro](https://github.com/Cristioro) — UI ideas and suggestions
  to make the code more efficient.
- [christvh / box-rpg](https://gitlab.com/christvh/box-rpg) — I used the logic
  from this repository and adapted it to a UI (RPG Maker runtime).
- [@ZzEdovec](https://github.com/ZzEdovec) — for some interfaces and icons
  I used or took inspiration from this person's repositories.

---

## License

AGPL-3.0 — see [LICENSE](LICENSE).
