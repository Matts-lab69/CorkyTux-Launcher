# Build CorkyTux v3.0.6 from source

Short guide (Gentoo / Debian-Ubuntu / Fedora / Arch / openSUSE).

## 1. Build dependencies

| Package | Gentoo | Debian/Ubuntu | Fedora/RHEL | Arch | openSUSE |
|---------|--------|---------------|-------------|------|----------|
| Rust + Cargo 1.70+ | `dev-lang/rust` | `cargo rustc` | `cargo rust` | `rust` | `cargo rust` |
| pkg-config + gcc | `dev-util/pkgconfig sys-devel/gcc` | `pkg-config build-essential` | `pkg-config gcc` (+ `base-devel` group) | `pkgconf base-devel` | `pkgconf patterns-devel-base` |
| GTK4 dev (4.12+) | `gui-libs/gtk:4` | `libgtk-4-dev` | `gtk4-devel` | `gtk4` | `gtk4-devel` |
| libadwaita dev (1.4+) | `gui-libs/libadwaita` | `libadwaita-1-dev` | `libadwaita-devel` | `libadwaita` | `libadwaita-devel` |

Check installed versions:

```bash
pkg-config --modversion gtk4 libadwaita-1
rustc --version; cargo --version
```

## 2. Clone and build

```bash
git clone https://github.com/Matts-lab69/CorkyTux-Launcher.git
cd CorkyTux-Launcher
cargo build --release
./target/release/corkytux
```

The release binary lands in `target/release/corkytux` (~6 MB, LTO + strip).

## 3. Create the installable tarball

```bash
chmod +x release/build-release.sh
./release/build-release.sh
# generates: corkytux-3.0.6-linux-x86_64.tar.gz
# contains a corkytux-VERSION/ folder with: corkytux, install.sh, uninstall.sh, corkytux.png
tar tzf corkytux-*-linux-*.tar.gz
```

## 4. Install

```bash
tar -xzf corkytux-3.0.6-linux-x86_64.tar.gz
cd corkytux-3.0.6 && ./install.sh
```

No `sudo`. It installs to `~/.local/share/corkytux/`, symlinks
`~/.local/bin/corkytux`, plus icon + `.desktop`. Plugins are **not** in this
tarball: install them from the launcher (Settings > Plugins) or from
[CorkyTux-Plugins](https://github.com/Matts-lab69/CorkyTux-Plugins).

## 5. Notes

- Minimum runtime: `libgtk-4.so.1` + `libadwaita-1.so.0` (`install.sh` checks them).
- Python plugins: `python3` + `pip install --user requests minecraft_launcher_lib psutil`
  (only the Minecraft plugin needs all of them; the rest use stdlib).
- Minecraft needs Java 8/17/21/25 (the plugin detects them in `~/jdk`, `/opt/jvm`,
  `/usr/lib/jvm`, `update-alternatives`).
- `umu-run` is optional (the heroic-store plugin installs it into `tools/umu` v1.4.4).
