# Merge plan

Order in which the open pull requests land, what has to be rebased after
each merge, and where the `cargo fmt` commit fits. Nothing here merges
anything: it is the plan to follow once each PR is approved.

Rules that apply to the whole plan:

- One PR at a time per repository. `main` only moves through a merge.
- After every merge, the branches that were based on the old `main` are
  rebased (or retargeted) before the next merge.
- A rebase of a pushed branch is a force push. The branches here are
  review branches, so that is acceptable, but it must be deliberate.
- No commit is rewritten inside a merged PR.

## 1. CorkyTux-Plugins

PR #3 is already in `main` (`f8de746`, merge commit): it brought the lint
workflow and the ruff config that covers the entrypoints. Every branch below
is rebased on top of the previous one, so the order of this table is also the
order of the commits in the stack.

| Order | PR | Branch | Why here | After the merge |
|---|---|---|---|---|
| 1 | #1 | `fix/dotnet-mono-gate-all-verbs` | Pre-existing PR, same file as #4. | #4 is already rebased on its new base. |
| 2 | #4 | `fix/dependency-installer-verbs` | It carries `e001905` out of the local `main`. | `git branch -f main origin/main` (the commit is in `main` through the PR). |
| 3 | #2 | `fix/plugin-secrets` | Independent of the rest (Minecraft and Heroic files). | #5 is already rebased on its new base. |
| 4 | #5 | `fix/ely-password-stdin` | It needs the private state of #2 and ships with Launcher #8 (Ely.by on stdin). | It is retargeted to `main` and merged. |
| 5 | #7 | `fix/epic-catalog-version` | The catalog version and date of the Epic games. | It is retargeted to `main` and merged. |
| 6 | #6 | `style/entrypoint-lint` | Last: it fixes the 58 findings and turns the ruff step blocking again. | `main` ends with ruff at zero findings. |

After the plugins are in `main`:

- The ruff step in `.github/workflows/lint.yml` is `continue-on-error` while
  the entrypoints carry 58 findings. A dedicated commit
  (`style: entrypoint lint`, one commit per plugin file) fixes them, and the
  step goes back to blocking. The findings are listed in PR #3.
- The local `main` of the plugins repo still points at `e001905` until it is
  reset (see the table).

## 2. CorkyTux-Launcher

PRs #1, #2, #4, #9 and #10 are already in `main`. #7 was closed without
merging: its two commits are inside #4 (squash).

| Order | PR | Branch | Why here | After the merge |
|---|---|---|---|---|
| 1 | #3 | `fix/plugin-runtime-safety` | It is the base of #5 and #8: the environment allowlist, the timeouts and the test fixes. | #5 is retargeted to `main`. |
| 2 | #5 | `fix/plugin-process-group` | It needs the timeouts of #3. | #8 is retargeted to `main`. |
| 3 | #8 | `fix/ely-password-stdin` | Leaf of the stack. It ships with Plugins #5 (Ely.by on stdin). | Nothing. |
| 4 | #6 | `fix/launcher-secret-logs` | Standalone. It shares `plugins.rs` and `proton.rs` with the others, but every merge is clean. | Nothing. |

## 3. Merge strategy

A PR that stands on its own is **squashed**: `main` gets one commit that
says what the change does, and the branch's own commits stay in the PR for
the review.

**Exception, decided on 2026-10-22: every PR of a chain that has been
rebased one on top of another is merged with a merge commit, leaves
included.** Squashing a branch that is the base of the next one rewrites the
commits the children were rebased on, so every child would need a rebase
again before it could merge; with the whole Plugins chain in that shape
(#1 -> #4 -> #2 -> #5 -> #7 -> #6), a squash in the middle turns a clean
sequence into a chain of force pushes. A merge commit keeps the branch SHAs
the review approved and leaves the children exactly where they were rebased.
The price is one merge commit per PR in the history; the rule "no commit is
rewritten inside a merged PR" stays intact.

| Repository | PR | Strategy | Why |
|---|---|---|---|
| Launcher | #6 | squash | Standalone: secret logs. It is not the base of any other PR. |
| Launcher | #3 | **merge commit** | Base of the stack #5 -> #8. |
| Launcher | #5 | **merge commit** | Second of the stack: retargeted to `main` after #3, then merged. |
| Launcher | #8 | **merge commit** | Leaf of the stack: retargeted to `main` after #5, then merged. |
| Plugins | #1, #4, #2, #5, #7, #6 | **merge commit** (all of them) | The chain is rebased one on top of another: a squash anywhere in the middle moves the base of the next PR. |

Before each merge, `git merge-tree --write-tree --name-only origin/main
origin/<branch>` must come out clean (exit 0). If it reports a conflict, the
sequence stops and the conflict is resolved in the branch, not in `main`.

### 3.1 Never delete a branch that is the base of another PR

A branch in the middle of a stack is not deleted in the same step as its
merge. On 2026-10-22 Plugins #3 was merged with `--delete-branch`, which
deleted `docs/plugin-docs-and-lint`, the base of PR #6. GitHub closed #6 at
`01:13:27Z`; the branch had to be recreated at `01:14:25Z`, then the PR
reopened at `01:14:29Z` and retargeted at `01:14:33Z`. The PR was green
before and after: the only thing the deletion added was a closed PR and a
recreated branch in the middle of the sequence.

Order for every base PR (Plugins #1, #4, #2, #5, #7; Launcher #3 and #5):

1. Mark the PR ready and merge it **without** `--delete-branch`.
2. Retarget the child to `main`: `gh pr edit <child> --base main`
   (a no-op when its base is already `main`).
3. Wait for the child CI on the new base and confirm the child is still open,
   its checks are green and it is mergeable:
   `gh pr view <child> --json state,isDraft,mergeable,statusCheckRollup`.
4. Only then delete the old branch: `git push origin --delete <branch>`.

Leaf PRs (Plugins #6; Launcher #8 and #6) can be merged with
`--delete-branch` directly. If step 3 finds the child closed or conflicted,
the sequence stops and the child is reported before anything is recreated.

## 4. Coordinated release: the Ely.by password on stdin

Launcher PR #8 and Plugins PR #5 change one feature from both sides, so
they must be **published together**: a plugin release
(`minecraft-launcher` 1.2.0) and a Launcher release. Merging the code in
the two repositories can happen in any order; the **binaries** have to
reach the user together.

| Launcher | Plugin | What the user sees |
|---|---|---|
| new | old | The Ely.by sign-in fails with a clear message: "the installed Ely.by plugin is too old: it does not accept the password on stdin. Update it from Settings > Plugins. I do not put the password in the command line on purpose." The password never reaches the command line. |
| old | new | The sign-in fails with the plugin's message: "the password must arrive on stdin (--password-stdin): the command line is visible to every local user". The old launcher still sends the password in the argv, so the plugin refuses it. |
| new | new | The sign-in works: the launcher writes the password on the plugin stdin and closes the pipe. |
| old | old | Works as before, with the password in the argv. |

Both failure messages are deliberate: neither side falls back to the
command line, so a mismatch is a visible refusal and not a silent leak.

### 4.1 How an update reaches the user (read from the code)

| Piece | Automatic? | Evidence |
|---|---|---|
| Launcher update | No silent update. The startup check runs only when `checkUpdatesOnStartup=1` (default off: `unwrap_or(false)`) and it opens a dialog the user has to accept. | `src/ui/update_dialog.rs` (`auto_check`), `src/backend/self_update.rs` (`should_auto_check`) |
| Plugin update | Manual. The registry is fetched when Settings > Plugins is built; the install starts on a click. Nothing at startup reads the registry. | `src/ui/settings.rs:1979`, `src/backend/plugins.rs:383` |
| Compatibility gate | None. `minLauncherVersion` exists only in the protocol document and the two JSON schemas; no Rust code reads it. | `docs/PLUGIN_PROTOCOL.md:18`, `docs/schema/plugin-v1.schema.json:37`, `docs/schema/plugin-v1-strict.schema.json:56`; `grep -rn minLauncherVersion src/` is empty |
| Runtime handshake | The launcher asks `ely-auth --probe` for `password_stdin` and refuses to call the plugin without it. | `src/backend/external.rs` (`ely_auth`) |

The consequences are the same on both sides: nobody is pushed into the new
pair by a background job. A pair breaks only when the user performs one of
the two manual updates, and it stays broken until the user performs the
other one.

### 4.2 Publish order and the window

Order:

1. `dependency-installer` 2.1.4 (independent of the pair).
2. `heroic-store` 1.2.0 (independent of the pair).
3. `minecraft-launcher` 1.2.0.
4. CorkyTux 3.2.0, minutes after step 3.

The plugins go first because the new launcher, on finding an old plugin,
answers "Update it from Settings > Plugins": that instruction only works if
the new plugin is already published. The reverse order points the user to a
release that does not exist yet. `minecraft-launcher` goes immediately
before the launcher, and not earlier, to shrink the only asymmetrical case:
an old launcher with the new plugin.

Window, by case:

| First update | Second update | Time broken |
|---|---|---|
| Launcher 3.2.0 (accepted in the update dialog) | Plugin, from Settings > Plugins | Until the user opens Settings > Plugins. The error names the tab, so minutes in practice. |
| Plugin 1.2.0 (clicked in Settings > Plugins) | Launcher 3.2.0 | Until the user installs the launcher. The plugin message explains the refusal but does not name the launcher, so this can last days. |
| Both, in the reading order of the notes | - | No broken window. |

There is no forced cut-off: the old launcher keeps working with the old
plugin indefinitely, and the new launcher keeps working with the old plugin
for everything except the Ely.by sign-in. Publishing step 3 and step 4 the
same day is what keeps the second row rare, and the release notes put the
pairing in the first paragraph of both entries.

Optional follow-up, not part of this release: add "update CorkyTux to 3.2.0
or newer" to the plugin refusal message
(`minecraft-launcher/minecraft-launcher`, `cmd_ely_auth`). It changes the
tested plugin tree, so it belongs to the next plugin release.

### 4.3 Release notes (draft, English)

Launcher 3.2.0:

```
CorkyTux 3.2.0

Ely.by sign-in: install minecraft-launcher 1.2.0 as well
This release and minecraft-launcher 1.2.0 change how the Ely.by password
reaches the plugin: it now travels on stdin, never on the command line,
where every local user can read it. Update the launcher and the plugin
together. If only one of the two is updated, the sign-in refuses on purpose
instead of falling back to the command line:

- new launcher + old plugin: "the installed Ely.by plugin is too old: it
  does not accept the password on stdin. Update it from Settings > Plugins.
  I do not put the password in the command line on purpose."
- old launcher + new plugin: "the password must arrive on stdin
  (--password-stdin): the command line is visible to every local user".

To get it working: open Settings > Plugins, install minecraft-launcher
1.2.0, then sign in again.

Also in this release
- Plugin processes: allowlisted environment, per-call timeouts and a kill
  scope that reaches the whole process group.
- Secrets never reach the launcher log: plugin output is redacted first.
- Destructive deletes confirm first and refuse protected paths: Steam
  libraries, shared prefixes, symlinks and ancestors.
- In-app launcher update with a verified download.
- Plugin protocol v1 specified (docs/PLUGIN_PROTOCOL.md).
```

Plugin `minecraft-launcher` 1.2.0:

```
Minecraft Launcher 1.2.0

Ely.by: the password arrives on stdin
The plugin now takes the Ely.by password on stdin (--password-stdin) and
refuses it on the command line, which is visible to every local user.
Requires CorkyTux 3.2.0 or newer: update the launcher and this plugin
together. An old launcher sends the password in the argv and the sign-in
will refuse with "the password must arrive on stdin (--password-stdin)".

Other changes
- Secrets are redacted from every message before it leaves the plugin.
- Atomic mod update.
```

`heroic-store` 1.2.0 and `dependency-installer` 2.1.4 keep their own short
entries: the Epic catalog build version and date, the lint fixes, the Mono
gate for every .NET verb and the valid winetricks verbs.

### 4.4 Runtime floor and the release build

Measured on the tested binary (`target/release/corkytux`):

- `objdump -T target/release/corkytux | grep -o 'GLIBC_[0-9.]*' | sort -V -u | tail -3`
  gives `GLIBC_2.34`, `GLIBC_2.35`, `GLIBC_2.39`. The 2.39 entries
  (`pidfd_spawnp`, `pidfd_getpid`) are weak in the symbol table but they
  still appear in `objdump -p` under `VERNEED` with flags `0x00`. A two-line
  C program that references both with `__attribute__((weak))` produces the
  same `0x00`, so this is how the linker records the reference; glibc only
  tolerates a missing version when the flag is `0x02` (`VER_FLG_WEAK`). The
  loader therefore asks for glibc 2.39. Treat **glibc 2.39** as the floor. No older-glibc runtime was
  available here, so the floor is read from the ELF headers, not from a run
  on 2.35.
- GTK, libadwaita and glib: `gtk4` feature `v4_12`, `libadwaita` feature
  `v1_5` and `glib` feature `v2_80` in `Cargo.toml`, repeated as
  `require_min gtk4 4.12`, `require_min libadwaita-1 1.5` and
  `require_min glib-2.0 2.80` in `.github/workflows/ci.yml:44-59`.
- The machine that built the tested binary has GTK 4.20.4 and libadwaita
  1.8.6; that does not raise the floor, the features above set it.
- GTK does not version its exported symbols, so the GTK floor cannot be read
  from the ELF the way the glibc floor can. What can be said is that the
  `v4_12` and `v1_5` features make the build itself fail against an older
  GTK or libadwaita, so the declared floor is also the enforced one. No
  runtime test on GTK 4.12 or 4.14 was possible here.

Distro versions below are the ones in their stable repositories, not
measured on this machine; the enforced floor is the one above.

| Distribution | glibc | GTK | Verdict |
|---|---|---|---|
| Ubuntu 24.04 LTS | 2.39 | 4.14 | supported |
| Ubuntu 22.04 LTS | 2.35 | 4.6 | not supported: GTK too old |
| Debian 13 | 2.41 | 4.18 | supported |
| Debian 12 | 2.36 | 4.8 | not supported: GTK too old |
| Fedora 40 | 2.39 | 4.14 | supported |
| Fedora 39 | 2.38 | 4.12 | not supported: glibc too old |
| Arch, Gentoo, openSUSE Tumbleweed | rolling | >= 4.14 | supported |

Proposal: build the release in the CI on `ubuntu-24.04`, the same runner the
tests already use, triggered by the tag. That gives the same glibc 2.39 floor
as the binary built here and removes the "built on the maintainer's Gentoo"
question. Sketch, to be added in its own PR after the merges:

```yaml
name: release
on:
  push:
    tags: ["v*"]
permissions:
  contents: read          # default for every job
jobs:
  build:
    runs-on: ubuntu-24.04
    permissions:
      contents: read
    steps:
      - uses: actions/checkout@<sha>        # v4, pinned like ci.yml
      - uses: Swatinem/rust-cache@<sha>     # v2
      - run: sudo apt-get update && sudo apt-get install -y libgtk-4-dev libadwaita-1-dev
      - run: cargo test --locked
      - run: cargo clippy --all-targets -- -D warnings
      - run: ./release/build-release.sh "$GITHUB_REF_NAME"
      - uses: actions/upload-artifact@<sha>
  publish:
    needs: build
    runs-on: ubuntu-24.04
    permissions:
      contents: write       # only this job can write
    steps:
      - uses: actions/download-artifact@<sha>
      - run: gh release create "$GITHUB_REF_NAME" ./*.tar.gz checksums.txt --verify-tag --notes-file ...
```

Rules for that workflow: no `pull_request_target`, no repository secrets,
`GITHUB_TOKEN` only in the publish job, every action pinned by SHA as in
`ci.yml`, and `build-release.sh` already refuses a tag that does not match
`Cargo.toml`. The binaries stay unsigned; the maintainer uploads the
`.minisig` files by hand with `gh release upload` after the release exists,
so no signing key ever reaches the CI.

### 4.5 The tarball install and uninstall test

Verified with a temporary `HOME` (and `XDG_DATA_HOME`, `XDG_CONFIG_HOME`,
`XDG_CACHE_HOME` pointed at empty temp dirs), on the tarball built by
`./release/build-release.sh v3.1.2`; the tarball and `checksums.txt` were
deleted from the repository afterwards:

- `install.sh` exits 0. It installs the binary, the `webdriver_login`
  helper, the assets, the icon and the desktop entry, and creates
  `~/Games` and the `~/.local/bin/corkytux` symlink.
- The PATH warning is printed: `Add to ~/.bashrc / ~/.zshrc: export
  PATH="$HOME/.local/bin:$PATH"`.
- `install.sh` ignores `XDG_*`: it writes under `$HOME/.local` only
  (`release/install.sh:14-17`). This is the documented behaviour, not a
  bug, but a user with a different `XDG_DATA_HOME` still gets `~/.local`.
- `uninstall.sh` (answering yes to every prompt) removes the install
  directory, the symlink, the desktop entry and the icon, and leaves
  `~/Games` in place.
- The maintainer's real installation was not touched: the whole run used
  the temporary `HOME`.

### 4.6 Plugin tarball checksums

Every plugin release should carry a `checksums.txt` next to the tarball:

```
cd /tmp/release && sha256sum minecraft-launcher-1.2.0.tar.gz > checksums.txt
gh release create minecraft-launcher-v1.2.0 \
  minecraft-launcher-1.2.0.tar.gz checksums.txt --title ... --notes ...
```

One asset per release, so one line per file. **The launcher does not verify
them yet**: `grep -n sha256 src/backend/plugins.rs` is empty and
`download_plugin` extracts the tarball without comparing a digest. That is
D05 in `PENDIENTES.md` and stays open after these merges. Until it lands,
`checksums.txt` is for users who download by hand and for the release audit.

### 4.7 Versions to bump before publishing

- Launcher: `Cargo.toml` 3.1.2 -> 3.2.0 in its own commit; `build-release.sh`
  refuses a tag that does not match the manifest.
- `dependency-installer`: `plugin.json` stays at 2.1.3 in PRs #1 and #4, but
  `dependency-installer-v2.1.3` is already published and the tag cannot be
  reused. Publishing those fixes needs 2.1.4 (tag
  `dependency-installer-v2.1.4`, asset `dependency-installer-2.1.4.tar.gz`).
  Without the bump the fixes never reach the registry.
- `minecraft-launcher` 1.2.0 and `heroic-store` 1.2.0 are already bumped in
  the branches and the local integration build reports those versions.

## 5. The `cargo fmt` commit

It goes last, after every code PR is in `main`, so the reformat diff never
mixes formatting with new code.

1. Branch `style/cargo-fmt` from the new `main`.
2. `cargo fmt --all` and one commit: `style: cargo fmt`.
3. Check the commit is formatting only: `git diff -w main` must be empty.
4. Add the commit SHA to `.git-blame-ignore-revs` (create the file if it
   does not exist) with a short comment, in its own commit:
   `chore: ignore the reformat commit in git blame`.
   From then on: `git blame --ignore-revs-file .git-blame-ignore-revs`.
5. In `.github/workflows/ci.yml`, remove `continue-on-error: true` from the
   `cargo fmt --check` step: from that moment the step blocks the merge.
6. The step prints `hunks=0 files=0` when the tree is clean.

The same idea applies to the plugins repo with ruff, but there the tool is
ruff and the ignored findings are the 58 of the entrypoints.

## 6. The `CHANGELOG.md` task

It belongs to the preparation of the next release, not to this plan. The
draft lives in `docs/changelog-unreleased` (PR #13) and is not merged:

- `CHANGELOG.md` with an `## [Unreleased]` section at the top, grouped as
  Security, Added, Changed and Fixed, with the pull request number on the
  entries that have one.
- The header names the planned version (3.2.0) and the plugin releases that
  ship with it.
- The fixes that this plan closes are not copied into `PENDIENTES.md`: they
  live in the git history and in the changelog. `PENDIENTES.md` only keeps
  what is still open.
- The release notes for the Ely.by sign-in have to say that it needs the
  new `minecraft-launcher` plugin (1.2.0) together with the new launcher
  (see section 4.3).

## 7. After everything is merged

- `main` of both repositories has the CI (the launcher) and the lint
  (the plugins).
- The next PR in either repository runs the full gate: format, clippy, tests
  and build for the launcher; ruff, shellcheck and manifest JSON for the
  plugins.
- The protocol document (`docs/PLUGIN_PROTOCOL.md`, PR #1) describes the
  target state; the runtime still serves the v0 adapter until the protocol
  migration lands.
