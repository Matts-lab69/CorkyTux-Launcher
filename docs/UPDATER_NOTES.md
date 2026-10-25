# Updater Release Notes

Merge the updater redirect investigation/fix into `main` before making the
separate `Cargo.toml` version-bump commit for 3.2.0. Keep the version bump out
of the updater change.

The published `v3.1.2` tag predates the updater commit `dbfdeda`, so an
installed release binary from 3.1.2 cannot test self-update. If 3.2.0 ships the
updater, the first real release-to-release test is **3.2.0 → 3.2.1**.

## Redirect investigation

The reported failure is not reproduced by the checked binary source or by the
current anonymous GitHub response. `c948a61`, `dbfdeda`, and `origin/main`
have the same policy: metadata requests pass `API_HOSTS` (`api.github.com`,
`github.com`); artifact requests pass `ASSET_HOSTS` (`github.com`,
`objects.githubusercontent.com`, `release-assets.githubusercontent.com`).
Both the original URL and every redirect target are checked against the same
list supplied to that request. The limit is five hops and HTTPS/exact-host
checks apply to each target.

The message could name `api.github.com` if a caller accidentally validated the
metadata URL using `ASSET_HOSTS`, which excludes that host. The current
`api_get_latest` call site uses `API_HOSTS`, so this is a reproducible
misrouting scenario, not a demonstrated cause of the reported installation.
The live anonymous request returned HTTP 200 without a `Location` header.
Keep the exact host lists and hop limit unchanged unless a failing binary or
debug trace demonstrates a different path.

To capture a future attempt, set `CORKYTUX_DEBUG_UPDATER=1` before starting the
launcher. `[updater-debug]` records each URL, response status, redirect target,
exact allowlist and result. URL credentials and query/fragment values are
redacted. Share those lines together with the displayed error; do not share
tokens or environment values.