<p align="center">
  <img src="qml/assets/corkytux.png" width="120" alt="CorkyTux Logo">
</p>

<h1 align="center">CorkyTux</h1>

<p align="center">
  <strong>Native Linux Game Launcher</strong><br>
  Run Windows games via Proton/Wine and manage retro emulators from one place.
</p>

<p align="center">
  <a href="#features">Features</a> •
  <a href="#supported-emulators">Emulators</a> •
  <a href="#installation">Installation</a> •
  <a href="#building-from-source">Build</a> •
  <a href="#plugins">Plugins</a> •
  <a href="#architecture">Architecture</a> •
  <a href="#contributing">Contributing</a> •
  <a href="#license">License</a>
</p>

<p align="center">
  <img src="https://img.shields.io/badge/C%2B%2B-20-blue?logo=cplusplus&logoColor=white" alt="C++20">
  <img src="https://img.shields.io/badge/Qt-6.4%2B-green?logo=qt&logoColor=white" alt="Qt 6.4+">
  <img src="https://img.shields.io/badge/Platform-Linux-orange?logo=linux&logoColor=white" alt="Linux">
  <img src="https://img.shields.io/badge/License-AGPL--3.0-purple" alt="License">
</p>

---

## What is CorkyTux?

CorkyTux is a **native Linux game launcher** built from the ground up for performance and simplicity. It unifies two worlds:

- **Windows gaming on Linux** — Run your Windows game library through Proton or Wine with automatic prefix management, GE-Proton downloads, and per-game configuration.
- **Retro emulation** — Manage all your favorite emulators from a single interface. Install, configure, and launch retro games without switching between apps.

Built with **C++20** and **Qt6 QML**, CorkyTux delivers a GPU-composited interface at 60fps with near-zero idle memory usage.

---

## Features

### Game Management
- **Unified library** — All your games in one place, regardless of source
- **Smart scanning** — Automatically discover games from Steam and Lutris installations
- **Artwork pipeline** — Auto-fetch banners, icons, and covers from SteamGridDB, ProtonDB, and IGDB
- **Per-game settings** — Custom Proton version, prefix path, DLL overrides, environment variables, and launch arguments
- **Favorites & sorting** — Filter by All, Favorites, A-Z, Most Played, or Recently Added
- **Time tracking** — Automatic play time recording

### Proton/Wine Support
- **GE-Proton management** — Download, install, and switch between GE-Proton versions directly from the launcher
- **Per-game Proton versions** — Choose which Proton build runs each game
- **Prefix management** — Automatic Wine prefix creation and configuration
- **DLL overrides** — Configure WINEDLLOVERRIDES per game
- **Environment variables** — Set custom env vars for each game
- **Steam overlay toggle** — Enable/disable Steam overlay per game
- **wined3d/DXVK toggle** — Switch between rendering backends
- **Wayland support** — Native Wayland driver preference

### Emulator Integration
- **12+ emulators supported** — Dolphin, PCSX2, PPSSPP, RPCS3, Ryujinx, and more
- **AppImage install** — Download emulator AppImages directly from GitHub releases
- **Link existing emulators** — Use emulators you already have installed
- **ROM detection** — Auto-detect game files by extension
- **Per-emulator settings** — Configure each emulator's options from CorkyTux

### Plugin System
- **Extensible architecture** — Install plugins to add new capabilities
- **Dependency installer** — Auto-detect and install missing Windows components (VC++, DirectX, .NET)
- **DLL overrides automator** — Intelligent DLL override management
- **Emulator manager** — Full emulator lifecycle management

### UI/UX
- **Dark & Light themes** — Choose your preferred look
- **10 accent colors** — Customize the launcher's accent color
- **GPU-composited** — Smooth 60fps animations via Qt Quick
- **Responsive design** — Adapts to different screen sizes
- **Minimal footprint** — <60MB idle memory usage

---

## Supported Emulators

| Emulator | System | Extensions |
|----------|--------|------------|
| Dolphin | GameCube / Wii | `.iso`, `.gcm`, `.wbfs`, `.rvz`, `.wia` |
| PCSX2 | PlayStation 2 | `.iso`, `.bin`, `.img`, `.mdf`, `.nrg` |
| PPSSPP | PlayStation Portable | `.iso`, `.cso` |
| RPCS3 | PlayStation 3 | `.pkg`, `.iso` |
| Ryujinx | Nintendo Switch | `.nca`, `.xci`, `.nsp` |
| melonDS | Nintendo DS | `.nds`, `.srl`, `.dsi` |
| DeSmuME | Nintendo DS | `.nds`, `.srl`, `.dsi` |
| Mupen64Plus | Nintendo 64 | `.z64`, `.n64`, `.v64`, `.rom` |
| DuckStation | PlayStation 1 | `.iso`, `.bin`, `.cue`, `.img`, `.mdf` |
| Cemu | Wii U | `.wud`, `.wua`, `.rpx`, `.wux` |
| Vita3K | PlayStation Vita | `.vpk` |
| Azahar | Nintendo 3DS | `.3ds`, `.cia`, `.3dsx` |

---

## Installation

### Pre-built Release

Download the latest release from the [Releases page](https://github.com/Matts-lab69/CorkyTux-Launcher/releases).

```bash
# Extract and run
tar -xzf corkytux-*.tar.gz
cd corkytux
./corkytux
```

### Package Managers

*(Coming soon — packages for major distros)*

---

## Building from Source

### Prerequisites

| Dependency | Version |
|------------|---------|
| Qt | 6.4+ (Core, Quick, QuickControls2, Network, Concurrent, Svg) |
| CMake | 3.21+ |
| GCC | With C++20 support |
| sqlite3 | CLI (for Lutris pga.db scan) |

### Build

```bash
# Clone the repository
git clone https://github.com/Matts-lab69/CorkyTux-Launcher.git
cd CorkyTux-Launcher

# Configure
cmake -S . -B build -DCMAKE_BUILD_TYPE=Release

# Build
cmake --build build -j$(nproc)

# Run
./build/corkytux
```

### Debug Build

```bash
cmake -S . -B build -DCMAKE_BUILD_TYPE=Debug
cmake --build build -j$(nproc)
./build/corkytux
```

---

## Plugins

CorkyTux supports plugins to extend its functionality. Install plugins from the **Settings > Plugins** tab.

### Available Plugins

| Plugin | Description |
|--------|-------------|
| **Dependency Installer** | Auto-detect and install missing Windows components (VC++, DirectX, .NET) |
| **DLL Overrides Automator** | Intelligent DLL override management for game compatibility |
| **Emulator Manager** | Full emulator lifecycle — install, link, configure, and launch |

### Installing Plugins

1. Open **Settings** → **Plugins**
2. Browse the **Available Plugins** section
3. Click **Install** on the plugin you want
4. Enable the plugin with the toggle switch

### For Plugin Developers

Plugins are distributed as standalone executables with a `plugin.json` manifest. See the [CorkyTux-Plugins repository](https://github.com/Matts-lab69/CorkyTux-Plugins) for examples and documentation.

---

## Architecture

```
corkytux-qt/
├── src/
│   ├── main.cpp                    # Entry point, singleton wiring
│   └── backend/
│       ├── ConfigManager.h/.cpp    # XDG-aware INI persistence
│       ├── GameModel.h/.cpp        # Game list model, filtering, recent
│       ├── ProtonManager.h/.cpp    # Proton/Wine lifecycle, GE-Proton download
│       ├── IntegrationManager.h/.cpp # Steam/Lutris scans, artwork, ProtonDB/SGDB/IGDB
│       ├── PluginManager.h/.cpp    # Plugin discovery, registry, emulator-manager
│       └── ThemeManager.h/.cpp     # Dark/light + 10 accent colors
├── qml/
│   ├── Main.qml                    # ApplicationWindow, top bar, sidebar, center
│   ├── Sidebar.qml                 # Library filters + search + game list
│   ├── DetailsPanel.qml            # Floating right overlay with game info
│   ├── GameCard.qml                # 224x140px game tile
│   ├── AddGameModal.qml            # Add game wizard (executor selection)
│   ├── GameSettingsModal.qml       # Per-game config (View/Run/Graphics tabs)
│   ├── SettingsModal.qml           # Global settings (7 tabs)
│   ├── RemoveModal.qml             # Game removal confirmation
│   ├── LogModal.qml                # Game log viewer
│   ├── ProtonModal.qml             # GE-Proton download manager
│   └── components/                 # Reusable UI components
│       ├── CButton.qml             # Primary/outline/action/icon buttons
│       ├── CCard.qml               # Rounded panel container
│       ├── CModal.qml              # Centered dialog with overlay
│       ├── CSwitch.qml             # Accent toggle switch
│       ├── CTextField.qml          # Themed text input
│       ├── CComboBox.qml           # Themed dropdown
│       └── CIcon.qml               # SVG icon renderer
└── CMakeLists.txt                  # Build configuration
```

### Data Storage

| Data | Location |
|------|----------|
| Game list | `~/.config/CorkyTux/Games.ini` |
| Launcher settings | `~/.config/CorkyTux/Launcher.ini` |
| Game banners | `~/.config/CorkyTux/banners/` |
| Game icons | `~/.config/CorkyTux/icons/` |
| Wine prefixes | `~/.config/CorkyTux/prefixes/` |
| Proton builds | `~/.config/CorkyTux/protons/` |

---

## Integrations

| Service | Purpose | API Key Required |
|---------|---------|------------------|
| **Steam** | Import installed games | No |
| **Lutris** | Import Lutris games | No |
| **ProtonDB** | Compatibility ratings | No |
| **SteamGridDB** | Game artwork | No (bundled key) |
| **IGDB** | Game info & ratings | Yes (Twitch ClientID:Secret) |

---

## Contributing

Contributions are welcome! Please feel free to submit a Pull Request.

1. Fork the repository
2. Create your feature branch (`git checkout -b feature/amazing-feature`)
3. Commit your changes (`git commit -m 'Add amazing feature'`)
4. Push to the branch (`git push origin feature/amazing-feature`)
5. Open a Pull Request

---

## License

This project is licensed under the **GNU Affero General Public License v3.0** — see the [LICENSE](LICENSE) file for details.

---

## Acknowledgments

- [Proton](https://github.com/ValveSoftware/Proton) — Valve's compatibility layer for running Windows games on Linux
- [GE-Proton](https://github.com/GloriousEggroll/proton-ge-custom) — GloriousEggroll's custom Proton build
- [Wine](https://www.winehq.org/) — Windows compatibility layer
- [Qt](https://www.qt.io/) — Cross-platform application framework

---

<p align="center">
  Made with care for the Linux gaming community.
</p>
