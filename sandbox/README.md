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

This will:
1. Create a git worktree at `~/worktrees/<repo>-<name>`
2. Start a Docker sandbox with the worktree mounted
3. Launch Claude Code with `--dangerously-skip-permissions`

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

# Initialize default Dockerfile template
sandbox config init-template
```

## Custom Docker Templates

To use a custom Docker image with additional tools:

1. Initialize the template:
   ```bash
   sandbox config init-template
   ```

2. Edit `~/.config/cli-programs/sandbox-template/Dockerfile`

3. Set the template image name:
   ```bash
   sandbox config set template_image sandbox-dev
   ```

The CLI will automatically build the template when needed and rebuild when the Dockerfile changes.

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
- `~/.config/cli-programs/sandbox-template.hash` - Template build tracking
