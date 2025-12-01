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
    /// Directory where worktrees are created
    #[serde(default = "default_worktree_dir")]
    pub worktree_dir: String,

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

fn default_worktree_dir() -> String {
    "~/worktrees".to_string()
}

fn default_binary_dirs() -> Vec<String> {
    vec!["~/.local/bin".to_string()]
}

impl Default for Config {
    fn default() -> Self {
        Self {
            worktree_dir: default_worktree_dir(),
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
            ],
        }
    }
}

impl Config {
    /// Get the config directory path
    pub fn config_dir() -> Result<PathBuf> {
        let home = dirs::home_dir().context("Could not determine home directory")?;
        Ok(home.join(".config").join("cli-programs"))
    }

    /// Get the config file path
    pub fn config_path() -> Result<PathBuf> {
        Ok(Self::config_dir()?.join("sandbox.toml"))
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

    /// Get the expanded worktree directory path
    pub fn worktree_dir_expanded(&self) -> Result<PathBuf> {
        let expanded = shellexpand::tilde(&self.worktree_dir);
        Ok(PathBuf::from(expanded.as_ref()))
    }

    /// Expand environment variables in a string value
    pub fn expand_env(value: &str) -> Result<String> {
        let expanded = shellexpand::env(value)
            .with_context(|| format!("Failed to expand environment variables in: {}", value))?;
        Ok(expanded.to_string())
    }

    /// Expand a path (tilde and env vars)
    pub fn expand_path(path: &str) -> Result<PathBuf> {
        let expanded = shellexpand::full(path)
            .with_context(|| format!("Failed to expand path: {}", path))?;
        Ok(PathBuf::from(expanded.as_ref()))
    }
}
