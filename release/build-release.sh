#!/usr/bin/env bash
set -euo pipefail

# ─── CorkyTux Release Builder ────────────────────────────────────
# Builds the Qt binary, packages a release tarball.

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
cd "$SCRIPT_DIR/.."

VERSION=$(grep -oP 'project\(corkytux VERSION \K[0-9.]+' CMakeLists.txt)
ARCH=$(uname -m)
TARBALL="corkytux-${VERSION}-linux-${ARCH}.tar.gz"

echo ""
echo "=== Building CorkyTux v${VERSION} ==="
echo ""

# Build
cmake -S . -B build -DCMAKE_BUILD_TYPE=Release 2>&1
cmake --build build -j"$(nproc)" 2>&1

if [[ ! -f build/corkytux ]]; then
  echo "Build failed: build/corkytux not found" >&2
  exit 1
fi

echo ""
echo "=== Packaging release tarball ==="
echo ""

# Prepare release dir
rm -rf release/corkytux release/corkytux.jar
cp build/corkytux release/corkytux
chmod +x release/corkytux

# Create tarball
tar czf "$TARBALL" -C release \
  corkytux \
  install.sh \
  uninstall.sh \
  corkytux.png

echo "Created: ${TARBALL}"
echo "Size: $(du -h "$TARBALL" | cut -f1)"
echo ""
echo "Contents:"
tar tzf "$TARBALL"
echo ""
echo "Install: tar xzf ${TARBALL} && cd corkytux-* && ./install.sh"
echo ""
