# CorkyTux Plugin Protocol v1

Status: specification for plugin protocol v1. The launcher keeps a v0
compatibility adapter while plugins migrate.

This document is normative for manifests, command envelopes, timeouts and
signature policy. The JSON Schemas are:

- Runtime schema: `docs/schema/plugin-v1.schema.json`
- Strict registry/CI schema: `docs/schema/plugin-v1-strict.schema.json`

## 1. Versioning and compatibility

A v1 manifest declares `protocolVersion: 1`. The major version is frozen:
the launcher rejects an unknown major with `unsupported_protocol`. Minor
versions add optional fields; older clients ignore fields they do not know.

`minLauncherVersion` is a SemVer string. The registry must not offer a plugin
whose `minLauncherVersion` is newer than the user's launcher. The launcher
must also check an already installed manifest and fail with `launcher_too_old`
instead of running it.

v0 manifests are accepted through the legacy adapter:

- `capabilities` is a list of command names, not a permission list.
- Top-level result fields become the v1 `data` object.
- A top-level `message` on an error becomes `error.message`.
- v1 `permissions` and `commands` are not required for v0.

New manifests must use `permissions` and `commands`. `capabilities` is kept
only as a v0 alias and must not be used by v1 plugins.

## 2. Manifest

Required in a v1 manifest:

| Field | Type | Meaning |
|---|---|---|
| `protocolVersion` | integer | Must be `1`. |
| `id` | string | Stable lowercase plugin id; must match the install directory. |
| `name` | string | Human-readable name. |
| `version` | string | Plugin SemVer. |
| `entry` | string | Executable file name inside the plugin directory. No path separators, no symlink. |
| `type` | string | UI category. See below. |
| `minLauncherVersion` | string | Minimum launcher SemVer. |
| `permissions` | array of strings | Closed permission enum. |
| `commands` | array of strings | Lowercase kebab-case command names. |

Optional: `description`, `source`, `protocol` (legacy metadata). Unknown
fields are allowed at runtime for forward compatibility.

Example:

```json
{
  "protocolVersion": 1,
  "id": "minecraft-launcher",
  "name": "Minecraft Launcher",
  "version": "1.2.0",
  "description": "Minecraft Java: accounts, versions, Java, instances and mods.",
  "entry": "minecraft-launcher",
  "type": "minecraft",
  "minLauncherVersion": "3.10.0",
  "permissions": [
    "network",
    "filesystem.read",
    "filesystem.write",
    "process.spawn",
    "process.signal",
    "credentials.read",
    "credentials.write",
    "game.launch"
  ],
  "commands": [
    "auth",
    "auth-config",
    "offline",
    "versions",
    "install",
    "launch",
    "stop",
    "status"
  ]
}
```

### 2.1 Types

The canonical v1 enum is lowercase kebab-case:

`appimage`, `tool`, `store`, `minecraft`, `rpgmaker`, `emulator`,
`emulator-manager`, `proton`, `integration`.

An unknown `type` at runtime is treated as `tool`. Type is a UI hint, not a
security boundary: a plugin published by a newer registry must not become
unusable because an older launcher does not know its new category. The strict
CI schema still rejects unknown types so the registry stays canonical.

### 2.2 Permissions

Permissions are a closed, frozen enum. The current set is:

| Permission | Grants |
|---|---|
| `network` | Outbound HTTP(S). |
| `filesystem.read` | Read game directories, prefixes and user-selected paths. |
| `filesystem.write` | Write game directories, prefixes and user-selected paths. |
| `process.spawn` | Execute external programs. |
| `process.signal` | Signal plugin, game or helper processes. |
| `credentials.read` | Read tokens, API keys and account state. |
| `credentials.write` | Store or rotate tokens, API keys and account state. |
| `desktop.integrate` | Write `.desktop` files or add menu entries. |
| `game.launch` | Start a game process that outlives the plugin. |
| `system.package` | Install or update runtimes, emulators and dependencies. |

An unknown permission fails closed with `unsupported_permission`. The launcher
must not silently ignore a permission the plugin says it needs: the plugin may
assume access it will not get, or the launcher may not understand the risk.
Adding a permission to the enum is a protocol minor change, but a plugin that
uses it must raise `minLauncherVersion` so older clients refuse it cleanly.

## 3. Commands and envelopes

A command is invoked as:

```text
<entry> <command> [arguments...]
```

The command must appear in `commands` (for v0, in `capabilities`). Commands are
lowercase kebab-case: `mods-add`, not `mods_add` or `ModsAdd`.

A plugin may use either transport:

- **Single JSON**: one JSON object on stdout, then exit.
- **JSON Lines**: one JSON object per line on stdout; progress events may
  precede the terminal event.

Diagnostics go to stderr. stdout must contain only protocol JSON.

### 3.1 Success and progress

```json
{"type": "done", "ok": true, "data": {"account_id": "abc", "username": "Player"}}
```

```json
{"type": "progress", "ok": true, "data": {"stage": "download", "percent": 42.5}}
```

`data` is an object. `percent` is optional; when present it is in the range
`0..100`.

### 3.2 Errors

```json
{
  "type": "error",
  "ok": false,
  "error": {
    "code": "unsupported_permission",
    "message": "This launcher does not support permission: credentials.export",
    "details": {}
  }
}
```

Reserved error codes:

| Code | Meaning |
|---|---|
| `bad_request` | Invalid arguments or manifest. |
| `unsupported_permission` | The launcher does not know a requested permission. |
| `unsupported_protocol` | Unknown protocol major. |
| `launcher_too_old` | `minLauncherVersion` is not satisfied. |
| `timeout` | The plugin exceeded its command deadline. |
| `plugin_failed` | The plugin reported an operational failure. |
| `auth_required` | The user must authenticate first. |
| `not_found` | Requested game, account or resource does not exist. |

Plugins may add their own codes, but `message` must be human-readable and must
not contain tokens, authorization codes or passwords. Exit code `0` means the
command completed without a protocol error; any other exit code must be
accompanied by an error envelope when possible.

### 3.3 v0 adapter

The launcher normalizes v0 before dispatch:

- `{"ok": true, "type": "done", "account_id": "abc"}` becomes
  `{"type": "done", "ok": true, "data": {"account_id": "abc"}}`.
- `{"ok": false, "type": "error", "message": "nope"}` becomes
  `{"type": "error", "ok": false, "error": {"code": "plugin_failed", "message": "nope"}}`.
- A document with no `type` but with `ok` is treated as `done` or `error`.
- v0 `capabilities` entries are copied to `commands` as-is.

The adapter does not invent permissions. A v0 plugin is treated as having the
legacy implicit permission set and is shown as such before first execution.

## 4. Timeouts

The launcher owns the deadlines. Values are upper bounds, not a target:

| Command class | Examples | Timeout |
|---|---|---|
| Launch/stop handshake | `run`, `stop` | 60 s |
| Ordinary query | `status`, `versions`, `list` | 120 s |
| Scan/diagnose | `scan`, `diagnose`, `heroic-scan` | 300 s |
| Install/auth/setup | `install`, `setup`, `auth`, `java-install`, `umu-setup` | 1800 s |

For `run`, the timeout covers only the plugin handshake and response. The
started game must not inherit that deadline; its lifetime belongs to the user
or the game session manager.

On timeout the launcher terminates the plugin. The v1 target is:

- For `run`, signal only the plugin handshake process. A game that has already
  been started must survive.
- For every other command, terminate the plugin process group, so helper
  children do not survive a cancelled scan or install.
- Do not use a `kill` binary resolved through `PATH`; use a direct syscall.

The process-group rule is a v1 requirement; the current launcher still signals
the direct plugin PID and therefore leaves grandchildren alive. That gap is
tracked separately from this document.

## 5. Signatures and trust

Signatures are mandatory for the official registry and for stable
self-update releases. Checksums alone are not a trust root.

### 5.1 Official plugin registry

Every published plugin tarball has a detached minisign signature:

```text
plugin-id-1.2.0.tar.gz
plugin-id-1.2.0.tar.gz.minisig
```

The launcher verifies the signature with a pinned public key before extracting
the archive. It then verifies the archive hash and the manifest inside the
archive. A missing, malformed or wrong signature is a hard failure for the
official registry.

Unsigned local plugins are allowed only in explicit development mode:

```text
CORKYTUX_ALLOW_UNSIGNED_PLUGINS=1
```

Without that variable, an unsigned plugin from a local path is rejected. The
development flag must never apply to registry installs.

### 5.2 Self-update

Stable self-update releases must publish a minisign signature for the release
tarball. The launcher verifies it with the same pinned trust store before
extracting the archive. A missing signature on a stable release is a hard
failure; a development build may opt into unsigned updates explicitly.

The private key never goes to CI. Signing is local and only the public key and
`.minisig` files are uploaded.

### 5.3 Local signing procedure

Generate a key once, offline:

```sh
minisign -G -p corkytux-plugins.pub -s corkytux-plugins.key
```

Sign a plugin tarball:

```sh
minisign -S -s corkytux-plugins.key \
  -m plugin-id-1.2.0.tar.gz \
  -t "CorkyTux plugin plugin-id 1.2.0"
```

This writes `plugin-id-1.2.0.tar.gz.minisig` next to the archive. Verify before
publishing:

```sh
minisign -V -m plugin-id-1.2.0.tar.gz \
  -x plugin-id-1.2.0.tar.gz.minisig \
  -p corkytux-plugins.pub
```

Record the key id and fingerprint from the public key in the release notes and
show the signer in the install prompt.

### 5.4 Public key rotation

The launcher ships a pinned trust store, not a single hardcoded key forever.
A trust store entry is `(key_id, public_key, status, not_after)`.

Rotation procedure:

1. Generate the new key offline. Never reuse the old private key for new
   releases after the rotation date.
2. Publish a rotation statement containing the old and new fingerprints and the
   first release that trusts the new key. Sign the statement with the old key.
3. Ship a launcher release whose trust store contains both old and new keys.
   The new key is `next`, not yet the only key.
4. Sign the next plugin and self-update artifacts with both keys. Publish both
   detached signatures, for example `artifact.minisig` and
   `artifact.minisig.next`.
5. After the overlap window, ship a launcher release that marks the old key
   `retired` and removes it from accepted signatures.

Emergency rotation for a compromised key follows the same steps but skips the
overlap for the compromised key: revoke it immediately, publish the statement
signed by the new key through an out-of-band channel, and make older launcher
versions reject releases after the revocation version.

A key is never silently replaced. A signature from an unknown key, an expired
`not_after`, or a revoked key is rejected with a user-visible trust error.

## 6. Manifest validation

Runtime validation is intentionally open (`additionalProperties: true`) so a
newer plugin can add optional fields without breaking older launchers.
The launcher still enforces security-critical rules:

- reject an unknown protocol major;
- reject `entry` containing `/`, `\`, `..` or a symlink;
- reject a plugin whose `minLauncherVersion` is newer than the launcher;
- fail closed on an unknown permission;
- map an unknown `type` to `tool`.

Registry/CI validation is strict (`additionalProperties: false`, closed type
and permission enums, canonical command pattern). A plugin with a malformed
manifest may exist locally for development, but it is not published in the
official registry.

## 7. Deprecation policy

- v0 manifests remain accepted until a launcher release explicitly drops the
  adapter; that release must announce the removal and raise the minimum
  launcher version for affected plugins.
- `capabilities` as a command list is deprecated. It must not be used in v1.
- New required fields require a protocol major bump.
- New optional fields are protocol minors. Clients ignore them when unknown.
- Removing or renaming a command requires a plugin major version and a
  deprecation window in the registry metadata.

## 8. Security checklist for reviewers

- Manifest id matches its directory.
- `entry` is a regular file, not a symlink, and has no traversal.
- Every command is listed and kebab-case.
- Permissions match observed behavior; no broad `filesystem.write` when only a
  plugin-owned directory is touched.
- Tokens and account files are created `0600` and state directories `0700`.
- The plugin does not print tokens, authorization codes or passwords to stdout
  or logs.
- Official artifacts have a valid minisign signature from a trusted key.
- `minLauncherVersion` is high enough for every feature and permission used.
