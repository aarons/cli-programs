use anyhow::{Context, Result};
use chrono::{DateTime, Local};
use std::path::Path;
use std::process::Command;

/// Execute a git command in the specified directory and return the output
fn git(args: &[&str], working_dir: &Path) -> Result<String> {
    let output = Command::new("git")
        .args(args)
        .current_dir(working_dir)
        .output()
        .context("Failed to execute git command")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("git command failed: {}", stderr.trim());
    }

    String::from_utf8(output.stdout).context("Git output was not valid UTF-8")
}

/// Check if directory is a git repository
pub fn is_git_repo(path: &Path) -> bool {
    git2::Repository::open(path).is_ok()
}

/// Initialize a new git repository in the specified directory
pub fn init_repo(path: &Path) -> Result<()> {
    git2::Repository::init(path).context("Failed to initialize git repository")?;
    Ok(())
}

/// Get list of changed files from git status --porcelain
pub fn get_changed_files(path: &Path) -> Result<Vec<String>> {
    let status = git(&["status", "--porcelain"], path)?;
    Ok(status
        .lines()
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
        .collect())
}

/// Stage all changes and commit with timestamp
/// Returns the commit hash on success
pub fn commit_with_timestamp(path: &Path) -> Result<String> {
    // Stage all changes
    git(&["add", "-A"], path)?;

    // Create commit message with ISO timestamp
    let timestamp = chrono::Local::now().format("%Y-%m-%dT%H:%M:%S%:z");
    let message = format!("Auto-commit: {}", timestamp);

    git(&["commit", "-m", &message], path)?;

    // Get the commit hash
    let hash = git(&["rev-parse", "--short", "HEAD"], path)?;
    Ok(hash.trim().to_string())
}

/// Get the latest commit timestamp for a directory
pub fn get_last_commit_time(path: &Path) -> Result<Option<DateTime<Local>>> {
    match git(&["log", "-1", "--format=%aI"], path) {
        Ok(output) => {
            let timestamp = output.trim();
            if timestamp.is_empty() {
                return Ok(None);
            }
            let dt = DateTime::parse_from_rfc3339(timestamp)
                .context("Failed to parse commit timestamp")?;
            Ok(Some(dt.with_timezone(&Local)))
        }
        Err(_) => Ok(None), // No commits yet or other error
    }
}
