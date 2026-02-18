//! Example `ReAct` agent CLI.
//!
//! A file assistant that can list, read, and write files within a sandboxed directory.
//!
//! # Usage
//!
//! ```bash
//! react <working_dir> <query>
//! ```
//!
//! # Example
//!
//! ```bash
//! react ./sandbox "List all files"
//! ```

use examples::tools::FileToolsPlugin;
use examples::{AgentConfig, ContextManager, ReActAgent, ReactState};
use polaris::{
    agent::AgentExt,
    graph::GraphExecutor,
    models::{AnthropicPlugin, BedrockPlugin, ModelsPlugin},
    system::server::Server,
    tools::ToolsPlugin,
};
use std::path::PathBuf;

#[tokio::main]
async fn main() {
    let _ = dotenvy::dotenv();

    let args: Vec<String> = std::env::args().collect();
    if args.len() < 3 {
        tracing::error!("Usage: <working_dir> <query>");
        tracing::error!("Example: ./sandbox \"List all files\"");
        std::process::exit(1);
    }

    let working_dir = PathBuf::from(&args[1]);
    let query = &args[2];

    if !working_dir.is_dir() {
        tracing::error!("Error: {} is not a directory", working_dir.display());
        std::process::exit(1);
    }

    let working_dir = working_dir.canonicalize().unwrap_or_else(|err| {
        tracing::error!(
            "Error: cannot canonicalize {}: {}",
            working_dir.display(),
            err
        );
        std::process::exit(1);
    });

    let config = AgentConfig::new(
        "bedrock/global.anthropic.claude-sonnet-4-5-20250929-v1:0",
        working_dir,
    );

    // Initialize server with plugins
    let mut server = Server::new();
    server.add_plugins(ModelsPlugin);
    server.add_plugins(AnthropicPlugin::from_env("ANTHROPIC_API_KEY"));
    server.add_plugins(BedrockPlugin::from_env());
    server.add_plugins(ToolsPlugin);
    server.add_plugins(FileToolsPlugin::new(config.clone()));
    server.finish();

    // Create execution context
    let mut ctx = server
        .create_context()
        .with(config)
        .with(ContextManager::new(query))
        .with(ReactState::default());

    // Build and execute the agent graph
    let graph = ReActAgent.to_graph();
    let executor = GraphExecutor::new().with_default_max_iterations(10);

    if let Err(err) = executor.execute(&graph, &mut ctx, None).await {
        tracing::error!("Error: {err}");
    }
}
