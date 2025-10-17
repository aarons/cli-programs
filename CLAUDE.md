# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Overview

This is a monorepo for command-line utilities written in Rust for Unix environments (macOS and Linux). The repository uses Cargo workspaces to manage multiple independent CLI tools.

**Current tools:**
- gc - Automated git commit with AI-generated conventional commit messages using Claude CLI

## Development Commands

### Building
```bash
# Build all tools
cargo build --release

# Build specific tool
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
Each tool is a member workspace in its own directory with its own `Cargo.toml`.
Shared dependencies are defined at the workspace level in `[workspace.dependencies]`.

### Tool-Specific Documentation

Each tool has its own README.md with details.
When working on a specific tool, you MUST refer to its README for context:

- gc - `gc/README.md`

## Adding New Tools

1. Create new directory in workspace root
2. Add basic `Cargo.toml` with workspace dependencies
3. Add to `members` array in root `Cargo.toml`
4. Update README.md installation section
5. Follow Rust 2024 edition conventions
