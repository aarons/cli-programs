use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Mount {
    pub source: String,
    pub target: String,
    #[serde(default)]
    pub readonly: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    /// Custom Docker template image name
    #[serde(default)]
    pub template_image: Option<String>,

    /// Directories containing binaries to include in the template image
    #[serde(default = "default_binary_dirs")]
    pub binary_dirs: Vec<String>,

    /// Environment variables to pass to containers
    #[serde(default)]
    pub env: HashMap<String, String>,

    /// Additional volume mounts
    #[serde(default)]
    pub mounts: Vec<Mount>,
}

fn default_binary_dirs() -> Vec<String> {
    vec!["~/.local/bin".to_string()]
}

impl Default for Config {
    fn default() -> Self {
        Self {
            template_image: None,
            binary_dirs: default_binary_dirs(),
            env: HashMap::new(),
            mounts: vec![
                Mount {
                    source: "~/.ssh".to_string(),
                    target: "/home/agent/.ssh".to_string(),
                    readonly: true,
                },
                Mount {
                    source: "~/.gitconfig".to_string(),
                    target: "/home/agent/.gitconfig".to_string(),
                    readonly: true,
                },
                Mount {
                    source: "~/.claude".to_string(),
                    target: "/home/agent/.claude".to_string(),
                    readonly: false,
                },
            ],
        }
    }
}

impl Config {
    /// Get the config directory path.
    ///
    /// Can be overridden via the `SANDY_CONFIG_DIR` environment variable,
    /// which is useful for testing without affecting the user's real config.
    pub fn config_dir() -> Result<PathBuf> {
        if let Ok(override_dir) = std::env::var("SANDY_CONFIG_DIR") {
            return Ok(PathBuf::from(override_dir));
        }
        let home = dirs::home_dir().context("Could not determine home directory")?;
        Ok(home.join(".config").join("cli-programs"))
    }

    /// Get the config file path
    pub fn config_path() -> Result<PathBuf> {
        Ok(Self::config_dir()?.join("sandy.toml"))
    }

    /// Load configuration from file, creating default if it doesn't exist
    pub fn load() -> Result<Self> {
        let path = Self::config_path()?;

        if path.exists() {
            let content = fs::read_to_string(&path)
                .with_context(|| format!("Failed to read config file: {}", path.display()))?;
            let config: Config = toml::from_str(&content)
                .with_context(|| format!("Failed to parse config file: {}", path.display()))?;
            Ok(config)
        } else {
            // Create default config file for user to edit
            let config = Config::default();
            config.save()?;
            Ok(config)
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

    /// Expand environment variables in a string value
    pub fn expand_env(value: &str) -> Result<String> {
        let expanded = shellexpand::env(value)
            .with_context(|| format!("Failed to expand environment variables in: {}", value))?;
        Ok(expanded.to_string())
    }

    /// Expand a path (tilde and env vars)
    pub fn expand_path(path: &str) -> Result<PathBuf> {
        let expanded =
            shellexpand::full(path).with_context(|| format!("Failed to expand path: {}", path))?;
        Ok(PathBuf::from(expanded.as_ref()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;

    #[test]
    fn test_default_config_has_expected_values() {
        let config = Config::default();

        assert!(config.template_image.is_none());
        assert_eq!(config.binary_dirs, vec!["~/.local/bin".to_string()]);
        assert!(config.env.is_empty());
        assert_eq!(config.mounts.len(), 3);

        // Check default mounts
        assert_eq!(config.mounts[0].source, "~/.ssh");
        assert_eq!(config.mounts[0].target, "/home/agent/.ssh");
        assert!(config.mounts[0].readonly);

        assert_eq!(config.mounts[1].source, "~/.gitconfig");
        assert_eq!(config.mounts[1].target, "/home/agent/.gitconfig");
        assert!(config.mounts[1].readonly);

        assert_eq!(config.mounts[2].source, "~/.claude");
        assert_eq!(config.mounts[2].target, "/home/agent/.claude");
        assert!(!config.mounts[2].readonly);
    }

    #[test]
    fn test_mount_serialization() {
        let mount = Mount {
            source: "/src".to_string(),
            target: "/dst".to_string(),
            readonly: true,
        };

        let serialized = toml::to_string(&mount).unwrap();
        assert!(serialized.contains("source = \"/src\""));
        assert!(serialized.contains("target = \"/dst\""));
        assert!(serialized.contains("readonly = true"));

        let deserialized: Mount = toml::from_str(&serialized).unwrap();
        assert_eq!(deserialized.source, mount.source);
        assert_eq!(deserialized.target, mount.target);
        assert_eq!(deserialized.readonly, mount.readonly);
    }

    #[test]
    fn test_mount_readonly_defaults_to_false() {
        let toml_str = r#"
            source = "/src"
            target = "/dst"
        "#;

        let mount: Mount = toml::from_str(toml_str).unwrap();
        assert!(!mount.readonly);
    }

    #[test]
    fn test_config_serialization_roundtrip() {
        let mut config = Config::default();
        config.template_image = Some("my-template".to_string());
        config.binary_dirs = vec!["/usr/bin".to_string(), "~/.cargo/bin".to_string()];
        config.env.insert("MY_VAR".to_string(), "value".to_string());

        let serialized = toml::to_string_pretty(&config).unwrap();
        let deserialized: Config = toml::from_str(&serialized).unwrap();

        assert_eq!(deserialized.template_image, config.template_image);
        assert_eq!(deserialized.binary_dirs, config.binary_dirs);
        assert_eq!(deserialized.env, config.env);
        assert_eq!(deserialized.mounts.len(), config.mounts.len());
    }

    #[test]
    fn test_expand_env_with_home() {
        let home = env::var("HOME").unwrap();
        let expanded = Config::expand_env("$HOME/test").unwrap();
        assert!(expanded.starts_with(&home));
        assert!(expanded.ends_with("/test"));
    }

    #[test]
    fn test_expand_env_no_vars() {
        let result = Config::expand_env("/plain/path").unwrap();
        assert_eq!(result, "/plain/path");
    }

    #[test]
    fn test_expand_path_tilde() {
        let home = env::var("HOME").unwrap();
        let expanded = Config::expand_path("~/test").unwrap();
        let expected = PathBuf::from(home).join("test");
        assert_eq!(expanded, expected);
    }

    #[test]
    fn test_expand_path_with_env_var() {
        let home = env::var("HOME").unwrap();
        let expanded = Config::expand_path("$HOME/test").unwrap();
        let expected = PathBuf::from(home).join("test");
        assert_eq!(expanded, expected);
    }

    #[test]
    fn test_expand_path_absolute() {
        let expanded = Config::expand_path("/absolute/path").unwrap();
        assert_eq!(expanded, PathBuf::from("/absolute/path"));
    }

    #[test]
    fn test_config_with_empty_binary_dirs() {
        let toml_str = r#"
            binary_dirs = []
        "#;

        let config: Config = toml::from_str(toml_str).unwrap();
        assert!(config.binary_dirs.is_empty());
    }

    #[test]
    fn test_config_with_custom_mounts() {
        let toml_str = r#"
            [[mounts]]
            source = "/custom/source"
            target = "/custom/target"
            readonly = false
        "#;

        let config: Config = toml::from_str(toml_str).unwrap();
        assert_eq!(config.mounts.len(), 1);
        assert_eq!(config.mounts[0].source, "/custom/source");
        assert_eq!(config.mounts[0].target, "/custom/target");
        assert!(!config.mounts[0].readonly);
    }

    #[test]
    fn test_config_with_env_vars() {
        let toml_str = r#"
            [env]
            VAR1 = "value1"
            VAR2 = "value2"
        "#;

        let config: Config = toml::from_str(toml_str).unwrap();
        assert_eq!(config.env.len(), 2);
        assert_eq!(config.env.get("VAR1"), Some(&"value1".to_string()));
        assert_eq!(config.env.get("VAR2"), Some(&"value2".to_string()));
    }
}
