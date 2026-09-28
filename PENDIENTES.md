# PENDIENTES.md

Status of pending items and findings. Rule: nothing is declared "resolved"
without evidence; the items that need me looking at the app stay on hold for
the user (the agent does not run the app).

## Resolved (with evidence)

### Empty descriptions in the "View" modal (Library/Stores)
Commit: `50dfa79`
- Probe `examples/game_info_probe.rs` invoked `game-info` from the
  heroic-store plugin for the **21 games** in the legendary cache and
  cross-checked the result against the local metadata. Conclusion:
  **there is no pipeline bug**. Games with a real description (Mindcop,
  Voidwrought, Genshin, ZZZ, Marvel Rivals, Astral Ascent, Honkai Star
  Rail, River City Girls 2, Fortnite…) reach the modal with text.
- The ones that arrive empty are "stubs" at the source: legendary metadata
  has `description == title` (Fall Guys, VALORANT, DOOMBLADE, Rocket
  League, Shogun Showdown, Luftrausers, RCT3, LEGO Fortnite content,
  I Have No Mouth). The modal dropped `desc==title` and showed nothing.
- UX fix: when the description ends up empty the dimmed label
  "No description available" is shown (`opacity 0.6`, class `time-label`),
  with good contrast in light and dark theme. The TEMP-LOG from
  `game_info` was removed.

### GTK warnings "No property named: max-width/max-height"
Commits: `05bc3ab`
- Exact evidence in `/tmp/corkytux_run*.log`:
  `Theme parser error: <data>:1:5602-5611: No property named "max-width"`.
- GTK4 CSS does not define `max-width`/`max-height`. The real ceiling is set
  by `card.set_size_request(CARD_W, CARD_H)` in `game_card.rs` plus the
  `min-*` of the `.game-card` selector (`helpers.rs`). Both leftover
  properties were removed (`effeb9c`). Verified at startup: PID 25598,
  `/tmp/corkytux_run4.log`, zero CSS parser warnings.

### hicolor icon system
Commits: `5101d64` (+ `2f8119c`, `fd8b888` as support probes)
- Only real bundling path: universal hicolor fallback with the bundle
  path first in the `search_path` (`main.rs`). Leftover CorkyTux tree
  items removed from repo and runtime.
- Effective per-name resolution was demonstrated with
  `examples/icon_resolve_probe.rs` (replicating `for_display` + path
  prefix): with the active Mint-Breeze-Calm-Green theme most names
  resolve from the system theme; only 11 fall back to our bundle
  (epicgames, gogdotcom, applications-games,
  applications-engineering, alarm, display-brightness, non-starred,
  application-x-addon, image-x-generic, package-x-generic,
  text-x-generic), and of those **9 are byte-identical to Adwaita** (zero
  change versus before) while `epicgames`/`gogdotcom` are the custom
  brands that used to fail (intentional improvement).
- (Updated by T3: the 26 standard names became `corkytux-*` and now
  **37/37 resolve to the bundle**; see the "System theme icon
  independence" section below.)

### Cascaded description with Epic catalog version (T1)
Commit: `5419224`
- `legendary list --json` does NOT expose `app_version`; it does bring
  `asset_infos.{Windows|Other|Mac|Linux}.build_version` and
  `metadata.lastModifiedDate`. The plugin now uses the correct platform
  lookup and emits `version` and `last_updated` (`YYYY-MM-DD`) in
  `game-info`. Verified live: Sugar=`++Prime+Update60-CL-528314`/2026-09-02,
  Fall Guys=`EGS_11958`/2024-08-16, DOOMBLADE=`1.2`/2023-05-30,
  VALORANT=`2609-591`/2026-09-16.
- Modal: `ver` is only shown with a real description; if it is missing,
  cascade: `Latest version: {ver} (last update known to Epic: {date})`
  → `Latest version: {ver} (Epic catalog record)` → `No description available`.
  It is made clear the date is Epic's catalog date, not the local install.

### Description truncated with ellipsis in the View modal (T2)
Commit: `d2b5d50`
- Root cause: the label used `set_lines(6)` + `ellipsize End` (clips the
  text) and the existing "Read more" fired on `desc.lines().count() > 6`,
  which counts line breaks, not visual lines: a single long paragraph
  (Fortnite, 766 chars) **never** triggered the expander → unrecoverable.
- Fix: vertical `ScrolledWindow` (policy Automatic, `max_content_height` 200,
  vexpand) with the label wrapping fully and NO ellipsis. GTK measures the
  label's natural size and decides the scrollbar, the dialog stays bounded,
  no hardcoded char threshold (robust to font/width). Expander removed.
- Control: Mindcop (283 chars) generates no scroll; Fortnite (766) does.

### Independence from the system theme icons (T3)
Commit: `5e4f2dc`
- The 26 standard names in the bundle were being won by the active theme
  (Mint-Breeze-Calm-Green). They are renamed to `corkytux-<name>` (git mv)
  and the **53** references in `src/` are updated (`minecraft_view.rs` 40,
  `details_panel.rs` 5, `sidebar.rs` 4, `settings.rs` 2, `stores_view.rs` 1,
  `game_card.rs` 1). Verified: 0 dangling references.
- `icon_resolve_probe` evidence (same dynamic probe BEFORE/AFTER):
  - BEFORE: 11/37 bundle; 26/37 system theme. (full list saved in
    `/tmp/icon_probe_before.txt`)
  - AFTER: **37/37 bundle** (`/tmp/icon_probe_after.txt`).
- Runtime bundle synced: `diff -rq` repo == runtime.
- `icon_resolve_probe` now scans the SVGs of the bundle (dynamic list).

### BUG A regression — long description was clipped in the View modal
Commit: `c3b2592`
- Symptom: Marvel Rivals / Genshin / Fortnite clipped the text to ~2 lines,
  with no scrollbar. Diagnosis: a `GtkLabel` inside a `GtkScrolledWindow`
  **cannot scroll vertically**: its natural height is measured at its natural
  width (unwrapped line, ~2 lines), the viewport collapses to that minimum
  and the text is clipped without bars. This voided the `d2b5d50` fix.
- Fix: read-only `GtkTextView` (wrap WordChar, editable/cursor/focus off,
  class `time-label`) which does report its real wrapped height; the viewport
  measures `min(full height, max_content_height=200)` and GTK decides the bar
  from the natural height, no thresholds. T1 cascade and fallback opacity kept.

### BUG B regression — store icon looked like a padlock
Commit: `64d1dcd`
- The Stores button always asked for `system-software-install-symbolic`
  (da1d4ab). Pre-T3 the Mint theme won the lookup (modern box+arrow
  glyph); after prefixing with `corkytux-`, the bundle resolved first and
  exposed the legacy Adwaita copy, which is a **padlock** (rectangle
  `M3 8h10v7.059…` + shackle `M6.793 2.969…`). The reference was NOT
  renamed wrongly; the asset was the wrong one.
- Fix: that SVG is replaced by the modern `system-software-install` stroke
  from Mint-Breeze, normalized to `fill #2e3436` (bundle convention) and
  documented in `ATTRIBUTION.md`. Probe: `corkytux-system-software-install`
  → bundle (37/37, no resolution change).

### Modal scroll regression (24px viewport) + lost banner
Commits: `361d6c8` (issue a) and `6b6ed24` (issue b)
- Diagnosis with measurements (TEMP-LOG in `show_game_info`, post-map):
  - `scr` `measure(Vertical)` = **24px** with `min_content_height=-1` and
    `propagates_natural_height=false` → the viewport was allocated at 24px
    (text clipped to "2 lines", overlay bar invisible with no scroll).
  - `top` (cover + title) **`allocH=0`**: `git log -S` showed that
    `d2b5d50` removed `body.append(&top)` → the row was orphaned and the
    banner disappeared from the modal (the cover code was not deleted).
  - `tv` (TextView) did report its real wrapped height; the bottleneck
    was the ScrolledWindow with no minimum size and no propagation.
- Fix (a) `361d6c8`: `scr` with `min_content_height(120)`, `max(300)`,
  `propagate_natural_height(true)`, `vexpand(true)`, plus CSS `textview`/`textview > text` transparent in
  `.modal-bg`. Fix (b) `6b6ed24`: restores `body.append(&top)`.
- Measured verification (re-run with the fix): short desc → `tv` 16px,
  viewport 120; real long desc (Marvel Rivals) → `tv` natural 195px
  **propagated** by the `scr` (full text with no scroll because it fits);
  desc >300 caps at 300 with scroll; `top` `allocH=160` (banner visible);
  `body` 334/362, `content` 421.

## Multi-distro audit (6 critical; C14 already fixed)

Full detail, with `file:line` and minimal fix, in
[`docs/multidistro-audit.md`](docs/multidistro-audit.md) (42 triage
findings: 6 critical, 30 degrading, 6 cosmetic). Static audit: the app was
neither built nor run. Numbering inherited from the original triage.
**Critical: all 6 fixed.** C08 halfway (3 of 12 externalities).

### ~~C14 — `prefix_in_use` fails open without `pgrep`~~ — **FIXED**
It was the only finding with data-loss risk: without `pgrep` (NixOS,
containers) the prefix was declared **free** and the permanent import
moved it while the game was running.

- **Fix applied:** `prefix_usage() -> PrefixUsage { Busy, Free, Unknown }`
  instead of the boolean. `wineserver_pids()` reads `/proc/*/comm` and
  `wineprefix_of(pid)` reads `WINEPREFIX` from `/proc/<pid>/environ`, the exact
  attribution (wineserver never carries the prefix in its command line, so the
  substring on `pgrep -af` was not reliable). Both sides go through `norm()`.
- **Fails closed:** if there are live wineservers whose environment is
  unreadable (another user, procfs with `hidepid`), it returns `Unknown`
  and blocks with the new `Blocker::UsageUndeterminable`, instead of
  risking the data. The UI shows it unchanged: `import_manager.rs` uses
  `b.message()` generically.
- **Good side effect:** 2 of C08's 12 `pgrep` calls disappear.
- **Side effect:** a stale `wineserver.lock` no longer blocks the import
  forever; the state is decided by live processes, not by the socket.
- `wineserver_pids()`/`wineprefix_of()` stay private to the module;
  `prefix_in_use` is removed (its only two callers are in the same
  file).

### C08 — Hard dependency on external binaries — **PARTIALLY FIXED**
**Done: the 8 uses of `timeout` and the only use of `ldconfig`.** 4 sites
of `pgrep`, `pidof` and `which` remain.

- **Fix applied:** `plugin_process::output_with_timeout()` (`spawn` +
  `try_wait` in a 25 ms loop against a deadline `Instant`, `kill` + `wait`
  when it expires). Covers `plugins.rs:552,620,690,727`,
  `integration.rs:601,609,666` and `proton.rs:1314`. No external binaries.
- The streams are read in their own threads, so a plugin that fills the
  pipe buffer does not block waiting for its output, and they are joined
  **with a 2 s margin**: if the plugin left children with the pipe open, a
  direct `join` would hang the UI, which `timeout(1)` + `.output()` never covered.
- `list_emulators_in` still returns an empty list (contract of its 4
  callers; changing it would touch the UI) but now logs the reason to
  stderr as `[emu] corky-list …`, so a failure is already diagnosable.
- Side effect: `wineserver -k` in `proton.rs` stops dropping `ws.to_str()`,
  so a non-UTF8 prefix no longer degrades to an empty program.
- **Pending fix:** `pgrep` (`proton.rs:261`), `pidof` (`proton.rs:825`,
  replaceable by `/proc`) and `which` (`integration.rs:27`,
  `proton.rs:597,822`, replaceable by a `PATH` lookup).

### C01 — Hardcoded Steam paths: overlay broken on Flatpak/Snap — **FIXED**
`steam_client_path()` and both overlay resolvers only used
`~/.steam/steam`, while `find_steam_runtime` did list the four roots.
With Steam Flatpak or Snap, the overlay toggle stayed active and injected
nothing, with no warning.

- **Fix applied:** `steam_roots(home)` is the only list (the four
  variants) and it is consumed by `steam_client_path_for()`,
  `find_steam_runtime` and `steam_overlay_preload()`.
- `~/.steam/steam` goes first because it is the path that
  `STEAM_COMPAT_CLIENT_INSTALL_PATH` already used and, in a normal install, it
  is a symlink to `.local/share/Steam`: it changes nobody's behavior. If no
  root exists, the fallback is still `~/.steam/steam` (Proton uses it even
  without Steam).
- `steam_overlay_preload()` replaces the two duplicated blocks (emblem and
  no-emblem), walks all roots and preserves the leading `':'` that does not
  clobber an inherited `LD_PRELOAD`.
- **Extra hardcode that surfaced while implementing it:** `legendary_launch_cmd`
  set `STEAM_COMPAT_CLIENT_INSTALL_PATH` on its own, so fixing only
  `steam_client_path()` did not cover launching via Heroic. It now uses
  `steam_client_path_for()`, and the list lives in a single place.

### C03 — `component_status` only recognizes x86 tokens and probes two GameMode binaries — **FIXED**
Decisions I made: derive from `ARCH` + system paths, and always probe
`gamemoderun`.

- `component_bits(lib)` derives support from `std::env::consts::ARCH`. On
  aarch64 `installed32` is `false` **by construction**: there are no
  32-bit ARM distros to count, and pretending otherwise would be worse
  than not measuring it.
- `library_present(lib, bits)` searches by base-name prefix
  (`libMangoHud.so` covers `libMangoHud.so.1.2`) in multiarch, `/usr/lib`,
  `/usr/lib64`, `/usr/local/lib`, `/lib`, `lib32`/`libx32` for 32-bit and
  `/run/current-system/sw/lib` for NixOS. The multiarch triple comes from
  `gcc -print-multiarch`, with `$MULTIARCH` and `<arch>-linux-gnu` as
  fallback.
- **Fixed defect that was independent of the architecture:** `installed64`
  started out as `available`, so a `mangohud` without its 64-bit library was
  announced as installed; the flags could only be on, now they can be off.
- The `ldconfig` dependency was removed: that was its last use.
- `graphics_component_status()` now probes `gamemoderun` just like
  `component_status()`. Unified in the client because that is what CorkyTux
  injects on launch: the daemon being installed does not mean GameMode applies.
- `component_status_text()` was not touched: the contract of the UI labels
  is identical.

### C09 — Plugin registry takes the first `.tar.gz` without filtering by arch — **FIXED**
- **Real severity, verified against the API on 2026-09-25:** the five
  published releases use names without architecture
  (`heroic-store-1.0.8.tar.gz`, `minecraft-launcher-1.1.0.tar.gz`,
  `emulator-manager-1.1.0.tar.gz`, `dependency-installer-2.1.3.tar.gz`,
  `heroic-store-1.0.7.tar.gz`), so the bug is **latent**: someone has to
  publish a pair of assets per architecture. The symptom (successful
  install and `Exec format error` on every use) would be one of the most
  expensive to diagnose, which is why I fixed it anyway.
- **Fix applied:** `asset_arch_ok()` accepts assets with no architecture
  token (the current ones) and, if the token exists, requires it to map to
  `std::env::consts::ARCH` with the usual aliases: `x86_64`/`amd64`/`x64`,
  `aarch64`/`arm64`, `i386`–`i686`/`x86`, `arm`/`armv7l`/`armhf`,
  `ppc64`/`ppc64le`, `riscv64`/`s390x`. Tokens split on `-` and `.`, never
  on `_`, because `x86_64` has an underscore and `x86` would collide.
- `fetch_registry()` walks all `.tar.gz` instead of breaking at the first
  one, and **skips** a release whose assets are all another architecture
  (reason to stderr) instead of offering an impossible download.

### C10 — Lutris scan ignores `XDG_DATA_HOME` and Lutris Flatpak — **FIXED**
The artwork (450 lines earlier in the same file) did honor `XDG_DATA_HOME`
and the Flatpak path, but the game scan used a hardcoded
`~/.local/share/lutris`. Consequence: with `XDG_DATA_HOME` set the covers
showed up and 0 games were imported; with Lutris Flatpak, the same thing.

- **Fix applied:** `lutris_data_roots(home)` returns the data roots in
  preference order — `$XDG_DATA_HOME` if absolute and non-empty (`~/.local/share`
  is the spec default) and `~/.var/app/net.lutris.Lutris/data` (Flatpak sandbox
  home) — and both the scan (`pga.db`, `games/`) and the artwork
  (`coverart`/`banners` and `icons/hicolor/128x128/apps`) consume it.
- **Extra hardcode that surfaced while implementing it:**
  `lutris_yml_info()` rebuilt `~/.local/share/lutris/games` on its own, so
  fixing only `scan_lutris` was not enough. It now receives the
  `games_dir` from the caller, and the SQLite layer derives it from
  `db.parent()`.
- `scan_lutris` dedupes by slug across roots: a game does not appear twice
  during a half-done Flatpak migration.

## Separate pending item: per-store folders (Epic-Games / GOG-Games)

- `~/Games/Heroic` is Heroic's convention: don't use it as our own base.
  New installs: `~/Games/Epic-Games` and `~/Games/GOG-Games` (the launcher
  creates them if missing). Conditions: launcher separate from the plugin;
  the Remove guard still denies `~/Games/Heroic` (legacy) in addition to
  the new bases and `~/Games`; a single source for the default; Bloody
  Hell stays where it is (no reinstall).

## New findings (noted, NOT fixed)

### Stop button does not stop games launched outside the launcher
- Stop only controls child processes of the launcher. A game opened by hand
  (or by another launcher) cannot be stopped from here. Document it in the
  UI if needed; don't change behavior without a design.

### Duplicated installs.json reader in remove_modal.rs
- `remove_modal.rs` reads `plugins/heroic-store/installs.json` with its own
  reader instead of reusing the backend's one. Unify when that file is
  touched (a single read source).

### Fortnite DLCs fail in the "View" modal
- The add-ons "LEGO® Fortnite Content" (`94bc5ec13f8f438c97fdbef3e9019e27`)
  and "Fortnite Save the World Content" (`aa31f9e94e844b299ca757d1d0b97a09`)
  return the error `{"type":"error","message":"Game not in Epic library"}`
  in `game-info`. Evidence: real plugin output (see above).
- They show up with an empty description in the modal (now with the UX
  fallback). It is not in the pending list → noted, not touched.

### `legacy/system-software-install-symbolic.svg` and `legacy/web-browser-symbolic.svg` without viewBox
- With the active theme (Mint-Breeze) they are not used: both resolve from
  the system theme (`apps/symbolic/...`), not from our bundle. In the
  bundle they are byte-identical to `Adwaita/symbolic/legacy/` (same file
  Adwaita ships and renders fine). They are not touched.
- If one day they beat the active theme (e.g. another theme without those
  names) they render the same as Adwaita: acceptable.

## Awaiting visual verification (user opens the app)

1. **Final icon sweep** (Library, Minecraft, Settings, Integrations,
   sidebar): after T3 (37/37 to the bundle via probe) visually confirm that
   the stroke/shading of the 26 now-proprietary icons looks right
   (e.g. `emblem-ok` of the Minecraft check switches to the bundle copy).
2. **Description cascade (T1)**: open a stub game (Fall Guys or
   VALORANT) and check the dimmed text "Latest version: … (last update
   known to Epic: …)" is readable in light and dark. For a game WITHOUT
   Epic metadata: "No description available".
3. **View modal scroll (T2 + BUG A + 361d6c8)**: Mindcop (283 chars) with no
   scrollbar; Marvel Rivals / Genshin / Fortnite (long) with the full
   description (up to ~300px of viewport, SMOOTH scroll at the tail) and
   **cover + title visible at the top** (6b6ed24). The dialog must not
   exceed ~300px of description.
4. **Store icon (BUG B)**: in the sidebar, the button next to "Your Library"
   must go back to the modern install glyph (box+arrow), not a padlock.
4. Re-render of the icons after any future theme/icon change.
5. **Epic and GOG logout → re-login cycle**: login ended up confirmed by UI
   on both stores (`logged: epic/gog = true`, Epic token written to
   `~/.config/legendary/user.json`, GOG token in `gogdl_auth.json`), but
   **nobody tried logging out and back in**. Without checking that
   `logout` invalidates the token and that a later `Log in` re-exchanges
   without leaving leftovers from the previous attempt.
6. **GOG login end to end**: the exchange and the refresh were fixed and
   verified against an already-stored token, but the full attempt
   (navigate → capture → exchange → verify) was only confirmed after
   retrying with the previous attempt's token, not in one clean run.
7. **Final size of the action icons and the Play↔Stop cycle** in light and
   dark theme: the scaling and ink fix is applied but without visual
   verification.