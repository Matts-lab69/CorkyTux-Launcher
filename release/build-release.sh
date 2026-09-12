#!/usr/bin/env bash
set -euo pipefail
# ─── CorkyTux Release Builder v3.0.2 (Rust) ─────────────────────
SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
cd "$SCRIPT_DIR/.."

VERSION=$(grep -m1 '^version' Cargo.toml | grep -oE '[0-9]+\.[0-9]+\.[0-9]+')
ARCH=$(uname -m)
TARBALL="corkytux-${VERSION}-linux-${ARCH}.tar.gz"

echo "=== Building CorkyTux v${VERSION} (release) ==="
cargo build --release

BIN="target/release/corkytux"
[[ -f "$BIN" ]] || { echo "Build failed: $BIN not found" >&2; exit 1; }
ls -lh "$BIN"

echo "=== Packaging ${TARBALL} ==="
STAGE="$(mktemp -d)"
PKGDIR="$STAGE/corkytux-${VERSION}"
mkdir -p "$PKGDIR"
cp "$BIN" "$PKGDIR/corkytux"
cp release/install.sh release/uninstall.sh "$PKGDIR/"
chmod +x "$PKGDIR/corkytux" "$PKGDIR/install.sh" "$PKGDIR/uninstall.sh"
cp release/corkytux.png "$PKGDIR/" 2>/dev/null || cp assets/corkytux.png "$PKGDIR/corkytux.png"
cp release/corkytux.desktop.in "$PKGDIR/" 2>/dev/null || true
cp -r assets "$PKGDIR/assets"
tar czf "$TARBALL" -C "$STAGE" "corkytux-${VERSION}"
rm -rf "$STAGE"

echo "Created: ${TARBALL} ($(du -h "$TARBALL" | cut -f1))"
tar tzf "$TARBALL"
echo ""
echo "Install: tar xzf ${TARBALL} && cd corkytux-${VERSION} && ./install.sh"
