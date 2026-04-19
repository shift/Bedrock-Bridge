# ⛏️ Bedrock Bridge

A UDP relay for Minecraft Bedrock Edition. Forward local console traffic to any remote Bedrock server — consoles discover it as a LAN game.

**AGPL-3.0 Licensed**

## What It Does

- **Discovery**: Responds to RakNet ping on UDP 19132 with a custom MOTD — consoles see it as a LAN game instantly
- **Relay**: Bidirectionally forwards all non-discovery UDP packets between local clients and the remote server
- **MTU Management**: Caps RakNet connection MTU at 1400 bytes to avoid fragmentation issues
- **Auto-Reconnect**: Exponential backoff retry when remote server is unreachable (up to 10 retries)
- **Real-Time Stats**: Live traffic sparkline graph, PPS counters, bandwidth totals, connected client list

## Architecture

```
┌─────────────┐     UDP 19132     ┌──────────────────┐     UDP any     ┌──────────────┐
│  Minecraft   │ ──────────────▶  │  Bedrock Bridge  │ ──────────────▶ │ Remote Server│
│  Console     │ ◀──────────────  │  (Proxy)         │ ◀────────────── │ (Bedrock)    │
└─────────────┘   pong/response   └──────────────────┘   response      └──────────────┘
                                    │
                                    ├─ 0x01 Ping → custom 0x1c Pong (MOTD)
                                    └─ Other packets → forward + MTU cap
```

### Workspace Layout

```
bedrock-bridge/
├── crates/
│   ├── core/        # Library crate — zero Tauri deps
│   │   ├── discovery # RakNet ping/pong, MOTD builder, MTU capping
│   │   ├── profile   # Profile model + ProfileStore trait + JsonFileStore
│   │   └── proxy     # Two-socket bidirectional UDP relay
│   └── cli/          # CLI binary (clap)
├── src-tauri/        # Tauri v2 GUI (thin shim over core)
│   ├── src/
│   │   ├── profile/  # Profile CRUD via tauri-plugin-store
│   │   ├── proxy/    # Proxy commands + auto-reconnect + stats forwarding
│   │   └── settings/ # Autostart, theme, export/import
│   └── gen/android/  # Android Kotlin bridge (Foreground Service, MulticastLock)
├── src/              # Frontend (vanilla JS, dark/light theme, sparkline graph)
├── icon.svg          # Source icon (pickaxe + bridge arc)
└── flake.nix         # Nix dev shell (Linux + Android cross-compilation)
```

## Features

| Feature | Description |
|---------|-------------|
| **System Tray** | Minimize to tray, keep proxy running in background |
| **Auto-Reconnect** | Exponential backoff (1s→30s) with status events |
| **Traffic Sparkline** | 30-second canvas graph of PPS in/out |
| **Client List** | Shows connected console IP addresses in real-time |
| **Settings Page** | Theme toggle, auto-start on login, export/import |
| **Export/Import** | Save/load profiles as JSON via native file dialogs |
| **Custom Icon** | Pickaxe + bridge arc on dark background |
| **Dark/Light Theme** | CSS variable theming with localStorage persistence |

## Quick Start

### Prerequisites

- [Nix](https://nixos.org/) with flake support, or:
  - Rust 1.77+ (1.95 recommended)
  - Node.js 22+ and Bun
  - GTK3, webkitgtk 4.1, and other [Tauri deps](https://v2.tauri.app/start/prerequisites/#linux)

### With Nix (recommended)

```bash
# Enter dev shell (provides Rust, cargo-tauri, bun, system deps, Android SDK)
nix develop

# Build Linux GUI
cargo tauri build

# Build Android APK
cargo tauri android init    # first time only
cargo tauri android build
```

### Without Nix

#### Linux (Ubuntu/Debian)

```bash
# Install Rust
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# Install system deps
sudo apt install libwebkit2gtk-4.1-dev build-essential curl wget file \
  libssl-dev libgtk-3-dev libayatana-appindicator3-dev librsvg2-dev

# Install bun
curl -fsSL https://bun.sh/install | bash

# Build
bun install && bun run build
cargo install tauri-cli
cargo tauri build
# Output: target/release/bundle/deb/*.deb
```

#### macOS

```bash
# Install Rust
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# Install bun
brew install bun

# Build (no extra system deps needed on macOS)
bun install && bun run build
cargo install tauri-cli
cargo tauri build
# Output: target/release/bundle/dmg/*.dmg, target/release/bundle/macos/*.app
```

#### Windows

```powershell
# Install Rust
winget install Rustlang.Rustup

# Install bun
powershell -c "irm bun.sh/install.ps1 | iex"

# Install Visual Studio C++ Build Tools
# Download from https://visualstudio.microsoft.com/visual-cpp-build-tools/
# Select "Desktop development with C++" workload

# Build
bun install && bun run build
cargo install tauri-cli
cargo tauri build
# Output: target/release/bundle/msi/*.msi
```

### Cross-Compilation

To build for a platform you're not on, use CI (GitHub Actions). The `.github/workflows/ci.yml` can be extended with matrix builds for macOS and Windows. Alternatively, use a macOS/Windows machine or VM.

## Usage

### GUI

Launch the app, add a server profile (label, host, port), then toggle the switch to activate. The console on the same network will see the server appear in LAN games.

**Keyboard shortcuts**: Escape cancels forms, Enter saves.

**CLI flags**:
- `--hidden` — start minimized to system tray

### CLI

```bash
# Add a server profile
bedrock-bridge profiles add --label "My Server" --host 192.168.1.50 --port 19132

# List profiles
bedrock-bridge profiles list

# Start proxy directly
bedrock-bridge run --label "My Server" --host 192.168.1.50 --port 19132

# Start proxy using a saved profile
bedrock-bridge profiles start "My Server"

# Remove a profile
bedrock-bridge profiles remove "My Server"
```

The CLI prints live traffic stats (PPS, throughput, active sessions) to stdout. Press Ctrl+C to stop.

### Port Binding

The proxy binds to UDP 19132. On Linux you may need:

```bash
# Option 1: Run with capabilities
sudo setcap cap_net_bind_service=+ep ./target/release/bedrock-bridge

# Option 2: Just use sudo
sudo ./target/release/bedrock-bridge
```

## Android

The Android build requires the Nix dev shell (provides Android SDK/NDK):

```bash
nix develop
cargo tauri android init     # generates Gradle project
cargo tauri android build    # produces APK
```

The Kotlin bridge provides:
- **MulticastLock** — allows UDP broadcast on WiFi
- **Foreground Service** — keeps proxy alive in background
- **WakeLock** — prevents CPU sleep during active relay

## Development

```bash
nix develop

# Run tests (35 tests)
cargo test -p bedrock-bridge-core -p bedrock-bridge-cli

# Lint
cargo clippy --workspace -- -D warnings
cargo fmt --all -- --check

# Build CLI only (fast)
cargo build -p bedrock-bridge-cli

# Build GUI (slow, ~30 min release)
cargo tauri build

# Dev mode with hot reload
cargo tauri dev
```

## CI

GitHub Actions pipeline (`.github/workflows/ci.yml`):
1. **Lint** — rustfmt + clippy
2. **Test** — cargo test (core + CLI)
3. **Build** — cargo tauri build (uploads .deb + binary as artifacts)

## License

AGPL-3.0-only. See [LICENSE](./LICENSE).
