# Changelog

## [0.1.0] - 2025-11-30

### Added
- Initial release of bookworm (epub filename cleaner)
- AI-powered filename cleaning using configurable LLM providers
- Support for current directory, `--dir` path, and `--recursive` modes
- Automatic conflict resolution with numeric suffixes
- `--debug` flag for verbose output
- `--model` flag to override default LLM preset
- `bookworm config` subcommands for managing LLM configuration:
  - `bookworm config list` - Show available presets
  - `bookworm config show` - Display full configuration
  - `bookworm config set-default <preset>` - Change default preset
  - `bookworm config add-preset <name> -p <provider> -M <model>` - Add new preset
