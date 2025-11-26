use async_trait::async_trait;
use std::path::PathBuf;
use tokio::process::Command;

use crate::error::{LlmError, Result};
use crate::provider::{LlmProvider, LlmRequest, LlmResponse};

/// Provider that uses the Claude CLI (subprocess)
pub struct ClaudeCliProvider {
    model: String,
    cli_path: PathBuf,
}

impl ClaudeCliProvider {
    /// Create a new Claude CLI provider
    pub fn new(model: &str, cli_path: Option<PathBuf>) -> Result<Self> {
        let cli_path = cli_path.unwrap_or_else(|| {
            // Try to find claude in PATH
            which::which("claude").unwrap_or_else(|_| PathBuf::from("claude"))
        });

        Ok(Self {
            model: model.to_string(),
            cli_path,
        })
    }

    fn cli_exists(&self) -> bool {
        self.cli_path.exists() || which::which("claude").is_ok()
    }
}

#[async_trait]
impl LlmProvider for ClaudeCliProvider {
    async fn complete(&self, request: LlmRequest) -> Result<LlmResponse> {
        let mut cmd = Command::new(&self.cli_path);

        cmd.args(["--model", &self.model]);

        if let Some(system) = &request.system_prompt {
            cmd.args(["--system-prompt", system]);
        }

        cmd.args(["--print", &request.prompt]);

        let output = cmd
            .output()
            .await
            .map_err(|e| LlmError::ClaudeCliError(format!("Failed to execute: {}", e)))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(LlmError::ClaudeCliError(format!(
                "Command failed: {}",
                stderr
            )));
        }

        let content = String::from_utf8(output.stdout)
            .map_err(|e| LlmError::ClaudeCliError(format!("Invalid UTF-8: {}", e)))?
            .trim()
            .to_string();

        Ok(LlmResponse {
            content,
            model: self.model.clone(),
            usage: None,
        })
    }

    fn name(&self) -> &'static str {
        "Claude CLI"
    }

    fn is_available(&self) -> Result<()> {
        if self.cli_exists() {
            Ok(())
        } else {
            Err(LlmError::ProviderUnavailable(
                "Claude CLI not found. Install from https://docs.anthropic.com/en/docs/claude-code"
                    .into(),
            ))
        }
    }
}
