use anyhow::{bail, Context, Result};
use std::path::{Path, PathBuf};
use std::process::Command;

/// Get the current git repository root
pub fn get_repo_root(path: &Path) -> Result<PathBuf> {
    let output = Command::new("git")
        .args(["rev-parse", "--show-toplevel"])
        .current_dir(path)
        .output()
        .context("Failed to execute git rev-parse")?;

    if !output.status.success() {
        bail!(
            "Not a git repository: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    let root = String::from_utf8_lossy(&output.stdout).trim().to_string();
    Ok(PathBuf::from(root))
}

/// Get the current branch name
pub fn get_current_branch(repo_path: &Path) -> Result<String> {
    let output = Command::new("git")
        .args(["rev-parse", "--abbrev-ref", "HEAD"])
        .current_dir(repo_path)
        .output()
        .context("Failed to execute git rev-parse")?;

    if !output.status.success() {
        bail!(
            "Failed to get current branch: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

/// Get the repository name from its path
pub fn get_repo_name(repo_path: &Path) -> String {
    repo_path
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| "repo".to_string())
}

/// Create a git worktree
pub fn create_worktree(
    repo_path: &Path,
    worktree_path: &Path,
    branch: Option<&str>,
) -> Result<()> {
    // Ensure parent directory exists
    if let Some(parent) = worktree_path.parent() {
        if !parent.exists() {
            std::fs::create_dir_all(parent).with_context(|| {
                format!(
                    "Failed to create worktree parent directory: {}",
                    parent.display()
                )
            })?;
        }
    }

    let mut cmd = Command::new("git");
    cmd.args(["worktree", "add"]).current_dir(repo_path);

    // Add -b flag for new branch if specified
    if let Some(b) = branch {
        cmd.args(["-b", b]);
    }

    cmd.arg(worktree_path);

    let output = cmd.output().context("Failed to execute git worktree add")?;

    if !output.status.success() {
        bail!(
            "Failed to create worktree: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    Ok(())
}

/// Remove a git worktree
pub fn remove_worktree(repo_path: &Path, worktree_path: &Path) -> Result<()> {
    let output = Command::new("git")
        .args(["worktree", "remove", "--force"])
        .arg(worktree_path)
        .current_dir(repo_path)
        .output()
        .context("Failed to execute git worktree remove")?;

    if !output.status.success() {
        bail!(
            "Failed to remove worktree: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    Ok(())
}
