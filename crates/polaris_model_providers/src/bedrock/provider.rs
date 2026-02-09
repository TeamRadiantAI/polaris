//! AWS Bedrock [`LlmProvider`] implementation.

use async_trait::async_trait;
use aws_sdk_bedrockruntime::Client;
use aws_sdk_bedrockruntime::types as bedrock;
use polaris_models::llm::{GenerationError, GenerationRequest, GenerationResponse, LlmProvider};
use std::sync::Arc;

use super::request::{build_output_config, build_tool_config, convert_message};
use super::response::convert_response;

/// AWS Bedrock [`LlmProvider`] implementation.
pub struct BedrockProvider {
    client: Arc<Client>,
}

impl BedrockProvider {
    /// Creates a new provider with an already-initialized client.
    #[must_use]
    pub fn new(client: Arc<Client>) -> Self {
        Self { client }
    }
}

#[async_trait]
impl LlmProvider for BedrockProvider {
    async fn generate(
        &self,
        model: &str,
        request: GenerationRequest,
    ) -> Result<GenerationResponse, GenerationError> {
        let messages = request
            .messages
            .iter()
            .map(convert_message)
            .collect::<Result<Vec<_>, _>>()?;

        let system = request
            .system
            .as_ref()
            .map(|s| vec![bedrock::SystemContentBlock::Text(s.clone())]);

        let tool_config = build_tool_config(&request)?;
        let output_config = build_output_config(&request)?;

        let response = self
            .client
            .converse()
            .model_id(model)
            .set_messages(Some(messages))
            .set_system(system)
            .set_tool_config(tool_config)
            .set_output_config(output_config)
            .send()
            .await
            .map_err(|err| {
                let service_err = err.into_service_error();
                GenerationError::Provider {
                    status: None,
                    message: service_err.to_string(),
                    source: Some(Box::new(service_err)),
                }
            })?;

        convert_response(response)
    }
}
