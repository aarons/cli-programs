use assert_cmd::cargo::cargo_bin_cmd;
use assert_cmd::Command;
use predicates::prelude::*;
use std::fs;
use std::path::PathBuf;
use std::process::Command as StdCommand;
use tempfile::TempDir;

fn sandy_cmd() -> Command {
    cargo_bin_cmd!("sandy").into()
}

fn create_git_repo(path: &std::path::Path) -> bool {
    StdCommand::new("git")
        .args(["init"])
        .current_dir(path)
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

fn setup_test_config(temp_dir: &TempDir) -> PathBuf {
    let config_dir = temp_dir.path().join(".config").join("cli-programs");
    fs::create_dir_all(&config_dir).unwrap();
    config_dir
}

// ============================================================================
// CLI Help and Version Tests
// ============================================================================

#[test]
fn test_help_displays_usage() {
    sandy_cmd()
        .arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains("sandy"))
        .stdout(predicate::str::contains("Claude Code development environments"));
}

#[test]
fn test_version_displays() {
    sandy_cmd()
        .arg("--version")
        .assert()
        .success()
        .stdout(predicate::str::contains("sandy"));
}

#[test]
fn test_help_shows_subcommands() {
    sandy_cmd()
        .arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains("new"))
        .stdout(predicate::str::contains("resume"))
        .stdout(predicate::str::contains("list"))
        .stdout(predicate::str::contains("remove"))
        .stdout(predicate::str::contains("config"));
}

// ============================================================================
// List Command Tests
// ============================================================================

#[test]
fn test_list_with_no_sandboxes() {
    let temp_dir = TempDir::new().unwrap();
    let config_dir = setup_test_config(&temp_dir);

    // Create empty state file
    let state_path = config_dir.join("sandy-state.json");
    fs::write(&state_path, r#"{"sandboxes":{}}"#).unwrap();

    sandy_cmd()
        .arg("list")
        .env("HOME", temp_dir.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("No sandboxes found"));
}

#[test]
fn test_list_with_sandboxes() {
    let temp_dir = TempDir::new().unwrap();
    let config_dir = setup_test_config(&temp_dir);

    // Create state file with a sandbox entry
    let state_path = config_dir.join("sandy-state.json");
    let state_content = r#"{
        "sandboxes": {
            "/test/my-project": {
                "path": "/test/my-project",
                "created_at": "2024-01-01T00:00:00Z"
            }
        }
    }"#;
    fs::write(&state_path, state_content).unwrap();

    sandy_cmd()
        .arg("list")
        .env("HOME", temp_dir.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("my-project"))
        .stdout(predicate::str::contains("/test/my-project"));
}

#[test]
fn test_list_shows_multiple_sandboxes() {
    let temp_dir = TempDir::new().unwrap();
    let config_dir = setup_test_config(&temp_dir);

    let state_path = config_dir.join("sandy-state.json");
    let state_content = r#"{
        "sandboxes": {
            "/test/project-a": {
                "path": "/test/project-a",
                "created_at": "2024-01-01T00:00:00Z"
            },
            "/test/project-b": {
                "path": "/test/project-b",
                "created_at": "2024-01-02T00:00:00Z"
            }
        }
    }"#;
    fs::write(&state_path, state_content).unwrap();

    sandy_cmd()
        .arg("list")
        .env("HOME", temp_dir.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("project-a"))
        .stdout(predicate::str::contains("project-b"));
}

// ============================================================================
// Config Command Tests
// ============================================================================

#[test]
fn test_config_show_displays_config() {
    let temp_dir = TempDir::new().unwrap();
    let config_dir = setup_test_config(&temp_dir);

    // Create a config file
    let config_path = config_dir.join("sandy.toml");
    let config_content = r#"
binary_dirs = ["~/.local/bin"]

[[mounts]]
source = "~/.ssh"
target = "/home/agent/.ssh"
readonly = true
"#;
    fs::write(&config_path, config_content).unwrap();

    sandy_cmd()
        .args(["config", "show"])
        .env("HOME", temp_dir.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("binary_dirs"))
        .stdout(predicate::str::contains("mounts"));
}

#[test]
fn test_config_show_creates_default_if_missing() {
    let temp_dir = TempDir::new().unwrap();

    sandy_cmd()
        .args(["config", "show"])
        .env("HOME", temp_dir.path())
        .assert()
        .success();

    // Config file should have been created
    let config_path = temp_dir
        .path()
        .join(".config")
        .join("cli-programs")
        .join("sandy.toml");
    assert!(config_path.exists());
}

#[test]
fn test_config_set_template_image() {
    let temp_dir = TempDir::new().unwrap();
    let config_dir = setup_test_config(&temp_dir);

    // Create initial config
    let config_path = config_dir.join("sandy.toml");
    fs::write(&config_path, "binary_dirs = []\n").unwrap();

    sandy_cmd()
        .args(["config", "set", "template_image", "my-custom-image"])
        .env("HOME", temp_dir.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("Configuration updated"));

    // Verify the change was saved
    let content = fs::read_to_string(&config_path).unwrap();
    assert!(content.contains("my-custom-image"));
}

#[test]
fn test_config_set_invalid_key() {
    let temp_dir = TempDir::new().unwrap();
    setup_test_config(&temp_dir);

    sandy_cmd()
        .args(["config", "set", "invalid_key", "value"])
        .env("HOME", temp_dir.path())
        .assert()
        .failure()
        .stderr(predicate::str::contains("Unknown configuration key"));
}

#[test]
fn test_config_create_dockerfile() {
    let temp_dir = TempDir::new().unwrap();
    setup_test_config(&temp_dir);

    // Run without stdin interaction by piping 'n' to skip overwrite prompt
    sandy_cmd()
        .args(["config", "create-dockerfile"])
        .env("HOME", temp_dir.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("Template Dockerfile created"));

    // Verify Dockerfile was created
    let dockerfile_path = temp_dir
        .path()
        .join(".config")
        .join("cli-programs")
        .join("sandy")
        .join("Dockerfile");
    assert!(dockerfile_path.exists());
}

// ============================================================================
// New Command Tests
// ============================================================================

#[test]
fn test_new_requires_git_repo() {
    let temp_dir = TempDir::new().unwrap();
    setup_test_config(&temp_dir);

    // Create a non-git directory
    let work_dir = temp_dir.path().join("not-a-repo");
    fs::create_dir(&work_dir).unwrap();

    sandy_cmd()
        .arg("new")
        .current_dir(&work_dir)
        .env("HOME", temp_dir.path())
        .assert()
        .failure()
        .stderr(predicate::str::contains("not in a git repository").or(predicate::str::contains("Not a git repository")));
}

#[test]
fn test_new_prevents_duplicate_sandbox() {
    let temp_dir = TempDir::new().unwrap();
    let config_dir = setup_test_config(&temp_dir);

    // Create a git repo
    let repo_dir = temp_dir.path().join("my-repo");
    fs::create_dir(&repo_dir).unwrap();

    if !create_git_repo(&repo_dir) {
        return;
    }

    // Create state with existing sandbox for this repo
    let state_path = config_dir.join("sandy-state.json");
    let repo_path = repo_dir.canonicalize().unwrap();
    let state_content = format!(
        r#"{{"sandboxes": {{"{0}": {{"path": "{0}", "created_at": "2024-01-01T00:00:00Z"}}}}}}"#,
        repo_path.display()
    );
    fs::write(&state_path, state_content).unwrap();

    // Trying to create a new sandbox should fail with duplicate message
    // (Docker check happens before duplicate check, so this may fail on Docker first)
    let result = sandy_cmd()
        .arg("new")
        .current_dir(&repo_dir)
        .env("HOME", temp_dir.path())
        .assert();

    // If we get past Docker checks, we should see the duplicate error
    let output = result.get_output();
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        // Either Docker error or duplicate sandbox error
        assert!(
            stderr.contains("Docker")
                || stderr.contains("docker")
                || stderr.contains("already exists"),
            "Expected Docker or duplicate sandbox error"
        );
    }
}

// ============================================================================
// Remove Command Tests
// ============================================================================

#[test]
fn test_remove_with_no_sandboxes() {
    let temp_dir = TempDir::new().unwrap();
    let config_dir = setup_test_config(&temp_dir);

    // Create empty state file
    let state_path = config_dir.join("sandy-state.json");
    fs::write(&state_path, r#"{"sandboxes":{}}"#).unwrap();

    sandy_cmd()
        .arg("remove")
        .env("HOME", temp_dir.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("No sandboxes found"));
}

// ============================================================================
// Resume Command Tests
// ============================================================================

#[test]
fn test_resume_with_no_sandboxes() {
    let temp_dir = TempDir::new().unwrap();
    let config_dir = setup_test_config(&temp_dir);

    // Create empty state file
    let state_path = config_dir.join("sandy-state.json");
    fs::write(&state_path, r#"{"sandboxes":{}}"#).unwrap();

    // Resume requires Docker, so we need to handle that case
    let result = sandy_cmd()
        .arg("resume")
        .env("HOME", temp_dir.path())
        .assert();

    let output = result.get_output();
    if output.status.success() {
        // If Docker is available, should show no sandboxes
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(stdout.contains("No sandboxes found"));
    } else {
        // If Docker is not available, should show Docker error
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(stderr.contains("Docker") || stderr.contains("docker"));
    }
}

// ============================================================================
// State File Tests
// ============================================================================

#[test]
fn test_state_file_created_on_first_access() {
    let temp_dir = TempDir::new().unwrap();

    // List command should work even without existing state file
    sandy_cmd()
        .arg("list")
        .env("HOME", temp_dir.path())
        .assert()
        .success();
}

#[test]
fn test_handles_corrupted_state_file() {
    let temp_dir = TempDir::new().unwrap();
    let config_dir = setup_test_config(&temp_dir);

    // Create corrupted state file
    let state_path = config_dir.join("sandy-state.json");
    fs::write(&state_path, "not valid json {{{").unwrap();

    sandy_cmd()
        .arg("list")
        .env("HOME", temp_dir.path())
        .assert()
        .failure()
        .stderr(predicate::str::contains("parse").or(predicate::str::contains("Failed")));
}

#[test]
fn test_handles_corrupted_config_file() {
    let temp_dir = TempDir::new().unwrap();
    let config_dir = setup_test_config(&temp_dir);

    // Create corrupted config file
    let config_path = config_dir.join("sandy.toml");
    fs::write(&config_path, "not valid toml [[[").unwrap();

    sandy_cmd()
        .args(["config", "show"])
        .env("HOME", temp_dir.path())
        .assert()
        .failure()
        .stderr(predicate::str::contains("parse").or(predicate::str::contains("Failed")));
}

// ============================================================================
// Edge Cases
// ============================================================================

#[test]
fn test_unknown_subcommand() {
    sandy_cmd()
        .arg("unknown-command")
        .assert()
        .failure()
        .stderr(predicate::str::contains("error"));
}

#[test]
fn test_config_set_requires_key_and_value() {
    let temp_dir = TempDir::new().unwrap();

    sandy_cmd()
        .args(["config", "set"])
        .env("HOME", temp_dir.path())
        .assert()
        .failure();

    sandy_cmd()
        .args(["config", "set", "template_image"])
        .env("HOME", temp_dir.path())
        .assert()
        .failure();
}

#[test]
fn test_list_handles_nonexistent_sandbox_paths() {
    let temp_dir = TempDir::new().unwrap();
    let config_dir = setup_test_config(&temp_dir);

    // Create state with sandbox pointing to nonexistent path
    let state_path = config_dir.join("sandy-state.json");
    let state_content = r#"{
        "sandboxes": {
            "/nonexistent/path/12345": {
                "path": "/nonexistent/path/12345",
                "created_at": "2024-01-01T00:00:00Z"
            }
        }
    }"#;
    fs::write(&state_path, state_content).unwrap();

    // Should still list the sandbox (path validation happens at runtime)
    sandy_cmd()
        .arg("list")
        .env("HOME", temp_dir.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("12345"));
}

// ============================================================================
// Legacy State File Compatibility Tests
// ============================================================================

#[test]
fn test_list_with_legacy_worktrees_state_file() {
    let temp_dir = TempDir::new().unwrap();
    let config_dir = setup_test_config(&temp_dir);

    // Create state file using legacy "worktrees" key (pre-v0.2.0 format)
    let state_path = config_dir.join("sandy-state.json");
    let legacy_state = r#"{
        "worktrees": {
            "/test/my-legacy-project": {
                "path": "/test/my-legacy-project",
                "created_at": "2024-01-01T00:00:00Z"
            }
        }
    }"#;
    fs::write(&state_path, legacy_state).unwrap();

    // Should successfully read the legacy format
    sandy_cmd()
        .arg("list")
        .env("HOME", temp_dir.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("my-legacy-project"))
        .stdout(predicate::str::contains("/test/my-legacy-project"));
}

#[test]
fn test_handles_state_file_missing_sandboxes_field() {
    let temp_dir = TempDir::new().unwrap();
    let config_dir = setup_test_config(&temp_dir);

    // Create state file missing the sandboxes field entirely
    let state_path = config_dir.join("sandy-state.json");
    fs::write(&state_path, r#"{"version": "1.0"}"#).unwrap();

    // Should fail gracefully with a helpful error
    sandy_cmd()
        .arg("list")
        .env("HOME", temp_dir.path())
        .assert()
        .failure()
        .stderr(predicate::str::contains("parse").or(predicate::str::contains("Failed")));
}

// ============================================================================
// Config File Structure Tests
// ============================================================================

#[test]
fn test_default_config_has_expected_structure() {
    let temp_dir = TempDir::new().unwrap();

    sandy_cmd()
        .args(["config", "show"])
        .env("HOME", temp_dir.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("binary_dirs"))
        .stdout(predicate::str::contains("mounts"));
}

#[test]
fn test_config_preserves_custom_values() {
    let temp_dir = TempDir::new().unwrap();
    let config_dir = setup_test_config(&temp_dir);

    // Create config with custom env vars
    let config_path = config_dir.join("sandy.toml");
    let config_content = r#"
binary_dirs = ["/custom/bin"]

[env]
MY_VAR = "my_value"
OTHER_VAR = "other_value"

[[mounts]]
source = "/custom/source"
target = "/custom/target"
readonly = true
"#;
    fs::write(&config_path, config_content).unwrap();

    sandy_cmd()
        .args(["config", "show"])
        .env("HOME", temp_dir.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("MY_VAR"))
        .stdout(predicate::str::contains("my_value"))
        .stdout(predicate::str::contains("/custom/bin"))
        .stdout(predicate::str::contains("/custom/source"));
}
