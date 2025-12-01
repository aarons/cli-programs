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
    build_template, check_docker, check_docker_sandbox, remove_sandbox, sandbox_status,
    start_sandbox, template_exists, template_needs_rebuild, SandboxStatus,
};
use interactive::{confirm, display_sandbox_list, get_sandbox_entries, prompt_selection, prompt_string};
use state::State;
use worktree::{create_worktree, get_current_branch, get_repo_name, get_repo_root, remove_worktree};

/// Default template image name used when no custom template is configured
const DEFAULT_TEMPLATE_IMAGE: &str = "sandbox-dev";

#[derive(Parser)]
#[command(name = "sandbox")]
#[command(about = "Manage Claude Code development environments using git worktrees and Docker sandboxes")]
#[command(version)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    /// Create a new sandbox environment
    New {
        /// Name for the new sandbox
        name: String,
        /// Path to the git repository (defaults to current directory)
        #[arg(long)]
        repo: Option<PathBuf>,
        /// Branch to create the worktree from
        #[arg(long)]
        branch: Option<String>,
    },
    /// Resume an existing sandbox environment
    Resume {
        /// Name of the sandbox to resume (interactive selection if not provided)
        name: Option<String>,
    },
    /// List all sandbox environments
    List,
    /// Remove a sandbox environment
    Remove {
        /// Name of the sandbox to remove
        name: String,
        /// Also remove the git worktree
        #[arg(long)]
        worktree: bool,
    },
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
        Some(Commands::New { name, repo, branch }) => cmd_new(&name, repo, branch),
        Some(Commands::Resume { name }) => cmd_resume(name),
        Some(Commands::List) => cmd_list(),
        Some(Commands::Remove { name, worktree }) => cmd_remove(&name, worktree),
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
        println!("  1. New      - Create a new sandbox");
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
                let name = prompt_string("Sandbox name", None)?;
                if name.is_empty() {
                    println!("Name is required.\n");
                    continue;
                }
                return cmd_new(&name, None, None);
            }
            "2" | "resume" | "r" => {
                return cmd_resume(None);
            }
            "3" | "list" | "l" => {
                cmd_list()?;
                println!();
            }
            "4" | "remove" | "rm" => {
                let name = prompt_string("Sandbox name to remove", None)?;
                if name.is_empty() {
                    println!("Name is required.\n");
                    continue;
                }
                let remove_worktree = confirm("Also remove the git worktree?")?;
                return cmd_remove(&name, remove_worktree);
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

fn cmd_new(name: &str, repo: Option<PathBuf>, branch: Option<String>) -> Result<()> {
    // Check Docker availability
    check_docker()?;
    check_docker_sandbox()?;

    // Load configuration
    let mut config = Config::load()?;
    let mut state = State::load()?;

    // Determine repository path
    let repo_path = if let Some(r) = repo {
        r.canonicalize()
            .with_context(|| format!("Repository path does not exist: {}", r.display()))?
    } else {
        let cwd = env::current_dir().context("Failed to get current directory")?;
        get_repo_root(&cwd).context("Current directory is not in a git repository")?
    };

    // Get repository info
    let repo_name = get_repo_name(&repo_path);
    let source_branch = branch.clone().unwrap_or_else(|| {
        get_current_branch(&repo_path).unwrap_or_else(|_| "main".to_string())
    });

    // Check if worktree directory is configured, prompt if not
    let worktree_dir = config.worktree_dir_expanded()?;
    if !worktree_dir.exists() {
        println!("Worktree directory does not exist: {}", worktree_dir.display());
        let dir = prompt_string(
            "Enter worktree directory",
            Some(&config.worktree_dir),
        )?;
        config.worktree_dir = dir;
        config.save()?;

        // Create the directory
        let worktree_dir = config.worktree_dir_expanded()?;
        std::fs::create_dir_all(&worktree_dir)
            .with_context(|| format!("Failed to create worktree directory: {}", worktree_dir.display()))?;
    }

    // Generate worktree path
    let worktree_name = format!("{}-{}", repo_name, name);
    let worktree_path = config.worktree_dir_expanded()?.join(&worktree_name);

    if worktree_path.exists() {
        bail!("Worktree already exists: {}", worktree_path.display());
    }

    // Check if name already exists in state
    if state.worktrees.contains_key(name) {
        bail!("Sandbox with name '{}' already exists", name);
    }

    // Handle template - auto-create and build if needed
    let template_name = config
        .template_image
        .clone()
        .unwrap_or_else(|| DEFAULT_TEMPLATE_IMAGE.to_string());
    let template_dockerfile = get_template_dockerfile()?;

    // Check if we need to create and/or build the template
    let dockerfile_exists = template_dockerfile.exists();
    let image_exists = template_exists(&template_name)?;

    if !dockerfile_exists && !image_exists {
        // First-time setup: create default Dockerfile and build
        println!("Setting up sandbox template (first-time setup)...");
        let template_dir = template_dockerfile
            .parent()
            .context("Invalid template path")?;
        std::fs::create_dir_all(template_dir)?;
        std::fs::write(&template_dockerfile, DEFAULT_DOCKERFILE)?;
        println!("Created default Dockerfile at: {}", template_dockerfile.display());
        build_template(&template_dockerfile, &template_name, &config)?;
    } else if dockerfile_exists {
        // Dockerfile exists - check if rebuild needed
        let needs_build = !image_exists || template_needs_rebuild(&template_dockerfile)?;
        if needs_build {
            println!("Building sandbox template...");
            build_template(&template_dockerfile, &template_name, &config)?;
        }
    }
    // If only image exists (no dockerfile), use it as-is

    // Update config with template_image if not already set
    if config.template_image.is_none() {
        config.template_image = Some(template_name);
        config.save()?;
    }

    // Create the worktree
    println!("Creating worktree at: {}", worktree_path.display());
    create_worktree(&repo_path, &worktree_path, branch.as_deref())?;

    // Save state
    state.add_worktree(
        name.to_string(),
        worktree_path.clone(),
        repo_path,
        source_branch,
    );
    state.save()?;

    println!("Sandbox '{}' created successfully!", name);
    println!("Starting sandbox...");

    // Start the sandbox
    start_sandbox(&worktree_path, &config)?;

    Ok(())
}

fn cmd_resume(name: Option<String>) -> Result<()> {
    check_docker()?;
    check_docker_sandbox()?;

    let config = Config::load()?;
    let state = State::load()?;

    // Get the sandbox to resume
    let (sandbox_name, info) = if let Some(n) = name {
        let info = state
            .get_worktree(&n)
            .with_context(|| format!("Sandbox '{}' not found", n))?;
        (n, info.clone())
    } else {
        // Interactive selection
        let entries = get_sandbox_entries(&state)?;
        if entries.is_empty() {
            println!("No sandboxes found. Create one with 'sandbox new <name>'");
            return Ok(());
        }

        match prompt_selection(&entries)? {
            Some(entry) => (entry.name.clone(), entry.info.clone()),
            None => return Ok(()),
        }
    };

    // Check sandbox status and start if needed
    let status = sandbox_status(&info.path)?;

    match status {
        SandboxStatus::Running => {
            println!("Attaching to running sandbox '{}'...", sandbox_name);
            docker::attach_sandbox(&info.path)?;
        }
        SandboxStatus::Stopped | SandboxStatus::NotFound => {
            println!("Starting sandbox '{}'...", sandbox_name);
            start_sandbox(&info.path, &config)?;
        }
    }

    Ok(())
}

fn cmd_list() -> Result<()> {
    let state = State::load()?;
    let entries = get_sandbox_entries(&state)?;

    display_sandbox_list(&entries);

    Ok(())
}

fn cmd_remove(name: &str, remove_worktree_flag: bool) -> Result<()> {
    let mut state = State::load()?;

    let info = state
        .get_worktree(name)
        .with_context(|| format!("Sandbox '{}' not found", name))?
        .clone();

    // Remove Docker sandbox
    println!("Removing sandbox container...");
    let _ = remove_sandbox(&info.path);

    // Optionally remove worktree
    if remove_worktree_flag {
        if confirm(&format!(
            "Remove git worktree at {}?",
            info.path.display()
        ))? {
            println!("Removing worktree...");
            remove_worktree(&info.source_repo, &info.path)?;
        }
    } else {
        println!(
            "Worktree preserved at: {}\nUse --worktree to also remove it.",
            info.path.display()
        );
    }

    // Remove from state
    state.remove_worktree(name);
    state.save()?;

    println!("Sandbox '{}' removed.", name);

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
                "worktree_dir" => config.worktree_dir = value,
                "template_image" => config.template_image = Some(value),
                _ => bail!("Unknown configuration key: {}", key),
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
