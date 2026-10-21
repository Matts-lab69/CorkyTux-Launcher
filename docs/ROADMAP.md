# Plugin and Security Roadmap

Status: proposal. No GitHub issues have been created from this document.

Scope: CorkyTux Launcher plugin discovery, execution and installation; the
official `CorkyTux-Plugins` repository; and the security boundaries those two
projects share. Each item is written so it can become one GitHub issue.

Priorities: P0 blocks a secure or reliable plugin release; P1 is required for
the first versioned protocol; P2 is follow-up work.

## Priority order

1. Runtime safety: file permissions, command timeouts and a clean environment.
2. Key rotation: remove the leaked SteamGridDB key from the binary.
3. Safe extraction: plugins, Proton/GE and AppImages.
4. Integrity: checksums and signatures for plugins and self-update.
5. Protocol v1 and migration.

## 1. Runtime safety

### 1. Fix secrets at rest and directory permissions (P0)

New `Launcher.ini` files are created with the process umask and can be `0644`,
yet the file can contain `IntegrationApiKey` and a CurseForge key. Plugin token
files have the same problem: Minecraft writes `accounts.json` with
`Path.write_text`, while Heroic already uses `0600` for GOG auth.

Files: `src/backend/config.rs` (`atomic_write_same_dir`),
`CorkyTux-Plugins/minecraft-launcher/minecraft-launcher` (account/auth writes),
`CorkyTux-Plugins/heroic-store/heroic-store`.

Acceptance: new config and token files are `0600`; plugin state directories are
`0700`; existing `0644` secrets are migrated or reported; tests assert modes.

Security finding: #6.

### 2. Enforce per-command timeouts and cancellation (P0)

`run_single_json`, emulator install/link/remove/unlink and AppImage run currently
use unbounded `Command::output()`. Give every command a declared deadline, kill
the process group on expiry, and surface a retryable error.

Files: `src/backend/plugin_process.rs` (`run_single_json`),
`src/backend/plugins.rs` (`install_emulator_in`, link/remove helpers).

Acceptance: a plugin that sleeps forever is terminated at the deadline; the UI
stays responsive; the error says which command timed out. The `run` deadline is
the time allowed for the plugin to respond, not a lifetime limit for the game
it launches.

Security finding: #9.

### 3. Clean the plugin environment and terminate by PID (P0)

Plugin processes inherit the full launcher environment, including tokens and
loader variables, and `ProcessKiller` resolves `kill` through `PATH`. Build an
explicit environment allowlist and terminate with a direct syscall.

Files: `src/backend/plugin_process.rs` (`spawn_streaming_inner`,
`run_single_json`, `ProcessKiller::kill`).

Acceptance: a plugin cannot see unlisted variables such as API keys or
`LD_PRELOAD`; killing a plugin does not depend on `PATH`; tests cover the
allowlist and the direct kill path.

Security finding: #12.

## 2. Key rotation

### 4. Rotate and remove the hardcoded SteamGridDB key (P0)

`IntegrationManager` embeds a public SteamGridDB key in the binary. Treat it as
leaked, rotate it and move to a user-supplied key or a server-side proxy with
per-user quotas. Never ship a shared secret in a desktop binary.

Files: `src/backend/integration.rs` (API key constant and request code).

Acceptance: no production secret in the repository or binary; the UI explains
how to configure a personal key; the old key is revoked.

Security finding: #7.

## 3. Safe extraction

### 5. Replace plugin `tar -xf` with a safe extractor (P0)

The plugin installer currently runs `tar -xf` as the user with no path,
symlink, hardlink, entry-count or size checks. Reuse the hardened extraction
model already present in `src/backend/self_update.rs` or implement an equivalent
staging extractor.

Files: `src/backend/plugins.rs`; reference:
`src/backend/self_update.rs` (`extract_tarball_limited`).

Acceptance: `../`, absolute paths, symlinks, hardlinks, device nodes and bomb
archives are rejected; extraction is atomic and cleaned on failure.

### 6. Harden Proton, GE and AppImage downloads (P0)

Proton/GE downloads and emulator AppImage downloads use HTTPS but no artifact
hash or signature, and Proton extraction uses `tar -xzf` directly into the
destination. Add published hashes or signatures, size limits, safe extraction
and destination-name validation. Reuse the `self_update.rs` extraction limits.

Files: `src/backend/proton.rs` (`download_proton`, `fetch_releases`),
`CorkyTux-Plugins/emulator-manager/emulator-manager` (`install_emulator`).

Acceptance: a corrupted or replaced archive is rejected; extraction cannot
write outside the target; an oversized archive is refused before unpacking.

## 4. Integrity

### 7. Sign plugin releases and verify before install and execution (P0)

Sign each release artifact with minisign, publish the public key and a detached
signature, and verify the signature before extraction. Re-verify the installed
entrypoint hash before execution or on every launch. The private key stays
off CI: signing happens locally and only the `.minisig` is uploaded.

Files: `src/backend/plugins.rs`, `src/backend/plugin_process.rs`.

Acceptance: an unsigned or wrongly signed artifact is rejected; a modified
installed entrypoint is refused; the UI shows the signer and fingerprint.

### 8. Add SHA-256 checksums and pin the download origin (P0)

Publish `checksums.txt` per release and verify the downloaded archive against
it. Pin the repository, release tag and asset name; do not trust an arbitrary
`browser_download_url` returned by the API.

Files: `src/backend/plugins.rs` (`fetch_registry`, `download_plugin_in`).

Acceptance: checksum mismatch aborts before extraction; redirects to other
hosts are rejected; the selected asset must match the expected name.

### 9. Add signed self-update and key rotation (P0)

Self-update already checks SHA-256 against the GitHub release digest and an
optional `checksums.txt`, but both come from the same release. Add a detached
minisign signature with a pinned public key, document key rotation and make a
missing signature a hard failure for stable releases.

Files: `src/backend/self_update.rs`, `src/ui/update_dialog.rs`.

Acceptance: a compromised GitHub release cannot install without the private
key; a rotated key is accepted only through an explicit signed transition.

### 10. Close the self-update TOCTOU window (P1)

The staged binary is health-checked and later copied by path; a same-user writer
can swap it in between. Hash and copy from the same file descriptor, or copy to
the final temp file first and health-check that copy immediately before rename.

Files: `src/backend/self_update.rs` (`install_if_healthy`,
`atomic_replace_mode`).

Acceptance: a test swaps the staged file between check and copy and the install
must fail; no path-based re-open remains between verification and rename.

## 5. Protocol v1 and migration

### 11. Publish the versioned plugin protocol and JSON Schema (P1)

Write `docs/PLUGIN_PROTOCOL.md` with the v1 contract and add a machine-readable
JSON Schema under `docs/schema/plugin-v1.schema.json`. Use a resolvable `$id`
and define extension rules that do not contradict forward-compatible minors.

Acceptance: every field, command, event, timeout and exit code is documented;
the schema rejects the current malformed manifests; the document has a
compatibility and deprecation section.

### 12. Make manifests canonical and validate them at load time (P1)

Require `protocolVersion`, `id`, `name`, `version`, `entry`, `type`,
`capabilities` and `protocol`. Validate `id` against the directory name, reject
path separators and symlinks in `entry`, and reject unknown protocol major
versions with a user-visible error.

Files: `src/backend/plugins.rs` (`parse_plugin_json`, `refresh`),
`CorkyTux-Plugins/*/plugin.json`.

Acceptance: a manifest missing a required field is not offered as installed;
the error names the file and the missing field.

### 13. Dispatch plugins by manifest, not hardcoded ids (P1)

Replace the fixed `ID`/`ENTRY` pairs and `plugin_exe(id, entry)` call sites with
a registry loaded from validated manifests. Keep typed adapters where the UI
needs a specific capability, but resolve the executable through the manifest.

Files: `src/backend/plugin_process.rs`, `src/backend/external.rs`,
`src/backend/plugins.rs`.

Acceptance: changing `entry` in a valid manifest changes the executed file; no
code path uses a hardcoded plugin path for a new plugin.

### 14. Standardize the response envelope and error parsing (P1)

Define one envelope for single and JSON Lines modes: `type`, `ok`, `data`,
`error`. The launcher must read the same error field in `run_single_json` and
`run_json` instead of one reading `message` and the other treating stdout as
opaque text.

Files: `src/backend/plugin_process.rs`, `src/backend/plugins.rs`.

Acceptance: fixtures for success, `type=error`, `ok=false`, non-JSON stdout and
a crash all produce the same UI error path.

### 15. Define and enforce the exit-code contract (P1)

Keep `0`, `2`, `3`, `4`, `5` and `6`; document what each plugin may use and map
codes to UI categories. Reject undocumented non-zero codes unless the manifest
declares them.

Acceptance: the exit-code table is tested; Minecraft `4`/`5` and Heroic `5`
behave as documented; a plugin returning an unknown code gets a generic error.

### 16. Add a `describe` command for plugin metadata (P2)

Let a plugin report its commands, capabilities, settings schema and runtime
requirements without the launcher parsing plugin-specific output. Use it for
emulator settings and add-game cards.

Acceptance: the launcher can render a plugin card and a settings form from
`describe`; no UI code parses `emulators` JSON directly.

### 17. Specify the release archive layout (P1)

Document and test that every release asset is a `.tar.gz` with one top-level
`<id>/` directory containing the executable, `plugin.json`, `README.md` and
runtime assets. Reject archives with multiple top-level directories, absolute
paths or a missing manifest.

Files: `src/backend/plugins.rs` (`download_plugin_in`),
`CorkyTux-Plugins/README.md`.

Acceptance: a fixture archive installs; a flat archive is rejected; a tarball
with two plugin directories is rejected.

### 18. Add a compatibility matrix and deprecation policy (P2)

Maintain a table of Launcher versions, protocol versions and plugin versions.
Define how long old majors are supported and how the UI warns before an
incompatible plugin is disabled.

Acceptance: the matrix is checked in; the loader uses it to warn or block.

### 19. Add a trust and permission prompt before first execution (P1)

Show plugin id, publisher, version, signer, capabilities and install path.
Require explicit consent for capabilities that execute code, write outside the
plugin directory, access accounts or launch games. Store the decision per
version and per signature.

Acceptance: a new or changed signer requires a new prompt; disabling a plugin
stops all execution paths.

### 20. Keep auth codes out of argv and logs (P1)

The Epic/EOS exchange code and display name are concatenated into a launch
argument string. The code is visible in `/proc/<pid>/cmdline`, and a display
name containing spaces or option-like text can change the argument list. Pass
values as separate argv entries, redact them in logs and prefer a pipe or
environment variable over the command line.

Files: `src/backend/proton.rs` (EOS argument construction and
`split_quoted_args`).

Acceptance: the code never appears in argv or logs; a display name with spaces
or `-`-prefixed text cannot inject extra arguments.

### 21. Repair the official plugin documentation (P1)

The current docs describe commands that the plugins do not accept: AppImage
`scan` is a single-file command, RPG Maker `install` uses subcommands, Heroic
`logout` requires `--store`, and the Minecraft guide uses an older CLI. Versions
and capabilities are stale in the main README.

Acceptance: every command in `CorkyTux-Plugins/README.md` and each plugin README
is executed in CI or a smoke script; versions match `plugin.json`; AppImage,
RPG Maker, Heroic and Minecraft examples use the current syntax.

### 22. Add the AGPL license, per-plugin guides and stable assets (P1)

Add a root `LICENSE` (AGPL-3.0), fix the README link to it, add minimal guides
for `dependency-installer` and `dll-overrides-automator`, and keep the logo at
the stable `assets/corkytux.png` path.

Acceptance: every plugin directory has a README; the license link resolves; the
logo URL returns 200 from the default branch; the vendor `NOTICE` remains.

### 23. Improve plugin install and update UX (P2)

Show version, signer, capabilities, size, install path and update availability
before install. After install, show whether the signature and manifest are
valid. On removal, offer to keep or delete plugin state.

Acceptance: users can tell what a plugin can do before granting consent; an
invalid plugin is never shown as installed; removal never deletes game data
without confirmation.

### 24. Make plugin errors actionable (P2)

Replace generic "plugin failed" messages with the command, exit code, plugin
id and a remediation hint. Keep the raw stderr available in a details expander
and redact tokens.

Acceptance: each exit code maps to a message; tests cover missing backend,
auth failure, timeout and malformed JSON.

### 25. Add linting and integration tests (P1)

Run `ruff` on all plugin Python, `shellcheck` on `heroic-store/install.sh`,
`python -m json.tool` on every manifest, and the JSON Schema validation from
item 11 in CI. Add integration tests for registry parsing, asset selection,
checksum/signature failure, archive traversal, manifest validation, timeout
kill and generic dispatch. Use temp directories and loopback HTTP servers only.

Acceptance: CI fails on a syntax error, an invalid manifest or a shellcheck
error; a malicious archive and a hanging plugin are exercised in tests; the
suite runs with `cargo test` and does not require network access.

## Suggested sequence

1. Permissions, timeouts and environment (items 1-3).
2. SteamGridDB key rotation (item 4).
3. Safe plugin, Proton and AppImage extraction (items 5-6).
4. Checksums and signatures (items 7-10).
5. Protocol v1, manifests and migration (items 11-25).

## Out of scope

- Creating GitHub issues automatically.
- Changing game prefixes, saves or registry files during tests.
- Shipping a new plugin protocol before the design is approved.
