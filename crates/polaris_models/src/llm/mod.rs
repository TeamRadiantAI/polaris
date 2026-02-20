//! LLM (Large Language Model) generation capabilities.
//!
//! This module provides the core traits and types for text generation
//! with LLMs, including support for:
//!
//! - Text generation with tool calling
//! - Structured outputs
//! - Multi-modal inputs (images, audio, documents)

mod error;
mod model;
mod provider;
mod types;

pub use error::{ExtractionError, GenerationError};
pub use model::Llm;
pub use provider::LlmProvider;
pub use types::{
    AssistantBlock, AudioBlock, AudioMediaType, DocumentBlock, DocumentMediaType, DocumentSource,
    GenerationRequest, GenerationResponse, ImageBlock, ImageMediaType, Message, ReasoningBlock,
    TextBlock, ToolCall, ToolChoice, ToolDefinition, ToolFunction, ToolResult, ToolResultContent,
    ToolResultStatus, Usage, UserBlock,
};
