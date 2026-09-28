# Wine/Proton isolation (Bottles design → CorkyTux)

How Bottles does it (sources: `bottles/backend/managers/sandbox.py`,
`bottles/backend/wine/winecommand.py`, GPL-3.0, © Bottles developers):

- One bottle per game: `WINEPREFIX` + bottle dir hold prefix, shaders,
  configs. Runners live outside, shared read-only.
- `SandboxManager` builds a `bwrap` (or `flatpak-spawn --sandbox` under
  Flatpak) command around every launch: `--clearenv` + explicit `--setenv`
  allowlist, `--ro-bind / /` + `--tmpfs /tmp`, `--bind/--ro-bind` for the
  bottle (rw), runners/temp (ro), chdir bind, Pulse socket when sound is
  shared, tmpfs over `/dev/input` and `/dev/bus/usb` unless shared,
  `--share-net`/`--unshare-net`, `--unshare-user`, per-launch override.
- `WineCommand` keeps a curated environment (`WineEnv` allowlist via
  `Limit_System_Environment`), redirects shader caches into the bottle,
  and can disable the dedicated sandbox per launch (`sandbox_override`).
- Trust model: games are untrusted; the sandbox is mandatory per bottle
  setting, never silently skipped (override is explicit and logged).

Translation to CorkyTux (reimplemented, no literal copy; AGPL-3.0):

- No bottle abstraction exists: the unit is the existing prefix
  (individual `.../pfx` or shared) + the game folder. Isolation is a
  launch-time bwrap envelope over the final `Command` in
  `ProtonManager::run_game` (all paths: umu, plain Proton, Steam
  runtime, legendary, gamemoderun) and `run_wine_tool`. Prefixes are
  never moved or modified; existing prefixes keep working untouched.
- Selective binds instead of `--ro-bind / /`: the game sees a private
  `$HOME` (`~/.local/share/CorkyTux/sandbox-home/<game>`), the prefix
  (rw), the game folder (rw), Proton/umu/Steam-runtime/legendary and
  anti-cheat runtimes (ro), `/usr` + `/etc` (ro), `/proc`, `/dev`
  (with `/dev/dri` bound rw for the GPU), a fresh `/tmp`, and the
  host `XDG_RUNTIME_DIR` (audio/display sockets). The real home is
  hidden; user `IsolatePaths` add explicit rw grants.
- Environment is inherited (Proton/umu/pressure-vessel need a full
  host env; an allowlist would be brittle) except `HOME`,
  `XDG_CACHE_HOME`, `XDG_CONFIG_HOME`, which point into the sandbox
  home, plus every variable the launcher already set on the command
  (re-applied as `--setenv`, so `WINEPREFIX`, `PROTON_*`,
  `STEAM_COMPAT_*` survive). Network stays shared in v1 (online
  games); a no-network option is future work, documented not silent.
- Control: global `IsolateNewPrefixes` (User Settings, default off;
  applies to games without an explicit choice) + per-game `Isolated`
  (`true`/`false` in Games.ini; an explicit value always wins) +
  per-game `IsolatePaths` (`;`-separated extra rw paths).
- `bwrap` missing or user namespaces broken: launch proceeds
  UNSANDBOXED with a loud warning (log + Debug header + settings
  status line). Never a silent downgrade, never a hard failure.
- Credit: design inspired by Bottles <https://github.com/bottlesdevs/Bottles>
  (GPL-3.0). No Bottles code is vendored or copied.

Caveats verified while implementing:

- Proton ≥ 8 / umu nest their own sandbox (pressure-vessel bwrap)
  inside ours; nested user namespaces are required (checked at
  runtime by the same smoke test as `bwrap` presence).
- Steam overlay / Heroic EAC-BE runtimes live under the real home:
  their exact paths are bound ro only when that game uses them.
- `cwd`: only umu/AppImage set `current_dir` today; the sandbox sets
  `--chdir` to the game folder always (the non-isolated flow is
  byte-identical to before).
