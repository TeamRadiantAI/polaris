//! `ReAct` (Reasoning + Acting) agent definition.
//!
//! # Architecture
//!
//! ```text
//! ┌────────────────────────────────────────────────────────┐
//! │  ReAct Loop                                            │
//! │                                                        │
//! │  ┌────────┐   ┌──────────┐   ┌─────────┐              │
//! │  │ Reason │──▶│ Decision │──▶│ Execute │              │
//! │  └────────┘   └────┬─────┘   └────┬────┘              │
//! │       ▲            │               │                   │
//! │       └────────────┼───────────────┘                   │
//! │                    ▼                                   │
//! │              ┌──────────┐                              │
//! │              │ Respond  │                              │
//! │              └──────────┘                              │
//! └────────────────────────────────────────────────────────┘
//! ```

use super::config::AgentConfig;
use super::context::ContextManager;
use super::state::ReactState;

use polaris::agent::Agent;
use polaris::graph::Graph;
use polaris::models::ModelRegistry;
use polaris::models::llm::{
    AssistantBlock, GenerationRequest, Llm, Message, ToolChoice, ToolResult, ToolResultContent,
    ToolResultStatus,
};
use polaris::plugins::{IOContent, IOMessage, IOSource, PersistenceAPI, UserIO};
use polaris::prelude::Out;
use polaris::system::param::{Res, ResMut};
use polaris::system::plugin::{Plugin, Version};
use polaris::system::prelude::SystemError;
use polaris::system::resource::LocalResource;
use polaris::system::server::Server;
use polaris::system::system;
use polaris::tools::ToolRegistry;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::ops::Deref;

/// Wrapper for the current LLM instance used by the agent.
#[derive(Clone)]
pub struct AgentLlm(Llm);

impl Deref for AgentLlm {
    type Target = Llm;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl LocalResource for AgentLlm {}

/// Plugin that registers the `ReAct` agent's local resources.
pub struct ReActPlugin;

impl Plugin for ReActPlugin {
    const ID: &'static str = "examples::react";
    const VERSION: Version = Version::new(0, 0, 1);

    fn build(&self, server: &mut Server) {
        server.register_local(ContextManager::default);
        server.register_local(ReactState::default);
    }

    fn ready(&self, server: &mut Server) {
        // If a PersistenceAPI is available, register ContextManager for persistence.
        if let Some(api) = server.api::<PersistenceAPI>() {
            api.register::<ContextManager>(Self::ID);
        }
    }
}

const SYSTEM_PROMPT: &str = "You are a helpful ReAct agent. Think step by step.";

/// Action decided by the reasoning step.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Action {
    /// Use a tool to gather information.
    UseTool,
    /// Respond to the user with a final answer.
    Respond,
}

/// Reasoning for taking an action.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
struct ReasoningOutput {
    /// Reasoning for the action.
    thought: String,
    /// Action to take.
    action: Action,
}

/// Helper to send a trace message via `UserIO`.
async fn send_trace(user_io: &UserIO, text: impl Into<String>) {
    let msg =
        IOMessage::from_agent("react", IOContent::Text(text.into())).with_metadata("type", "trace");
    let _ = user_io.send(msg).await;
}

/// Helper to send an error message via `UserIO`.
async fn send_error(user_io: &UserIO, text: impl Into<String>) {
    let msg = IOMessage::new(IOContent::Text(text.into()), IOSource::System)
        .with_metadata("type", "error");
    let _ = user_io.send(msg).await;
}

/// Receive user input from the input buffer and add to conversation history.
#[system]
async fn receive_user_input(
    user_io: Res<UserIO>,
    mut context: ResMut<ContextManager>,
) -> Result<(), SystemError> {
    let message = user_io
        .receive()
        .await
        .map_err(|err| SystemError::ExecutionError(err.to_string()))?;

    if let IOContent::Text(text) = message.content {
        context.push(Message::user(text));
    }

    Ok(())
}

/// Initialize the LLM from the registry.
#[system]
async fn init_llm(config: Res<AgentConfig>, registry: Res<ModelRegistry>) -> AgentLlm {
    let llm = registry
        .llm(&config.model_id)
        .expect("model not found in registry");
    AgentLlm(llm)
}

/// Decide whether to use a tool or respond.
#[system]
async fn reason(
    context: Res<ContextManager>,
    llm: Out<AgentLlm>,
    tool_registry: Res<ToolRegistry>,
    user_io: Res<UserIO>,
) -> Result<ReasoningOutput, SystemError> {
    let messages = context.messages.clone();

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

    let output = llm
        .generate_structured::<ReasoningOutput>(gen_request)
        .await
        .map_err(|err| SystemError::ExecutionError(err.to_string()))?;

    send_trace(&user_io, format!("[Reasoning] {}", output.thought)).await;
    send_trace(&user_io, format!("[Decision]  {:?}", output.action)).await;

    Ok(output)
}

/// Call a tool: select from LLM, execute, and add results to history.
#[system]
async fn call_tool(
    mut context: ResMut<ContextManager>,
    llm: Out<AgentLlm>,
    tool_registry: Res<ToolRegistry>,
    user_io: Res<UserIO>,
) -> Result<(), SystemError> {
    let messages = context.messages.clone();

    let gen_request = GenerationRequest::with_system(SYSTEM_PROMPT, "Select a tool to use.")
        .history(messages)
        .tools(tool_registry.definitions())
        .tool_choice(ToolChoice::Required);

    let response = llm
        .generate(gen_request)
        .await
        .map_err(|err| SystemError::ExecutionError(err.to_string()))?;

    let tool_call = response
        .content
        .iter()
        .find_map(|block| {
            if let AssistantBlock::ToolCall(call) = block {
                Some(call.clone())
            } else {
                None
            }
        })
        .ok_or_else(|| {
            SystemError::ExecutionError("LLM did not provide a tool call".to_string())
        })?;

    send_trace(
        &user_io,
        format!(
            "[Tool Call]  {}({})",
            tool_call.function.name, tool_call.function.arguments
        ),
    )
    .await;

    // Add assistant message with tool call to history
    context.push(Message::Assistant {
        id: None,
        content: vec![AssistantBlock::ToolCall(tool_call.clone())],
    });

    // Execute the tool
    let result = match tool_registry
        .execute(&tool_call.function.name, &tool_call.function.arguments)
        .await
    {
        Ok(value) => {
            let output = value
                .as_str()
                .map(String::from)
                .unwrap_or_else(|| value.to_string());
            send_trace(&user_io, format!("[Tool Result] {output}")).await;
            ToolResult {
                id: tool_call.id.clone(),
                call_id: tool_call.call_id.clone(),
                content: ToolResultContent::Text(output),
                status: ToolResultStatus::Success,
            }
        }
        Err(err) => {
            let output = err.to_string();
            send_error(&user_io, format!("[Tool Error] {output}")).await;
            ToolResult {
                id: tool_call.id.clone(),
                call_id: tool_call.call_id.clone(),
                content: ToolResultContent::Text(output),
                status: ToolResultStatus::Error,
            }
        }
    };

    // Add tool result to message history
    let message = if result.status == ToolResultStatus::Success {
        Message::tool_result(&result.id, result.content.clone())
    } else {
        Message::tool_error(&result.id, result.content.clone())
    };
    context.push(message);

    Ok(())
}

/// Initialize the agent loop.
async fn init_loop() -> ReactState {
    ReactState { is_complete: false }
}

/// Generate and output the final response.
#[system]
async fn respond(
    mut context: ResMut<ContextManager>,
    llm: Out<AgentLlm>,
    tool_registry: Res<ToolRegistry>,
    user_io: Res<UserIO>,
) -> ReactState {
    let messages = context.messages.clone();

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
            let msg = IOMessage::from_agent("react", IOContent::Text(text.clone()));
            let _ = user_io.send(msg).await;

            // Add assistant response to history
            context.push(Message::assistant(&text));
        }
        Err(err) => {
            send_error(&user_io, format!("LLM error: {err}")).await;
            let msg = IOMessage::from_agent(
                "react",
                IOContent::Text("I encountered an error processing your request.".to_string()),
            );
            let _ = user_io.send(msg).await;
        }
    }

    ReactState { is_complete: true }
}

/// `ReAct` agent implementing the Reasoning + Acting pattern.
#[derive(Debug, Clone, Default)]
pub struct ReActAgent;

impl Agent for ReActAgent {
    fn build(&self, graph: &mut Graph) {
        graph.add_system(receive_user_input);
        graph.add_system(init_llm);
        graph.add_system(init_loop);

        graph.add_loop::<ReactState, _, _>(
            "react_loop",
            |state| state.is_complete,
            |g| {
                g.add_system(reason);

                g.add_conditional_branch::<ReasoningOutput, _, _, _>(
                    "action",
                    |result| result.action == Action::UseTool,
                    |tool_branch| {
                        tool_branch.add_system(call_tool);
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
