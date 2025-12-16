// TTS backend trait and types

pub mod macos_say;

use anyhow::Result;
use async_trait::async_trait;
use std::path::Path;

/// Represents an available voice
#[derive(Debug, Clone)]
pub struct Voice {
    /// Voice identifier
    pub id: String,
    /// Human-readable name
    pub name: String,
    /// Language/locale (e.g., "en_US")
    pub language: Option<String>,
}

/// Options for TTS synthesis
#[derive(Debug, Clone, Default)]
pub struct TtsOptions {
    /// Speaking rate (words per minute)
    pub rate: Option<u32>,
    /// Voice to use
    pub voice: Option<String>,
}

/// TTS backend trait - all TTS engines implement this
#[async_trait]
pub trait TtsBackend: Send + Sync {
    /// Synthesize text to audio file
    async fn synthesize(&self, text: &str, output_path: &Path, options: &TtsOptions) -> Result<()>;

    /// List available voices
    fn list_voices(&self) -> Result<Vec<Voice>>;

    /// Backend name
    fn name(&self) -> &str;
}

/// Create a TTS backend by name
pub fn create_backend(name: &str) -> Result<Box<dyn TtsBackend>> {
    match name {
        "macos-say" => Ok(Box::new(macos_say::MacOsSayBackend::new())),
        _ => anyhow::bail!("Unknown TTS backend: {}. Available: macos-say", name),
    }
}
