#!/usr/bin/env bash
set -euo pipefail

INSTALL_DIR="${HOME}/.local/share/corkytux"
DATA_DIR="${HOME}/.local/share/CorkyTux"
CONFIG_DIR="${HOME}/.config/CorkyTux"
BIN_DIR="${HOME}/.local/bin"

echo "=== CorkyTux — Uninstaller ==="
echo ""

# Remove install dir
if [[ -d "$INSTALL_DIR" ]]; then
  rm -rf "$INSTALL_DIR"
  echo "  Removed ${INSTALL_DIR}"
fi

# Remove data (protons, prefixes, etc.)
if [[ -d "$DATA_DIR" ]]; then
  rm -rf "$DATA_DIR"
  echo "  Removed ${DATA_DIR}"
fi

# Remove config
if [[ -d "$CONFIG_DIR" ]]; then
  rm -rf "$CONFIG_DIR"
  echo "  Removed ${CONFIG_DIR}"
fi

# Remove symlink
rm -f "${BIN_DIR}/corkytux" 2>/dev/null

# Remove desktop entry
rm -f "${HOME}/.local/share/applications/corkytux.desktop" 2>/dev/null

# Remove icon
rm -f "${HOME}/.local/share/icons/corkytux.png" 2>/dev/null

echo ""
echo "=== Completely uninstalled ==="
