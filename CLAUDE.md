# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

`dropkitten` is a Rust utility for creating Sway window manager dropdown terminals. It spawns a floating terminal window (kitty) with customizable dimensions and positioning, and automatically closes when focus is lost.

### Core Architecture

- **Single-file application**: All logic is in `src/main.rs`
- **Sway IPC integration**: Uses `swayipc` crate for window manager communication
- **Async runtime**: Built on tokio for handling Sway events and IPC
- **CLI interface**: Uses clap for argument parsing with flexible sizing options

### Key Components

- **Size handling**: Supports both pixel (`100`) and fractional (`0.3`) sizing
- **Window management**: Applies Sway rules for floating, positioning, and sizing
- **Focus monitoring**: Watches for window focus changes to auto-close dropdown
- **Mouse warping**: Temporarily adjusts mouse behavior during dropdown lifecycle

## Development Commands

### Build and Run
```bash
# Build in debug mode
cargo build

# Build optimized release
cargo build --release

# Run with default terminal
cargo run

# Run with custom command
cargo run -- zsh
cargo run -- -W 0.5 -H 0.6 -- nvim
```

### Testing and Quality
```bash
# Run tests
cargo test

# Format code
cargo fmt

# Lint code
cargo clippy

# Check all targets
cargo clippy --all-targets
```

### Nix Development
```bash
# Enter development shell
nix develop

# Build via Nix
nix build

# Check flake
nix flake check
```

The project uses vendored dependencies in the `vendor/` directory and includes a Nix flake for reproducible builds with a complete Rust development environment.

## Dependencies

- `swayipc`: Sway window manager IPC communication
- `tokio`: Async runtime for event handling
- `clap`: Command-line argument parsing
- `shell-escape`: Safe shell command escaping
- `thiserror`: Error handling
- `futures`: Stream processing
- `regex`: Configuration parsing