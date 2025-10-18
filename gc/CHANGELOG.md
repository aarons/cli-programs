# Changelog

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
