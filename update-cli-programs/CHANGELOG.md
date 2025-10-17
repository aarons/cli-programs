# Changelog

## [Unreleased]

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
