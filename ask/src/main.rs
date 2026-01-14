mod llm;

use anyhow::{Context, Result};
use clap::{CommandFactory, Parser, Subcommand};
use llm::LlmClient;
use llm_client::{Config, ModelPreset};
use std::io::{self, BufRead, IsTerminal, Read, Write};
use std::path::PathBuf;
use std::process::{Command, Stdio};

const SHELL_SYSTEM_PROMPT: &str = "This is a user question directly from their MacOS command line. Respond with a single example of a solution to their question. Important: Only provide valid zsh bash commands, do not use markup such as triple backticks.";

#[derive(Parser, Debug)]
#[command(
    name = "ask",
    about = "Standalone Ask Helper using LLM providers",
    long_about = "Provides command line assistance and general AI interaction using configurable LLM providers"
)]
#[command(version)]
struct Args {
    /// General question mode (doesn't apply shell prompt or copy to clipboard)
    #[arg(short, long)]
    general: bool,

    /// Enable debug mode for verbose output
    #[arg(short, long, default_value_t = false)]
    debug: bool,

    /// Model preset to use (overrides default from config)
    #[arg(short, long)]
    model: Option<String>,

    /// File(s) to include in the request (for multimodal models)
    #[arg(short, long = "file", value_name = "PATH")]
    files: Vec<PathBuf>,

    /// JSON schema for structured output (file path or inline JSON)
    #[arg(short, long, value_name = "FILE_OR_JSON")]
    json: Option<String>,

    /// Configuration subcommand
    #[command(subcommand)]
    command: Option<Commands>,

    /// The question to ask (can also be piped via stdin)
    #[arg(trailing_var_arg = true)]
    question: Vec<String>,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Configuration management
    Config {
        #[command(subcommand)]
        action: ConfigAction,
    },
    /// Shell integration setup
    Setup {
        #[command(subcommand)]
        action: Option<SetupAction>,
    },
}

#[derive(Subcommand, Debug)]
enum SetupAction {
    /// Check if shell integration is installed
    Check,
    /// Install shell integration to your shell config
    Install,
}

#[derive(Subcommand, Debug)]
enum ConfigAction {
    /// Set the default model preset
    SetDefault {
        /// Name of the preset to use as default
        preset: String,
    },
    /// List available presets
    List,
    /// Show current configuration
    Show,
    /// Add a new preset
    AddPreset {
        /// Preset name
        name: String,
        /// Provider (claude-cli, anthropic, openrouter, cerebras)
        #[arg(short, long)]
        provider: String,
        /// Model identifier
        #[arg(short = 'M', long)]
        model: String,
    },
}

/// Handle config subcommands
fn handle_config_command(action: &ConfigAction) -> Result<()> {
    match action {
        ConfigAction::SetDefault { preset } => {
            let mut config = Config::load()?;
            // Verify preset exists
            config.get_preset(preset)?;
            config.defaults.insert("ask".to_string(), preset.clone());
            config.save()?;
            println!("Default preset for ask set to: {}", preset);
        }
        ConfigAction::List => {
            let config = Config::load()?;
            let current_default = config.get_default_for_program("ask");
            println!("Available presets:");
            for (name, preset) in &config.presets {
                let default_marker = if name == current_default {
                    " (default)"
                } else {
                    ""
                };
                println!(
                    "  {} - {} / {}{}",
                    name, preset.provider, preset.model, default_marker
                );
            }
        }
        ConfigAction::Show => {
            let config = Config::load()?;
            let path = Config::config_path()?;
            println!("Config file: {}", path.display());
            println!();
            println!("{:#?}", config);
        }
        ConfigAction::AddPreset {
            name,
            provider,
            model,
        } => {
            let mut config = Config::load()?;
            config.presets.insert(
                name.clone(),
                ModelPreset {
                    provider: provider.clone(),
                    model: model.clone(),
                    fallback: None,
                    api_key_env: None,
                },
            );
            config.save()?;
            println!("Added preset: {}", name);
        }
    }
    Ok(())
}

/// Get shell name and RC file path
fn get_shell_info() -> Option<(&'static str, PathBuf)> {
    let shell = std::env::var("SHELL").ok()?;
    let shell_name = std::path::Path::new(&shell).file_name()?.to_str()?;
    let home = std::env::var("HOME").ok()?;

    match shell_name {
        "zsh" => Some(("zsh", PathBuf::from(home).join(".zshrc"))),
        "bash" => Some(("bash", PathBuf::from(home).join(".bashrc"))),
        _ => None,
    }
}

/// Check if shell integration is installed (returns shell name, rc file, and whether installed)
fn check_shell_integration() -> Result<Option<(&'static str, PathBuf, bool)>> {
    let Some((shell_name, rc_file)) = get_shell_info() else {
        return Ok(None);
    };

    if !rc_file.exists() {
        return Ok(Some((shell_name, rc_file, false)));
    }

    let content = std::fs::read_to_string(&rc_file)?;
    let has_integration =
        content.contains("alias ask=") || content.contains("ask()") || content.contains("ask ()");

    Ok(Some((shell_name, rc_file, has_integration)))
}

/// Get the shell integration code for a given shell
fn get_shell_integration_code(shell_name: &str) -> &'static str {
    match shell_name {
        "zsh" => "alias ask='noglob command ask'",
        "bash" => {
            r#"ask() {
  set -f
  command ask "$@"
  local ret=$?
  set +f
  return $ret
}"#
        }
        _ => unreachable!(),
    }
}

/// Install shell integration to the rc file
fn do_install(shell_name: &str, rc_file: &PathBuf) -> Result<()> {
    let integration_code = get_shell_integration_code(shell_name);
    let full_block = format!(
        "\n# ask shell integration - handle special characters without quoting\n{}\n",
        integration_code
    );

    use std::fs::OpenOptions;
    let mut file = OpenOptions::new().append(true).create(true).open(rc_file)?;
    file.write_all(full_block.as_bytes())?;

    println!("\nInstalled! Run this to activate:");
    println!("  source {}", rc_file.display());
    Ok(())
}

/// Handle setup subcommands
fn handle_setup_command(action: Option<&SetupAction>) -> Result<()> {
    // Get shell info
    let Some((shell_name, rc_file, is_installed)) = check_shell_integration()? else {
        println!("Unknown shell. Supported shells: zsh, bash");
        return Ok(());
    };

    // For explicit check command, just show status
    if matches!(action, Some(SetupAction::Check)) {
        println!("Shell: {}", shell_name);
        println!("Config: {}", rc_file.display());
        if is_installed {
            println!("\nStatus: Shell integration is installed");
        } else {
            println!("\nStatus: Shell integration is NOT installed");
            println!("\nRun `ask setup` to install.");
        }
        return Ok(());
    }

    // For setup (no subcommand) or setup install, show full info and offer to install
    println!("Shell Integration Setup");
    println!("=======================");
    println!();
    println!("Shell integration allows you to use special characters without quoting.");
    println!();
    println!("Without integration:");
    println!("  ask 'how do I find files matching *.txt?'   # quotes required");
    println!();
    println!("With integration:");
    println!("  ask how do I find files matching *.txt?     # just works");
    println!();
    println!("This handles glob characters (? *) and history expansion (!) but not");
    println!("shell syntax like pipes (|), redirects (>), or semicolons (;).");
    println!();
    println!("---");
    println!();
    println!("Shell:  {}", shell_name);
    println!("Config: {}", rc_file.display());

    if is_installed {
        println!();
        println!("Status: Already installed");
        return Ok(());
    }

    println!();
    println!("Status: Not installed");

    let integration_code = get_shell_integration_code(shell_name);

    println!();
    println!("To install manually, add this to {}:", rc_file.display());
    println!();
    println!("  # ask shell integration - handle special characters without quoting");
    for line in integration_code.lines() {
        println!("  {}", line);
    }

    println!();
    eprint!("Install automatically? [y/N] ");
    io::stderr().flush().ok();

    let mut response = String::new();
    io::stdin().lock().read_line(&mut response)?;

    if response.trim().eq_ignore_ascii_case("y") {
        do_install(shell_name, &rc_file)?;
    } else {
        println!("Skipped.");
    }

    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();

    // Handle config subcommands first (before LLM initialization)
    if let Some(Commands::Config { action }) = &args.command {
        return handle_config_command(action);
    }

    // Handle setup subcommand
    if let Some(Commands::Setup { action }) = &args.command {
        return handle_setup_command(action.as_ref());
    }

    // Get the question from args
    let question = args.question.join(" ");

    // Check for piped input (ignore if empty/whitespace-only)
    let piped_input = if !io::stdin().is_terminal() {
        let mut buffer = String::new();
        io::stdin()
            .read_to_string(&mut buffer)
            .context("Failed to read piped input")?;
        let trimmed = buffer.trim();
        if trimmed.is_empty() {
            None
        } else {
            Some(buffer)
        }
    } else {
        None
    };

    // If no input provided, show help
    if question.is_empty() && piped_input.is_none() && args.files.is_empty() {
        Args::command().print_long_help()?;
        return Ok(());
    }

    // Validate files exist
    for file in &args.files {
        if !file.exists() {
            anyhow::bail!("File not found: {}", file.display());
        }
    }

    // Initialize LLM client with selected preset
    let llm = LlmClient::new(args.model.as_deref(), args.debug)?;

    // Parse JSON schema if provided
    let json_schema = match &args.json {
        Some(input) => Some(load_json_schema(input)?),
        None => None,
    };

    // Build the prompt and optional system prompt
    let (prompt, system_prompt) = build_prompt(&question, piped_input.as_deref(), args.general);

    // Call LLM
    let response = llm
        .complete(&prompt, system_prompt, &args.files, json_schema)
        .await?;

    if response.is_empty() {
        anyhow::bail!("Empty response from LLM");
    }

    // Display the response
    println!("{}", response.trim());

    // Copy to clipboard if not general mode (macOS only)
    if !args.general {
        copy_to_clipboard(&response)?;
    }

    Ok(())
}

/// Build prompt and optional system prompt based on mode
/// Returns (user_prompt, Option<system_prompt>)
fn build_prompt<'a>(
    question: &str,
    piped_input: Option<&str>,
    is_general: bool,
) -> (String, Option<&'a str>) {
    let user_content = match piped_input {
        Some(piped_data) if !question.is_empty() => {
            format!("{}\n\n{}", question, piped_data)
        }
        Some(piped_data) => piped_data.to_string(),
        None => question.to_string(),
    };

    if is_general {
        // General mode: no system prompt
        (user_content, None)
    } else {
        // Shell mode: use shell system prompt
        (user_content, Some(SHELL_SYSTEM_PROMPT))
    }
}

fn copy_to_clipboard(text: &str) -> Result<()> {
    // Use pbcopy on macOS
    let mut cmd = Command::new("pbcopy");
    cmd.stdin(Stdio::piped());

    let mut child = cmd.spawn().context("Failed to spawn pbcopy")?;

    if let Some(mut stdin) = child.stdin.take() {
        stdin
            .write_all(text.as_bytes())
            .context("Failed to write to pbcopy")?;
    }

    let status = child.wait().context("Failed to wait for pbcopy")?;

    if !status.success() {
        anyhow::bail!("pbcopy failed");
    }

    Ok(())
}

/// Load JSON schema from a file path or parse inline JSON string.
/// Auto-detects: if the string is a valid file path that exists, reads from file.
/// Otherwise, attempts to parse as inline JSON.
fn load_json_schema(input: &str) -> Result<serde_json::Value> {
    let path = PathBuf::from(input);

    // Check if it's a file path that exists
    if path.exists() && path.is_file() {
        let content = std::fs::read_to_string(&path)
            .with_context(|| format!("Failed to read JSON schema file: {}", path.display()))?;
        serde_json::from_str(&content)
            .with_context(|| format!("Invalid JSON in schema file: {}", path.display()))
    } else {
        // Try to parse as inline JSON
        serde_json::from_str(input)
            .context("Invalid JSON schema: not a valid file path and failed to parse as JSON")
    }
}
