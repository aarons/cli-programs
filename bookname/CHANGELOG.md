# Changelog

## [0.1.0] - 2025-11-30

### Added
- Initial release of bookname (epub filename cleaner)
- AI-powered filename cleaning using configurable LLM providers
- Support for current directory, `--dir` path, and `--recursive` modes
- Automatic conflict resolution with numeric suffixes
- `--debug` flag for verbose output
- `--model` flag to override default LLM preset
- `bookname config` subcommands for managing LLM configuration:
  - `bookname config list` - Show available presets
  - `bookname config show` - Display full configuration
  - `bookname config set-default <preset>` - Change default preset
  - `bookname config add-preset <name> -p <provider> -M <model>` - Add new preset
