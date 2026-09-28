# AGENTS.md — CorkyTux-Launcher

## App execution

Do not run the app, background processes, captures, or GTK Inspector,
and do not build the binary (`cargo build` / `cargo run`) unless the
user literally says "compila y abre" (only exception). The user tests
the app. Automated UI interaction scripts are forbidden.

Autonomous exception: a session explicitly ordered as autonomous may
open the app and take its own screenshots to verify UI, logging time
and PID, restoring user config afterwards, and never touching games,
prefixes, saves, or registry files without timestamped `.bak` copies.

## Commits

- Diffs before commit, one commit per logical patch, conventional
  style, no emojis, subject ≤ 72 chars. Each commit builds and passes
  tests on its own. Never `push --force`, never rewrite history.
- Pending work from other sessions stays uncommitted until reviewed.

## Verification

- `cargo build` + `cargo test` after every change; plugin commands run
  live (read-only or with temp data in `/tmp`).
- New UI classes go in `DESIGN.md`; tone is terse and factual.
