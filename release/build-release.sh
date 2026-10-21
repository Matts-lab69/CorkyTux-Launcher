#!/usr/bin/env bash
set -euo pipefail
# ─── CorkyTux Release Builder v3.0.13 (Rust) ─────────────────────
SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
cd "$SCRIPT_DIR/.."

VERSION=$(grep -m1 '^version' Cargo.toml | grep -oE '[0-9]+\.[0-9]+\.[0-9]+')
ARCH=$(uname -m)
TARBALL="corkytux-${VERSION}-linux-${ARCH}.tar.gz"

# Tag argument is mandatory: a release can never ship an untagged
# version. Fails when it does not match Cargo.toml.
# The self-updater requires tag vX.Y.Z == Cargo version.
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

# El login de las tiendas lo hace este binario aparte, que habla WebDriver y
# por eso es async. El launcher lo busca a su lado, asi que tiene que viajar
# en el mismo tarball.
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
