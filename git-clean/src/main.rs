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

    String::from_utf8(output.stdout)
        .context("Git output was not valid UTF-8")
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
    // TODO: Parse output of: git worktree list --porcelain
    // Extract lines starting with "branch " and collect branch names
    todo!("Parse git worktree list --porcelain output")
}

/// Detect main branch (main or master)
fn get_main_branch() -> Result<String> {
    // TODO: Check if refs/heads/main exists, else check refs/heads/master
    // Use git show-ref --verify --quiet refs/heads/main
    // Default to "main" if neither exists
    todo!("Detect main or master branch")
}

/// Get list of local branches merged into main
/// Excludes: current branch (*), main, master, develop
fn get_merged_local_branches(main_branch: &str) -> Result<Vec<String>> {
    // TODO: Run: git branch --merged <main_branch>
    // Filter out branches starting with *, and exact matches for main/master/develop
    // Return cleaned branch names (trimmed whitespace)
    todo!("Get merged local branches")
}

/// Get list of remote branches merged into origin/main
/// Excludes: HEAD, main, master, develop, origin/main, origin/master, origin/develop
fn get_merged_remote_branches(main_branch: &str) -> Result<Vec<String>> {
    // TODO: Run: git branch -r --merged origin/<main_branch>
    // Filter out HEAD and protected branches
    // Strip "origin/" prefix from branch names
    // Return cleaned branch names
    todo!("Get merged remote branches")
}

// =============================================================================
// Branch Status Functions
// =============================================================================

/// Check if a remote branch exists for the given local branch
fn has_remote_branch(branch: &str) -> Result<bool> {
    // TODO: Use git show-ref --verify --quiet refs/remotes/origin/<branch>
    // Return true if command succeeds (exit code 0)
    todo!("Check if remote branch exists")
}

/// Check if a local branch exists
fn has_local_branch(branch: &str) -> Result<bool> {
    // TODO: Use git show-ref --verify --quiet refs/heads/<branch>
    // Return true if command succeeds (exit code 0)
    todo!("Check if local branch exists")
}

/// Check if remote branch is merged into origin/main
fn is_remote_merged(branch: &str, main_branch: &str) -> Result<bool> {
    // TODO: Run: git branch -r --merged origin/<main_branch>
    // Check if "origin/<branch>" appears in output
    todo!("Check if remote branch is merged")
}

/// Get ahead/behind commit counts between local and remote branch
/// Returns (ahead, behind) tuple
fn get_branch_ahead_behind(local_branch: &str, remote_branch: &str) -> Result<(usize, usize)> {
    // TODO: Get ahead count: git rev-list --count <remote>..<local>
    // TODO: Get behind count: git rev-list --count <local>..<remote>
    // Return tuple of (ahead, behind)
    todo!("Calculate ahead/behind counts")
}

// =============================================================================
// User Interaction
// =============================================================================

#[derive(Debug)]
enum BranchAction {
    Skip,
    Push,
    DeleteLocal,
    DeleteBoth,
}

/// Prompt user for action when branches are out of sync
/// Shows merge status, ahead/behind info, and available options
fn prompt_user_for_action(
    branch: &str,
    local_merged: bool,
    remote_merged: bool,
    has_remote: bool,
    ahead: usize,
    behind: usize,
) -> Result<BranchAction> {
    // TODO: Display branch status:
    // - Local merge status
    // - Remote merge status (if has_remote)
    // - Ahead/behind info (if has_remote)

    // TODO: Display options based on has_remote:
    // With remote: push, skip, local, both
    // Without remote: skip, local

    // TODO: Read user input from stdin
    // Parse response and return appropriate BranchAction

    todo!("Implement user prompt and input handling")
}

// =============================================================================
// Branch Deletion Functions
// =============================================================================

/// Delete local branch (safe delete with -d)
fn delete_local_branch_safe(branch: &str) -> Result<()> {
    // TODO: Run: git branch -d <branch>
    todo!("Delete local branch with -d")
}

/// Force delete local branch (with -D)
fn delete_local_branch_force(branch: &str) -> Result<()> {
    // TODO: Run: git branch -D <branch>
    todo!("Force delete local branch with -D")
}

/// Delete remote branch
fn delete_remote_branch(branch: &str) -> Result<()> {
    // TODO: Run: git push origin --delete <branch>
    // Ignore errors (branch might already be deleted)
    todo!("Delete remote branch")
}

/// Push local branch to remote
fn push_branch(branch: &str) -> Result<()> {
    // TODO: Run: git push origin <branch>
    todo!("Push branch to remote")
}

// =============================================================================
// Main Cleaning Logic
// =============================================================================

/// Handle the "push" option - push branch and re-evaluate
fn handle_push_option(branch: &str, main_branch: &str) -> Result<()> {
    println!("Pushing '{}' to origin...", branch);

    // TODO: Push branch
    // TODO: Re-fetch: git fetch --prune
    // TODO: Check if remote is now merged
    // TODO: If merged, delete both branches
    // TODO: If not merged, print warning and keep

    todo!("Implement push and re-evaluation logic")
}

/// Process a single local branch that is merged to main
fn process_merged_local_branch(
    branch: &str,
    main_branch: &str,
    worktree_branches: &[String],
) -> Result<()> {
    // Skip if branch is used by a worktree
    if worktree_branches.contains(&branch.to_string()) {
        return Ok(());
    }

    let local_merged = true; // We know it's merged (from query)

    // TODO: Check if remote branch exists
    let has_remote = false; // TODO: has_remote_branch(branch)?

    if !has_remote {
        // No remote branch - safe to delete local
        // TODO: delete_local_branch_safe(branch)?
        println!("Deleted: {} (local)", branch);
        return Ok(());
    }

    // TODO: Check if remote branch is also merged
    let remote_merged = false; // TODO: is_remote_merged(branch, main_branch)?

    if remote_merged {
        // Both local and remote are merged - safe to delete both
        // TODO: delete_local_branch_safe(branch)?
        // TODO: delete_remote_branch(branch)?
        println!("Deleted: {} (local, remote)", branch);
        return Ok(());
    }

    // Local is merged but remote is not - require user decision
    // TODO: Get ahead/behind counts
    let (ahead, behind) = (0, 0); // TODO: get_branch_ahead_behind(branch, &format!("origin/{}", branch))?

    // TODO: Prompt user for action
    let action = BranchAction::Skip; // TODO: prompt_user_for_action(...)

    // TODO: Execute chosen action
    match action {
        BranchAction::Skip => {
            println!("Skipping: {}", branch);
        }
        BranchAction::Push => {
            // TODO: handle_push_option(branch, main_branch)?
        }
        BranchAction::DeleteLocal => {
            // TODO: delete_local_branch_force(branch)?
            println!("Deleted: {} (local)", branch);
        }
        BranchAction::DeleteBoth => {
            // TODO: delete_local_branch_force(branch)?
            // TODO: delete_remote_branch(branch)?
            println!("Deleted: {} (local, remote) - FORCED", branch);
        }
    }

    Ok(())
}

/// Clean up merged local branches
fn clean_local_branches(main_branch: &str) -> Result<()> {
    // TODO: Get worktree branches
    let worktree_branches: Vec<String> = Vec::new(); // TODO: get_worktree_branches()?

    // TODO: Get merged local branches
    let merged_branches = Vec::<String>::new(); // TODO: get_merged_local_branches(main_branch)?

    // Process each merged branch
    for branch in merged_branches {
        // TODO: Call process_merged_local_branch for each branch
        // Handle errors gracefully, continue with remaining branches
    }

    Ok(())
}

/// Clean up merged remote branches that don't have local counterparts
fn clean_remote_branches(main_branch: &str) -> Result<()> {
    // TODO: Get merged remote branches
    let remote_merged_branches = Vec::<String>::new(); // TODO: get_merged_remote_branches(main_branch)?

    // Process each remote-only branch
    for branch in remote_merged_branches {
        // Skip if local branch exists (already handled in clean_local_branches)
        // TODO: if has_local_branch(&branch)? { continue; }

        // TODO: delete_remote_branch(&branch)?
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
    let main_branch = get_main_branch()
        .context("Failed to determine main branch")?;

    if main_branch == "master" {
        println!("Using 'master' as main branch");
    }

    // Fetch and prune remote references
    println!("Fetching and pruning remote references...");
    git(&["fetch", "--prune"])
        .context("Failed to fetch and prune")?;

    println!("Evaluating branches");
    println!();

    // Clean local branches (includes handling of associated remotes)
    clean_local_branches(&main_branch)
        .context("Failed to clean local branches")?;

    // Clean remote-only branches
    clean_remote_branches(&main_branch)
        .context("Failed to clean remote branches")?;

    println!();
    println!("Done!");

    Ok(())
}
