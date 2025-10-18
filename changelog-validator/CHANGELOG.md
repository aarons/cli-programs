# Changelog

## [1.1.0] - 2025-10-17

### Removed
- Unreleased section requirement from changelog validation

---

## [1.0.1] - 2025-10-17

### Changed
- Improved integration test to dynamically discover changelogs via filesystem walking instead of hardcoded list

---

## [1.0.0] - 2025-10-17

### Added
- Initial release of changelog validation library
- Keep a Changelog format validation
- Semantic versioning validation (X.Y.Z format)
- Date format validation (YYYY-MM-DD or TBD)
- Standard section header validation (Added, Changed, Deprecated, Removed, Fixed, Security)
- Empty section detection
- Required section enforcement (Changelog header, Unreleased section)
- Integration tests for workspace-wide changelog validation
- Comprehensive unit tests for validation logic
- Public API for programmatic validation
- Detailed error messages with file path context
