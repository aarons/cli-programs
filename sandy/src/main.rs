mod config;
mod docker;
mod interactive;
mod state;
mod worktree;

use anyhow::{Context, Result, bail};
use clap::{Parser, Subcommand};
use std::env;
use std::path::PathBuf;

use config::Config;
use docker::{
    DefaultTemplateStatus, backup_dockerfile, build_template, build_template_no_cache,
    check_default_template_status, check_docker, check_docker_sandbox, new_default_available,
    remove_sandbox, start_sandbox, template_exists, template_needs_rebuild,
    update_dockerfile_from_default,
};
use interactive::{confirm, display_sandbox_list, get_sandbox_entries, prompt_selection};
use state::State;
use worktree::{get_repo_name, get_workspace_path};

/// Default template image name used when no custom template is configured
const DEFAULT_TEMPLATE_IMAGE: &str = "sandy-dev";

#[derive(Parser)]
#[command(name = "sandy")]
#[command(about = "Manage Claude Code development environments in Docker containers")]
#[command(version)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    /// Create a new sandbox for the current repository
    New {
        /// CLI tool to run (claude, gemini, codex). Defaults to config value.
        #[arg(long, short)]
        tool: Option<String>,
    },
    /// Resume an existing sandbox (interactive selection)
    Resume,
    /// List all sandbox environments
    List,
    /// Remove a sandbox environment (interactive selection)
    Remove,
    /// Build or rebuild the sandbox template image
    Build {
        /// Force a complete rebuild, ignoring Docker's build cache
        #[arg(long, short)]
        force: bool,
    },
    /// Update the Dockerfile template to the latest embedded default
    Update {
        /// Force update even if Dockerfile has been customized (creates backup)
        #[arg(long, short)]
        force: bool,
    },
    /// Manage settings (sandy.toml) and Dockerfile template
    #[command(long_about = "Manage sandy configuration.\n\n\
        Sandy uses two configuration files:\n\n\
        1. sandy.toml - Settings like template image name, volume mounts, and environment variables\n\
        2. Dockerfile - The Docker image template that defines what's installed in your sandbox\n\n\
        Use 'sandy config show' to view settings, 'sandy config dockerfile' to view the Dockerfile.")]
    Config {
        #[command(subcommand)]
        action: ConfigAction,
    },
}

#[derive(Subcommand)]
enum ConfigAction {
    /// Show current configuration (sandy.toml settings)
    Show,
    /// Set a configuration value in sandy.toml
    Set {
        /// Configuration key
        key: String,
        /// Configuration value
        value: String,
    },
    /// Show Dockerfile path and contents
    Dockerfile,
    /// Create or reset the Dockerfile template for customization
    CreateDockerfile,
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Some(Commands::New { tool }) => cmd_new(tool),
        Some(Commands::Resume) => cmd_resume(),
        Some(Commands::List) => cmd_list(),
        Some(Commands::Remove) => cmd_remove(),
        Some(Commands::Build { force }) => cmd_build(force),
        Some(Commands::Update { force }) => cmd_update(force),
        Some(Commands::Config { action }) => cmd_config(action),
        None => cmd_interactive(),
    }
}

/// Check if a sandbox exists for the current working directory
/// Returns the workspace name if it exists, None otherwise
fn current_workspace_has_sandbox() -> Option<String> {
    let cwd = env::current_dir().ok()?;
    let workspace_path = get_workspace_path(&cwd);
    let workspace_key = workspace_path.to_string_lossy().to_string();
    let state = State::load().ok()?;

    if state.sandboxes.contains_key(&workspace_key) {
        Some(get_repo_name(&workspace_path))
    } else {
        None
    }
}

/// Interactive menu when no subcommand is provided
fn cmd_interactive() -> Result<()> {
    use std::io::{self, Write};

    println!("sandy - Claude Code Development Environments\n");

    loop {
        let sandbox_exists = current_workspace_has_sandbox();

        println!("What would you like to do?\n");

        // Option 1: Context-aware Resume/New
        if let Some(ref name) = sandbox_exists {
            println!("  1. Resume         - Resume sandbox for '{}'", name);
        } else {
            println!("  1. New            - Create a new sandbox for current directory");
        }

        println!("  2. Rebuild & New  - Fresh sandbox with template rebuild");
        println!("  3. List           - List all sandboxes");
        println!("  4. Remove         - Remove a sandbox");
        println!("  5. Config         - Show configuration");
        println!("  q. Quit\n");

        print!("Select an option: ");
        io::stdout().flush()?;

        let mut input = String::new();
        io::stdin().read_line(&mut input)?;
        let input = input.trim();

        match input {
            "1" => {
                if sandbox_exists.is_some() {
                    return cmd_resume();
                } else {
                    return cmd_new(None);
                }
            }
            "2" => {
                return cmd_rebuild_and_new(None);
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

fn cmd_new(tool_override: Option<String>) -> Result<()> {
    // Check Docker availability
    check_docker()?;
    check_docker_sandbox()?;

    // Load configuration
    let mut config = Config::load()?;
    let mut state = State::load()?;

    // Resolve tool: flag overrides config default
    let tool = tool_override.unwrap_or_else(|| config.default_tool.clone());

    // Get current workspace (git root or current directory)
    let cwd = env::current_dir().context("Failed to get current directory")?;
    let workspace_path = get_workspace_path(&cwd);
    let workspace_key = workspace_path.to_string_lossy().to_string();
    let workspace_name = get_repo_name(&workspace_path);

    // Check if sandbox already exists for this workspace
    if state.sandboxes.contains_key(&workspace_key) {
        bail!(
            "Sandbox already exists for '{}'. Use 'sandy resume' to continue.",
            workspace_name
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
            println!("Updated Dockerfile at: {}", template_dockerfile.display());
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

    // Save state with tool info
    state.add_sandbox(workspace_path.clone(), &tool);
    state.save()?;

    println!("Starting sandbox for '{}' with {}...", workspace_name, tool);

    // Start the sandbox
    start_sandbox(&workspace_path, &config, &tool)?;

    Ok(())
}

fn cmd_rebuild_and_new(tool_override: Option<String>) -> Result<()> {
    // Check Docker availability
    check_docker()?;
    check_docker_sandbox()?;

    // Load configuration
    let mut config = Config::load()?;
    let mut state = State::load()?;

    // Resolve tool: flag overrides config default
    let tool = tool_override.unwrap_or_else(|| config.default_tool.clone());

    // Get current workspace (git root or current directory)
    let cwd = env::current_dir().context("Failed to get current directory")?;
    let workspace_path = get_workspace_path(&cwd);
    let workspace_key = workspace_path.to_string_lossy().to_string();
    let workspace_name = get_repo_name(&workspace_path);

    // Remove existing sandbox if present
    if state.sandboxes.contains_key(&workspace_key) {
        println!("Removing existing sandbox for '{}'...", workspace_name);
        let _ = remove_sandbox(&workspace_path);
        state.remove_sandbox(&workspace_key);
        state.save()?;
    }

    // Get or create template name
    let template_name = config
        .template_image
        .clone()
        .unwrap_or_else(|| DEFAULT_TEMPLATE_IMAGE.to_string());
    let template_dockerfile = get_template_dockerfile()?;

    // Check template status and handle updates
    let template_status = check_default_template_status(&template_dockerfile, DEFAULT_DOCKERFILE)?;

    match template_status {
        DefaultTemplateStatus::NeedsCreation => {
            println!("Creating default Dockerfile...");
            update_dockerfile_from_default(&template_dockerfile, DEFAULT_DOCKERFILE)?;
            println!(
                "Created default Dockerfile at: {}",
                template_dockerfile.display()
            );
        }
        DefaultTemplateStatus::NeedsUpdate => {
            println!("Updating Dockerfile to latest default...");
            update_dockerfile_from_default(&template_dockerfile, DEFAULT_DOCKERFILE)?;
            println!("Updated Dockerfile at: {}", template_dockerfile.display());
        }
        DefaultTemplateStatus::UpToDate => {
            // Nothing to do, Dockerfile is current
        }
        DefaultTemplateStatus::Customized => {
            // Check if new default is available and warn
            if new_default_available(DEFAULT_DOCKERFILE)? {
                println!("Note: A new default Dockerfile template is available.");
                println!(
                    "Your Dockerfile has been customized, so it was not updated automatically."
                );
                println!(
                    "Run 'sandy update --force' to update (your current file will be backed up)."
                );
                println!();
            }
        }
    }

    // Force rebuild template (no cache)
    println!("Rebuilding template (ignoring Docker cache)...");
    build_template_no_cache(&template_dockerfile, &template_name, &config)?;

    // Update config with template_image if not already set
    if config.template_image.is_none() {
        config.template_image = Some(template_name);
        config.save()?;
    }

    // Save state with tool info
    state.add_sandbox(workspace_path.clone(), &tool);
    state.save()?;

    println!("Starting sandbox for '{}' with {}...", workspace_name, tool);

    // Start the sandbox
    start_sandbox(&workspace_path, &config, &tool)?;

    Ok(())
}

fn cmd_resume() -> Result<()> {
    check_docker()?;
    check_docker_sandbox()?;

    let config = Config::load()?;
    let state = State::load()?;

    // Try to auto-select sandbox for current working directory
    if let Ok(cwd) = env::current_dir() {
        let workspace_path = get_workspace_path(&cwd);
        let workspace_key = workspace_path.to_string_lossy().to_string();
        if let Some(info) = state.sandboxes.get(&workspace_key) {
            let repo_name = get_repo_name(&info.path);
            // Use stored tool, or fall back to config default for legacy sandboxes
            let tool = info
                .tool
                .clone()
                .unwrap_or_else(|| config.default_tool.clone());
            println!("Resuming sandbox '{}' with {}...", repo_name, tool);
            start_sandbox(&info.path, &config, &tool)?;
            return Ok(());
        }
    }

    // Fall back to interactive selection
    let entries = get_sandbox_entries(&state)?;
    if entries.is_empty() {
        println!("No sandboxes found. Create one with 'sandy new'");
        return Ok(());
    }

    let entry = match prompt_selection(&entries)? {
        Some(e) => e,
        None => return Ok(()),
    };

    // Use stored tool, or fall back to config default for legacy sandboxes
    let tool = entry
        .info
        .tool
        .clone()
        .unwrap_or_else(|| config.default_tool.clone());

    // Docker Sandbox handles reconnection automatically - just call run again
    println!("Resuming sandbox '{}' with {}...", entry.name, tool);
    start_sandbox(&entry.info.path, &config, &tool)?;

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

fn cmd_build(force: bool) -> Result<()> {
    check_docker()?;

    let mut config = Config::load()?;

    // Get or create template name
    let template_name = config
        .template_image
        .clone()
        .unwrap_or_else(|| DEFAULT_TEMPLATE_IMAGE.to_string());
    let template_dockerfile = get_template_dockerfile()?;

    // Check template status and handle updates
    let template_status = check_default_template_status(&template_dockerfile, DEFAULT_DOCKERFILE)?;

    match template_status {
        DefaultTemplateStatus::NeedsCreation => {
            println!("Creating default Dockerfile...");
            update_dockerfile_from_default(&template_dockerfile, DEFAULT_DOCKERFILE)?;
            println!(
                "Created default Dockerfile at: {}",
                template_dockerfile.display()
            );
        }
        DefaultTemplateStatus::NeedsUpdate => {
            println!("Updating Dockerfile to latest default...");
            update_dockerfile_from_default(&template_dockerfile, DEFAULT_DOCKERFILE)?;
            println!("Updated Dockerfile at: {}", template_dockerfile.display());
        }
        DefaultTemplateStatus::UpToDate => {
            // Nothing to do, Dockerfile is current
        }
        DefaultTemplateStatus::Customized => {
            // Check if new default is available and warn
            if new_default_available(DEFAULT_DOCKERFILE)? {
                println!("Note: A new default Dockerfile template is available.");
                println!(
                    "Your Dockerfile has been customized, so it was not updated automatically."
                );
                println!(
                    "Run 'sandy update --force' to update (your current file will be backed up)."
                );
                println!();
            }
        }
    }

    // Build the template
    if force {
        println!("Force rebuilding template (ignoring Docker cache)...");
        build_template_no_cache(&template_dockerfile, &template_name, &config)?;
    } else {
        println!("Building template...");
        build_template(&template_dockerfile, &template_name, &config)?;
    }

    // Update config with template_image if not already set
    if config.template_image.is_none() {
        config.template_image = Some(template_name);
        config.save()?;
    }

    Ok(())
}

fn cmd_update(force: bool) -> Result<()> {
    let template_dockerfile = get_template_dockerfile()?;
    let template_status = check_default_template_status(&template_dockerfile, DEFAULT_DOCKERFILE)?;

    match template_status {
        DefaultTemplateStatus::NeedsCreation => {
            println!("Creating default Dockerfile...");
            update_dockerfile_from_default(&template_dockerfile, DEFAULT_DOCKERFILE)?;
            println!(
                "Created default Dockerfile at: {}",
                template_dockerfile.display()
            );
        }
        DefaultTemplateStatus::NeedsUpdate => {
            println!("Updating Dockerfile to latest default...");
            update_dockerfile_from_default(&template_dockerfile, DEFAULT_DOCKERFILE)?;
            println!("Updated Dockerfile at: {}", template_dockerfile.display());
        }
        DefaultTemplateStatus::UpToDate => {
            println!("Dockerfile is already up to date.");
        }
        DefaultTemplateStatus::Customized => {
            if force {
                let backup_path = backup_dockerfile(&template_dockerfile)?;
                println!(
                    "Backed up customized Dockerfile to: {}",
                    backup_path.display()
                );
                update_dockerfile_from_default(&template_dockerfile, DEFAULT_DOCKERFILE)?;
                println!("Updated Dockerfile to latest default.");
            } else {
                println!("Your Dockerfile has been customized and differs from the default.");
                println!();
                println!("To update to the latest default template, run: sandy update --force");
                println!("This will back up your current Dockerfile before updating.");
            }
        }
    }

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
        ConfigAction::Dockerfile => {
            let dockerfile_path = get_template_dockerfile()?;
            println!("Dockerfile path: {}", dockerfile_path.display());
            println!("{:-<60}", "");

            if dockerfile_path.exists() {
                let contents = std::fs::read_to_string(&dockerfile_path)
                    .context("Failed to read Dockerfile")?;
                println!("{}", contents);
            } else {
                println!("(Dockerfile does not exist yet)");
                println!();
                println!(
                    "A default Dockerfile will be created automatically when you run 'sandy new'."
                );
                println!("To create it now for customization, run: sandy config create-dockerfile");
            }
        }
        ConfigAction::Set { key, value } => {
            let mut config = Config::load()?;

            match key.as_str() {
                "template_image" => config.template_image = Some(value),
                _ => bail!(
                    "Unknown configuration key: {}. Valid keys: template_image",
                    key
                ),
            }

            config.save()?;
            println!("Configuration updated.");
        }
        ConfigAction::CreateDockerfile => {
            let template_path = get_template_dockerfile()?;

            if template_path.exists() && !confirm("Template Dockerfile already exists. Overwrite?")?
            {
                return Ok(());
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
            println!("Changes will be automatically built on your next 'sandy new'.");
        }
    }

    Ok(())
}

/// Get the path to the user's template Dockerfile
fn get_template_dockerfile() -> Result<PathBuf> {
    Ok(Config::config_dir()?.join("sandy").join("Dockerfile"))
}

/// Default Dockerfile template loaded from template/Dockerfile at compile time
const DEFAULT_DOCKERFILE: &str = include_str!("../template/Dockerfile");
