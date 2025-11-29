//! LLM client wrapper for ask
//!
//! Provides a simplified interface to the llm-client crate.

use anyhow::{Context, Result};
use llm_client::{get_provider, Config, LlmProvider, LlmRequest};

/// Wrapper around LLM providers for ask
pub struct LlmClient {
    provider: Box<dyn LlmProvider>,
    debug: bool,
}

impl LlmClient {
    /// Create a new LLM client
    ///
    /// If preset_name is None, uses the default preset from config.
    pub fn new(preset_name: Option<&str>, debug: bool) -> Result<Self> {
        let config = Config::load().context("Failed to load LLM configuration")?;

        let preset_name = preset_name.unwrap_or(&config.default_preset);
        let preset = config
            .get_preset(preset_name)
            .context(format!("Unknown preset: {}", preset_name))?;

        let provider_config = config.get_provider_config(&preset.provider);
        let provider = get_provider(preset, provider_config).context(format!(
            "Failed to initialize provider '{}' for preset '{}'",
            preset.provider, preset_name
        ))?;

        if debug {
            eprintln!(
                "Using LLM provider: {} (model: {})",
                provider.name(),
                preset.model
            );
        }

        Ok(Self { provider, debug })
    }

    /// Send a completion request to the LLM
    ///
    /// System prompt is optional - used in shell mode, not in general mode.
    pub async fn complete(&self, prompt: &str, system_prompt: Option<&str>) -> Result<String> {
        let request = LlmRequest {
            prompt: prompt.to_string(),
            system_prompt: system_prompt.map(String::from),
            max_tokens: None,
            temperature: None,
        };

        if self.debug {
            eprintln!("Sending request to {}", self.provider.name());
        }

        let response = self
            .provider
            .complete(request)
            .await
            .context("LLM request failed")?;

        if self.debug {
            if let Some(usage) = &response.usage {
                eprintln!(
                    "Tokens: {} in, {} out",
                    usage.input_tokens, usage.output_tokens
                );
            }
        }

        Ok(response.content)
    }
}
