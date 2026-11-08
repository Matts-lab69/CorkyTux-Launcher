# Build CorkyTux from source

Step-by-step guide for Debian/Ubuntu, Fedora/RHEL, Arch, openSUSE, and
Gentoo.

## 1. Prerequisites

| Distro | Rust | pkg-config | gcc | GTK4 dev | libadwaita dev |
|--------|------|------------|-----|----------|----------------|
| Debian/Ubuntu | `cargo rustc` | `pkg-config` | `build-essential` | `libgtk-4-dev` | `libadwaita-1-dev` |
| Fedora/RHEL | `cargo rust` | `pkg-config` | `gcc` (+ `base-devel`) | `gtk4-devel` | `libadwaita-devel` |
| Arch | `rust` | `pkgconf` | `base-devel` | `gtk4` | `libadwaita` |
| openSUSE | `cargo rust` | `pkgconf` | `patterns-devel-base` | `gtk4-devel` | `libadwaita-devel` |
| Gentoo | `dev-lang/rust` | `dev-util/pkgconfig` | `sys-devel/gcc` | `gui-libs/gtk:4` | `gui-libs/libadwaita` |

`Cargo.toml` enables the GTK `v4_12` and libadwaita `v1_5` features, so
GTK 4.12+ and libadwaita 1.5+ are required. `Cargo.toml` does not declare
`rust-version`; use a recent stable Rust toolchain (edition 2021).

## 2. Install Rust

If Rust is not installed, use rustup:

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source "$HOME/.cargo/env"
```

Check the toolchain:

```bash
rustc --version
cargo --version
```

## 3. Verify GTK and libadwaita

```bash
pkg-config --modversion gtk4 libadwaita-1
```

The output must be 4.12 or newer for `gtk4` and 1.5 or newer for
`libadwaita-1`. If a module is missing, install the development package
listed in the table above.

## 4. Clone and build

```bash
git clone https://github.com/Matts-lab69/CorkyTux-Launcher.git
cd CorkyTux-Launcher
cargo build --release --bins
```

The build produces:

- `target/release/corkytux`
- `target/release/webdriver_login`

`webdriver_login` is the Epic/GOG WebDriver login helper. The release
profile enables LTO and strip.

### Optional SteamGridDB fallback

The binary can embed a fallback SteamGridDB key for users who have none of
their own. It is read at compile time and is entirely optional:

```bash
cargo build --release --locked --bins
```

For a build with the shared fallback, supply `CORKYTUX_SGDB_BUILTIN_KEY` securely
in the exported environment before that command. Do not put its value in a
command, file, build flag or shell history.

Leave the variable unset for a normal build: the launcher still compiles and
simply skips SteamGridDB. `release/build-release.sh` inherits the variable when
it is already exported, and `.github/workflows/release.yml` passes the
`CORKYTUX_SGDB_BUILTIN_KEY` secret to the build step only. Never commit a key.
A compiled fallback can be extracted from the binary, so treat it as a shared
fallback rather than a secret. At runtime each user's
`CORKYTUX_STEAMGRIDDB_API_KEY`, `steamgriddb.key` file or `IntegrationApiKey`
setting wins over it.

## 5. Run without installing

```bash
./target/release/corkytux
```

## 6. Test

```bash
cargo test
```

## 7. Create the installable tarball

```bash
chmod +x release/build-release.sh
./release/build-release.sh vX.Y.Z
```

Replace `vX.Y.Z` with the `version` in `Cargo.toml`; the version argument
is mandatory and must match it. The script does not check that the git tag
exists. The script runs `cargo build --release --locked --bins` and
creates:

- `corkytux-<version>-linux-<arch>.tar.gz`
- `checksums.txt`
- `SHA256SUMS` (identical checksum list)

To include the plugins from a clean, local checkout without network downloads:

```bash
CORKYTUX_PLUGINS_SOURCE=/path/to/CorkyTux-Plugins ./release/build-release.sh v3.2.0
```

The exported `CORKYTUX_SGDB_BUILTIN_KEY`, if supplied securely, is inherited by
this command without being printed. The tarball has one root directory,
`corkytux-3.2.0/`, containing the binaries, assets, bundled plugins, installers,
desktop entry, icon, LICENSE, README, CREDITS and THIRD-PARTY documentation.
The packaged desktop entry is `com.corkytux.CorkyTux.desktop`; its `Icon` and
`StartupWMClass` match the GTK app ID. The installer places the matching PNG in
hicolor, using absolute `XDG_DATA_HOME` or `~/.local/share` for desktop/icons.
Only tracked plugin files are copied; test directories are omitted.

`./install.sh` installs bundled code into the launcher's plugin directory:
absolute `XDG_DATA_HOME` plus `/CorkyTux/plugins`, or
`~/.local/share/CorkyTux/plugins` otherwise. `./install.sh --no-plugins` leaves
plugins untouched. Uninstall removes only plugin code recorded by the
installer; unrelated plugins and runtime files are preserved unless the user
explicitly requests deletion of their data. No plugin dependencies are fetched.

For example, version `3.2.0` on x86_64 produces
`corkytux-3.2.0-linux-x86_64.tar.gz`. The architecture comes from `uname -m`
(`x86_64`, `aarch64`, ...). The tarball contains `corkytux`,
`webdriver_login`, `install.sh`, `uninstall.sh`, `corkytux.png`,
`corkytux.desktop.in`, and `assets/`.

Inspect the tarball:

```bash
VERSION=$(grep -m1 '^version' Cargo.toml | grep -oE '[0-9]+\.[0-9]+\.[0-9]+')
tar tzf "corkytux-${VERSION}-linux-$(uname -m).tar.gz"
```

## 8. Install

Download the generated tarball from
[Releases](https://github.com/Matts-lab69/CorkyTux-Launcher/releases), or
use the one built above, then run:

```bash
tar -xzf corkytux-*-linux-*.tar.gz
cd corkytux-*/
./install.sh
```

Do not use `sudo`. `install.sh` writes to `~/.local/`. It installs:

| File | Destination |
|------|-------------|
| `corkytux` | `~/.local/share/corkytux/corkytux` |
| `webdriver_login` | `~/.local/share/corkytux/webdriver_login` |
| `assets/` | `~/.local/share/corkytux/assets/` |
| `corkytux` (symlink) | `~/.local/bin/corkytux` |
| `corkytux.png` | `~/.local/share/icons/corkytux.png` |
| `corkytux.desktop` | `~/.local/share/applications/corkytux.desktop` |

If `~/.local/bin` is not on `PATH`, add:

```bash
export PATH="$HOME/.local/bin:$PATH"
```

Uninstall with `./uninstall.sh` from the extracted tarball.

## 9. Release workflow

1. Bump `version` in `Cargo.toml`.
2. Refresh `Cargo.lock` so it records the new version:

```bash
cargo check
```

`cargo check` refreshes the version entry in `Cargo.lock` without upgrading dependencies.

3. Commit `Cargo.toml` and `Cargo.lock` together.
4. Build the release and check the tarball:

```bash
./release/build-release.sh vX.Y.Z
VERSION=$(grep -m1 '^version' Cargo.toml | grep -oE '[0-9]+\.[0-9]+\.[0-9]+')
tar tzf "corkytux-${VERSION}-linux-$(uname -m).tar.gz"
```

5. Create and push the tag that matches the version:

```bash
git tag vX.Y.Z
git push origin vX.Y.Z
```

**Plugin pin:** the release build checks out
`Matts-lab69/CorkyTux-Plugins` at the commit in the `Checkout the plugins`
step of `.github/workflows/release.yml` (`ref:`) and bundles those files.
Before tagging, set that `ref` to the current `main` of CorkyTux-Plugins and
confirm both point at the same commit:

```bash
git ls-remote https://github.com/Matts-lab69/CorkyTux-Plugins refs/heads/main
grep -A3 'Checkout the plugins' .github/workflows/release.yml
```

Update the pin on every release; the CHANGELOG names the commit that
shipped. The build fails if the tarball does not contain the seven plugins.

6. Pushing the tag runs `.github/workflows/release.yml`, which builds the
   tarball, checks it with the in-app updater extractor and a full loopback
   install, and opens a **draft** release with the tarball, `checksums.txt` and
   `SHA256SUMS`. Review the draft, then publish it.

`vX.Y.Z` is the version from `Cargo.toml` (the tarball example in step 7
uses the same value). The self-updater requires the release tag to match the
`Cargo.toml` version, the asset name
`corkytux-<version>-linux-<arch>.tar.gz`, and that asset's SHA256 digest from
the release API. `checksums.txt` is optional and must match the digest when
present; `SHA256SUMS` is for manual verification and the updater does not read
it. The updater installs the binary, `webdriver_login`, the icon and `assets/`;
the bundled plugins are skipped and reported in the install log, so a
plugin-only change needs a reinstall.

## 10. Runtime notes

- Runtime libraries checked by `install.sh`: `libgtk-4.so.1` and `libadwaita-1.so.0`, plus pango, cairo, pixbuf, and glib.
- Python plugins: `python3` and `requests`; the Minecraft plugin also needs `minecraft_launcher_lib` and `psutil`.
- Minecraft needs Java 8/17/21/25; the plugin detects them in `~/jdk`, `/opt/jvm`, `/usr/lib/jvm`, and `update-alternatives`.
- `umu-run` is optional; the heroic-store plugin installs it into `tools/umu` v1.4.4.

## 11. Troubleshooting

| Symptom | Cause | Fix | Basis |
|---------|-------|-----|-------|
| `pkg-config` cannot find `gtk4` or `libadwaita-1` | Development packages are missing or `PKG_CONFIG_PATH` is incomplete | Install the GTK4 and libadwaita development packages for your distribution | Deduced |
| GTK or libadwaita is too old | Installed version is below 4.12 or 1.5 | Upgrade the distribution packages or build newer libraries | Deduced; minimum comes from `Cargo.toml` |
| Linker or `gcc` is missing | No C toolchain | Install `build-essential`, `gcc`, or `base-devel` | Deduced |
| `rustc` is too old | `Cargo.toml` does not declare `rust-version`; the crate uses edition 2021 | Run `rustup update stable`; no numeric minimum is declared | Deduced; missing `rust-version` is verified |
| `version argument does not match Cargo.toml` | The version argument differs from `Cargo.toml` | Pass the exact version from `Cargo.toml` | Verified by `release/build-release.sh` |
| `corkytux` is not in `PATH` | `~/.local/bin` is not on `PATH` | Add `export PATH="$HOME/.local/bin:$PATH"` to your shell profile | Verified by `release/install.sh` |
