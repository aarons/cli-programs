mod config;
mod git;
mod launchd;
mod log;

use anyhow::{Context, Result};
use chrono::Local;
use clap::{Parser, Subcommand};
use std::io::{self, Write};
use std::path::PathBuf;

use config::Config;
use log::{CommitLog, LogEntry};

#[derive(Parser, Debug)]
#[command(name = "track-changes")]
#[command(about = "Watch directories and auto-commit changes with timestamps")]
#[command(version)]
struct Cli {
    /// Directory to add and immediately check for changes
    #[arg(short, long)]
    dir: Option<PathBuf>,

    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Add a directory to the watch list and trigger initial commit
    #[command(after_help = "Tip: Use \".\" for the current directory")]
    Add {
        /// Directory path to add (use "." for current directory)
        directory: PathBuf,
    },
    /// Remove a directory from the watch list
    #[command(after_help = "Tip: Use \".\" for the current directory")]
    Remove {
        /// Directory path to remove (use "." for current directory)
        directory: PathBuf,
    },
    /// List all watched directories with status
    List,
    /// Commit changes in all watched directories now
    Now,
    /// Install launchd plist for hourly runs
    Install,
    /// Remove launchd plist
    Uninstall,
    /// Show recent commit log
    Log {
        /// Number of entries to show
        #[arg(short, long, default_value = "20")]
        count: usize,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    match (&cli.dir, &cli.command) {
        // --dir <directory>: Add to watch list AND run commit check
        (Some(dir), None) => {
            cmd_add_directory(dir)?;
            run_commit_for_directory(dir)?;
        }
        // No args: Show help
        (None, None) => {
            use clap::CommandFactory;
            Cli::command().print_help()?;
        }
        // Subcommands
        (None, Some(Commands::Add { directory })) => {
            let newly_added = cmd_add_directory(directory)?;
            if newly_added {
                // Trigger initial commit for newly added directories
                run_commit_for_directory(directory)?;
            }
        }
        (None, Some(Commands::Remove { directory })) => cmd_remove_directory(directory)?,
        (None, Some(Commands::List)) => cmd_list()?,
        (None, Some(Commands::Now)) => cmd_run_all()?,
        (None, Some(Commands::Install)) => launchd::install()?,
        (None, Some(Commands::Uninstall)) => launchd::uninstall()?,
        (None, Some(Commands::Log { count })) => cmd_show_log(*count)?,
        // Error: --dir with subcommand
        (Some(_), Some(_)) => {
            anyhow::bail!("Cannot use --dir with a subcommand");
        }
    }

    Ok(())
}

/// Prompt the user for a yes/no confirmation
fn prompt_yes_no(prompt: &str) -> Result<bool> {
    print!("{} [y/N]: ", prompt);
    io::stdout().flush().context("Failed to flush stdout")?;

    let mut input = String::new();
    io::stdin()
        .read_line(&mut input)
        .context("Failed to read input")?;

    let answer = input.trim().to_lowercase();
    Ok(answer == "y" || answer == "yes")
}

/// Add a directory to the watch list
/// Returns Ok(true) if the directory was newly added, Ok(false) if already watching
fn cmd_add_directory(path: &PathBuf) -> Result<bool> {
    // Validate it's a git repo, offer to initialize if not
    if !git::is_git_repo(path) {
        println!("Not a git repository: {}", path.display());

        if prompt_yes_no("Initialize a git repository here?")? {
            git::init_repo(path)?;
            println!("Initialized git repository");
        } else {
            anyhow::bail!("Cannot add non-git directory to watch list");
        }
    }

    let mut config = Config::load()?;
    let added = config.add_directory(path)?;

    if added {
        config.save()?;
        println!("Added: {}", path.canonicalize()?.display());
    } else {
        println!("Already watching: {}", path.canonicalize()?.display());
    }

    Ok(added)
}

/// Remove a directory from the watch list
fn cmd_remove_directory(path: &PathBuf) -> Result<()> {
    let mut config = Config::load()?;
    let removed = config.remove_directory(path)?;

    if removed {
        config.save()?;
        println!("Removed: {}", path.display());
    } else {
        println!("Not in watch list: {}", path.display());
    }

    Ok(())
}

/// List all watched directories with their status
fn cmd_list() -> Result<()> {
    let config = Config::load()?;

    if config.directories.is_empty() {
        println!("No directories being watched.");
        println!("\nAdd directories with:");
        println!("  track-changes add <directory>");
        println!("  track-changes --dir <directory>");
        return Ok(());
    }

    println!("Watched directories:\n");

    for dir in &config.directories {
        println!("  {}", dir.display());

        if !dir.exists() {
            println!("    Status: directory not found");
            println!();
            continue;
        }

        if !git::is_git_repo(dir) {
            println!("    Status: NOT a git repo (will be skipped)");
            println!();
            continue;
        }

        // Check for changes
        match git::get_changed_files(dir) {
            Ok(files) => {
                if files.is_empty() {
                    println!("    Status: no pending changes");
                } else {
                    println!("    Status: {} pending change(s)", files.len());
                }
            }
            Err(e) => {
                println!("    Status: error checking status - {}", e);
            }
        }

        // Get last commit time
        match git::get_last_commit_time(dir) {
            Ok(Some(time)) => {
                println!("    Last commit: {}", time.format("%Y-%m-%d %H:%M:%S"));
            }
            Ok(None) => {
                println!("    Last commit: no commits yet");
            }
            Err(_) => {}
        }

        println!();
    }

    // Show launchd status
    if launchd::is_installed()? {
        println!("Scheduler: installed (hourly)");
    } else {
        println!("Scheduler: not installed");
        println!("  Run 'track-changes install' to enable hourly auto-commits");
    }

    Ok(())
}

/// Run commit check on all watched directories
fn cmd_run_all() -> Result<()> {
    let config = Config::load()?;

    if config.directories.is_empty() {
        println!("No directories being watched.");
        println!("Add directories with: track-changes add <directory>");
        return Ok(());
    }

    println!("Processing {} directory(ies)...\n", config.directories.len());

    let mut committed = 0;
    let mut skipped = 0;
    let mut errors = 0;

    for dir in &config.directories {
        print!("{}", dir.display());

        if !dir.exists() {
            println!(" - directory not found, skipping");
            skipped += 1;
            continue;
        }

        if !git::is_git_repo(dir) {
            println!(" - not a git repo, skipping");
            skipped += 1;
            continue;
        }

        match run_commit_for_directory(dir) {
            Ok(true) => committed += 1,
            Ok(false) => {} // No changes, already printed
            Err(e) => {
                println!(" - error: {}", e);
                errors += 1;
            }
        }
    }

    println!();
    println!(
        "Done. {} committed, {} skipped, {} errors.",
        committed, skipped, errors
    );

    Ok(())
}

/// Run commit check for a single directory
/// Returns Ok(true) if a commit was made, Ok(false) if no changes
fn run_commit_for_directory(path: &PathBuf) -> Result<bool> {
    // Check for changes
    let files = git::get_changed_files(path)?;

    if files.is_empty() {
        println!(" - no changes");
        return Ok(false);
    }

    // Commit the changes
    let hash = git::commit_with_timestamp(path)
        .with_context(|| format!("Failed to commit in {}", path.display()))?;

    println!(" - committed: {} ({} file(s))", hash, files.len());

    // Log the commit
    let entry = LogEntry {
        directory: path.clone(),
        timestamp: Local::now(),
        files_changed: files,
        commit_hash: hash,
    };

    if let Err(e) = CommitLog::append(&entry) {
        eprintln!("Warning: failed to write log entry: {}", e);
    }

    Ok(true)
}

/// Show recent commit log entries
fn cmd_show_log(count: usize) -> Result<()> {
    let entries = CommitLog::read_recent(count)?;

    if entries.is_empty() {
        println!("No commits logged yet.");
        return Ok(());
    }

    println!("Recent commits:\n");

    for entry in entries.iter().rev() {
        println!(
            "{}  {}  [{}]",
            entry.timestamp.format("%Y-%m-%d %H:%M:%S"),
            entry.directory.display(),
            entry.commit_hash
        );

        for file in &entry.files_changed {
            println!("  {}", file);
        }
        println!();
    }

    Ok(())
}
