# 3.2.0 release notes and commands

Status: preparation. No tag and no release exists for 3.2.0. Nothing here is
signed; signature verification is in preparation and the notes do not mention
it.

## Prerequisites (merge order)

1. PR #22 (`fix/no-embedded-sgdb-key`): remove the embedded SteamGridDB key,
   so the 3.2.0 binary never carries it. Key rotation is the maintainer's
   task.
2. The updater redirect fix (agent B delivers it; **I merge it**, and only
   with the maintainer's approval, once B delivers and the maintainer
   reviews it). Branch `fix/updater-redirect`, worktree
   `/tmp/corkytux-updater`. The installed 3.1.2 binary fails with
   "Update failed: redirect target not allowed:
   https://api.github.com/repos/Matts-lab69/CorkyTux-Launcher/releases/latest";
   the 64 `self_update` tests pass but do not cover that case. Agent B's
   notes go in `docs/UPDATER_NOTES.md` (agent B's file; I do not edit it).
   I do not reimplement the fix; I only integrate B's PR.
3. PR #23 (`release/3.2.0`): the version bump and this preparation. It
   goes last.

PR #21 (docs) and Plugins PR #11 (perf, 1.3.0 candidate) are independent.

Release set:

| Repository | Tag | Assets |
|---|---|---|
| CorkyTux-Plugins | `dependency-installer-v2.1.4` | `dependency-installer-2.1.4.tar.gz`, `checksums.txt` |
| CorkyTux-Plugins | `heroic-store-v1.2.0` | `heroic-store-1.2.0.tar.gz`, `checksums.txt` |
| CorkyTux-Plugins | `minecraft-launcher-v1.2.0` | `minecraft-launcher-1.2.0.tar.gz`, `checksums.txt` |
| CorkyTux-Launcher | `v3.2.0` | `corkytux-3.2.0-linux-x86_64.tar.gz`, `checksums.txt` |

Order: the three plugins, `minecraft-launcher` immediately before the
launcher. The new launcher tells an old-plugin user to update from Settings >
Plugins, which only works if the new plugin is already published.

## CorkyTux 3.2.0

```
CorkyTux 3.2.0

Ely.by sign-in: update minecraft-launcher 1.2.0 at the same time

The Ely.by password now travels on the plugin stdin, never on the command
line, where every local user can read it. The launcher and the plugin must
be updated together:

- new launcher + old plugin: the sign-in refuses with "the installed Ely.by
  plugin is too old: it does not accept the password on stdin. Update it
  from Settings > Plugins. I do not put the password in the command line on
  purpose."
- old launcher + new plugin: the plugin refuses with "the password must
  arrive on stdin (--password-stdin): the command line is visible to every
  local user".
- new launcher + new plugin: the sign-in works.
- old launcher + old plugin: it keeps working as before, with the password
  in the argv.

To update: open Settings > Plugins, install minecraft-launcher 1.2.0, then
sign in again. The plugin is published just before this release.

Also in 3.2.0
- Plugin processes: allowlisted environment, per-command deadlines and
  process-group kills; the game survives a timeout.
- Secrets are redacted before the log, the saved copy and the installer
  errors.
- Destructive deletes confirm first and refuse Steam libraries, shared
  prefixes, symlinks and ancestors.
- In-app launcher update with a verified download and a restart gate.
- Individual game prefixes, change-executable, session parking and a
  supervised stop.
- Minecraft: batched loader probes, beta loaders without an error probe,
  and NeoForge 26.3 through its legacy id.
- Store cards at a fixed size with sharp cropped covers and English source
  labels.
- Plugin protocol v1 in docs/PLUGIN_PROTOCOL.md; the runtime still serves
  the v0 adapter.
- Runtime floor: GTK 4.12+, libadwaita 1.5+, built on Ubuntu 24.04
  (glibc 2.39). Older glibc is not tested.
- SteamGridDB covers now require a key of your own: Settings > Integrations.
  Without one the lookup is skipped and the status says so; Steam and Lutris
  covers keep working.
```

## minecraft-launcher 1.2.0

```
Minecraft Launcher 1.2.0

Ely.by: the password arrives on stdin

The plugin takes the Ely.by password on stdin (--password-stdin) after an
ely-auth --probe handshake, and refuses it on the command line, which is
visible to every local user. Requires CorkyTux 3.2.0 or newer: update the
launcher and this plugin together. An old launcher sends the password in
the argv and the sign-in refuses with "the password must arrive on stdin
(--password-stdin)".

CurseForge downloads with spaces and accents

CurseForge returns some file names with raw spaces, brackets or accents.
The plugin now percent-encodes the download URL exactly once, so a file
such as OpenTerrainGenerator-1.12.2 - v8.3.jar is fetched instead of
failing with "URL can't contain control characters". This unblocks the
Karmaland 3 installs that hit that file.

Also in 1.2.0
- Secrets are redacted from every message before it leaves the plugin;
  account state stays private.
- Atomic mod update.
- NeoForge 26.3 installs through its legacy id, and loader-versions uses
  one batched call.
- The Ely probe accepts the launcher's command line without --username.
```

## dependency-installer 2.1.4

```
Dependency Installer 2.1.4

- Mono detection now gates every .NET verb, not only one, so a Wine/Proton
  mscoree.dll stub no longer produces a false positive.
- Only valid winetricks verbs are passed; an invalid verb is rejected with
  the details instead of being sent to winetricks.
- Tests cover the verb map and the media confidence path.
- Style and lint cleanups; no behavior change outside the fixes above.
```

## heroic-store 1.2.0

```
Heroic Store 1.2.0

- The Epic catalog now reports its build version and date, so the launcher
  can show which catalog it is reading.
- Plugin state stays private.
- Style and lint fixes; the GraphQL templates are excluded from the lint.
```

## Pre-tag checks

Build the plugin tarballs from Plugins `main` at `ef6cc57` and check each
one. The output directories are separate so each `checksums.txt` has one
line:

```
cd /home/mattsgaming/CorkyTux-Plugins
git rev-parse --short main          # must be ef6cc57
for p in dependency-installer:2.1.4 heroic-store:1.2.0 minecraft-launcher:1.2.0; do
  id=${p%%:*}; ver=${p##*:}; d=/tmp/opencode/rel-3.2.0/$id
  mkdir -p "$d"
  tools/plugin-tarball.sh "$id" "$d"
  tar tzf "$d/$id-$ver.tar.gz"
  (cd "$d" && sha256sum -c checksums.txt)
  tar xzf "$d/$id-$ver.tar.gz" -O "$id/plugin.json" | python3 -c \
    'import json,sys; print(json.load(sys.stdin)["version"])'
done
```

After the release PR is merged, run the launcher dry run from `main` with
`v3.2.0` and check the CI artifact. It creates no tag and no release:

```
cd /home/mattsgaming/CorkyTux-Launcher
git checkout main && git pull --ff-only
grep -m1 '^version' Cargo.toml            # 3.2.0
gh workflow run release.yml -f tag=v3.2.0
sleep 5
run=$(gh run list --workflow=release.yml --limit 1 --json databaseId --jq '.[0].databaseId')
gh run watch "$run" --exit-status
rm -rf /tmp/opencode/release-3.2.0-dry && mkdir -p /tmp/opencode/release-3.2.0-dry
gh run download "$run" -n corkytux-v3.2.0 -D /tmp/opencode/release-3.2.0-dry
tar tzf /tmp/opencode/release-3.2.0-dry/corkytux-3.2.0-linux-x86_64.tar.gz
(cd /tmp/opencode/release-3.2.0-dry && sha256sum -c checksums.txt)
tar xzf /tmp/opencode/release-3.2.0-dry/corkytux-3.2.0-linux-x86_64.tar.gz \
  -C /tmp/opencode/release-3.2.0-dry
objdump -T /tmp/opencode/release-3.2.0-dry/corkytux-3.2.0/corkytux \
  | grep -o 'GLIBC_[0-9.]*' | sort -V -u | tail -3
```

Expected: one top-level `corkytux-3.2.0/`, `sha256sum -c` clean, and
`GLIBC_2.34`, `GLIBC_2.35`, `GLIBC_2.39`.

## Publish

The plugin notes go in files so nothing is retyped. The tags point at the
commit the tarballs were built from:

```
cd /home/mattsgaming/CorkyTux-Plugins
git rev-parse --short main          # ef6cc57

gh release create dependency-installer-v2.1.4 \
  /tmp/opencode/rel-3.2.0/dependency-installer/dependency-installer-2.1.4.tar.gz \
  /tmp/opencode/rel-3.2.0/dependency-installer/checksums.txt \
  --target ef6cc57 --title "Dependency Installer 2.1.4" \
  --notes-file /tmp/opencode/rel-3.2.0/notes-dependency-installer.md

gh release create heroic-store-v1.2.0 \
  /tmp/opencode/rel-3.2.0/heroic-store/heroic-store-1.2.0.tar.gz \
  /tmp/opencode/rel-3.2.0/heroic-store/checksums.txt \
  --target ef6cc57 --title "Heroic Store 1.2.0" \
  --notes-file /tmp/opencode/rel-3.2.0/notes-heroic-store.md

gh release create minecraft-launcher-v1.2.0 \
  /tmp/opencode/rel-3.2.0/minecraft-launcher/minecraft-launcher-1.2.0.tar.gz \
  /tmp/opencode/rel-3.2.0/minecraft-launcher/checksums.txt \
  --target ef6cc57 --title "Minecraft Launcher 1.2.0" \
  --notes-file /tmp/opencode/rel-3.2.0/notes-minecraft-launcher.md
```

Then tag the launcher. The workflow builds the tarball and opens a **draft**
release; publish it only after checking the assets:

```
cd /home/mattsgaming/CorkyTux-Launcher
git checkout main && git pull --ff-only
grep -m1 '^version' Cargo.toml      # 3.2.0
git tag -a v3.2.0 -m "CorkyTux 3.2.0"
git push origin v3.2.0

run=$(gh run list --workflow=release.yml --limit 1 --json databaseId --jq '.[0].databaseId')
gh run watch "$run" --exit-status
gh release view v3.2.0 --json isDraft,assets \
  --jq '{draft:.isDraft,assets:[.assets[].name]}'

gh release edit v3.2.0 --draft=false --title "CorkyTux 3.2.0" \
  --notes-file /tmp/opencode/rel-3.2.0/notes-corkytux-3.2.0.md
```

## Dry run result (2026-10-25)

Run `40148473363` (`workflow_dispatch`, ref `release/3.2.0`, input `tag=v3.2.0`)
succeeded. The artifact is named `corkytux-v3.2.0`, as the workflow and the
notes assume. Checks on the downloaded artifact:

- `tar tzf`: one top-level `corkytux-3.2.0/`, no absolute paths.
- `sha256sum -c checksums.txt`: clean.
- `objdump -T`: `GLIBC_2.34`, `GLIBC_2.35`, `GLIBC_2.39`.

The `publish` job did not run (no tag), so no release and no tag were
created. That run was from the release branch; after the three
prerequisites and PR #23 merge, **repeat the dry run from `main`** with
`v3.2.0` and re-check `tar tzf`, `sha256sum -c` and `objdump`. Then, once
the tag push creates the draft, verify the sha256 digest of every draft
asset with `gh api` before publishing (see below).

## Updater test plan (temp HOME, mock API)

The installed 3.1.2 binary predates the updater (first updater commit
`dbfdeda`, 2026-10-21, in no tag), so it cannot self-update. The first real
update that can be tested is **3.2.0 -> 3.2.1**. To exercise the updater now:

1. Build a launcher tarball from a commit that contains the updater (for
   example the current `main` after the updater PR merges) with the version
   set to `3.1.2` so the updater sees a newer release:
   `cargo build --release`, then `release/build-release.sh 3.1.2` (or edit
   `Cargo.toml` temporarily on a throwaway branch).
2. Install it into a temp HOME with the shipped installer:
   `HOME=/tmp/corky-updater-test ./install.sh /tmp/corky-updater-test/.local ...`
   (or extract the tarball by hand). Keep the real `~/.config/CorkyTux` and
   `~/.local/share/CorkyTux` untouched.
3. Point the binary at a local mock of the GitHub API. The override is
   compiled only under `cfg(test)`, so a release binary cannot be redirected
   without a test build; the practical options are a hosts-file redirect of
   `api.github.com` to `127.0.0.1` plus a local HTTPS server with a trusted
   self-signed certificate, or running the loopback suite in
   `src/backend/self_update.rs` (64 tests, `test_api_override`). The unit
   suite is the verified path; the end-to-end binary path above was **not**
   run by me.
4. Serve a release JSON whose `tag_name` is `v3.2.0`, with an asset named
   `corkytux-3.2.0-linux-x86_64.tar.gz`, a real `digest`, and a matching
   `checksums.txt`. Let the installed 3.1.2 binary check for updates and
   accept.
5. Verify: the download, the `--version` health check on the staged binary,
   the atomic replace, the `assets.bak-*` backup, and the rollback on a
   forced bad digest.

Backup and rollback steps for a real machine:
- Back up `~/.local/share/corkytux` and `~/.config/CorkyTux` before testing.
- Keep the previous binary as `corkytux.bak-<timestamp>` (the installer
  already does this; do not delete it).
- If the update breaks, restore the backup binary and the config directory,
  then run `corkytux --version` to confirm.

## Before publishing (draft checks)

Drafts do not appear in `/releases/latest`, so list the draft directly and
check every asset digest before flipping it to published:

```
gh release view v3.2.0 --json isDraft,assets \
  --jq '{draft:.isDraft,assets:[.assets[]|{name,digest}]}'
gh api repos/Matts-lab69/CorkyTux-Launcher/releases/tags/v3.2.0 \
  --jq '{tag:.tag_name,draft:.draft,assets:[.assets[]|{name,digest}]}'
```

Both must show the tarball and `checksums.txt` with a non-null `sha256:`
digest. A null digest means GitHub did not finish hashing the upload; do not
publish.

## Performance work

Plugins PR #11 (`perf/minecraft-install`) is a 1.3.0 candidate and is **not**
part of 3.2.0. It is not merged here.

## After publishing

```
gh release view dependency-installer-v2.1.4 --json isDraft,isPrerelease,assets \
  --jq '{draft:.isDraft,pre:.isPrerelease,assets:[.assets[].name]}'
gh release view heroic-store-v1.2.0 --json isDraft,isPrerelease,assets \
  --jq '{draft:.isDraft,pre:.isPrerelease,assets:[.assets[].name]}'
gh release view minecraft-launcher-v1.2.0 --json isDraft,isPrerelease,assets \
  --jq '{draft:.isDraft,pre:.isPrerelease,assets:[.assets[].name]}'
gh api repos/Matts-lab69/CorkyTux-Launcher/releases/latest \
  --jq '{tag:.tag_name,draft:.draft,pre:.prerelease,assets:[.assets[]|{name,digest}]}'
```

The last command must show `v3.2.0`, `draft:false`, `prerelease:false`, the
tarball and `checksums.txt`, and a non-null `sha256:` digest. The updater
refuses a release without a digest.
