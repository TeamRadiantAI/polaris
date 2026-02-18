//! Model provider interface and registry for Polaris.
//!
//! Provides a unified interface for AI model access, decoupling consumers from
//! provider implementations.
//!
//! # Overview
//!
//! - Provider-agnostic: Consumers depend only on this crate, not specific provider plugins.
//!
//! - Modular provider plugins: Provider crates register at runtime, allowing
//!   models to be swapped via configuration without code changes.
//!
//! - Minimal dependencies: Each provider lives in a separate crate.
//!
//! # Example
//!
//! ```ignore
//! use polaris_models::{ModelRegistry, ModelsPlugin};
//! use polaris_models::llm::{GenerationRequest, Message};
//! use polaris_system::param::Res;
//!
//! #[system]
//! async fn my_agent(registry: Res<ModelRegistry>) -> Response {
//!     let llm = registry.llm("openai/gpt-4o")?;
//!
//!     let request = GenerationRequest::with_system("You are helpful", "Hello!");
//!     let response = llm.generate(request).await?;
//!
//!     // ...
//! }
//! ```

// Self-reference so tool macros can use `polaris_models::` paths within this crate.
extern crate self as polaris_models;

pub mod error;
pub mod llm;
mod plugin;
mod registry;

pub use plugin::ModelsPlugin;
pub use registry::ModelRegistry;
