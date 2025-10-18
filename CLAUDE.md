# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Overview

This is a monorepo for command-line utilities written in Rust for Unix environments (macOS and Linux). The repository uses Cargo workspaces to manage multiple independent CLI tools.

**Current tools:**
- gc - Automated git commit with AI-generated conventional commit messages using Claude CLI
- update-cli-programs - Automated installer/updater for all workspace binaries to ~/.local/bin

## Installation

Use the automated Rust installer to install all tools to ~/.local/bin:

```bash
cargo run -p update-cli-programs --release
```

## Development Commands

### Building
```bash
# Build all tools
cargo build --release

# Build specific program
cargo build -p gc --release
```

### Testing
```bash
# Run all tests (unit + integration)
cargo test

# Run tests for specific package
cargo test -p gc

# Run with output for debugging
cargo test -- --nocapture
```

## Architecture

### Workspace Structure

The repository uses Cargo workspaces defined in the root `Cargo.toml`.
Each program is a member workspace in its own directory with its own `Cargo.toml`.
Shared dependencies are defined at the workspace level in `[workspace.dependencies]`.

Each program has it's own readme and changelog:

- ./program-name/README.md
- ./program-name/CHANGELOG.md

Changelog's follow the keep a changelog format.

## Adding New Tools

1. Create new directory in workspace root
2. Add basic `Cargo.toml` with workspace dependencies
3. Add to `members` array in root `Cargo.toml`
4. Add a README.md for the project
5. Add a CHANGELOG.md for the project
6. Follow Rust 2024 edition conventions
