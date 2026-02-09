//! Anthropic provider backend.
//!
//! Uses the Anthropic messages API.
//!
//! ```ignore
//! server.add_plugins(AnthropicPlugin::from_env("ANTHROPIC_API_KEY"));
//! ```

mod client;
mod plugin;
mod provider;
mod types;

pub use plugin::AnthropicPlugin;
pub use provider::AnthropicProvider;
