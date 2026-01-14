//! LLM client wrapper for ask
//!
//! Provides a simplified interface to the llm-client crate.

use anyhow::{Context, Result};
use llm_client::{Config, FileAttachment, LlmProvider, LlmRequest, get_provider};
use serde_json::Value;
use std::path::Path;

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

        let preset_name = preset_name.unwrap_or_else(|| config.get_default_for_program("ask"));
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
    /// Files are optional - used for multimodal requests (e.g., audio/image analysis).
    /// json_schema is optional - used for structured output with OpenAI-compatible providers.
    pub async fn complete(
        &self,
        prompt: &str,
        system_prompt: Option<&str>,
        file_paths: &[impl AsRef<Path>],
        json_schema: Option<Value>,
    ) -> Result<String> {
        // Load files and determine MIME types
        let files = file_paths
            .iter()
            .map(|path| load_file_attachment(path.as_ref()))
            .collect::<Result<Vec<_>>>()?;

        let request = LlmRequest {
            prompt: prompt.to_string(),
            system_prompt: system_prompt.map(String::from),
            max_tokens: None,
            temperature: None,
            files,
            json_schema,
        };

        if self.debug {
            eprintln!("Sending request to {}", self.provider.name());
            if !request.files.is_empty() {
                eprintln!("  with {} file attachment(s)", request.files.len());
            }
            if request.json_schema.is_some() {
                eprintln!("  with JSON schema for structured output");
            }
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

/// Load a file and determine its MIME type
fn load_file_attachment(path: &Path) -> Result<FileAttachment> {
    let data = std::fs::read(path)
        .with_context(|| format!("Failed to read file: {}", path.display()))?;

    let mime_type = mime_type_from_extension(path);

    Ok(FileAttachment { data, mime_type })
}

/// Determine MIME type from file extension
fn mime_type_from_extension(path: &Path) -> String {
    let extension = path
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_lowercase());

    match extension.as_deref() {
        Some("wav") => "audio/wav".to_string(),
        Some("mp3") => "audio/mpeg".to_string(),
        Some("ogg") => "audio/ogg".to_string(),
        Some("flac") => "audio/flac".to_string(),
        Some("m4a") => "audio/mp4".to_string(),
        Some("png") => "image/png".to_string(),
        Some("jpg") | Some("jpeg") => "image/jpeg".to_string(),
        Some("gif") => "image/gif".to_string(),
        Some("webp") => "image/webp".to_string(),
        Some("pdf") => "application/pdf".to_string(),
        _ => "application/octet-stream".to_string(),
    }
}
