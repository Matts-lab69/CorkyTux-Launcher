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

**Release notes**: the entry has to say that the Ely.by sign-in needs the
new `minecraft-launcher` plugin (1.2.0) together with the new launcher, and
that the mismatched pairs refuse the sign-in on purpose.

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

It belongs to the preparation of the next release, not to this plan. When
the release is assembled:

- Create `CHANGELOG.md` with an `## [Unreleased]` section at the top.
- One line per change, grouped by area (runtime safety, secrets, protocol
  v1, CI, docs), each line with the pull request it came from.
- The fixes that this plan closes are not copied into `PENDIENTES.md`: they
  live in the git history and in the changelog. `PENDIENTES.md` only keeps
  what is still open.
- The release notes for the Ely.by sign-in have to say that it needs the
  new `minecraft-launcher` plugin (1.2.0) together with the new launcher
  (see the section above).

## 7. After everything is merged

- `main` of both repositories has the CI (the launcher) and the lint
  (the plugins).
- The next PR in either repository runs the full gate: format, clippy, tests
  and build for the launcher; ruff, shellcheck and manifest JSON for the
  plugins.
- The protocol document (`docs/PLUGIN_PROTOCOL.md`, PR #1) describes the
  target state; the runtime still serves the v0 adapter until the protocol
  migration lands.
