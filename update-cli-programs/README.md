# update-cli-programs

A Rust-based installer/updater for cli-programs binaries.

## Overview

This tool automates the installation and updating of all Rust CLI tools in this workspace to a target directory (defaults to `~/.local/bin`). It:

1. Reads the workspace members from the root `Cargo.toml`
2. Builds all tools in release mode (including itself)
3. Copies binaries to the target directory
4. Makes them executable (755 permissions)

## Usage

From the repository root:

```bash
# Install/update to default location (~/.local/bin)
cargo run -p update-cli-programs --release

# Install/update to custom location
cargo run -p update-cli-programs --release -- --target /usr/local/bin
```

## Requirements

- Rust toolchain (cargo)
- Unix-like environment (macOS, Linux)

## What Gets Installed

The installer automatically discovers and installs all workspace members.

The `changelog-validator` member is excluded.

## Target Directory

By default, binaries are installed to `~/.local/bin`.

`~/.local/bin` will need to be in your `PATH`. Most modern Linux distributions include it by default, but you may need to add it manually:

```bash
export PATH="$HOME/.local/bin:$PATH"
```

Add this to your shell configuration file (`~/.bashrc`, `~/.zshrc`, etc.) to make it permanent.

You can install to a different location using the `--target` flag:

```bash
cargo run -p update-cli-programs --release -- --target /usr/local/bin
```
