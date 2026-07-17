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

Choose the path that's easiest for you:

### 🚀 One-liner (from source, requires Rust)

```bash
git clone https://github.com/Orangutank217/dictatord.git && cd dictatord && ./install.sh
```

### 📦 One-liner (pre-built binary, no Rust needed)

_Available once the first release build completes:_

```bash
curl -L https://github.com/Orangutank217/dictatord/releases/latest/download/dictatord-v0.1.0-x86_64-linux.tar.gz | tar xz && cd dictatord-* && ./install.sh
```

### 🔧 Step by step

<details>
<summary>Click to expand for manual install instructions</summary>

**1. Install system dependencies**

| Distro | Command |
|--------|---------|
| Ubuntu / Debian | `sudo apt install libgtk-3-dev libx11-dev libxdo-dev libasound2-dev pkg-config` |
| Fedora | `sudo dnf install gtk3-devel libX11-devel libxdo-devel alsa-lib-devel pkgconfig` |
| Arch | `sudo pacman -S gtk3 libx11 libxdo alsa-lib pkg-config` |
| openSUSE | `sudo zypper install gtk3-devel libX11-devel libxdo-devel alsa-devel pkg-config` |

**2. Build & install**

```bash
git clone https://github.com/Orangutank217/dictatord.git
cd dictatord
./install.sh
```

The script checks for missing deps (offers to install them), builds the binary, copies it to `~/.cargo/bin/`, creates a default config, and optionally enables the systemd service.

</details>

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

## First Steps

### 🔑 1. Choose your hotkey

After installing, **the first thing you should do** is pick a hotkey that works for you:

```bash
dictatord --settings
```

This opens an interactive menu. Select **Hotkey** → **Key** and enter your preferred combination. Or edit `~/.config/dictatord/config.toml` directly:

```toml
[hotkey]
key = "§"   # ← Change this to whatever you want (e.g. "Ctrl+Shift+D", "F2", etc.)
mode = "both"
```

See the [Configuration](#configuration) section for all available options.

### 🎤 2. Start dictating

```bash
# Start the daemon
dictatord

# Or run it as a background service
systemctl --user start dictatord
```

Press your hotkey and speak! The transcribed text appears at your cursor.

### 📖 3. Learn the modes

| Mode | How it works |
|------|-------------|
| `toggle` | Press hotkey to start recording, press again to stop & transcribe |
| `ptt` (push-to-talk) | Hold hotkey while speaking, release to transcribe |
| `both` | Quick tap = toggle, hold down = push-to-talk (best of both) |

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
key = "§"                # ← CHANGE THIS to any key you want
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
