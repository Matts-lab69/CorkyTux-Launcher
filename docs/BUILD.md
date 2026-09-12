# Compilar CorkyTux v3.0.0 desde código fuente

Guía corta (Gentoo / Debian-Ubuntu / Fedora / Arch / openSUSE).

## 1. Dependencias de compilación

| Paquete | Gentoo | Debian/Ubuntu | Fedora/RHEL | Arch | openSUSE |
|---------|--------|---------------|-------------|------|----------|
| Rust + Cargo 1.70+ | `dev-lang/rust` | `cargo rustc` | `cargo rust` | `rust` | `cargo rust` |
| pkg-config + gcc | `dev-util/pkgconfig sys-devel/gcc` | `pkg-config build-essential` | `pkg-config gcc` (+ `base-devel` group) | `pkgconf base-devel` | `pkgconf patterns-devel-base` |
| GTK4 dev (4.12+) | `gui-libs/gtk:4` | `libgtk-4-dev` | `gtk4-devel` | `gtk4` | `gtk4-devel` |
| libadwaita dev (1.4+) | `gui-libs/libadwaita` | `libadwaita-1-dev` | `libadwaita-devel` | `libadwaita` | `libadwaita-devel` |

Verifica versiones instaladas:

```bash
pkg-config --modversion gtk4 libadwaita-1
rustc --version; cargo --version
```

## 2. Clonar y compilar

```bash
git clone https://github.com/Matts-lab69/CorkyTux-Launcher.git
cd CorkyTux-Launcher
cargo build --release
./target/release/corkytux
```

El binario release sale en `target/release/corkytux` (~6 MB, LTO + strip).

## 3. Crear tarball instalable

```bash
chmod +x release/build-release.sh
./release/build-release.sh
# genera: corkytux-3.0.0-linux-x86_64.tar.gz
# contiene: corkytux, install.sh, uninstall.sh, corkytux.png
tar tzf corkytux-*-linux-*.tar.gz
```

## 4. Instalar

```bash
tar -xzf corkytux-3.0.0-linux-x86_64.tar.gz
./install.sh
```

Sin `sudo`. Instala en `~/.local/share/corkytux/`, symlink `~/.local/bin/corkytux`,
icono + `.desktop`. Los plugins **no** van en este tarball: se instalan desde el
launcher (Settings > Plugins) o desde
[CorkyTux-Plugins](https://github.com/Matts-lab69/CorkyTux-Plugins).

## 5. Notas

- Runtime mínimo: `libgtk-4.so.1` + `libadwaita-1.so.0` (`install.sh` los verifica).
- Plugins Python: `python3` + `pip install --user requests minecraft_launcher_lib psutil`
  (solo el plugin de Minecraft los exige todos; el resto usa stdlib).
- Minecraft necesita Java 8/17/21/25 (el plugin los detecta en `~/jdk`, `/opt/jvm`,
  `/usr/lib/jvm`, `update-alternatives`).
- `umu-run` es opcional (el plugin heroic-store lo instala en `tools/umu` v1.4.4).
