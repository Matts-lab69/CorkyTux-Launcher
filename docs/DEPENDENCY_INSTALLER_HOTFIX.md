# Dependency Installer hotfix: .NET false positives

## What changed

`dependency-installer` gated Mono detection only for `dotnet48` and
`dotnet35sp1`. Unity/Mono games still triggered `dotnet472` / `dotnet40`
/ `dotnet20` because they expose `mscoree.dll` via string scanning, even
though Mono already handles those runtimes.

This caused:

1. false-positive "Missing dependencies" dialogs for games that do not
   need winetricks .NET,
2. `winetricks` failures when trying to install .NET into a prefix that
   already uses Mono,
3. broken prefixes when `remove_mono` ran but the .NET installer never
   finished.

## Fixes applied

- `src/backend/plugins.rs`

  - `dep_install_in` now wraps long installs with a larger timeout for
    any `dotnet*` verb (1800 s instead of the old no-timeout path), and
    surfaces per-dependency errors from the plugin JSON instead of the
    old generic "install failed" string.

- `vendor/patches/dependency-installer-dotnet-hotfix.patch`

  - Unified patch against the upstream `dependency-installer` plugin.

- `~/.local/share/CorkyTux/plugins/dependency-installer/dependency-installer`

  - Local copy already patched: Mono detection now covers
    `dotnet48`, `dotnet472`, `dotnet40`, `dotnet35sp1`, and `dotnet20`.

## How to reapply after plugin updates

The plugin binary is fetched from GitHub, not built from this repo.
After updating the plugin, reapply the patch locally:

```bash
PATCH="/home/mattsgaming/CorkyTux-Launcher/vendor/patches/dependency-installer-dotnet-hotfix.patch"
PLUGIN="/home/mattsgaming/.local/share/CorkyTux/plugins/dependency-installer/dependency-installer"

cp "$PLUGIN" "$PLUGIN.bak"
patch -p1 --directory "$(dirname "$PLUGIN")" < "$PATCH"
```

If the upstream plugin already contains the fix, `patch` will report
already-applied hunks and can be skipped.

## Recommended upstream fix

Open a PR/MR in the plugins repo to change the Mono gate to:

```python
if dep["id"] in ("dotnet48", "dotnet472", "dotnet40", "dotnet35sp1", "dotnet20"):
```

This removes the need for the local patch once merged.
