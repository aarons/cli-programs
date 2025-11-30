# Changelog

## [0.2.1] - 2025-11-30

### Changed
- User's Dockerfile template now stored at `~/.config/cli-programs/sandbox/Dockerfile` (was `sandbox-template/Dockerfile`)
- Default Dockerfile loaded from source `template/Dockerfile` at compile time via `include_str!`

## [0.2.0] - 2025-11-30

### Changed
- `sandbox new` is now self-sufficient and works without prior configuration
- Renamed `sandbox config init-template` to `sandbox config create-dockerfile`
- Template is automatically created and built on first `sandbox new` if not present

### Added
- Default template image name (`sandbox-dev`) used when no custom template configured
- Auto-setup of sandbox template on first use

## [0.1.0] - 2025-11-29

### Added
- Initial release of sandbox CLI for Claude Code development environments
- `sandbox new <name>` command to create new sandbox environments with git worktrees
- `sandbox resume [name]` command to resume existing sandboxes (interactive selection if no name provided)
- `sandbox list` command to show all sandboxes with status (running/stopped/no container)
- `sandbox remove <name>` command to remove sandboxes (with optional `--worktree` flag)
- `sandbox config show` command to display current configuration
- `sandbox config set <key> <value>` command to modify configuration
- `sandbox config create-dockerfile` command to create Dockerfile template for customization
- Configuration file at `~/.config/cli-programs/sandbox.toml`
- State tracking at `~/.config/cli-programs/sandbox-state.json`
- Support for custom Docker templates with automatic rebuild on Dockerfile changes
- Default mounts for `~/.ssh` and `~/.gitconfig`
- Environment variable expansion in configuration values
- Auto-mount of `~/.claude` for authentication passthrough
