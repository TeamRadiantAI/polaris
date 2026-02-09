//! Core types for LLM generation requests and responses.

use serde::{Deserialize, Serialize};
use serde_json::Value;

// ─────────────────────
// Request / Response
// ─────────────────────

/// A generation request to a model.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GenerationRequest {
    /// System prompt for the model.
    pub system: Option<String>,
    /// The messages to send to the model.
    pub messages: Vec<Message>,
    /// Available tools the model can call.
    pub tools: Option<Vec<ToolDefinition>>,
    /// How the model should choose tools.
    pub tool_choice: Option<ToolChoice>,
    /// JSON Schema for structured output (optional).
    ///
    /// When provided, the model will generate output conforming to this schema.
    /// This is set automatically by `Llm::generate_structured()`.
    pub output_schema: Option<Value>,
}

impl GenerationRequest {
    /// Creates a new generation request with a user message.
    ///
    /// # Example
    ///
    /// ```rust
    /// use polaris_models::llm::GenerationRequest;
    ///
    /// let request = GenerationRequest::new("What's the weather like?");
    /// ```
    #[must_use]
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            system: None,
            messages: vec![Message::user(message)],
            tools: None,
            tool_choice: None,
            output_schema: None,
        }
    }

    /// Creates a new generation request with a system prompt and user message.
    ///
    /// # Example
    ///
    /// ```rust
    /// use polaris_models::llm::GenerationRequest;
    ///
    /// let request = GenerationRequest::with_system(
    ///     "You are a helpful assistant",
    ///     "What's the weather like?"
    /// );
    /// ```
    #[must_use]
    pub fn with_system(system: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            system: Some(system.into()),
            messages: vec![Message::user(message)],
            tools: None,
            tool_choice: None,
            output_schema: None,
        }
    }

    /// Sets the system prompt for the model.
    #[must_use]
    pub fn system(mut self, system: impl Into<String>) -> Self {
        self.system = Some(system.into());
        self
    }

    /// Adds conversation history before the current message.
    ///
    /// The messages provided will be prepended to the existing messages.
    #[must_use]
    pub fn history(mut self, mut messages: Vec<Message>) -> Self {
        messages.append(&mut self.messages);
        self.messages = messages;
        self
    }

    /// Adds a single tool to the request.
    ///
    /// This can be called multiple times to add multiple tools.
    #[must_use]
    pub fn tool(mut self, tool: ToolDefinition) -> Self {
        self.tools.get_or_insert_with(Vec::new).push(tool);
        self
    }

    /// Sets all available tools, replacing any previously added tools.
    #[must_use]
    pub fn tools(mut self, tools: Vec<ToolDefinition>) -> Self {
        self.tools = Some(tools);
        self
    }

    /// Sets how the model should choose tools.
    #[must_use]
    pub fn tool_choice(mut self, choice: ToolChoice) -> Self {
        self.tool_choice = Some(choice);
        self
    }

    /// Requires the model to call at least one tool.
    ///
    /// Shorthand for `.tool_choice(ToolChoice::Required)`.
    #[must_use]
    pub fn require_tool(mut self) -> Self {
        self.tool_choice = Some(ToolChoice::Required);
        self
    }

    /// Requires the model to call a specific tool.
    ///
    /// Shorthand for `.tool_choice(ToolChoice::Specific(name))`.
    #[must_use]
    pub fn require_tool_named(mut self, name: impl Into<String>) -> Self {
        self.tool_choice = Some(ToolChoice::Specific(name.into()));
        self
    }

    /// Allows the model to decide whether to call tools.
    ///
    /// Shorthand for `.tool_choice(ToolChoice::Auto)`.
    #[must_use]
    pub fn auto_tool(mut self) -> Self {
        self.tool_choice = Some(ToolChoice::Auto);
        self
    }

    /// Prevents the model from calling any tools.
    ///
    /// Shorthand for `.tool_choice(ToolChoice::None)`.
    #[must_use]
    pub fn no_tool(mut self) -> Self {
        self.tool_choice = Some(ToolChoice::None);
        self
    }
}

/// A generation response from a model.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GenerationResponse {
    /// The generated content blocks.
    pub content: Vec<AssistantBlock>,
    /// Token usage information.
    pub usage: Usage,
}

impl GenerationResponse {
    /// Returns all text content blocks concatenated into a single string.
    ///
    /// This is a convenience method for the common case of extracting
    /// text from a response. If multiple text blocks exist, they are
    /// concatenated together.
    ///
    /// Returns an empty string if no text content is found.
    #[must_use]
    pub fn text(&self) -> String {
        self.content
            .iter()
            .filter_map(|block| match block {
                AssistantBlock::Text(text) => Some(text.as_str()),
                _ => None,
            })
            .collect::<Vec<_>>()
            .join("")
    }
}

/// Token usage information.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Usage {
    /// Number of tokens in the input.
    pub input_tokens: Option<u64>,
    /// Number of tokens in the output.
    pub output_tokens: Option<u64>,
    /// Total tokens (input + output).
    pub total_tokens: Option<u64>,
}

// ─────────────────────
// Messages
// ─────────────────────

/// An input (user) or output (assistant) message in a conversation. Each message contains at least one content block.
///
/// Since models may not support all content types, the conversion from `Message` to provider-specific formats may be lossy (e.g., images may be omitted for text-only models).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "role", rename_all = "lowercase")]
pub enum Message {
    /// A message from the user.
    User {
        /// The content blocks of the user message.
        content: Vec<UserBlock>,
    },
    /// A message from the assistant.
    Assistant {
        /// Optional identifier for this assistant.
        id: Option<String>,
        /// The content blocks of the assistant message.
        content: Vec<AssistantBlock>,
    },
}

impl Message {
    /// Creates a user message with text content.
    #[must_use]
    pub fn user(text: impl Into<String>) -> Self {
        Self::User {
            content: vec![UserBlock::Text(text.into())],
        }
    }

    /// Creates an assistant message with text content.
    #[must_use]
    pub fn assistant(text: impl Into<String>) -> Self {
        Self::Assistant {
            id: None,
            content: vec![AssistantBlock::Text(text.into())],
        }
    }

    /// Creates an assistant message with text content and an ID.
    #[must_use]
    pub fn assistant_with_id(id: impl Into<String>, text: impl Into<String>) -> Self {
        Self::Assistant {
            id: Some(id.into()),
            content: vec![AssistantBlock::Text(text.into())],
        }
    }

    /// Creates a user message with a tool result.
    #[must_use]
    pub fn tool_result(id: impl Into<String>, content: ToolResultContent) -> Self {
        Self::User {
            content: vec![UserBlock::tool_result(id, content)],
        }
    }

    /// Creates a user message with a tool error result.
    #[must_use]
    pub fn tool_error(id: impl Into<String>, content: ToolResultContent) -> Self {
        Self::User {
            content: vec![UserBlock::tool_error(id, content)],
        }
    }

    /// Creates a user message with a tool result that includes a call ID.
    #[must_use]
    pub fn tool_result_with_call_id(
        id: impl Into<String>,
        call_id: impl Into<String>,
        content: ToolResultContent,
    ) -> Self {
        Self::User {
            content: vec![UserBlock::tool_result_with_call_id(id, call_id, content)],
        }
    }

    /// Creates a user message with a tool error result that includes a call ID.
    #[must_use]
    pub fn tool_error_with_call_id(
        id: impl Into<String>,
        call_id: impl Into<String>,
        content: ToolResultContent,
    ) -> Self {
        Self::User {
            content: vec![UserBlock::tool_error_with_call_id(id, call_id, content)],
        }
    }
}

// ─────────────────────
// Content Blocks
// ─────────────────────

/// Content that can appear in a user message.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum UserBlock {
    /// Plain text content.
    Text(String),
    /// Image content for vision models.
    Image(ImageBlock),
    /// Audio content for speech models.
    Audio(AudioBlock),
    /// Document content (PDF, code, etc.).
    Document(DocumentBlock),
    /// A tool call result from execution.
    ToolResult(ToolResult),
}

impl UserBlock {
    /// Creates a text content block.
    #[must_use]
    pub fn text(text: impl Into<String>) -> Self {
        Self::Text(text.into())
    }

    /// Creates an image content block from base64-encoded data.
    #[must_use]
    pub fn image_base64(data: impl Into<String>, media_type: ImageMediaType) -> Self {
        Self::Image(ImageBlock {
            data: DocumentSource::Base64(data.into()),
            media_type,
            additional_params: None,
        })
    }

    /// Creates an audio content block from base64-encoded data.
    #[must_use]
    pub fn audio_base64(data: impl Into<String>, media_type: AudioMediaType) -> Self {
        Self::Audio(AudioBlock {
            data: DocumentSource::Base64(data.into()),
            media_type,
            additional_params: None,
        })
    }

    /// Creates a document content block from base64-encoded data.
    #[must_use]
    pub fn document_base64(
        name: impl Into<String>,
        data: impl Into<String>,
        media_type: DocumentMediaType,
    ) -> Self {
        Self::Document(DocumentBlock {
            name: name.into(),
            data: DocumentSource::Base64(data.into()),
            media_type,
            additional_params: None,
        })
    }

    /// Creates a tool result content block.
    #[must_use]
    pub fn tool_result(id: impl Into<String>, content: ToolResultContent) -> Self {
        Self::ToolResult(ToolResult {
            id: id.into(),
            call_id: None,
            content,
            status: ToolResultStatus::Success,
        })
    }

    /// Creates an error tool result content block.
    #[must_use]
    pub fn tool_error(id: impl Into<String>, content: ToolResultContent) -> Self {
        Self::ToolResult(ToolResult {
            id: id.into(),
            call_id: None,
            content,
            status: ToolResultStatus::Error,
        })
    }

    /// Creates a tool result content block with a call ID.
    #[must_use]
    pub fn tool_result_with_call_id(
        id: impl Into<String>,
        call_id: impl Into<String>,
        content: ToolResultContent,
    ) -> Self {
        Self::ToolResult(ToolResult {
            id: id.into(),
            call_id: Some(call_id.into()),
            content,
            status: ToolResultStatus::Success,
        })
    }

    /// Creates an error tool result content block with a call ID.
    #[must_use]
    pub fn tool_error_with_call_id(
        id: impl Into<String>,
        call_id: impl Into<String>,
        content: ToolResultContent,
    ) -> Self {
        Self::ToolResult(ToolResult {
            id: id.into(),
            call_id: Some(call_id.into()),
            content,
            status: ToolResultStatus::Error,
        })
    }
}

/// Content that can appear in an assistant message.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum AssistantBlock {
    /// Plain text content.
    Text(String),
    /// A tool call request from the model.
    ToolCall(ToolCall),
    /// Reasoning/thinking content from the model.
    Reasoning(ReasoningBlock),
}

impl AssistantBlock {
    /// Creates a text content block.
    #[must_use]
    pub fn text(text: impl Into<String>) -> Self {
        Self::Text(text.into())
    }

    /// Creates a tool call content block.
    #[must_use]
    pub fn tool_call(call: ToolCall) -> Self {
        Self::ToolCall(call)
    }

    /// Creates a reasoning content block.
    #[must_use]
    pub fn reasoning(reasoning: impl Into<String>) -> Self {
        Self::Reasoning(ReasoningBlock {
            id: None,
            reasoning: vec![reasoning.into()],
            signature: None,
        })
    }

    /// Creates a reasoning content block with a signature.
    #[must_use]
    pub fn reasoning_with_signature(
        reasoning: impl Into<String>,
        signature: impl Into<String>,
    ) -> Self {
        Self::Reasoning(ReasoningBlock {
            id: None,
            reasoning: vec![reasoning.into()],
            signature: Some(signature.into()),
        })
    }
}

/// Image content.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImageBlock {
    /// The image data.
    pub data: DocumentSource,
    /// The image format.
    pub media_type: ImageMediaType,
    /// Provider-specific parameters.
    pub additional_params: Option<Value>,
}

/// Supported image formats. A provider may support a subset of these formats.
#[expect(missing_docs, reason = "variants are self-explanatory format names")]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ImageMediaType {
    JPEG,
    PNG,
    GIF,
    WEBP,
    HEIC,
    HEIF,
    SVG,
}

/// Audio content.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AudioBlock {
    /// The audio data.
    pub data: DocumentSource,
    /// The audio format.
    pub media_type: AudioMediaType,
    /// Provider-specific parameters.
    pub additional_params: Option<Value>,
}

/// Supported audio formats. A provider may support a subset of these formats.
#[expect(missing_docs, reason = "variants are self-explanatory format names")]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AudioMediaType {
    WAV,
    MP3,
    AIFF,
    AAC,
    OGG,
    FLAC,
}

/// Document content.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocumentBlock {
    /// The document name.
    pub name: String,
    /// The document data.
    pub data: DocumentSource,
    /// The document format.
    pub media_type: DocumentMediaType,
    /// Provider-specific parameters.
    pub additional_params: Option<Value>,
}

/// Supported document formats. A provider may support a subset of these formats.
#[expect(missing_docs, reason = "variants are self-explanatory format names")]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DocumentMediaType {
    PDF,
    TXT,
    HTML,
    MARKDOWN,
    CSV,
}

/// Reasoning/thinking content from extended thinking models.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReasoningBlock {
    /// Provider-assigned identifier for this reasoning block.
    pub id: Option<String>,
    /// The reasoning steps or thoughts.
    pub reasoning: Vec<String>,
    /// Signature for verification (required by some providers).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub signature: Option<String>,
}

/// Source of binary data for media content.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DocumentSource {
    /// Base64-encoded data.
    Base64(String),
}

// ─────────────────────
// Tool Calling
// ─────────────────────

/// Definition of a tool that can be called by the model.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolDefinition {
    /// Name of the tool (e.g., `get_weather`, `search_database`).
    pub name: String,
    /// Human-readable description of what the tool does.
    pub description: String,
    /// JSON Schema defining the tool's parameters.
    ///
    /// This should be an object schema with properties defining each parameter.
    /// Example:
    /// ```json
    /// {
    ///   "type": "object",
    ///   "properties": {
    ///     "city": {"type": "string", "description": "City name"},
    ///     "units": {"type": "string", "enum": ["celsius", "fahrenheit"]}
    ///   },
    ///   "required": ["city"]
    /// }
    /// ```
    pub parameters: Value,
}

/// Controls how the model should select tools.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolChoice {
    /// Model decides whether to call tools or respond with text.
    Auto,
    /// Model must call at least one tool.
    Required,
    /// Model must call this specific tool.
    Specific(String),
    /// Model must not call any tools.
    None,
}

/// A tool call request from the model.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCall {
    /// Unique identifier for this tool call.
    pub id: String,
    /// Provider-specific call identifier (e.g., for `OpenAI` function calling).
    pub call_id: Option<String>,
    /// The function to call.
    pub function: ToolFunction,
    /// Optional cryptographic signature for verification.
    pub signature: Option<String>,
    /// Provider-specific parameters.
    pub additional_params: Option<Value>,
}

/// A tool function to be called.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolFunction {
    /// The name of the function to call.
    pub name: String,
    /// The arguments to pass to the function.
    pub arguments: Value,
}

/// Status of a tool result.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolResultStatus {
    /// The tool executed successfully.
    #[default]
    Success,
    /// The tool encountered an error.
    Error,
}

/// Result of a tool call execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolResult {
    /// Identifier linking this result to the original tool call.
    pub id: String,
    /// Optional provider-specific call identifier.
    pub call_id: Option<String>,
    /// The result content.
    pub content: ToolResultContent,
    /// Whether this result represents a success or error.
    #[serde(default)]
    pub status: ToolResultStatus,
}

/// Content of a tool result.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ToolResultContent {
    /// Text result.
    Text(String),
    /// Image result.
    Image(ImageBlock),
}
