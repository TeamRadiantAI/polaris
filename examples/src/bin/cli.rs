//! Interactive CLI REPL for the ReAct agent.
//!
//! Multi-turn conversational interface that maintains conversation history
//! across turns, with optional session persistence.
//!
//! # Usage
//!
//! ```bash
//! cargo run -p cli -- <working_dir> [--session <id>]
//! ```
//!
//! # Commands
//!
//! - `/help` — Show available commands
//! - `/history` — Show conversation history
//! - `/clear` — Clear conversation history
//! - `/exit` or `/quit` — Exit the REPL

use examples::plugins::{FileToolsConfig, FileToolsPlugin, SessionPlugin, TerminalIOPlugin};
use examples::react_agent::{AgentConfig, ContextManager, ReActAgent, ReActPlugin, ReactState};
use polaris::models::llm::{AssistantBlock, Message, UserBlock};
use polaris::plugins::{IOMessage, InputBuffer, PersistencePlugin};
use polaris::{
    agent::AgentExt,
    graph::{GraphExecutor, hooks::HooksAPI},
    models::{BedrockPlugin, ModelsPlugin},
    plugins::{IOPlugin, ServerInfoPlugin},
    system::server::Server,
    tools::ToolsPlugin,
};
use rustyline::DefaultEditor;
use rustyline::error::ReadlineError;
use std::path::PathBuf;

// ANSI style constants
const STYLE_DIM: &str = "\x1b[2m";
const STYLE_BOLD: &str = "\x1b[1m";
const STYLE_RED: &str = "\x1b[31m";
const STYLE_RESET: &str = "\x1b[0m";

fn print_help() {
    eprintln!("{STYLE_DIM}Commands:");
    eprintln!("  /help    — Show this help message");
    eprintln!("  /history — Show conversation history");
    eprintln!("  /clear   — Clear conversation history");
    eprintln!("  /exit    — Exit the REPL{STYLE_RESET}");
}

/// Extracts text content from user message blocks.
fn extract_user_text(content: &[UserBlock]) -> String {
    content
        .iter()
        .filter_map(|b| {
            if let UserBlock::Text(t) = b {
                Some(t.text.as_str())
            } else {
                None
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// Extracts text content from assistant message blocks.
fn extract_assistant_text(content: &[AssistantBlock]) -> String {
    content
        .iter()
        .filter_map(|b| {
            if let AssistantBlock::Text(t) = b {
                Some(t.text.as_str())
            } else {
                None
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn print_history(ctx: &polaris::system::param::SystemContext<'_>) {
    let cm = ctx
        .get_resource::<ContextManager>()
        .expect("ContextManager missing");
    if cm.messages.is_empty() {
        eprintln!("{STYLE_DIM}  (no messages){STYLE_RESET}");
        return;
    }
    for (i, msg) in cm.messages.iter().enumerate() {
        match msg {
            Message::User { content } => {
                let text = extract_user_text(content);
                eprintln!("{STYLE_DIM}  [{i}] User: {text}{STYLE_RESET}");
            }
            Message::Assistant { content, .. } => {
                let text = extract_assistant_text(content);
                if !text.is_empty() {
                    eprintln!("{STYLE_DIM}  [{i}] Assistant: {text}{STYLE_RESET}");
                } else {
                    eprintln!("{STYLE_DIM}  [{i}] Assistant: (tool call){STYLE_RESET}");
                }
            }
        }
    }
}

#[tokio::main]
async fn main() {
    let _ = dotenvy::dotenv();

    // Parse arguments: <working_dir> [--session <id>]
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage: cli <working_dir> [--session <id>]");
        std::process::exit(1);
    }

    let working_dir = PathBuf::from(&args[1])
        .canonicalize()
        .unwrap_or_else(|err| {
            eprintln!("Error: {err}");
            std::process::exit(1);
        });

    let session_id = args
        .iter()
        .position(|a| a == "--session")
        .and_then(|i| args.get(i + 1))
        .cloned()
        .unwrap_or_else(|| "default".to_string());

    let agent_config = AgentConfig::new("bedrock/global.anthropic.claude-sonnet-4-5-20250929-v1:0");
    let file_tools_config = FileToolsConfig::new(&working_dir);

    // Build server
    let mut server = Server::new();

    server
        .add_plugins(ServerInfoPlugin)
        .add_plugins(IOPlugin)
        .add_plugins(TerminalIOPlugin)
        .add_plugins(ModelsPlugin)
        .add_plugins(BedrockPlugin::from_env())
        .add_plugins(ToolsPlugin)
        .add_plugins(FileToolsPlugin::new(file_tools_config))
        .add_plugins(PersistencePlugin)
        .add_plugins(ReActPlugin)
        .add_plugins(SessionPlugin::new(&session_id, "data"));

    server.finish();

    // Build graph and executor
    let graph = ReActAgent.to_graph();
    let executor = GraphExecutor::new().with_default_max_iterations(10);
    let hooks = server.api::<HooksAPI>();

    // Create context and restore session
    let mut ctx = server.create_context().with(agent_config);
    if let Err(err) = SessionPlugin::load(&session_id, "data", &server, &mut ctx) {
        eprintln!("{STYLE_RED}Failed to load session: {err}{STYLE_RESET}");
    }

    // Welcome banner
    eprintln!("{STYLE_BOLD}Polaris ReAct Agent{STYLE_RESET}");
    eprintln!(
        "{STYLE_DIM}Session: {session_id} | {}/{STYLE_RESET}",
        working_dir.display()
    );
    eprintln!("{STYLE_DIM}Type /help for commands, Ctrl+D or /exit to quit.{STYLE_RESET}");
    eprintln!();

    // Initialize rustyline editor
    let mut rl = DefaultEditor::new().expect("Failed to create readline editor");

    // REPL loop
    loop {
        // Read input with rustyline
        let readline = rl.readline(&format!("{STYLE_BOLD}> {STYLE_RESET}"));

        match readline {
            Ok(line) => {
                let trimmed = line.trim();

                // Skip empty lines
                if trimmed.is_empty() {
                    continue;
                }

                // Add to history
                let _ = rl.add_history_entry(trimmed);

                // Handle commands
                match trimmed {
                    "/exit" | "/quit" => {
                        break;
                    }
                    "/help" => {
                        print_help();
                        continue;
                    }
                    "/history" => {
                        print_history(&ctx);
                        continue;
                    }
                    "/clear" => {
                        ctx.insert(ContextManager::default());
                        eprintln!("{STYLE_DIM}  Conversation cleared.{STYLE_RESET}");
                        continue;
                    }
                    _ => {}
                }

                // Add user message to input buffer (will be processed by receive_user_input system)
                {
                    let mut input_buffer = ctx
                        .get_resource_mut::<InputBuffer>()
                        .expect("InputBuffer missing");
                    input_buffer.push(IOMessage::user_text(trimmed));
                }

                // Reset agent state for this turn
                ctx.insert(ReactState::default());
                ctx.clear_outputs();

                // Execute agent graph
                match executor.execute(&graph, &mut ctx, hooks).await {
                    Ok(result) => {
                        eprintln!(
                            "{STYLE_DIM}  ({} nodes, {:.2}s){STYLE_RESET}\n",
                            result.nodes_executed,
                            result.duration.as_secs_f64()
                        );
                    }
                    Err(err) => {
                        eprintln!("{STYLE_RED}Execution error: {err}{STYLE_RESET}\n");
                    }
                }
            }
            Err(ReadlineError::Interrupted) => {
                eprintln!("^C");
                continue;
            }
            Err(ReadlineError::Eof) => {
                break;
            }
            Err(err) => {
                eprintln!("{STYLE_RED}Error: {err}{STYLE_RESET}");
                break;
            }
        }
    }
}
