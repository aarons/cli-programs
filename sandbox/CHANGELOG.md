# Changelog

## [0.3.0] - 2025-12-01

### Added
- Interactive mode when run without subcommands (menu-driven interface)
- `binary_dirs` config option to specify directories containing binaries to include in template
- Default `binary_dirs` is `["~/.local/bin"]`

### Changed
- Template now copies all executables from `binary_dirs` instead of building specific binaries from cargo workspace
- Simplified build process - no longer requires being in the cli-programs workspace
- Dockerfile template uses `COPY assets/bin/` for all binaries instead of individual COPY commands

### Removed
- Dependency on cargo workspace location during template build
- Hardcoded binary list (`TEMPLATE_BINARIES`)

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
