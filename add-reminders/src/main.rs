use anyhow::{Context, Result};
use chrono::Local;
use clap::Parser;
use log::{debug, info, warn};
use regex::Regex;
use std::env;
use std::fs::OpenOptions;
use std::io::{self, Read, Write};
use std::process::Command;
use std::sync::OnceLock;

#[derive(Parser, Debug)]
#[command(name = "add-reminders")]
#[command(about = "Process text and add reminders to macOS Reminders app", long_about = None)]
struct Cli {
    /// The text containing todos to add (one per line). If not provided, reads from stdin.
    #[arg(short = 't', long = "todos")]
    todos: Option<String>,

    /// The Reminders list to add to
    #[arg(short = 'l', long = "list", default_value = "inbox")]
    list: String,

    /// Show detailed processing information
    #[arg(short = 'v', long = "verbose")]
    verbose: bool,
}

/// Get the compiled regex pattern for extracting todo text
fn todo_pattern() -> &'static Regex {
    static PATTERN: OnceLock<Regex> = OnceLock::new();
    PATTERN.get_or_init(|| {
        // Match optional whitespace, followed by optional markdown prefixes (-, *, +, 1., etc.),
        // followed by optional checkbox syntax ([ ], [x], or partial like "x]" after Unicode stripping),
        // followed by whitespace, then capture everything from the first alphanumeric character to the end
        Regex::new(r"^\s*(?:[-*+]|\d+\.)?(?:\s*\[[x ]\]|\s*x\])?\s*([a-zA-Z0-9].*)$").unwrap()
    })
}

/// Strip leading invisible/problematic characters until we reach meaningful content.
/// This handles Unicode format characters, zero-width spaces, object replacement chars, etc.
/// that can appear at the start of lines when copying from certain applications.
fn strip_leading_junk(text: &str) -> &str {
    // Find the first alphanumeric character - everything else (whitespace, markdown markers)
    // will be handled by the regex pattern and trimming
    let start_pos = text.find(|c: char| c.is_alphanumeric());

    match start_pos {
        Some(pos) => &text[pos..],
        None => text, // No meaningful content found, return as-is (will be filtered later)
    }
}

/// Process a single line of text to extract a clean todo item
fn process_line(line: &str) -> Option<String> {
    debug!("Processing line: {:?}", line);

    // Strip any leading invisible Unicode characters (zero-width spaces, object replacement chars, etc.)
    let cleaned = strip_leading_junk(line);

    // Trim standard whitespace
    let trimmed = cleaned.trim();

    // Skip empty lines
    if trimmed.is_empty() {
        debug!("  → Skipped (empty line)");
        return None;
    }

    // Use regex to extract the todo text
    // The pattern matches common markdown prefixes and checkbox syntax,
    // capturing everything from the first alphanumeric character onward
    let result = todo_pattern()
        .captures(trimmed)
        .and_then(|caps| caps.get(1))
        .map(|m| m.as_str().trim_end().to_string());

    match &result {
        Some(text) => debug!("  → Extracted: {:?}", text),
        None => debug!("  → Skipped (no match)"),
    }

    result
}

/// Process the input text and extract all todo items
fn process_todos(text: &str) -> Vec<String> {
    info!("Processing input text ({} bytes, {} lines)", text.len(), text.lines().count());
    debug!("Input text: {:?}", text);

    let todos: Vec<String> = text.lines()
        .filter_map(process_line)
        .collect();

    info!("Extracted {} todos", todos.len());
    todos
}

/// Add a single reminder to macOS Reminders using AppleScript
fn add_reminder(list_name: &str, reminder_text: &str) -> Result<()> {
    debug!("Adding reminder to list '{}': {:?}", list_name, reminder_text);

    // Escape double quotes in the reminder text for AppleScript
    let escaped_text = reminder_text.replace('"', "\\\"");
    let escaped_list = list_name.replace('"', "\\\"");

    let applescript = format!(
        r#"tell application "Reminders"
    set theList to first list whose name is "{}"
    make new reminder at theList with properties {{name:"{}"}}
end tell"#,
        escaped_list, escaped_text
    );

    debug!("AppleScript: {}", applescript);

    let output = Command::new("osascript")
        .arg("-e")
        .arg(&applescript)
        .output()
        .context("Failed to execute osascript command")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        warn!("Failed to add reminder: {}", stderr);
        anyhow::bail!(
            "Failed to add reminder '{}' to list '{}': {}",
            reminder_text,
            list_name,
            stderr
        );
    }

    debug!("Successfully added reminder");
    Ok(())
}

/// Initialize logging to a file in the project's logs directory
fn init_logging() -> Result<String> {
    // Use compile-time path to workspace root (parent of add-reminders)
    // CARGO_MANIFEST_DIR points to add-reminders/, so go up one level to workspace root
    let workspace_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .context("Failed to get workspace root from CARGO_MANIFEST_DIR")?;

    // Create logs directory
    let logs_dir = workspace_root.join("logs");
    std::fs::create_dir_all(&logs_dir)
        .context("Failed to create logs directory")?;

    let log_path = logs_dir.join("add-reminders.log");

    // Check if log file exists and is over 1MB
    if let Ok(metadata) = std::fs::metadata(&log_path) {
        if metadata.len() > 1_048_576 {  // 1MB in bytes
            // Truncate the log file
            std::fs::remove_file(&log_path)
                .context("Failed to truncate log file")?;
        }
    }

    let log_file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&log_path)
        .context("Failed to open log file")?;

    // Write a separator for this run
    let mut file = log_file;
    writeln!(file, "\n========== New Run: {} ==========",
        Local::now().format("%Y-%m-%d %H:%M:%S"))?;

    env_logger::Builder::from_default_env()
        .filter_level(log::LevelFilter::Debug)
        .target(env_logger::Target::Pipe(Box::new(file)))
        .format_timestamp_millis()
        .init();

    Ok(log_path.to_string_lossy().to_string())
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    // Initialize logging - errors here are non-fatal
    let log_path = match init_logging() {
        Ok(path) => {
            if cli.verbose {
                println!("Logging to: {}", path);
            }
            Some(path)
        }
        Err(e) => {
            if cli.verbose {
                eprintln!("Warning: Failed to initialize logging: {}", e);
            }
            None
        }
    };

    info!("add-reminders started");
    info!("Arguments: list={}, verbose={}", cli.list, cli.verbose);

    // Get input text from either --todos flag or stdin
    let input_text = if let Some(todos) = cli.todos {
        info!("Reading todos from command-line argument");
        todos
    } else {
        info!("Reading todos from stdin");
        let mut buffer = String::new();
        io::stdin()
            .read_to_string(&mut buffer)
            .context("Failed to read from stdin")?;

        if buffer.trim().is_empty() {
            anyhow::bail!("No input provided. Either use --todos flag or pipe text to stdin.");
        }

        buffer
    };

    if cli.verbose {
        println!("=== Input Text ===");
        println!("{:?}", input_text);
        println!("\n=== Raw Lines ===");
        for (i, line) in input_text.lines().enumerate() {
            println!("Line {}: {:?}", i + 1, line);
        }
        println!();
    }

    // Process the input text to extract todos
    let todos = process_todos(&input_text);

    if cli.verbose {
        println!("=== Processed Todos ===");
        for (i, todo) in todos.iter().enumerate() {
            println!("{}: {:?}", i + 1, todo);
        }
        println!();
    }

    if todos.is_empty() {
        warn!("No todos found in input text");
        println!("No todos found in input text.");
        if let Some(path) = log_path {
            println!("Check the log for details: {}", path);
        }
        return Ok(());
    }

    // Add each todo as a reminder
    info!("Adding {} reminders to list '{}'", todos.len(), cli.list);
    for (index, todo) in todos.iter().enumerate() {
        if cli.verbose {
            println!("Adding reminder #{}: {:?}", index + 1, todo);
        }
        add_reminder(&cli.list, todo)
            .with_context(|| format!("Failed to add todo #{}: {}", index + 1, todo))?;
        println!("✓ Added: {}", todo);
    }

    info!("Successfully added {} reminders", todos.len());
    println!("\nSuccessfully added {} reminder(s) to '{}'", todos.len(), cli.list);

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_process_line_basic() {
        assert_eq!(
            process_line("simple todo"),
            Some("simple todo".to_string())
        );
    }

    #[test]
    fn test_process_line_with_leading_spaces() {
        assert_eq!(
            process_line("    indented todo"),
            Some("indented todo".to_string())
        );
    }

    #[test]
    fn test_process_line_markdown_unchecked() {
        assert_eq!(
            process_line("- [ ] practice stepping back"),
            Some("practice stepping back".to_string())
        );
    }

    #[test]
    fn test_process_line_markdown_checked() {
        assert_eq!(
            process_line("- [x] completed task"),
            Some("completed task".to_string())
        );
    }

    #[test]
    fn test_process_line_markdown_with_indentation() {
        assert_eq!(
            process_line("\t- [ ] stand up and stretch when needed"),
            Some("stand up and stretch when needed".to_string())
        );
    }

    #[test]
    fn test_process_line_empty() {
        assert_eq!(process_line(""), None);
        assert_eq!(process_line("   "), None);
    }

    #[test]
    fn test_process_todos_full_example() {
        let input = r#"- [ ] practice stepping back to problem solve when overwhelmed
	- [ ] stand up and stretch when needed
	- [ ] lean into using llms for support
do another load of laundry
change the sheets"#;

        let expected = vec![
            "practice stepping back to problem solve when overwhelmed",
            "stand up and stretch when needed",
            "lean into using llms for support",
            "do another load of laundry",
            "change the sheets",
        ];

        assert_eq!(process_todos(input), expected);
    }

    #[test]
    fn test_process_todos_with_empty_lines() {
        let input = "todo 1\n\ntodo 2\n   \ntodo 3";
        let expected = vec!["todo 1", "todo 2", "todo 3"];
        assert_eq!(process_todos(input), expected);
    }

    #[test]
    fn test_process_line_various_prefixes() {
        // Test different markdown list prefixes
        assert_eq!(
            process_line("* a todo"),
            Some("a todo".to_string())
        );
        assert_eq!(
            process_line("+ another todo"),
            Some("another todo".to_string())
        );
        assert_eq!(
            process_line("1. numbered todo"),
            Some("numbered todo".to_string())
        );
        assert_eq!(
            process_line("42. another numbered"),
            Some("another numbered".to_string())
        );
    }

    #[test]
    fn test_process_line_whitespace_variations() {
        // All these should extract "foobar"
        assert_eq!(
            process_line(" \n - [ ] foobar"),
            Some("foobar".to_string())
        );
        assert_eq!(
            process_line("     \n\n  foobar"),
            Some("foobar".to_string())
        );
        assert_eq!(
            process_line("- [x] foobar"),
            Some("foobar".to_string())
        );
        assert_eq!(
            process_line("- foobar"),
            Some("foobar".to_string())
        );
        assert_eq!(
            process_line("foobar"),
            Some("foobar".to_string())
        );
    }

    #[test]
    fn test_process_line_preserves_content_after_first_word() {
        // Ensure we preserve punctuation, spaces, special chars in the content
        assert_eq!(
            process_line("- [ ] call mom @ 3pm!"),
            Some("call mom @ 3pm!".to_string())
        );
        assert_eq!(
            process_line("- review PR #123 (high priority)"),
            Some("review PR #123 (high priority)".to_string())
        );
    }

    #[test]
    fn test_strip_leading_junk_with_unicode() {
        // Test with zero-width space (U+200B) - strips to first alphanumeric
        assert_eq!(
            strip_leading_junk("\u{200B}- [ ] test todo"),
            "test todo"
        );

        // Test with object replacement character (U+FFFC)
        assert_eq!(
            strip_leading_junk("\u{FFFC}- [ ] test todo"),
            "test todo"
        );

        // Test with both zero-width space and object replacement character
        assert_eq!(
            strip_leading_junk("\u{200B}\u{FFFC}- [ ] test todo"),
            "test todo"
        );

        // Test with multiple invisible characters
        assert_eq!(
            strip_leading_junk("\u{FFFC}\u{200B}\u{FFFC}acknowledge anxiety"),
            "acknowledge anxiety"
        );
    }

    #[test]
    fn test_process_line_with_leading_unicode_junk() {
        // Test with zero-width space before markdown checkbox
        assert_eq!(
            process_line("\u{200B}- [ ] practice stepping back"),
            Some("practice stepping back".to_string())
        );

        // Test with object replacement character before markdown checkbox
        assert_eq!(
            process_line("\u{FFFC}- [ ] acknowledge anxiety when it arises"),
            Some("acknowledge anxiety when it arises".to_string())
        );

        // Test with multiple invisible Unicode characters
        assert_eq!(
            process_line("\u{200B}\u{FFFC}- [ ] remind myself the job interview process is a journey"),
            Some("remind myself the job interview process is a journey".to_string())
        );

        // Test with invisible characters and tab
        assert_eq!(
            process_line("\u{FFFC}\t- [ ] stand up and stretch when needed"),
            Some("stand up and stretch when needed".to_string())
        );
    }

    #[test]
    fn test_process_todos_with_unicode_junk() {
        // Real-world example from logs with invisible Unicode characters
        let input = "\u{FFFC}- [ ] acknowledge anxiety when it arises
\u{FFFC}- [ ] remind myself the job interview process is a journey (can take a year or more)
\u{200B}\u{FFFC}- [ ] practice stepping back to problem solve when overwhelmed";

        let expected = vec![
            "acknowledge anxiety when it arises",
            "remind myself the job interview process is a journey (can take a year or more)",
            "practice stepping back to problem solve when overwhelmed",
        ];

        assert_eq!(process_todos(input), expected);
    }
}
