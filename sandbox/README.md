# sandbox

A CLI tool for managing Claude Code development environments using git worktrees and Docker sandboxes.

## Overview

`sandbox` creates isolated development environments by combining:
- **Git worktrees**: Separate working directories from the same repository
- **Docker sandboxes**: Containerized Claude Code environments with `--dangerously-skip-permissions`

This enables fully autonomous Claude work in isolated environments while sharing your authentication and configuration.

## Installation

```bash
cargo install --path sandbox
# Or using the workspace installer:
cargo run -p update-cli-programs --release
```

## Prerequisites

- Docker Desktop with the sandbox extension installed
- Git

## Quick Start

```bash
# In a git repository - just works!
sandbox new feature-auth
```

On first run, `sandbox new` automatically:
1. Creates the default Dockerfile template
2. Builds the sandbox image (`sandbox-dev`)
3. Creates a git worktree at `~/worktrees/<repo>-<name>`
4. Starts a Docker sandbox with Claude Code

No configuration required - sensible defaults are used.

## Usage

### Create a new sandbox

```bash
# In a git repository
sandbox new feature-auth

# Specify a different repository
sandbox new feature-auth --repo ~/code/my-project

# Create from a specific branch
sandbox new bugfix-123 --branch develop
```

### Resume an existing sandbox

```bash
# Interactive selection
sandbox resume

# By name
sandbox resume feature-auth
```

### List all sandboxes

```bash
sandbox list
```

Output shows sandbox name, status, and path:
```
Available sandboxes:
------------------------------------------------------------
  1. feature-auth [running] - /Users/aaron/worktrees/my-project-feature-auth
  2. bugfix-123 [stopped] - /Users/aaron/worktrees/my-project-bugfix-123
------------------------------------------------------------
```

### Remove a sandbox

```bash
# Remove container only (keeps worktree)
sandbox remove feature-auth

# Remove container and worktree
sandbox remove feature-auth --worktree
```

## Configuration

Configuration is stored at `~/.config/cli-programs/sandbox.toml`:

```toml
# Directory where worktrees are created
worktree_dir = "~/worktrees"

# Custom Docker template image name (optional)
template_image = "sandbox-dev"

# Environment variables to pass to containers
[env]
GITHUB_TOKEN = "${GITHUB_TOKEN}"
NPM_TOKEN = "${NPM_TOKEN}"

# Additional volume mounts
[[mounts]]
source = "~/.ssh"
target = "/home/agent/.ssh"
readonly = true

[[mounts]]
source = "~/.gitconfig"
target = "/home/agent/.gitconfig"
readonly = true
```

### Configuration commands

```bash
# Show current configuration
sandbox config show

# Set configuration values
sandbox config set worktree_dir ~/dev/worktrees
sandbox config set template_image my-custom-template

# Create Dockerfile for customization
sandbox config create-dockerfile
```

## Custom Docker Templates

The default template is automatically created and built on first use. To customize:

### Option 1: Pre-configure before first sandbox

```bash
# Set custom worktree directory
sandbox config set worktree_dir /tmp/sandboxes

# Set custom template name
sandbox config set template_image my-sandbox

# Now create your first sandbox (uses your config)
sandbox new feature-auth
```

### Option 2: Customize the Dockerfile

```bash
# Create a Dockerfile you can edit
sandbox config create-dockerfile

# Edit ~/.config/cli-programs/sandbox/Dockerfile

# Next sandbox will auto-rebuild with your changes
sandbox new feature-auth
```

The CLI automatically rebuilds the template when the Dockerfile changes.

### Default template

The default template includes:
- Rust toolchain
- Node.js with pnpm
- Tauri development dependencies
- cargo-watch and cargo-expand

## How It Works

### Authentication

The CLI mounts `~/.claude` into the container, sharing your Claude authentication. Combined with `--credentials=none`, this uses your existing subscription without Docker's credential management.

### Worktree Management

Each sandbox creates a git worktree, providing:
- Independent working directory
- Shared git history with main repo
- Ability to work on multiple features simultaneously

### Container Lifecycle

- Containers are named based on the workspace path hash
- `resume` auto-starts stopped containers
- `remove` cleans up containers (worktree removal is optional)

## State Files

- `~/.config/cli-programs/sandbox.toml` - Configuration
- `~/.config/cli-programs/sandbox-state.json` - Worktree tracking
- `~/.config/cli-programs/sandbox/Dockerfile` - User's custom Dockerfile template
- `~/.config/cli-programs/sandbox-template.hash` - Template build tracking
