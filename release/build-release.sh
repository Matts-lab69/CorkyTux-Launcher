#!/usr/bin/env bash
set -euo pipefail
# ─── CorkyTux Release Builder (Rust) ────────────────────────────
SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
cd "$SCRIPT_DIR/.."

VERSION=$(grep -m1 '^version' Cargo.toml | grep -oE '[0-9]+\.[0-9]+\.[0-9]+')
ARCH=$(uname -m)
case "$ARCH" in
  amd64) ARCH=x86_64 ;;
  arm64) ARCH=aarch64 ;;
  x86_64|aarch64) ;;
  *) echo "Unsupported architecture: $ARCH" >&2; exit 1 ;;
esac
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
# An already exported CORKYTUX_SGDB_BUILTIN_KEY is inherited by cargo and
# embedded by option_env! in src/backend/integration.rs. Absent means a normal
# build without the fallback. Never echo, log, persist or export the value here.
cargo build --release --locked --bins

TARGET_DIR="${CARGO_TARGET_DIR:-target}"
BIN="${TARGET_DIR}/release/corkytux"
[[ -f "$BIN" ]] || { echo "Build failed: $BIN not found" >&2; exit 1; }
ls -lh "$BIN"

# Store login runs in this separate binary, which speaks WebDriver and
# is therefore async. The launcher looks for it next to itself, so it
# has to travel in the same tarball.
HELPER="${TARGET_DIR}/release/webdriver_login"
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
cp release/corkytux.desktop.in "$PKGDIR/com.corkytux.CorkyTux.desktop"
cp LICENSE README.md CREDITS.md "$PKGDIR/"
cp docs/THIRD-PARTY.md "$PKGDIR/THIRD-PARTY.md"
cp -r assets "$PKGDIR/assets"

# Opt-in local plugin checkout for the combined release. Never download from
# another repository or copy untracked runtime/configuration files.
if [[ -n "${CORKYTUX_PLUGINS_SOURCE:-}" ]]; then
  PLUGINS_SOURCE="$(cd "$CORKYTUX_PLUGINS_SOURCE" && pwd)"
  git -C "$PLUGINS_SOURCE" diff --quiet
  git -C "$PLUGINS_SOURCE" diff --cached --quiet
  mkdir -p "$PKGDIR/plugins"
  cp "$PLUGINS_SOURCE/LICENSE" "$PKGDIR/plugins/LICENSE"
  cp "$PLUGINS_SOURCE/README.md" "$PKGDIR/plugins/README.md"
  for meta in "$PLUGINS_SOURCE"/*/plugin.json; do
    [[ -f "$meta" ]] || continue
    plugin_id="${meta%/plugin.json}"
    plugin_id="${plugin_id##*/}"
    while IFS= read -r -d '' rel; do
      [[ "$rel" != */tests/* && "$rel" != */__pycache__/* ]] || continue
      [[ ! -L "$PLUGINS_SOURCE/$rel" ]] || { echo "Plugin symlink requires review" >&2; exit 1; }
      mkdir -p "$PKGDIR/plugins/${rel%/*}"
      cp -p "$PLUGINS_SOURCE/$rel" "$PKGDIR/plugins/$rel"
    done < <(git -C "$PLUGINS_SOURCE" ls-files -z -- "$plugin_id")
  done
fi
tar czf "$TARBALL" -C "$STAGE" "corkytux-${VERSION}"
rm -rf "$STAGE"

echo "Created: ${TARBALL} ($(du -h "$TARBALL" | cut -f1))"
tar tzf "$TARBALL"
echo ""

# checksums.txt travels next to the tarball: the self-updater requires
# the API digest, and when this file is attached it must agree too.
CHECKSUMS="checksums.txt"
sha256sum "$TARBALL" > "$CHECKSUMS"
cp "$CHECKSUMS" SHA256SUMS
echo "Checksums (${CHECKSUMS}):"
cat "$CHECKSUMS"
echo ""
echo "Upload BOTH ${TARBALL} and ${CHECKSUMS} to the GitHub release."
echo ""
echo "Install: tar xzf ${TARBALL} && cd corkytux-${VERSION} && ./install.sh"
