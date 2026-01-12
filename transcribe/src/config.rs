use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    /// Path to the whisper-cli binary
    #[serde(default = "default_whisper_cli_path")]
    pub whisper_cli_path: String,

    /// Directory containing whisper models
    #[serde(default = "default_models_dir")]
    pub models_dir: String,

    /// Default model to use: "medium" or "large-turbo"
    #[serde(default = "default_model")]
    pub default_model: String,
}

fn default_whisper_cli_path() -> String {
    let home = std::env::var("HOME").unwrap_or_default();
    format!("{}/code/whisper.cpp/build/bin/whisper-cli", home)
}

fn default_models_dir() -> String {
    let home = std::env::var("HOME").unwrap_or_default();
    format!("{}/code/whisper.cpp/models", home)
}

fn default_model() -> String {
    "medium".to_string()
}

impl Default for Config {
    fn default() -> Self {
        Self {
            whisper_cli_path: default_whisper_cli_path(),
            models_dir: default_models_dir(),
            default_model: default_model(),
        }
    }
}

impl Config {
    /// Get the config file path: ~/.config/cli-programs/transcribe.toml
    pub fn config_path() -> Result<PathBuf> {
        let home = std::env::var("HOME").or_else(|_| std::env::var("USERPROFILE"))?;
        Ok(PathBuf::from(home)
            .join(".config")
            .join("cli-programs")
            .join("transcribe.toml"))
    }

    /// Load config from file, returning default if file doesn't exist
    pub fn load() -> Result<Self> {
        let path = Self::config_path()?;

        if !path.exists() {
            return Ok(Self::default());
        }

        let content = fs::read_to_string(&path).context("Failed to read config file")?;
        let config: Config = toml::from_str(&content).context("Failed to parse config file")?;
        Ok(config)
    }

    /// Save config to file
    pub fn save(&self) -> Result<()> {
        let path = Self::config_path()?;

        // Create parent directories if they don't exist
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).context("Failed to create config directory")?;
        }

        let content = toml::to_string_pretty(self).context("Failed to serialize config")?;
        fs::write(&path, content).context("Failed to write config file")?;
        Ok(())
    }

    /// Get the full path to a model file
    pub fn model_path(&self, model: &str) -> PathBuf {
        let model_file = match model {
            "medium" => "ggml-medium.en.bin",
            "large-turbo" => "ggml-large-v3-turbo.bin",
            _ => model, // Allow passing full filename
        };
        PathBuf::from(&self.models_dir).join(model_file)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = Config::default();
        assert_eq!(config.default_model, "medium");
        assert!(config.whisper_cli_path.contains("whisper-cli"));
        assert!(config.models_dir.contains("models"));
    }

    #[test]
    fn test_model_path() {
        let config = Config {
            whisper_cli_path: "/usr/bin/whisper-cli".to_string(),
            models_dir: "/models".to_string(),
            default_model: "medium".to_string(),
        };

        assert_eq!(
            config.model_path("medium"),
            PathBuf::from("/models/ggml-medium.en.bin")
        );
        assert_eq!(
            config.model_path("large-turbo"),
            PathBuf::from("/models/ggml-large-v3-turbo.bin")
        );
    }

    #[test]
    fn test_parse_config() {
        let toml_str = r#"
whisper_cli_path = "/custom/path/whisper-cli"
models_dir = "/custom/models"
default_model = "large-turbo"
"#;
        let config: Config = toml::from_str(toml_str).unwrap();
        assert_eq!(config.whisper_cli_path, "/custom/path/whisper-cli");
        assert_eq!(config.models_dir, "/custom/models");
        assert_eq!(config.default_model, "large-turbo");
    }
}
