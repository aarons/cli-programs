// gc - Git commit with AI-generated conventional commit messages

mod prompts;

use addr::parse_domain_name;
use anyhow::{Context, Result};
use clap::Parser;
use email_address::EmailAddress;
use git_conventional::Commit;
use std::collections::HashSet;
use std::path::Path;
use std::process::Command;
use unicode_segmentation::UnicodeSegmentation;
use git2::Repository;
use url::Url;

#[derive(Parser, Debug)]
#[command(name = "gc")]
#[command(about = "Generate conventional commit messages using AI", long_about = None)]
#[command(version)]
struct Args {
    /// Enable debug mode for verbose output
    #[arg(short, long, default_value_t = false)]
    debug: bool,

    /// Only commit staged changes (don't auto-stage)
    #[arg(long, default_value_t = false)]
    staged: bool,

    /// Skip pushing to remote after commit
    #[arg(long, default_value_t = false)]
    nopush: bool,

    /// Additional context to include in the prompt
    #[arg(short, long)]
    context: Option<String>,

    /// High-level description of changes to guide the commit message
    #[arg(trailing_var_arg = true)]
    message: Vec<String>,
}

#[derive(Debug, Clone)]
struct LlmResponse {
    message: String,
    raw_response: String,
}

#[derive(Debug, Clone)]
enum ValidationResult {
    Valid,
    Invalid(Vec<String>),
}

impl ValidationResult {
    fn is_valid(&self) -> bool {
        matches!(self, ValidationResult::Valid)
    }

    fn errors(&self) -> Vec<String> {
        match self {
            ValidationResult::Valid => vec![],
            ValidationResult::Invalid(errors) => errors.clone(),
        }
    }
}

// Git helper function - wraps git commands with error handling
fn git(args: &[&str]) -> Result<String> {
    let output = Command::new("git")
        .args(args)
        .output()
        .context("Failed to execute git command")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("git command failed: {}", stderr);
    }

    String::from_utf8(output.stdout)
        .context("Git output was not valid UTF-8")
}

fn is_git_repo() -> bool {
    Repository::open(".").is_ok()
}

/// Check if Claude CLI is available and executable
fn check_claude_cli() -> Result<()> {
    let output = Command::new("which")
        .arg("claude")
        .output()
        .context("Failed to check for claude CLI")?;

    if !output.status.success() {
        anyhow::bail!(
            "Claude CLI not found.\n\
             Please ensure Claude Code is installed.\n\
             You can install it by following: https://docs.anthropic.com/en/docs/claude-code"
        );
    }

    Ok(())
}

/// Get staged diff with specific formatting flags (matches gc.sh behavior)
fn get_staged_diff() -> Result<String> {
    // Matches: git diff -U1 --staged --no-color --no-prefix --minimal --ignore-all-space --ignore-blank-lines
    // Then filters out: lines starting with "index", "---", "+++"
    let diff = git(&[
        "diff",
        "-U1",
        "--staged",
        "--no-color",
        "--no-prefix",
        "--minimal",
        "--ignore-all-space",
        "--ignore-blank-lines",
    ])?;

    // Filter out metadata lines
    let filtered: Vec<&str> = diff
        .lines()
        .filter(|line| {
            !line.starts_with("index ") &&
            !line.starts_with("--- ") &&
            !line.starts_with("+++ ")
        })
        .collect();

    Ok(filtered.join("\n"))
}

/// Get file status for staged changes
fn get_name_status() -> Result<String> {
    git(&["diff", "--staged", "--name-status", "--no-color"])
}

fn get_status() -> Result<String> {
    git(&["status", "--porcelain"])
}

fn get_current_branch() -> Result<String> {
    // Try symbolic-ref first (handles unborn branches like in initial commit)
    match git(&["symbolic-ref", "--short", "HEAD"]) {
        Ok(branch) => Ok(branch.trim().to_string()),
        Err(_) => {
            // Fallback for detached HEAD state or other issues
            git(&["rev-parse", "--abbrev-ref", "HEAD"])
                .map(|s| s.trim().to_string())
        }
    }
}

/// Detect main branch (main or master)
fn get_main_branch() -> Result<String> {
    if git(&["show-ref", "--verify", "--quiet", "refs/heads/main"]).is_ok() {
        return Ok("main".to_string());
    }
    if git(&["show-ref", "--verify", "--quiet", "refs/heads/master"]).is_ok() {
        return Ok("master".to_string());
    }
    Ok("main".to_string())
}

/// Get commits in current branch since branching from main
fn get_branch_commits(current_branch: &str, main_branch: &str) -> Result<String> {
    let merge_base = git(&["merge-base", main_branch, current_branch]);

    match merge_base {
        Ok(branch_point) => {
            let branch_point = branch_point.trim();
            let commits = git(&[
                "log",
                "--pretty=format:%ad - %s",
                "--date=short",
                &format!("{}..{}", branch_point, current_branch),
            ])?;

            if commits.trim().is_empty() {
                Ok(format!("No commits since branching from {}", main_branch))
            } else {
                Ok(commits)
            }
        }
        Err(_) => {
            // Fallback to simple commit history
            // Use match to handle case where git log fails (e.g. empty repo with no commits yet)
            match git(&["log", "--pretty=format:%ad - %s", "--date=short", "-n", "5"]) {
                Ok(logs) => Ok(logs),
                Err(_) => Ok("Initial commit".to_string()),
            }
        }
    }
}

fn stage_all_changes() -> Result<()> {
    git(&["add", "-A"])?;
    Ok(())
}

fn commit(message: &str) -> Result<()> {
    git(&["commit", "-m", message])?;
    Ok(())
}

fn push() -> Result<()> {
    git(&["push"])?;
    Ok(())
}

fn get_repo_filenames() -> Result<HashSet<String>> {
    let repo_root = git(&["rev-parse", "--show-toplevel"])?.trim().to_string();

    let tracked = Command::new("git")
        .current_dir(&repo_root)
        .args(["ls-files", "--cached"])
        .output()?;

    let untracked = Command::new("git")
        .current_dir(&repo_root)
        .args(["ls-files", "--others", "--exclude-standard"])
        .output()?;

    let mut all_files = String::from_utf8_lossy(&tracked.stdout).into_owned();
    all_files.push_str(&String::from_utf8_lossy(&untracked.stdout));

    let filenames = all_files
        .lines()
        .filter_map(|line| {
            Path::new(line.trim())
                .file_name()
                .and_then(|f| f.to_str())
                .map(|s| s.to_string())
        })
        .collect();

    Ok(filenames)
}

// LLM interaction functions
const MAX_RETRIES: usize = 3;

/// Call Claude CLI and get response
fn call_claude(prompt: &str, system_prompt: &str) -> Result<String> {
    let output = Command::new("claude")
        .args([
            "--model", "sonnet",
            "--system-prompt", system_prompt,
            "--print",
            prompt,
        ])
        .output()
        .context("Failed to execute claude command")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("Claude command failed: {}", stderr);
    }

    String::from_utf8(output.stdout)
        .context("Invalid UTF-8 in claude output")
        .map(|s| s.trim().to_string())
}

/// Parse LLM response into structured format
/// Expected format:
/// <observations>
/// [observations]
/// </observations>
/// <commit_message>
/// [commit message]
/// </commit_message>
fn parse_llm_response(response: String) -> Result<LlmResponse> {
    // Validate observations section exists
    extract_xml_tag(&response, "observations")
        .ok_or_else(|| anyhow::anyhow!("Response missing '<observations>' section"))?;

    let message = extract_xml_tag(&response, "commit_message")
        .ok_or_else(|| anyhow::anyhow!("Response missing '<commit_message>' section"))?;

    if message.trim().is_empty() {
        anyhow::bail!("Message section is empty");
    }

    Ok(LlmResponse {
        message,
        raw_response: response,
    })
}

/// Extract content between XML tags
fn extract_xml_tag(text: &str, tag: &str) -> Option<String> {
    let start_tag = format!("<{}>", tag);
    let end_tag = format!("</{}>", tag);

    let start_idx = text.find(&start_tag)?;
    let content_start = start_idx + start_tag.len();
    let end_idx = text[content_start..].find(&end_tag)?;

    Some(text[content_start..content_start + end_idx].trim().to_string())
}

/// Generate commit message with retry logic inline in main flow
fn generate_commit_message(
    prompt: &str,
    system_prompt: &str,
    debug: bool,
) -> Result<LlmResponse> {
    let mut attempts = 0;

    loop {
        attempts += 1;

        if debug {
            eprintln!("Attempt {}/{}", attempts, MAX_RETRIES);
        }

        let response = call_claude(prompt, system_prompt)?;

        if debug {
            eprintln!("Raw response:\n{}", response);
        }

        match parse_llm_response(response) {
            Ok(parsed) => return Ok(parsed),
            Err(e) if attempts >= MAX_RETRIES => {
                anyhow::bail!("Failed to get properly formatted response after {} attempts: {}", MAX_RETRIES, e);
            }
            Err(e) => {
                if debug {
                    eprintln!("Parse error: {}, retrying...", e);
                }
                continue;
            }
        }
    }
}

/// Request LLM to fix commit message issues
fn fix_commit_message(
    original_prompt: &str,
    previous_response: &str,
    system_prompt: &str,
    debug: bool,
) -> Result<LlmResponse> {
    let fix_prompt = prompts::fix_message_format(original_prompt, previous_response);
    generate_commit_message(&fix_prompt, system_prompt, debug)
}

/// Request LLM to clean policy violations from message
fn clean_commit_message(
    message: &str,
    system_prompt: &str,
    debug: bool,
) -> Result<LlmResponse> {
    let clean_prompt = prompts::fix_message_content(message);

    if debug {
        eprintln!("Cleaning prompt:\n{}", clean_prompt);
    }

    let response = call_claude(&clean_prompt, system_prompt)?;

    if debug {
        eprintln!("Clean response:\n{}", response);
    }

    let cleaned_message = response.trim().to_string();

    Ok(LlmResponse {
        message: cleaned_message.clone(),
        raw_response: response,
    })
}

// Validation functions
/// Check for policy violations in commit message
fn check_policy_violations(message: &str) -> Vec<String> {
    let mut violations = Vec::new();

    if message.split_whitespace()
        .any(EmailAddress::is_valid)
    {
        violations.push("Contains email address".to_string());
    }

    let repo_filenames = get_repo_filenames().unwrap_or_default();

    if message.split_whitespace()
        .any(|word| {
            // Strip trailing period (end of sentence punctuation)
            let word = word.strip_suffix('.').unwrap_or(word);

            // Skip if word is an exact match for a filename in the repo
            if repo_filenames.contains(word) {
                return false;
            }

            if let Ok(url) = Url::parse(word) {
                return url.has_host();
            }

            if word.contains('.') {
                if let Ok(domain) = parse_domain_name(word) {
                    return domain.has_known_suffix();
                }
            }

            false
        })
    {
        violations.push("Contains URL".to_string());
    }

    let has_emoji = message.graphemes(true)
        .any(|grapheme| emojis::get(grapheme).is_some());

    if has_emoji {
        violations.push("Contains emoji characters".to_string());
    }

    violations
}

/// Validate conventional commit format using git-conventional crate
fn validate_conventional_commit(message: &str) -> ValidationResult {
    match Commit::parse(message) {
        Ok(_) => ValidationResult::Valid,
        Err(_) => ValidationResult::Invalid(vec![
            "Does not follow Conventional Commits format (type: description or type(scope): description)".to_string()
        ]),
    }
}

fn main() -> Result<()> {
    let args = Args::parse();

    // Prerequisites validation
    check_claude_cli()?;

    if !is_git_repo() {
        anyhow::bail!("Not in a git repository. Please run this command from within a git repository.");
    }

    // Check for changes and stage if needed
    if args.staged {
        // Staged-only mode: check for already staged changes
        let staged_files = get_name_status()
            .context("Failed to check staged changes")?;

        if staged_files.trim().is_empty() {
            println!("No staged changes detected. Use 'git add' to stage files first, or run without --staged to auto-stage all changes.");
            return Ok(());
        }
        println!("Found staged changes, proceeding with commit");
    } else {
        // Normal mode: check for any changes, then stage all
        let status = get_status()
            .context("Failed to check git status")?;

        if status.trim().is_empty() {
            println!("No changes detected.");
            return Ok(());
        }

        stage_all_changes()
            .context("Failed to stage changes")?;

        // Verify we have staged changes after adding
        let staged_files = get_name_status()
            .context("Failed to verify staged changes")?;

        if staged_files.trim().is_empty() {
            println!("No changes staged for commit (perhaps only untracked files were added and git config ignores them?).");
            return Ok(());
        }
    }

    // Determine mode reference for user feedback
    let mode_ref = if args.context.is_some() {
        "squash merge"
    } else if args.staged {
        "staged changes"
    } else {
        "all changes"
    };
    println!("Gathering context for {}", mode_ref);

    let git_diff = get_staged_diff()
        .context("Failed to get git diff")?;
    let git_name_status = get_name_status()
        .context("Failed to get file status")?;
    let current_branch = get_current_branch()
        .context("Failed to get current branch")?;
    let main_branch = get_main_branch()
        .context("Failed to determine main branch")?;
    let branch_commits = get_branch_commits(&current_branch, &main_branch)
        .context("Failed to get branch commits")?;

    // Build context string (matches gc.sh lines 540-551)
    let mut context = String::new();

    if !args.message.is_empty() {
        let user_message = args.message.join(" ");
        context.push_str(&format!(
            "The user has provided the following high-level description of their changes. Use this to guide the commit message:\n{}\n\n---\n\n",
            user_message
        ));
    }

    if let Some(provided_context) = &args.context {
        context.push_str(&format!(
            "We're doing a squash merge, here's the full development log from the feature branch:\n{}\n\n",
            provided_context
        ));
    } else {
        context.push_str(&format!(
            "Current branch: {}\n\nCommits in {} since branching from {}:\n{}\n\n",
            current_branch,
            current_branch,
            main_branch,
            branch_commits
        ));
    }

    context.push_str(&format!(
        "Changed files:\n{}\n\nStaged changes:\n{}",
        git_name_status,
        git_diff
    ));

    println!("Generating commit message with Claude Code");

    // Generate commit message
    let prompt = prompts::generate_commit_prompt(&context);

    let mut llm_response = generate_commit_message(
        &prompt,
        &prompts::SYSTEM_PROMPT,
        args.debug,
    ).context("Failed to generate commit message")?;

    let mut commit_message = llm_response.message.clone();

    let format_validation = validate_conventional_commit(&commit_message);
    if !format_validation.is_valid() {
        if args.debug {
            eprintln!("Warning: Commit message format issues: {:?}", format_validation.errors());
        }
        llm_response = fix_commit_message(
            &prompt,
            &llm_response.raw_response,
            &prompts::SYSTEM_PROMPT,
            args.debug,
        ).context("Failed to fix commit message format")?;

        commit_message = llm_response.message.clone();
    }

    const MAX_CLEAN_ATTEMPTS: usize = 3;
    let mut clean_attempts = 0;

    loop {
        let violations = check_policy_violations(&commit_message);

        if violations.is_empty() {
            break;
        }

        clean_attempts += 1;
        if clean_attempts > MAX_CLEAN_ATTEMPTS {
            eprintln!("Error: Message still contains policy violations after {} attempts. Cannot proceed.", MAX_CLEAN_ATTEMPTS);
            eprintln!("Final message:\n{}", commit_message);
            anyhow::bail!("Message validation failed after {} cleaning attempts", MAX_CLEAN_ATTEMPTS);
        }

        eprintln!("Warning: Commit message contains policy violations: {}", violations.join(", "));
        eprintln!("{}", commit_message);
        eprintln!();
        eprintln!("Cleaning attempt {} of {}...", clean_attempts, MAX_CLEAN_ATTEMPTS);

        llm_response = clean_commit_message(
            &commit_message,
            &prompts::SYSTEM_PROMPT,
            args.debug,
        ).context("Failed to clean commit message")?;

        commit_message = llm_response.message.clone();
    }

    if commit_message.trim().is_empty() {
        anyhow::bail!("Final commit message is empty after validation. Exiting.");
    }

    println!("--- commit ---");
    println!("{}", commit_message);
    println!("--------------");

    commit(&commit_message)
        .context("Failed to commit changes")?;

    if args.nopush {
        println!("Commit successful (skipped push due to --nopush flag)");
        return Ok(());
    }

    // TODO: Capture remote info and provide better feedback
    match push() {
        Ok(_) => {
            // Get remote URL for better feedback
            if let Ok(remote_url) = git(&["remote", "get-url", "origin"]) {
                let cleaned_url = remote_url
                    .trim()
                    .replace("https://", "")
                    .replace("git@", "")
                    .replace(".git", "")
                    .replace(":", "/");
                println!("Pushed to {} {}", cleaned_url, current_branch);
            } else {
                println!("Pushed to remote");
            }
        }
        Err(e) => {
            eprintln!("Warning: git push failed. Commit was successful but not pushed to remote.");
            eprintln!("Error: {}", e);
            eprintln!("You may need to manually push with: git push");
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_url_detection() {
        // Test message with HTTPS URL
        let msg_with_https = "feat: see https://example.com for details";
        let violations = check_policy_violations(msg_with_https);
        assert!(violations.contains(&"Contains URL".to_string()));

        // Test message with git URL
        let msg_with_git = "feat: clone git://github.com/user/repo.git";
        let violations = check_policy_violations(msg_with_git);
        assert!(violations.contains(&"Contains URL".to_string()));

        // Test message without URL
        let msg_without_url = "feat: add new feature";
        let violations = check_policy_violations(msg_without_url);
        assert!(!violations.contains(&"Contains URL".to_string()));

        // Test with domain-like text
        let msg_with_domain = "fix: update example.com config";
        let violations = check_policy_violations(msg_with_domain);
        assert!(violations.contains(&"Contains URL".to_string()));
    }

    #[test]
    fn test_email_detection() {
        // Test message with email
        let msg_with_email = "feat: add user@example.com to contacts";
        let violations = check_policy_violations(msg_with_email);
        assert!(violations.contains(&"Contains email address".to_string()));

        // Test message without email
        let msg_without_email = "feat: add new feature";
        let violations = check_policy_violations(msg_without_email);
        assert!(!violations.contains(&"Contains email address".to_string()));

        // Test with various email formats
        let msg_with_plus = "fix: contact john.doe+test@example.org";
        let violations = check_policy_violations(msg_with_plus);
        assert!(violations.contains(&"Contains email address".to_string()));
    }

    #[test]
    fn test_emoji_detection() {
        // Test messages with emojis
        let msg_with_emoji = "feat: add new feature 🎉";
        let violations = check_policy_violations(msg_with_emoji);
        assert!(violations.contains(&"Contains emoji characters".to_string()));

        // Test message without emojis
        let msg_without_emoji = "feat: add new feature";
        let violations = check_policy_violations(msg_without_emoji);
        assert!(!violations.contains(&"Contains emoji characters".to_string()));

        // Test with various emoji types
        let msg_with_flag = "fix: update code 🇺🇸";
        let violations = check_policy_violations(msg_with_flag);
        assert!(violations.contains(&"Contains emoji characters".to_string()));

        let msg_with_symbol = "chore: cleanup ✅";
        let violations = check_policy_violations(msg_with_symbol);
        assert!(violations.contains(&"Contains emoji characters".to_string()));
    }

    #[test]
    fn test_conventional_commit_validation() {
        // Valid conventional commits
        assert!(validate_conventional_commit("feat: add new feature").is_valid());
        assert!(validate_conventional_commit("fix: resolve bug").is_valid());
        assert!(validate_conventional_commit("feat(parser): Add new parser").is_valid());
        assert!(validate_conventional_commit("refactor(core): improve code").is_valid());

        // Invalid - wrong format
        let result = validate_conventional_commit("Add new feature");
        assert!(!result.is_valid());
        assert!(result.errors().iter().any(|e| e.contains("Conventional Commits format")));

        // Valid - all standard types
        for commit_type in ["feat", "fix", "docs", "style", "refactor", "test", "chore", "perf", "ci", "build", "revert"] {
            let msg = format!("{}: test message", commit_type);
            assert!(validate_conventional_commit(&msg).is_valid());
        }
    }

    #[test]
    fn test_extract_xml_tag_valid() {
        // Test extracting a valid tag
        let text = r#"Some preamble text
<observations>
This is the content inside the tag.
It can span multiple lines.
</observations>
Some trailing text"#;

        let result = extract_xml_tag(text, "observations");
        assert!(result.is_some());
        assert_eq!(
            result.unwrap(),
            "This is the content inside the tag.\nIt can span multiple lines."
        );

        // Test extracting commit_message tag
        let text_with_commit = r#"<commit_message>
feat: add new feature

This is a detailed description.
</commit_message>"#;

        let result = extract_xml_tag(text_with_commit, "commit_message");
        assert!(result.is_some());
        assert_eq!(
            result.unwrap(),
            "feat: add new feature\n\nThis is a detailed description."
        );

        // Test that whitespace is properly trimmed
        let text_with_whitespace = r#"<test>
  Content with leading and trailing whitespace
  </test>"#;

        let result = extract_xml_tag(text_with_whitespace, "test");
        assert!(result.is_some());
        assert_eq!(result.unwrap(), "Content with leading and trailing whitespace");
    }

    #[test]
    fn test_extract_xml_tag_missing() {
        // Test with missing opening tag
        let text_no_open = r#"Some text
</observations>
More text"#;

        let result = extract_xml_tag(text_no_open, "observations");
        assert!(result.is_none());

        // Test with missing closing tag
        let text_no_close = r#"Some text
<observations>
Content without closing tag"#;

        let result = extract_xml_tag(text_no_close, "observations");
        assert!(result.is_none());

        // Test with completely missing tag
        let text_no_tag = r#"This text has no XML tags at all.
Just plain text content."#;

        let result = extract_xml_tag(text_no_tag, "observations");
        assert!(result.is_none());

        // Test with wrong tag name
        let text_wrong_tag = r#"<different_tag>
Some content
</different_tag>"#;

        let result = extract_xml_tag(text_wrong_tag, "observations");
        assert!(result.is_none());
    }

    #[test]
    fn test_parse_llm_response_valid() {
        // Test valid response with both observations and commit_message sections
        let valid_response = r#"<observations>
This change adds a new feature to handle user authentication.
The implementation includes input validation and error handling.
</observations>
<commit_message>
feat: add user authentication with input validation

Implement secure user authentication flow with proper error handling
and input validation to prevent common security issues.
</commit_message>"#;

        let result = parse_llm_response(valid_response.to_string());
        assert!(result.is_ok());

        let llm_response = result.unwrap();
        assert!(llm_response.message.contains("feat: add user authentication"));
        assert!(llm_response.message.contains("input validation"));
        assert_eq!(llm_response.raw_response, valid_response);
    }

    #[test]
    fn test_parse_llm_response_missing_commit_message() {
        // Test invalid response missing commit_message tag
        let invalid_response = r#"<observations>
This change adds a new feature to handle user authentication.
</observations>

Some text without the commit_message tags."#;

        let result = parse_llm_response(invalid_response.to_string());
        assert!(result.is_err());

        let error = result.unwrap_err();
        assert!(error.to_string().contains("missing '<commit_message>'"));
    }

    #[test]
    fn test_parse_llm_response_missing_observations() {
        // Test invalid response missing observations tag
        let invalid_response = r#"Some initial text without observations.

<commit_message>
feat: add user authentication
</commit_message>"#;

        let result = parse_llm_response(invalid_response.to_string());
        assert!(result.is_err());

        let error = result.unwrap_err();
        assert!(error.to_string().contains("missing '<observations>'"));
    }

    #[test]
    fn test_get_repo_filenames() {
        let filenames = get_repo_filenames();
        assert!(filenames.is_ok());

        let filenames = filenames.unwrap();
        eprintln!("Found {} filenames", filenames.len());
        eprintln!("Sample filenames: {:?}", filenames.iter().take(10).collect::<Vec<_>>());
        assert!(!filenames.is_empty(), "Should find filenames in the repo");

        assert!(filenames.contains("main.rs"), "Should contain main.rs. Got: {:?}", filenames.iter().take(20).collect::<Vec<_>>());
        assert!(filenames.contains("Cargo.toml"), "Should contain Cargo.toml");
    }

    #[test]
    fn test_filename_excludes_url_detection() {
        let msg_with_real_filename = "fix: update Cargo.toml dependencies";
        let violations = check_policy_violations(msg_with_real_filename);
        assert!(!violations.contains(&"Contains URL".to_string()),
            "Real filename should not be flagged as URL");

        let msg_with_nonexistent_file = "fix: update example.com config";
        let violations = check_policy_violations(msg_with_nonexistent_file);
        assert!(violations.contains(&"Contains URL".to_string()),
            "Domain-like string that's not a filename should be flagged");
    }

    #[test]
    fn test_filename_and_url_mixed() {
        let msg = "fix: update Cargo.toml to use https://crates.io/new-crate";
        let violations = check_policy_violations(msg);
        assert!(violations.contains(&"Contains URL".to_string()),
            "Should flag actual URL even when message contains valid filenames");
    }

    #[test]
    fn test_dotted_filenames() {
        let msg = "fix: update main.rs formatting";
        let violations = check_policy_violations(msg);
        assert!(!violations.contains(&"Contains URL".to_string()),
            "Filename with dot should not be flagged as domain");
    }

    #[test]
    fn test_trailing_period_handling() {
        // Words ending with period (end of sentence) should not be flagged as URLs
        let msg = "fix: resolve workspace root to enable execution from any directory. Previously relied on relative paths which only worked when executed from workspace root.";
        let violations = check_policy_violations(msg);
        assert!(!violations.contains(&"Contains URL".to_string()),
            "Words with trailing periods at end of sentence should not be flagged as URLs");

        // But actual domains at end of sentence should still be caught
        let msg_with_domain = "fix: see documentation at example.com.";
        let violations = check_policy_violations(msg_with_domain);
        assert!(violations.contains(&"Contains URL".to_string()),
            "Actual domain with trailing period should still be flagged");

        // Multiple sentences with various words ending in periods
        let msg_multiple = "feat: add new feature. Update configuration. Test everything.";
        let violations = check_policy_violations(msg_multiple);
        assert!(!violations.contains(&"Contains URL".to_string()),
            "Regular words ending sentences should not be flagged");

        // Mixed: filename with trailing period should not be flagged
        let msg_filename = "fix: update Cargo.toml.";
        let violations = check_policy_violations(msg_filename);
        assert!(!violations.contains(&"Contains URL".to_string()),
            "Filename with trailing period should not be flagged");
    }
}
