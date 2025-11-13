use anyhow::{Context, Result};
use clap::Parser;
use regex::Regex;
use std::process::Command;

#[derive(Parser, Debug)]
#[command(name = "add-reminders")]
#[command(about = "Process text and add reminders to macOS Reminders app", long_about = None)]
struct Cli {
    /// The text containing todos to add (one per line)
    #[arg(short = 't', long = "todos", required = true)]
    todos: String,

    /// The Reminders list to add to
    #[arg(short = 'l', long = "list", default_value = "inbox")]
    list: String,
}

/// Process a single line of text to extract a clean todo item
fn process_line(line: &str) -> Option<String> {
    // Trim leading/trailing whitespace
    let trimmed = line.trim();

    // Skip empty lines
    if trimmed.is_empty() {
        return None;
    }

    // Regex to match markdown todo markers like "- [ ]" or "- [x]"
    // Pattern: optional whitespace, dash, optional whitespace, [, any char, ], optional whitespace
    let markdown_todo_re = Regex::new(r"^-\s*\[[^\]]*\]\s*").unwrap();

    // Remove markdown todo markers
    let cleaned = markdown_todo_re.replace(trimmed, "").to_string();

    // Trim again after removing markers
    let final_text = cleaned.trim();

    // Only return non-empty results
    if final_text.is_empty() {
        None
    } else {
        Some(final_text.to_string())
    }
}

/// Process the input text and extract all todo items
fn process_todos(text: &str) -> Vec<String> {
    text.lines()
        .filter_map(process_line)
        .collect()
}

/// Add a single reminder to macOS Reminders using AppleScript
fn add_reminder(list_name: &str, reminder_text: &str) -> Result<()> {
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

    let output = Command::new("osascript")
        .arg("-e")
        .arg(&applescript)
        .output()
        .context("Failed to execute osascript command")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!(
            "Failed to add reminder '{}' to list '{}': {}",
            reminder_text,
            list_name,
            stderr
        );
    }

    Ok(())
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    // Process the input text to extract todos
    let todos = process_todos(&cli.todos);

    if todos.is_empty() {
        println!("No todos found in input text.");
        return Ok(());
    }

    // Add each todo as a reminder
    for (index, todo) in todos.iter().enumerate() {
        add_reminder(&cli.list, todo)
            .with_context(|| format!("Failed to add todo #{}: {}", index + 1, todo))?;
        println!("✓ Added: {}", todo);
    }

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
}
