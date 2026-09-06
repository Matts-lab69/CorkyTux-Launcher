#!/usr/bin/env bash
set -euo pipefail

INSTALL_DIR="${HOME}/.local/share/corkytux"
DATA_DIR="${HOME}/.local/share/CorkyTux"
CONFIG_DIR="${HOME}/.config/CorkyTux"
BIN_DIR="${HOME}/.local/bin"
ICON_DIR="${HOME}/.local/share/icons"
DESKTOP_DIR="${HOME}/.local/share/applications"

echo ""
echo "=== CorkyTux Uninstaller ==="
echo ""

removed=0

if [[ -d "$INSTALL_DIR" ]]; then
  rm -rf "$INSTALL_DIR"
  echo "  Removed ${INSTALL_DIR}"
  ((removed++))
fi

if [[ -d "$DATA_DIR" ]]; then
  echo -n "  Remove game data (${DATA_DIR})? [y/N]: "
  read -r ans
  if [[ "$ans" =~ ^[Yy]$ ]]; then
    rm -rf "$DATA_DIR"
    echo "  Removed game data"
    ((removed++))
  else
    echo "  Skipped game data"
  fi
fi

if [[ -d "$CONFIG_DIR" ]]; then
  echo -n "  Remove config (${CONFIG_DIR})? [y/N]: "
  read -r ans
  if [[ "$ans" =~ ^[Yy]$ ]]; then
    rm -rf "$CONFIG_DIR"
    echo "  Removed config"
    ((removed++))
  else
    echo "  Skipped config"
  fi
fi

rm -f "${BIN_DIR}/corkytux" 2>/dev/null && echo "  Removed symlink" && ((removed++))
rm -f "${DESKTOP_DIR}/corkytux.desktop" 2>/dev/null && echo "  Removed desktop entry" && ((removed++))
rm -f "${ICON_DIR}/corkytux.png" 2>/dev/null && echo "  Removed icon" && ((removed++))

echo ""
if [[ $removed -gt 0 ]]; then
  echo "=== CorkyTux uninstalled ==="
else
  echo "=== Nothing to remove ==="
fi
echo ""
