# =============================================================================
#  dictatord — Makefile
# =============================================================================
# Targets:
#   make build       — Build the release binary
#   make install     — Build + install to user home directories
#   make uninstall   — Remove installed files (keeps config)
#   make purge       — Remove everything including config & cache
#   make clean       — Clean cargo build artifacts
#   make help        — Show this help message
# =============================================================================

BINARY_NAME    := dictatord
CARGO_BIN_DIR  := $(HOME)/.cargo/bin
CONFIG_DIR     := $(HOME)/.config/$(BINARY_NAME)
DATA_DIR       := $(HOME)/.local/share/$(BINARY_NAME)
SOUNDS_DIR     := $(DATA_DIR)/sounds
SERVICE_DIR    := $(HOME)/.config/systemd/user
MANIFEST_FILE  := $(DATA_DIR)/install_manifest.txt

.PHONY: all build install uninstall purge clean help

all: build

# --- Build ---

build:
	cargo build --release

# --- Install ---

install: build
	@echo ""
	@echo "  === Installing dictatord ==="
	@echo ""
	
	# Create directories
	mkdir -p "$(CARGO_BIN_DIR)" "$(CONFIG_DIR)" "$(SOUNDS_DIR)" "$(SERVICE_DIR)"
	
	# Copy binary
	cp target/release/$(BINARY_NAME) "$(CARGO_BIN_DIR)/$(BINARY_NAME)"
	@echo "  ✓ Binary installed -> $(CARGO_BIN_DIR)/$(BINARY_NAME)"
	
	# Copy config (don't overwrite existing)
	if [ ! -f "$(CONFIG_DIR)/config.toml" ]; then \
		cp config/default.toml "$(CONFIG_DIR)/config.toml"; \
		echo "  ✓ Default config created -> $(CONFIG_DIR)/config.toml"; \
	else \
		echo "  - Config exists, keeping yours at $(CONFIG_DIR)/config.toml"; \
	fi
	
	# Copy sound files
	cp sounds/*.ogg "$(SOUNDS_DIR)/"
	@echo "  ✓ Sound files installed -> $(SOUNDS_DIR)"
	
	# Copy systemd service
	cp systemd/dictatord.service "$(SERVICE_DIR)/dictatord.service"
	@echo "  ✓ Systemd service installed -> $(SERVICE_DIR)/dictatord.service"
	systemctl --user daemon-reload
	
	# Write install manifest
	@echo "Writing install manifest..."
	@printf '%s\n' \
		"$(CARGO_BIN_DIR)/$(BINARY_NAME)" \
		"$(CONFIG_DIR)/config.toml" \
		"$(SOUNDS_DIR)/start.ogg" \
		"$(SOUNDS_DIR)/stop.ogg" \
		"$(SERVICE_DIR)/dictatord.service" \
		> "$(MANIFEST_FILE)"
	@echo "  ✓ Manifest written -> $(MANIFEST_FILE)"
	
	@echo ""
	@echo "  ────────────────────────────────────────────"
	@echo "  ✅ dictatord installed!"
	@echo ""
	@echo "  Start dictation with:  $(BINARY_NAME)"
	@echo "  Enable on login:       systemctl --user enable --now $(BINARY_NAME)"
	@echo "  View logs:             journalctl --user -u $(BINARY_NAME) -f"
	@echo "  Uninstall:             make uninstall"
	@echo "  ────────────────────────────────────────────"
	@echo ""

# --- Uninstall (keep config) ---

uninstall:
	@echo ""
	@echo "  === Uninstalling dictatord ==="
	@echo ""
	
	# Stop & disable service
	-systemctl --user stop $(BINARY_NAME) 2>/dev/null
	-systemctl --user disable $(BINARY_NAME) 2>/dev/null
	
	# Remove binary
	rm -f "$(CARGO_BIN_DIR)/$(BINARY_NAME)"
	@echo "  ✓ Removed binary"
	
	# Remove service file
	rm -f "$(SERVICE_DIR)/dictatord.service"
	@echo "  ✓ Removed systemd service"
	systemctl --user daemon-reload 2>/dev/null || true
	
	# Remove data dir (sounds, manifest)
	rm -rf "$(DATA_DIR)"
	@echo "  ✓ Removed data directory"
	
	@echo ""
	@echo "  Config kept at: $(CONFIG_DIR)"
	@echo "  Cache kept at:  $(HOME)/.cache/$(BINARY_NAME)"
	@echo "  To also remove those, run:  make purge"
	@echo ""

# --- Purge (remove everything) ---

purge: uninstall
	@echo "  === Purging config and cache ==="
	rm -rf "$(CONFIG_DIR)"
	@echo "  ✓ Removed config: $(CONFIG_DIR)"
	rm -rf "$(HOME)/.cache/$(BINARY_NAME)"
	@echo "  ✓ Removed cache: $(HOME)/.cache/$(BINARY_NAME)"
	@echo ""
	@echo "  ✅ dictatord fully purged."
	@echo ""

# --- Clean build artifacts ---

clean:
	cargo clean
	@echo "  ✓ Build artifacts cleaned"

# --- Help ---

help:
	@echo "Usage: make [target]"
	@echo ""
	@echo "Targets:"
	@echo "  build       Build the release binary (default)"
	@echo "  install     Build and install to user home directories"
	@echo "  uninstall   Remove installed files (keeps config)"
	@echo "  purge       Remove everything including config and cache"
	@echo "  clean       Remove cargo build artifacts"
	@echo "  help        Show this help message"
