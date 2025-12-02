// Integration tests for gc CLI

use assert_cmd::cargo::cargo_bin_cmd;
use assert_cmd::Command;
use predicates::prelude::*;
use tempfile::TempDir;

fn gc_cmd() -> Command {
    cargo_bin_cmd!("gc").into()
}

#[test]
fn test_help_flag() {
    gc_cmd()
        .arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains("Generate conventional commit messages"));
}

#[test]
fn test_version_info() {
    gc_cmd()
        .arg("--version")
        .assert()
        .success();
}

#[test]
fn test_not_in_git_repo() {
    let temp_dir = TempDir::new().unwrap();

    gc_cmd()
        .current_dir(temp_dir.path())
        .assert()
        .failure()
        .stderr(predicate::str::contains("Not in a git repository"));
}

// TODO: Add more integration tests
// - Test with staged changes
// - Test with unstaged changes
// - Test --nopush flag
// - Test --context flag
// - Mock git operations for controlled testing
