# Multi-distro static audit

Date: 2026-09-25 · Audited HEAD: `ac0e0fb` (branch `main`, not pushed)

## Method and limits

Static **audit**: reading code, release scripts and documentation.
I did **not** compile, did not run the app, did not launch processes or take
screenshots (`AGENTS.md` rule). Every finding cites `file:line` so it is
verifiable without opening the app.

That is why the findings marked *critical* are failures with functional impact
verifiable by reading, not regressions observed while running. The ones that
need visual confirmation are listed separately at the end.

## Distros covered by the project

Gentoo, Debian/Ubuntu, Fedora/RHEL, Arch, openSUSE (the five from
`docs/BUILD.md`). The audit adds two axes the table does not cover:
**multi-arch** (x86_64 vs aarch64) and **environments without the system
coreutils** (NixOS, minimal containers).

## Summary

| Severity | Count |
| --- | --- |
| Critical | 6 |
| Degrading | 30 |
| Cosmetic | 6 |
| **Total** | **42** |

Counts from the original triage, kept as a record. **Current state: all 6
criticals are fixed.** C08 is fixed in 9 of its 12 sites (the 8 `timeout` ones
and the single `ldconfig` one); 3 low-impact external dependencies remain.
No degrading or cosmetic finding has been touched.

Definitions:

- **Critical**: a feature broken silently, or a user action impossible on an
  officially supported distro/architecture.
- **Degrading**: it works but with loss of function, a false warning, or
  unnecessary coupling.
- **Cosmetic**: documentation drift or inconsistencies with no functional effect.

---

## Criticals

### C01 — Hardcoded Steam paths: the overlay does not exist on Steam Flatpak/Snap

**Status: FIXED.**

- `src/backend/proton.rs:335-340` — `steam_client_path()` builds
  `~/.steam/steam` with no variants.
- `src/backend/proton.rs:940-942` and `1035-1037` — the overlay's
  `gameoverlayrenderer.so` is looked up in `~/.steam/steam/ubuntu12_32` and
  `~/.steam/steam/ubuntu12_64`.
- Contrast: `src/backend/proton.rs:480-485` (`find_steam_runtime`) **did**
  enumerate the four roots, including
  `~/.var/app/com.valvesoftware.Steam/.steam/steam`.

**Impact:** with Steam installed as Flatpak or Snap, I enable `SteamOverlay` on
the game entry, the switch stays "on" and the overlay is never injected. There
is no error and no warning: a silent loss of a feature I believe is active.

**Root cause:** the list of Steam roots was duplicated in three places and only
one of them kept it up to date.

**Fix applied:** `steam_roots(home)` is now the only list, with the four
variants (`~/.steam/steam`, `~/.local/share/Steam`,
`~/.var/app/com.valvesoftware.Steam/data/Steam` and
`~/.var/app/com.valvesoftware.Steam/.steam/steam`). Its consumers:

1. `steam_client_path_for(home)` — the first root that exists, with fallback to
   `~/.steam/steam` for when Steam is not installed (Proton uses the variable
   anyway). `~/.steam/steam` goes first because it is the path
   `STEAM_COMPAT_CLIENT_INSTALL_PATH` already used and, in a normal install, it
   is a link to `.local/share/Steam`: nobody's behavior changes.
2. `find_steam_runtime` — now iterates the resolver instead of its own list.
3. `steam_overlay_preload(home)` — replaces the two duplicated overlay blocks
   (rum and non-rum) and walks **all** the roots, not just the first. It keeps
   the leading `':'` that does not clobber an inherited `LD_PRELOAD`.

**Extra hardcode that surfaced while implementing it:** `legendary_launch_cmd`
was setting `STEAM_COMPAT_CLIENT_INSTALL_PATH` on its own (lines 185-186), so
fixing only `steam_client_path()` did not cover launching via Heroic. It now
uses `steam_client_path_for()`. That leaves the list in **one** place, which
was the real origin of the finding.

### C03 — `component_status` only recognizes x86 tokens and queries two different GameMode binaries

**Status: FIXED** (decisions I made: derive from `ARCH` + system paths, and
always probe `gamemoderun`).

- `src/backend/proton.rs:617-628` (audited version) — parsed `ldconfig -p`
  looking for `x86-64`/`lib64` for 64-bit and `i386`/`lib32` for 32-bit.
- `src/backend/proton.rs:614` — `installed64` was initialized to `available`, so
  the `ldconfig` block could only **turn on** flags, never correct them.
- `src/backend/proton.rs:605` probed `gamemoderun`; `src/backend/proton.rs:687`
  probed `gamemoded` for the same feature.

**Impact:**

1. On aarch64 `ldconfig -p` prints `aarch64`/`arm64`; neither token matches, so
   `installed32` could **never** be `true` on ARM and the UI claimed there is no
   32-bit support even with the package installed.
2. Without `ldconfig` (NixOS, containers) the whole block was skipped and
   `installed32` stayed `false` with no explanation.
3. The most serious defect was architecture-independent: since `installed64`
   was born as `available`, a `mangohud` installed **without** its 64-bit
   library announced itself as installed. The flag could never be set to
   `false`.
4. `gamemoderun` is the client and `gamemoded` the daemon. Probing the daemon
   says nothing about whether GameMode can be applied: without the active user
   socket the check fails even when the client works, and with the client
   present but the daemon missing the check passes even though nothing
   happens. The two UI cards can disagree.

**Fix applied:**

1. `component_bits(lib)` derives support from `std::env::consts::ARCH`: the
   host is 64-bit if its arch is `x86_64`/`aarch64`/`powerpc64`/`riscv64`/`s390x`,
   and can run 32-bit if it is `x86`/`x86_64`. On aarch64 `installed32` is
   `false` **by construction**, not because of a token that fails to match:
   there are no 32-bit ARM distros to count, and pretending otherwise would be
   worse than not measuring.
2. `library_present(lib, bits)` searches by base name prefix
   (`libMangoHud.so` covers `libMangoHud.so.1.2`) in the real directories: the
   multiarch dir, `/usr/lib`, `/usr/lib64`, `/usr/local/lib`, `/lib`, the
   `lib32`/`libx32` subdirectories for 32-bit, and
   `/run/current-system/sw/lib` for NixOS. The multiarch name comes from
   `gcc -print-multiarch`, with `$MULTIARCH` and `<arch>-linux-gnu` as
   fallbacks.
3. The flags can now **be negative**: they are truly resolved instead of only
   being turned on.
4. The `ldconfig` dependency is gone (that closes the last C08 site for this
   binary).
5. `graphics_component_status()` probes `gamemoderun`, same as
   `component_status()`. I unified it on the **client**: that is what CorkyTux
   injects at launch, and the daemon being installed does not mean GameMode
   gets applied.
6. `component_status_text()` **was not touched**: its contract is identical, so
   the UI labels do not change shape.

**Static verification against the real system:** on this host
(`x86_64`, without a working `gcc -print-multiarch`, without `MULTIARCH`) the
fallback yields `x86_64-linux-gnu`; `/usr/lib/libMangoHud.so`,
`/usr/lib/libgamemodeauto.so.0` (symlink) and `/usr/bin/gamemoderun` exist, so
`available` and `installed64` stay `true` as before the change, and
`installed32` is `false` because there is no multilib. No x86 regression.

### C08 — Hard dependency on external binaries (`timeout`, `ldconfig`, `pgrep`, `pidof`, `which`) in 12 sites

**Status: PARTIALLY FIXED.** The 8 uses of `timeout` and the single one of
`ldconfig` are removed (9 of 12); 3 remain: `pgrep`, `pidof` and `which`.

Distribution verified in the triage:

| Binary | Call sites |
| --- | --- |
| `timeout` | **fixed**: `plugins.rs:552`, `plugins.rs:620`, `plugins.rs:690`, `plugins.rs:727`, `integration.rs:601`, `integration.rs:609`, `integration.rs:666`, `proton.rs:1314` |
| `ldconfig` | **fixed** with C03: `proton.rs:617` no longer invokes it |
| `pgrep` | **fixed** the 2 in `import_move.rs` (C14); `proton.rs:261` remains |
| `pidof` | `proton.rs:825` |
| `which` | `integration.rs:27`, `proton.rs:597`, `proton.rs:822` |

`timeout`, `ldconfig`, `pgrep` and `pidof` come from coreutils/procps, not from
the POSIX base. On NixOS none of them is in the default `PATH`; in containers
of minimal distros one of them is usually missing.

**Impact:** it is not a uniform problem, and that is the serious part. There
are three distinct failure modes:

- **Silent failure** — `plugins.rs:727`: if `timeout` does not exist,
  `Command::new` fails, the function falls into `_ => return Vec::new()` and the
  Emulators tab comes up empty **with no error message at all**.
- **Open and dangerous failure** — a prefix in use was declared free (C14;
  already fixed, but the pattern is still latent in `proton.rs:261`).
- **Noisy failure** — `plugins.rs:552/620/690` and `integration.rs:601`: I see
  an error that does not mention the real cause.

**Fix applied (`timeout`):** `plugin_process::output_with_timeout()` replaces
`timeout(1)` for the same eight calls:

1. `spawn` + `try_wait` in a 25 ms loop against a deadline `Instant`; once
   exhausted, `kill` + `wait`. It never sleeps longer than the poll interval,
   and it does not depend on any external binary.
2. stdout and stderr are read in their own threads so a plugin that fills the
   pipe buffer does not block itself waiting for its output.
3. `join_reader()` joins those readers **with a 2 s margin**: if the plugin
   left children with the pipe open, a direct `join` would have hung the
   interface, something the previous version did not cover.
4. The timeout returns an error with the seconds limit in the message, instead
   of `timeout(1)`'s exit code 124 that the callers did not interpret.
5. `list_emulators_in` still returns an empty list —that is the contract of its
   4 callers, and changing it would touch the UI—, but now it leaves the reason
   in `stderr` (`[emu] corky-list …`) so a failure can be diagnosed.

Along the way, `wineserver -k` in `proton.rs:1317` stops dropping
`ws.to_str()`: the `PathBuf` is passed directly, so a non-UTF8 prefix no longer
degrades to an empty program.

**Pending fix:** remove `pgrep`, `pidof` and `which` from the 3 sites that
remain. `pidof` can be replaced by `/proc` (the same pattern applied in C14) and
`which` by a `$PATH` search, which is POSIX.

### C09 — The plugin registry picks the first `.tar.gz` without filtering by architecture

**Status: FIXED.**

- `src/backend/plugins.rs:347-363` (audited version) — iterates `assets` and
  `break`s on the first name ending in `.tar.gz`.
- `src/backend/plugins.rs:418` — downloads it as `{tag}.tar.gz` and extracts it
  without checking anything other than `size >= 100` (lines 448-452).

**Impact:** on a release publishing `plugin-x86_64.tar.gz` and
`plugin-aarch64.tar.gz`, an aarch64 host installs the x86_64 binary according to
API order. The plugin installs "successfully" and then fails with
`Exec format error` on every invocation, with the UI not distinguishing
"installed" from "installed and usable".

**Real severity (verified against the API on 2026-09-25):** today the five
published releases use names without architecture —`heroic-store-1.0.8.tar.gz`,
`minecraft-launcher-1.1.0.tar.gz`, `emulator-manager-1.1.0.tar.gz`,
`dependency-installer-2.1.3.tar.gz`, `heroic-store-1.0.7.tar.gz`—, so the
failure is **latent**, not active: someone has to publish a pair of
per-architecture assets first. I fixed it anyway because on the day that
happens, the symptom (correct install and failure on every use) is one of the
most expensive to diagnose.

**Fix applied:** `asset_arch_ok()` decides whether an asset serves this host, and
`fetch_registry()` walks **all** of the release's `.tar.gz` instead of breaking
at the first:

1. An asset **without an architecture token** is accepted: that is the case of
   the current releases and there is no way to tell it apart from a universal
   asset.
2. An asset **with a token** is only accepted if the token maps to
   `std::env::consts::ARCH`, covering the usual aliases: `x86_64`/`amd64`/
   `x64`, `aarch64`/`arm64`, `i386`–`i686`/`x86`, `arm`/`armv7l`/`armhf`,
   `ppc64`/`ppc64le` and `riscv64`/`s390x`.
3. Tokens are split on `-` and `.`, **never on `_`**: `x86_64` has an
   underscore, and splitting it would produce two meaningless tokens that would
   also collide with `x86`.
4. A release whose assets are all of another architecture is **omitted** from
   the registry, with the reason on stderr, instead of offering a download that
   cannot work.

**Related pending item (D05):** the GitHub API already publishes a `digest`
field (`sha256:…`) per asset. Verifying it closes the integrity finding and, in
passing, gives a trust error separate from the compatibility one.

### C10 — The Lutris scan ignores `XDG_DATA_HOME` and Lutris Flatpak, in the same file that does respect them

**Status: FIXED.**

- `src/backend/integration.rs:852-857` — `pga.db` hardcoded to
  `~/.local/share/lutris/pga.db`.
- `src/backend/integration.rs:871-876` — `games/` hardcoded to
  `~/.local/share/lutris/games`.
- Contrast, **450 lines earlier in the same file**:
  `src/backend/integration.rs:395-397` did resolve `XDG_DATA_HOME` with fallback
  to `~/.local/share`, and `403-407` did add Lutris's Flatpak path.

**Impact:**

1. With `XDG_DATA_HOME` configured (common in tiling setups) Lutris installs
   into `~/data/lutris`: the UI resolved the covers correctly (line 399) but the
   game scan looked at `~/.local/share/lutris` and found nothing. The result is
   "covers yes, games no", which looks like a Lutris bug and not a CorkyTux one.
2. With Lutris as Flatpak, `~/.var/app/net.lutris.Lutris/data/lutris` was used
   for artwork but never for `pga.db` nor for `games/`: 0 games imported.

**Fix applied:** a single resolver `lutris_data_roots(home)` that returns
Lutris's data roots in preference order, consumed by both the scan and the
artwork:

1. `$XDG_DATA_HOME` if it is absolute and non-empty (an empty or relative value
   is ignored instead of building paths under the CWD), with the spec default
   `~/.local/share` as fallback.
2. `~/.var/app/net.lutris.Lutris/data`, which is where the Flatpak sandbox
   rewrites the process home.

From each root hang `lutris/` (database, games, covers) and
`icons/hicolor/...` (app icons), so both consumers derive their paths from the
same place.

I also fixed a third hardcode of the same kind that the triage had not listed:
`lutris_yml_info()` was rebuilding `~/.local/share/lutris/games` on its own, so
fixing only `scan_lutris` was not enough. It now receives the `games_dir` from
the caller, and the SQLite layer derives it from `db.parent()`, which by
construction belongs to the same root as the database.

Since several roots are now walked, `scan_lutris` dedupes by `slug` (first root
wins) so a game does not show up twice during a half-done Flatpak migration.

### C14 — `prefix_in_use` fails open when `pgrep` does not exist

**Status: FIXED** (see "Fix applied" at the end of the section; the rest of the
criticals stay open).

- `src/backend/import_move.rs:351-374` (audited version).
- Lines 353-356: if `Command::new("pgrep")` fails, `running = false` and the
  function returns `false` (prefix free).
- Lines 360-361: `lock.exists()` — `wineserver.lock` is a unix socket; a socket
  left over from a dead session still exists.
- Lines 365-370: the fallback compares `prefix.display()` as a raw substring
  against the `pgrep -af` line, without going through the `norm()` that shared
  prefix grouping does use (line 410).

**Impact:** two opposite failures, both over game data.

- Without `pgrep` (NixOS, container): the function claims a prefix **in use is
  free** and the permanent import moves it while the game is running. The copy
  is the one that survives, but the original disappears and Heroic/Lutris
  break. It is the only finding in the audit with data-loss potential.
- With a stale `wineserver.lock`: the import stays **permanently blocked**
  until I delete the socket by hand, with no explanation in the UI.
- With the prefix path written differently in the config (for example `..` or a
  symlink), the substring does not match and the first case returns.

**Fix applied:** the boolean was replaced with a tri-state
`PrefixUsage { Busy, Free, Unknown }` and the `pgrep` dependency removed:

1. `wineserver_pids()` enumerates `/proc/*/comm` directly. Without `pgrep`
   there is no false "free", and one of the 12 C08 calls disappears as a bonus.
   `comm` is compared by substring, not equality, so variants like
   `wineserver-preloader` are not lost; the CorkyTux name does not contain
   `wineserver`, so there is no self-matching.
2. `wineprefix_of(pid)` reads `WINEPREFIX` from `/proc/<pid>/environ`, which is
   **exact** attribution: wineserver receives the prefix through the
   environment, never through the command line, so the substring over
   `pgrep -af` was not reliable.
3. Both sides go through `norm()`, which canonicalizes, so the comparison no
   longer depends on how the path is written in the configuration.
4. A stale `wineserver.lock` no longer blocks anything: the state is decided by
   live processes, not by the presence of the socket.
5. If there are live wineservers but their environment is unreadable (another
   user, procfs with `hidepid`), the result is `Unknown` and it **blocks** with
   the new `Blocker::UsageUndeterminable` instead of risking the data. The UI
   renders it unchanged: `import_manager.rs` uses `b.message()` generically.

The original triage `Fix:` ("invert the default and normalize the path") is
fulfilled, but on a better basis: `/proc` attribution replaces the heuristic
instead of merely patching it.

---

## Degrading

| ID | Location | Finding |
| --- | --- | --- |
| D01 | `src/backend/config.rs:463-483` | `all_proton_paths()` only scans user paths; never Steam's `compatibilitytools.d` nor the package manager's Proton. On SteamOS/Bazzite/Nobara the UI says "No Proton builds installed" with Proton working. Mitigated by CorkyTux's auto-downloader, but not for anyone who already has Proton. |
| D02 | `src/backend/proton.rs:821-838` | Steam auto-start for the overlay depends on `which steam` + `pidof steam`; both fail with Steam Flatpak/Snap. |
| D03 | `src/backend/proton.rs:645-663` | `detect_game_arch_simple` only recognizes `0x10b`/`0x20b`; there is no ARM64EC (`0xa641`) nor aarch64 (`0xaa64`), so the label falls back to "all". |
| D04 | `src/backend/proton.rs:413-421` | `is_foreign_arch` only contrasts aarch64/arm64 against x86_64; it does not cover i686 naming nor armv7. |
| D05 | `src/backend/plugins.rs:418-461` | Downloads and extracts a remote tarball with no checksum and no signature; the only validation is `size >= 100`. |
| D06 | `src/backend/plugins.rs:327-345` | Unauthenticated GitHub API: 60 req/h per IP. Fails on NAT/CGNAT networks (university, office, mobile) with 403. |
| D07 | `src/backend/plugins.rs:371` | `body.chars().take(200)` cuts the description mid-sentence and shows raw markdown. |
| D08 | `src/backend/plugins.rs:552,620,690` | Dependency and DLL scanning/installation depends on `timeout`; without it the error does not mention the cause. |
| D09 | `src/backend/integration.rs:589-593,601,609,666` | Icon extraction requires `timeout` + `icoextract`/`ffmpeg`; it degrades to a generic icon with no warning. |
| D10 | `src/backend/integration.rs:394-407` vs `850-876` | The artwork path and the games path use different resolvers (see C10). |
| D11 | `src/backend/integration.rs:1261-1281` | `libraryfolders.vdf` discovery does not cover Steam Flatpak. |
| D12 | `src/backend/shortcuts.rs:138` | `Exec={exe}` without quotes: an install path with spaces breaks the `.desktop`. |
| D13 | `src/backend/shortcuts.rs:135-143` | `Name`, `Comment` and `X-CorkyTux-Game` unescaped: a game name with a newline injects keys into the `.desktop`. |
| D14 | `src/backend/shortcuts.rs:33-54` | The app menu hardcodes `~/.local/share/applications`; the desktop directory depends on `xdg-user-dir`, and if that is missing a `~/Desktop` I do not use gets **created**. |
| D15 | `src/backend/shortcuts.rs:157,180` | `.desktop` with mode 0755; the specification suggests 0644. |
| D16 | `src/backend/plugin_process.rs:88-95` | Hardcoded plugin directory, without `XDG_DATA_HOME`. Consistent with the rest of the project, but not with the standard. |
| D17 | `src/backend/plugin_process.rs:97-100` | `plugin_available` checks existence, not the executable bit: a plugin without permission appears installed and fails when invoked. |
| D18 | `src/backend/config.rs:583-590` | `dirs::home_dir()` is only `$HOME`, duplicated in `proton.rs:9`, `plugins.rs:8`, `integration.rs:8`. With `HOME` empty, `plugin_process` falls to `"."` (CWD). |
| D19 | `src/ui/import_manager.rs:357` | `PathBuf::from(HOME.unwrap_or_default()).join("Games")` with `HOME` empty produces the **relative** path `Games`: the import would write into the CWD. |
| D20 | `src/main.rs:220-221` | The `IconTheme` only adds `~/.local/share/corkytux/assets/icons`; run from the source tree it finds no packaged asset. |
| D21 | `src/ui/details_panel.rs:557-563` | The default prefix that is **displayed** is hardcoded to `~/.local/share/Steam/...`; it can diverge from the path used at launch. |
| D22 | `release/install.sh:86` | The libs fallback tries `/usr/lib/x86_64-linux-gnu`; on aarch64 the multiarch is `aarch64-linux-gnu` → false "missing libs" warning and a continue prompt. |
| D23 | `release/install.sh:84` | Without `ldconfig` the check falls to the path sweep, which does not include `/run/current-system/sw/lib` (NixOS). |
| D24 | `release/install.sh:56-61` | Depends on the `file` binary; without it the installer aborts with "corkytux ELF binary not found" even though the binary exists. |
| D25 | `release/install.sh:201-212` | `Exec=` and `Icon=` without quotes in the `.desktop` the installer generates. |
| D26 | `release/install.sh:107-111` | Steam detection via `command -v steam` or `~/.steam`; it does not see Flatpak nor Snap. |
| D27 | `release/install.sh:119-122` | Recommends `pip install --user`, which does not work on Gentoo and is blocked by PEP 668 on Debian 12+ and Fedora. |
| D28 | `release/build-release.sh:8-9` | The tarball name uses raw `uname -m` (`armv7l`, `i686`) without normalizing or checking that the binary builds for that architecture. |
| D29 | `release/install.sh:47-52` | Creates `~/Games` unconditionally on every install, even though the Import Manager may never be used. |
| D30 | `release/install.sh:222-226` | The `PATH` warning only mentions bash and zsh; in fish, nushell or csh the `~/.local/bin` symlink stays unused. |

## Cosmetic

| ID | Location | Finding |
| --- | --- | --- |
| K01 | `Cargo.toml:3`, `release/uninstall.sh:12`, `release/build-release.sh:3`, `release/install.sh:4`, `docs/BUILD.md:1,37,45`, `README.md:101-102` | Version drift: the code is at `3.0.19`; the docs say `3.0.11`, the uninstaller `3.0.11`, the builder `3.0.13` and the installer header `3.0.16`. |
| K02 | `README.md:148-161` | The architecture map omits `plugins.rs`, `integration.rs`, `import_move.rs`, `theme.rs`, `shortcuts.rs` and `external.rs`. |
| K03 | `README.md:101`, `docs/BUILD.md:37` | Only the `x86_64` tarball is documented even though `build-release.sh` uses `uname -m`. |
| K04 | `src/ui/import_manager.rs:19` | The user-visible text says "a Games folder in your /home"; it is wrong on `/var/home` (Fedora Silverblue). |
| K05 | `docs/DEPENDENCY_INSTALLER_HOTFIX.md:43-44` | The author's absolute home paths in instructions meant to be copy-pasted (fixed: `$HOME`). |
| K06 | `src/Cargo.toml` | Legacy manifest (package `demo`, `cdylib`) coexisting with the real `Cargo.toml`; it confuses any dependency reading. |

---

## Pending visual confirmation

It requires me to open the app; the agent does not run it.

1. Transient-state matrix of the emulator row: `Installing…`,
   `.add-btn:disabled` and the 96 px width (already recorded in `DESIGN.md`).
2. Centering of the Papirus icons and of the monogram in the 48 px slot.
   **`Align::Center` is already in HEAD since `ac0e0fb`**; if the icons look
   uncentered, it is almost always because a binary older than the fix is
   running (see "Binary in use" below), not because the CSS fails.
3. Static `Found in system` and clickable `Set as linked` on the same row.
4. `*** BUG *** In pixman_region32_init_rect: Invalid rectangle passed` warning.
   Refined on 2026-09-25 with three runs: **7 warnings**, then **5**, then
   **1** — same binary, same assets, and the last one already with the binary
   reinstalled over the launcher's.

   ### What I did rule out by reading

   - **Texture loading with dimension 0**: all of the paths in
     `helpers.rs` are guarded —`nw/nh … .max(1)` in `load_texture`
     (`helpers.rs:162-163`), `if w <= 0 || h <= 0 { return None }` in
     `load_card_banner` (`helpers.rs:178`), and `Pixbuf::new(...)?` which fails
     instead of creating an empty surface.
   - **`background-image` on an empty widget**: the 6 CSS rules that mention it
     (`helpers.rs:570,571,597,650,652,653`) are all
     `background-image: none`, that is they **remove** background. None adds
     it.
   - **The session adoption path**: `adopt_session()`
     (`proton.rs:1516-1532`) only writes state and does an `eprintln`; it does
     not create widgets. The log shows up before the warnings because of
     startup order, not causality.

   ### What the variable count implies

   The number of warnings **changes between runs**: 7, 5, 1 and 8, across four
   startups. There is no trend nor correlation with usage: it is noise. A
   deterministic defect —a 0-size asset, a fixed code path— would give a
   constant count, so the cause is a **timing or first-frame condition**, not a
   deterministic drawing bug.

   **Correction of a previous inference:** with only the first two samples
   (7 and 5) I tended to conclude that the count was decreasing and that the
   fourth run, with 1 warning, confirmed it. The fifth sample (8) refutes it.
   There is no decrease; the variance is simply high and must not be read as a
   trend.

   There remain 46 `border-radius` rules in the CSS, which is the known
   remaining trigger when a rounded rectangle is computed over a width or
   height allocation of 0.

   **Open hypothesis, unconfirmed:** the jump from 1 to 8 warnings happened on
   the run immediately after replacing `GtkImage` with `GtkPicture` in the
   emulator rows. If the number of listed emulators were 8, it would fit one
   warning per row —a `GtkPicture` with `can_shrink` receiving a 0 allocation
   on an intermediate frame. It is unconfirmed: the earlier counts (7, 5, 1)
   already varied without `GtkPicture`, so correlation with a single sample
   proves nothing. I would have to count the rows and compare.

   **To actually close it** I need to isolate it at runtime (GTK Inspector on
   the first frame, or a bisect of the widget construction order), which
   exceeds the current "compile and open" authorization. Until then it stays
   cosmetic: the app works and the warning prevents nothing.

   **Update 2026-09-27 — closed as a known cosmetic, no fix.**
   Audit by reading (no debugger): the main suspect became every library tile
   —`.game-card { border-radius: 22px }` and
   `.accent-strip { border-radius: 0 0 20px 20px }` (`helpers.rs`) over
   `GtkPicture(can_shrink, Cover)` whose paintable arrives async
   (`game_card.rs`, `load_cover_async`). It is the only hypothesis that explains
   variable counts with the same binary and the same assets: how many pictures
   remain at size 0 depends on the timing of each startup. The emulator-row one
   is downgraded to secondary (8 rows vs dozens of tiles). Medium-high
   confidence in the family, low in naming the exact widget. Decision: no
   experiment and no fix (they would require a debugger or re-downloading
   ~576 MB of covers). The app works and the warning prevents nothing.

## Icon centering measurement (2026-09-25)

Measured with `rsvg-convert` + alpha channel analysis, without running the app.
The 12 SVGs in `assets/icons/emulators/` declare `width="48" height="48"` (exact
1:1 aspect, none with a `viewBox`), so there is no scaling distortion.

| Group | bbox (viewBox units) | center | offset |
| --- | --- | --- | --- |
| The 11 remaining Papirus icons | — | — | ≤ 0.4 px at 40 px |
| `mupen64plus-qt.svg`, global bbox | 41.0 × 43.0 · x[6.0; 47.0] y[4.0; 47.0] | (26.5; 25.5) | **(+2.51; +1.50)** |
| `mupen64plus-qt.svg`, **logo only** (M + background) | 36.0 × 41.1 · x[6.0; 42.0] y[4.0; 45.1] | **(24.0; 24.5)** | **(+0.00; +0.54)** |
| `mupen64plus-qt.svg`, green badge | 22.0 × 23.0 · x[25.0; 47.0] y[24.0; 47.0] | (36.0; 35.5) | (+12.00; +11.51) |

**The global offset is NOT a defect: it is the green badge.** It is a status
circle in the bottom-right corner (`<circle>`/`<rect>` elements of the asset,
`cx=36 cy=35 r=11`), deliberately placed there by Papirus. The main logo —the
"M" and its rounded background— is **horizontally centered with numeric
exactness** (+0.00) and +0.54 units vertically, which is 0.45 px at a 40 px
render size: imperceptible.

### Why I do NOT crop it

I simulated the literal requested adjustment (adding `viewBox="2.51 1.50 48 48"`,
which centers the global bbox) on a temporary copy. Result:

| Measurement | Before | After the `viewBox` |
| --- | --- | --- |
| Main logo | (+0.00; +0.54) | **(−2.52; −0.96)** |
| Green badge | (+12.00; +11.51) | (+9.49; +10.01) |
| Global bbox | (+2.51; +1.50) | (−0.01; +0.00) |

The crop centered the global bbox and **moved the logo 2.1 px to the left and
0.8 px up**, which is exactly what it was meant to remove. It trades a
0.45 px, imperceptible badge asymmetry for a visible off-center logo. **The
asset is left untouched** and the claim in `assets/icons/ATTRIBUTION.md` that
they are "unmodified copies" remains true.

## Emulator link verification (2026-09-25)

Static review of the full chain, without running the plugin. **The linking
works correctly.** The chain is:

1. `emulator-manager corky-list` → `get_source()` / `get_path()` /
   `get_launch_args()` and `settings` from the `EMULATORS` catalog.
2. `list_emulators_in()` (`plugins.rs:828-847`) reads `path` unconditionally
   and `source` from the plugin's explicit field, with compatibility inference
   for older backends. `native = (source == "linked")`.
3. `Set as linked` → `link_emulator_in(dir, emu.name, emu.path)`
   (`settings.rs:2645`). `emu.name` is the catalog key and `emu.path` the
   plugin's `shutil.which` result, so the two match.
4. `link_emulator()` (plugin, line 341) validates `os.path.isfile` **and**
   `os.access(X_OK)`, and fills in `launch_args`, `description` and
   `extensions` from `EMULATORS` when they are not passed explicitly. That is
   why relinking melonDS or Dolphin recovers `-e {rom}` and its extensions:
   the information is not lost.
5. `get_source()` reads `linked.json` again and returns `"linked"`; the row
   ends up with the `Linked` badge.

Also, `link_emulator` is **self-healing**: if the linked path stops existing or
stops being executable, `get_source()` does not return `"linked"` but falls to
`"system"` or `"none"`, and the row becomes actionable again.

### Observation: the link is a one-way door

`show_btn` (`settings.rs:2572`) only includes `system | appimage | none`. A
`linked` row **has no button**, and `corky-unlink` / `unlink_emulator()` exist
in the plugin but **are not invoked from anywhere in the Rust code** (`grep` for
`unlink` in `src/`: zero results). It is consistent with the design decision to
compact the `linked` row, but it means the only way to unlink from the launcher
is to break the link by moving or deleting the binary, or by editing
`linked.json` by hand.

**Not verified by running:** the check is a code check, not functional. A real
test would require invoking `emulator-manager corky-link`, which is a script
and falls outside the current authorization.

## Binary in use (2026-09-25)

`~/.local/share/applications/corkytux.desktop` launches
`~/.local/share/corkytux/corkytux`, whose binary is **older than `ac0e0fb`**
(compiled 02:58:41; the fix is from 05:18:26). I confirmed it with strings:
that binary **does not contain** `emu-system-notice` nor `Found in system`,
while `target/debug/corkytux` (05:55) does. Before the fix, `icon_slot` used
`Align::Start` and the image had no `set_size_request`, that is the image
stayed anchored to the top-left corner of the slot: that is the "uncentered
icons" symptom. For the menu launcher to see the fix I have to reinstall the
binary and the assets.

## Fix order

1. ~~**C14**~~ — **done**: tri-state `PrefixUsage` + attribution via
   `/proc/<pid>/environ` instead of `pgrep`.
2. ~~**C08**~~ — **done in its large part**: `output_with_timeout()` covers the
   8 `timeout` sites (including the Emulators tab that came up empty with no
   error) and C03 removed the single `ldconfig`. 3 low-impact external
   dependencies remain: `pgrep`, `pidof` and `which`.
3. ~~**C10**~~ — **done**: `lutris_data_roots()` shared by scan and artwork.
4. ~~**C09**~~ — **done**: `asset_arch_ok()` filters by `std::env::consts::ARCH`.
5. ~~**C01**~~ — **done**: `steam_roots()` shared by the four consumers.
6. ~~**C03**~~ — **done**: 32/64 support derived from `ARCH` and system paths,
   and `gamemoderun` as the only GameMode criterion.
7. The rest of the degrading ones in batches. The next with the best
   effort/impact ratio is **D05**: the API already publishes
   `digest: sha256:…`, so verifying the download is directly implementable.
   Then D12, D13, D19 and D22.
