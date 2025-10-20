use anyhow::{bail, Context, Result};
use clap::Parser;
use std::process::{Command, Stdio};

/// Merge a feature branch into main with optional squash
#[derive(Parser, Debug)]
#[command(name = "git-merge")]
#[command(about = "Merge a feature branch into main", long_about = None)]
struct Args {
    /// Feature branch to merge (defaults to current branch)
    #[arg(value_name = "BRANCH")]
    branch: Option<String>,

    /// Perform a squash merge instead of a regular merge
    #[arg(short, long)]
    squash: bool,

    /// Main branch name (defaults to 'main')
    #[arg(short, long, default_value = "main")]
    main_branch: String,
}

fn main() {
    if let Err(e) = run() {
        eprintln!("Error: {}", e);
        std::process::exit(1);
    }
}

fn run() -> Result<()> {
    let args = Args::parse();

    // Check prerequisites
    check_git_installed()?;
    check_in_git_repo()?;

    // Determine feature branch
    let feature_branch = determine_feature_branch(args.branch, &args.main_branch)?;
    println!("Feature branch: {}", feature_branch);

    // Push feature branch to origin
    println!("Ensuring remote 'origin' has the latest '{}'...", feature_branch);
    push_branch(&feature_branch)?;

    // Switch to main branch
    println!("Checking out '{}'...", args.main_branch);
    checkout_branch(&args.main_branch)?;

    // Update main branch
    println!("Fetching updates from origin...");
    run_git_command(&["fetch", "origin"])?;

    println!("Pulling latest changes for '{}'...", args.main_branch);
    run_git_command(&["pull", "origin", &args.main_branch])?;

    // Check for clean status
    if !is_git_status_clean()? {
        bail!(
            "Git status is not clean after pulling '{}'. Manual intervention required.",
            args.main_branch
        );
    }

    // Perform merge
    if args.squash {
        perform_squash_merge(&feature_branch, &args.main_branch)?;
    } else {
        perform_simple_merge(&feature_branch)?;
    }

    println!("Pushing '{}' to origin...", args.main_branch);
    push_branch(&args.main_branch)?;

    println!("Merge process completed successfully.");
    Ok(())
}

fn check_git_installed() -> Result<()> {
    Command::new("git")
        .arg("--version")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .context("git is not installed. Please install it.")?;
    Ok(())
}

fn check_in_git_repo() -> Result<()> {
    let status = Command::new("git")
        .args(["rev-parse", "--is-inside-work-tree"])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .context("Failed to check if inside git repository")?;

    if !status.success() {
        bail!("Not inside a git repository.");
    }
    Ok(())
}

fn determine_feature_branch(branch_arg: Option<String>, main_branch: &str) -> Result<String> {
    if let Some(branch) = branch_arg {
        println!("Using provided feature branch: {}", branch);
        Ok(branch)
    } else {
        let current_branch = get_current_branch()?;
        if current_branch == main_branch {
            bail!(
                "Currently on '{}'. Please provide a feature branch name as an argument or run this from the feature branch.",
                main_branch
            );
        }
        println!("Detected current feature branch: {}", current_branch);
        Ok(current_branch)
    }
}

fn get_current_branch() -> Result<String> {
    let output = Command::new("git")
        .args(["rev-parse", "--abbrev-ref", "HEAD"])
        .output()
        .context("Failed to get current branch")?;

    if !output.status.success() {
        bail!("Failed to determine current branch");
    }

    Ok(String::from_utf8(output.stdout)?.trim().to_string())
}

fn push_branch(branch: &str) -> Result<()> {
    let status = Command::new("git")
        .args(["push", "origin", branch])
        .status()
        .context("Failed to push branch")?;

    if !status.success() {
        bail!(
            "Failed to push '{}' to origin. Remote might have changes not present locally. Please resolve manually.",
            branch
        );
    }
    Ok(())
}

fn checkout_branch(branch: &str) -> Result<()> {
    let status = Command::new("git")
        .args(["checkout", branch])
        .status()
        .context("Failed to checkout branch")?;

    if !status.success() {
        bail!(
            "Failed to checkout '{}'. Check for uncommitted changes or other issues.",
            branch
        );
    }
    Ok(())
}

fn run_git_command(args: &[&str]) -> Result<()> {
    let status = Command::new("git")
        .args(args)
        .status()
        .context(format!("Failed to run git command: git {}", args.join(" ")))?;

    if !status.success() {
        bail!("Git command failed: git {}", args.join(" "));
    }
    Ok(())
}

fn is_git_status_clean() -> Result<bool> {
    let output = Command::new("git")
        .args(["status", "--porcelain"])
        .output()
        .context("Failed to check git status")?;

    Ok(output.stdout.is_empty())
}

fn perform_simple_merge(feature_branch: &str) -> Result<()> {
    println!("Performing simple merge of '{}' into current branch...", feature_branch);

    let status = Command::new("git")
        .args(["merge", feature_branch])
        .status()
        .context("Failed to merge branch")?;

    if !status.success() {
        bail!(
            "Merge failed. Please resolve conflicts manually and complete the merge."
        );
    }

    // Delete the feature branch after successful merge
    println!("Deleting local branch '{}'...", feature_branch);
    let status = Command::new("git")
        .args(["branch", "-d", feature_branch])
        .status()
        .context("Failed to delete branch")?;

    if !status.success() {
        eprintln!(
            "Warning: Failed to delete local branch '{}'. You may need to delete it manually.",
            feature_branch
        );
    }

    Ok(())
}

fn perform_squash_merge(feature_branch: &str, main_branch: &str) -> Result<()> {
    // Get feature branch history
    println!("Gathering commit history from '{}'...", feature_branch);
    let output = Command::new("git")
        .args([
            "log",
            &format!("{}..{}", main_branch, feature_branch),
            "--pretty=format:%ad - %s",
            "--date=short",
        ])
        .output()
        .context("Failed to get commit history")?;

    let branch_history = String::from_utf8(output.stdout)?;
    if branch_history.trim().is_empty() {
        eprintln!(
            "Warning: No commit history found between '{}' and '{}'. The branch might be empty or already merged.",
            main_branch, feature_branch
        );
    }

    // Perform squash merge
    println!("Attempting squash merge of '{}' into '{}'...", feature_branch, main_branch);
    let status = Command::new("git")
        .args(["merge", "--squash", feature_branch])
        .status()
        .context("Failed to perform squash merge")?;

    if !status.success() {
        // Check for conflicts
        let has_conflicts = check_for_conflicts()?;
        if has_conflicts {
            bail!("Merge conflict detected after 'git merge --squash'. Resolve conflicts, then run 'gc' manually.");
        } else {
            bail!("git merge --squash failed for an unknown reason.");
        }
    }

    // Check if squash merge resulted in any changes
    if is_git_status_clean()? {
        println!(
            "No changes detected after squash merge. '{}' might have been already merged or contained no new changes relative to '{}'.",
            feature_branch, main_branch
        );
        println!("Skipping commit.");
    } else {
        // Check if gc is available
        if !is_gc_available()? {
            eprintln!("Warning: 'gc' command not found. Changes are staged.");
            eprintln!("Please commit manually or install 'gc' from this workspace.");
            return Ok(());
        }

        // Generate commit message and commit using gc
        println!("Staging changes and generating commit message using gc...");
        let context_msg = format!("Commit history from '{}':\n{}", feature_branch, branch_history);

        let last_commit_before = get_current_commit()?;
        println!("Last commit before gc: {}", last_commit_before);

        // Run gc with context
        let status = Command::new("gc")
            .args(["--context", &context_msg])
            .status()
            .context("Failed to run gc")?;

        if !status.success() {
            bail!("'gc' failed. The squashed changes are staged. Please commit manually.");
        }

        let last_commit_after = get_current_commit()?;
        println!("Last commit after gc: {}", last_commit_after);

        if last_commit_before == last_commit_after {
            bail!(
                "'gc' script completed, but no new commit was created on '{}'. The squashed changes are likely still staged. Please investigate and commit manually.",
                main_branch
            );
        }
        println!("New commit successfully created by gc.");
    }

    // Clean up local branch
    println!("Force deleting local branch '{}'...", feature_branch);
    let status = Command::new("git")
        .args(["branch", "-D", feature_branch])
        .status()
        .context("Failed to delete branch")?;

    if !status.success() {
        eprintln!(
            "Warning: Failed to force delete local branch '{}'. Check permissions or other issues.",
            feature_branch
        );
    }

    Ok(())
}

fn check_for_conflicts() -> Result<bool> {
    let output = Command::new("git")
        .args(["status", "--porcelain"])
        .output()
        .context("Failed to check for conflicts")?;

    let status_output = String::from_utf8(output.stdout)?;
    Ok(status_output.lines().any(|line| {
        line.starts_with("AA") || line.starts_with("UU") || line.starts_with("DD") ||
        line.starts_with("AU") || line.starts_with("UA") || line.starts_with("DU") ||
        line.starts_with("UD")
    }))
}

fn is_gc_available() -> Result<bool> {
    let status = Command::new("gc")
        .arg("--version")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();

    Ok(status.map(|s| s.success()).unwrap_or(false))
}

fn get_current_commit() -> Result<String> {
    let output = Command::new("git")
        .args(["rev-parse", "HEAD"])
        .output()
        .context("Failed to get current commit SHA")?;

    if !output.status.success() {
        bail!("Failed to get current commit SHA");
    }

    Ok(String::from_utf8(output.stdout)?.trim().to_string())
}
