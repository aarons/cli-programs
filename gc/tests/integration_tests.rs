// Integration tests for gc CLI

use assert_cmd::Command;
use assert_cmd::cargo::cargo_bin_cmd;
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
        .stdout(predicate::str::contains(
            "Generate conventional commit messages",
        ));
}

#[test]
fn test_version_info() {
    gc_cmd().arg("--version").assert().success();
}

#[test]
fn test_config_help_shows_example_config() {
    gc_cmd()
        .args(["config", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("[local_server]"))
        .stdout(predicate::str::contains("gc config init"));
}

#[test]
fn test_config_init_creates_example_then_never_overwrites() {
    let temp_home = TempDir::new().unwrap();
    let config_path = temp_home
        .path()
        .join(".config")
        .join("cli-programs")
        .join("gc.toml");

    gc_cmd()
        .env("HOME", temp_home.path())
        .args(["config", "init"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Wrote example config"));
    let written = std::fs::read_to_string(&config_path).unwrap();
    assert!(written.contains("max_diff_tokens"));

    // A customized config must survive a second `init` untouched
    let customized = "max_diff_tokens = 12345\n";
    std::fs::write(&config_path, customized).unwrap();
    gc_cmd()
        .env("HOME", temp_home.path())
        .args(["config", "init"])
        .assert()
        .success()
        .stdout(predicate::str::contains("not overwriting"));
    assert_eq!(std::fs::read_to_string(&config_path).unwrap(), customized);
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
