# Changelog

## [0.1.1] - 2025-11-13

### Fixed
- Text processing now handles invisible Unicode characters that can appear when copying text from certain applications (e.g., zero-width spaces, object replacement characters)

## [0.1.0] - 2025-11-13

### Added
- Initial release of add-reminders (text-to-reminders CLI tool)
- Process text input and add reminders to macOS Reminders app
- Text processing: remove indentation from all lines
- Text processing: remove markdown todo markers (e.g., `- [ ]` and `- [x]`)
- Text processing: skip empty lines
- `-t, --todos` flag for providing todo text (required)
- `-l, --list` flag for specifying target Reminders list (default: "inbox")
- macOS Reminders integration via AppleScript
- Batch processing: create multiple reminders from multi-line input
- Error handling with context-aware error messages
- Unit tests for text processing logic
