#!/usr/bin/env bash
set -euo pipefail

INSTALL_DIR="${HOME}/.local/share/corkytux"
DATA_DIR="${HOME}/.local/share/CorkyTux"
CONFIG_DIR="${HOME}/.config/CorkyTux"
BIN_DIR="${HOME}/.local/bin"
ICON_DIR="${HOME}/.local/share/icons"
DESKTOP_DIR="${HOME}/.local/share/applications"

echo ""
echo "=== CorkyTux Uninstaller v3.0.11 ==="
echo ""

if [[ "${EUID}" -eq 0 ]]; then
  echo "Do NOT run with sudo. Run as your user: ./uninstall.sh"
  exit 1
fi

removed=0
[[ -d "$INSTALL_DIR" ]] && { rm -rf "$INSTALL_DIR"; echo "  Removed ${INSTALL_DIR}"; removed=$((removed+1)); }

if [[ -d "$DATA_DIR" ]]; then
  read -rp "  Remove plugin/game data (${DATA_DIR})? [y/N]: " ans
  if [[ "$ans" =~ ^[Yy]$ ]]; then rm -rf "$DATA_DIR"; echo "  Removed data"; removed=$((removed+1));
  else echo "  Skipped data"; fi
fi

if [[ -d "$CONFIG_DIR" ]]; then
  read -rp "  Remove config (${CONFIG_DIR})? [y/N]: " ans
  if [[ "$ans" =~ ^[Yy]$ ]]; then rm -rf "$CONFIG_DIR"; echo "  Removed config"; removed=$((removed+1));
  else echo "  Skipped config"; fi
fi

rm -f "${BIN_DIR}/corkytux" && echo "  Removed symlink" && removed=$((removed+1))
rm -f "${DESKTOP_DIR}/corkytux.desktop" && echo "  Removed desktop entry" && removed=$((removed+1))
rm -f "${ICON_DIR}/corkytux.png" && echo "  Removed icon" && removed=$((removed+1))

command -v update-desktop-database &>/dev/null && update-desktop-database "$DESKTOP_DIR" 2>/dev/null || true

echo ""
[[ $removed -gt 0 ]] && echo "=== CorkyTux uninstalled ===" || echo "=== Nothing to remove ==="
echo ""
