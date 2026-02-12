//! Bedrock to Polaris response conversions.

use super::types::document_to_json;
use aws_sdk_bedrockruntime::operation::converse::ConverseOutput;
use aws_sdk_bedrockruntime::types as bedrock;
use polaris_models::llm::{self as polaris_llm, GenerationError, GenerationResponse};

/// Converts a Bedrock converse response to a Polaris generation response.
pub fn convert_response(response: ConverseOutput) -> Result<GenerationResponse, GenerationError> {
    let content = match response.output {
        Some(bedrock::ConverseOutput::Message(msg)) => msg
            .content
            .into_iter()
            .map(convert_content_block)
            .collect::<Result<Vec<_>, _>>()?,
        Some(unexpected) => {
            return Err(GenerationError::InvalidResponse(format!(
                "unexpected output type {unexpected:?} from Bedrock"
            )));
        }
        None => Vec::new(),
    };

    let usage = convert_usage(response.usage);

    Ok(GenerationResponse { content, usage })
}

/// Converts a Bedrock content block to a Polaris assistant block.
fn convert_content_block(
    block: bedrock::ContentBlock,
) -> Result<polaris_llm::AssistantBlock, GenerationError> {
    match block {
        bedrock::ContentBlock::Text(text) => Ok(polaris_llm::AssistantBlock::Text(text)),
        bedrock::ContentBlock::ToolUse(tool_use) => Ok(convert_tool_use(tool_use)),
        other => Err(GenerationError::InvalidResponse(format!(
            "unsupported Bedrock content block type: {other:?}"
        ))),
    }
}

/// Converts a Bedrock tool use block to a Polaris tool call.
fn convert_tool_use(tool_use: bedrock::ToolUseBlock) -> polaris_llm::AssistantBlock {
    polaris_llm::AssistantBlock::ToolCall(polaris_llm::ToolCall {
        id: tool_use.tool_use_id,
        call_id: None,
        function: polaris_llm::ToolFunction {
            name: tool_use.name,
            arguments: document_to_json(&tool_use.input),
        },
        signature: None,
        additional_params: None,
    })
}

/// Converts Bedrock token usage to Polaris usage.
fn convert_usage(usage: Option<bedrock::TokenUsage>) -> polaris_llm::Usage {
    usage.map_or_else(polaris_llm::Usage::default, |u| polaris_llm::Usage {
        input_tokens: Some(u.input_tokens as u64),
        output_tokens: Some(u.output_tokens as u64),
        total_tokens: Some(u.total_tokens as u64),
    })
}
