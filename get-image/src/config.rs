use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

/// Persistent defaults for image generation.
///
/// Stored at `~/.config/cli-programs/get-image.toml`. Every field is a
/// default that command-line flags override for a single run.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    /// OpenRouter model identifier, e.g. "google/gemini-2.5-flash-image"
    #[serde(default = "default_model")]
    pub model: String,

    /// Image quality: "low", "medium", "high", or "auto"
    #[serde(default = "default_quality")]
    pub quality: String,

    /// Image size: a resolution tier ("512", "1K", "2K", "4K"), a single
    /// dimension ("1024"), or "WIDTHxHEIGHT" ("1024x768")
    #[serde(default = "default_size")]
    pub size: String,

    /// Number of images to generate per prompt
    #[serde(default = "default_count")]
    pub count: u32,

    /// Open images in the system viewer after saving
    #[serde(default)]
    pub open_after_save: bool,
}

fn default_model() -> String {
    "google/gemini-2.5-flash-image".to_string()
}

fn default_quality() -> String {
    "low".to_string()
}

fn default_size() -> String {
    "512".to_string()
}

fn default_count() -> u32 {
    1
}

impl Default for Config {
    fn default() -> Self {
        Self {
            model: default_model(),
            quality: default_quality(),
            size: default_size(),
            count: default_count(),
            open_after_save: false,
        }
    }
}

impl Config {
    /// Get the config file path: ~/.config/cli-programs/get-image.toml
    pub fn config_path() -> Result<PathBuf> {
        let home = std::env::var("HOME").context("HOME not set")?;
        Ok(PathBuf::from(home)
            .join(".config")
            .join("cli-programs")
            .join("get-image.toml"))
    }

    /// Load config from file, returning defaults if the file doesn't exist
    pub fn load() -> Result<Self> {
        let path = Self::config_path()?;

        if !path.exists() {
            return Ok(Self::default());
        }

        let content = fs::read_to_string(&path)
            .with_context(|| format!("Failed to read config file: {}", path.display()))?;
        let config: Config = toml::from_str(&content)
            .with_context(|| format!("Failed to parse config file: {}", path.display()))?;
        Ok(config)
    }

    /// Save config to file
    pub fn save(&self) -> Result<()> {
        let path = Self::config_path()?;

        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).context("Failed to create config directory")?;
        }

        let content = toml::to_string_pretty(self).context("Failed to serialize config")?;
        fs::write(&path, content)
            .with_context(|| format!("Failed to write config file: {}", path.display()))?;
        Ok(())
    }

    /// Set a named field from a string value, validating known fields
    pub fn set(&mut self, key: &str, value: &str) -> Result<()> {
        match key {
            "model" => self.model = value.to_string(),
            "quality" => self.quality = parse_quality(value)?,
            "size" => self.size = parse_size(value)?,
            "count" => self.count = parse_count(value)?,
            "open_after_save" => {
                self.open_after_save = value
                    .parse()
                    .with_context(|| format!("Invalid boolean: {}", value))?
            }
            _ => anyhow::bail!(
                "Unknown config key: {} (valid: model, quality, size, count, open_after_save)",
                key
            ),
        }
        Ok(())
    }
}

/// Validate a quality setting
pub fn parse_quality(value: &str) -> Result<String> {
    let value = value.trim().to_lowercase();
    match value.as_str() {
        "low" | "medium" | "high" | "auto" => Ok(value),
        _ => anyhow::bail!("Invalid quality: {} (valid: low, medium, high, auto)", value),
    }
}

/// Validate a size setting: a resolution tier ("512", "1K", "2K", "4K"),
/// a single dimension ("1024"), or "WIDTHxHEIGHT" ("1024x768")
pub fn parse_size(value: &str) -> Result<String> {
    let value = value.trim().to_lowercase();

    // Resolution tiers, normalized to the uppercase form the API uses
    if let "1k" | "2k" | "4k" = value.as_str() {
        return Ok(value.to_uppercase());
    }

    let dimensions: Vec<&str> = value.split('x').collect();
    let valid = match dimensions.as_slice() {
        [single] => single.parse::<u32>().is_ok_and(|n| n > 0),
        [width, height] => {
            width.parse::<u32>().is_ok_and(|n| n > 0) && height.parse::<u32>().is_ok_and(|n| n > 0)
        }
        _ => false,
    };

    if !valid {
        anyhow::bail!(
            "Invalid size: {} (examples: 512, 1K, 2K, 4K, 1024, 1024x768)",
            value
        );
    }
    Ok(value)
}

/// Validate an image count. Copies are made with parallel requests, so the
/// ceiling is a courtesy guard against accidental large bills.
pub fn parse_count(value: &str) -> Result<u32> {
    let count: u32 = value
        .trim()
        .parse()
        .with_context(|| format!("Invalid count: {}", value))?;
    if count == 0 || count > 10 {
        anyhow::bail!("Count must be between 1 and 10, got {}", count);
    }
    Ok(count)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config_uses_inexpensive_settings() {
        let config = Config::default();
        assert_eq!(config.model, "google/gemini-2.5-flash-image");
        assert_eq!(config.quality, "low");
        assert_eq!(config.size, "512");
        assert_eq!(config.count, 1);
        assert!(!config.open_after_save);
    }

    #[test]
    fn test_partial_config_file_fills_in_defaults() {
        let config: Config = toml::from_str(r#"model = "openai/gpt-image-1""#).unwrap();
        assert_eq!(config.model, "openai/gpt-image-1");
        assert_eq!(config.quality, "low");
        assert_eq!(config.size, "512");
    }

    #[test]
    fn test_set_rejects_unknown_key_and_invalid_values() {
        let mut config = Config::default();
        assert!(config.set("model", "openai/gpt-image-1").is_ok());
        assert_eq!(config.model, "openai/gpt-image-1");

        assert!(config.set("nonexistent", "x").is_err());
        assert!(config.set("quality", "ultra").is_err());
        assert!(config.set("size", "abc").is_err());
        assert!(config.set("count", "0").is_err());
    }

    #[test]
    fn test_parse_quality_normalizes_case() {
        assert_eq!(parse_quality("LOW").unwrap(), "low");
        assert!(parse_quality("best").is_err());
    }

    #[test]
    fn test_parse_size_accepts_tiers_dimensions_and_rectangles() {
        assert_eq!(parse_size("512").unwrap(), "512");
        assert_eq!(parse_size("1k").unwrap(), "1K");
        assert_eq!(parse_size("4K").unwrap(), "4K");
        assert_eq!(parse_size("1024X768").unwrap(), "1024x768");
        assert!(parse_size("0").is_err());
        assert!(parse_size("512x").is_err());
        assert!(parse_size("1x2x3").is_err());
        assert!(parse_size("8k").is_err());
    }
}
