mod launchd;

use anyhow::Result;
use clap::{Parser, Subcommand};

#[derive(Parser, Debug)]
#[command(name = "zoom-remove")]
#[command(about = "Remove Zoom's unauthorized updater services from macOS LaunchAgents")]
#[command(version)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Install daily launchd schedule to auto-remove Zoom updaters
    Install,
    /// Remove the launchd schedule
    Uninstall,
    /// Show current status (installed Zoom agents and schedule)
    Status,
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        None => cmd_remove()?,
        Some(Commands::Install) => launchd::install()?,
        Some(Commands::Uninstall) => launchd::uninstall()?,
        Some(Commands::Status) => cmd_status()?,
    }

    Ok(())
}

/// Remove all Zoom updater LaunchAgents
fn cmd_remove() -> Result<()> {
    let agents = launchd::find_zoom_agents()?;

    if agents.is_empty() {
        println!("No Zoom updater agents found.");
        return Ok(());
    }

    println!("Found {} Zoom updater agent(s):\n", agents.len());

    let mut removed = 0;
    let mut errors = 0;

    for agent in agents {
        print!("  {}", agent.display());

        match launchd::bootout_and_remove(&agent) {
            Ok(()) => {
                println!(" - removed");
                removed += 1;
            }
            Err(e) => {
                println!(" - error: {}", e);
                errors += 1;
            }
        }
    }

    println!();
    if errors == 0 {
        println!("Successfully removed {} agent(s).", removed);
    } else {
        println!(
            "Removed {} agent(s), {} error(s).",
            removed, errors
        );
    }

    Ok(())
}

/// Show current status
fn cmd_status() -> Result<()> {
    // Check for Zoom agents
    let agents = launchd::find_zoom_agents()?;

    if agents.is_empty() {
        println!("Zoom updater agents: none found");
    } else {
        println!("Zoom updater agents found:");
        for agent in &agents {
            println!("  {}", agent.display());
        }
    }

    println!();

    // Check scheduler status
    if launchd::is_installed()? {
        println!("Daily cleanup: installed");
        println!("  Run 'zoom-remove uninstall' to disable");
    } else {
        println!("Daily cleanup: not installed");
        println!("  Run 'zoom-remove install' to enable daily auto-removal");
    }

    Ok(())
}
