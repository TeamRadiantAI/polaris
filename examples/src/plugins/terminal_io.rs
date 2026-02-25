//! Terminal I/O provider for CLI-based agent interaction.
//!
//! Provides [`TerminalIOProvider`] which implements [`IOProvider`] for stdin/stdout
//! communication, and [`TerminalIOPlugin`] which registers it as the [`UserIO`] global.

use polaris::plugins::{IOContent, IOError, IOMessage, IOPlugin, IOProvider, IOSource, UserIO};
use polaris::system::plugin::{Plugin, PluginId, Version};
use polaris::system::server::Server;
use std::sync::Arc;

// ANSI style constants
const STYLE_DIM: &str = "\x1b[2m";
const STYLE_RED: &str = "\x1b[31m";
const STYLE_RESET: &str = "\x1b[0m";

/// I/O provider that reads from stdin and writes to stdout/stderr.
///
/// Routes messages based on type and source:
/// - stderr for `"type": "trace"`, `"type": "error"` and system messages
/// - stdout for agent messages
pub struct TerminalIOProvider;

impl IOProvider for TerminalIOProvider {
    async fn send(&self, message: IOMessage) -> Result<(), IOError> {
        let is_trace = message.metadata.get("type").map(|v| v == "trace") == Some(true);
        let is_error = message.metadata.get("type").map(|v| v == "error") == Some(true);

        let text = match &message.content {
            IOContent::Text(s) => s.clone(),
            IOContent::Structured(v) => v.to_string(),
            IOContent::Binary { mime_type, data } => {
                format!("[binary: {mime_type}, {} bytes]", data.len())
            }
        };

        if is_trace {
            // Trace messages on stderr, indented and dimmed
            eprintln!("  {STYLE_DIM}{text}{STYLE_RESET}");
        } else if is_error {
            // Error messages on stderr, indented and red
            eprintln!("  {STYLE_RED}{text}{STYLE_RESET}");
        } else if matches!(message.source, IOSource::System) {
            // System messages on stderr
            eprintln!("{text}");
        } else {
            // Agent messages on stdout
            println!("{text}");
        }

        Ok(())
    }

    async fn receive(&self) -> Result<IOMessage, IOError> {
        tokio::task::spawn_blocking(|| {
            let mut line = String::new();
            std::io::stdin()
                .read_line(&mut line)
                .map_err(|err| IOError::Provider(err.to_string()))?;

            if line.is_empty() {
                return Err(IOError::Closed);
            }

            let trimmed = line.trim_end().to_string();
            Ok(IOMessage::user_text(trimmed))
        })
        .await
        .map_err(|err| IOError::Provider(err.to_string()))?
    }
}

/// Plugin that registers [`TerminalIOProvider`] as the [`UserIO`] local resource.
///
/// Depends on [`IOPlugin`].
pub struct TerminalIOPlugin;

impl Plugin for TerminalIOPlugin {
    const ID: &'static str = "examples::terminal_io";
    const VERSION: Version = Version::new(0, 0, 1);

    fn build(&self, server: &mut Server) {
        let provider = Arc::new(TerminalIOProvider);
        server.register_local(move || UserIO::new(provider.clone()));
    }

    fn dependencies(&self) -> Vec<PluginId> {
        vec![PluginId::of::<IOPlugin>()]
    }
}
