mod config;
mod docker;
mod interactive;
mod state;
mod worktree;

use anyhow::{bail, Context, Result};
use chrono::Local;
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

#[derive(Parser)]
#[command(name = "sandbox")]
#[command(about = "Manage Claude Code development environments using git worktrees and Docker sandboxes")]
#[command(version)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Create a new sandbox environment
    New {
        /// Name for the new sandbox (defaults to timestamp: YYYY-MM-DD-HH-MM)
        name: Option<String>,
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
    /// Initialize the default Dockerfile template
    InitTemplate,
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::New { name, repo, branch } => cmd_new(name.as_deref(), repo, branch),
        Commands::Resume { name } => cmd_resume(name),
        Commands::List => cmd_list(),
        Commands::Remove { name, worktree } => cmd_remove(&name, worktree),
        Commands::Config { action } => cmd_config(action),
    }
}

fn cmd_new(name: Option<&str>, repo: Option<PathBuf>, branch: Option<String>) -> Result<()> {
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

    // Generate sandbox name: use provided name or timestamp
    let sandbox_name = match name {
        Some(n) => n.to_string(),
        None => Local::now().format("%Y-%m-%d-%H-%M").to_string(),
    };

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

    // Generate worktree path: worktrees/repo-name/sandbox-name
    let worktree_path = config.worktree_dir_expanded()?.join(&repo_name).join(&sandbox_name);

    if worktree_path.exists() {
        bail!("Worktree already exists: {}", worktree_path.display());
    }

    // Check if name already exists in state
    if state.worktrees.contains_key(&sandbox_name) {
        bail!("Sandbox with name '{}' already exists", sandbox_name);
    }

    // Handle template building
    if let Some(ref template_name) = config.template_image {
        let template_dockerfile = get_template_dockerfile()?;

        if template_dockerfile.exists() {
            let needs_build = !template_exists(template_name)?
                || template_needs_rebuild(&template_dockerfile)?;

            if needs_build {
                println!("Building custom template...");
                build_template(&template_dockerfile, template_name)?;
            }
        } else if !template_exists(template_name)? {
            println!("Warning: Template '{}' not found and no Dockerfile available", template_name);
            println!("Run 'sandbox config init-template' to create a default Dockerfile");
        }
    }

    // Create the worktree
    println!("Creating worktree at: {}", worktree_path.display());
    create_worktree(&repo_path, &worktree_path, branch.as_deref())?;

    // Save state
    state.add_worktree(
        sandbox_name.clone(),
        worktree_path.clone(),
        repo_path,
        source_branch,
    );
    state.save()?;

    println!("Sandbox '{}' created successfully!", sandbox_name);
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
        ConfigAction::InitTemplate => {
            let template_path = get_template_dockerfile()?;

            if template_path.exists() {
                if !confirm("Template Dockerfile already exists. Overwrite?")? {
                    return Ok(());
                }
            }

            // Create template directory
            if let Some(parent) = template_path.parent() {
                std::fs::create_dir_all(parent)?;
            }

            // Write default template
            std::fs::write(&template_path, DEFAULT_DOCKERFILE)?;

            println!("Template Dockerfile created at: {}", template_path.display());
            println!("\nTo use it, set the template_image in your config:");
            println!("  sandbox config set template_image sandbox-dev");
        }
    }

    Ok(())
}

/// Get the path to the user's template Dockerfile
fn get_template_dockerfile() -> Result<PathBuf> {
    Ok(Config::config_dir()?.join("sandbox-template").join("Dockerfile"))
}

const DEFAULT_DOCKERFILE: &str = r#"FROM docker/sandbox-templates:claude-code

# Rust toolchain
RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
ENV PATH="/home/agent/.cargo/bin:$PATH"

# Node.js tools (npm already included)
RUN npm install -g pnpm

# Tauri dependencies (for Linux container)
RUN sudo apt-get update && sudo apt-get install -y \
    libwebkit2gtk-4.1-dev \
    build-essential \
    curl \
    wget \
    file \
    libssl-dev \
    libgtk-3-dev \
    libayatana-appindicator3-dev \
    librsvg2-dev \
    && sudo rm -rf /var/lib/apt/lists/*

# Additional Rust tools
RUN cargo install cargo-watch cargo-expand
"#;
