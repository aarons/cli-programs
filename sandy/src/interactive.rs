use anyhow::Result;
use std::io::{self, Write};

use crate::docker::{SandboxStatus, sandbox_status};
use crate::state::{SandboxInfo, State};
use crate::worktree::get_repo_name;

/// Display entry for interactive selection
pub struct SelectionEntry {
    /// Canonical path key (used for state lookup)
    pub key: String,
    /// Display name (derived from repo directory name)
    pub name: String,
    pub info: SandboxInfo,
    pub status: SandboxStatus,
}

/// Get all sandbox entries with their status
pub fn get_sandbox_entries(state: &State) -> Result<Vec<SelectionEntry>> {
    let mut entries = Vec::new();

    for (key, info) in &state.sandboxes {
        let status = sandbox_status(&info.path).unwrap_or(SandboxStatus::NotFound);
        let name = get_repo_name(&info.path);
        entries.push(SelectionEntry {
            key: key.clone(),
            name,
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

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use std::path::PathBuf;

    fn create_test_state_with_sandboxes(count: usize) -> State {
        let mut state = State::default();
        for i in 0..count {
            let path = PathBuf::from(format!("/test/repo{}", i));
            state.sandboxes.insert(
                path.to_string_lossy().to_string(),
                SandboxInfo {
                    path,
                    created_at: Utc::now() - chrono::Duration::hours(i as i64),
                    tool: Some("claude".to_string()),
                },
            );
        }
        state
    }

    #[test]
    fn test_selection_entry_fields() {
        let entry = SelectionEntry {
            key: "/test/repo".to_string(),
            name: "repo".to_string(),
            info: SandboxInfo {
                path: PathBuf::from("/test/repo"),
                created_at: Utc::now(),
                tool: Some("claude".to_string()),
            },
            status: SandboxStatus::Running,
        };

        assert_eq!(entry.key, "/test/repo");
        assert_eq!(entry.name, "repo");
        assert_eq!(entry.status, SandboxStatus::Running);
    }

    #[test]
    fn test_format_status_running() {
        let formatted = format_status(&SandboxStatus::Running);
        assert_eq!(formatted, "[running]");
    }

    #[test]
    fn test_format_status_stopped() {
        let formatted = format_status(&SandboxStatus::Stopped);
        assert_eq!(formatted, "[stopped]");
    }

    #[test]
    fn test_format_status_not_found() {
        let formatted = format_status(&SandboxStatus::NotFound);
        assert_eq!(formatted, "[no container]");
    }

    #[test]
    fn test_get_sandbox_entries_empty_state() {
        let state = State::default();
        let entries = get_sandbox_entries(&state).unwrap();
        assert!(entries.is_empty());
    }

    #[test]
    fn test_get_sandbox_entries_single() {
        let mut state = State::default();
        state.add_sandbox(PathBuf::from("/test/my-repo"), "claude");

        let entries = get_sandbox_entries(&state).unwrap();

        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].name, "my-repo");
        assert_eq!(entries[0].key, "/test/my-repo");
    }

    #[test]
    fn test_get_sandbox_entries_multiple() {
        let state = create_test_state_with_sandboxes(3);
        let entries = get_sandbox_entries(&state).unwrap();

        assert_eq!(entries.len(), 3);
    }

    #[test]
    fn test_get_sandbox_entries_sorted_by_creation_time() {
        let mut state = State::default();

        // Add sandboxes with different creation times
        let older_time = Utc::now() - chrono::Duration::hours(2);
        let newer_time = Utc::now();

        state.sandboxes.insert(
            "/older".to_string(),
            SandboxInfo {
                path: PathBuf::from("/older"),
                created_at: older_time,
                tool: Some("claude".to_string()),
            },
        );
        state.sandboxes.insert(
            "/newer".to_string(),
            SandboxInfo {
                path: PathBuf::from("/newer"),
                created_at: newer_time,
                tool: Some("gemini".to_string()),
            },
        );

        let entries = get_sandbox_entries(&state).unwrap();

        // Newer should be first (sorted by most recent)
        assert_eq!(entries[0].key, "/newer");
        assert_eq!(entries[1].key, "/older");
    }

    #[test]
    fn test_get_sandbox_entries_derives_name_from_path() {
        let mut state = State::default();
        state.add_sandbox(
            PathBuf::from("/home/user/projects/awesome-project"),
            "claude",
        );

        let entries = get_sandbox_entries(&state).unwrap();

        assert_eq!(entries[0].name, "awesome-project");
    }

    #[test]
    fn test_display_sandbox_list_empty() {
        // Just ensure it doesn't panic
        let entries: Vec<SelectionEntry> = vec![];
        display_sandbox_list(&entries);
    }

    #[test]
    fn test_display_sandbox_list_with_entries() {
        // Just ensure it doesn't panic
        let entries = vec![
            SelectionEntry {
                key: "/test/repo1".to_string(),
                name: "repo1".to_string(),
                info: SandboxInfo {
                    path: PathBuf::from("/test/repo1"),
                    created_at: Utc::now(),
                    tool: Some("claude".to_string()),
                },
                status: SandboxStatus::Running,
            },
            SelectionEntry {
                key: "/test/repo2".to_string(),
                name: "repo2".to_string(),
                info: SandboxInfo {
                    path: PathBuf::from("/test/repo2"),
                    created_at: Utc::now(),
                    tool: Some("gemini".to_string()),
                },
                status: SandboxStatus::Stopped,
            },
        ];
        display_sandbox_list(&entries);
    }

    #[test]
    fn test_selection_entry_with_different_statuses() {
        let statuses = [
            SandboxStatus::Running,
            SandboxStatus::Stopped,
            SandboxStatus::NotFound,
        ];

        for status in statuses {
            let entry = SelectionEntry {
                key: "/test".to_string(),
                name: "test".to_string(),
                info: SandboxInfo {
                    path: PathBuf::from("/test"),
                    created_at: Utc::now(),
                    tool: Some("claude".to_string()),
                },
                status: status.clone(),
            };

            // Verify the status is correctly stored
            assert_eq!(entry.status, status);
        }
    }
}
