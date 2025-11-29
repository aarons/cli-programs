use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

use crate::config::Config;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorktreeInfo {
    /// Full path to the worktree
    pub path: PathBuf,
    /// Path to the source repository
    pub source_repo: PathBuf,
    /// Branch the worktree was created from
    pub source_branch: String,
    /// When the worktree was created
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct State {
    /// Map of worktree name to info
    pub worktrees: HashMap<String, WorktreeInfo>,
}

impl State {
    /// Get the state file path
    pub fn state_path() -> Result<PathBuf> {
        Ok(Config::config_dir()?.join("sandbox-state.json"))
    }

    /// Load state from file
    pub fn load() -> Result<Self> {
        let path = Self::state_path()?;

        if path.exists() {
            let content = fs::read_to_string(&path)
                .with_context(|| format!("Failed to read state file: {}", path.display()))?;
            let state: State = serde_json::from_str(&content)
                .with_context(|| format!("Failed to parse state file: {}", path.display()))?;
            Ok(state)
        } else {
            Ok(State::default())
        }
    }

    /// Save state to file
    pub fn save(&self) -> Result<()> {
        let path = Self::state_path()?;
        let dir = path.parent().unwrap();

        if !dir.exists() {
            fs::create_dir_all(dir)
                .with_context(|| format!("Failed to create state directory: {}", dir.display()))?;
        }

        let content = serde_json::to_string_pretty(self).context("Failed to serialize state")?;
        fs::write(&path, content)
            .with_context(|| format!("Failed to write state file: {}", path.display()))?;

        Ok(())
    }

    /// Add a worktree to the state
    pub fn add_worktree(
        &mut self,
        name: String,
        path: PathBuf,
        source_repo: PathBuf,
        source_branch: String,
    ) {
        self.worktrees.insert(
            name,
            WorktreeInfo {
                path,
                source_repo,
                source_branch,
                created_at: Utc::now(),
            },
        );
    }

    /// Remove a worktree from the state
    pub fn remove_worktree(&mut self, name: &str) -> Option<WorktreeInfo> {
        self.worktrees.remove(name)
    }

    /// Get a worktree by name
    pub fn get_worktree(&self, name: &str) -> Option<&WorktreeInfo> {
        self.worktrees.get(name)
    }

}

/// Get the template hash file path
pub fn template_hash_path() -> Result<PathBuf> {
    Ok(Config::config_dir()?.join("sandbox-template.hash"))
}

/// Load the stored template hash
pub fn load_template_hash() -> Result<Option<String>> {
    let path = template_hash_path()?;
    if path.exists() {
        let hash = fs::read_to_string(&path)
            .with_context(|| format!("Failed to read template hash: {}", path.display()))?;
        Ok(Some(hash.trim().to_string()))
    } else {
        Ok(None)
    }
}

/// Save the template hash
pub fn save_template_hash(hash: &str) -> Result<()> {
    let path = template_hash_path()?;
    let dir = path.parent().unwrap();

    if !dir.exists() {
        fs::create_dir_all(dir)
            .with_context(|| format!("Failed to create directory: {}", dir.display()))?;
    }

    fs::write(&path, hash)
        .with_context(|| format!("Failed to write template hash: {}", path.display()))?;

    Ok(())
}
