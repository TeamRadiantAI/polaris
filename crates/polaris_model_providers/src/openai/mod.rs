//! `OpenAI` provider backend.
//!
//! Uses the `OpenAI` Responses API.

mod plugin;
mod provider;

pub use plugin::OpenAiPlugin;
pub use provider::OpenAiProvider;
