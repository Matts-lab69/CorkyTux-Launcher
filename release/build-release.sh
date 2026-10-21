#!/usr/bin/env bash
set -euo pipefail
# ─── CorkyTux Release Builder (Rust) ────────────────────────────
SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
cd "$SCRIPT_DIR/.."

VERSION=$(grep -m1 '^version' Cargo.toml | grep -oE '[0-9]+\.[0-9]+\.[0-9]+')
ARCH=$(uname -m)
TARBALL="corkytux-${VERSION}-linux-${ARCH}.tar.gz"

# Version argument is mandatory and must match Cargo.toml.
# This script does not verify that the git tag exists.
# The self-updater requires the published tag vX.Y.Z == Cargo version.
if [[ -z "${1:-}" ]]; then
  echo "Usage: ./build-release.sh vX.Y.Z (must match Cargo.toml)" >&2
  exit 1
fi
EXPECTED="${1#v}"
if [[ "$EXPECTED" != "$VERSION" ]]; then
  echo "Version mismatch: arg $1 vs Cargo.toml $VERSION" >&2
  exit 1
fi

echo "=== Building CorkyTux v${VERSION} (release) ==="
cargo build --release --bins

BIN="target/release/corkytux"
[[ -f "$BIN" ]] || { echo "Build failed: $BIN not found" >&2; exit 1; }
ls -lh "$BIN"

# Store login runs in this separate binary, which speaks WebDriver and
# is therefore async. The launcher looks for it next to itself, so it
# has to travel in the same tarball.
HELPER="target/release/webdriver_login"
[[ -f "$HELPER" ]] || { echo "Build failed: $HELPER not found" >&2; exit 1; }
ls -lh "$HELPER"

echo "=== Packaging ${TARBALL} ==="
STAGE="$(mktemp -d)"
PKGDIR="$STAGE/corkytux-${VERSION}"
mkdir -p "$PKGDIR"
cp "$BIN" "$PKGDIR/corkytux"
cp "$HELPER" "$PKGDIR/webdriver_login"
cp release/install.sh release/uninstall.sh "$PKGDIR/"
chmod +x "$PKGDIR/corkytux" "$PKGDIR/webdriver_login" "$PKGDIR/install.sh" "$PKGDIR/uninstall.sh"
cp release/corkytux.png "$PKGDIR/" 2>/dev/null || cp assets/corkytux.png "$PKGDIR/corkytux.png"
cp release/corkytux.desktop.in "$PKGDIR/" 2>/dev/null || true
cp -r assets "$PKGDIR/assets"
tar czf "$TARBALL" -C "$STAGE" "corkytux-${VERSION}"
rm -rf "$STAGE"

echo "Created: ${TARBALL} ($(du -h "$TARBALL" | cut -f1))"
tar tzf "$TARBALL"
echo ""

# checksums.txt travels next to the tarball: the self-updater requires
# the API digest, and when this file is attached it must agree too.
CHECKSUMS="checksums.txt"
sha256sum "$TARBALL" > "$CHECKSUMS"
echo "Checksums (${CHECKSUMS}):"
cat "$CHECKSUMS"
echo ""
echo "Upload BOTH ${TARBALL} and ${CHECKSUMS} to the GitHub release."
echo ""
echo "Install: tar xzf ${TARBALL} && cd corkytux-${VERSION} && ./install.sh"
