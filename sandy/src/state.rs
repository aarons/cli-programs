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
    /// Alias "worktrees" for backwards compatibility with pre-v0.2.0 state files
    #[serde(alias = "worktrees")]
    pub sandboxes: HashMap<String, SandboxInfo>,
}

impl State {
    /// Get the state file path
    pub fn state_path() -> Result<PathBuf> {
        Ok(Config::config_dir()?.join("sandy-state.json"))
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

/// Get the template hash file path (tracks user's Dockerfile hash after build)
pub fn template_hash_path() -> Result<PathBuf> {
    Ok(Config::config_dir()?.join("sandy-template.hash"))
}

/// Get the default template hash file path (tracks which embedded default was used)
pub fn default_template_hash_path() -> Result<PathBuf> {
    Ok(Config::config_dir()?.join("sandy-default-template.hash"))
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

/// Load the stored default template hash (tracks which embedded default was used)
pub fn load_default_template_hash() -> Result<Option<String>> {
    let path = default_template_hash_path()?;
    if path.exists() {
        let hash = fs::read_to_string(&path)
            .with_context(|| format!("Failed to read default template hash: {}", path.display()))?;
        Ok(Some(hash.trim().to_string()))
    } else {
        Ok(None)
    }
}

/// Save the default template hash
pub fn save_default_template_hash(hash: &str) -> Result<()> {
    let path = default_template_hash_path()?;
    let dir = path.parent().unwrap();

    if !dir.exists() {
        fs::create_dir_all(dir)
            .with_context(|| format!("Failed to create directory: {}", dir.display()))?;
    }

    fs::write(&path, hash)
        .with_context(|| format!("Failed to write default template hash: {}", path.display()))?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_add_sandbox() {
        let mut state = State::default();
        let path = PathBuf::from("/test/repo");

        state.add_sandbox(path.clone());

        assert_eq!(state.sandboxes.len(), 1);
        let key = path.to_string_lossy().to_string();
        assert!(state.sandboxes.contains_key(&key));

        let info = state.sandboxes.get(&key).unwrap();
        assert_eq!(info.path, path);
    }

    #[test]
    fn test_add_multiple_sandboxes() {
        let mut state = State::default();
        let path1 = PathBuf::from("/test/repo1");
        let path2 = PathBuf::from("/test/repo2");

        state.add_sandbox(path1.clone());
        state.add_sandbox(path2.clone());

        assert_eq!(state.sandboxes.len(), 2);
        assert!(
            state
                .sandboxes
                .contains_key(&path1.to_string_lossy().to_string())
        );
        assert!(
            state
                .sandboxes
                .contains_key(&path2.to_string_lossy().to_string())
        );
    }

    #[test]
    fn test_add_sandbox_overwrites_existing() {
        let mut state = State::default();
        let path = PathBuf::from("/test/repo");

        state.add_sandbox(path.clone());
        let first_time = state
            .sandboxes
            .get(&path.to_string_lossy().to_string())
            .unwrap()
            .created_at;

        // Small delay to ensure different timestamp
        std::thread::sleep(std::time::Duration::from_millis(10));

        state.add_sandbox(path.clone());
        let second_time = state
            .sandboxes
            .get(&path.to_string_lossy().to_string())
            .unwrap()
            .created_at;

        assert_eq!(state.sandboxes.len(), 1);
        assert!(second_time > first_time);
    }

    #[test]
    fn test_remove_sandbox() {
        let mut state = State::default();
        let path = PathBuf::from("/test/repo");

        state.add_sandbox(path.clone());
        assert_eq!(state.sandboxes.len(), 1);

        let removed = state.remove_sandbox(&path.to_string_lossy().to_string());
        assert!(removed.is_some());
        assert_eq!(removed.unwrap().path, path);
        assert!(state.sandboxes.is_empty());
    }

    #[test]
    fn test_remove_nonexistent_sandbox() {
        let mut state = State::default();
        let removed = state.remove_sandbox("/nonexistent");
        assert!(removed.is_none());
    }

    #[test]
    fn test_sandbox_info_serialization() {
        let info = SandboxInfo {
            path: PathBuf::from("/test/path"),
            created_at: Utc::now(),
        };

        let serialized = serde_json::to_string(&info).unwrap();
        let deserialized: SandboxInfo = serde_json::from_str(&serialized).unwrap();

        assert_eq!(deserialized.path, info.path);
        assert_eq!(deserialized.created_at, info.created_at);
    }

    #[test]
    fn test_state_serialization_roundtrip() {
        let mut state = State::default();
        state.add_sandbox(PathBuf::from("/repo1"));
        state.add_sandbox(PathBuf::from("/repo2"));

        let serialized = serde_json::to_string_pretty(&state).unwrap();
        let deserialized: State = serde_json::from_str(&serialized).unwrap();

        assert_eq!(deserialized.sandboxes.len(), state.sandboxes.len());
        for (key, info) in &state.sandboxes {
            let other = deserialized.sandboxes.get(key).unwrap();
            assert_eq!(other.path, info.path);
        }
    }

    #[test]
    fn test_state_save_and_load() {
        let temp_dir = TempDir::new().unwrap();
        let state_path = temp_dir.path().join("sandbox-state.json");

        // Create and save state
        let mut state = State::default();
        state.add_sandbox(PathBuf::from("/test/repo"));

        let content = serde_json::to_string_pretty(&state).unwrap();
        fs::write(&state_path, &content).unwrap();

        // Load and verify
        let loaded_content = fs::read_to_string(&state_path).unwrap();
        let loaded_state: State = serde_json::from_str(&loaded_content).unwrap();

        assert_eq!(loaded_state.sandboxes.len(), 1);
        assert!(loaded_state.sandboxes.contains_key("/test/repo"));
    }

    #[test]
    fn test_state_with_special_characters_in_path() {
        let mut state = State::default();
        let path = PathBuf::from("/test/repo with spaces/and-dashes_underscores");

        state.add_sandbox(path.clone());

        let serialized = serde_json::to_string(&state).unwrap();
        let deserialized: State = serde_json::from_str(&serialized).unwrap();

        assert_eq!(deserialized.sandboxes.len(), 1);
        let key = path.to_string_lossy().to_string();
        assert!(deserialized.sandboxes.contains_key(&key));
    }

    #[test]
    fn test_sandbox_info_created_at_is_current() {
        let before = Utc::now();
        let mut state = State::default();
        state.add_sandbox(PathBuf::from("/test"));
        let after = Utc::now();

        let info = state.sandboxes.get("/test").unwrap();
        assert!(info.created_at >= before);
        assert!(info.created_at <= after);
    }

    #[test]
    fn test_legacy_state_file_with_worktrees_key() {
        // Pre-v0.2.0 state files used "worktrees" instead of "sandboxes"
        // This test ensures backwards compatibility
        let legacy_json = r#"{
            "worktrees": {
                "/test/repo": {
                    "path": "/test/repo",
                    "created_at": "2024-01-01T00:00:00Z"
                }
            }
        }"#;

        let state: State = serde_json::from_str(legacy_json)
            .expect("Should parse legacy state file with 'worktrees' key");

        assert_eq!(state.sandboxes.len(), 1);
        assert!(state.sandboxes.contains_key("/test/repo"));
        let info = state.sandboxes.get("/test/repo").unwrap();
        assert_eq!(info.path, PathBuf::from("/test/repo"));
    }

    #[test]
    fn test_state_ignores_unknown_fields() {
        // Forward compatibility: unknown fields should be ignored
        let json_with_extra = r#"{
            "sandboxes": {
                "/test/repo": {
                    "path": "/test/repo",
                    "created_at": "2024-01-01T00:00:00Z"
                }
            },
            "version": "2.0",
            "some_future_field": "value"
        }"#;

        let state: State = serde_json::from_str(json_with_extra)
            .expect("Should ignore unknown fields for forward compatibility");

        assert_eq!(state.sandboxes.len(), 1);
    }
}
