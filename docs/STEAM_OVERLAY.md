# Steam Overlay and Startup

Status: analysis of the `main` code; neither CorkyTux nor Steam was run.
Code findings are kept separate from the hypotheses that need a visual test.
File and line references point at the base commit of this branch.

## What the option does

`Steam overlay` appears in the per-game settings next to `Steam runtime` and
`umu-launcher`. The UI reads and saves `SteamOverlay` through `save_switch`;
the `Games.ini` parser normalizes keys to lowercase and the launcher reads
`steamoverlay`. In practice the persisted key is `steamoverlay=true` or
`steamoverlay=false`; when it is missing, the default is off.

- UI and saving: [src/ui/game_settings.rs:377](../src/ui/game_settings.rs#L377), [src/ui/game_settings.rs:401](../src/ui/game_settings.rs#L401), [src/ui/game_settings.rs:173](../src/ui/game_settings.rs#L173).
- Key the launcher consumes and default value: [src/backend/proton.rs:1315](../src/backend/proton.rs#L1315).
- `Games.ini` access: [src/backend/config.rs:464](../src/backend/config.rs#L464), [src/backend/config.rs:468](../src/backend/config.rs#L468).

When it is enabled, the launcher looks for `gameoverlayrenderer.so` under
these four roots, in `ubuntu12_32` and `ubuntu12_64`:
`~/.steam/steam`, `~/.local/share/Steam`,
`~/.var/app/com.valvesoftware.Steam/data/Steam` and
`~/.var/app/com.valvesoftware.Steam/.steam/steam`. If it finds libraries, it
concatenates both paths into one `LD_PRELOAD` string with colons and a
leading `:`. If it finds none, it adds no overlay variables.

- Roots: [src/backend/proton.rs:690](../src/backend/proton.rs#L690).
- 32-bit and 64-bit library resolution: [src/backend/proton.rs:706](../src/backend/proton.rs#L706).
- The leading `:` and the `LD_PRELOAD` assignments: [src/backend/proton.rs:704](../src/backend/proton.rs#L704), [src/backend/proton.rs:1537](../src/backend/proton.rs#L1537), [src/backend/proton.rs:1626](../src/backend/proton.rs#L1626).
- Vulkan layer: `ENABLE_VK_LAYER_VALVE_steam_overlay_1=1`; overlay id: `SteamOverlayGameId`, default `480`: [src/backend/proton.rs:1316](../src/backend/proton.rs#L1316), [src/backend/proton.rs:1538](../src/backend/proton.rs#L1538), [src/backend/proton.rs:1539](../src/backend/proton.rs#L1539), [src/backend/proton.rs:1627](../src/backend/proton.rs#L1627), [src/backend/proton.rs:1628](../src/backend/proton.rs#L1628).

`SteamGameId` is not set in this flow. Direct Proton sets `SteamAppId` to the
game's Steam ID or to `0`, and sets `STEAM_COMPAT_CLIENT_INSTALL_PATH` to the
first existing Steam root (or `~/.steam/steam` when there is none).
`steam_client_path_for` uses the same root list, which includes Flatpak but
not Snap.

- Client path: [src/backend/proton.rs:740](../src/backend/proton.rs#L740).
- Direct Proton: [src/backend/proton.rs:1558](../src/backend/proton.rs#L1558), [src/backend/proton.rs:1574](../src/backend/proton.rs#L1574).
- Legendary: [src/backend/proton.rs:527](../src/backend/proton.rs#L527), [src/backend/proton.rs:533](../src/backend/proton.rs#L533).

The paths are not the same on every runner:

- Direct Proton injects the preload and both overlay variables.
- UMU also injects those three variables; it uses `GAMEID` (Steam ID or `0`)
  and `STORE=steam`. The UMU block does not set `SteamAppId` or
  `STEAM_COMPAT_CLIENT_INSTALL_PATH`.
- Heroic/Legendary has an early return before reading `steamoverlay`: it sets
  `STEAM_COMPAT_CLIENT_INSTALL_PATH` and `SteamAppId`, but it does not apply
  the overlay preload or its `SteamOverlayGameId`.
- `VK_LAYER_PATH` is not set explicitly.

Evidence: UMU [src/backend/proton.rs:1522](../src/backend/proton.rs#L1522), [src/backend/proton.rs:1531](../src/backend/proton.rs#L1531); Legendary early return [src/backend/proton.rs:1424](../src/backend/proton.rs#L1424), client path [src/backend/proton.rs:527](../src/backend/proton.rs#L527).

MangoHud is enabled through `MANGOHUD=1`. GameMode wraps the command
afterwards; it copies the inner command's arguments and variables. The order
and the effective loader compatibility also depend on which wrapper each game
uses; this repository has no overlay test with MangoHud/GameMode/gamescope.

- MangoHud: [src/backend/proton.rs:1610](../src/backend/proton.rs#L1610).
- GameMode wrapper: [src/backend/proton.rs:1667](../src/backend/proton.rs#L1667).

## Who starts Steam

There is no global plugin or setting called "start Steam first". There are
two automatic starts in `ProtonManager::run_game`:

1. If the game has a valid Steam ID, the launcher checks `which steam`,
   queries `pidof steam` and, if it sees no process, starts `steam -silent`.
   Immediately afterwards it asks `xdg-open` to open
   `steam://rungameid/<id>`.
2. For the Proton/UMU path, if `steamoverlay` is on or the game has a
   `steamid`, it runs the same detection and, if Steam looks closed, starts
   `steam -silent`.

Evidence: `steam://` path [src/backend/proton.rs:1233](../src/backend/proton.rs#L1233), [src/backend/proton.rs:1242](../src/backend/proton.rs#L1242), [src/backend/proton.rs:1252](../src/backend/proton.rs#L1252), [src/backend/proton.rs:1265](../src/backend/proton.rs#L1265); runner auto-start [src/backend/proton.rs:1353](../src/backend/proton.rs#L1353), [src/backend/proton.rs:1356](../src/backend/proton.rs#L1356), [src/backend/proton.rs:1365](../src/backend/proton.rs#L1365), [src/backend/proton.rs:1375](../src/backend/proton.rs#L1375).

`which` only finds the executable available in `PATH`; `pidof` only checks
that a process with that name exists. It does not read
`~/.steam/registry.vdf`, `ActiveUser`, a Steam pipe/socket or the session
state. The process is started with `spawn()` and the code does not wait for
the session to start, finish an update or accept `steam://`. The `steam://`
path also `spawn()`s `xdg-open` without waiting for its result. The "Steam
starting / game launched" race is therefore visible in the flow; its duration
and whether it explains a specific failure have to be measured on a machine
with Steam.

The `which` and `pidof` calls are already recorded in C08 and D02/D26 of
`PENDIENTES.md`: [PENDIENTES.md:18](../PENDIENTES.md#L18), [PENDIENTES.md:20](../PENDIENTES.md#L20).

## Hypotheses about failures

| Hypothesis | Available evidence | Status |
|---|---|---|
| Start race | `steam -silent` is asynchronous and there is no readiness wait before opening the protocol or starting the runner. | Confirmed in the code order; latency and visual effect not measured. |
| Steam with no session, updating or with a pending login | The code only checks whether a PID exists; it does not read `ActiveUser`, the pipe/socket or an update indicator. | The code does not distinguish those states; which one occurs in a specific case is not verified. |
| Flatpak | There are Flatpak roots to find the preload, but auto-start looks for `steam` through `PATH`; it does not run `flatpak run`. | Partial incompatibility confirmed in discovery/start. Effective injection into a Flatpak game not tested. |
| Snap | There are no Snap roots in `steam_roots`, and start does not invoke `snap run` either. | Not explicitly supported by these paths; a Snap install is still untested. |
| bwrap isolation | The sandbox hides the real HOME with tmpfs and creates a new `/tmp`. It exposes the private HOME and `XDG_RUNTIME_DIR`; it does not mount the host Steam HOME. | Risk confirmed by the configuration. The `iso_ro` path tries to derive the directory from the `LD_PRELOAD` string including the leading `:`, which can produce an invalid path and skip the `.so` bind. Not tested visually. |
| MangoHud/GameMode/gamescope order | MangoHud uses `MANGOHUD`; GameMode wraps the command and copies the environment. There is no explicit gamescope configuration in this flow. | The effect on a real overlay was not measured. |
| 32-bit games | The resolver includes `ubuntu12_32` and `ubuntu12_64` when both files exist. | File presence is checked; correct loading in a 32-bit game not tested. |
| Wayland/X11 | `nativewayland` sets `PROTON_ENABLE_WAYLAND` only in direct Proton. | The setting is independent of the overlay; there is no visual comparison on both compositors. |
| UMU vs Proton | Both have preload code. UMU sets `GAMEID`/`STORE`; direct Proton also sets `SteamAppId` and `STEAM_COMPAT_CLIENT_INSTALL_PATH`. Legendary skips the overlay block. | Differences confirmed; runtime behavior not tested. |
| Non-Steam game without SteamAppId | Proton sets `SteamAppId=0`; the overlay uses `SteamOverlayGameId`, which defaults to `480`. UMU uses `GAMEID=0` when there is no Steam ID. | Values confirmed; a non-Steam title may also need Steam to register it as an external game. Not tested here. |

The comment in `steam_overlay_preload` says the leading `:` preserves an
inherited `LD_PRELOAD`. However, `Command::env("LD_PRELOAD", ...)` replaces
the inherited value, and the code does not concatenate the previous value.
That is another discrepancy visible in the code, not a conclusion about the
overlay on a specific machine.

## Instrumentation and decision

The usual log titled "CorkyTux debug log" still does not include the overlay
environment. This branch adds separate instrumentation, off by default: set
`CORKYTUX_DEBUG_STEAM=1` before opening CorkyTux to write `[steam-debug]`
lines with a Unix timestamp in milliseconds. It records detection of the
command/process, the request and result of `steam -silent`, a readiness
snapshot before and after `steam://`, and the runner/environment just before
launch. The values listed include `LD_PRELOAD` (library names only),
`ENABLE_VK_LAYER_VALVE_steam_overlay_1`, `SteamOverlayGameId`, `SteamAppId`,
`SteamGameId`, `GAMEID`, `STORE` and `STEAM_COMPAT_CLIENT_INSTALL_PATH` (HOME
is redacted). It does not write PIDs, ActiveUser, account IDs or other
environment variables.

The detector uses `/proc/<pid>/comm` to locate `steam` and queries the
native, Flatpak and Snap roots for a `registry.vdf` with a non-zero
`ActiveUser`. It returns `NotRunning`, `NoActiveUser` or `Ready`; it is not
used yet to block or modify startup. Temporary fixtures cover the three
locations without starting processes.

Snapshot of this machine: `/proc` showed no `comm` named exactly `steam`;
among the paths queried there was a readable `registry.vdf`, but it did not
contain a recognizable `ActiveUser` key. The time to `Ready` was not measured:
that would require observing a real start, and Steam was not started.

The code shows a race and a possible path discrepancy in bwrap, but isolating
which one explains the observed failure is still pending. Before changing the
default start I propose an A/B test with the following steps. If the race is
confirmed, the wait should be limited to the explicit overlay toggle, use
readiness and a configurable timeout, and show a warning when it expires; the
new detector must not introduce `which`, `pgrep` or `pidof`.

## Manual test

1. Pick a non-critical Windows game that already starts with Proton. In the
   game settings enable `Steam overlay`; note the current values for
   isolation, UMU, GameMode and MangoHud.
2. With Steam fully closed, start the game and measure from the click until
   the window appears. Check whether Steam opens, whether it finishes signing
   in before the game, and whether `Shift+Tab` shows the overlay.
3. Close the game. Open Steam manually, wait until the library is usable and
   start the same game with the same settings. Check `Shift+Tab` and measure
   the same interval.
4. Repeat with isolation off only if the first case had it on; do not change
   several settings in the same comparison.
5. If it fails, share the `[steam-debug]` lines from the terminal from
   detection to `game-spawn`, the game debug log, the runner (direct Proton or
   UMU), whether Steam is native/Flatpak/Snap, Wayland/X11 and the three
   measurements. The instrumentation redacts HOME and does not print PIDs or
   account; still review the text before sharing it.

To enable the trace in a manual test, start CorkyTux from a terminal with
`CORKYTUX_DEBUG_STEAM=1` in the process environment. No need to edit
`Games.ini`, change settings or restart Steam to enable logging.

I cannot verify without real Steam: the login/`ActiveUser` state, the real
time to a ready session, the `Shift+Tab` display, the Wayland/X11 effect,
Flatpak/Snap, 32-bit games or the combination with MangoHud/GameMode. The app
and a game were not run.
