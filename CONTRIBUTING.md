# Contributing to Bedrock Bridge

## Development Setup

### With Nix (recommended)

```bash
git clone https://github.com/your-org/Bedrock-Bridge.git
cd Bedrock-Bridge
nix develop
```

This gives you Rust (with Android targets), cargo-tauri, bun, Android SDK/NDK, and all system deps.

### Without Nix

1. Install [Rust](https://rustup.rs/) (1.77+, 1.95 recommended)
2. Install [Bun](https://bun.sh/)
3. Install Linux system deps:
   ```bash
   sudo apt install libwebkit2gtk-4.1-dev build-essential curl wget file \
     libssl-dev libgtk-3-dev libayatana-appindicator3-dev librsvg2-dev
   ```
4. Install `cargo-tauri`: `cargo install tauri-cli`

## Workspace Structure

```
crates/core/    — Core library (zero Tauri deps)
crates/cli/     — CLI binary (clap)
src-tauri/      — Tauri v2 GUI
  src/profile/  — Profile CRUD via tauri-plugin-store
  src/proxy/    — Proxy commands + auto-reconnect
  src/settings/ — Autostart, theme, export/import
src/            — Frontend (vanilla JS, dark/light theme)
```

**Rule**: `crates/core/` must never depend on Tauri. The GUI and CLI are thin consumers of the core library.

## Commands

```bash
# Run all tests (35 tests)
cargo test -p bedrock-bridge-core -p bedrock-bridge-cli

# Lint
cargo clippy --workspace -- -D warnings
cargo fmt --all -- --check

# Build CLI (fast, ~30s)
cargo build -p bedrock-bridge-cli

# Build Linux GUI (slow, ~30 min release)
cargo tauri build

# Build Android APK
cargo tauri android build

# Dev mode with hot reload
cargo tauri dev
```

## Coding Conventions

- **Rust edition 2024** across all crates
- **AGPL-3.0-only** license on all crates
- Every public function needs rustdoc comments
- Every `#[tauri::command]` must return `Result<_, String>`
- Error handling: use `anyhow` in binaries, `thiserror` in library crates
- No `unwrap()` or `expect()` outside of tests — use `?` and `map_err`
- All async code uses `tokio` runtime
- Desktop-only code gated with `#[cfg(desktop)]` (tray, menu, window hide)
- Profile persistence goes through `ProfileStore` trait (core) or `tauri-plugin-store` (GUI)

## Testing

- Unit tests live in the same file as the code (`#[cfg(test)] mod tests`)
- Integration tests live in `crates/*/tests/`
- The Tauri crate cannot run `cargo test` due to `generate_context!()` macro — test core logic in the core crate
- CLI integration tests use isolated `XDG_CONFIG_HOME` via `tempfile`

## PR Process

1. Fork and create a feature branch
2. Make your changes
3. Ensure all tests pass: `cargo test -p bedrock-bridge-core -p bedrock-bridge-cli`
4. Ensure lint passes: `cargo clippy --workspace -- -D warnings && cargo fmt --all -- --check`
5. Add tests for new functionality
6. Submit PR with a clear description

## Adding a New Feature

1. Implement the core logic in `crates/core/` with tests
2. Add CLI commands in `crates/cli/` if applicable
3. Add Tauri commands in `src-tauri/` if applicable
4. Update frontend in `src/` if applicable
5. Update README if user-facing

## Reporting Bugs

Include:
- OS and version
- Bedrock Bridge version (`bedrock-bridge --version`)
- Steps to reproduce
- Expected vs actual behavior
- Logs if available (`RUST_LOG=debug` for verbose output)
