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

The lint workflow lives in PR #3, so the other plugin PRs only get CI once
it is in `main`. That is why #3 goes first.

| Order | PR | Branch | Why here | Rebase after |
|---|---|---|---|---|
| 1 | #3 | `docs/plugin-docs-and-lint` | It adds the lint workflow and the ruff config that covers the entrypoints. | #2 and #4 carry a cherry-pick of the lint fixes: drop that commit when rebasing. #1 does not touch those files. |
| 2 | #1 | `fix/dotnet-mono-gate-all-verbs` | Pre-existing PR, same file as #4. The merge is clean, but #4 is easier to review on top of it. | #4 (same file: `dependency-installer/dependency-installer`). |
| 3 | #4 | `fix/dependency-installer-verbs` | It carries `e001905` out of the local `main`. | After the merge, reset the local `main`: `git branch -f main origin/main`. The commit is already in `main` through the PR. |
| 4 | #2 | `fix/plugin-secrets` | Independent of the rest (Minecraft and Heroic files). | Nothing. |

After the plugins are in `main`:

- The ruff step in `.github/workflows/lint.yml` is `continue-on-error` while
  the entrypoints carry 58 findings. A dedicated commit
  (`style: entrypoint lint`, one commit per plugin file) fixes them, and the
  step goes back to blocking. The findings are listed in PR #3.
- The local `main` of the plugins repo still points at `e001905` until it is
  reset (see the table).

## 2. CorkyTux-Launcher

| Order | PR | Branch | Why here | Rebase after |
|---|---|---|---|---|
| 1 | #3 | `fix/plugin-runtime-safety` | It is the base of #5 and it carries the environment allowlist and the per-command timeouts. | #5: retarget from `fix/plugin-runtime-safety` to `main` (it is stacked). #6 and #7 merge clean, but rebasing keeps the history linear. |
| 2 | #5 | `fix/plugin-process-group` | It needs the timeouts from #3. | Nothing. |
| 3 | #6 | `fix/launcher-secret-logs` | It shares `plugins.rs` and `proton.rs` with #3 and #7. All the merges are clean. | Nothing. |
| 4 | #7 | `test/hermetic-ui-tests` | The two tests that needed a display and an installed plugin. Its commits are already merged into #4. | If #4 is merged first, #7 becomes empty and is closed instead of merged. |
| 5 | #4 | `ci/launcher-workflow` | It needs the hermetic tests (they are merged in) and it is the workflow that runs on every later PR. | Nothing. |
| 6 | #1, #2 | `docs/plugin-protocol-v1`, `docs/roadmap` | Documentation only, no dependency with anything. | Nothing. |

## 3. Merge strategy

A PR that stands on its own is **squashed**: `main` gets one commit that
says what the change does, and the branch's own commits stay in the PR for
the review. A PR that is the **base of a stack** is merged with a **merge
commit**: squashing it would move the base of the children and force a
rebase of every branch stacked on it. The leaves of a stack are merged with
a merge commit too, once their branch is retargeted to `main`.

| Repository | PR | Strategy | Why |
|---|---|---|---|
| Launcher | #1, #2, #6 | squash | Standalone: protocol document, roadmap, secret logs. |
| Launcher | #3 | **merge commit** | Base of the stack #5 (process group) and #8 (Ely.by stdin). |
| Launcher | #5, #8 | merge commit, after retargeting to `main` | They are the leaves of the #3 stack. |
| Launcher | #4 | squash | Standalone: the CI workflow, the hermetic tests merged into it and the two planning documents. One unit of work. |
| Launcher | #7 | **closed without merging** | Its commits are already inside #4. Once #4 is in `main`, #7 has nothing left to merge: close it with a comment pointing at #4. |
| Plugins | #1, #4 | squash | Standalone: the Mono gate and the dependency installer verbs. |
| Plugins | #3 | **merge commit** | Base of the stack #6 (entrypoint lint). |
| Plugins | #6 | merge commit, after retargeting to `main` | Leaf of the #3 stack. |
| Plugins | #2 | **merge commit** | Base of the stack #5 (Ely.by stdin). |
| Plugins | #5 | merge commit, after retargeting to `main` | Leaf of the #2 stack. |

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
