//! Changelog validation library for Keep a Changelog format
//!
//! This library provides validation for CHANGELOG.md files following the
//! [Keep a Changelog](https://keepachangelog.com/) format.

use anyhow::{Context, Result, bail};
use once_cell::sync::Lazy;
use regex::Regex;
use std::path::Path;

/// Valid section headers according to Keep a Changelog
const VALID_SECTIONS: &[&str] = &[
    "Added",
    "Changed",
    "Deprecated",
    "Removed",
    "Fixed",
    "Security",
];

/// Regex patterns for validation
static VERSION_PATTERN: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"^## \[([^\]]+)\] - (.+)$").unwrap());
static DATE_PATTERN: Lazy<Regex> = Lazy::new(|| Regex::new(r"^\d{4}-\d{2}-\d{2}$|^TBD$").unwrap());
static SECTION_PATTERN: Lazy<Regex> = Lazy::new(|| Regex::new(r"^### (.+)$").unwrap());

/// Represents a parsed changelog
#[derive(Debug)]
pub struct Changelog {
    pub content: String,
    pub versions: Vec<Version>,
}

/// Represents a version entry in the changelog
#[derive(Debug)]
pub struct Version {
    pub version: String,
    pub date: String,
    pub sections: Vec<Section>,
}

/// Represents a section within a version
#[derive(Debug)]
pub struct Section {
    pub name: String,
    pub entries: Vec<String>,
}

/// Validates a changelog file at the given path
///
/// # Errors
///
/// Returns an error if:
/// - File cannot be read
/// - File doesn't start with "# Changelog"
/// - Invalid version format
/// - Invalid date format
/// - Invalid section headers
/// - Empty versions with no content
pub fn validate_changelog<P: AsRef<Path>>(path: P) -> Result<Changelog> {
    let path = path.as_ref();
    let content = std::fs::read_to_string(path)
        .with_context(|| format!("Failed to read changelog at {}", path.display()))?;

    validate_content(&content, path)
}

/// Validates changelog content
pub fn validate_content(content: &str, path: &Path) -> Result<Changelog> {
    let lines: Vec<&str> = content.lines().collect();

    // Validate header
    if lines.is_empty() || !lines[0].starts_with("# Changelog") {
        bail!("{}: Must start with '# Changelog' header", path.display());
    }

    // Check for [Unreleased] section (disallowed)
    if content.contains("## [Unreleased]") {
        bail!("{}: [Unreleased] sections are not allowed", path.display());
    }

    // Validate that only blank lines appear between header and first version
    validate_header_format(&lines, path)?;

    // Parse and validate versions
    let versions = parse_versions(&lines, path)?;

    if versions.is_empty() {
        bail!(
            "{}: Must have at least one versioned release",
            path.display()
        );
    }

    Ok(Changelog {
        content: content.to_string(),
        versions,
    })
}

/// Validates that only blank lines appear between the header and first version
fn validate_header_format(lines: &[&str], path: &Path) -> Result<()> {
    let mut found_header = false;

    for (i, line) in lines.iter().enumerate() {
        let trimmed = line.trim();

        // Found the header
        if trimmed.starts_with("# Changelog") {
            found_header = true;
            continue;
        }

        // After header, check for non-blank lines before first version
        if found_header {
            // If we hit a version header, we're done
            if VERSION_PATTERN.is_match(trimmed) {
                break;
            }

            // If we find a non-blank line that's not a version header
            if !trimmed.is_empty() {
                bail!(
                    "{}: Line {}: Found content between '# Changelog' header and first version section. Only blank lines are allowed.",
                    path.display(),
                    i + 1
                );
            }
        }
    }

    Ok(())
}

/// Parses version entries from changelog lines
fn parse_versions(lines: &[&str], path: &Path) -> Result<Vec<Version>> {
    let mut versions = Vec::new();
    let mut current_version: Option<Version> = None;
    let mut current_section: Option<Section> = None;

    for line in lines {
        let trimmed = line.trim();

        // Check for version header
        if let Some(caps) = VERSION_PATTERN.captures(trimmed) {
            // Save previous version if exists
            if let Some(mut ver) = current_version.take() {
                if let Some(sec) = current_section.take() {
                    ver.sections.push(sec);
                }
                versions.push(ver);
            }

            let version = caps.get(1).unwrap().as_str().to_string();
            let date = caps.get(2).unwrap().as_str().to_string();

            // Validate semver format
            if !is_valid_semver(&version) {
                bail!(
                    "{}: Invalid semver format '{}' (expected X.Y.Z)",
                    path.display(),
                    version
                );
            }

            // Validate date format
            if !DATE_PATTERN.is_match(&date) {
                bail!(
                    "{}: Invalid date format '{}' for version {} (expected YYYY-MM-DD or TBD)",
                    path.display(),
                    date,
                    version
                );
            }

            current_version = Some(Version {
                version,
                date,
                sections: Vec::new(),
            });
        }
        // Check for section header
        else if let Some(caps) = SECTION_PATTERN.captures(trimmed) {
            // Save previous section if exists
            if let Some(sec) = current_section.take() {
                if let Some(ref mut ver) = current_version {
                    ver.sections.push(sec);
                }
            }

            let section_name = caps.get(1).unwrap().as_str();

            // Validate section name
            if !VALID_SECTIONS.contains(&section_name) {
                bail!(
                    "{}: Invalid section '{}' (expected one of: {})",
                    path.display(),
                    section_name,
                    VALID_SECTIONS.join(", ")
                );
            }

            current_section = Some(Section {
                name: section_name.to_string(),
                entries: Vec::new(),
            });
        }
        // Check for section entry (list item)
        else if trimmed.starts_with("- ") {
            if let Some(ref mut sec) = current_section {
                sec.entries.push(trimmed.to_string());
            }
        }
    }

    // Save final version and section
    if let Some(sec) = current_section {
        if let Some(ref mut ver) = current_version {
            ver.sections.push(sec);
        }
    }
    if let Some(ver) = current_version {
        versions.push(ver);
    }

    // Validate that each version has content
    for version in &versions {
        if version.sections.is_empty() {
            bail!(
                "{}: Version {} has no sections",
                path.display(),
                version.version
            );
        }

        for section in &version.sections {
            if section.entries.is_empty() {
                bail!(
                    "{}: Section '{}' in version {} is empty",
                    path.display(),
                    section.name,
                    version.version
                );
            }
        }
    }

    Ok(versions)
}

/// Validates semver format (X.Y.Z where X, Y, Z are numbers)
fn is_valid_semver(version: &str) -> bool {
    let parts: Vec<&str> = version.split('.').collect();
    if parts.len() != 3 {
        return false;
    }
    parts.iter().all(|p| p.parse::<u32>().is_ok())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_valid_semver() {
        assert!(is_valid_semver("1.0.0"));
        assert!(is_valid_semver("0.1.0"));
        assert!(is_valid_semver("10.20.30"));
        assert!(!is_valid_semver("1.0"));
        assert!(!is_valid_semver("1.0.0.0"));
        assert!(!is_valid_semver("1.0.x"));
    }

    #[test]
    fn test_valid_changelog() {
        let content = r#"# Changelog

## [1.0.0] - 2025-10-17

### Added
- Initial release
- New feature

### Fixed
- Bug fix
"#;

        let result = validate_content(content, Path::new("test.md"));
        assert!(result.is_ok());
        let changelog = result.unwrap();
        assert_eq!(changelog.versions.len(), 1);
        assert_eq!(changelog.versions[0].version, "1.0.0");
        assert_eq!(changelog.versions[0].date, "2025-10-17");
        assert_eq!(changelog.versions[0].sections.len(), 2);
    }

    #[test]
    fn test_missing_header() {
        let content = "## [Unreleased]";
        let result = validate_content(content, Path::new("test.md"));
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("Must start with '# Changelog'")
        );
    }

    #[test]
    fn test_unreleased_section_disallowed() {
        let content = r#"# Changelog

## [Unreleased]

## [1.0.0] - 2025-10-17

### Added
- Initial release
"#;
        let result = validate_content(content, Path::new("test.md"));
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("[Unreleased] sections are not allowed")
        );
    }

    #[test]
    fn test_content_after_header_disallowed() {
        let content = r#"# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [1.0.0] - 2025-10-17

### Added
- Initial release
"#;
        let result = validate_content(content, Path::new("test.md"));
        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(
            err_msg
                .contains("Found content between '# Changelog' header and first version section")
        );
    }

    #[test]
    fn test_invalid_semver() {
        let content = r#"# Changelog

## [1.0] - 2025-10-17

### Added
- Initial release
"#;
        let result = validate_content(content, Path::new("test.md"));
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Invalid semver"));
    }

    #[test]
    fn test_invalid_date() {
        let content = r#"# Changelog

## [1.0.0] - not-a-date

### Added
- Initial release
"#;
        let result = validate_content(content, Path::new("test.md"));
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Invalid date"));
    }

    #[test]
    fn test_invalid_section() {
        let content = r#"# Changelog

## [1.0.0] - 2025-10-17

### NewStuff
- Initial release
"#;
        let result = validate_content(content, Path::new("test.md"));
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Invalid section"));
    }

    #[test]
    fn test_empty_section() {
        let content = r#"# Changelog

## [1.0.0] - 2025-10-17

### Added
"#;
        let result = validate_content(content, Path::new("test.md"));
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("is empty"));
    }

    #[test]
    fn test_tbd_date() {
        let content = r#"# Changelog

## [1.0.0] - TBD

### Added
- Initial release
"#;
        let result = validate_content(content, Path::new("test.md"));
        assert!(result.is_ok());
    }
}
