use anyhow::{Context, Result};
use clap::Parser;
use std::io::{self, IsTerminal, Read};
use std::path::PathBuf;
use std::process::{Command, Stdio};

const SHELL_SYSTEM_PROMPT: &str = "This is a user question directly from their MacOS command line. Respond with a single example of a solution to their question. Important: Only provide valid zsh bash commands, do not use markup such as triple backticks.";

#[derive(Parser, Debug)]
#[command(
    name = "ask",
    about = "Standalone Ask Helper using Claude Code CLI",
    long_about = "Provides command line assistance and general AI interaction using Claude Code CLI"
)]
struct Args {
    /// General question mode (doesn't apply shell prompt or copy to clipboard)
    #[arg(short, long)]
    general: bool,

    /// Output format to pass to Claude CLI
    #[arg(long)]
    output_format: Option<String>,

    /// The question to ask (if not provided, will prompt or read from stdin)
    #[arg(trailing_var_arg = true)]
    question: Vec<String>,
}

fn main() -> Result<()> {
    let args = Args::parse();

    // Find the claude CLI
    let claude_cli = find_claude_cli()?;

    // Get the question from args or stdin
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

    // Build the full prompt
    let full_prompt = build_prompt(&question, args.general);

    // Call Claude CLI
    let response = call_claude_cli(&claude_cli, &full_prompt, piped_input.as_deref(), &args)?;

    if response.is_empty() {
        anyhow::bail!("Empty response from Claude CLI");
    }

    // Display the response
    println!("{}", response);

    // Copy to clipboard if not general mode (macOS only)
    if !args.general {
        copy_to_clipboard(&response)?;
        eprintln!("[INFO] Response copied to clipboard");
    }

    Ok(())
}

fn find_claude_cli() -> Result<PathBuf> {
    // Try the specific path first
    let specific_path = PathBuf::from("/Users/aaron/.local/bin/claude");
    if specific_path.exists() && specific_path.is_file() {
        return Ok(specific_path);
    }

    // Try to find in PATH
    if let Ok(output) = Command::new("which").arg("claude").output() {
        if output.status.success() {
            let path = String::from_utf8(output.stdout)
                .context("Invalid UTF-8 in which output")?
                .trim()
                .to_string();
            if !path.is_empty() {
                return Ok(PathBuf::from(path));
            }
        }
    }

    anyhow::bail!(
        "Claude CLI not found. Please ensure Claude Code CLI is installed and available in PATH."
    )
}

fn prompt_for_question() -> Result<String> {
    eprint!("Please enter your question: ");
    let mut question = String::new();
    io::stdin()
        .read_line(&mut question)
        .context("Failed to read question")?;
    Ok(question.trim().to_string())
}

fn build_prompt(question: &str, is_general: bool) -> String {
    if is_general {
        // General mode: no system prompt prefix
        question.to_string()
    } else {
        // Shell mode: add system prompt
        if question.is_empty() {
            SHELL_SYSTEM_PROMPT.to_string()
        } else {
            format!("{}\n\n{}", SHELL_SYSTEM_PROMPT, question)
        }
    }
}

fn call_claude_cli(
    claude_cli: &PathBuf,
    prompt: &str,
    piped_input: Option<&str>,
    args: &Args,
) -> Result<String> {
    let mut cmd = Command::new(claude_cli);

    // Add output format if specified
    if let Some(ref format) = args.output_format {
        cmd.arg("--output-format").arg(format);
    }

    // Build the input based on whether we have piped data
    let input = if let Some(piped_data) = piped_input {
        // If we have piped input, combine it with the prompt
        format!("{}\n\n{}", prompt, piped_data)
    } else {
        // Just the prompt
        prompt.to_string()
    };

    // Set up stdin
    cmd.stdin(Stdio::piped());
    cmd.stdout(Stdio::piped());
    cmd.stderr(Stdio::inherit());

    let mut child = cmd.spawn().context("Failed to spawn Claude CLI")?;

    // Write input to stdin
    if let Some(mut stdin) = child.stdin.take() {
        use std::io::Write;
        stdin
            .write_all(input.as_bytes())
            .context("Failed to write to Claude CLI stdin")?;
    }

    // Wait for the command to complete
    let output = child
        .wait_with_output()
        .context("Failed to wait for Claude CLI")?;

    if !output.status.success() {
        anyhow::bail!(
            "Claude CLI failed with exit code: {}",
            output.status.code().unwrap_or(-1)
        );
    }

    let response = String::from_utf8(output.stdout).context("Invalid UTF-8 in Claude CLI output")?;

    Ok(response.trim_end().to_string())
}

fn copy_to_clipboard(text: &str) -> Result<()> {
    // Use pbcopy on macOS
    let mut cmd = Command::new("pbcopy");
    cmd.stdin(Stdio::piped());

    let mut child = cmd.spawn().context("Failed to spawn pbcopy")?;

    if let Some(mut stdin) = child.stdin.take() {
        use std::io::Write;
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
