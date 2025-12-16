// gena configuration management

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

const DEFAULT_BACKEND: &str = "macos-say";
const DEFAULT_RATE: u32 = 175;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GenaConfig {
    /// TTS backend to use
    #[serde(default = "default_backend")]
    pub backend: String,

    /// Default voice (None uses system default)
    #[serde(default)]
    pub voice: Option<String>,

    /// Speaking rate in words per minute
    #[serde(default = "default_rate")]
    pub rate: u32,
}

fn default_backend() -> String {
    DEFAULT_BACKEND.to_string()
}

fn default_rate() -> u32 {
    DEFAULT_RATE
}

impl Default for GenaConfig {
    fn default() -> Self {
        Self {
            backend: default_backend(),
            voice: None,
            rate: default_rate(),
        }
    }
}

impl GenaConfig {
    /// Get the config file path: ~/.config/cli-programs/gena.toml
    pub fn config_path() -> Result<PathBuf> {
        let home = std::env::var("HOME").or_else(|_| std::env::var("USERPROFILE"))?;
        Ok(PathBuf::from(home)
            .join(".config")
            .join("cli-programs")
            .join("gena.toml"))
    }

    /// Load config from file, returning default if file doesn't exist
    pub fn load() -> Result<Self> {
        let path = Self::config_path()?;

        if !path.exists() {
            return Ok(Self::default());
        }

        let content = fs::read_to_string(&path)?;
        let config: GenaConfig = toml::from_str(&content)?;
        Ok(config)
    }

    /// Save config to file
    pub fn save(&self) -> Result<()> {
        let path = Self::config_path()?;

        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }

        let content = toml::to_string_pretty(self)?;
        fs::write(&path, content)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = GenaConfig::default();
        assert_eq!(config.backend, "macos-say");
        assert_eq!(config.rate, 175);
        assert!(config.voice.is_none());
    }

    #[test]
    fn test_config_path() {
        let path = GenaConfig::config_path();
        assert!(path.is_ok());
        let path = path.unwrap();
        assert!(path.ends_with("cli-programs/gena.toml"));
    }

    #[test]
    fn test_parse_config() {
        let toml_str = r#"
backend = "elevenlabs"
voice = "Rachel"
rate = 200
"#;
        let config: GenaConfig = toml::from_str(toml_str).unwrap();
        assert_eq!(config.backend, "elevenlabs");
        assert_eq!(config.voice, Some("Rachel".to_string()));
        assert_eq!(config.rate, 200);
    }

    #[test]
    fn test_parse_empty_config() {
        let toml_str = "";
        let config: GenaConfig = toml::from_str(toml_str).unwrap();
        assert_eq!(config.backend, "macos-say");
        assert_eq!(config.rate, 175);
    }
}
