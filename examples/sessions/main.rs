//! Sessions example demonstrating persistence across runs.
//!
//! Reuses the `ReActAgent` graph and shows how conversation history
//! survives across sessions via the `SessionPlugin`.
//!
//! ```bash
//! cargo run -p examples --bin sessions -- <session_id> <working_dir> <query>
//! ```

mod plugin;

use examples::{
    AgentConfig, ContextManager, FileToolsPlugin, PersistencePlugin, ReActAgent, ReActPlugin,
};
use plugin::SessionPlugin;
use polaris::models::llm::Message;
use polaris::{
    agent::AgentExt,
    graph::{GraphExecutor, hooks::HooksAPI},
    models::{AnthropicPlugin, BedrockPlugin, ModelsPlugin},
    system::server::Server,
    tools::ToolsPlugin,
};
use std::path::PathBuf;

#[tokio::main]
async fn main() {
    let _ = dotenvy::dotenv();

    let args: Vec<String> = std::env::args().collect();
    if args.len() < 4 {
        eprintln!("Usage: sessions <session_id> <working_dir> <query>");
        std::process::exit(1);
    }

    let session_id = &args[1];
    let working_dir = PathBuf::from(&args[2])
        .canonicalize()
        .unwrap_or_else(|err| {
            eprintln!("Error: {err}");
            std::process::exit(1);
        });
    let query = &args[3];

    let config = AgentConfig::new(
        "bedrock/global.anthropic.claude-sonnet-4-5-20250929-v1:0",
        &working_dir,
    );

    // Build server
    let mut server = Server::new();
    server.add_plugins(ModelsPlugin);
    server.add_plugins(AnthropicPlugin::from_env("ANTHROPIC_API_KEY"));
    server.add_plugins(BedrockPlugin::from_env());
    server.add_plugins(ToolsPlugin);
    server.add_plugins(FileToolsPlugin::new(config.clone()));
    server.add_plugins(PersistencePlugin);
    server.add_plugins(ReActPlugin);
    server.add_plugins(SessionPlugin::new(session_id, "data"));
    server.finish();

    // Create context, restore prior session, then append the new query.
    let mut ctx = server.create_context().with(config);
    SessionPlugin::load(session_id, "data", &server, &mut ctx);
    {
        let mut cm = ctx
            .get_resource_mut::<ContextManager>()
            .expect("ContextManager missing");
        cm.push(Message::user(query));
    }

    // Execute
    let graph = ReActAgent.to_graph();
    let executor = GraphExecutor::new().with_default_max_iterations(10);
    let hooks = server.api::<HooksAPI>();

    match executor.execute(&graph, &mut ctx, hooks).await {
        Ok(r) => println!("\n[Done] {} nodes in {:?}", r.nodes_executed, r.duration),
        Err(err) => eprintln!("Error: {err}"),
    }
}
