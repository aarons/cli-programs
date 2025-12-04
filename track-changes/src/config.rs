use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Config {
    /// List of directories to watch for changes
    #[serde(default)]
    pub directories: Vec<PathBuf>,
}

impl Config {
    /// Get the config file path
    pub fn config_path() -> Result<PathBuf> {
        let home = dirs::home_dir().context("Could not determine home directory")?;
        Ok(home
            .join(".config")
            .join("cli-programs")
            .join("track-changes.toml"))
    }

    /// Load configuration from file, returning default if it doesn't exist
    pub fn load() -> Result<Self> {
        let path = Self::config_path()?;

        if path.exists() {
            let content = fs::read_to_string(&path)
                .with_context(|| format!("Failed to read config file: {}", path.display()))?;
            let config: Config = toml::from_str(&content)
                .with_context(|| format!("Failed to parse config file: {}", path.display()))?;
            Ok(config)
        } else {
            Ok(Config::default())
        }
    }

    /// Save configuration to file
    pub fn save(&self) -> Result<()> {
        let path = Self::config_path()?;
        let dir = path.parent().unwrap();

        if !dir.exists() {
            fs::create_dir_all(dir)
                .with_context(|| format!("Failed to create config directory: {}", dir.display()))?;
        }

        let content = toml::to_string_pretty(self).context("Failed to serialize config")?;
        fs::write(&path, content)
            .with_context(|| format!("Failed to write config file: {}", path.display()))?;

        Ok(())
    }

    /// Add a directory to the watch list
    pub fn add_directory(&mut self, path: &Path) -> Result<bool> {
        let canonical = path
            .canonicalize()
            .with_context(|| format!("Failed to resolve path: {}", path.display()))?;

        if self.directories.contains(&canonical) {
            return Ok(false); // Already exists
        }

        self.directories.push(canonical);
        Ok(true)
    }

    /// Remove a directory from the watch list
    pub fn remove_directory(&mut self, path: &Path) -> Result<bool> {
        let canonical = path
            .canonicalize()
            .unwrap_or_else(|_| path.to_path_buf());

        let initial_len = self.directories.len();
        self.directories.retain(|d| d != &canonical);
        Ok(self.directories.len() < initial_len)
    }
}
