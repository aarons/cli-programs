# Changelog

## [1.2.1] - 2025-12-04

### Fixed
- URL validation no longer flags deleted or renamed filenames as URLs in commit messages

## [1.2.0] - 2025-12-04

### Added
- Large commit handling: when diffs exceed a configurable token limit, gc now prompts for a description and uses file list + context instead of full diff
- Configurable token limit via `~/.config/cli-programs/gc.toml` (default: 30,000 tokens)

## [1.1.0] - 2025-11-28

### Added
- Multi-LLM provider support via new llm-client shared crate
- Support for Anthropic API, OpenRouter, and Cerebras providers (in addition to Claude CLI)
- `--model <preset>` flag to override default model preset
- `gc config` subcommands for managing LLM configuration:
  - `gc config list` - Show available presets
  - `gc config show` - Display full configuration
  - `gc config set-default <preset>` - Change default preset
  - `gc config add-preset <name> -p <provider> -M <model>` - Add new preset
- Configuration file at `~/.config/cli-programs/llm.toml`

### Changed
- Converted to async runtime using tokio
- LLM interaction now uses llm-client crate instead of direct Claude CLI calls

### Fixed
- Restored fallback handling for initial commits in empty repositories
- `--context` flag now adds to branch commits instead of replacing them

## [1.0.1] - 2025-10-20

### Fixed
- URL detection no longer triggers false positives on words ending with periods at end of sentences

## [1.0.0] - 2025-10-17

### Added
- Initial release of gc (git commit automation tool)
- AI-powered conventional commit message generation using Claude CLI
- Automatic staging and committing of changes with intelligent commit messages
- Format validation for conventional commit structure (type, scope, description)
- Policy enforcement to prevent URLs, email addresses, and emojis in commit messages
- Automatic retry and cleaning logic when validation fails (max 3 attempts)
- `--debug` flag for verbose output showing LLM interactions and validation steps
- `--staged` flag to commit only staged changes without auto-staging
- `--nopush` flag to skip pushing to remote after commit
- `--context` flag for providing additional context (e.g., for squash merges)
- Support for trailing arguments to provide high-level description guidance
- Context gathering from git diff, file status, branch info, and commit history
- Smart detection of main branch (main/master) for relevant commit history
