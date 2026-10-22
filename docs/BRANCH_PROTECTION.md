# Branch protection for `main`

What to configure in the repository rulesets, and when, following the merge
order in `docs/MERGE_PLAN.md`. I do not touch the settings: this is the
checklist to apply by hand in Settings > Rules > Rulesets.

Two repositories, one ruleset each, on the `main` branch:

- `Matts-lab69/CorkyTux-Launcher`
- `Matts-lab69/CorkyTux-Plugins`

## The rules I recommend

| Rule | Launcher | Plugins | When to turn it on |
|---|---|---|---|
| Restrict direct pushes, allow merges | ON | ON | Now. `main` only moves through a PR. |
| Block force pushes | ON | ON | Now. Nothing in the plan needs a force push. |
| Allow deletions | OFF | OFF | Now. A deleted `main` is not recoverable by a PR. |
| Require a pull request before merging | ON | ON | Now. |
| Require conversation resolution before merging | ON | ON | Now. Cheap, and it keeps the review honest. |
| Require status checks to pass | `ci` | `lint` | **Only after** the CI PR (Launcher #4) and the lint PR (Plugins #3) are merged: until the check exists, the rule blocks every PR. |
| Require branches to be up to date before merging | ON | ON | With the status check above. Before that it only produces noise. |
| Required approving reviews | 0 now, 1 later | 0 now, 1 later | When a second person reviews. A rule of 1 would block a solo merge. |
| Require signed commits | OFF | OFF | When the signing key exists. Today there is none, and the minisign work in the protocol document is not implemented. |
| Require linear history | OFF | OFF | The plan merges PRs with merge commits. |
| Restrict who can bypass | Owner only | Owner only | Now. The bypass is audited in the audit log. |

The bypass list matters: the owner can still push to `main` in an
emergency (a broken release, a security fix), and every bypass is recorded.
"Do not allow bypassing the above settings" stays OFF on purpose.

## Why the status checks wait

A required check that does not exist yet blocks every merge, including the
one that would create it. The order is therefore:

1. Merge the plugins lint PR (#3). The `lint` job now runs on every PR of
   that repository.
2. Merge the launcher CI PR (#4). The `ci` job now runs on every PR of that
   repository.
3. Only then, create or edit the ruleset of each repository and require the
   corresponding check.

Before step 3 the protection is already useful: no direct pushes, no force
pushes, no deletions, and every change reviewed as a PR.

## What the checks mean

- Launcher `ci`: format (reporting only for now), clippy without
  `-D warnings`, tests and a release build, on `ubuntu-24.04` with GTK 4.12+
  and libadwaita 1.5+ proven with `pkg-config`.
- Plugins `lint`: ruff, shellcheck and the manifest JSON. The ruff step is
  `continue-on-error` until the entrypoint findings are closed, so today it
  reports instead of blocking. When it becomes blocking, the ruleset starts
  to enforce it.

## After everything is merged

- The `style: cargo fmt` commit lands (see `docs/MERGE_PLAN.md`) and the
  `cargo fmt --check` step stops being `continue-on-error`. From that
  moment the format is part of the gate.
- If the entrypoint lint findings are closed, the plugins ruff step stops
  being `continue-on-error` too.
- When the protocol v1 migration lands (the runtime still serves the v0
  adapter today), the schema check can join the plugins ruleset as a second
  required check.
