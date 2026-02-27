//! AWS Bedrock provider backend.
//!
//! Uses the AWS Bedrock Converse API.
//!
//! # Examples
//!
//! ```no_run
//! # use polaris_model_providers::bedrock::BedrockPlugin;
//! # use polaris_system::server::Server;
//! # let mut server = Server::new();
//!
//! // Using default AWS credential chain
//! server.add_plugins(BedrockPlugin::from_env());
//!
//! // With custom SDK config
//! # tokio::runtime::Runtime::new().unwrap().block_on(async {
//! let sdk_config = aws_config::defaults(aws_config::BehaviorVersion::latest())
//!     .region("us-west-2")
//!     .load()
//!     .await;
//! server.add_plugins(BedrockPlugin::from_sdk_config(sdk_config));
//! # });
//! ```

mod plugin;
mod provider;
mod request;
mod response;
mod types;

pub use plugin::BedrockPlugin;
pub use provider::BedrockProvider;
