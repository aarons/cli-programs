use async_trait::async_trait;
use genai::chat::{ChatMessage, ChatRequest};
use genai::resolver::{AuthData, AuthResolver};
use genai::Client;

use crate::error::{LlmError, Result};
use crate::provider::{LlmProvider, LlmRequest, LlmResponse, TokenUsage};

/// Provider for direct Anthropic API calls
pub struct AnthropicProvider {
    model: String,
    client: Client,
}

impl AnthropicProvider {
    /// Create a new Anthropic provider
    pub fn new(model: &str, api_key: String) -> Result<Self> {
        // Create auth resolver with the API key
        let auth_resolver = AuthResolver::from_resolver_fn(move |_model_iden| {
            // Return Some(AuthData) with the API key as bearer token
            Ok(Some(AuthData::from_single(api_key.clone())))
        });

        let client = Client::builder().with_auth_resolver(auth_resolver).build();

        Ok(Self {
            model: model.to_string(),
            client,
        })
    }
}

#[async_trait]
impl LlmProvider for AnthropicProvider {
    async fn complete(&self, request: LlmRequest) -> Result<LlmResponse> {
        let mut messages = Vec::new();

        if let Some(system) = &request.system_prompt {
            messages.push(ChatMessage::system(system));
        }

        messages.push(ChatMessage::user(&request.prompt));

        let chat_req = ChatRequest::new(messages);

        let chat_res = self
            .client
            .exec_chat(&self.model, chat_req, None)
            .await
            .map_err(|e| LlmError::ApiError {
                message: e.to_string(),
                status_code: None,
            })?;

        let content = chat_res.first_text().unwrap_or("").to_string();

        let usage = {
            let u = &chat_res.usage;
            if u.prompt_tokens.is_some() || u.completion_tokens.is_some() {
                Some(TokenUsage {
                    input_tokens: u.prompt_tokens.unwrap_or(0) as u32,
                    output_tokens: u.completion_tokens.unwrap_or(0) as u32,
                })
            } else {
                None
            }
        };

        Ok(LlmResponse {
            content,
            model: self.model.clone(),
            usage,
        })
    }

    fn name(&self) -> &'static str {
        "Anthropic API"
    }

    fn is_available(&self) -> Result<()> {
        // Client was created with API key in constructor
        Ok(())
    }
}
