#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
JAR_NAME="corkytux.jar"
INSTALL_DIR="${HOME}/.local/share/corkytux"
BIN_DIR="${HOME}/.local/bin"
JAR="${SCRIPT_DIR}/${JAR_NAME}"

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
CYAN='\033[0;36m'
NC='\033[0m'

log()  { echo -e "${GREEN}[OK]${NC} $*"; }
warn() { echo -e "${YELLOW}[!]${NC} $*"; }
err()  { echo -e "${RED}[ERROR]${NC} $*"; }
info() { echo -e "${CYAN}[i]${NC} $*"; }

echo ""
echo "╔══════════════════════════════════════════════╗"
echo "║  CorkyTux — Installer v2.8.0 ║"
echo "╚══════════════════════════════════════════════╝"
echo ""

# ─── Check JAR ────────────────────────────────────
# Accept both plain and versioned jar names
if [[ ! -f "$JAR" ]]; then
  VERSIONED=$(ls "${SCRIPT_DIR}"/corkytux-*.jar 2>/dev/null | head -1)
  if [[ -n "$VERSIONED" ]]; then
    JAR="$VERSIONED"
  fi
fi
if [[ ! -f "$JAR" ]]; then
  err "${JAR_NAME} not found in ${SCRIPT_DIR}"
  echo "  Download it from: https://github.com/Matts-lab69/onlinefix-linux/releases"
  exit 1
fi
log "JAR found"

# ─── Detect distro ───────────────────────────────
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

# ─── Sudo check ──────────────────────────────────
SUDO=""
if [[ "$(id -u)" -ne 0 ]]; then
  if command -v sudo &>/dev/null; then
    SUDO="sudo"
  else
    warn "sudo not found. Some operations may need root."
  fi
fi

# ─── Find or install Java ────────────────────────
find_java() {
  local candidates=(
    /opt/jvm/jdk-25.0.4.1+1 /opt/jvm/jdk-21.0.4.1+1
    /opt/jvm/temurin-25 /opt/jvm/temurin-24 /opt/jvm/temurin-23 /opt/jvm/temurin-22 /opt/jvm/temurin-21
    /usr/lib/jvm/java-25-temurin /usr/lib/jvm/java-21-temurin
    /usr/lib/jvm/java-25-temurin-amd64 /usr/lib/jvm/java-21-temurin-amd64
    /usr/lib/jvm/java-25-temurin-arm64 /usr/lib/jvm/java-21-temurin-arm64
    /usr/lib/jvm/openjdk-25 /usr/lib/jvm/openjdk-21
    /usr/lib/jvm/java-25-openjdk-amd64 /usr/lib/jvm/java-21-openjdk-amd64
    /usr/lib/jvm/java-25-openjdk /usr/lib/jvm/java-21-openjdk
  )
  for p in "${candidates[@]}"; do
    if [[ -x "$p/bin/java" ]]; then
      local ver
      ver=$("$p/bin/java" -version 2>&1 | head -1 | sed -n 's/.*"\([0-9][0-9]*\).*/\1/p')
      if [[ "$ver" -ge 21 ]]; then
        echo "$p"
        return 0
      fi
    fi
  done
  # Fallback: java on PATH
  if command -v java &>/dev/null; then
    local jp ver
    jp=$(readlink -f "$(command -v java)")
    ver=$(java -version 2>&1 | head -1 | sed -n 's/.*"\([0-9][0-9]*\).*/\1/p')
    if [[ "$ver" -ge 21 ]]; then
      dirname "$(dirname "$jp")"
      return 0
    fi
  fi
  return 1
}

install_java_gentoo() {
  info "Installing Temurin JDK via emerge..."
  $SUDO emerge --ask=n --autounmask-write dev-java/temurin:25 2>&1 \
    || $SUDO emerge --ask=n --autounmask-write dev-java/temurin:21 2>&1 \
    || { err "Failed to install via emerge. Check output above."; return 1; }
}

install_java_debian() {
  info "Installing Temurin JDK via apt..."
  # Get codename
  local CODENAME=""
  if command -v lsb_release &>/dev/null; then
    CODENAME=$(lsb_release -cs)
  elif [[ -f /etc/os-release ]]; then
    CODENAME=$(. /etc/os-release; echo "${VERSION_CODENAME:-}")
  fi
  if [[ -z "$CODENAME" ]]; then
    err "Cannot determine distribution codename. Install lsb-release first."
    return 1
  fi
  # Add Adoptium repo
  if [[ ! -f /etc/apt/keyrings/adoptium.gpg ]]; then
    $SUDO mkdir -p /etc/apt/keyrings
    if command -v wget &>/dev/null; then
      wget -qO- https://packages.adoptium.net/artifactory/api/gpg/key/public | $SUDO gpg --dearmor -o /etc/apt/keyrings/adoptium.gpg
    elif command -v curl &>/dev/null; then
      curl -fsSL https://packages.adoptium.net/artifactory/api/gpg/key/public | $SUDO gpg --dearmor -o /etc/apt/keyrings/adoptium.gpg
    else
      err "Neither wget nor curl found. Install one and retry."
      return 1
    fi
    echo "deb [signed-by=/etc/apt/keyrings/adoptium.gpg] https://packages.adoptium.net/artifactory/deb ${CODENAME} main" \
      | $SUDO tee /etc/apt/sources.list.d/adoptium.list > /dev/null
  fi
  $SUDO apt-get update -qq
  $SUDO apt-get install -y temurin-21-jdk || $SUDO apt-get install -y temurin-25-jdk
}

install_java_fedora() {
  info "Installing Temurin JDK..."
  if command -v dnf &>/dev/null; then
    $SUDO dnf install -y temurin-21-jdk || $SUDO dnf install -y temurin-25-jdk
  elif command -v yum &>/dev/null; then
    $SUDO yum install -y temurin-21-jdk || $SUDO yum install -y temurin-25-jdk
  else
    err "Neither dnf nor yum found."
    return 1
  fi
}

install_java_arch() {
  info "Installing JDK via pacman..."
  $SUDO pacman -S --noconfirm jdk21-openjdk || $SUDO pacman -S --noconfirm jdk25-openjdk
}

install_java_suse() {
  info "Installing Temurin JDK via zypper..."
  $SUDO zypper --non-interactive install temurin-21-jdk || $SUDO zypper --non-interactive install temurin-25-jdk
}

JAVA_HOME=""
JAVA_HOME=$(find_java 2>/dev/null) || true
if [[ -n "$JAVA_HOME" ]]; then
  log "Java found: ${JAVA_HOME}"
else
  warn "Java 21+ not found. Installing..."
  case "$DISTRO_LIKE" in
    *gentoo*)   install_java_gentoo ;;
    *debian*|*ubuntu*) install_java_debian ;;
    *fedora*|*rhel*|*centos*) install_java_fedora ;;
    *arch*)     install_java_arch ;;
    *suse*)     install_java_suse ;;
    *)
      err "Unsupported distro: ${DISTRO_ID}"
      echo "  Install Java 21+ manually: https://adoptium.net"
      echo "  Then re-run this installer."
      exit 1
      ;;
  esac
  JAVA_HOME=$(find_java 2>/dev/null) || true
  if [[ -n "$JAVA_HOME" ]]; then
    log "Java installed: ${JAVA_HOME}"
  else
    err "Java installation failed. Install manually and re-run."
    exit 1
  fi
fi

# ─── Check dependencies ──────────────────────────
check_deps() {
  local missing=()
  local optional=()
  
  # Required
  for cmd in ffmpeg; do
    if ! command -v "$cmd" &>/dev/null; then
      missing+=("$cmd")
    fi
  done
  
  # Steam - check binary or flatpak
  if ! command -v steam &>/dev/null; then
    if [[ -f /usr/bin/steam ]]; then
      : # ok
    elif [[ -f "${HOME}/.var/app/com.valvesoftware.Steam/data/Steam" ]]; then
      : # flatpak steam
    else
      missing+=("steam")
    fi
  fi
  
  # Optional
  for cmd in icoextract aria2c; do
    if ! command -v "$cmd" &>/dev/null; then
      optional+=("$cmd")
    fi
  done
  
  if [[ ${#missing[@]} -gt 0 ]]; then
    warn "Missing required packages: ${missing[*]}"
    info "Installing dependencies..."
    
    case "$DISTRO_LIKE" in
      *gentoo*)
        for pkg in "${missing[@]}"; do
          case "$pkg" in
            ffmpeg)   $SUDO emerge --ask=n --autounmask-write media-video/ffmpeg ;;
            steam)    $SUDO emerge --ask=n --autounmask-write games-util/steam-launcher ;;
          esac
        done
        ;;
      *debian*|*ubuntu*)
        $SUDO apt-get install -y "${missing[@]}"
        ;;
      *fedora*|*rhel*|*centos*)
        $SUDO dnf install -y "${missing[@]}" 2>/dev/null || $SUDO yum install -y "${missing[@]}"
        ;;
      *arch*)
        $SUDO pacman -S --noconfirm "${missing[@]}"
        ;;
      *suse*)
        $SUDO zypper --non-interactive install "${missing[@]}"
        ;;
      *)
        err "Cannot auto-install dependencies. Please install manually:"
        echo "  ${missing[*]}"
        return 1
        ;;
    esac
  fi
  
  if [[ ${#optional[@]} -gt 0 ]]; then
    warn "Optional packages not found: ${optional[*]}"
    echo ""
    echo "  - icoextract: better .exe icon extraction"
    echo "  - aria2c: game downloads (aria2 package)"
    echo ""
    read -rp "Install optional packages now? [y/N]: " INSTALL_OPTIONAL
    if [[ "$INSTALL_OPTIONAL" =~ ^[Yy]$ ]]; then
      local pkgs=()
      for pkg in "${optional[@]}"; do
        case "$pkg" in
          icoextract) pkgs+=("icoextract") ;;
          aria2c)     pkgs+=("aria2") ;;
        esac
      done
      
      case "$DISTRO_LIKE" in
        *gentoo*)
          for pkg in "${pkgs[@]}"; do
            case "$pkg" in
              icoextract) $SUDO emerge --ask=n --autounmask-write dev-python/icoextract || warn "Failed to install icoextract, skipping" ;;
              aria2)      $SUDO emerge --ask=n --autounmask-write net-misc/aria2 || warn "Failed to install aria2, skipping" ;;
            esac
          done
          ;;
        *debian*|*ubuntu*)
          $SUDO apt-get install -y "${pkgs[@]}"
          ;;
        *fedora*|*rhel*|*centos*)
          $SUDO dnf install -y "${pkgs[@]}" 2>/dev/null || $SUDO yum install -y "${pkgs[@]}"
          ;;
        *arch*)
          $SUDO pacman -S --noconfirm "${pkgs[@]}"
          ;;
        *suse*)
          $SUDO zypper --non-interactive install "${pkgs[@]}"
          ;;
        *)
          warn "Cannot auto-install. Install manually:"
          echo "  ${pkgs[*]}"
          ;;
      esac
    else
      info "Skipping optional packages"
    fi
  fi
}

check_deps
info "Installing launcher to ${INSTALL_DIR}..."
mkdir -p "${INSTALL_DIR}"
mkdir -p "${BIN_DIR}"
cp -f "$JAR" "${INSTALL_DIR}/${JAR_NAME}"
cp -f "${SCRIPT_DIR}/corkytux" "${INSTALL_DIR}/corkytux"
chmod +x "${INSTALL_DIR}/corkytux"
log "Files installed"

# ─── Launch wrapper ──────────────────────────────
cat > "${INSTALL_DIR}/launch.sh" << LAUNCH
#!/bin/bash
set -e
export DISPLAY="\${DISPLAY:-:0}"
export DBUS_SESSION_BUS_ADDRESS="\${DBUS_SESSION_BUS_ADDRESS:-}"
JAVA_BIN="${JAVA_HOME}/bin/java"
JAR_PATH="${INSTALL_DIR}/${JAR_NAME}"
if [[ ! -x "\$JAVA_BIN" ]]; then
  notify-send "CorkyTux" "Java not found at \$JAVA_BIN" 2>/dev/null
  echo "Error: Java not found at \$JAVA_BIN" >&2
  exit 1
fi
exec "\$JAVA_BIN" \${CORKYTUX_JAVA_OPTS:--Xmx512m} -jar "\$JAR_PATH" "\$@"
LAUNCH
chmod +x "${INSTALL_DIR}/launch.sh"
log "Launch wrapper created"

# ─── Symlink ─────────────────────────────────────
ln -sf "${INSTALL_DIR}/corkytux" "${BIN_DIR}/corkytux"
log "Symlink: ${BIN_DIR}/corkytux"

# ─── PATH check ──────────────────────────────────
if ! echo "$PATH" | tr ':' '\n' | grep -qxF "$BIN_DIR"; then
  warn "Add to your shell profile (~/.bashrc, ~/.zshrc, etc.):"
  echo "  export PATH=\"\$HOME/.local/bin:\$PATH\""
fi

# ─── Icon ────────────────────────────────────────
ICON_DIR="${HOME}/.local/share/icons"
mkdir -p "$ICON_DIR"
if [[ -f "${SCRIPT_DIR}/corkytux.png" ]]; then
  cp -f "${SCRIPT_DIR}/corkytux.png" "${ICON_DIR}/corkytux.png"
  log "Icon installed"
elif [[ -f "${ICON_DIR}/corkytux.png" ]]; then
  log "Icon already exists"
else
  warn "Icon not found. Desktop entry may show no icon."
fi

# ─── Desktop entry ───────────────────────────────
DESKTOP_DIR="${HOME}/.local/share/applications"
mkdir -p "$DESKTOP_DIR"
cat > "${DESKTOP_DIR}/corkytux.desktop" << DESKTOP
[Desktop Entry]
Version=1.0
Name=CorkyTux
Comment=Play OnlineFix games on Linux
Exec=${INSTALL_DIR}/launch.sh
Icon=${HOME}/.local/share/icons/corkytux.png
Terminal=false
Type=Application
Categories=Game;
StartupWMClass=com-corkytux
DESKTOP
log "Desktop entry created"

# ─── Verify ──────────────────────────────────────
echo ""
echo "╔══════════════════════════════════════════════╗"
echo "║             Installation complete!           ║"
echo "╠══════════════════════════════════════════════╣"
echo "║  Run: corkytux                                   ║"
echo "║  Or find 'CorkyTux' in menu  ║"
echo "╚══════════════════════════════════════════════╝"
echo ""
