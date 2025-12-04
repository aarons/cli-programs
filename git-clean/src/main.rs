// git-clean - Clean up merged local and remote git branches

use anyhow::{Context, Result};
use clap::Parser;
use git2::Repository;
use std::process::Command;

#[derive(Parser, Debug)]
#[command(name = "git-clean")]
#[command(about = "Clean up merged local and remote git branches", long_about = None)]
#[command(version)]
struct Args {
    // Currently no arguments, but could add --dry-run, --yes, etc.
}

// =============================================================================
// Git Helper Functions (similar to gc tool)
// =============================================================================

/// Execute git command and return output as string
fn git(args: &[&str]) -> Result<String> {
    let output = Command::new("git")
        .args(args)
        .output()
        .context("Failed to execute git command")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("git command failed: {}", stderr);
    }

    String::from_utf8(output.stdout).context("Git output was not valid UTF-8")
}

/// Check if current directory is a git repository
fn is_git_repo() -> bool {
    Repository::open(".").is_ok()
}

// =============================================================================
// Branch Detection Functions
// =============================================================================

/// Get branches currently used by worktrees
/// Returns a Vec of branch names that are checked out in worktrees
fn get_worktree_branches() -> Result<Vec<String>> {
    let output = git(&["worktree", "list", "--porcelain"])?;

    let branches: Vec<String> = output
        .lines()
        .filter(|line| line.starts_with("branch "))
        .map(|line| {
            // Extract branch name after "branch refs/heads/"
            line.strip_prefix("branch refs/heads/")
                .unwrap_or(line.strip_prefix("branch ").unwrap_or(""))
                .to_string()
        })
        .collect();

    Ok(branches)
}

/// Detect main branch (main or master)
fn get_main_branch() -> Result<String> {
    // Check if main exists
    let main_check = Command::new("git")
        .args(&["show-ref", "--verify", "--quiet", "refs/heads/main"])
        .status()
        .context("Failed to check for main branch")?;

    if main_check.success() {
        return Ok("main".to_string());
    }

    // Check if master exists
    let master_check = Command::new("git")
        .args(&["show-ref", "--verify", "--quiet", "refs/heads/master"])
        .status()
        .context("Failed to check for master branch")?;

    if master_check.success() {
        return Ok("master".to_string());
    }

    // Neither exists - this is an error
    anyhow::bail!("Could not find main or master branch")
}

/// Get list of local branches merged into main
/// Excludes: current branch (*), main, master, develop
fn get_merged_local_branches(main_branch: &str) -> Result<Vec<String>> {
    let output = git(&["branch", "--merged", main_branch])?;

    let protected_branches = ["main", "master", "develop"];

    let branches: Vec<String> = output
        .lines()
        .map(|line| line.trim())
        .filter(|line| !line.starts_with('*')) // Exclude current branch
        .map(|line| line.trim_start_matches("* ").trim())
        .filter(|branch| !protected_branches.contains(branch)) // Exclude protected branches
        .map(|s| s.to_string())
        .collect();

    Ok(branches)
}

/// Get list of remote branches merged into origin/main
/// Excludes: HEAD, main, master, develop, origin/main, origin/master, origin/develop
fn get_merged_remote_branches(main_branch: &str) -> Result<Vec<String>> {
    // Check against origin/main to properly evaluate remote branch state
    let output = git(&[
        "branch",
        "-r",
        "--merged",
        &format!("origin/{}", main_branch),
    ])?;

    let protected_branches = ["main", "master", "develop"];

    let branches: Vec<String> = output
        .lines()
        .map(|line| line.trim())
        .filter(|line| !line.contains("HEAD")) // Exclude HEAD
        .filter_map(|line| {
            // Strip "origin/" prefix
            line.strip_prefix("origin/").map(|s| s.to_string())
        })
        .filter(|branch| !protected_branches.contains(&branch.as_str())) // Exclude protected branches
        .collect();

    Ok(branches)
}

// =============================================================================
// Branch Deletion Functions
// =============================================================================

/// Delete local branch (safe delete with -d)
fn delete_local_branch_safe(branch: &str) -> Result<()> {
    git(&["branch", "-d", branch])?;
    Ok(())
}

/// Delete remote branch
fn delete_remote_branch(branch: &str) -> Result<()> {
    git(&["push", "origin", "--delete", branch])?;
    Ok(())
}

// =============================================================================
// Main Cleaning Logic
// =============================================================================

/// Clean up merged local branches
/// Evaluates against local main only - remote state is irrelevant
fn clean_local_branches(main_branch: &str) -> Result<()> {
    let worktree_branches = get_worktree_branches()?;
    let merged_branches = get_merged_local_branches(main_branch)?;

    for branch in merged_branches {
        // Skip if branch is used by a worktree
        if worktree_branches.contains(&branch.to_string()) {
            continue;
        }

        // Delete local branch merged to local main
        if let Err(e) = delete_local_branch_safe(&branch) {
            eprintln!("Error deleting branch '{}': {}", branch, e);
        } else {
            println!("Deleted: {} (local)", branch);
        }
    }

    Ok(())
}

/// Clean up merged remote branches
fn clean_remote_branches(main_branch: &str) -> Result<()> {
    let remote_merged_branches = get_merged_remote_branches(main_branch)?;

    // Process each remote branch merged to origin/main
    // Local branch state is irrelevant - remote cleanup is independent
    for branch in remote_merged_branches {
        delete_remote_branch(&branch)?;
        println!("Deleted: {} (remote)", branch);
    }

    Ok(())
}

// =============================================================================
// Main Entry Point
// =============================================================================

fn main() -> Result<()> {
    let _args = Args::parse();

    // Ensure we're in a git repository
    if !is_git_repo() {
        anyhow::bail!("Error: Not in a git repository");
    }

    // Detect main branch (main or master)
    let main_branch = get_main_branch().context("Failed to determine main branch")?;

    if main_branch == "master" {
        println!("Using 'master' as main branch");
    }

    // Fetch and prune remote references
    println!("Fetching and pruning remote references...");
    git(&["fetch", "--prune"]).context("Failed to fetch and prune")?;

    println!("Evaluating branches");
    println!();

    // Clean local branches (includes handling of associated remotes)
    clean_local_branches(&main_branch).context("Failed to clean local branches")?;

    // Clean remote branches (independent of local branch state)
    clean_remote_branches(&main_branch).context("Failed to clean remote branches")?;

    println!();
    println!("Done!");

    Ok(())
}
