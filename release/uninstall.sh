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

if [[ "${EUID}" -eq 0 ]]; then
  echo "Do NOT run with sudo. Run as your user: ./uninstall.sh"
  exit 1
fi

removed=0
rm_path() { # $1=path $2=label [$3=dir?]: only reports when something existed
  if [[ "${3:-}" == "dir" && -d "$1" ]]; then rm -rf "$1"; echo "  Removed $2"; removed=$((removed+1));
  elif [[ "${3:-}" != "dir" && -e "$1" || -L "$1" ]]; then rm -f "$1"; echo "  Removed $2"; removed=$((removed+1));
  else echo "  (not present) $2"; fi
}
[[ -d "$INSTALL_DIR" ]] && { rm -rf "$INSTALL_DIR"; echo "  Removed ${INSTALL_DIR}"; removed=$((removed+1)); }

if [[ -d "$DATA_DIR" ]]; then
  read -rp "  Remove plugin/game data (${DATA_DIR})? Prefixes, saves and plugin runtimes live here. [y/N]: " ans
  if [[ "$ans" =~ ^[Yy]$ ]]; then rm -rf "$DATA_DIR"; echo "  Removed data"; removed=$((removed+1));
  else echo "  Skipped data"; fi
fi

CACHE_DIR="${HOME}/.cache/CorkyTux"
if [[ -d "$CACHE_DIR" ]]; then
  read -rp "  Remove icon/thumbnail cache (${CACHE_DIR})? Regenerates on its own. [y/N]: " ans
  if [[ "$ans" =~ ^[Yy]$ ]]; then rm -rf "$CACHE_DIR"; echo "  Removed cache"; removed=$((removed+1));
  else echo "  Skipped cache"; fi
fi

if [[ -d "$CONFIG_DIR" ]]; then
  read -rp "  Remove config (${CONFIG_DIR})? [y/N]: " ans
  if [[ "$ans" =~ ^[Yy]$ ]]; then rm -rf "$CONFIG_DIR"; echo "  Removed config"; removed=$((removed+1));
  else echo "  Skipped config"; fi
fi

rm_path "${BIN_DIR}/corkytux" "symlink"
rm_path "${DESKTOP_DIR}/corkytux.desktop" "desktop entry"
rm_path "${ICON_DIR}/corkytux.png" "icon"

command -v update-desktop-database &>/dev/null && update-desktop-database "$DESKTOP_DIR" 2>/dev/null || true

echo ""
[[ $removed -gt 0 ]] && echo "=== CorkyTux uninstalled ===" || echo "=== Nothing to remove ==="
echo ""
