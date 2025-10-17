// Integration tests for gc CLI

use assert_cmd::Command;
use predicates::prelude::*;

#[test]
fn test_help_flag() {
    let mut cmd = Command::cargo_bin("gc").unwrap();
    cmd.arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains("Generate conventional commit messages"));
}

#[test]
fn test_version_info() {
    let mut cmd = Command::cargo_bin("gc").unwrap();
    cmd.arg("--version")
        .assert()
        .success();
}

#[test]
fn test_not_in_git_repo() {
    // Create a temporary directory that's not a git repo
    let temp_dir = assert_cmd::cargo::cargo_bin("gc")
        .parent()
        .unwrap()
        .join("../../../test_temp");
    std::fs::create_dir_all(&temp_dir).ok();

    let mut cmd = Command::cargo_bin("gc").unwrap();
    cmd.current_dir(&temp_dir)
        .assert()
        .failure()
        .stderr(predicate::str::contains("Not in a git repository"));

    // Cleanup
    std::fs::remove_dir_all(&temp_dir).ok();
}

// TODO: Add more integration tests
// - Test with staged changes
// - Test with unstaged changes
// - Test --nopush flag
// - Test --context flag
// - Mock git operations for controlled testing
