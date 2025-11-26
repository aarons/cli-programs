use async_trait::async_trait;
use genai::adapter::AdapterKind;
use genai::chat::{ChatMessage, ChatRequest};
use genai::resolver::{AuthData, Endpoint, ServiceTargetResolver};
use genai::{Client, ModelIden, ServiceTarget};

use crate::error::{LlmError, Result};
use crate::provider::{LlmProvider, LlmRequest, LlmResponse, TokenUsage};

/// Provider for Cerebras API (fast Llama inference)
pub struct CerebrasProvider {
    model: String,
    client: Client,
}

impl CerebrasProvider {
    /// Create a new Cerebras provider
    pub fn new(model: &str, api_key: String) -> Result<Self> {
        let model_for_resolver = model.to_string();

        // Cerebras uses OpenAI-compatible API
        let target_resolver =
            ServiceTargetResolver::from_resolver_fn(move |_service_target: ServiceTarget| {
                let endpoint = Endpoint::from_static("https://api.cerebras.ai/v1/");
                let auth = AuthData::from_single(api_key.clone());
                let model = ModelIden::new(AdapterKind::OpenAI, &model_for_resolver);

                Ok(ServiceTarget {
                    endpoint,
                    auth,
                    model,
                })
            });

        let client = Client::builder()
            .with_service_target_resolver(target_resolver)
            .build();

        Ok(Self {
            model: model.to_string(),
            client,
        })
    }
}

#[async_trait]
impl LlmProvider for CerebrasProvider {
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
        "Cerebras"
    }

    fn is_available(&self) -> Result<()> {
        // Client was created with API key in constructor
        Ok(())
    }
}
