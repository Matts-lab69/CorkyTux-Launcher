# Updater Release Notes

Merge the updater redirect investigation/fix into `main` before making the
separate `Cargo.toml` version-bump commit for 3.2.0. Keep the version bump out
of the updater change.

The published `v3.1.2` tag predates the updater commit `dbfdeda`, so an
installed release binary from 3.1.2 cannot test self-update. If 3.2.0 ships the
updater, the first real release-to-release test is **3.2.0 → 3.2.1**.