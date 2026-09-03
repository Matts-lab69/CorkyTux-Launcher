# OnlineFix Linux Launcher v2.8.0 — Full Changelog

## NEW: Theme Colors (Accent Colors)

Complete accent color system with 10 colors that changes all main buttons, toggles, scrollbars, checkboxes, tooltips, progress bars, list-cell selections and more.

- **AccentColorManager** — Singleton that generates a CSS override (`accent-override.css`) applied on top of the base `style.fx.css`
- **10 colors**: Green, Blue, Cyan, Purple, Pink, Red, Orange, Yellow, Teal, Indigo
- **Visuals tab** in LauncherSettings with 10 color swatches
- **SwitchComponent** reads colors directly from AccentColorManager for its gradient
- **~60 UI elements** affected by accent color
- Persisted in `Launcher.ini` under `[User Settings].accentColor`

### Elements that change color:
- 22 `.jfx-button` buttons (MainForm, GameSettings, LauncherSettings, EnvViewer, NewGameConfigurator)
- Play button `#playButton`
- Yes button `confirm-button` (GameRemover)
- 12 SwitchComponent instances (all toggles)
- ScrollBar thumbs
- CheckBox marks (GameRemover)
- Text input highlights (TextField, TextArea)
- List-cell selections (ComboBox, ListView)
- Table row selections (EnvViewer)
- Toggle switch selected state
- Progress bar (ProtonDownloader)
- Tooltips (MainForm)
- Table column headers
- `#addGame`, `#selectFileButton`
- `.accent-badge` (version label)

---

## CRITICAL FIXES (crashes and data loss)

### v2.8.0
| Bug | File | Description |
|-----|------|-------------|
| SwitchComponent onAction | GameSettings.java | 5 toggles (steamOverlay, steamRuntime, noSteamPath, wined3d, wayland) used `Button.setOnAction()` which never fired when clicking the SwitchComponent graphic. Moved to `SwitchComponent.setOnToggle()` |
| SwitchComponent onAction | LauncherSettings.java | 4 toggles (fullscreen, requestSteam, wined3d, wayland) had the same bug. Fixed |
| setSelected vs setSelectedSilent | GameSettings.java | `updateToggleStates()` used `setSelected()` which triggered callbacks during init. Changed to `setSelectedSilent()` |
| RAR retry discarded | NewGameConfigurator.java:471 | When `retryWithEnsureError` returned a successful result, the code did `return` without calling `prepareForGame()`. Game was never configured |
| FtpInstaller Map vs GameParams | FtpInstaller.java:301 | Passed `Map.of()` instead of a real `GameParams` object. `trySetField` failed silently. Added public `setGameParams()` setter |
| Second Wini race condition | LauncherSettings.java:817 | Second `Wini` instance to delete `defaultProton` — race condition with AppModule. Removed, `setLauncher(key, null)` already handles cleanup |
| Second Wini stale data | GameSettings.java:730 | Second `Wini` instance to read section before rename — stale data. Removed, uses `appModule.getGameSection()` |
| Play button stuck | MainForm.java:1428 | Play button stayed in "wait" state permanently when `generateProcess` returned null after proton auto-download |
| LD_PRELOAD overwrite | FilesWorker.java:268 | `LD_PRELOAD` overwrote existing value, breaking MangoHud, gamemoderun and other injectors. Now preserves parent LD_PRELOAD |
| LD_PRELOAD overlay files | FilesWorker.java:267 | Added Steam overlay `.so` files to LD_PRELOAD without checking if they existed. Flatpak and different Steam versions failed |
| findNewestAvailableProton | FilesWorker.java:1184 | Only searched the primary proton path. Now searches all 3 configured paths |
| fetchLatestProton sync | AppModule.java:260 | `FilesWorker.setLatestProtonUrl()` was called directly without syncing through the static sync method |
| AppModule singleton | AppModule.java:82 | Public constructor allowed creating a second instance. Changed to `private` |
| NPE getParent() | NewGameConfigurator.java:628 | `Path.of(originalFile).getParent().toString()` caused NPE when `getParent()` returned null |
| NPE currentGameName | MainForm.java:1734 | gameFolderMenu lambda accessed `currentGameName` which could be null |
| NPE gameName | NewGameConfigurator.java:515 | `gameName.getText()` without null-check on `@FXML` field |
| AccentColorManager paths | AccentColorManager.java | Used `System.getProperty("user.home")` instead of `FilesWorker.getExpectedHome()`. Now persists through `AppModule.setLauncher()` |
| Path consistency | Multiple files | 13 references to `System.getProperty("user.home")` replaced with `FilesWorker.getExpectedHome()` for root safety |
| accent-override.css path | Launcher.java | Used `user.home` instead of `getExpectedHome()` |
| FXML inline styles | 3 files | `#addGame`, `#selectFileButton`, Yes button had inline `-fx-background-color:#55de1b` that ignored the accent override |

---

## IMPROVEMENTS vs Original Launcher (PHP/JPHP)

### Complete port to pure Java
- **19 PHP files** → **~14,500 LOC Java** + **11 FXML** → **~1,250 LOC FXML**
- Removed DevelNext/JPHP runtime dependency
- Executable as standard JAR with JavaFX
- Compatible with Adoptium Temurin 21+

### Multi-distro and portability
- **Flatpak Steam** — Detects `~/.var/app/com.valvesoftware.Steam/data/Steam`
- **3 proton paths** — Configure up to 3 proton directories
- **Multi-user** — `FilesWorker.getExpectedHome()` resolves `/home/<user>` even when running as root
- **Portable** — Works on most Linux distributions
- **Wrapper script** — `~/.local/bin/ofll` for direct execution
- **Installer script** — `install.sh` for automatic setup

### Steam detection
- Custom VDF parser (`VdfParser.java`) instead of external library dependency
- Regex fallback for corrupted VDFs
- ini4j fallback for non-standard libraryfolders
- Login users detection for Steam ID
- Flatpak Steam path detection

### Proton management
- **Auto-download** of GE-Proton when no proton is installed
- **3 proton paths** with automatic detection
- **"GE-Proton Latest"** dynamic resolution from GitHub API
- **ProtonDownloader** with progress bar
- **Proton cell factory** with installed vs available version preview
- **Filter combo** to search proton by name
- **Multiple path info** — each proton shows " - Path X"

### UI/UX
- **Theme Colors** — 10 accent colors with persistence
- **Banner system** — Loads banners from Steam CDN with multi-CDN fallback
- **Icon extraction** — Extracts icons from .exe via wrestic
- **Game tiles** with banners and icons
- **Stub games** — Visual placeholder while adding a game
- **Toast notifications** — Instead of invasive modals
- **Context menus** — Game folder, utilities, proton management
- **Dark theme** — Hardcoded CSS with dark panels

### Game management
- **Game Remover** — Deletion with options (files, prefix, shortcuts)
- **Rename game** — Name change with .desktop entry updates
- **Desktop shortcuts** — Create/delete on Desktop and App Menu
- **Environment editor** — Per-game environment variables
- **Game settings** — Steam Overlay, Steam Runtime, WineD3D, Wayland, Fake Steam
- **Proton selection** — Per-game with default fallback
- **Prefix management** — Automatic creation, move between paths
- **Args before/after** — Custom pre/post executable arguments

### Code quality
- **Singleton pattern** — AppModule with double-checked locking
- **Thread safety** — `synchronized` on INI operations, `volatile` on shared fields
- **Virtual threads** — Game runs on `Thread.ofVirtual()` without blocking UI
- **Error handling** — Try-catch on file operations, SLF4J logging
- **Localization** — Keys in `en.json` and `ru.json`
- **CSS override pattern** — Accent colors without modifying base CSS

### FixParser improvements
- Multi-CDN banner download (steam, steamcdn, steamstatic)
- Fallback banner by steamID
- Icon extraction with multiple paths
- Error string not stored as banner

### FtpInstaller
- Headless mode for auto-install
- Progress tracking
- Prefix cleanup on failure
- Remote game configuration via reflection

---

## KNOWN ISSUES (intentionally unfixed)

### Race conditions (not noticed in normal use)
| Bug | Description | Impact |
|-----|-------------|---------|
| INI write race | `FilesWorker.setIniProperty` and `AppModule.setGame` can write simultaneously | Settings may be lost during rapid cascading clicks |
| Dual Wini | `FilesWorker` reads INI from disk, `AppModule` has in-memory copy | Possible stale data between modules |

### Known limitations
| Issue | Description |
|-------|-------------|
| splitArgs doesn't handle quotes | Arguments with spaces in quotes are broken (same as PHP original) |
| GE-Proton-only version detection | `findSteamRuntime` regex only matches GE-Proton format |
| Wini doesn't close file handles | `new Wini()` is never closed (minor file descriptor leak) |
| Thread pool leak | `hookProcessOuts` executor never `awaitTermination` (daemon threads, doesn't block JVM) |

---

## Files modified (v2.8.0)

### Java (11 files)
- `AccentColorManager.java` — NEW: Accent color system
- `AppModule.java` — Private constructor, sync proton URL, getExpectedHome
- `FilesWorker.java` — findNewestAllPaths, LD_PRELOAD fix, getExpectedHome
- `FtpInstaller.java` — GameParams fix, setGameParams reflection
- `Launcher.java` — attachAccentOverride, getExpectedHome paths
- `LauncherSettings.java` — Visuals tab, accent swatches, Wini race fix
- `MainForm.java` — Play button fix, NPE fixes, getExpectedHome paths
- `NewGameConfigurator.java` — RAR retry fix, NPE fixes, getExpectedHome paths
- `GameSettings.java` — SwitchComponent fix, setSelectedSilent, Wini fix
- `SwitchComponent.java` — Accent color gradient, setOnToggle pattern

### FXML (5 files)
- `launcherSettings.fxml` — Visuals tab, accent-badge class
- `MainForm.fxml` — Removed inline background on #addGame
- `gameRemover.fxml` — confirm-button class, removed inline style
- `log.fxml` — Removed jfx-button from Save/Telegram/Close
- `newGameConfigurator.fxml` — Removed inline background from buttons

### Locale (2 files)
- `en.json` — LAUNCHERSETTINGS.TABS.VISUALS
- `ru.json` — LAUNCHERSETTINGS.TABS.VISUALS

### CSS (0 files)
- `style.fx.css` — NOT MODIFIED (accent colors via override)
