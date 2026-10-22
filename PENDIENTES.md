# PENDIENTES.md

Open items only. What is already fixed lives in the git history and in the
merged pull requests. The strategy and the milestones live in
`docs/ROADMAP.md`; this file holds the concrete defects, with the place
where each one lives.

Rule: nothing is declared fixed without evidence. The items that need me
looking at the app are marked as such, because the agent does not run the
app.

The IDs of the multi-distro audit (`docs/multidistro-audit.md`) are kept
where the item comes from there. The `N` items are new findings.

| ID | Item | Where | Priority | Covered by | Needs my eyes |
|---|---|---|---|---|---|
| D05 | The plugin download has no checksum and no signature, and the archive is unpacked with `tar -xf` | `src/backend/plugins.rs:472-540` (size check only), `src/backend/plugins.rs:534` | P0 | roadmap #5, #8 | no |
| C08 | 12 sites still call external binaries (`pgrep`, `pidof`, `which`): 6 `pgrep`, 2 `pidof`, 4 `which` | `src/backend/proton.rs:452,1079,1083,1175,1179,2115,2420,2577,2600,2610,3545`, `src/backend/plugins.rs:1192` | P1 | Launcher #3 (timeouts), Launcher #5 (group kill) | no |
| D06 | The GitHub API is called without authentication: 60 requests per hour and per IP, so a NAT or CGNAT network gets a 403 | `src/backend/plugins.rs:384` | P1 | roadmap #4 | no |
| D02, D26 | The Steam and GameMode detection depends on `which steam` and `pidof steam`, so a Flatpak or Snap Steam is not seen | `src/backend/proton.rs:1079,1083,1175,1179`, `release/install.sh:139` | P1 | | no |
| D12, D13, D15 | The generated `.desktop` has an unquoted `Exec=`, an unescaped `Name=`, `Comment=` and `X-CorkyTux-Game=`, and mode 0755 | `src/backend/shortcuts.rs:134-143`, `src/backend/shortcuts.rs:157,180` | P1 | | no |
| N01 | The plugin does not emit the Epic catalog version nor the date: it read `app_version`, a field `legendary list --json` does not expose, so the version line of the modal never appeared. The old `PENDIENTES.md` claimed this was fixed: it was not, the change never reached any branch (`git log --all -S last_updated` returns nothing, the stash does not contain it and the installed copy was byte identical to `origin/main`) | `CorkyTux-Plugins` `heroic-store/heroic-store:946` (before the change) | P1 | Plugins #7 (the catalog version and date) | no |
| D16, D17 | The plugin directory ignores `XDG_DATA_HOME`, and `plugin_available` checks the existence but not the executable bit | `src/backend/plugin_process.rs:102-105`, `src/backend/plugin_process.rs:111-114` | P1 | roadmap #13 | no |
| N02 | The Epic exchange code has to travel in the game argv: the game reads it from `-auth_password` and there is no environment or file alternative. The argv is readable by every local user | `src/backend/proton.rs:1162-1170` | P1 | Launcher #6 (redacted copies) | no |
| D03, D04 | The architecture detection only knows `0x10b` and `0x20b`, and the foreign architecture check only contrasts aarch64 against x86_64 | `src/backend/proton.rs:968-985`, `src/backend/proton.rs:665-672` | P2 | | no |
| D14 | The app menu directory is hardcoded to `~/.local/share/applications`; `xdg-user-dir` is only used for the desktop | `src/backend/shortcuts.rs:35`, `src/backend/shortcuts.rs:39-53` | P2 | | no |
| D01 | `all_proton_paths` only scans the user paths: the Steam `compatibilitytools.d` and the package manager Proton are never listed | `src/backend/config.rs:600-618` | P2 | | no |
| D11 | The Lutris scan does not cover the Flatpak `libraryfolders.vdf` | `src/backend/integration.rs:1192-1204` | P2 | | no |
| D18, D19 | `$HOME` is read in many places, and an empty `HOME` leaves a relative path, so the import would write into the current directory | `src/backend/proton.rs:9`, `src/backend/plugins.rs:8`, `src/backend/integration.rs:8`, `src/backend/shortcuts.rs:34`, `src/ui/import_manager.rs:370` | P2 | | no |
| D09 | The icon extraction needs `icoextract` or `ffmpeg` and degrades to a generic icon with no warning | `src/backend/integration.rs:616,632,643` | P2 | | no |
| D07 | The plugin description is cut at 200 characters, in the middle of a sentence, and the raw markdown is shown | `src/backend/plugins.rs:451` | P3 | roadmap #24 | no |
| D20 | The icon theme only adds the installed assets path, so a run from the source tree finds no packaged asset | `src/main.rs:280` | P3 | | no |
| K02, K03, K04 | The README architecture map leaves out six backend files, only the x86_64 tarball is documented, and the import text says "a Games folder in your /home", which is wrong on `/var/home` | `README.md:314-339`, `README.md:90`, `src/ui/import_manager.rs:19` | P3 | | no |
| N03 | `corkytux-web-browser-symbolic.svg` has no `viewBox` | `assets/icons/hicolor/symbolic/legacy/corkytux-web-browser-symbolic.svg` | P3 | | no |
| D22, D23, D24, D25, D27, D28, D29, D30 | The installer is not portable: the library sweep is hardcoded to `/usr/lib/x86_64-linux-gnu` and misses the NixOS path, it needs the `file` binary, it writes `Exec=` and `Icon=` without quotes, it recommends `pip install --user`, the tarball name is the raw `uname -m`, it creates `~/Games` on every install, and the PATH warning only mentions bash and zsh | `release/install.sh:64,81,118,139,153,172,222,266-267,284-285`, `release/build-release.sh:8` | P2 | | no |
| N04 | Epic and GOG logout and back in: nobody tried that cycle | stores view, Epic and GOG | P1 | | yes |
| N05 | The GOG login end to end in one clean run (navigate, capture, exchange, verify) | stores view, GOG | P1 | | yes |
| N06 | The icon sweep after the bundle rename: Library, Minecraft, Settings, Integrations and the sidebar | `src/ui/game_card.rs`, `src/ui/minecraft_view.rs`, `src/ui/settings.rs`, `src/ui/sidebar.rs` | P2 | | yes |
| N07 | The description cascade in light and dark: a stub game shows the version line, a game without metadata shows the neutral text | `src/ui/stores_view.rs:2663-2672` | P2 | | yes |
| N08 | The description modal: a short text has no scrollbar, a long text scrolls, the cover and the title are visible at the top and the description area stays around 300 px | `src/ui/stores_view.rs:2687-2700` | P2 | | yes |
| N09 | The store icon is the modern install glyph, not a padlock | `src/ui/sidebar.rs` | P2 | | yes |
| N10 | The size of the action icons and the Play/Stop cycle in light and dark | game card and details panel | P3 | | yes |
| N11 | The transient states of the emulator row: `Installing...` and the disabled add button | emulator row | P3 | | yes |
| N12 | The centering of the Papirus icons and of the monogram in the 48 px slot | emulator row | P3 | | yes |
| N13 | The static `Found in system` and the clickable `Set as linked` on the same row | emulator row | P3 | | yes |
| N14 | The `pixman_region32_init_rect` warning at startup: the number of warnings changes between runs with the same binary and the same assets | GTK, whole app | P3 | | yes |
