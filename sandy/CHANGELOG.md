# Changelog

## [1.6.0] - 2025-12-22

### Added
- `sandy update` command to manually update Dockerfile to latest default template
- `--force` flag to override customized Dockerfiles (creates date-stamped backup)
- Auto-detection during build when a new default Dockerfile template is available

## [1.5.0] - 2025-12-05

### Changed
- Switched from `--credentials=host` to `--credentials=sandbox` for Docker sandbox credential management
- Claude authentication now persists in Docker's managed volume, shared across all sandboxes
- First-time users authenticate once; subsequent sandboxes automatically use stored credentials
- JAVA_HOME now uses architecture-detected symlink (`/usr/lib/jvm/java-17`) instead of `/usr/lib/jvm/default-java`

## [1.4.0] - 2025-12-04

### Fixed
- Sandboxes now use the exact Docker image digest instead of the image name, fixing an issue where Docker Sandbox would use stale cached templates even after rebuilding

## [1.3.0] - 2025-12-04

### Added
- `sandy build` command to manually build or rebuild the template image
- `--force` flag to rebuild from scratch, ignoring Docker's build cache

### Fixed
- JAVA_HOME now uses architecture-agnostic path (`/usr/lib/jvm/default-java`) instead of hardcoded amd64 path

## [1.2.0] - 2025-12-04

### Changed
- Container names now include the directory name for easier identification (e.g., `sandy-cli-programs-e1664b` instead of `sandy-e1664bd8231b`)

## [1.1.0] - 2025-12-04

### Added
- `SANDY_CONFIG_DIR` environment variable to override the config directory path

### Changed
- Improved test isolation by separating I/O from business logic in template status checking

## [1.0.0] - 2025-12-04

### Changed
- `sandy resume` now auto-selects the sandbox for the current directory when available
- Interactive pick list only shown when not in a directory with an existing sandbox

## [0.5.0] - 2025-12-04

### Changed
- **Breaking**: Renamed from `sandbox` to `sandy` to avoid conflict with macOS system sandbox utility
- All config/state files renamed:
  - `~/.config/cli-programs/sandbox.toml` → `sandy.toml`
  - `~/.config/cli-programs/sandbox-state.json` → `sandy-state.json`
  - `~/.config/cli-programs/sandbox/Dockerfile` → `sandy/Dockerfile`
  - `~/.config/cli-programs/sandbox-template.hash` → `sandy-template.hash`
  - `~/.config/cli-programs/sandbox-default-template.hash` → `sandy-default-template.hash`
- Default template image renamed from `sandbox-dev` to `sandy-dev`
- Container name prefix changed from `sandbox-` to `sandy-`

## [0.4.3] - 2025-12-04

### Fixed
- `sandy new` now automatically detects and updates the Dockerfile when the embedded default template changes after an update
- Users no longer need to manually delete their Dockerfile to get template updates

### Added
- Track embedded default template hash separately from user's Dockerfile hash
- New `~/.config/cli-programs/sandy-default-template.hash` file to detect template updates

## [0.4.2] - 2025-12-04

### Added
- Java 17 (OpenJDK) and Maven to default Docker template for Java project support

## [0.4.1] - 2025-12-02

### Fixed
- Resume command no longer hangs when attaching to running sandboxes
- Credentials now pass through from host instead of requiring manual authentication

## [0.4.0] - 2025-12-02

### Changed
- **Breaking**: Sandboxes now keyed by canonical repository path instead of user-provided names
- Replaced git worktree-based sandboxes with direct repository binding
- `new`, `resume`, and `remove` commands now use interactive selection instead of requiring explicit names
- Moved `~/.claude` mount from hardcoded docker setup to default mounts in config

### Removed
- Git worktree dependency - no longer creates worktrees for sandboxes
- Worktree directory configuration and branch tracking
- Name parameter from CLI commands (now fully interactive)

### Added
- Backwards compatibility for legacy state files using 'worktrees' key (pre-v0.4.0)
- Comprehensive unit and integration tests with TempDir isolation

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
- User's Dockerfile template now stored at `~/.config/cli-programs/sandy/Dockerfile` (was `sandy-template/Dockerfile`)
- Default Dockerfile loaded from source `template/Dockerfile` at compile time via `include_str!`

## [0.2.0] - 2025-11-30

### Changed
- `sandy new` is now self-sufficient and works without prior configuration
- Renamed `sandy config init-template` to `sandy config create-dockerfile`
- Template is automatically created and built on first `sandy new` if not present

### Added
- Default template image name (`sandy-dev`) used when no custom template configured
- Auto-setup of template on first use

## [0.1.0] - 2025-11-29

### Added
- Initial release of sandy CLI for Claude Code development environments
- `sandy new` command to create new sandbox environments
- `sandy resume` command to resume existing sandboxes (interactive selection)
- `sandy list` command to show all sandboxes with status (running/stopped/no container)
- `sandy remove` command to remove sandboxes
- `sandy config show` command to display current configuration
- `sandy config set <key> <value>` command to modify configuration
- `sandy config create-dockerfile` command to create Dockerfile template for customization
- Configuration file at `~/.config/cli-programs/sandy.toml`
- State tracking at `~/.config/cli-programs/sandy-state.json`
- Support for custom Docker templates with automatic rebuild on Dockerfile changes
- Default mounts for `~/.ssh` and `~/.gitconfig`
- Environment variable expansion in configuration values
- Auto-mount of `~/.claude` for authentication passthrough
