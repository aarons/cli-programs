use anyhow::{Context, Result};
use chrono::Local;
use clap::Parser;
use std::path::PathBuf;
use std::process::Command;

const EXAMPLES: &str = r#"
BEHAVIOR:
    Auto-detects the appropriate review mode based on git state:

    - Has uncommitted changes -> Reviews uncommitted changes (codex review --uncommitted)
    - No uncommitted changes  -> Reviews commits against main (codex review --base main)

EXAMPLES:
    # Auto-detect mode based on uncommitted changes
    code-review

    # Provide custom review instructions
    code-review "Focus on security vulnerabilities"

    # Force review of uncommitted changes only
    code-review --uncommitted

    # Review a specific commit with custom prompt
    code-review --commit abc123 "Check for breaking changes"
"#;

#[derive(Parser, Debug)]
#[command(name = "code-review")]
#[command(about = "Get LLM code reviews using codex")]
#[command(version)]
#[command(after_help = EXAMPLES)]
struct Args {
    /// Custom review instructions
    #[arg(value_name = "PROMPT")]
    prompt: Option<String>,

    /// Review only uncommitted changes (staged, unstaged, untracked)
    #[arg(long)]
    uncommitted: bool,

    /// Review a specific commit
    #[arg(long, value_name = "SHA")]
    commit: Option<String>,
}

#[derive(Debug)]
enum ReviewMode {
    Uncommitted,
    Committed,
    SpecificCommit(String),
}

fn git(args: &[&str]) -> Result<String> {
    let output = Command::new("git")
        .args(args)
        .output()
        .context("Failed to execute git command")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("git command failed: {}", stderr);
    }

    String::from_utf8(output.stdout).context("Git output was not valid UTF-8")
}

fn is_git_repo() -> bool {
    Command::new("git")
        .args(["rev-parse", "--git-dir"])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

fn has_uncommitted_changes() -> Result<bool> {
    let status = git(&["status", "--porcelain"])?;
    Ok(!status.trim().is_empty())
}

fn get_main_branch() -> Result<String> {
    if git(&["show-ref", "--verify", "--quiet", "refs/heads/main"]).is_ok() {
        return Ok("main".to_string());
    }
    if git(&["show-ref", "--verify", "--quiet", "refs/heads/master"]).is_ok() {
        return Ok("master".to_string());
    }
    anyhow::bail!("Could not find 'main' or 'master' branch. Use --uncommitted or --commit instead.")
}

fn determine_mode(args: &Args) -> Result<ReviewMode> {
    // Priority:
    // 1. If --commit specified -> SpecificCommit
    // 2. If --uncommitted specified OR has uncommitted changes -> Uncommitted
    // 3. Otherwise -> Committed (feature branch with commits against main)

    if let Some(sha) = &args.commit {
        return Ok(ReviewMode::SpecificCommit(sha.clone()));
    }

    if args.uncommitted || has_uncommitted_changes()? {
        return Ok(ReviewMode::Uncommitted);
    }

    Ok(ReviewMode::Committed)
}

fn run_codex(mode: &ReviewMode, main_branch: &str, prompt: Option<&str>) -> Result<String> {
    let mut args: Vec<&str> = match mode {
        ReviewMode::Uncommitted => {
            vec!["review", "--uncommitted"]
        }
        ReviewMode::Committed => {
            vec!["review", "--base", main_branch]
        }
        ReviewMode::SpecificCommit(sha) => {
            vec!["review", "--commit", sha]
        }
    };

    if let Some(p) = prompt {
        args.push(p);
    }

    eprintln!("Running: codex {}", args.join(" "));

    let output = Command::new("codex")
        .args(&args)
        .output()
        .context("Failed to execute codex command")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("codex command failed: {}", stderr);
    }

    // codex writes to stderr when not running in a TTY, so prefer stderr over stdout
    let content = if output.stdout.is_empty() {
        String::from_utf8(output.stderr).context("Codex stderr was not valid UTF-8")?
    } else {
        String::from_utf8(output.stdout).context("Codex stdout was not valid UTF-8")?
    };

    if content.trim().is_empty() {
        anyhow::bail!("codex produced no output");
    }

    Ok(content)
}

fn parse_codex_output(output: &str) -> Result<String> {
    let lines: Vec<&str> = output.lines().collect();

    let codex_start = lines
        .iter()
        .position(|line| line.trim() == "codex")
        .ok_or_else(|| anyhow::anyhow!("Could not find 'codex' section in output"))?;

    let tokens_end = lines[codex_start..]
        .iter()
        .position(|line| line.trim() == "tokens used")
        .map(|pos| codex_start + pos)
        .unwrap_or(lines.len());

    // Extract content between "codex" line and "tokens used" line
    let review_content = lines[codex_start + 1..tokens_end].join("\n");
    let review_content = review_content.trim().to_string();

    if review_content.is_empty() {
        anyhow::bail!("Codex review section was empty");
    }

    Ok(review_content)
}

fn log_codex_output(output: &str) -> Result<PathBuf> {
    let logs_dir = PathBuf::from("logs");
    std::fs::create_dir_all(&logs_dir).context("Failed to create logs directory")?;

    let timestamp = Local::now().format("%Y%m%d_%H%M%S");
    let log_file = logs_dir.join(format!("codex_output_{}.log", timestamp));

    std::fs::write(&log_file, output).context("Failed to write log file")?;
    Ok(log_file)
}

fn main() -> Result<()> {
    let args = Args::parse();

    // Validate we're in a git repo
    if !is_git_repo() {
        anyhow::bail!("Not in a git repository");
    }

    // Determine review mode
    let mode = determine_mode(&args)?;
    let main_branch = get_main_branch()?;

    // Run codex review
    let output = run_codex(&mode, &main_branch, args.prompt.as_deref())?;

    // Parse output
    match parse_codex_output(&output) {
        Ok(review) => {
            println!("{}", review);
            Ok(())
        }
        Err(e) => {
            let log_path = log_codex_output(&output)?;
            eprintln!("Failed to parse codex output: {}", e);
            eprintln!("Full output logged to: {}", log_path.display());
            Err(e)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_codex_output_valid() {
        let input = r#"OpenAI Codex v0.77.0 (research preview)
--------
workdir: /Users/aaron/code/gen-audio-py
model: gpt-5.2-codex
--------
user
changes against 'main'

thinking
<thinking steps>
exec
<bash executions>
thinking
<more thinking>
codex
Queue daemon writes outputs based solely on the EPUB basename.

Review comment:

- [P1] Daemon overwrites outputs — /path/to/file.py:145-154
  Description of the issue here.
tokens used
27,427
"#;

        let result = parse_codex_output(input).unwrap();
        assert!(result.contains("Queue daemon writes outputs"));
        assert!(result.contains("[P1] Daemon overwrites outputs"));
        assert!(!result.contains("tokens used"));
        assert!(!result.contains("27,427"));
    }

    #[test]
    fn test_parse_codex_output_no_tokens_line() {
        let input = r#"thinking
some thinking
codex
This is the review content.
Some more review content.
"#;

        let result = parse_codex_output(input).unwrap();
        assert!(result.contains("This is the review content."));
        assert!(result.contains("Some more review content."));
    }

    #[test]
    fn test_parse_codex_output_missing_codex_section() {
        let input = r#"thinking
some thinking
exec
some exec
tokens used
100
"#;

        let result = parse_codex_output(input);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Could not find 'codex' section"));
    }

    #[test]
    fn test_parse_codex_output_empty_review() {
        let input = r#"codex
tokens used
100
"#;

        let result = parse_codex_output(input);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Codex review section was empty"));
    }
}
