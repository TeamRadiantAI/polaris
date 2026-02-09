//! AWS Bedrock provider backend.
//!
//! Uses the AWS Bedrock Converse API.
//!
//! # Examples
//!
//! ```ignore
//! // Using default AWS credential chain
//! server.add_plugins(BedrockPlugin::from_env());
//!
//! // With custom SDK config
//! let sdk_config = aws_config::defaults(aws_config::BehaviorVersion::latest())
//!     .region("us-west-2")
//!     .load()
//!     .await;
//! server.add_plugins(BedrockPlugin::from_sdk_config(sdk_config));
//! ```

mod plugin;
mod provider;
mod request;
mod response;
mod types;

pub use plugin::BedrockPlugin;
pub use provider::BedrockProvider;
