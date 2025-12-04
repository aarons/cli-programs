// gc-specific configuration

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

/// Default maximum tokens for diff content before switching to summary mode
const DEFAULT_MAX_DIFF_TOKENS: usize = 30000;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GcConfig {
    /// Maximum estimated tokens for diff before prompting for context
    #[serde(default = "default_max_diff_tokens")]
    pub max_diff_tokens: usize,
}

fn default_max_diff_tokens() -> usize {
    DEFAULT_MAX_DIFF_TOKENS
}

impl Default for GcConfig {
    fn default() -> Self {
        Self {
            max_diff_tokens: DEFAULT_MAX_DIFF_TOKENS,
        }
    }
}

impl GcConfig {
    /// Get the config file path: ~/.config/cli-programs/gc.toml
    pub fn config_path() -> Result<PathBuf> {
        let home = std::env::var("HOME").or_else(|_| std::env::var("USERPROFILE"))?;
        Ok(PathBuf::from(home)
            .join(".config")
            .join("cli-programs")
            .join("gc.toml"))
    }

    /// Load config from file, returning default if file doesn't exist
    pub fn load() -> Result<Self> {
        let path = Self::config_path()?;

        if !path.exists() {
            return Ok(Self::default());
        }

        let content = fs::read_to_string(&path)?;
        let config: GcConfig = toml::from_str(&content)?;
        Ok(config)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = GcConfig::default();
        assert_eq!(config.max_diff_tokens, 30000);
    }

    #[test]
    fn test_config_path() {
        let path = GcConfig::config_path();
        assert!(path.is_ok());
        let path = path.unwrap();
        assert!(path.ends_with("cli-programs/gc.toml"));
    }

    #[test]
    fn test_parse_config() {
        let toml_str = r#"
max_diff_tokens = 50000
"#;
        let config: GcConfig = toml::from_str(toml_str).unwrap();
        assert_eq!(config.max_diff_tokens, 50000);
    }

    #[test]
    fn test_parse_empty_config() {
        let toml_str = "";
        let config: GcConfig = toml::from_str(toml_str).unwrap();
        assert_eq!(config.max_diff_tokens, 30000); // default
    }
}
