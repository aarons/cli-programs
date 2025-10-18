# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Overview

This is a monorepo for command-line utilities written in Rust for Unix environments (macOS and Linux). The repository uses Cargo workspaces to manage multiple independent CLI programs.

**Installable programs:**
- ask - Claude CLI wrapper for shell commands and questions
- gc - Automated git commit with AI-generated conventional commit messages using Claude CLI
- git-clean - Interactive tool to clean up local and remote Git branches
- update-cli-programs - Automated installer/updater for all workspace binaries to ~/.local/bin

**Development/repository tools:**
- changelog-validator - Validates CHANGELOG.md files across all workspace projects (not installed, run via `cargo test -p changelog-validator`)

## Installation

Use the automated Rust installer to install all programs to ~/.local/bin:

```bash
cargo run -p update-cli-programs --release
```

## Development Commands

### Building
```bash
# Build all programs
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

Changelogs must follow a strict format validated by `changelog-validator`. See changelog-validator/README.md for detailed schema rules and examples.

## Adding New Programs

1. Create new directory in workspace root
2. Add basic `Cargo.toml` with workspace dependencies
3. Add to `members` array in root `Cargo.toml`
4. Add a README.md for the project
5. Add a CHANGELOG.md for the project (see gc/CHANGELOG.md for valid template)
6. Run `cargo test -p changelog-validator` to verify changelog format
7. Follow Rust 2024 edition conventions
