use anyhow::{Context, Result};
use chrono::{DateTime, Local};
use serde::{Deserialize, Serialize};
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogEntry {
    /// Directory that was committed
    pub directory: PathBuf,
    /// When the commit was made
    pub timestamp: DateTime<Local>,
    /// Files that were changed (from git status)
    pub files_changed: Vec<String>,
    /// The commit hash
    pub commit_hash: String,
}

pub struct CommitLog;

impl CommitLog {
    /// Get the log file path
    pub fn log_path() -> Result<PathBuf> {
        let home = dirs::home_dir().context("Could not determine home directory")?;
        Ok(home
            .join(".local")
            .join("share")
            .join("track-changes")
            .join("commits.log"))
    }

    /// Append a log entry to the log file (JSON Lines format)
    pub fn append(entry: &LogEntry) -> Result<()> {
        let path = Self::log_path()?;

        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("Failed to create log directory: {}", parent.display()))?;
        }

        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)
            .with_context(|| format!("Failed to open log file: {}", path.display()))?;

        let line = serde_json::to_string(entry).context("Failed to serialize log entry")?;
        writeln!(file, "{}", line).context("Failed to write log entry")?;

        Ok(())
    }

    /// Read the most recent N log entries
    pub fn read_recent(count: usize) -> Result<Vec<LogEntry>> {
        let path = Self::log_path()?;

        if !path.exists() {
            return Ok(vec![]);
        }

        let content = fs::read_to_string(&path)
            .with_context(|| format!("Failed to read log file: {}", path.display()))?;

        let entries: Vec<LogEntry> = content
            .lines()
            .filter_map(|line| serde_json::from_str(line).ok())
            .collect();

        // Return last N entries (most recent last in file, so take from end)
        let start = entries.len().saturating_sub(count);
        Ok(entries[start..].to_vec())
    }
}
