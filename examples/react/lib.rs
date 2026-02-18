//! Example `ReAct` (Reasoning + Acting) agent built with Polaris.
//!
//! This example demonstrates how to build an agentic loop using Polaris's graph-based
//! execution model. The agent can reason about user requests and use tools to fulfill them.
//!
//! # Architecture
//!
//! ```text
//! ┌────────────────────────────────────────────────────────────┐
//! │  ReAct Loop                                                │
//! │                                                            │
//! │  ┌────────┐   ┌──────────┐   ┌─────────┐   ┌───────┐       │
//! │  │ Reason │──▶│ Decision │──▶│ Execute │──▶│Observe│       │
//! │  └────────┘   └────┬─────┘   └─────────┘   └───┬───┘       │
//! │       ▲            │                           │           │
//! │       └────────────┼───────────────────────────┘           │
//! │                    ▼                                       │
//! │              ┌──────────┐                                  │
//! │              │ Respond  │                                  │
//! │              └──────────┘                                  │
//! └────────────────────────────────────────────────────────────┘
//! ```

mod config;
mod context;
mod state;
pub mod tools;

pub use config::AgentConfig;
pub use context::ContextManager;
pub use state::ReactState;

use polaris::agent::Agent;
use polaris::graph::Graph;
use polaris::models;
use polaris::models::ModelRegistry;
use polaris::models::llm::{
    AssistantBlock, GenerationRequest, Message, ToolChoice, ToolFunction, ToolResultContent,
};
use polaris::system::param::{Out, Res, ResMut};
use polaris::system::system;
use polaris::tools::ToolRegistry;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;

const SYSTEM_PROMPT: &str = "You are a helpful ReAct agent. Think step by step.";

/// Action decided by the reasoning step.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    /// Use a tool to gather information.
    UseTool,
    /// Respond to the user with a final answer.
    Respond,
}

/// Output from the reasoning step.
#[derive(Debug, Clone)]
pub struct ReasoningResult {
    /// The decided action type.
    pub action: Action,
}

/// Tool call to execute.
#[derive(Debug, Clone)]
pub struct ToolCall {
    /// Tool call ID (for linking results).
    pub id: String,
    /// Name of the tool.
    pub name: String,
    /// Arguments for the tool.
    pub args: Value,
}

/// Result of tool execution.
#[derive(Debug, Clone)]
pub struct ToolResult {
    /// Tool call ID (for linking to the call).
    pub id: String,
    /// Output from the tool.
    pub output: String,
    /// Whether execution succeeded.
    pub success: bool,
}

/// Schema for `LLM` structured reasoning output.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
struct LlmReasoningOutput {
    /// The agent's thought process.
    thought: String,
    /// Action: `use_tool` or `respond`.
    action: String,
}

/// Decide whether to use a tool or respond.
#[system]
async fn reason(
    context: Res<ContextManager>,
    config: Res<AgentConfig>,
    registry: Res<ModelRegistry>,
    tool_registry: Res<ToolRegistry>,
) -> ReasoningResult {
    let model_id = &config.model_id;
    let messages = context.messages.clone();

    let llm = registry.llm(model_id).expect("model not found");

    let tools_text = tool_registry
        .definitions()
        .iter()
        .map(|t| format!("- {}: {}", t.name, t.description))
        .collect::<Vec<_>>()
        .join("\n");

    let system = format!(
        "{SYSTEM_PROMPT}

Available tools:
{tools_text}

Based on the conversation, decide what to do next.
- If you need more information, choose \"use_tool\" to call a tool.
- If you have enough information to answer, choose \"respond\"."
    );

    // Build request with conversation history.
    let mut gen_request =
        GenerationRequest::with_system(system, "What should I do next?").history(messages);

    // Bedrock requires toolConfig when history contains toolUse/toolResult blocks.
    if context.has_tool_blocks() {
        gen_request = gen_request.tools(tool_registry.definitions());
    }

    match llm
        .generate_structured::<LlmReasoningOutput>(gen_request)
        .await
    {
        Ok(output) => {
            println!("\n[Reasoning] {}", output.thought);
            println!("[Decision] {}\n", output.action);
            ReasoningResult {
                action: if output.action == "use_tool" {
                    Action::UseTool
                } else {
                    Action::Respond
                },
            }
        }
        Err(err) => {
            eprintln!("LLM error: {err}");
            ReasoningResult {
                action: Action::Respond,
            }
        }
    }
}

/// Select a tool and add the assistant message to history.
#[system]
async fn select_tool(
    mut context: ResMut<ContextManager>,
    config: Res<AgentConfig>,
    registry: Res<ModelRegistry>,
    tool_registry: Res<ToolRegistry>,
) -> ToolCall {
    let model_id = &config.model_id;
    let messages = context.messages.clone();

    let llm = registry.llm(model_id).expect("model not found");

    let gen_request = GenerationRequest::with_system(SYSTEM_PROMPT, "Select a tool to use.")
        .history(messages)
        .tools(tool_registry.definitions())
        .tool_choice(ToolChoice::Required);

    let tool_call = match llm.generate(gen_request).await {
        Ok(response) => response.content.iter().find_map(|block| {
            if let AssistantBlock::ToolCall(call) = block {
                Some(call.clone())
            } else {
                None
            }
        }),
        Err(err) => {
            eprintln!("LLM error: {err}");
            None
        }
    };

    match tool_call {
        Some(call) => {
            println!(
                "[Tool Call] {}({})",
                call.function.name, call.function.arguments
            );

            // Add assistant message with tool call to history
            context.push(Message::Assistant {
                id: None,
                content: vec![AssistantBlock::ToolCall(call.clone())],
            });

            ToolCall {
                id: call.id,
                name: call.function.name,
                args: call.function.arguments.clone(),
            }
        }
        None => {
            println!("[Tool Call] list_files({{\"path\": \".\"}}) (fallback)");

            // Create fallback tool call and add to history
            let fallback = models::llm::ToolCall {
                id: "fallback".to_string(),
                call_id: None,
                function: ToolFunction {
                    name: "list_files".to_string(),
                    arguments: serde_json::json!({"path": "."}),
                },
                signature: None,
                additional_params: None,
            };

            context.push(Message::Assistant {
                id: None,
                content: vec![AssistantBlock::ToolCall(fallback.clone())],
            });

            ToolCall {
                id: fallback.id,
                name: fallback.function.name,
                args: serde_json::json!({"path": "."}),
            }
        }
    }
}

/// Execute the selected tool.
#[system]
async fn execute_tool(call: Out<ToolCall>, tool_registry: Res<ToolRegistry>) -> ToolResult {
    match tool_registry.execute(&call.name, &call.args).await {
        Ok(value) => {
            let output = value
                .as_str()
                .map(String::from)
                .unwrap_or_else(|| value.to_string());
            println!("[Tool Result] {}\n", output);
            ToolResult {
                id: call.id.clone(),
                output,
                success: true,
            }
        }
        Err(err) => {
            let output = err.to_string();
            eprintln!("[Tool Error] {}\n", output);
            ToolResult {
                id: call.id.clone(),
                output,
                success: false,
            }
        }
    }
}

/// Add tool result to message history.
#[system]
async fn observe(result: Out<ToolResult>, mut context: ResMut<ContextManager>) -> ReactState {
    let message = if result.success {
        Message::tool_result(&result.id, ToolResultContent::Text(result.output.clone()))
    } else {
        Message::tool_error(&result.id, ToolResultContent::Text(result.output.clone()))
    };
    context.push(message);

    ReactState { is_complete: false }
}

/// Initialize the agent loop.
async fn init() -> ReactState {
    ReactState { is_complete: false }
}

/// Generate and output the final response.
#[system]
async fn respond(
    mut context: ResMut<ContextManager>,
    config: Res<AgentConfig>,
    registry: Res<ModelRegistry>,
    tool_registry: Res<ToolRegistry>,
) -> ReactState {
    let model_id = &config.model_id;
    let messages = context.messages.clone();

    let llm = registry.llm(model_id).expect("model not found");

    // Bedrock requires toolConfig when history contains toolUse/toolResult blocks.
    let mut gen_request =
        GenerationRequest::with_system(SYSTEM_PROMPT, "Please provide your final response.")
            .history(messages);

    if context.has_tool_blocks() {
        gen_request = gen_request.tools(tool_registry.definitions());
    }

    match llm.generate(gen_request).await {
        Ok(response) => {
            let text = response.text();
            println!("[Response]\n{text}");

            // Add assistant response to history
            context.push(Message::assistant(&text));
        }
        Err(err) => {
            eprintln!("LLM error: {err}");
            println!("I encountered an error processing your request.");
        }
    }

    ReactState { is_complete: true }
}

/// `ReAct` agent implementing the Reasoning + Acting pattern.
#[derive(Debug, Clone, Default)]
pub struct ReActAgent;

impl Agent for ReActAgent {
    fn build(&self, graph: &mut Graph) {
        graph.add_system(init);

        graph.add_loop::<ReactState, _, _>(
            "react_loop",
            |state| state.is_complete,
            |g| {
                g.add_system(reason);

                g.add_conditional_branch::<ReasoningResult, _, _, _>(
                    "action",
                    |result| result.action == Action::UseTool,
                    |tool_branch| {
                        tool_branch.add_system(select_tool);
                        tool_branch.add_system(execute_tool);
                        tool_branch.add_system(observe);
                    },
                    |respond_branch| {
                        respond_branch.add_system(respond);
                    },
                );
            },
        );
    }

    fn name(&self) -> &str {
        "ReActAgent"
    }
}
