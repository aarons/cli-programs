//! OpenAI-compatible API provider
//!
//! Used for providers that implement the OpenAI chat completions API:
//! - OpenRouter
//! - Cerebras
//! - LM Studio (with multimodal support)
//! - And others

use async_trait::async_trait;
use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
use reqwest::Client;
use serde::{Deserialize, Serialize};

use crate::error::{LlmError, Result};
use crate::provider::{FileAttachment, LlmProvider, LlmRequest, LlmResponse, TokenUsage};

/// Provider for OpenAI-compatible APIs
pub struct OpenAICompatibleProvider {
    model: String,
    base_url: String,
    api_key: Option<String>,
    name: &'static str,
    client: Client,
}

impl OpenAICompatibleProvider {
    /// Create a new OpenAI-compatible provider
    pub fn new(
        model: &str,
        base_url: &str,
        api_key: Option<String>,
        name: &'static str,
    ) -> Result<Self> {
        let client = Client::new();

        Ok(Self {
            model: model.to_string(),
            base_url: base_url.trim_end_matches('/').to_string(),
            api_key,
            name,
            client,
        })
    }

    /// Create an OpenRouter provider
    pub fn openrouter(model: &str, api_key: String) -> Result<Self> {
        Self::new(
            model,
            "https://openrouter.ai/api/v1",
            Some(api_key),
            "OpenRouter",
        )
    }

    /// Create a Cerebras provider
    pub fn cerebras(model: &str, api_key: String) -> Result<Self> {
        Self::new(
            model,
            "https://api.cerebras.ai/v1",
            Some(api_key),
            "Cerebras",
        )
    }

    /// Create an LM Studio provider (local, no API key required)
    pub fn lm_studio(model: &str, base_url: Option<&str>) -> Result<Self> {
        let url = base_url.unwrap_or("http://127.0.0.1:1234/v1");
        Self::new(model, url, None, "LM Studio")
    }
}

// OpenAI API request/response types

#[derive(Debug, Serialize)]
struct ChatCompletionRequest {
    model: String,
    messages: Vec<Message>,
    #[serde(skip_serializing_if = "Option::is_none")]
    response_format: Option<ResponseFormat>,
}

/// Response format for structured output
#[derive(Debug, Serialize)]
struct ResponseFormat {
    #[serde(rename = "type")]
    format_type: String,
    json_schema: JsonSchemaWrapper,
}

/// Wrapper for JSON schema in response_format
#[derive(Debug, Serialize)]
struct JsonSchemaWrapper {
    name: String,
    strict: bool,
    schema: serde_json::Value,
}

#[derive(Debug, Serialize)]
struct Message {
    role: String,
    content: MessageContent,
}

/// Message content - either a simple string or multimodal array
#[derive(Debug, Serialize)]
#[serde(untagged)]
enum MessageContent {
    Text(String),
    Multimodal(Vec<ContentPart>),
}

/// A part of multimodal content
#[derive(Debug, Serialize)]
#[serde(tag = "type")]
enum ContentPart {
    #[serde(rename = "text")]
    Text { text: String },
    #[serde(rename = "image_url")]
    ImageUrl { image_url: ImageUrl },
    #[serde(rename = "input_audio")]
    InputAudio { input_audio: InputAudioData },
}

#[derive(Debug, Serialize)]
struct ImageUrl {
    url: String,
}

#[derive(Debug, Serialize)]
struct InputAudioData {
    data: String,
    format: String,
}

#[derive(Debug, Deserialize)]
struct ChatCompletionResponse {
    choices: Vec<Choice>,
    usage: Option<Usage>,
}

#[derive(Debug, Deserialize)]
struct Choice {
    message: ResponseMessage,
}

#[derive(Debug, Deserialize)]
struct ResponseMessage {
    content: String,
}

#[derive(Debug, Deserialize)]
struct Usage {
    prompt_tokens: u32,
    completion_tokens: u32,
}

#[derive(Debug, Deserialize)]
struct ErrorResponse {
    error: ApiError,
}

#[derive(Debug, Deserialize)]
struct ApiError {
    message: String,
}

/// Check if a MIME type is an audio type
fn is_audio_mime_type(mime_type: &str) -> bool {
    mime_type.starts_with("audio/")
}

/// Get the audio format string from MIME type (for OpenAI input_audio)
fn audio_format_from_mime(mime_type: &str) -> &str {
    match mime_type {
        "audio/wav" | "audio/wave" | "audio/x-wav" => "wav",
        "audio/mpeg" | "audio/mp3" => "mp3",
        "audio/flac" => "flac",
        "audio/ogg" => "ogg",
        "audio/mp4" | "audio/m4a" => "m4a",
        "audio/webm" => "webm",
        // Default to wav for unknown audio types
        _ => "wav",
    }
}

/// Build message content, either as text or multimodal
fn build_user_content(prompt: &str, files: &[FileAttachment]) -> MessageContent {
    if files.is_empty() {
        MessageContent::Text(prompt.to_string())
    } else {
        let mut parts = Vec::new();

        // Add text prompt first
        if !prompt.is_empty() {
            parts.push(ContentPart::Text {
                text: prompt.to_string(),
            });
        }

        // Add file attachments with appropriate content type
        for file in files {
            let base64_data = BASE64.encode(&file.data);

            if is_audio_mime_type(&file.mime_type) {
                // Audio files use input_audio content type
                let format = audio_format_from_mime(&file.mime_type);
                parts.push(ContentPart::InputAudio {
                    input_audio: InputAudioData {
                        data: base64_data,
                        format: format.to_string(),
                    },
                });
            } else {
                // Images and other files use image_url with data URL
                let data_url = format!("data:{};base64,{}", file.mime_type, base64_data);
                parts.push(ContentPart::ImageUrl {
                    image_url: ImageUrl { url: data_url },
                });
            }
        }

        MessageContent::Multimodal(parts)
    }
}

#[async_trait]
impl LlmProvider for OpenAICompatibleProvider {
    async fn complete(&self, request: LlmRequest) -> Result<LlmResponse> {
        let mut messages = Vec::new();

        if let Some(system) = &request.system_prompt {
            messages.push(Message {
                role: "system".to_string(),
                content: MessageContent::Text(system.clone()),
            });
        }

        messages.push(Message {
            role: "user".to_string(),
            content: build_user_content(&request.prompt, &request.files),
        });

        // Build response_format if json_schema is provided
        let response_format = request.json_schema.map(|schema| ResponseFormat {
            format_type: "json_schema".to_string(),
            json_schema: JsonSchemaWrapper {
                name: "response".to_string(),
                strict: true,
                schema,
            },
        });

        let chat_request = ChatCompletionRequest {
            model: self.model.clone(),
            messages,
            response_format,
        };

        let url = format!("{}/chat/completions", self.base_url);

        let mut request_builder = self
            .client
            .post(&url)
            .header("Content-Type", "application/json");

        // Only add Authorization header if API key is provided
        if let Some(ref api_key) = self.api_key {
            request_builder = request_builder.header("Authorization", format!("Bearer {}", api_key));
        }

        let response = request_builder
            .json(&chat_request)
            .send()
            .await
            .map_err(|e| LlmError::ApiError {
                message: format!("Request failed: {}", e),
                status_code: None,
            })?;

        let status = response.status();
        if !status.is_success() {
            let error_text = response.text().await.unwrap_or_default();
            let message =
                if let Ok(error_response) = serde_json::from_str::<ErrorResponse>(&error_text) {
                    error_response.error.message
                } else {
                    error_text
                };

            // Handle 503 (server overloaded) separately for retry logic
            if status.as_u16() == 503 {
                return Err(LlmError::ServerOverloaded { message });
            }

            return Err(LlmError::ApiError {
                message,
                status_code: Some(status.as_u16()),
            });
        }

        let chat_response: ChatCompletionResponse =
            response.json().await.map_err(|e| LlmError::ApiError {
                message: format!("Failed to parse response: {}", e),
                status_code: None,
            })?;

        let content = chat_response
            .choices
            .first()
            .map(|c| c.message.content.clone())
            .unwrap_or_default();

        let usage = chat_response.usage.map(|u| TokenUsage {
            input_tokens: u.prompt_tokens,
            output_tokens: u.completion_tokens,
        });

        Ok(LlmResponse {
            content,
            model: self.model.clone(),
            usage,
        })
    }

    fn name(&self) -> &'static str {
        self.name
    }

    fn is_available(&self) -> Result<()> {
        // API key was provided in constructor
        Ok(())
    }
}
