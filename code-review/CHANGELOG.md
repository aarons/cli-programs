# Changelog

## [0.1.0] - 2026-01-01

### Added
- Initial release of code-review CLI tool
- Wraps codex review command for AI-powered code reviews
- Auto-detection of review mode based on git state (uncommitted vs committed changes)
- Support for --uncommitted flag to force uncommitted changes review
- Support for --commit flag to review specific commits
- Output parsing to extract only the review section from codex output
- Error logging to ./logs/ when parsing fails
- Unit tests for output parsing logic
