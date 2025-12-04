//! LLM client wrapper for gc
//!
//! Provides a simplified interface to the llm-client crate.

use anyhow::{Context, Result};
use llm_client::{Config, LlmError, LlmProvider, LlmRequest, get_provider};
use std::time::Duration;

/// Constants for retry logic
const MAX_RETRIES: u32 = 3;
const INITIAL_BACKOFF_MS: u64 = 1000;
const FALLBACK_PRESET: &str = "claude-cli";

/// Wrapper around LLM providers for gc
pub struct LlmClient {
    provider: Box<dyn LlmProvider>,
    config: Config,
    preset_name: String,
    debug: bool,
}

impl LlmClient {
    /// Create a new LLM client
    ///
    /// If preset_name is None, uses the default preset from config.
    pub fn new(preset_name: Option<&str>, debug: bool) -> Result<Self> {
        let config = Config::load().context("Failed to load LLM configuration")?;

        let preset_name = preset_name
            .map(String::from)
            .unwrap_or_else(|| config.get_default_for_program("gc").to_string());
        let preset = config
            .get_preset(&preset_name)
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

        Ok(Self {
            provider,
            config,
            preset_name,
            debug,
        })
    }

    /// Send a completion request to the LLM with retry logic and fallback
    pub async fn complete(&self, prompt: &str, system_prompt: &str) -> Result<String> {
        let request = LlmRequest {
            prompt: prompt.to_string(),
            system_prompt: Some(system_prompt.to_string()),
            max_tokens: None,
            temperature: None,
        };

        if self.debug {
            eprintln!("Sending request to {}", self.provider.name());
        }

        // Try with exponential backoff
        let mut last_error = None;
        for attempt in 0..MAX_RETRIES {
            match self.provider.complete(request.clone()).await {
                Ok(response) => {
                    if self.debug {
                        if let Some(usage) = &response.usage {
                            eprintln!(
                                "Tokens: {} in, {} out",
                                usage.input_tokens, usage.output_tokens
                            );
                        }
                    }
                    return Ok(response.content);
                }
                Err(LlmError::ServerOverloaded { ref message }) => {
                    last_error = Some(format!("Server overloaded: {}", message));
                    if attempt < MAX_RETRIES - 1 {
                        let backoff = Duration::from_millis(INITIAL_BACKOFF_MS * 2u64.pow(attempt));
                        if self.debug {
                            eprintln!(
                                "Server overloaded (attempt {}/{}), retrying in {:?}...",
                                attempt + 1,
                                MAX_RETRIES,
                                backoff
                            );
                        }
                        tokio::time::sleep(backoff).await;
                    }
                }
                Err(e) => {
                    // Non-retryable error, bail out immediately
                    return Err(e).context("LLM request failed");
                }
            }
        }

        // All retries exhausted, try fallback if different provider
        if self.preset_name != FALLBACK_PRESET && self.can_fallback() {
            if self.debug {
                eprintln!(
                    "Retries exhausted, falling back to {} provider...",
                    FALLBACK_PRESET
                );
            }
            return self.complete_with_fallback(&request).await;
        }

        // No fallback available or already using fallback
        anyhow::bail!(
            "LLM request failed after {} retries: {}",
            MAX_RETRIES,
            last_error.unwrap_or_else(|| "Unknown error".to_string())
        )
    }

    /// Check if we can create a fallback provider
    fn can_fallback(&self) -> bool {
        self.config.get_preset(FALLBACK_PRESET).is_ok()
    }

    /// Try to complete using the fallback provider
    async fn complete_with_fallback(&self, request: &LlmRequest) -> Result<String> {
        let preset = self
            .config
            .get_preset(FALLBACK_PRESET)
            .context("Fallback preset not available")?;

        let provider_config = self.config.get_provider_config(&preset.provider);
        let fallback_provider = get_provider(preset, provider_config).context(format!(
            "Failed to initialize fallback provider '{}'",
            FALLBACK_PRESET
        ))?;

        if self.debug {
            eprintln!("Using fallback provider: {}", fallback_provider.name());
        }

        let response = fallback_provider
            .complete(request.clone())
            .await
            .context("Fallback LLM request failed")?;

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

    /// Get the provider name
    pub fn provider_name(&self) -> &'static str {
        self.provider.name()
    }
}
