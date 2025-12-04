# track-changes

A CLI tool that watches directories and automatically commits changes with timestamps. Designed for automatic backup-style commits via macOS launchd.

## Installation

```bash
cargo install --path track-changes
# Or use the workspace installer:
cargo run -p update-cli-programs --release
```

## Usage

### Adding directories to watch

```bash
# Add a directory and immediately check for changes
track-changes --dir ~/Documents/notes

# Add a directory without committing
track-changes add ~/Projects/wiki
```

### Managing watched directories

```bash
# List all watched directories with status
track-changes list

# Remove a directory from the watch list
track-changes remove ~/Documents/notes
```

### Running manually

```bash
# Check all watched directories and commit any changes
track-changes

# Check a specific directory
track-changes --dir ~/Documents/notes
```

### Viewing commit history

```bash
# Show last 20 commits
track-changes log

# Show last 50 commits
track-changes log -c 50
```

### Scheduling (macOS)

```bash
# Install launchd plist for hourly auto-commits
track-changes install

# Remove the scheduled task
track-changes uninstall
```

## How it works

1. The tool maintains a list of directories to watch in `~/.config/cli-programs/track-changes.toml`
2. When run (manually or via launchd), it checks each directory for changes
3. If changes exist, it runs `git add -A` and commits with message `Auto-commit: <ISO timestamp>`
4. Commits are logged to `~/.local/share/track-changes/commits.log`

## Configuration

Config file location: `~/.config/cli-programs/track-changes.toml`

```toml
directories = [
    "/Users/username/Documents/notes",
    "/Users/username/.dotfiles"
]
```

## Log format

Commits are logged in JSON Lines format to `~/.local/share/track-changes/commits.log`:

```json
{"directory":"/Users/username/notes","timestamp":"2025-12-04T10:30:00-08:00","files_changed":["M notes.md"],"commit_hash":"abc1234"}
```

## Requirements

- macOS (for launchd scheduling)
- Git repositories must already be initialized in watched directories
- Does NOT push to remote by default (local commits only)
