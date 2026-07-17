#!/usr/bin/env bash
# =============================================================================
#  dictatord — Install Script
# =============================================================================
# Installs dictatord and its dependencies.
#
# Usage:
#   ./install.sh              — Build (release) and install
#   ./install.sh --debug      — Build in debug mode instead of release
#   ./install.sh --help       — Show this help
#
# This script will:
#   1. Check for required system dependencies
#   2. Attempt to auto-install missing deps (if known distro)
#   3. Build the binary
#   4. Install to ~/.cargo/bin/, ~/.config/dictatord/, etc.
#   5. Optionally enable the systemd user service
# =============================================================================

set -euo pipefail

# ---------------------------------------------------------------------------
# Colors
# ---------------------------------------------------------------------------
GREEN='\033[0;32m'
BLUE='\033[0;34m'
YELLOW='\033[1;33m'
RED='\033[0;31m'
NC='\033[0m'

# ---------------------------------------------------------------------------
# Paths
# ---------------------------------------------------------------------------
PROJECT_DIR="$(cd "$(dirname "$0")" && pwd)"
BINARY_NAME="dictatord"
SERVICE_NAME="dictatord.service"

CARGO_BIN_DIR="$HOME/.cargo/bin"
CONFIG_DIR="$HOME/.config/dictatord"
DATA_DIR="$HOME/.local/share/dictatord"
SOUNDS_DIR="$DATA_DIR/sounds"
SERVICE_DIR="$HOME/.config/systemd/user"
CACHE_DIR="$HOME/.cache/dictatord"
MANIFEST_FILE="$DATA_DIR/install_manifest.txt"

# ---------------------------------------------------------------------------
# Parse arguments
# ---------------------------------------------------------------------------
BUILD_MODE="release"
for arg in "$@"; do
    case "$arg" in
        --debug|-d)
            BUILD_MODE="debug"
            ;;
        --help|-h)
            echo "Usage: $0 [--debug] [--help]"
            echo ""
            echo "  --debug, -d   Build in debug mode (default: release)"
            echo "  --help, -h    Show this help"
            exit 0
            ;;
    esac
done

# ---------------------------------------------------------------------------
# Print header
# ---------------------------------------------------------------------------
echo -e "${BLUE}╔════════════════════════════════════════╗${NC}"
echo -e "${BLUE}║     dictatord - Install Script        ║${NC}"
echo -e "${BLUE}╚════════════════════════════════════════╝${NC}"
echo ""

# ---------------------------------------------------------------------------
# Detect distro for package manager
# ---------------------------------------------------------------------------
detect_package_manager() {
    if command -v apt-get &>/dev/null; then
        echo "apt"
    elif command -v dnf &>/dev/null; then
        echo "dnf"
    elif command -v pacman &>/dev/null; then
        echo "pacman"
    elif command -v zypper &>/dev/null; then
        echo "zypper"
    else
        echo "unknown"
    fi
}

PKG_MANAGER=$(detect_package_manager)

# Map package names per distro
# Debian/Ubuntu (apt)
APT_DEPS="libgtk-3-dev libx11-dev libxdo-dev libasound2-dev pkg-config"
# Fedora (dnf)
DNF_DEPS="gtk3-devel libX11-devel libxdo-devel alsa-lib-devel pkgconfig"
# Arch (pacman)
PACMAN_DEPS="gtk3 libx11 libxdo alsa-lib pkg-config"
# openSUSE (zypper)
ZYPPER_DEPS="gtk3-devel libX11-devel libxdo-devel alsa-devel pkg-config"

# ---------------------------------------------------------------------------
# Print a dependency table for manual install
# ---------------------------------------------------------------------------
print_deps_manual() {
    echo ""
    echo -e "${YELLOW}  Please install the following development packages:${NC}"
    echo ""
    printf "  ${BLUE}%-12s${NC} %s\n" "Ubuntu/Debian:" "sudo apt install $APT_DEPS"
    printf "  ${BLUE}%-12s${NC} %s\n" "Fedora:"      "sudo dnf install $DNF_DEPS"
    printf "  ${BLUE}%-12s${NC} %s\n" "Arch:"        "sudo pacman -S $PACMAN_DEPS"
    printf "  ${BLUE}%-12s${NC} %s\n" "openSUSE:"    "sudo zypper install $ZYPPER_DEPS"
    echo ""
    echo -e "  Then re-run this install script."
}

# ---------------------------------------------------------------------------
# Step 1 — Check system dependencies
# ---------------------------------------------------------------------------
echo -e "${YELLOW}[1/5] Checking system dependencies...${NC}"

DEPS_MISSING=false

# Check runtime commands
for cmd in rustc cargo pactl curl; do
    if ! command -v "$cmd" &>/dev/null; then
        echo -e "  ${YELLOW}Warning:${NC} '$cmd' not found (may be optional)"
    fi
done

# Check pkg-config itself
if ! command -v pkg-config &>/dev/null; then
    echo -e "  ${RED}Error:${NC} 'pkg-config' not found."
    DEPS_MISSING=true
fi

# Check for GTK dev headers
if ! pkg-config --exists gtk+-3.0 2>/dev/null; then
    echo -e "  ${YELLOW}Warning:${NC} GTK3 development headers not found."
    echo -e "           (Visualizer requires GTK3)"
    DEPS_MISSING=true
fi

# Check for X11 dev headers
if ! pkg-config --exists x11 2>/dev/null; then
    echo -e "  ${YELLOW}Warning:${NC} X11 development headers not found."
    echo -e "           (Hotkey support requires X11)"
    DEPS_MISSING=true
fi

# Check for ALSA
if ! pkg-config --exists alsa 2>/dev/null; then
    echo -e "  ${YELLOW}Warning:${NC} ALSA development headers not found."
    DEPS_MISSING=true
fi

# Check for libxdo
if ! pkg-config --exists xdo 2>/dev/null; then
    echo -e "  ${YELLOW}Warning:${NC} libxdo development headers not found."
    DEPS_MISSING=true
fi

if [ "$DEPS_MISSING" = true ]; then
    echo ""
    echo -e "${YELLOW}  Some dependencies are missing.${NC}"

    case "$PKG_MANAGER" in
        apt)
            echo -e "${BLUE}  Detected: apt (Debian/Ubuntu)${NC}"
            echo -e "  Install missing packages? [Y/n]: "
            read -r answer
            case "${answer:-Y}" in
                [Yy]*|"")
                    echo -e "${YELLOW}  Installing dependencies (sudo required)...${NC}"
                    sudo apt-get update -qq && sudo apt-get install -y $APT_DEPS
                    echo -e "${GREEN}  ✓ Dependencies installed${NC}"
                    ;;
                *)
                    echo -e "  ${YELLOW}Skipping. You may need to install deps manually.${NC}"
                    print_deps_manual
                    ;;
            esac
            ;;
        dnf)
            echo -e "${BLUE}  Detected: dnf (Fedora)${NC}"
            echo -e "  Install missing packages? [Y/n]: "
            read -r answer
            case "${answer:-Y}" in
                [Yy]*|"")
                    echo -e "${YELLOW}  Installing dependencies (sudo required)...${NC}"
                    sudo dnf install -y $DNF_DEPS
                    echo -e "${GREEN}  ✓ Dependencies installed${NC}"
                    ;;
                *)
                    echo -e "  ${YELLOW}Skipping. You may need to install deps manually.${NC}"
                    print_deps_manual
                    ;;
            esac
            ;;
        pacman)
            echo -e "${BLUE}  Detected: pacman (Arch)${NC}"
            echo -e "  Install missing packages? [Y/n]: "
            read -r answer
            case "${answer:-Y}" in
                [Yy]*|"")
                    echo -e "${YELLOW}  Installing dependencies (sudo required)...${NC}"
                    sudo pacman -S --needed $PACMAN_DEPS
                    echo -e "${GREEN}  ✓ Dependencies installed${NC}"
                    ;;
                *)
                    echo -e "  ${YELLOW}Skipping. You may need to install deps manually.${NC}"
                    print_deps_manual
                    ;;
            esac
            ;;
        zypper)
            echo -e "${BLUE}  Detected: zypper (openSUSE)${NC}"
            echo -e "  Install missing packages? [Y/n]: "
            read -r answer
            case "${answer:-Y}" in
                [Yy]*|"")
                    echo -e "${YELLOW}  Installing dependencies (sudo required)...${NC}"
                    sudo zypper install -y $ZYPPER_DEPS
                    echo -e "${GREEN}  ✓ Dependencies installed${NC}"
                    ;;
                *)
                    echo -e "  ${YELLOW}Skipping. You may need to install deps manually.${NC}"
                    print_deps_manual
                    ;;
            esac
            ;;
        *)
            echo -e "${YELLOW}  Unknown package manager.${NC}"
            print_deps_manual
            echo -e "  ${YELLOW}After installing dependencies, re-run this script.${NC}"
            echo ""
            echo -e "  ${YELLOW}Press Enter to continue anyway (build may fail)...${NC}"
            read -r
            ;;
    esac
else
    echo -e "${GREEN}  ✓ All dependencies found${NC}"
fi

echo ""

# ---------------------------------------------------------------------------
# Step 2 — Build the binary
# ---------------------------------------------------------------------------
echo -e "${YELLOW}[2/5] Building dictatord...${NC}"

cd "$PROJECT_DIR"

if [ "$BUILD_MODE" = "release" ]; then
    echo -e "  Building in ${GREEN}release${NC} mode (optimized)..."
    cargo build --release
    BINARY_PATH="$PROJECT_DIR/target/release/$BINARY_NAME"
else
    echo -e "  Building in ${YELLOW}debug${NC} mode..."
    cargo build
    BINARY_PATH="$PROJECT_DIR/target/debug/$BINARY_NAME"
fi

echo -e "${GREEN}  ✓ Build complete${NC}"
echo ""

# ---------------------------------------------------------------------------
# Step 3 — Install binary
# ---------------------------------------------------------------------------
echo -e "${YELLOW}[3/5] Installing binary...${NC}"

mkdir -p "$CARGO_BIN_DIR"
cp "$BINARY_PATH" "$CARGO_BIN_DIR/$BINARY_NAME"
echo -e "${GREEN}  ✓ Installed to $CARGO_BIN_DIR/$BINARY_NAME${NC}"
echo ""

# ---------------------------------------------------------------------------
# Step 4 — Configuration, sounds, cache
# ---------------------------------------------------------------------------
echo -e "${YELLOW}[4/5] Setting up configuration and data files...${NC}"

# Config
mkdir -p "$CONFIG_DIR"
if [ ! -f "$CONFIG_DIR/config.toml" ]; then
    cp "$PROJECT_DIR/config/default.toml" "$CONFIG_DIR/config.toml"
    echo -e "${GREEN}  ✓ Default config created at $CONFIG_DIR/config.toml${NC}"
else
    echo -e "  ${YELLOW}Config exists, keeping your existing config${NC}"
fi

# Model cache
mkdir -p "$CACHE_DIR/models"
echo -e "${GREEN}  ✓ Model cache directory created${NC}"

# Sound files
mkdir -p "$SOUNDS_DIR"
if [ -d "$PROJECT_DIR/sounds" ]; then
    cp "$PROJECT_DIR"/sounds/*.ogg "$SOUNDS_DIR/" 2>/dev/null || true
    echo -e "${GREEN}  ✓ Sound files installed${NC}"
else
    echo -e "  ${YELLOW}No sound files found in repo, skipping${NC}"
fi

# Install manifest
mkdir -p "$DATA_DIR"
cat > "$MANIFEST_FILE" <<- EOF
$CARGO_BIN_DIR/$BINARY_NAME
$CONFIG_DIR/config.toml
$SOUNDS_DIR/start.ogg
$SOUNDS_DIR/stop.ogg
$SERVICE_DIR/$SERVICE_NAME
EOF
echo -e "${GREEN}  ✓ Install manifest created${NC}"
echo ""

# ---------------------------------------------------------------------------
# Step 5 — Install systemd user service
# ---------------------------------------------------------------------------
echo -e "${YELLOW}[5/5] Installing systemd user service...${NC}"

mkdir -p "$SERVICE_DIR"

if [ -f "$PROJECT_DIR/systemd/$SERVICE_NAME" ]; then
    cp "$PROJECT_DIR/systemd/$SERVICE_NAME" "$SERVICE_DIR/$SERVICE_NAME"
    systemctl --user daemon-reload
    echo -e "${GREEN}  ✓ Service file installed${NC}"

    echo ""
    echo -e "${BLUE}  Do you want to enable and start the service now?${NC}"
    echo -n "  Start dictatord on login? [Y/n]: "
    read -r answer
    case "${answer:-Y}" in
        [Yy]*|"")
            systemctl --user enable "$SERVICE_NAME" 2>/dev/null || true
            systemctl --user start "$SERVICE_NAME" 2>/dev/null || true
            echo -e "${GREEN}  ✓ Service enabled and started${NC}"
            ;;
        *)
            echo -e "  ${YELLOW}Service not enabled. You can start manually:${NC}"
            echo "    systemctl --user start dictatord"
            echo "    systemctl --user enable dictatord"
            ;;
    esac
else
    echo -e "  ${YELLOW}Service file not found in repo, skipping${NC}"
fi

# ---------------------------------------------------------------------------
# Done!
# ---------------------------------------------------------------------------
echo ""
echo -e "${GREEN}╔════════════════════════════════════════╗${NC}"
echo -e "${GREEN}║     Installation Complete!             ║${NC}"
echo -e "${GREEN}╚════════════════════════════════════════╝${NC}"
echo ""
echo -e "  ${BLUE}Usage:${NC}"
echo -e "    Press ${YELLOW}Super+D${NC} to start/stop dictation"
echo -e "    Run '${YELLOW}dictatord --help${NC}' for CLI options"
echo ""
echo -e "  ${BLUE}Configuration:${NC}"
echo -e "    Edit ${YELLOW}~/.config/dictatord/config.toml${NC}"
echo ""
echo -e "  ${BLUE}Logs:${NC}"
echo -e "    ${YELLOW}journalctl --user -u dictatord -f${NC}"
echo ""
echo -e "  ${BLUE}Uninstall:${NC}"
echo -e "    ${YELLOW}./uninstall.sh${NC}"
echo -e "    ${YELLOW}./uninstall.sh --purge${NC}  (removes config and cache too)"
echo ""
echo -e "  ${BLUE}Tips:${NC}"
echo -e "    - The first run will download the Whisper model (~75 MB)"
echo -e "    - Tap ${YELLOW}Super+D${NC} to start/stop"
echo -e "    - Hold ${YELLOW}Super+D${NC} for push-to-talk"
echo -e "    - Press ${YELLOW}Escape${NC} to cancel dictation"
echo ""

# Make the binary executable
chmod +x "$CARGO_BIN_DIR/$BINARY_NAME" 2>/dev/null || true
