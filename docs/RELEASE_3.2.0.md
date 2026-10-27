# Release 3.2.0 — preparation notes

Status: **not released**. No tag `v3.2.0` and no GitHub release exist yet.
The dry-run artifact below is the candidate payload.

## Source

- Launcher SHA: `0a7e4deb1df0a742882efcaed0eb9283b68d38c8` (`origin/main`).
- Run: `gh run download 40159908153` (workflow `release`, event
  `workflow_dispatch`, conclusion `success`, 2026-10-25).
- Artifact: `corkytux-v3.2.0/corkytux-3.2.0-linux-x86_64.tar.gz` plus
  `checksums.txt`.

## Verification

- `tar tzf`: top-level `corkytux-3.2.0/` with `corkytux`, `webdriver_login`,
  `install.sh`, `uninstall.sh`, `corkytux.png`, `corkytux.desktop.in` and
  `assets/`. Result: **pass**.
- `sha256sum -c checksums.txt`: **pass** (sum matches).
- `objdump -T` glibc floor of the CI binary: `GLIBC_2.39` (highest symbol
  version). Matches the documented runtime floor (Ubuntu 24.04).
- Cargo.toml version at the SHA: 3.2.0.

## Plugin tarballs (rebuilt from CorkyTux-Plugins @ 8eeb6d6)

- dependency-installer 2.1.4: `dist/dependency-installer/...tar.gz`,
  sha256 `62c666...`, `plugin.json` version 2.1.4. tar tzf + sha256sum -c pass.
- heroic-store 1.2.0: `dist/heroic-store/...tar.gz`,
  sha256 `d5bfdf...`, `plugin.json` version 1.2.0. pass.
- minecraft-launcher 1.2.0: `dist/minecraft-launcher/...tar.gz`,
  sha256 `3016db...`, `plugin.json` version 1.2.0. pass. Entrypoint contains
  the `_ProgressThrottle` coalescing (#11) and the cfpack temp-dir cleanup
  on download failure (#17).

## Tag `plugins-v1.2.0`

Annotated tag created 2026-09-04 by Matts-lab69, pointing at commit
`c02e1c80a86545619f9acb3fa2429b07ee526407`
(`feat: Dependency Installer plugin v1.0.0`). Name and target disagree
(`plugins-v1.2.0` vs a v1.0.0 commit). Recommendation: leave it untouched
(historical artifact); do not delete from this flow. Clean up manually if
desired.
