//! Plugins providing support for various model provider backends.
//!
//! Each provider is packaged as a standalone plugin. When added to the server, they register themselves with the [`ModelRegistry`](polaris_models::ModelRegistry), allowing standardized access to different model providers.
//!
//! # Supported Providers
//!
//! | Provider | Feature Flag | Description |
//! |----------|--------------|-------------|
//! | Anthropic | `anthropic` (default) | Direct Anthropic API access |
//! | AWS Bedrock | `bedrock` | AWS Bedrock Converse API |
//!
//! # Feature Flags
//!
//! Each provider is gated behind a feature flag to avoid pulling in unnecessary dependencies.
//!
//! ```toml
//! # Enable only Anthropic (default)
//! polaris_model_providers = { path = "../polaris_model_providers" }
//!
//! # Enable only Bedrock
//! polaris_model_providers = { path = "../polaris_model_providers", default-features = false, features = ["bedrock"] }
//!
//! # Enable both providers
//! polaris_model_providers = { path = "../polaris_model_providers", features = ["bedrock"] }
//! ```
//!
//! # Usage
//!
//! All provider plugins depend on [`ModelsPlugin`](polaris_models::ModelsPlugin), which must be added first to register the [`ModelRegistry`](polaris_models::ModelRegistry) resource.
//!
//! ```ignore
//! use polaris_model_providers::AnthropicPlugin;
//! use polaris_models::ModelsPlugin;
//! use polaris_system::server::Server;
//!
//! let mut server = Server::new();
//! server.add_plugins(ModelsPlugin);
//! server.add_plugins(AnthropicPlugin::from_env("ANTHROPIC_API_KEY"));
//! ```
//!
//! For AWS Bedrock, credentials are loaded from the default AWS credential chain:
//!
//! ```ignore
//! use polaris_model_providers::BedrockPlugin;
//! use polaris_models::ModelsPlugin;
//! use polaris_system::server::Server;
//!
//! let mut server = Server::new();
//! server.add_plugins(ModelsPlugin);
//! server.add_plugins(BedrockPlugin::from_env());
//! ```

mod schema;

#[cfg(feature = "anthropic")]
pub mod anthropic;

#[cfg(feature = "anthropic")]
pub use anthropic::AnthropicPlugin;

#[cfg(feature = "bedrock")]
pub mod bedrock;

#[cfg(feature = "bedrock")]
pub use bedrock::BedrockPlugin;
