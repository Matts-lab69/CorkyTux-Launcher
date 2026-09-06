#!/usr/bin/env bash
set -euo pipefail

# ─── CorkyTux Installer v2.10.0 ──────────────────────────────────
# Supports: Gentoo, Debian/Ubuntu, Fedora/RHEL, Arch, openSUSE
# Checks Qt6 runtime deps, installs binary, icon, desktop entry.

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
INSTALL_DIR="${HOME}/.local/share/corkytux"
BIN_DIR="${HOME}/.local/bin"
ICON_DIR="${HOME}/.local/share/icons"
DESKTOP_DIR="${HOME}/.local/share/applications"
VERSION="2.12.0"

# ─── Colors ──────────────────────────────────────────────────────
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
CYAN='\033[0;36m'
BOLD='\033[1m'
NC='\033[0m'

log()  { echo -e "${GREEN}[OK]${NC} $*"; }
warn() { echo -e "${YELLOW}[!]${NC} $*"; }
err()  { echo -e "${RED}[ERROR]${NC} $*"; }
info() { echo -e "${CYAN}[i]${NC} $*"; }

# ─── Header ──────────────────────────────────────────────────────
echo ""
echo -e "${BOLD}╔══════════════════════════════════════════════╗${NC}"
echo -e "${BOLD}║       CorkyTux Installer v${VERSION}            ║${NC}"
echo -e "${BOLD}╚══════════════════════════════════════════════╝${NC}"
echo ""

# ─── Uninstall mode ──────────────────────────────────────────────
if [[ "${1:-}" == "--uninstall" || "${1:-}" == "-u" ]]; then
  echo -e "${YELLOW}Uninstalling CorkyTux...${NC}"
  rm -rf "${INSTALL_DIR}" 2>/dev/null && echo "  Removed ${INSTALL_DIR}"
  rm -rf "${HOME}/.local/share/CorkyTux" 2>/dev/null && echo "  Removed data dir"
  rm -rf "${HOME}/.config/CorkyTux" 2>/dev/null && echo "  Removed config dir"
  rm -f "${BIN_DIR}/corkytux" 2>/dev/null && echo "  Removed symlink"
  rm -f "${DESKTOP_DIR}/corkytux.desktop" 2>/dev/null && echo "  Removed desktop entry"
  rm -f "${ICON_DIR}/corkytux.png" 2>/dev/null && echo "  Removed icon"
  echo ""
  echo -e "${GREEN}CorkyTux completely uninstalled.${NC}"
  exit 0
fi

# ─── Check binary ────────────────────────────────────────────────
APP=""
for candidate in "${SCRIPT_DIR}/corkytux" "${SCRIPT_DIR}/build/corkytux"; do
  if [[ -f "$candidate" ]] && file "$candidate" | grep -q 'ELF'; then
    APP="$candidate"
    break
  fi
done

if [[ -z "$APP" ]]; then
  err "CorkyTux ELF binary not found in ${SCRIPT_DIR}"
  echo "  Build it first:"
  echo "    cmake -S . -B build -DCMAKE_BUILD_TYPE=Release"
  echo "    cmake --build build -j\$(nproc)"
  echo "    cp build/corkytux release/"
  exit 1
fi
log "Binary found: $(basename "$APP")"

# ─── Detect distro ───────────────────────────────────────────────
detect_distro() {
  if [[ -f /etc/os-release ]]; then
    . /etc/os-release
    DISTRO_ID="${ID:-unknown}"
    DISTRO_LIKE="${ID_LIKE:-$ID}"
  elif command -v lsb_release &>/dev/null; then
    DISTRO_ID=$(lsb_release -is | tr '[:upper:]' '[:lower:]')
    DISTRO_LIKE="$DISTRO_ID"
  else
    DISTRO_ID="unknown"
    DISTRO_LIKE="unknown"
  fi
}
detect_distro
info "Detected: ${DISTRO_ID} (${DISTRO_LIKE})"

# ─── Check Qt6 runtime deps ─────────────────────────────────────
# Required .so libraries the binary links against
QT6_LIBS=(
  libQt6Core.so.6
  libQt6Gui.so.6
  libQt6Quick.so.6
  libQt6Qml.so.6
  libQt6QmlModels.so.6
  libQt6Network.so.6
  libQt6Concurrent.so.6
  libQt6Svg.so.6
  libQt6OpenGL.so.6
  libQt6DBus.so.6
)

missing_qt=()
for lib in "${QT6_LIBS[@]}"; do
  if ! ldconfig -p 2>/dev/null | grep -q "$lib"; then
    # Fallback: check common lib paths
    found=false
    for path in /usr/lib64 /usr/lib /usr/lib/x86_64-linux-gnu /usr/local/lib; do
      if [[ -f "${path}/${lib}" ]]; then
        found=true
        break
      fi
    done
    if ! $found; then
      missing_qt+=("$lib")
    fi
  fi
done

# Other runtime deps
missing_other=()
for cmd in ffmpeg; do
  if ! command -v "$cmd" &>/dev/null; then
    missing_other+=("$cmd")
  fi
done

# Steam (optional but important)
has_steam=false
if command -v steam &>/dev/null || [[ -f /usr/bin/steam ]] \
   || [[ -f "${HOME}/.var/app/com.valvesoftware.Steam/data/Steam" ]]; then
  has_steam=true
fi

# ─── Report status ───────────────────────────────────────────────
echo ""
if [[ ${#missing_qt[@]} -eq 0 ]]; then
  log "All Qt6 libraries found"
else
  warn "Missing Qt6 libraries: ${missing_qt[*]}"
fi

if [[ ${#missing_other[@]} -gt 0 ]]; then
  warn "Missing optional packages: ${missing_other[*]}"
fi

if $has_steam; then
  log "Steam detected"
else
  warn "Steam not found (needed for Proton games)"
fi

# ─── Install missing deps ───────────────────────────────────────
if [[ ${#missing_qt[@]} -gt 0 ]]; then
  echo ""
  info "Qt6 is required. Install commands by distro:"
  echo ""
  case "$DISTRO_LIKE" in
    *gentoo*)
      echo "  sudo emerge --ask=n dev-qt/qtbase dev-qt/qtdeclarative dev-qt/qtsvg"
      ;;
    *debian*|*ubuntu*)
      echo "  sudo apt install qt6-base-dev qt6-declarative-dev libqt6svg6-dev"
      echo "  # Or for runtime only:"
      echo "  sudo apt install libqt6core6t64 libqt6gui6t64 libqt6quick6 libqt6network6 \\"
      echo "    libqt6concurrent6t64 libqt6svg6 libqt6opengl6t64 libqt6qml6 libqt6dbus6t64"
      ;;
    *fedora*|*rhel*|*centos*)
      echo "  sudo dnf install qt6-qtbase qt6-qtdeclarative qt6-qtsvg qt6-qt5compat"
      ;;
    *arch*)
      echo "  sudo pacman -S qt6-base qt6-declarative qt6-svg"
      ;;
    *suse*)
      echo "  sudo zypper install libQt6Core6 libQt6Gui6 libQt6Quick6 libQt6Svg6"
      ;;
    *)
      echo "  Install Qt6 development packages for your distro."
      echo "  Required: Qt6 Core, Gui, Quick, Qml, Network, Concurrent, Svg, OpenGL, DBus"
      ;;
  esac
  echo ""
  read -rp "Continue installation anyway? [y/N]: " CONTINUE
  if [[ ! "$CONTINUE" =~ ^[Yy]$ ]]; then
    echo "Install Qt6 first, then re-run: ./install.sh"
    exit 1
  fi
fi

# ─── Install ─────────────────────────────────────────────────────
echo ""
info "Installing CorkyTux v${VERSION}..."

mkdir -p "$INSTALL_DIR" "$BIN_DIR" "$ICON_DIR" "$DESKTOP_DIR"
install -m 0755 "$APP" "${INSTALL_DIR}/corkytux"
log "Binary: ${INSTALL_DIR}/corkytux"

# Symlink
ln -sf "${INSTALL_DIR}/corkytux" "${BIN_DIR}/corkytux"
log "Symlink: ${BIN_DIR}/corkytux"

# Icon
if [[ -f "${SCRIPT_DIR}/corkytux.png" ]]; then
  install -m 0644 "${SCRIPT_DIR}/corkytux.png" "${ICON_DIR}/corkytux.png"
  log "Icon: ${ICON_DIR}/corkytux.png"
else
  warn "corkytux.png not found, skipping icon"
fi

# Desktop entry
cat > "${DESKTOP_DIR}/corkytux.desktop" <<DESKTOP
[Desktop Entry]
Version=1.0
Name=CorkyTux
Comment=Native Linux game launcher
Exec=${INSTALL_DIR}/corkytux
Icon=${ICON_DIR}/corkytux.png
Terminal=false
Type=Application
Categories=Game;
StartupWMClass=corkytux
DESKTOP
log "Desktop entry: ${DESKTOP_DIR}/corkytux.desktop"

# PATH hint
if ! echo "$PATH" | tr ':' '\n' | grep -qxF "$BIN_DIR"; then
  echo ""
  warn "Add to your shell profile (~/.bashrc, ~/.zshrc, etc.):"
  echo "  export PATH=\"\$HOME/.local/bin:\$PATH\""
fi

# ─── Done ────────────────────────────────────────────────────────
echo ""
echo -e "${BOLD}╔══════════════════════════════════════════════╗${NC}"
echo -e "${BOLD}║           Installation complete!             ║${NC}"
echo -e "${BOLD}╠══════════════════════════════════════════════╣${NC}"
echo -e "${BOLD}║  Run:  corkytux                              ║${NC}"
echo -e "${BOLD}║  Or find 'CorkyTux' in your app menu         ║${NC}"
echo -e "${BOLD}║                                              ║${NC}"
echo -e "${BOLD}║  Uninstall:  ./install.sh --uninstall        ║${NC}"
echo -e "${BOLD}╚══════════════════════════════════════════════╝${NC}"
echo ""
