# changelog-validator

Shared changelog validation library for workspace programs.

## Overview

This library provides validation for `CHANGELOG.md` files following the [Keep a Changelog](https://keepachangelog.com/) format. It's used as a workspace-level validation tool to ensure all programs maintain consistent and valid changelog documentation.

## Features

- ✅ Validates Keep a Changelog format compliance
- ✅ Validates semantic versioning format (X.Y.Z)
- ✅ Validates date format (YYYY-MM-DD or TBD)
- ✅ Validates section headers (Added, Changed, Deprecated, Removed, Fixed, Security)
- ✅ Ensures no empty sections
- ✅ Ensures clean header format (no content between title and first version)
- ✅ Disallows [Unreleased] sections
- ✅ Automatically tests all workspace changelogs

## Usage

### As a Library

```rust
use changelog_validator::validate_changelog;

fn main() -> anyhow::Result<()> {
    let changelog = validate_changelog("path/to/CHANGELOG.md")?;

    println!("Valid changelog with {} versions", changelog.versions.len());
    Ok(())
}
```

### Running Tests

The library includes integration tests that automatically validate all workspace changelogs:

```bash
# Run all tests including workspace changelog validation
cargo test -p changelog-validator

# Run with output to see which changelogs are validated
cargo test -p changelog-validator -- --nocapture
```

## Validation Rules

A valid changelog must:

1. **Start with header**: `# Changelog`
2. **Clean header format**: Only blank lines allowed between `# Changelog` and first version (no descriptive text)
3. **No [Unreleased] sections**: These are not allowed
4. **Have at least one version**: `## [X.Y.Z] - YYYY-MM-DD`
5. **Use semantic versioning**: Version numbers must be in X.Y.Z format
6. **Use valid dates**: Either `YYYY-MM-DD` or `TBD`
7. **Use standard sections**: Only `Added`, `Changed`, `Deprecated`, `Removed`, `Fixed`, `Security`
8. **No empty sections**: Every section must have at least one list item
9. **No empty versions**: Every version must have at least one section

## Example Valid Changelog

```markdown
# Changelog

## [1.0.0] - 2025-10-17

### Added
- Initial release
- Core functionality

### Fixed
- Bug fixes from beta

## [0.1.0] - 2025-10-01

### Added
- Beta release
- Basic features
```

## Integration with Workspace

The `tests/validate_all_changelogs.rs` integration test automatically:
- Discovers all `CHANGELOG.md` files in workspace members
- Validates each one against the schema
- Provides clear error messages for any violations
- Fails the test suite if any changelog is invalid

This ensures changelog quality is maintained across the entire workspace.

## Architecture

- `lib.rs`: Core validation logic and public API
- `tests/validate_all_changelogs.rs`: Integration tests for workspace validation

The validator is designed to be:
- **Fast**: Runs in milliseconds
- **Thorough**: Comprehensive format validation
- **Helpful**: Clear error messages with line context
- **Reusable**: Can be used in other tools or CI pipelines
