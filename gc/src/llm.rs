//! LLM client wrapper for gc
//!
//! Provides a simplified interface to the llm-client crate with
//! automatic fallback support.

use anyhow::{Context, Result};
use llm_client::{Config, FallbackProvider, LlmError, LlmProvider, LlmRequest, get_provider_with_fallback};
use std::time::Duration;

/// Constants for retry logic
const MAX_RETRIES: u32 = 3;
const INITIAL_BACKOFF_MS: u64 = 1000;

/// Wrapper around LLM providers for gc
pub struct LlmClient {
    provider: FallbackProvider,
    debug: bool,
}

impl LlmClient {
    /// Create a new LLM client with fallback chain support
    ///
    /// If preset_name is None, uses the default preset from config.
    /// The fallback chain is automatically built from the preset's `fallback` field.
    pub fn new(preset_name: Option<&str>, debug: bool) -> Result<Self> {
        let config = Config::load().context("Failed to load LLM configuration")?;

        let preset_name = preset_name
            .map(String::from)
            .unwrap_or_else(|| config.get_default_for_program("gc").to_string());

        let provider = get_provider_with_fallback(&config, &preset_name)
            .context(format!("Failed to initialize provider chain for preset '{}'", preset_name))?
            .with_debug(debug);

        if debug {
            eprintln!(
                "Using LLM provider: {} (chain length: {})",
                provider.primary_name(),
                provider.chain_len()
            );
        }

        Ok(Self { provider, debug })
    }

    /// Send a completion request to the LLM with retry logic
    ///
    /// On server overload (503), retries with exponential backoff.
    /// On other errors, the fallback chain (configured via preset) is tried.
    pub async fn complete(&self, prompt: &str, system_prompt: &str) -> Result<String> {
        let request = LlmRequest {
            prompt: prompt.to_string(),
            system_prompt: Some(system_prompt.to_string()),
            max_tokens: None,
            temperature: None,
            files: vec![],
            json_schema: None,
        };

        if self.debug {
            eprintln!("Sending request to {}", self.provider.name());
        }

        // Try with exponential backoff for 503 errors
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
                    // Non-retryable error (fallback already attempted by FallbackProvider)
                    return Err(e).context("LLM request failed");
                }
            }
        }

        // All retries exhausted
        anyhow::bail!(
            "LLM request failed after {} retries: {}",
            MAX_RETRIES,
            last_error.unwrap_or_else(|| "Unknown error".to_string())
        )
    }

    /// Get the provider name
    pub fn provider_name(&self) -> &'static str {
        self.provider.name()
    }

    /// Create an LlmClient with an injected provider (for testing)
    #[cfg(test)]
    pub fn with_provider(provider: FallbackProvider) -> Self {
        Self {
            provider,
            debug: false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use llm_client::MockProvider;

    #[tokio::test]
    async fn retries_on_server_overloaded_then_succeeds() {
        // Provider fails twice with 503, then succeeds on third attempt
        let provider = MockProvider::fails_then_succeeds(
            2,
            LlmError::ServerOverloaded {
                message: "server busy".to_string(),
            },
            "feat: add tests",
        );

        // Create a fallback chain with just this provider
        let chain = vec![(
            "test".to_string(),
            Box::new(provider) as Box<dyn LlmProvider>,
        )];
        let fallback = FallbackProvider::from_chain(chain);
        let client = LlmClient::with_provider(fallback);

        // Retry logic should handle 2 failures and succeed on the 3rd attempt
        let result = client.complete("prompt", "system").await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "feat: add tests");
    }

    #[tokio::test]
    async fn no_retry_on_non_retryable_errors() {
        // Provider fails with a non-retryable error (missing API key)
        let provider = MockProvider::always_fails(LlmError::MissingApiKey {
            provider: "test".to_string(),
            env_var: "TEST_API_KEY".to_string(),
        });

        let chain = vec![(
            "test".to_string(),
            Box::new(provider) as Box<dyn LlmProvider>,
        )];
        let fallback = FallbackProvider::from_chain(chain);
        let client = LlmClient::with_provider(fallback);

        let result = client.complete("prompt", "system").await;
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        // The error is wrapped with "LLM request failed" context from complete()
        assert!(err.contains("LLM request failed"));
    }

    #[tokio::test]
    async fn exhausts_retries_on_server_overload() {
        // Provider always fails with 503
        let provider = MockProvider::always_fails(LlmError::ServerOverloaded {
            message: "server busy".to_string(),
        });

        let chain = vec![(
            "test".to_string(),
            Box::new(provider) as Box<dyn LlmProvider>,
        )];
        let fallback = FallbackProvider::from_chain(chain);
        let client = LlmClient::with_provider(fallback);

        let result = client.complete("prompt", "system").await;
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("failed after 3 retries"));
    }
}
