//! Integration tests to validate all workspace changelogs
//!
//! This test automatically discovers and validates all CHANGELOG.md files
//! in the workspace, ensuring they conform to the Keep a Changelog format.

use changelog_validator::validate_changelog;
use std::path::PathBuf;

/// Get the workspace root directory
fn workspace_root() -> PathBuf {
    // Start from the test binary location and go up to find workspace root
    let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    path.pop(); // Go up from changelog-validator to workspace root
    path
}

/// Finds all CHANGELOG.md files in the workspace
fn find_all_changelogs() -> Vec<PathBuf> {
    let workspace = workspace_root();
    let mut changelogs = Vec::new();

    // Read all entries in the workspace root
    if let Ok(entries) = std::fs::read_dir(&workspace) {
        for entry in entries.flatten() {
            if let Ok(metadata) = entry.metadata() {
                // Only look in directories
                if metadata.is_dir() {
                    let changelog_path = entry.path().join("CHANGELOG.md");
                    if changelog_path.exists() {
                        changelogs.push(changelog_path);
                    }
                }
            }
        }
    }

    changelogs
}

#[test]
fn all_workspace_changelogs_are_valid() {
    let changelogs = find_all_changelogs();

    assert!(
        !changelogs.is_empty(),
        "No CHANGELOG.md files found in workspace"
    );

    println!("\nValidating {} changelog(s):", changelogs.len());

    let mut errors = Vec::new();

    for changelog_path in &changelogs {
        let relative_path = changelog_path
            .strip_prefix(&workspace_root())
            .unwrap_or(changelog_path);

        print!("  - {}... ", relative_path.display());

        match validate_changelog(changelog_path) {
            Ok(changelog) => {
                println!(
                    "✓ ({} version(s))",
                    changelog.versions.len()
                );
            }
            Err(e) => {
                println!("✗");
                errors.push(format!("{}: {}", relative_path.display(), e));
            }
        }
    }

    if !errors.is_empty() {
        eprintln!("\n❌ Changelog validation errors:\n");
        for error in &errors {
            eprintln!("  {}", error);
        }
        panic!("\n{} changelog(s) failed validation", errors.len());
    }

    println!("\n✅ All changelogs are valid!");
}

#[test]
fn changelog_validator_has_changelog() {
    let changelog_path = workspace_root()
        .join("changelog-validator")
        .join("CHANGELOG.md");

    assert!(
        changelog_path.exists(),
        "changelog-validator must have its own CHANGELOG.md"
    );

    validate_changelog(&changelog_path)
        .expect("changelog-validator's own CHANGELOG.md must be valid");
}
