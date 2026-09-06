# Release tarball

A binary release tarball should contain the already-built Qt executable and the installer:

```text
corkytux-2.10.0-linux-x86_64/
|-- corkytux
|-- install.sh
|-- uninstall.sh
|-- README.md
`-- corkytux.png                 # optional desktop icon
```

The executable must be built from the repository with:

```bash
cmake -S . -B build -DCMAKE_BUILD_TYPE=Release
cmake --build build -j"$(nproc)"
cp build/corkytux release/corkytux
```

Install from the extracted directory:

```bash
./install.sh
```

The installer places the executable in `~/.local/share/corkytux/corkytux`, creates
`~/.local/bin/corkytux`, and writes a desktop entry in
`~/.local/share/applications/corkytux.desktop`.

The tarball does not contain `build/`, source files, CMake metadata, or Java files.
Qt runtime libraries and system dependencies are provided by the target Linux system.
