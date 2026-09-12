#!/usr/bin/env bash
set -euo pipefail

# ─── CorkyTux Installer v3.0.0 (Rust + GTK4/libadwaita) ──────────
# Installs prebuilt binary to user dir. NO sudo.
# Supports: Gentoo, Debian/Ubuntu, Fedora/RHEL, Arch, openSUSE

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
INSTALL_DIR="${HOME}/.local/share/corkytux"
BIN_DIR="${HOME}/.local/bin"
ICON_DIR="${HOME}/.local/share/icons"
DESKTOP_DIR="${HOME}/.local/share/applications"
VERSION="3.0.0"

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

echo ""
echo -e "${BOLD}╔══════════════════════════════════════════════╗${NC}"
echo -e "${BOLD}║       CorkyTux Installer v${VERSION} (Rust)     ║${NC}"
echo -e "${BOLD}╚══════════════════════════════════════════════╝${NC}"
echo ""

if [[ "${EUID}" -eq 0 ]]; then
  err "Do NOT run with sudo!"
  echo "  CorkyTux installs to ~/.local/. Run as your user: ./install.sh"
  exit 1
fi

if [[ "${1:-}" == "--uninstall" || "${1:-}" == "-u" ]]; then
  exec "${SCRIPT_DIR}/uninstall.sh"
fi

# ─── Binary ──────────────────────────────────────────────────────
APP=""
for candidate in "${SCRIPT_DIR}/corkytux" "${SCRIPT_DIR}/release/corkytux"; do
  if [[ -f "$candidate" ]] && file "$candidate" | grep -q 'ELF'; then
    APP="$candidate"
    break
  fi
done
if [[ -z "$APP" ]]; then
  err "corkytux ELF binary not found in ${SCRIPT_DIR}"
  echo "  See docs/BUILD.md to compile from source."
  exit 1
fi
log "Binary found: $(basename "$APP") ($(du -h "$APP" | cut -f1))"

# ─── Distro ──────────────────────────────────────────────────────
if [[ -f /etc/os-release ]]; then
  # shellcheck disable=SC1091
  . /etc/os-release
  DISTRO_ID="${ID:-unknown}"
  DISTRO_LIKE="${ID_LIKE:-$ID}"
else
  DISTRO_ID="unknown"; DISTRO_LIKE="unknown"
fi
info "Detected: ${DISTRO_ID} (${DISTRO_LIKE})"

# ─── Check GTK4/libadwaita runtime ───────────────────────────────
GTK_LIBS=( libgtk-4.so.1 libadwaita-1.so.0 libpango-1.0.so.0 libcairo.so.2 libgdk_pixbuf-2.0.so.0 libglib-2.0.so.0 )
missing_libs=()
for lib in "${GTK_LIBS[@]}"; do
  if ! ldconfig -p 2>/dev/null | grep -q "$lib"; then
    found=false
    for path in /usr/lib64 /usr/lib /usr/lib/x86_64-linux-gnu /usr/local/lib; do
      [[ -f "${path}/${lib}" ]] && { found=true; break; }
    done
    $found || missing_libs+=("$lib")
  fi
done

echo ""
if [[ ${#missing_libs[@]} -eq 0 ]]; then
  log "GTK4 + libadwaita runtime OK"
else
  warn "Missing runtime libs: ${missing_libs[*]}"
fi

# ─── Optional tools ──────────────────────────────────────────────
missing_opt=()
command -v python3 &>/dev/null || missing_opt+=("python3 (plugins)")
command -v sqlite3 &>/dev/null || missing_opt+=("sqlite3 (Lutris scan)")
command -v curl &>/dev/null || missing_opt+=("curl (Proton downloads)")
command -v tar &>/dev/null || missing_opt+=("tar")
command -v java &>/dev/null && log "Java found: $(java -version 2>&1 | head -1)" || warn "Java not found (needed for Minecraft plugin)"
if command -v steam &>/dev/null || [[ -d "${HOME}/.steam" ]]; then
  log "Steam detected"
else
  warn "Steam not found (optional, for Proton/Steam scan)"
fi
command -v umu-run &>/dev/null && log "umu-run found" || warn "umu-launcher not found (optional, auto-installed by heroic-store plugin to tools/umu)"
if [[ ${#missing_opt[@]} -gt 0 ]]; then
  warn "Missing optional tools: ${missing_opt[*]}"
fi
# Python extra modules for minecraft-launcher plugin
if command -v python3 &>/dev/null; then
  pymissing=()
  python3 -c "import requests" 2>/dev/null || pymissing+=("requests")
  if [[ ${#pymissing[@]} -gt 0 ]]; then
    warn "Missing python modules: ${pymissing[*]} (pip install --user ${pymissing[*]} minecraft_launcher_lib psutil)"
  else
    log "python3 requests OK"
  fi
fi

# ─── Install missing? ────────────────────────────────────────────
if [[ ${#missing_libs[@]} -gt 0 ]]; then
  echo ""
  info "Install runtime by distro:"
  echo ""
  case "$DISTRO_LIKE" in
    *gentoo*)
      echo "  sudo emerge --ask gui-libs/gtk:4 gui-libs/libadwaita"
      ;;
    *debian*|*ubuntu*)
      echo "  sudo apt install libgtk-4-1 libadwaita-1-0 librsvg2-common libglib2.0-0"
      ;;
    *fedora*|*rhel*|*centos*)
      echo "  sudo dnf install gtk4 libadwaita"
      ;;
    *arch*)
      echo "  sudo pacman -S gtk4 libadwaita"
      ;;
    *suse*)
      echo "  sudo zypper install gtk4 libadwaita"
      ;;
    *) echo "  Install GTK4 + libadwaita packages for your distro." ;;
  esac
  echo ""
  read -rp "Continue anyway? [y/N]: " CONTINUE
  [[ "$CONTINUE" =~ ^[Yy]$ ]] || { echo "Install GTK4 first, then re-run."; exit 1; }
fi

# ─── Install ─────────────────────────────────────────────────────
echo ""
info "Installing CorkyTux v${VERSION}..."
mkdir -p "$INSTALL_DIR" "$BIN_DIR" "$ICON_DIR" "$DESKTOP_DIR"
install -m 0755 "$APP" "${INSTALL_DIR}/corkytux"
log "Binary: ${INSTALL_DIR}/corkytux"
ln -sf "${INSTALL_DIR}/corkytux" "${BIN_DIR}/corkytux"
log "Symlink: ${BIN_DIR}/corkytux"

# UI assets (the launcher loads themed icons from INSTALL_DIR/assets)
ASSETS=""
for candidate in "${SCRIPT_DIR}/assets" "${SCRIPT_DIR}/release/assets"; do
  if [[ -d "$candidate" ]]; then
    ASSETS="$candidate"
    break
  fi
done
if [[ -n "$ASSETS" ]]; then
  mkdir -p "${INSTALL_DIR}/assets"
  cp -r "${ASSETS}/." "${INSTALL_DIR}/assets/"
  chmod 0644 "${INSTALL_DIR}"/assets/* 2>/dev/null || true
  log "Assets: ${INSTALL_DIR}/assets ($(ls "$ASSETS" | wc -l) files)"
else
  warn "assets/ not found, UI icons will be missing"
fi

if [[ -f "${SCRIPT_DIR}/corkytux.png" ]]; then
  install -m 0644 "${SCRIPT_DIR}/corkytux.png" "${ICON_DIR}/corkytux.png"
  log "Icon installed"
elif [[ -f "${SCRIPT_DIR}/release/corkytux.png" ]]; then
  install -m 0644 "${SCRIPT_DIR}/release/corkytux.png" "${ICON_DIR}/corkytux.png"
  log "Icon installed"
else
  warn "corkytux.png not found, skipping icon"
fi

cat > "${DESKTOP_DIR}/corkytux.desktop" <<DESKTOP
[Desktop Entry]
Version=1.0
Name=CorkyTux
Comment=Linux game launcher (Rust + GTK4) — Proton/Wine, Minecraft, Epic/GOG
Exec=${INSTALL_DIR}/corkytux
Icon=${ICON_DIR}/corkytux.png
Terminal=false
Type=Application
Categories=Game;
StartupWMClass=corkytux
DESKTOP
log "Desktop entry: ${DESKTOP_DIR}/corkytux.desktop"

if command -v update-desktop-database &>/dev/null; then
  update-desktop-database "$DESKTOP_DIR" 2>/dev/null || true
fi
if command -v gtk-update-icon-cache &>/dev/null; then
  gtk-update-icon-cache -f -t "$ICON_DIR" 2>/dev/null || true
fi

if ! echo "$PATH" | tr ':' '\n' | grep -qxF "$BIN_DIR"; then
  echo ""
  warn "Add to ~/.bashrc / ~/.zshrc:"
  echo "  export PATH=\"\$HOME/.local/bin:\$PATH\""
fi

echo ""
echo -e "${BOLD}Plugins install separately from the launcher${NC}"
echo "  Settings > Plugins, or repo: https://github.com/Matts-lab69/CorkyTux-Plugins"
echo ""
echo -e "${BOLD}╔══════════════════════════════════════════════╗${NC}"
echo -e "${BOLD}║           Installation complete!             ║${NC}"
echo -e "${BOLD}║  Run: corkytux (or app menu > CorkyTux)      ║${NC}"
echo -e "${BOLD}║  Uninstall: ./uninstall.sh                  ║${NC}"
echo -e "${BOLD}╚══════════════════════════════════════════════╝${NC}"
echo ""
