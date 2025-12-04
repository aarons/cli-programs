# Changelog

## [0.2.0] - 2025-12-04

### Added
- `now` subcommand to commit changes in all watched directories on demand

### Changed
- Running `track-changes` with no arguments now shows help instead of auto-committing

## [0.1.1] - 2025-12-04

### Changed
- `add` subcommand now triggers an initial commit when a directory is first added

## [0.1.0] - 2025-12-04

### Added
- Initial release of track-changes
- Watch directories for changes and auto-commit with timestamps
- `--dir` flag to add directory and immediately commit
- `add` and `remove` subcommands for managing watched directories
- `list` subcommand showing watched directories with status
- `log` subcommand to view recent commit history
- `install` and `uninstall` subcommands for launchd scheduling (hourly)
- Configuration file at `~/.config/cli-programs/track-changes.toml`
- Commit logging to `~/.local/share/track-changes/commits.log` (JSON Lines format)
- Git repository validation before attempting commits
