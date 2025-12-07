# sandy

A CLI tool for managing Claude Code development environments using Docker containers.

## Overview

`sandy` creates isolated development environments by combining:
- **Docker containers**: Containerized Claude Code environments with `--dangerously-skip-permissions`
- **Persistent credentials**: Claude auth persists across sandboxes via Docker's managed volume

This enables fully autonomous Claude work in isolated environments while sharing your authentication and configuration.

## Installation

```bash
cargo install --path sandy
# Or using the workspace installer:
cargo run -p update-cli-programs --release
```

## Prerequisites

- Docker Desktop with the sandbox extension installed
- Git

## Quick Start

```bash
# Run without arguments for interactive mode
sandy

# Or use direct commands
sandy new
```

On first run, `sandy` automatically:
1. Creates the default Dockerfile template
2. Copies binaries from `~/.local/bin` into the template
3. Builds the sandy image (`sandy-dev`)
4. Starts a Docker container with Claude Code

No configuration required - sensible defaults are used.

## Usage

### Create a new sandbox

```bash
# In a git repository
sandy new
```

### Resume an existing sandbox

```bash
# Interactive selection
sandy resume
```

### List all sandboxes

```bash
sandy list
```

Output shows sandbox name, status, and path:
```
Available sandboxes:
------------------------------------------------------------
  1. my-project [running] - /Users/aaron/code/my-project
  2. other-project [stopped] - /Users/aaron/code/other-project
------------------------------------------------------------
```

### Remove a sandbox

```bash
# Interactive selection
sandy remove
```

## Configuration

Configuration is stored at `~/.config/cli-programs/sandy.toml`:

```toml
# Custom Docker template image name (optional)
template_image = "sandy-dev"

# Directories containing binaries to include in the template image
# All executable files from these directories are copied into the Docker image
binary_dirs = ["~/.local/bin"]

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
sandy config show

# Set configuration values
sandy config set template_image my-custom-template

# Create Dockerfile for customization
sandy config create-dockerfile
```

## Custom Docker Templates

The default template is automatically created and built on first use. To customize:

### Option 1: Pre-configure before first sandbox

```bash
# Set custom template name
sandy config set template_image my-sandy

# Now create your first sandbox (uses your config)
sandy new
```

### Option 2: Customize the Dockerfile

```bash
# Create a Dockerfile you can edit
sandy config create-dockerfile

# Edit ~/.config/cli-programs/sandy/Dockerfile

# Next sandbox will auto-rebuild with your changes
sandy new
```

The CLI automatically rebuilds the template when the Dockerfile changes.

### Default template

The default template includes:
- Rust toolchain
- Node.js with pnpm
- Tauri development dependencies
- cargo-watch and cargo-expand
- Java 17 (OpenJDK) with Maven
- All executables from configured `binary_dirs` (default: `~/.local/bin`)

## How It Works

### Authentication

Sandy uses `--credentials=sandbox` which stores Claude authentication in a persistent Docker volume (`docker-claude-sandbox-data`). This means:
- First sandbox prompts for authentication once
- All subsequent sandboxes automatically use the stored credentials
- Credentials persist across sandbox restarts and deletion

The `~/.claude` directory is also mounted for custom settings and configuration.

### Container Lifecycle

- Containers are named based on the workspace path hash
- `resume` auto-starts stopped containers
- `remove` cleans up containers

## State Files

- `~/.config/cli-programs/sandy.toml` - Configuration
- `~/.config/cli-programs/sandy-state.json` - Sandbox tracking
- `~/.config/cli-programs/sandy/Dockerfile` - User's custom Dockerfile template
- `~/.config/cli-programs/sandy-template.hash` - Template build tracking
