mod llm;

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use llm::LlmClient;
use llm_client::{Config, ModelPreset};
use std::io::{self, IsTerminal, Read, Write};
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

    /// Configuration subcommand
    #[command(subcommand)]
    command: Option<Commands>,

    /// The question to ask (if not provided, will prompt or read from stdin)
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
            config.default_preset = preset.clone();
            config.save()?;
            println!("Default preset set to: {}", preset);
        }
        ConfigAction::List => {
            let config = Config::load()?;
            println!("Available presets:");
            for (name, preset) in &config.presets {
                let default_marker = if name == &config.default_preset {
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
                },
            );
            config.save()?;
            println!("Added preset: {}", name);
        }
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

    // Get the question from args
    let question = args.question.join(" ");

    // Check for piped input
    let piped_input = if !io::stdin().is_terminal() {
        let mut buffer = String::new();
        io::stdin()
            .read_to_string(&mut buffer)
            .context("Failed to read piped input")?;
        Some(buffer)
    } else {
        None
    };

    // If no question and no piped input, prompt for question
    let question = if question.is_empty() && piped_input.is_none() {
        prompt_for_question()?
    } else {
        question
    };

    // Validate we have something to ask
    if question.is_empty() && piped_input.is_none() {
        anyhow::bail!("No question provided");
    }

    // Initialize LLM client with selected preset
    let llm = LlmClient::new(args.model.as_deref(), args.debug)?;

    // Build the prompt and optional system prompt
    let (prompt, system_prompt) = build_prompt(&question, piped_input.as_deref(), args.general);

    // Call LLM
    let response = llm.complete(&prompt, system_prompt).await?;

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

fn prompt_for_question() -> Result<String> {
    eprint!("Please enter your question: ");
    io::stderr().flush().ok();
    let mut question = String::new();
    io::stdin()
        .read_line(&mut question)
        .context("Failed to read question")?;
    Ok(question.trim().to_string())
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
