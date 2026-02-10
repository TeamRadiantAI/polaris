//! Tool definitions and execution.

use crate::config::AgentConfig;
use polaris_models::llm::ToolDefinition;
use schemars::{JsonSchema, schema_for};
use serde::{Deserialize, Serialize};

/// Parameters for the `list_files` tool.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ListFilesParams {
    /// Directory path (relative to working directory).
    pub path: String,
}

/// Parameters for the `read_file` tool.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ReadFileParams {
    /// File path (relative to working directory).
    pub path: String,
}

/// Parameters for the `write_file` tool.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct WriteFileParams {
    /// File path (relative to working directory).
    pub path: String,
    /// Content to write.
    pub content: String,
}

fn tool_def<T: JsonSchema>(name: &str, description: &str) -> ToolDefinition {
    let mut schema = serde_json::to_value(schema_for!(T)).expect("schema serialization failed");
    // Anthropic requires additionalProperties: false
    if let Some(obj) = schema.as_object_mut() {
        obj.insert("additionalProperties".to_string(), serde_json::json!(false));
    }
    ToolDefinition {
        name: name.to_string(),
        description: description.to_string(),
        parameters: schema,
    }
}

/// Returns definitions for all available tools.
#[must_use]
pub fn get_tool_definitions() -> Vec<ToolDefinition> {
    vec![
        tool_def::<ListFilesParams>("list_files", "List files in a directory."),
        tool_def::<ReadFileParams>("read_file", "Read the contents of a file."),
        tool_def::<WriteFileParams>("write_file", "Write content to a file."),
    ]
}

/// Executes a tool by name.
///
/// # Errors
///
/// Returns an error if the tool is unknown, parameters are invalid,
/// or the operation fails.
pub fn execute_tool(
    name: &str,
    args: &serde_json::Value,
    config: &AgentConfig,
) -> Result<String, String> {
    match name {
        "list_files" => {
            let params: ListFilesParams = serde_json::from_value(args.clone())
                .map_err(|err| format!("invalid params: {err}"))?;
            list_files(&params.path, config)
        }
        "read_file" => {
            let params: ReadFileParams = serde_json::from_value(args.clone())
                .map_err(|err| format!("invalid params: {err}"))?;
            read_file(&params.path, config)
        }
        "write_file" => {
            let params: WriteFileParams = serde_json::from_value(args.clone())
                .map_err(|err| format!("invalid params: {err}"))?;
            write_file(&params.path, &params.content, config)
        }
        _ => Err(format!("unknown tool: {name}")),
    }
}

fn list_files(path: &str, config: &AgentConfig) -> Result<String, String> {
    let resolved = config
        .resolve_path(path)
        .ok_or_else(|| format!("path '{path}' escapes sandbox"))?;

    let entries = std::fs::read_dir(&resolved).map_err(|err| err.to_string())?;
    let files: Vec<String> = entries
        .filter_map(Result::ok)
        .map(|entry| entry.file_name().to_string_lossy().to_string())
        .collect();

    Ok(if files.is_empty() {
        "(empty directory)".to_string()
    } else {
        files.join("\n")
    })
}

fn read_file(path: &str, config: &AgentConfig) -> Result<String, String> {
    let resolved = config
        .resolve_path(path)
        .ok_or_else(|| format!("path '{path}' escapes sandbox"))?;
    std::fs::read_to_string(&resolved).map_err(|err| err.to_string())
}

fn write_file(path: &str, content: &str, config: &AgentConfig) -> Result<String, String> {
    let resolved = config
        .resolve_path(path)
        .ok_or_else(|| format!("path '{path}' escapes sandbox"))?;
    std::fs::write(&resolved, content).map_err(|err| err.to_string())?;
    Ok(format!("Wrote to {}", resolved.display()))
}
