#!/usr/bin/env bash
set -euo pipefail

# ─── CorkyTux Installer (Rust + GTK4/libadwaita) ────────────────────
# Install prebuilt binary to user dir. Run without sudo.
# Cover Gentoo, Debian/Ubuntu, Fedora/RHEL, Arch, openSUSE.

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
# Read version from tarball dir name (corkytux-X.Y.Z). Use dev for repo runs.
APP_VERSION="$(basename "$SCRIPT_DIR" | grep -oE '[0-9]+\.[0-9]+\.[0-9]+' 2>/dev/null | head -1 || true)"
APP_VERSION="${APP_VERSION:-dev}"
INSTALL_DIR="${HOME}/.local/share/corkytux"
BIN_DIR="${HOME}/.local/bin"
APP_ID="com.corkytux.CorkyTux"
DATA_HOME="${HOME}/.local/share"
[[ "${XDG_DATA_HOME:-}" != /* ]] || DATA_HOME="${XDG_DATA_HOME%/}"
ICON_DIR="${DATA_HOME}/icons"
ICON_THEME_DIR="${ICON_DIR}/hicolor"
DESKTOP_DIR="${DATA_HOME}/applications"
DOCS_URL="https://github.com/Matts-lab69/CorkyTux-Launcher/blob/main/docs/BUILD.md"

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
echo -e "${BOLD}║       CorkyTux Installer v${APP_VERSION} (Rust)     ║${NC}"
echo -e "${BOLD}╚══════════════════════════════════════════════╝${NC}"
echo ""

if [[ "${EUID}" -eq 0 ]]; then
  err "Do NOT run with sudo!"
  echo "  CorkyTux installs to ~/.local/. Run as your user: ./install.sh"
  exit 1
fi

INSTALL_PLUGINS=true
for arg in "$@"; do
  case "$arg" in
    --no-plugins) INSTALL_PLUGINS=false ;;
    --uninstall|-u) exec "${SCRIPT_DIR}/uninstall.sh" ;;
    --help|-h) echo "Usage: ./install.sh [--no-plugins | --uninstall]"; exit 0 ;;
    *) err "Unknown option: $arg"; exit 1 ;;
  esac
done

# Match plugin_process::plugins_base_dir: only an absolute XDG_DATA_HOME
# overrides HOME. Track installed code files so uninstall can remove them
# without deleting downloaded runtimes, saves or user configuration.
PLUGINS_DIR="${DATA_HOME}/CorkyTux/plugins"
BUNDLED_PLUGINS="${SCRIPT_DIR}/plugins"
PLUGIN_MANIFEST="${INSTALL_DIR}/plugins-installed.txt"
PLUGIN_ROOT_FILE="${INSTALL_DIR}/plugins-install-root.txt"

valid_plugin_path() {
  [[ "$1" != /* && "$1" == */* && "$1" != *$'\n'* &&
     "/$1/" != */../* && "/$1/" != */./* ]]
}

# Refuse symlinked destinations before mutating the installation. Replacing
# plugin code must never follow a user link into an unrelated directory.
check_plugin_target() {
  local rel="$1" parent="${1%/*}"
  valid_plugin_path "$rel" || { err "Unsafe bundled plugin path"; exit 1; }
  [[ ! -L "${PLUGINS_DIR}/$rel" ]] || { err "Plugin destination is a symlink"; exit 1; }
  while :; do
    [[ ! -L "${PLUGINS_DIR}/$parent" ]] || { err "Plugin directory is a symlink"; exit 1; }
    [[ "$parent" == */* ]] || break
    parent="${parent%/*}"
  done
}

if $INSTALL_PLUGINS && [[ -d "$BUNDLED_PLUGINS" ]]; then
  [[ ! -L "$BUNDLED_PLUGINS" && ! -L "$PLUGINS_DIR" ]] || { err "Plugin root is a symlink"; exit 1; }
  if [[ -f "$PLUGIN_ROOT_FILE" ]] && [[ "$(cat "$PLUGIN_ROOT_FILE")" != "$PLUGINS_DIR" ]]; then
    err "Previous plugin installation uses a different data root; uninstall it first"
    exit 1
  fi
  for meta in "$BUNDLED_PLUGINS"/*/plugin.json; do
    [[ -f "$meta" ]] || continue
    source_dir="${meta%/plugin.json}"
    plugin_id="${source_dir##*/}"
    [[ "$plugin_id" =~ ^[a-zA-Z0-9][a-zA-Z0-9._-]*$ && -f "$source_dir/$plugin_id" ]] || { err "Invalid bundled plugin"; exit 1; }
    [[ -z "$(find "$source_dir" -type l -print -quit)" ]] || { err "Bundled plugin contains a symlink"; exit 1; }
    while IFS= read -r -d '' file; do
      check_plugin_target "${file#"$BUNDLED_PLUGINS/"}"
    done < <(find "$source_dir" -type f -print0)
  done
fi

# Install UI assets into INSTALL_DIR/assets. Fail when source is missing.
# Validate before any mutation.
ASSETS=""
for candidate in "${SCRIPT_DIR}/assets" "${SCRIPT_DIR}/release/assets" "${SCRIPT_DIR}/../assets"; do
  if [[ -d "$candidate" ]]; then
    ASSETS="$candidate"
    break
  fi
done
if [[ -z "$ASSETS" ]]; then
  err "Assets not found: none of assets/, release/assets/, ../assets/ exist"
  exit 1
fi

# A missing application icon is an incomplete package, not a silent fallback.
ICON_SRC=""
for candidate in "${SCRIPT_DIR}/corkytux.png" "${SCRIPT_DIR}/release/corkytux.png"; do
  if [[ -f "$candidate" ]]; then ICON_SRC="$candidate"; break; fi
done
[[ -n "$ICON_SRC" ]] || { err "corkytux.png not found; refusing incomplete installation"; exit 1; }

# ─── Binary ──────────────────────────────────────────────────────
# Locate prebuilt ELF binary next to this script.
elf() { [[ "$(head -c4 "$1" 2>/dev/null)" == $'\x7fELF' ]]; }
APP=""
for candidate in "${SCRIPT_DIR}/corkytux" "${SCRIPT_DIR}/release/corkytux"; do
  if [[ -f "$candidate" ]] && elf "$candidate"; then
    APP="$candidate"
    break
  fi
done
if [[ -z "$APP" ]]; then
  err "corkytux ELF binary not found in ${SCRIPT_DIR}"
  echo "  See ${DOCS_URL} to compile from source."
  exit 1
fi
log "Binary found: $(basename "$APP") ($(du -h "$APP" | cut -f1))"

# ─── Games folder ────────────────────────────────────────────────
# Keep permanent Heroic/Lutris imports here. Leave existing dir untouched.
if [[ -d "${HOME}/Games" ]]; then
  info "Games folder: ${HOME}/Games (already there, left untouched)"
else
  info "Games folder: ${HOME}/Games does not exist (create it if you import games there)"
fi

# Keep 3 most recent backups of installed binary.
# Copy current binary aside before atomic replace. Prune old copies.
backup_bin() {
  local target="${INSTALL_DIR}/corkytux"
  [[ -f "$target" ]] || return 0
  local ts
  ts="$(date +%Y%m%d-%H%M%S)"
  cp -p "$target" "${target}.bak-${ts}"
  local -a old
  readarray -t old < <(find "${INSTALL_DIR}" -name 'corkytux.bak-*' -type f -printf '%T@ %p\n' 2>/dev/null | sort -rn | cut -d' ' -f2-)
  local keep=3
  for ((i=keep; i<${#old[@]}; i++)); do
    rm -f "${old[i]}" 2>/dev/null || true
  done
}

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
LIB_DIRS=( /usr/lib64 /usr/lib /usr/local/lib "/usr/lib/$(uname -m)-linux-gnu" )
# NixOS keeps glibc and GTK in the store, not in /usr.
if [[ -d /nix/store ]]; then
  LIB_DIRS+=( /nix/store/*-glibc-*/lib /nix/store/*-gtk4-*/lib )
fi
missing_libs=()
for lib in "${GTK_LIBS[@]}"; do
  if ! ldconfig -p 2>/dev/null | grep -q "$lib"; then
    found=false
    for path in "${LIB_DIRS[@]}"; do
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
# Check Python extra modules for minecraft-launcher plugin.
if command -v python3 &>/dev/null; then
  pymissing=()
  python3 -c "import requests" 2>/dev/null || pymissing+=("requests")
  if [[ ${#pymissing[@]} -gt 0 ]]; then
    warn "Missing python modules: ${pymissing[*]} (python3 -m pip install --user ${pymissing[*]} minecraft_launcher_lib psutil, or use your distro package)"
  else
    log "python3 requests OK"
  fi
  # Check Pillow for webp mod icons. Warn only; fall back to generic icon.
  python3 -c "import PIL" 2>/dev/null || warn "Pillow (PIL) not found (webp mod icons need python3+PIL or ffmpeg, else generic icon)"
fi
command -v ffmpeg &>/dev/null && log "ffmpeg found" || warn "ffmpeg not found (webp mod icons need python3+PIL or ffmpeg, else generic icon)"
# Accept system icon theme only as fallback. Prefer bundled icons.
if [[ -d /usr/share/icons/Adwaita || -d /usr/share/icons/hicolor || -d "${HOME}/.local/share/icons/Adwaita" ]]; then
  log "System icon theme OK (fallback)"
else
  warn "No system icon theme found (bundled icons will be used)"
fi
# Require librsvg for bundled SVG icons. Ignore check for PNG art.
svg_ok=false
if ldconfig -p 2>/dev/null | grep -q "librsvg-2.so"; then
  svg_ok=true
else
  for path in "${LIB_DIRS[@]}"; do
    [[ -f "${path}/librsvg-2.so.2" ]] && { svg_ok=true; break; }
  done
fi
$svg_ok && log "SVG icons (librsvg) OK" || warn "librsvg2 not found (bundled symbolic icons need it)"

# ─── Install missing? ────────────────────────────────────────────
if [[ ${#missing_libs[@]} -gt 0 ]]; then
  echo ""
  info "Install runtime by distro:"
  echo ""
  case "$DISTRO_LIKE" in
    *gentoo*)
      echo "  sudo emerge --ask gui-libs/gtk:4 gui-libs/libadwaita gnome-base/librsvg"
      ;;
    *debian*|*ubuntu*)
      echo "  sudo apt install libgtk-4-1 libadwaita-1-0 librsvg2-common libglib2.0-0"
      ;;
    *fedora*|*rhel*|*centos*)
      echo "  sudo dnf install gtk4 libadwaita librsvg2"
      ;;
    *arch*)
      echo "  sudo pacman -S gtk4 libadwaita librsvg"
      ;;
    *suse*)
      echo "  sudo zypper install gtk4 libadwaita librsvg"
      ;;
    *) echo "  Install GTK4 + libadwaita packages for your distro." ;;
  esac
  echo ""
  read -rp "Continue anyway? [y/N]: " CONTINUE
  [[ "$CONTINUE" =~ ^[Yy]$ ]] || { echo "Install GTK4 first, then re-run."; exit 1; }
fi

# ─── Install ─────────────────────────────────────────────────────
echo ""
info "Installing CorkyTux v${APP_VERSION}..."
mkdir -p "$INSTALL_DIR" "$BIN_DIR" "$ICON_DIR" "$DESKTOP_DIR"
backup_bin
install -m 0755 "$APP" "${INSTALL_DIR}/corkytux.new"
mv -f "${INSTALL_DIR}/corkytux.new" "${INSTALL_DIR}/corkytux"
chmod 755 "${INSTALL_DIR}/corkytux"
log "Binary: ${INSTALL_DIR}/corkytux"
ln -sf "${INSTALL_DIR}/corkytux" "${BIN_DIR}/corkytux"
log "Symlink: ${BIN_DIR}/corkytux"

# Stage Epic/GOG login helper next to binary. Do not link into PATH.
# Honor CORKYTUX_LOGIN_HELPER override for tests.
HELPER=""
for candidate in "${SCRIPT_DIR}/webdriver_login" "${SCRIPT_DIR}/release/webdriver_login"; do
  if [[ -f "$candidate" ]] && elf "$candidate"; then
    HELPER="$candidate"
    break
  fi
done
if [[ -n "$HELPER" ]]; then
  install -m 0755 "$HELPER" "${INSTALL_DIR}/webdriver_login"
  log "Store login helper: ${INSTALL_DIR}/webdriver_login"
else
  warn "webdriver_login not found: the Epic/GOG sign-in will not work."
  warn "See ${DOCS_URL} to build it from source."
fi

# Copy pre-validated UI assets into INSTALL_DIR/assets.
mkdir -p "${INSTALL_DIR}/assets"
cp -r "${ASSETS}/." "${INSTALL_DIR}/assets/"
find "${INSTALL_DIR}/assets" -type f -exec chmod 0644 {} + 2>/dev/null || true
find "${INSTALL_DIR}/assets" -type d -exec chmod 0755 {} + 2>/dev/null || true
log "Assets: ${INSTALL_DIR}/assets ($(ls "$ASSETS" | wc -l) files)"

if $INSTALL_PLUGINS && [[ -d "$BUNDLED_PLUGINS" ]]; then
  plugin_list="${PLUGIN_MANIFEST}.new"
  : > "$plugin_list"
  [[ ! -f "$PLUGIN_MANIFEST" ]] || cat "$PLUGIN_MANIFEST" >> "$plugin_list"
  for meta in "$BUNDLED_PLUGINS"/*/plugin.json; do
    [[ -f "$meta" ]] || continue
    source_dir="${meta%/plugin.json}"
    while IFS= read -r -d '' file; do
      rel="${file#"$BUNDLED_PLUGINS/"}"
      mkdir -p "${PLUGINS_DIR}/${rel%/*}"
      mode=0644
      [[ ! -x "$file" ]] || mode=0755
      install -m "$mode" "$file" "${PLUGINS_DIR}/$rel"
      printf '%s\n' "$rel" >> "$plugin_list"
    done < <(find "$source_dir" -type f -print0)
    chmod 0755 "${PLUGINS_DIR}/${source_dir##*/}/${source_dir##*/}"
    log "Bundled plugin: ${source_dir##*/}"
  done
  LC_ALL=C sort -u "$plugin_list" > "$PLUGIN_MANIFEST"
  rm -f "$plugin_list"
  printf '%s\n' "$PLUGINS_DIR" > "$PLUGIN_ROOT_FILE"
elif ! $INSTALL_PLUGINS; then
  info "Bundled plugin installation skipped (--no-plugins)"
else
  info "No bundled plugins; existing plugins left untouched"
fi

# GTK app ID, desktop ID, WM class and hicolor icon share one identity.
install -d "${ICON_THEME_DIR}/128x128/apps"
install -m 0644 "$ICON_SRC" "${ICON_THEME_DIR}/128x128/apps/${APP_ID}.png"
# Keep the old names working for existing shortcuts.
install -m 0644 "$ICON_SRC" "${ICON_THEME_DIR}/128x128/apps/corkytux.png"
install -m 0644 "$ICON_SRC" "${ICON_DIR}/corkytux.png"
log "Icon installed: ${ICON_THEME_DIR}/128x128/apps/${APP_ID}.png"

# Back up current desktop entry before overwrite. Keep 3 most recent copies.
if [[ -f "${DESKTOP_DIR}/${APP_ID}.desktop" ]]; then
  cp -p "${DESKTOP_DIR}/${APP_ID}.desktop" "${DESKTOP_DIR}/${APP_ID}.desktop.bak-$(date +%Y%m%d-%H%M%S)" 2>/dev/null || true
  readarray -t _desk_old < <(find "${DESKTOP_DIR}" -maxdepth 1 -name "${APP_ID}.desktop.bak-*" -type f -printf '%T@ %p\n' 2>/dev/null | sort -rn | cut -d' ' -f2-)
  for ((i=3; i<${#_desk_old[@]}; i++)); do
    rm -f "${_desk_old[i]}" 2>/dev/null || true
  done
  unset _desk_old
fi
cat > "${DESKTOP_DIR}/${APP_ID}.desktop" <<DESKTOP
[Desktop Entry]
Version=1.0
Name=CorkyTux
Comment=Linux game launcher
Exec="${INSTALL_DIR}/corkytux"
Icon=${APP_ID}
Terminal=false
Type=Application
Categories=Game;
StartupWMClass=${APP_ID}
DESKTOP
[[ -s "${ICON_THEME_DIR}/128x128/apps/${APP_ID}.png" ]] || { err "Installed application icon is missing"; exit 1; }
# Migrate only the legacy entry owned by this install, preserving a backup.
legacy_desktop="${DESKTOP_DIR}/corkytux.desktop"
if [[ -f "$legacy_desktop" ]] && grep -Fqx -e "Exec=\"${INSTALL_DIR}/corkytux\"" -e "Exec=${INSTALL_DIR}/corkytux" -e 'Exec=corkytux' "$legacy_desktop"; then
  mv "$legacy_desktop" "${legacy_desktop}.bak-$(date +%Y%m%d-%H%M%S)"
fi
log "Desktop entry: ${DESKTOP_DIR}/${APP_ID}.desktop"

if command -v update-desktop-database &>/dev/null; then
  update-desktop-database "$DESKTOP_DIR" 2>/dev/null || true
fi
if command -v gtk-update-icon-cache &>/dev/null; then
  gtk-update-icon-cache -f -t "$ICON_THEME_DIR" 2>/dev/null || true
fi

if ! echo "$PATH" | tr ':' '\n' | grep -qxF "$BIN_DIR"; then
  echo ""
  warn "Add ${BIN_DIR} to PATH (bash/zsh: export PATH=\"${BIN_DIR}:\$PATH\"; fish: fish_add_path ${BIN_DIR})"
fi

echo ""
echo "  Bundled plugins are installed unless --no-plugins is used."
echo "  Optional plugin dependencies and game runtimes are not downloaded by this installer."
echo "  A running instance will use the new binary only after restart."
echo -e "${BOLD}╔══════════════════════════════════════════════╗${NC}"
echo -e "${BOLD}║           Installation complete!             ║${NC}"
echo -e "${BOLD}║  Run: corkytux (or app menu > CorkyTux)      ║${NC}"
echo -e "${BOLD}║  Uninstall: ./uninstall.sh                  ║${NC}"
echo -e "${BOLD}╚══════════════════════════════════════════════╝${NC}"
echo ""
