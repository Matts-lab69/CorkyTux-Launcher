# Changelog

All notable changes to CorkyTux. The format follows Keep a Changelog and the
project uses SemVer. The plugins are released from CorkyTux-Plugins and are
named here when they ship the same day as a launcher release.

## [3.2.0] - 2026-10-24

Ships with the plugin releases `minecraft-launcher` 1.2.0,
`heroic-store` 1.2.0 and `dependency-installer` 2.1.4. The Ely.by sign-in
needs the launcher and `minecraft-launcher` 1.2.0 updated together.

### Security

- Plugin processes: allowlisted environment, per-command deadlines and
  signalling by syscall instead of a PATH-resolved `kill` (#3).
- Every plugin child leads its own process group, and a timeout or a cancel
  kills the whole group; `run` keeps the direct-PID kill so the game
  survives (#5).
- The Ely.by password travels on the plugin stdin, after an
  `ely-auth --probe` handshake, and is never in the argv or in an error
  message. The plugin refuses the argv password in the other direction (#8).
- Game logs are created `0600` from the `open()`, an old `0644` log is
  tightened on the next open, and secrets are redacted from the log the UI
  shows, from the saved copy and from the dependency installer errors (#6).
- Destructive deletes confirm first and refuse Steam-managed prefixes,
  shared prefixes by canonical path, symlinks and ancestors, with an honest
  message when a removal is only partial.
- Sandboxing fails closed without `bwrap` and validates the extra paths.

### Added

- In-app launcher update with a verified download and a restart gate.
- Individual game prefixes, the shared-prefix membership predicate, adding a
  game to a shared prefix, unlinking one and pruning missing paths.
- Change executable action, with helper executables hidden from the list.
- Session handling: park non-Wine sessions instead of blocking on them,
  refuse to relaunch a running game, stop parked sessions, purge dead parked
  entries, and close RPG sessions from the supervised status.
- Minecraft: batched loader probe with a per-session cache, and beta loaders
  marked without an error probe.
- Dependency installer: optional low-confidence detections shown but never
  installed automatically.
- `release/build-release.sh` requires the tag and writes `checksums.txt`.
- Release workflow: a `vX.Y.Z` tag builds the tarball and opens a draft
  release with `checksums.txt`; a manual dispatch is the dry run (#15).
- Launcher CI: format, clippy, tests and GTK/libadwaita/glib version checks
  on every push and pull request (#4).
- Plugin protocol v1 specified in `docs/PLUGIN_PROTOCOL.md` (#1), the
  command inventory in section 9 (#17), roadmap (#2), pending items rewrite
  (#10) and this merge plan (#11).

### Changed

- Installer: atomic binary replace, three backups of the previous binary,
  an assets gate that validates before mutating anything, a dynamic version,
  a librsvg check and an uninstall that says what it keeps.
- Dialog text legible in both light and dark themes.
- Store cards at a fixed size with sharp cropped covers and English source
  labels.
- Performance: a real texture LRU with scale-aware decoding and an arena
  cap, static regexes and a bounded themed-image registry.
- Config: an atomic `Launcher.ini` write failure is reported instead of
  being swallowed.
- README states the runtime floor: GTK 4.12+, libadwaita 1.5+, built on
  Ubuntu 24.04 (glibc 2.39); older glibc is not tested (#14, #16).
- Documentation in English: README and BUILD rewritten (#9), design and
  audit notes, comments across the codebase, icon attribution.
- The protocol document describes the target state; the runtime still serves
  the v0 adapter until the protocol migration lands.

### Fixed

- Neoforge instance ids are parsed, and the bases inherited through
  `inheritsFrom` are dropped.
- The instance container is swept only when it is empty, so a delete no
  longer removes a directory that still holds games.
- Test environment races that broke the CI: process environment access is
  serialized and the isolation tests stop replacing `PATH` and `HOME` under
  other threads.
- The chmod-denied test parts are skipped when the filesystem does not
  enforce permissions, instead of reporting a failure that cannot happen on
  a real install.

### Updated 2026-10-25 (on top of the 3.2.0 preparation)

- #24 fix(updater): redirect diagnostics and loopback tests.
- #25 feat(steam): overlay startup readiness.
- #26 fix(settings): align plugin row action buttons.
- #58 fix: small pending items (#30, #31, #32, #33, #34, #35, #38, #40):
  `.desktop` fields quoted/escaped with mode 0644; `XDG_DATA_HOME` honoured
  in the plugin dir and in `app_menu_dir`; executable bit checked in
  `plugin_available`; ARM/ARM64EC and i686/armv7 arch detection; Steam
  `compatibilitytools.d` and system Proton in `all_proton_paths`; Flatpak
  Steam roots in `scan_steam`; plugin description cut at a word boundary;
  `/home` dropped from the import text and the README architecture map
  completed.
