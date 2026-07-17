# dictatord

> Native speech-to-text daemon for Linux with waveform visualization.

[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)
[![Rust](https://img.shields.io/badge/Rust-1.75%2B-orange)](https://www.rust-lang.org)
[![CI](https://github.com/Orangutank217/dictatord/actions/workflows/ci.yml/badge.svg)](https://github.com/Orangutank217/dictatord/actions/workflows/ci.yml)

---

## Features

- **Whisper-powered speech recognition** — runs locally, no cloud API needed
- **Push-to-talk, toggle, or both** — configurable hotkey modes
- **Waveform visualizer** — pulsing GTK overlay shows when you're being heard
- **Noise suppression** — WebRTC-based filtering for cleaner transcription
- **Pre-roll buffer** — captures audio from before you press the hotkey (no clipped words)
- **Auto-type at cursor** — transcribed text is typed wherever your cursor is
- **Live settings** — change config without restarting via IPC

## Demo

![dictatord visualizer](https://via.placeholder.com/400x80/1a1a1f/00ccff?text=dictatord+visualizer)

*A pulsing cyan orb with a listening indicator — appears at the top-right of your screen.*

---

## Quick Install

### Prerequisites

You need:
- **Rust toolchain** (1.75+) — install from [rustup.rs](https://rustup.rs)
- **System development libraries** (see table below)

| Distro | Install Command |
|--------|----------------|
| **Ubuntu / Debian** | `sudo apt install libgtk-3-dev libx11-dev libxdo-dev libasound2-dev pkg-config` |
| **Fedora** | `sudo dnf install gtk3-devel libX11-devel libxdo-devel alsa-lib-devel pkgconfig` |
| **Arch** | `sudo pacman -S gtk3 libx11 libxdo alsa-lib pkg-config` |
| **openSUSE** | `sudo zypper install gtk3-devel libX11-devel libxdo-devel alsa-devel pkg-config` |

### Install from source

```bash
# Clone (or download the release tarball)
git clone https://github.com/Orangutank217/dictatord.git
cd dictatord

# Run the install script
./install.sh
```

Or using `make`:

```bash
make && make install
```

The script will:
1. Check for missing dependencies (and offer to install them)
2. Build the binary
3. Install to `~/.cargo/bin/`
4. Create default config at `~/.config/dictatord/config.toml`
5. Install the systemd user service
6. Ask whether to enable the service for auto-start on login

### Install from pre-built binary

Download the latest tarball from the [Releases page](https://github.com/Orangutank217/dictatord/releases):

```bash
curl -L https://github.com/Orangutank217/dictatord/releases/latest/download/dictatord-x86_64-linux.tar.gz | tar xz
./install.sh
```

---

## Uninstall

```bash
cd dictatord
./uninstall.sh          # remove binary + service, keep config
./uninstall.sh --purge  # remove everything including config and cache
```

Or with `make`:

```bash
make uninstall   # remove binary + service, keep config
make purge       # remove everything
```

---

## Usage

### Start the daemon

```bash
# Start manually
dictatord

# Or as a background service
systemctl --user start dictatord
systemctl --user enable dictatord   # auto-start on login
```

### Dictation

Press **Super+D** to start/stop dictation (configurable).

| Mode | Behavior |
|------|----------|
| `toggle` | Press to start, press again to stop |
| `ptt` | Hold to record, release to transcribe |
| `both` | Tap to toggle, hold for PTT |

### View logs

```bash
journalctl --user -u dictatord -f
```

### CLI options

```
Usage: dictatord [OPTIONS]

Options:
      --debug      Print debug logs
      --settings   Open interactive settings menu (instead of running the daemon)
      --help       Print help
      --version    Print version
```

### Interactive settings

```bash
dictatord --settings
```

Opens a terminal menu to change audio, ASR, output, and visualizer settings on the fly.

---

## Configuration

Config file: `~/.config/dictatord/config.toml`

```toml
[hotkey]
key = "Super+D"
mode = "both"            # "toggle" | "ptt" | "both"

[audio]
device = "default"
sample_rate = 16000
pre_roll_ms = 2000       # captures audio from 2s before pressing

[asr]
model = "tiny"           # "tiny" (fast) | "base" | "small" (accurate)
language = "en"
gpu_offload = false
num_threads = 4

[output]
method = "xdotool"
capitalize = true
add_trailing_space = true
typing_delay_ms = 5

[visual]
enabled = true
position = "top-right"
color_hex = "#00CCFF"
opacity = 0.85
```

---

## Building from source

```bash
git clone https://github.com/Orangutank217/dictatord.git
cd dictatord
cargo build --release
```

### CUDA acceleration

If you have an NVIDIA GPU and the CUDA toolkit installed:

```bash
cargo build --release --features cuda
```

Then set `gpu_offload = true` in the config for faster transcription.

---

## Requirements

- **Linux** with X11 (Wayland is not yet supported)
- **PulseAudio** or PipeWire (with PulseAudio compatibility)
- **Rust** 1.75+ for building from source

---

## Project structure

```
dictatord/
├── src/
│   ├── main.rs       — Daemon entry point & event loop
│   ├── asr.rs        — Whisper speech-to-text engine
│   ├── audio.rs      — Audio capture, VAD, noise suppression
│   ├── config.rs     — Configuration loading & live updates
│   ├── feedback.rs   — Sound feedback (start/stop/error)
│   ├── hotkey.rs     — X11 global hotkey listener
│   ├── ipc.rs        — Unix socket IPC for live settings
│   ├── output.rs     — Text output via libxdo
│   ├── settings.rs   — Interactive settings TUI
│   └── visual/       — GTK waveform visualizer
├── config/
│   └── default.toml  — Default configuration
├── sounds/           — Feedback sounds
├── systemd/
│   └── dictatord.service
├── install.sh        — Install script
├── uninstall.sh      — Uninstall script
└── Makefile          — Build & install targets
```

---

## License

[MIT](LICENSE) &copy; 2026 Orangutank217
