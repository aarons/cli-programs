# update-cli-programs

A Rust-based installer/updater for cli-programs binaries.

## Overview

This tool automates the installation and updating of all Rust CLI tools in this workspace to a target directory (defaults to `~/code/bin`). It:

1. Reads the workspace members from the root `Cargo.toml`
2. Builds all tools in release mode (including itself)
3. Copies binaries to the target directory
4. Makes them executable (755 permissions)

## Usage

From the repository root:

```bash
# Install/update to default location (~/code/bin)
cargo run -p update-cli-programs --release

# Install/update to custom location
cargo run -p update-cli-programs --release -- --target /usr/local/bin
```

## Requirements

- Rust toolchain (cargo)
- Unix-like environment (macOS, Linux)

## What Gets Installed

The installer automatically discovers and installs all workspace members. Currently:

- `gc` - Git commit automation tool
- `update-cli-programs` - This installer/updater itself

As new tools are added to the workspace, they will automatically be included in the installation process.

## Target Directory

By default, binaries are installed to `~/code/bin`. Make sure this directory is in your `PATH`:

```bash
export PATH="$HOME/code/bin:$PATH"
```

Add this to your shell configuration file (`~/.bashrc`, `~/.zshrc`, etc.) to make it permanent.
