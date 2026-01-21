//! Fallback provider chain support
//!
//! Enables configuring multiple providers in a fallback chain,
//! where if one provider fails, the next one is tried.

use async_trait::async_trait;
use std::collections::HashSet;

use crate::config::Config;
use crate::error::{LlmError, Result};
use crate::provider::{LlmProvider, LlmRequest, LlmResponse};
use crate::providers::get_provider;

/// A provider that wraps a chain of fallback providers.
///
/// When a request fails on the primary provider, it automatically
/// tries the next provider in the chain until one succeeds or
/// all providers have been exhausted.
pub struct FallbackProvider {
    /// Chain of (preset_name, provider) pairs
    chain: Vec<(String, Box<dyn LlmProvider>)>,
    /// Whether to print debug info
    debug: bool,
}

impl std::fmt::Debug for FallbackProvider {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FallbackProvider")
            .field("chain_len", &self.chain.len())
            .field("preset_names", &self.chain.iter().map(|(n, _)| n.as_str()).collect::<Vec<_>>())
            .field("debug", &self.debug)
            .finish()
    }
}

impl FallbackProvider {
    /// Create a new FallbackProvider with the given chain
    fn new(chain: Vec<(String, Box<dyn LlmProvider>)>) -> Self {
        Self { chain, debug: false }
    }

    /// Create a FallbackProvider directly from a chain of providers.
    ///
    /// This is primarily useful for testing. For normal use, prefer
    /// `get_provider_with_fallback()` which builds the chain from config.
    pub fn from_chain(chain: Vec<(String, Box<dyn LlmProvider>)>) -> Self {
        Self::new(chain)
    }

    /// Enable debug output
    pub fn with_debug(mut self, debug: bool) -> Self {
        self.debug = debug;
        self
    }

    /// Get the name of the primary provider
    pub fn primary_name(&self) -> &str {
        self.chain
            .first()
            .map(|(name, _)| name.as_str())
            .unwrap_or("unknown")
    }

    /// Get the number of providers in the chain
    pub fn chain_len(&self) -> usize {
        self.chain.len()
    }
}

#[async_trait]
impl LlmProvider for FallbackProvider {
    async fn complete(&self, request: LlmRequest) -> Result<LlmResponse> {
        let mut last_error = None;

        for (i, (preset_name, provider)) in self.chain.iter().enumerate() {
            match provider.complete(request.clone()).await {
                Ok(response) => return Ok(response),
                Err(e) => {
                    if self.debug {
                        eprintln!(
                            "Provider '{}' failed: {}",
                            preset_name, e
                        );
                    }

                    // If there's a next provider, log and continue
                    if i + 1 < self.chain.len() {
                        if self.debug {
                            let next_name = &self.chain[i + 1].0;
                            eprintln!("Falling back to '{}'...", next_name);
                        }
                        last_error = Some(e);
                        continue;
                    } else {
                        // Last provider in chain, return the error
                        return Err(e);
                    }
                }
            }
        }

        // Should only reach here if chain is empty
        Err(last_error.unwrap_or_else(|| {
            LlmError::ProviderUnavailable("No providers in fallback chain".to_string())
        }))
    }

    fn name(&self) -> &'static str {
        // Return the primary provider's name
        self.chain
            .first()
            .map(|(_, p)| p.name())
            .unwrap_or("FallbackProvider")
    }

    fn is_available(&self) -> Result<()> {
        // Check if at least one provider is available
        for (_, provider) in &self.chain {
            if provider.is_available().is_ok() {
                return Ok(());
            }
        }
        Err(LlmError::ProviderUnavailable(
            "No providers in fallback chain are available".to_string(),
        ))
    }
}

/// Create a provider with fallback chain from a preset name.
///
/// This function builds the entire fallback chain by following the
/// `fallback` field in each preset configuration. It detects cycles
/// to prevent infinite loops.
///
/// # Example config
/// ```toml
/// [presets.cerebras-free]
/// provider = "cerebras"
/// model = "llama-4-scout"
/// api_key_env = "CEREBRAS_API_KEY_FREE"
/// fallback = "cerebras-paid"
///
/// [presets.cerebras-paid]
/// provider = "cerebras"
/// model = "llama-4-scout"
/// # Uses default CEREBRAS_API_KEY
/// ```
pub fn get_provider_with_fallback(config: &Config, preset_name: &str) -> Result<FallbackProvider> {
    let mut chain = Vec::new();
    let mut seen = HashSet::new();
    let mut current_name = Some(preset_name.to_string());

    while let Some(name) = current_name.take() {
        // Cycle detection
        if seen.contains(&name) {
            return Err(LlmError::ConfigError(format!(
                "Circular fallback detected: preset '{}' appears twice in the chain",
                name
            )));
        }
        seen.insert(name.clone());

        // Get the preset
        let preset = config.get_preset(&name)?;
        let provider_config = config.get_provider_config(&preset.provider);

        // Create the provider, skipping if API key is missing
        match get_provider(preset, provider_config) {
            Ok(provider) => {
                chain.push((name.clone(), provider));
            }
            Err(LlmError::MissingApiKey { provider, env_var }) => {
                eprintln!(
                    "Warning: Skipping '{}' - {} API key not found ({})",
                    name, provider, env_var
                );
                // Continue to next preset in chain
            }
            Err(e) => {
                // Other errors (config errors, invalid provider, etc.) should still fail
                return Err(e);
            }
        }

        // Check for next in chain
        current_name = preset.fallback.clone();
    }

    if chain.is_empty() {
        return Err(LlmError::ConfigError(
            "No providers in fallback chain".to_string(),
        ));
    }

    Ok(FallbackProvider::new(chain))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::ModelPreset;
    use crate::providers::MockProvider;
    use std::collections::HashMap;

    fn test_config() -> Config {
        let mut presets = HashMap::new();

        presets.insert(
            "primary".to_string(),
            ModelPreset {
                provider: "claude-cli".to_string(),
                model: "sonnet".to_string(),
                fallback: Some("fallback1".to_string()),
                api_key_env: None,
            },
        );

        presets.insert(
            "fallback1".to_string(),
            ModelPreset {
                provider: "claude-cli".to_string(),
                model: "sonnet".to_string(),
                fallback: Some("fallback2".to_string()),
                api_key_env: None,
            },
        );

        presets.insert(
            "fallback2".to_string(),
            ModelPreset {
                provider: "claude-cli".to_string(),
                model: "sonnet".to_string(),
                fallback: None,
                api_key_env: None,
            },
        );

        Config {
            default_preset: "primary".to_string(),
            defaults: HashMap::new(),
            presets,
            providers: HashMap::new(),
        }
    }

    fn test_config_with_cycle() -> Config {
        let mut presets = HashMap::new();

        presets.insert(
            "a".to_string(),
            ModelPreset {
                provider: "claude-cli".to_string(),
                model: "sonnet".to_string(),
                fallback: Some("b".to_string()),
                api_key_env: None,
            },
        );

        presets.insert(
            "b".to_string(),
            ModelPreset {
                provider: "claude-cli".to_string(),
                model: "sonnet".to_string(),
                fallback: Some("a".to_string()), // Cycle back to 'a'
                api_key_env: None,
            },
        );

        Config {
            default_preset: "a".to_string(),
            defaults: HashMap::new(),
            presets,
            providers: HashMap::new(),
        }
    }

    #[test]
    fn test_cycle_detection() {
        let config = test_config_with_cycle();
        let result = get_provider_with_fallback(&config, "a");
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("Circular fallback"));
    }

    #[test]
    fn test_missing_fallback_preset() {
        let mut config = test_config();
        // Point to non-existent preset
        config.presets.get_mut("fallback2").unwrap().fallback =
            Some("nonexistent".to_string());

        let result = get_provider_with_fallback(&config, "primary");
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_fallback_provider_success_on_first() {
        let chain = vec![(
            "primary".to_string(),
            Box::new(MockProvider::always_succeeds("response")) as Box<dyn LlmProvider>,
        )];

        let provider = FallbackProvider::new(chain);
        let request = LlmRequest {
            prompt: "test".to_string(),
            system_prompt: None,
            max_tokens: None,
            temperature: None,
            files: vec![],
            json_schema: None,
        };

        let result = provider.complete(request).await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap().content, "response");
    }

    #[tokio::test]
    async fn test_fallback_provider_falls_back_on_error() {
        let chain = vec![
            (
                "primary".to_string(),
                Box::new(MockProvider::always_fails(LlmError::ApiError {
                    message: "failed".to_string(),
                    status_code: Some(500),
                })) as Box<dyn LlmProvider>,
            ),
            (
                "fallback".to_string(),
                Box::new(MockProvider::always_succeeds("fallback response")) as Box<dyn LlmProvider>,
            ),
        ];

        let provider = FallbackProvider::new(chain);
        let request = LlmRequest {
            prompt: "test".to_string(),
            system_prompt: None,
            max_tokens: None,
            temperature: None,
            files: vec![],
            json_schema: None,
        };

        let result = provider.complete(request).await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap().content, "fallback response");
    }

    #[test]
    fn test_skips_provider_with_missing_api_key() {
        // Use unique env var names that are unlikely to exist
        let mut presets = HashMap::new();

        // Primary preset uses anthropic which requires an API key
        // Use a unique env var name that won't exist
        presets.insert(
            "primary".to_string(),
            ModelPreset {
                provider: "anthropic".to_string(),
                model: "claude-sonnet-4-20250514".to_string(),
                fallback: Some("fallback".to_string()),
                api_key_env: Some("__LLM_CLIENT_TEST_NONEXISTENT_KEY_12345__".to_string()),
            },
        );

        // Fallback uses claude-cli which doesn't require an API key
        presets.insert(
            "fallback".to_string(),
            ModelPreset {
                provider: "claude-cli".to_string(),
                model: "sonnet".to_string(),
                fallback: None,
                api_key_env: None,
            },
        );

        let config = Config {
            default_preset: "primary".to_string(),
            defaults: HashMap::new(),
            presets,
            providers: HashMap::new(),
        };

        // Should succeed by skipping anthropic and using claude-cli
        let result = get_provider_with_fallback(&config, "primary");
        assert!(result.is_ok(), "Expected success but got: {:?}", result);

        let provider = result.unwrap();
        // Chain should only have the fallback provider
        assert_eq!(provider.chain_len(), 1);
        assert_eq!(provider.primary_name(), "fallback");
    }

    #[test]
    fn test_fails_when_all_providers_missing_api_keys() {
        // Use unique env var names that are unlikely to exist
        let mut presets = HashMap::new();

        presets.insert(
            "primary".to_string(),
            ModelPreset {
                provider: "anthropic".to_string(),
                model: "claude-sonnet-4-20250514".to_string(),
                fallback: Some("fallback".to_string()),
                api_key_env: Some("__LLM_CLIENT_TEST_NONEXISTENT_KEY_A__".to_string()),
            },
        );

        presets.insert(
            "fallback".to_string(),
            ModelPreset {
                provider: "cerebras".to_string(),
                model: "llama-4-scout".to_string(),
                fallback: None,
                api_key_env: Some("__LLM_CLIENT_TEST_NONEXISTENT_KEY_B__".to_string()),
            },
        );

        let config = Config {
            default_preset: "primary".to_string(),
            defaults: HashMap::new(),
            presets,
            providers: HashMap::new(),
        };

        // Should fail because all providers in chain are missing API keys
        let result = get_provider_with_fallback(&config, "primary");
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("No providers in fallback chain"));
    }

    #[tokio::test]
    async fn test_fallback_provider_all_fail() {
        let chain = vec![
            (
                "primary".to_string(),
                Box::new(MockProvider::always_fails(LlmError::ApiError {
                    message: "primary failed".to_string(),
                    status_code: Some(500),
                })) as Box<dyn LlmProvider>,
            ),
            (
                "fallback".to_string(),
                Box::new(MockProvider::always_fails(LlmError::ApiError {
                    message: "fallback failed".to_string(),
                    status_code: Some(500),
                })) as Box<dyn LlmProvider>,
            ),
        ];

        let provider = FallbackProvider::new(chain);
        let request = LlmRequest {
            prompt: "test".to_string(),
            system_prompt: None,
            max_tokens: None,
            temperature: None,
            files: vec![],
            json_schema: None,
        };

        let result = provider.complete(request).await;
        assert!(result.is_err());
        // Should contain the last error message
        let err = result.unwrap_err().to_string();
        assert!(err.contains("fallback failed"));
    }
}
