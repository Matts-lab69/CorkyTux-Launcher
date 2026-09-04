<p align="center">
  <img src="qml/assets/corkytux.png" width="100" alt="CorkyTux Logo">
</p>

<h1 align="center">CorkyTux</h1>

<p align="center">
  <strong>Native Linux game launcher for Windows games & retro emulators</strong><br>
  Run your library with Proton/Wine, manage emulators, and preserve your games.
</p>

<p align="center">
  <a href="https://github.com/Matts-lab69/CorkyTux-Launcher/releases/tag/qt-v2.10.1">Download</a> •
  <a href="#features">Features</a> •
  <a href="#installation">Install</a> •
  <a href="#plugins">Plugins</a> •
  <a href="#build-from-source">Build</a>
</p>

---

## What is CorkyTux?

CorkyTux is a **native Linux game launcher** built with C++20 and Qt6/QML. It lets you run Windows games on Linux using Proton and Wine, manage retro console emulators, and keep your entire game library organized in one place.

Like Lutris, Bottles, or Heroic, CorkyTux exists to make gaming on Linux seamless. But it goes further: it's designed from the ground up for **game preservation** — keeping your saves, prefixes, and configurations safe across system changes, while giving you full control over every layer of compatibility.

Whether you're running a AAA title through GE-Proton or booting a Nintendo DS ROM through melonDS, CorkyTux handles it all from a single, clean interface.

---

## Features

### Game Management
- **Add any game** — Windows executables, installers, or ROM files
- **Automatic artwork** — fetches banners and icons from Steam, SteamGridDB, Lutris, and ProtonDB
- **Favorites & recently played** — quick access to what matters
- **Play time tracking** — know how many hours you've sunk into each game
- **Search & filter** — find games by name, sort by most played or recently added

### Proton & Wine
- **One-click Proton downloads** — installs GE-Proton and CachyOS builds directly from GitHub
- **Automatic prefix creation** — each game gets its own isolated Wine prefix
- **VC++ runtime setup** — installs Visual C++ redistributables on first launch
- **DXVK / wined3d toggle** — switch between Vulkan and OpenGL rendering per game
- **Steam Runtime support** — optional wrapper for Valve's containerized runtimes
- **Custom environment variables** — full control over WINEPREFIX, WINEDLLOVERRIDES, and more
- **Wine tools** — launch winecfg, taskmgr, explorer, or cmd in any game's prefix

### Integrations
- **Steam** — scans native and Flatpak libraries, imports installed games automatically
- **Lutris** — reads pga.db and YAML configs, imports your existing Lutris library
- **ProtonDB** — shows compatibility ratings for any Steam game
- **SteamGridDB** — fetches high-quality cover art and icons
- **IGDB** — game ratings and summaries via Twitch API

### Retro Emulation
- **Emulator Manager plugin** — install and manage emulator AppImages from one place
- **12 supported emulators** — melonDS, Dolphin, PCSX2, PPSSPP, RPCS3, DuckStation, Cemu, and more
- **ROM scanning** — automatically detects game files by extension
- **Per-emulator config** — launch arguments, game directory registration

### Plugin System
- **Extendable architecture** — plugins add new capabilities via a simple JSON protocol
- **DLL Overrides Automator** — automatic WINEDLLOVERRIDES configuration for multiplayer fixes
- **Scan plugins** — detect Steam AppIDs and compatibility layers automatically
- **Plugin marketplace** — download plugins directly from GitHub releases

### Interface
- **Dark & Light themes** — with 10 accent colors to choose from
- **Responsive layout** — adapts from 860px to fullscreen
- **Sidebar navigation** — filter your library with a single click
- **Details panel** — game info, play controls, and settings in a slide-out overlay
- **Toast notifications** — non-intrusive feedback for actions

---

## Installation

### Download the release

```bash
# Extract to your preferred location
tar xzf corkytux-2.10.1-linux-x86_64.tar.gz
cd corkytux-qt
./build/corkytux
```

### Dependencies

- **Qt 6.4+** — Core, Gui, Quick, Network, Concurrent, Svg
- **CMake 3.21+**
- **C++20 compiler** (GCC or Clang)
- **sqlite3** — for Lutris library import
- **unrar or 7z** — for RAR extraction (optional)
- **icoextract + ffmpeg** — for .exe icon extraction (optional)

---

## Build from source

```bash
git clone https://github.com/Matts-lab69/CorkyTux-Launcher.git
cd CorkyTux-Launcher

cmake -S . -B build -DCMAKE_BUILD_TYPE=Release
cmake --build build -j$(nproc)

./build/corkytux
```

---

## Plugins

CorkyTux supports plugins that extend its functionality. Plugins are downloaded and managed from within the launcher.

| Plugin | Description |
|--------|-------------|
| **DLL Overrides Automator** | Automatically configures DLL overrides for Windows games |
| **Emulator Manager** | Install and manage retro emulator AppImages |

Plugins are available from the [CorkyTux-Plugins](https://github.com/Matts-lab69/CorkyTux-Plugins) repository.

---

## How it works

1. **Add a game** — point CorkyTux to an `.exe`, installer, or game directory
2. **Select a Proton version** — choose from installed builds or download a new one
3. **Play** — CorkyTux creates a Wine prefix, configures the environment, and launches the game
4. **Artwork resolves automatically** — banners and icons are fetched from Steam, SteamGridDB, or Lutris

For emulators, install one through the Emulator Manager plugin, add your ROMs, and play.

---

## Directory structure

```
~/.config/CorkyTux/
├── Games.ini          # Your game library
├── Launcher.ini       # Settings & integrations
├── banners/           # Downloaded cover art
└── icons/             # Game icons

~/.local/share/CorkyTux/
├── prefixes/          # Wine prefixes (one per game)
├── protons/           # Installed Proton builds
└── plugins/           # Installed plugins
```

---

## License

CorkyTux is open source software. See [LICENSE](LICENSE) for details.
