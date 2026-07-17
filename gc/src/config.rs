// gc-specific configuration

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

/// Default maximum tokens for diff content before switching to summary mode
const DEFAULT_MAX_DIFF_TOKENS: usize = 30000;

/// Commented example config written by `gc config init`
pub const EXAMPLE_CONFIG: &str = r#"# gc configuration
# Location: ~/.config/cli-programs/gc.toml

# Maximum estimated diff tokens before gc switches to summary mode
# (prompts for a description and sends the file list instead of the full diff)
max_diff_tokens = 30000

# Optional: mirror every push to a local git server (e.g. soft-serve).
# When configured, gc pushes to this server after every commit, in addition
# to origin (or as the sole target when the repo has no origin).
# Failures pushing to the local server are warnings, never fatal.
# The repo path is derived from the git root directory name, so a repo at
# ~/code/cli-programs pushes to ssh://localhost:23231/cli-programs.
#
# [local_server]
# url = "ssh://localhost:23231"
# remote_name = "local-git"  # git remote to create/update (default: "local-git")
"#;

fn default_local_remote_name() -> String {
    "local-git".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LocalServerConfig {
    /// SSH URL base, e.g. "ssh://localhost:23231"
    pub url: String,
    /// Git remote name (default: "local-git")
    #[serde(default = "default_local_remote_name")]
    pub remote_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GcConfig {
    /// Maximum estimated tokens for diff before prompting for context
    #[serde(default = "default_max_diff_tokens")]
    pub max_diff_tokens: usize,
    /// Optional local git server for mirroring pushes
    #[serde(default)]
    pub local_server: Option<LocalServerConfig>,
}

fn default_max_diff_tokens() -> usize {
    DEFAULT_MAX_DIFF_TOKENS
}

impl Default for GcConfig {
    fn default() -> Self {
        Self {
            max_diff_tokens: DEFAULT_MAX_DIFF_TOKENS,
            local_server: None,
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
        assert!(config.local_server.is_none());
    }

    #[test]
    fn test_parse_config_with_local_server() {
        let toml_str = r#"
max_diff_tokens = 50000

[local_server]
url = "ssh://localhost:23231"
"#;
        let config: GcConfig = toml::from_str(toml_str).unwrap();
        assert_eq!(config.max_diff_tokens, 50000);
        let server = config.local_server.unwrap();
        assert_eq!(server.url, "ssh://localhost:23231");
        assert_eq!(server.remote_name, "local-git"); // default
    }

    #[test]
    fn test_parse_config_with_local_server_custom_remote() {
        let toml_str = r#"
[local_server]
url = "ssh://myserver:2222"
remote_name = "backup"
"#;
        let config: GcConfig = toml::from_str(toml_str).unwrap();
        let server = config.local_server.unwrap();
        assert_eq!(server.url, "ssh://myserver:2222");
        assert_eq!(server.remote_name, "backup");
    }

    #[test]
    fn test_example_config_parses_to_defaults() {
        // Guards against edits to the template breaking its TOML syntax or
        // shipping non-default values as the "example defaults"
        let config: GcConfig = toml::from_str(EXAMPLE_CONFIG).unwrap();
        assert_eq!(config.max_diff_tokens, DEFAULT_MAX_DIFF_TOKENS);
        assert!(config.local_server.is_none());
    }

    #[test]
    fn test_example_config_local_server_section_is_valid_when_uncommented() {
        // The commented-out [local_server] block must stay valid TOML so users
        // can enable it by just removing the leading "# " markers
        let uncommented: String = EXAMPLE_CONFIG
            .lines()
            .filter_map(|line| {
                let rest = line.strip_prefix("# ")?;
                (rest.starts_with('[') || rest.contains(" = ")).then_some(rest)
            })
            .collect::<Vec<_>>()
            .join("\n");
        let config: GcConfig = toml::from_str(&uncommented).unwrap();
        let server = config.local_server.expect("local_server should parse");
        assert_eq!(server.url, "ssh://localhost:23231");
        assert_eq!(server.remote_name, "local-git");
    }

    #[test]
    fn test_parse_config_without_local_server() {
        let toml_str = r#"
max_diff_tokens = 20000
"#;
        let config: GcConfig = toml::from_str(toml_str).unwrap();
        assert_eq!(config.max_diff_tokens, 20000);
        assert!(config.local_server.is_none());
    }
}
