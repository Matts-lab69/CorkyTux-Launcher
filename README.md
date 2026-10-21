# CorkyTux

CorkyTux is a Linux game launcher written in Rust with GTK4 and
libadwaita. It keeps a single game library and launches Windows titles
through Proton/Wine and umu. Optional plugins add Minecraft, Epic/GOG
stores, emulators, AppImages, RPG Maker runtimes, dependency tools, and
other integrations; plugins are installed separately from the launcher.

CorkyTux began as a C++/Qt/QML application; the current launcher is the
Rust/GTK4 codebase in this repository.

## Requirements and build from source

Build requirements:

- A Rust toolchain with Cargo (edition 2021).
- `pkg-config` and a C compiler.
- GTK 4.12 development files.
- libadwaita 1.5 development files.

`Cargo.toml` enables the GTK `v4_12` and libadwaita `v1_5` features. At
runtime, `install.sh` checks for `libgtk-4.so.1`, `libadwaita-1.so.0`,
`libpango-1.0.so.0`, `libcairo.so.2`, `libgdk_pixbuf-2.0.so.0`, and
`libglib-2.0.so.0`.

On Debian/Ubuntu, the build packages are `pkg-config`,
`build-essential`, `libgtk-4-dev`, and `libadwaita-1-dev`. Package names
for other distributions are listed in [docs/BUILD.md](docs/BUILD.md).

Build both binaries:

```bash
git clone https://github.com/Matts-lab69/CorkyTux-Launcher.git
cd CorkyTux-Launcher
cargo build --release --bins
./target/release/corkytux
```

The build produces `target/release/corkytux` and
`target/release/webdriver_login`. The second binary handles the
Epic/GOG WebDriver login used by the store plugin.

## Installation

Install from a release tarball. Do not use `sudo`; `install.sh` writes
only under your home directory.

```bash
tar -xzf corkytux-3.1.2-linux-x86_64.tar.gz
cd corkytux-3.1.2
./install.sh
```

The release script names the tarball from the `Cargo.toml` version and
`uname -m`. `install.sh` writes:

- `~/.local/share/corkytux/corkytux` (launcher binary)
- `~/.local/share/corkytux/webdriver_login` (store login helper)
- `~/.local/share/corkytux/assets/` (bundled UI assets)
- `~/.local/bin/corkytux` (symlink)
- `~/.local/share/icons/corkytux.png` (icon)
- `~/.local/share/applications/corkytux.desktop` (desktop entry)

If `~/.local/bin` is not on `PATH`, add it as `install.sh` suggests.

At runtime CorkyTux uses:

- `~/.config/CorkyTux/Launcher.ini` and `~/.config/CorkyTux/Games.ini`
- `~/.config/CorkyTux/banners/` and `~/.config/CorkyTux/icons/`
- `~/.local/share/CorkyTux/plugins/` (installed plugins)
- `~/.local/share/CorkyTux/prefixes/` (game prefixes)
- `~/.local/share/CorkyTux/downloads/` (self-update staging)
- `~/.cache/CorkyTux/` (icon cache)

Run `./uninstall.sh` from the extracted tarball to remove the installed
files. The uninstaller asks before deleting game and plugin data, the
cache, or the configuration.

## Self-update

Self-update is opt-in. The startup check is controlled by
**Settings > Misc > Check for updates on startup**, which is off by
default. Manual checks are available from **Settings > About > Check for
launcher updates**. After an update installs and waits for a restart,
the same button becomes **Restart to finish updating**.

Updates come from GitHub Releases
(`Matts-lab69/CorkyTux-Launcher`). The updater:

- Uses HTTPS only and follows redirects only to `github.com`,
  `objects.githubusercontent.com`, and
  `release-assets.githubusercontent.com`. Release metadata comes from
  `api.github.com` and `github.com`.
- Requires the SHA256 digest published by the GitHub release API. A
  release without a digest is refused. If the release also carries
  `checksums.txt`, its entry for the tarball must match the API digest.
- Extracts only the expected files, with a 200 MiB total limit, a 50 MiB
  per-file limit, and a 10,000-entry limit. Absolute paths, `..`,
  symlinks, hardlinks, and unexpected entries are rejected.
- Checks that the new binary answers `--version` with the release
  version before replacing anything.
- Backs up every file it replaces and restores the previous binary,
  helper, icon, and assets if any install step fails. Backups are pruned
  only after a successful install.
- Quits the running instance and starts the new binary through a
  detached sleeper that waits for the old process to exit.

Self-update only works for copies installed by `install.sh` under
`~/.local/share/corkytux`. A build run from the repository or another
directory reports that it was not installed with `install.sh` and must
be updated manually. The startup check runs at most once every six
hours.

## --version

```bash
corkytux --version
```

prints `corkytux X.Y.Z` and exits without opening a window. The flag is
recognized only when it is the sole argument; any other combination
falls through to the normal command-line handling.

## Release

Build a release tarball with the release script. The tag is mandatory and
must match the version in `Cargo.toml`:

```bash
./release/build-release.sh v3.1.2
```

The script runs `cargo build --release --bins`, packages
`corkytux-3.1.2-linux-<arch>.tar.gz` with the launcher, the login
helper, `install.sh`, `uninstall.sh`, the icon, and the assets, and
writes `checksums.txt` with the tarball's SHA256. Upload both the
tarball and `checksums.txt` to the GitHub release.

## License

CorkyTux is licensed under the GNU Affero General Public License v3.0.
See [LICENSE](LICENSE).
