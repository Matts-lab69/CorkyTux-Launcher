# CorkyTux Qt — C++20 + QML native launcher

Native launcher: zero dependencies beyond Qt, GPU-composited QML at 60fps, <60MB idle target.

## Build
```
cmake -S . -B build -DCMAKE_BUILD_TYPE=Release
cmake --build build -j$(nproc)
./build/corkytux
```
Requires: Qt 6.4+ (Core, Quick, QuickControls2, Network, Concurrent, Svg),
CMake 3.21+, GCC with C++20. sqlite3 CLI (for Lutris pga.db scan).

## Layout
- `src/main.cpp` – entry, singleton + model wiring, context properties.
- `src/backend/` – ConfigManager (INI+XDG), GameModel (+GameFilterProxy,
  RecentModel), ProtonManager (QProcess run/stop, GitHub GE-Proton download),
  IntegrationManager (Steam/Lutris scans, artwork chain, ProtonDB, SGDB, IGDB),
  ThemeManager (dark/light + 10 accents as QML properties).
- `qml/` – Main, Sidebar, GameCard, DetailsPanel, AddGameModal,
  SettingsModal (7 tabs), GameSettingsModal, RemoveModal, LogModal,
  ProtonModal, components (CButton/CCard/CModal/CSwitch/CTextField).

## Parity notes (vs Java)
- Same INI keys/paths (~/.config/CorkyTux, prefixes, banners, icons).
- Same filter modes, favorites, lastPlayed/timeSpent semantics.
- Artwork chain order identical; bundled SteamGridDB key identical.
- Modals mirror Java modal overlays; hide paths never kill the main window.
- Details overlay floats right with fast slide-in, instant hide.
