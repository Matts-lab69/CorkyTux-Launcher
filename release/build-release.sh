#!/usr/bin/env bash
set -euo pipefail
# ─── CorkyTux Release Builder v3.0.0 (Rust) ─────────────────────
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
cp "$BIN" "$STAGE/corkytux"
cp release/install.sh release/uninstall.sh "$STAGE/"
chmod +x "$STAGE/corkytux" "$STAGE/install.sh" "$STAGE/uninstall.sh"
cp release/corkytux.png "$STAGE/" 2>/dev/null || cp assets/corkytux.png "$STAGE/corkytux.png"
cp release/corkytux.desktop.in "$STAGE/" 2>/dev/null || true
tar czf "$TARBALL" -C "$STAGE" .
rm -rf "$STAGE"

echo "Created: ${TARBALL} ($(du -h "$TARBALL" | cut -f1))"
tar tzf "$TARBALL"
echo ""
echo "Install: tar xzf ${TARBALL} && ./install.sh"
