//! Core infrastructure plugins for Polaris.
//!
//! This crate provides foundational plugins that most Polaris applications need:
//!
//! - [`ServerInfoPlugin`] - Server metadata and runtime information
//! - [`TimePlugin`] - Time utilities with mockable clock for testing
//! - [`TracingPlugin`] - Logging and observability via the `tracing` crate
//! - [`DefaultPlugins`] - Convenient bundle of all infrastructure plugins
//!
//! # Feature Flags
//!
//! - `test-utils` - Enables [`MockClock`] for deterministic time testing
//!
//! # Example
//!
//! ```no_run
//! use polaris_system::server::Server;
//! use polaris_system::plugin::PluginGroup;
//! use polaris_core::DefaultPlugins;
//!
//! Server::new()
//!     .add_plugins(DefaultPlugins.build())
//!     .run();
//! ```
//!
//! # Individual Plugin Usage
//!
//! For fine-grained control, add plugins individually:
//!
//! ```
//! use polaris_system::server::Server;
//! use polaris_core::{ServerInfoPlugin, TimePlugin, TracingPlugin};
//! use tracing::Level;
//!
//! Server::new()
//!     .add_plugins(ServerInfoPlugin)
//!     .add_plugins(TimePlugin::default())
//!     .add_plugins(TracingPlugin::default().with_level(Level::DEBUG))
//!     .run();
//! ```
//!
//! # Architecture
//!
//! This crate is part of Layer 1 infrastructure:
//!
//! - **Layer 1** (`polaris_system`, `polaris_core`): Core primitives and infrastructure
//! - **Layer 2** (`polaris_graph`, `polaris_agent`): Graph execution and agent patterns
//! - **Layer 3** (plugins): Concrete agent implementations

mod server_info;
mod time;
mod tracing_plugin;

// Re-export plugins
pub use server_info::ServerInfoPlugin;
pub use time::{Clock, ClockProvider, Stopwatch, TimePlugin};
pub use tracing_plugin::{TracingFormat, TracingPlugin};

// Re-export test utilities
#[cfg(any(test, feature = "test-utils"))]
pub use time::MockClock;

// Re-export resources
pub use server_info::ServerInfo;
pub use tracing_plugin::TracingConfig;

use polaris_system::plugin::{PluginGroup, PluginGroupBuilder};

/// Default plugins for most Polaris applications.
///
/// Includes:
/// - [`ServerInfoPlugin`] - Server metadata
/// - [`TimePlugin`] - Time utilities
/// - [`TracingPlugin`] - Logging and observability
///
/// # Example
///
/// ```no_run
/// use polaris_system::server::Server;
/// use polaris_system::plugin::PluginGroup;
/// use polaris_core::DefaultPlugins;
///
/// Server::new()
///     .add_plugins(DefaultPlugins.build())
///     .run();
/// ```
///
/// # Customization
///
/// Use the builder pattern to customize:
///
/// ```ignore
/// Server::new()
///     .add_plugins(
///         DefaultPlugins
///             .build()
///             .disable::<TracingPlugin>()
///     )
///     .run();
/// ```
pub struct DefaultPlugins;

impl PluginGroup for DefaultPlugins {
    fn build(self) -> PluginGroupBuilder {
        PluginGroupBuilder::new()
            .add(ServerInfoPlugin)
            .add(TimePlugin::default())
            .add(TracingPlugin::default())
    }
}

/// Minimal plugins for headless or testing scenarios.
///
/// Includes only:
/// - [`ServerInfoPlugin`] - Server metadata
/// - [`TimePlugin`] - Time utilities
///
/// Does not include tracing, making it suitable for unit tests
/// that don't need logging output.
///
/// # Example
///
/// ```no_run
/// use polaris_system::server::Server;
/// use polaris_system::plugin::PluginGroup;
/// use polaris_core::MinimalPlugins;
///
/// Server::new()
///     .add_plugins(MinimalPlugins.build())
///     .run();
/// ```
pub struct MinimalPlugins;

impl PluginGroup for MinimalPlugins {
    fn build(self) -> PluginGroupBuilder {
        PluginGroupBuilder::new()
            .add(ServerInfoPlugin)
            .add(TimePlugin::default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use polaris_system::server::Server;

    #[test]
    fn default_plugins_builds() {
        let builder = DefaultPlugins.build();
        assert_eq!(builder.len(), 3);
    }

    #[test]
    fn minimal_plugins_builds() {
        let builder = MinimalPlugins.build();
        assert_eq!(builder.len(), 2);
    }

    #[test]
    fn server_with_minimal_plugins() {
        let mut server = Server::new();
        server.add_plugins(MinimalPlugins.build());
        server.finish();

        // Verify ServerInfo resource is available
        let ctx = server.create_context();
        assert!(ctx.contains_resource::<ServerInfo>());
        assert!(ctx.contains_resource::<Clock>());
    }
}
