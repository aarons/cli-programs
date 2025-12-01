use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

use crate::config::Config;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SandboxInfo {
    /// Full path to the repository
    pub path: PathBuf,
    /// When the sandbox was created
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct State {
    /// Map of canonical repo path to sandbox info
    pub sandboxes: HashMap<String, SandboxInfo>,
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

    /// Add a sandbox to the state (keyed by canonical repo path)
    pub fn add_sandbox(&mut self, repo_path: PathBuf) {
        let key = repo_path.to_string_lossy().to_string();
        self.sandboxes.insert(
            key,
            SandboxInfo {
                path: repo_path,
                created_at: Utc::now(),
            },
        );
    }

    /// Remove a sandbox from the state
    pub fn remove_sandbox(&mut self, key: &str) -> Option<SandboxInfo> {
        self.sandboxes.remove(key)
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
