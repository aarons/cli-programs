use anyhow::Result;
use std::io::{self, Write};

use crate::docker::{sandbox_status, SandboxStatus};
use crate::state::{State, WorktreeInfo};

/// Display entry for interactive selection
pub struct SelectionEntry {
    pub name: String,
    pub info: WorktreeInfo,
    pub status: SandboxStatus,
}

/// Get all sandbox entries with their status
pub fn get_sandbox_entries(state: &State) -> Result<Vec<SelectionEntry>> {
    let mut entries = Vec::new();

    for (name, info) in &state.worktrees {
        let status = sandbox_status(&info.path).unwrap_or(SandboxStatus::NotFound);
        entries.push(SelectionEntry {
            name: name.clone(),
            info: info.clone(),
            status,
        });
    }

    // Sort by creation time (most recent first)
    entries.sort_by(|a, b| b.info.created_at.cmp(&a.info.created_at));

    Ok(entries)
}

/// Format a status for display
fn format_status(status: &SandboxStatus) -> &'static str {
    match status {
        SandboxStatus::Running => "[running]",
        SandboxStatus::Stopped => "[stopped]",
        SandboxStatus::NotFound => "[no container]",
    }
}

/// Display the list of sandboxes
pub fn display_sandbox_list(entries: &[SelectionEntry]) {
    if entries.is_empty() {
        println!("No sandboxes found.");
        return;
    }

    println!("\nAvailable sandboxes:");
    println!("{:-<60}", "");

    for (i, entry) in entries.iter().enumerate() {
        let status = format_status(&entry.status);
        println!(
            "  {}. {} {} - {}",
            i + 1,
            entry.name,
            status,
            entry.info.path.display()
        );
    }

    println!("{:-<60}", "");
}

/// Prompt user to select a sandbox by number
pub fn prompt_selection(entries: &[SelectionEntry]) -> Result<Option<&SelectionEntry>> {
    if entries.is_empty() {
        return Ok(None);
    }

    display_sandbox_list(entries);

    print!("\nSelect sandbox (1-{}) or 'q' to quit: ", entries.len());
    io::stdout().flush()?;

    let mut input = String::new();
    io::stdin().read_line(&mut input)?;
    let input = input.trim();

    if input.eq_ignore_ascii_case("q") {
        return Ok(None);
    }

    match input.parse::<usize>() {
        Ok(n) if n >= 1 && n <= entries.len() => Ok(Some(&entries[n - 1])),
        _ => {
            println!("Invalid selection");
            Ok(None)
        }
    }
}

/// Prompt for confirmation
pub fn confirm(message: &str) -> Result<bool> {
    print!("{} [y/N]: ", message);
    io::stdout().flush()?;

    let mut input = String::new();
    io::stdin().read_line(&mut input)?;
    let input = input.trim();

    Ok(input.eq_ignore_ascii_case("y") || input.eq_ignore_ascii_case("yes"))
}

/// Prompt for a required string value
pub fn prompt_string(message: &str, default: Option<&str>) -> Result<String> {
    if let Some(def) = default {
        print!("{} [{}]: ", message, def);
    } else {
        print!("{}: ", message);
    }
    io::stdout().flush()?;

    let mut input = String::new();
    io::stdin().read_line(&mut input)?;
    let input = input.trim();

    if input.is_empty() {
        if let Some(def) = default {
            return Ok(def.to_string());
        }
    }

    Ok(input.to_string())
}
