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

## 3. The `cargo fmt` commit

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

## 4. After everything is merged

- `main` of both repositories has the CI (the launcher) and the lint
  (the plugins).
- The next PR in either repository runs the full gate: format, clippy, tests
  and build for the launcher; ruff, shellcheck and manifest JSON for the
  plugins.
- The protocol document (`docs/PLUGIN_PROTOCOL.md`, PR #1) describes the
  target state; the runtime still serves the v0 adapter until the protocol
  migration lands.
