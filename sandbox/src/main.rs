mod config;
mod docker;
mod interactive;
mod state;
mod worktree;

use anyhow::{bail, Context, Result};
use clap::{Parser, Subcommand};
use std::env;
use std::path::PathBuf;

use config::Config;
use docker::{
    build_template, check_default_template_status, check_docker, check_docker_sandbox,
    remove_sandbox, start_sandbox, template_exists, template_needs_rebuild,
    update_dockerfile_from_default, DefaultTemplateStatus,
};
use interactive::{confirm, display_sandbox_list, get_sandbox_entries, prompt_selection};
use state::State;
use worktree::{get_repo_name, get_repo_root};

/// Default template image name used when no custom template is configured
const DEFAULT_TEMPLATE_IMAGE: &str = "sandbox-dev";

#[derive(Parser)]
#[command(name = "sandbox")]
#[command(about = "Manage Claude Code development environments in Docker sandboxes")]
#[command(version)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    /// Create a new sandbox for the current repository
    New,
    /// Resume an existing sandbox (interactive selection)
    Resume,
    /// List all sandbox environments
    List,
    /// Remove a sandbox environment (interactive selection)
    Remove,
    /// Show or modify configuration
    Config {
        #[command(subcommand)]
        action: ConfigAction,
    },
}

#[derive(Subcommand)]
enum ConfigAction {
    /// Show current configuration
    Show,
    /// Set a configuration value
    Set {
        /// Configuration key
        key: String,
        /// Configuration value
        value: String,
    },
    /// Create a Dockerfile template for customization
    CreateDockerfile,
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Some(Commands::New) => cmd_new(),
        Some(Commands::Resume) => cmd_resume(),
        Some(Commands::List) => cmd_list(),
        Some(Commands::Remove) => cmd_remove(),
        Some(Commands::Config { action }) => cmd_config(action),
        None => cmd_interactive(),
    }
}

/// Interactive menu when no subcommand is provided
fn cmd_interactive() -> Result<()> {
    use std::io::{self, Write};

    println!("sandbox - Claude Code Development Environments\n");

    loop {
        println!("What would you like to do?\n");
        println!("  1. New      - Create a new sandbox for current repo");
        println!("  2. Resume   - Resume an existing sandbox");
        println!("  3. List     - List all sandboxes");
        println!("  4. Remove   - Remove a sandbox");
        println!("  5. Config   - Show configuration");
        println!("  q. Quit\n");

        print!("Select an option: ");
        io::stdout().flush()?;

        let mut input = String::new();
        io::stdin().read_line(&mut input)?;
        let input = input.trim();

        match input {
            "1" | "new" | "n" => {
                return cmd_new();
            }
            "2" | "resume" | "r" => {
                return cmd_resume();
            }
            "3" | "list" | "l" => {
                cmd_list()?;
                println!();
            }
            "4" | "remove" | "rm" => {
                return cmd_remove();
            }
            "5" | "config" | "c" => {
                cmd_config(ConfigAction::Show)?;
                println!();
            }
            "q" | "quit" | "exit" => {
                return Ok(());
            }
            _ => {
                println!("Invalid option.\n");
            }
        }
    }
}

fn cmd_new() -> Result<()> {
    // Check Docker availability
    check_docker()?;
    check_docker_sandbox()?;

    // Load configuration
    let mut config = Config::load()?;
    let mut state = State::load()?;

    // Get current repository
    let cwd = env::current_dir().context("Failed to get current directory")?;
    let repo_path = get_repo_root(&cwd).context("Current directory is not in a git repository")?;
    let repo_key = repo_path.to_string_lossy().to_string();
    let repo_name = get_repo_name(&repo_path);

    // Check if sandbox already exists for this repo
    if state.sandboxes.contains_key(&repo_key) {
        bail!(
            "Sandbox already exists for '{}'. Use 'sandbox resume' to continue.",
            repo_name
        );
    }

    // Handle template - auto-create, update, and build as needed
    let template_name = config
        .template_image
        .clone()
        .unwrap_or_else(|| DEFAULT_TEMPLATE_IMAGE.to_string());
    let template_dockerfile = get_template_dockerfile()?;

    // Check if we need to update the Dockerfile from the embedded default
    let template_status = check_default_template_status(&template_dockerfile, DEFAULT_DOCKERFILE)?;
    let image_exists = template_exists(&template_name)?;

    match template_status {
        DefaultTemplateStatus::NeedsCreation => {
            // First-time setup: create default Dockerfile and build
            println!("Setting up sandbox template (first-time setup)...");
            update_dockerfile_from_default(&template_dockerfile, DEFAULT_DOCKERFILE)?;
            println!(
                "Created default Dockerfile at: {}",
                template_dockerfile.display()
            );
            build_template(&template_dockerfile, &template_name, &config)?;
        }
        DefaultTemplateStatus::NeedsUpdate => {
            // Embedded default has changed - update user's Dockerfile and rebuild
            println!("Updating sandbox template to latest version...");
            update_dockerfile_from_default(&template_dockerfile, DEFAULT_DOCKERFILE)?;
            println!(
                "Updated Dockerfile at: {}",
                template_dockerfile.display()
            );
            build_template(&template_dockerfile, &template_name, &config)?;
        }
        DefaultTemplateStatus::UpToDate | DefaultTemplateStatus::Customized => {
            // Dockerfile is current or customized - only rebuild if needed
            let needs_build = !image_exists || template_needs_rebuild(&template_dockerfile)?;
            if needs_build {
                println!("Building sandbox template...");
                build_template(&template_dockerfile, &template_name, &config)?;
            }
        }
    }

    // Update config with template_image if not already set
    if config.template_image.is_none() {
        config.template_image = Some(template_name);
        config.save()?;
    }

    // Save state
    state.add_sandbox(repo_path.clone());
    state.save()?;

    println!("Starting sandbox for '{}'...", repo_name);

    // Start the sandbox in the repo directory
    start_sandbox(&repo_path, &config)?;

    Ok(())
}

fn cmd_resume() -> Result<()> {
    check_docker()?;
    check_docker_sandbox()?;

    let config = Config::load()?;
    let state = State::load()?;

    // Interactive selection
    let entries = get_sandbox_entries(&state)?;
    if entries.is_empty() {
        println!("No sandboxes found. Create one with 'sandbox new'");
        return Ok(());
    }

    let entry = match prompt_selection(&entries)? {
        Some(e) => e,
        None => return Ok(()),
    };

    // Docker Sandbox handles reconnection automatically - just call run again
    println!("Resuming sandbox '{}'...", entry.name);
    start_sandbox(&entry.info.path, &config)?;

    Ok(())
}

fn cmd_list() -> Result<()> {
    let state = State::load()?;
    let entries = get_sandbox_entries(&state)?;

    display_sandbox_list(&entries);

    Ok(())
}

fn cmd_remove() -> Result<()> {
    let mut state = State::load()?;

    // Interactive selection
    let entries = get_sandbox_entries(&state)?;
    if entries.is_empty() {
        println!("No sandboxes found.");
        return Ok(());
    }

    let entry = match prompt_selection(&entries)? {
        Some(e) => e,
        None => return Ok(()),
    };

    if !confirm(&format!("Remove sandbox for '{}'?", entry.name))? {
        return Ok(());
    }

    // Remove Docker sandbox
    println!("Removing sandbox container...");
    let _ = remove_sandbox(&entry.info.path);

    // Remove from state
    state.remove_sandbox(&entry.key);
    state.save()?;

    println!("Sandbox '{}' removed.", entry.name);

    Ok(())
}

fn cmd_config(action: ConfigAction) -> Result<()> {
    match action {
        ConfigAction::Show => {
            let config = Config::load()?;
            let toml_str = toml::to_string_pretty(&config)?;
            println!("Configuration file: {}", Config::config_path()?.display());
            println!("{:-<60}", "");
            println!("{}", toml_str);
        }
        ConfigAction::Set { key, value } => {
            let mut config = Config::load()?;

            match key.as_str() {
                "template_image" => config.template_image = Some(value),
                _ => bail!("Unknown configuration key: {}. Valid keys: template_image", key),
            }

            config.save()?;
            println!("Configuration updated.");
        }
        ConfigAction::CreateDockerfile => {
            let template_path = get_template_dockerfile()?;

            if template_path.exists() {
                if !confirm("Template Dockerfile already exists. Overwrite?")? {
                    return Ok(());
                }
            }

            // Create template directory
            let template_dir = template_path.parent().context("Invalid template path")?;
            std::fs::create_dir_all(template_dir)?;

            // Write default template
            std::fs::write(&template_path, DEFAULT_DOCKERFILE)?;

            println!(
                "Template Dockerfile created at: {}",
                template_path.display()
            );
            println!("\nEdit this file to customize your sandbox environment.");
            println!("Changes will be automatically built on your next 'sandbox new'.");
        }
    }

    Ok(())
}

/// Get the path to the user's template Dockerfile
fn get_template_dockerfile() -> Result<PathBuf> {
    Ok(Config::config_dir()?.join("sandbox").join("Dockerfile"))
}

/// Default Dockerfile template loaded from template/Dockerfile at compile time
const DEFAULT_DOCKERFILE: &str = include_str!("../template/Dockerfile");
