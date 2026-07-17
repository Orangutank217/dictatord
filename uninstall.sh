#!/usr/bin/env bash
# =============================================================================
#  dictatord — Uninstall Script
# =============================================================================
# Removes dictatord and all installed files from the user's home directory.
# Usage:
#   ./uninstall.sh           — Remove binary + service, keep config
#   ./uninstall.sh --purge   — Remove everything including config & cache
# =============================================================================

set -euo pipefail

GREEN='\033[0;32m'
BLUE='\033[0;34m'
YELLOW='\033[1;33m'
RED='\033[0;31m'
NC='\033[0m'

BINARY_NAME="dictatord"
SERVICE_NAME="dictatord.service"
CARGO_BIN_DIR="$HOME/.cargo/bin"
SERVICE_DIR="$HOME/.config/systemd/user"
CONFIG_DIR="$HOME/.config/dictatord"
DATA_DIR="$HOME/.local/share/dictatord"
CACHE_DIR="$HOME/.cache/dictatord"

echo -e "${BLUE}╔════════════════════════════════════════╗${NC}"
echo -e "${BLUE}║     dictatord - Uninstall Script       ║${NC}"
echo -e "${BLUE}╚════════════════════════════════════════╝${NC}"
echo ""

# --- Stop and disable systemd service ---

if systemctl --user --quiet is-active "$SERVICE_NAME" 2>/dev/null; then
    echo -e "${YELLOW}[1/5] Stopping dictatord service...${NC}"
    systemctl --user stop "$SERVICE_NAME"
    echo -e "${GREEN}  ✓ Service stopped${NC}"
else
    echo -e "${YELLOW}[1/5] Service not running, skipping...${NC}"
fi

if systemctl --user --quiet is-enabled "$SERVICE_NAME" 2>/dev/null; then
    echo -e "${YELLOW}[2/5] Disabling dictatord service...${NC}"
    systemctl --user disable "$SERVICE_NAME"
    echo -e "${GREEN}  ✓ Service disabled${NC}"
else
    echo -e "${YELLOW}[2/5] Service not enabled, skipping...${NC}"
fi

# --- Remove binary ---

echo -e "${YELLOW}[3/5] Removing binary...${NC}"
BINARY_PATH="$CARGO_BIN_DIR/$BINARY_NAME"
if [ -f "$BINARY_PATH" ]; then
    rm -f "$BINARY_PATH"
    echo -e "${GREEN}  ✓ Removed $BINARY_PATH${NC}"
else
    echo -e "  ${YELLOW}Binary not found, skipping${NC}"
fi

# --- Remove systemd service file ---

echo -e "${YELLOW}[4/5] Removing systemd service file...${NC}"
SERVICE_PATH="$SERVICE_DIR/$SERVICE_NAME"
if [ -f "$SERVICE_PATH" ]; then
    rm -f "$SERVICE_PATH"
    echo -e "${GREEN}  ✓ Removed $SERVICE_PATH${NC}"
else
    echo -e "  ${YELLOW}Service file not found, skipping${NC}"
fi

systemctl --user daemon-reload 2>/dev/null || true

# --- Remove data directory (sounds, manifest) ---

echo -e "${YELLOW}[5/5] Removing data directory...${NC}"
if [ -d "$DATA_DIR" ]; then
    rm -rf "$DATA_DIR"
    echo -e "${GREEN}  ✓ Removed $DATA_DIR${NC}"
else
    echo -e "  ${YELLOW}Data directory not found, skipping${NC}"
fi

# --- Handle config and cache (keep or purge) ---

PURGE=false
for arg in "$@"; do
    case "$arg" in
        --purge|-p)
            PURGE=true
            ;;
    esac
done

if [ "$PURGE" = true ]; then
    echo ""
    echo -e "${YELLOW}--purge mode: removing config and cache...${NC}"
    
    if [ -d "$CONFIG_DIR" ]; then
        rm -rf "$CONFIG_DIR"
        echo -e "${GREEN}  ✓ Removed config: $CONFIG_DIR${NC}"
    fi
    
    if [ -d "$CACHE_DIR" ]; then
        rm -rf "$CACHE_DIR"
        echo -e "${GREEN}  ✓ Removed cache: $CACHE_DIR${NC}"
    fi
    
    echo ""
    echo -e "${GREEN}✓ dictatord fully purged from your system.${NC}"
else
    echo ""
    echo -e "  ${YELLOW}Config kept at:${NC} $CONFIG_DIR"
    echo -e "  ${YELLOW}Cache kept at:${NC}  $CACHE_DIR"
    echo ""
    echo -e "  To also remove config and cache, re-run:"
    echo -e "    ${BLUE}./uninstall.sh --purge${NC}"
fi

echo ""
echo -e "${GREEN}╔════════════════════════════════════════╗${NC}"
echo -e "${GREEN}║     dictatord has been uninstalled     ║${NC}"
echo -e "${GREEN}╚════════════════════════════════════════╝${NC}"
