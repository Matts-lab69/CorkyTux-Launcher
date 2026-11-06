#!/usr/bin/env bash
set -euo pipefail

INSTALL_DIR="${HOME}/.local/share/corkytux"
DATA_DIR="${HOME}/.local/share/CorkyTux"
CONFIG_DIR="${HOME}/.config/CorkyTux"
BIN_DIR="${HOME}/.local/bin"
ICON_DIR="${HOME}/.local/share/icons"
ICON_THEME_DIR="${ICON_DIR}/hicolor"
DESKTOP_DIR="${HOME}/.local/share/applications"

echo ""
echo "=== CorkyTux Uninstaller ==="
echo ""

if [[ "${EUID}" -eq 0 ]]; then
  echo "Do NOT run with sudo. Run as your user: ./uninstall.sh"
  exit 1
fi

removed=0
rm_path() { # $1=path $2=label [$3=dir?]
  if [[ "${3:-}" == "dir" && -d "$1" ]]; then rm -rf "$1"; echo "  Removed $2"; removed=$((removed+1));
  elif [[ "${3:-}" != "dir" && ( -e "$1" || -L "$1" ) ]]; then rm -f "$1"; echo "  Removed $2"; removed=$((removed+1));
  else echo "  (not present) $2"; fi
}
# Only remove plugin code files this installer recorded, not unrelated plugins
# or runtime state created later. Run before removing the installation manifest.
if [[ -f "${INSTALL_DIR}/plugins-installed.txt" && -f "${INSTALL_DIR}/plugins-install-root.txt" ]]; then
  PLUGINS_DIR="$(cat "${INSTALL_DIR}/plugins-install-root.txt")"
  [[ "$PLUGINS_DIR" == /*/CorkyTux/plugins && "/$PLUGINS_DIR/" != */../* && ! -L "$PLUGINS_DIR" ]] || { echo "Unsafe installed plugin root; refusing removal"; exit 1; }
  while IFS= read -r rel; do
    [[ "$rel" != /* && "$rel" == */* && "/$rel/" != */../* && "/$rel/" != */./* ]] || { echo "Unsafe plugin manifest; refusing removal"; exit 1; }
    parent="${rel%/*}"
    check_parent="$parent"
    while :; do
      [[ ! -L "${PLUGINS_DIR}/$check_parent" ]] || { echo "Symlinked plugin directory; refusing removal"; exit 1; }
      [[ "$check_parent" == */* ]] || break
      check_parent="${check_parent%/*}"
    done
    rm_path "${PLUGINS_DIR}/$rel" "bundled plugin file"
    while [[ "$parent" == */* ]]; do
      rmdir "${PLUGINS_DIR}/$parent" 2>/dev/null || break
      parent="${parent%/*}"
    done
    rmdir "${PLUGINS_DIR}/$parent" 2>/dev/null || true
  done < "${INSTALL_DIR}/plugins-installed.txt"
  rmdir "$PLUGINS_DIR" 2>/dev/null || true
fi
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
rm_path "${ICON_THEME_DIR}/128x128/apps/corkytux.png" "icon"
rm_path "${ICON_DIR}/corkytux.png" "legacy icon"

command -v update-desktop-database &>/dev/null && update-desktop-database "$DESKTOP_DIR" 2>/dev/null || true
command -v gtk-update-icon-cache &>/dev/null && gtk-update-icon-cache -f -t "$ICON_THEME_DIR" 2>/dev/null || true

echo ""
[[ $removed -gt 0 ]] && echo "=== CorkyTux uninstalled ===" || echo "=== Nothing to remove ==="
echo ""
