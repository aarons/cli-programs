# Changelog

## [0.1.0] - 2025-01-14

### Added

- Initial release
- Project type detection for Rust and Python projects
- Mutation testing support via cargo-mutants (Rust) and mutmut (Python)
- Mutation score calculation and grading (A-F)
- Report generation in terminal and JSON formats
- LLM-powered test suggestions for surviving mutants
- Tool installation checking with `test-review check`
- Project info display with `test-review info`
- Package-specific testing for Rust workspaces (`-p` flag)
