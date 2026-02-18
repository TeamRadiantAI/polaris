//! The core [`Tool`] trait for executable tools.

use crate::error::ToolError;
use polaris_models::llm::ToolDefinition;
use std::future::Future;
use std::pin::Pin;

/// A tool that can be invoked by an LLM agent.
///
/// Tools expose a [`ToolDefinition`] (name, description, JSON schema) for the LLM,
/// and an async [`execute`](Tool::execute) method that runs with the tool's
/// captured environment.
pub trait Tool: Send + Sync + 'static {
    /// Returns the LLM-facing tool definition with JSON schema.
    fn definition(&self) -> ToolDefinition;

    /// Executes the tool with JSON arguments.
    fn execute(
        &self,
        args: serde_json::Value,
    ) -> Pin<Box<dyn Future<Output = Result<serde_json::Value, ToolError>> + Send + '_>>;
}
