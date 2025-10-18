# Changelog

## [Unreleased]

---

## [1.3.0] - 2025-10-17

### Changed
- Default installation directory changed from `~/code/bin` to `~/.local/bin` to align with XDG Base Directory specification and modern standards

---

## [1.2.0] - 2025-10-17

### Changed
- Improved output messaging

---

## [1.1.0] - 2025-10-17

### Added
- Package exclusion system to skip installation of library crates and development tools

---

## [1.0.0] - 2025-10-17

### Added
- Initial release of update-cli-programs installer/updater tool
- Automated discovery and installation of all workspace members from Cargo.toml
- Release mode compilation of all CLI tools in the workspace
- Binary installation to target directory (defaults to ~/code/bin)
- Automatic executable permission setting (755) for installed binaries
- `--target` flag to specify custom installation directory
- Self-updating capability - the tool can update itself along with other workspace binaries
- Cross-platform support for Unix-like environments (macOS and Linux)
