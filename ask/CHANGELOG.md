# Changelog

## [1.2.0] - 2025-11-28

### Added
- Shell integration for unquoted special characters (`?`, `*`, `!`)
- Setup script (`setup-shell.sh`) to install shell wrapper
- `ask setup` subcommand to check and install shell integration
  - `ask setup` or `ask setup check` - Check if shell integration is installed
  - `ask setup install` - Interactively install shell integration

### Changed
- Removed clipboard status message for cleaner output

### Fixed
- Zsh shell integration now uses alias instead of function (glob expansion was happening before function execution)

## [1.1.0] - 2025-11-28

### Added
- Multi-provider LLM support via `llm-client` library
- Support for Anthropic API, OpenRouter, and Cerebras providers in addition to Claude CLI
- `--model` flag to select a specific preset for the request
- `--debug` flag for verbose output (shows provider name, token usage)
- Configuration subcommands:
  - `ask config list` - List available presets
  - `ask config show` - Show current configuration
  - `ask config set-default <preset>` - Set default preset
  - `ask config add-preset <name> --provider <p> --model <m>` - Add new preset
- Shared configuration with `gc` at `~/.config/cli-programs/llm.toml`

### Changed
- Migrated from synchronous to async execution using tokio
- LLM calls now use proper system prompts (separated from user prompt)

### Removed
- `--output-format` flag (was Claude CLI specific)
- Direct Claude CLI path lookup (now handled by llm-client)

## [1.0.0] - 2025-10-17

### Added
- Initial release of `ask` CLI tool
- Shell command mode (default) for getting command suggestions
- General mode (`-g/--general`) for asking general questions
- Automatic clipboard copy for shell command suggestions (macOS)
- Support for piped input as context
- Interactive question prompt when no arguments provided
- `--output-format` option to pass through to Claude CLI
- Integration with Claude Code CLI
- Comprehensive error handling and user feedback
