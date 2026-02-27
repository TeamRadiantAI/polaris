//! Tracing and observability plugin.
//!
//! Provides [`TracingPlugin`] which configures the `tracing` subscriber and
//! exposes configuration as a resource.
//!
//! # Lifecycle
//!
//! - **`build()`** registers the [`TracingConfig`] resource so other plugins
//!   can read the intended configuration during build.
//! - **`ready()`** initializes the tracing subscriber. This deferred
//!   initialization allows other plugins (or [`DefaultPlugins`](crate::DefaultPlugins)
//!   configuration) to influence tracing settings before the subscriber is
//!   installed.
//!
//! # Example
//!
//! ```
//! use polaris_system::server::Server;
//! use polaris_system::param::Res;
//! use polaris_system::system;
//! use polaris_core_plugins::{ServerInfoPlugin, TracingPlugin, TracingConfig, TracingFormat};
//! use tracing::Level;
//!
//! #[system]
//! async fn logged_operation(config: Res<TracingConfig>) {
//!     tracing::info!("Starting operation");
//!
//!     if config.level <= Level::DEBUG {
//!         tracing::debug!("Detailed debug information...");
//!     }
//!
//!     tracing::info!("Operation complete");
//! }
//!
//! let mut server = Server::new();
//! server.add_plugins(ServerInfoPlugin);
//! server.add_plugins(
//!     TracingPlugin::default()
//!         .with_level(Level::DEBUG)
//!         .with_format(TracingFormat::Pretty)
//! );
//! server.finish();
//! ```

use crate::ServerInfoPlugin;
use polaris_system::plugin::{Plugin, PluginId, Version};
use polaris_system::resource::GlobalResource;
use polaris_system::server::Server;
use tracing::Level;
use tracing_subscriber::EnvFilter;
use tracing_subscriber::fmt::format::FmtSpan;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;

// ─────────────────────────────────────────────────────────────────────────────
// TracingFormat
// ─────────────────────────────────────────────────────────────────────────────

/// Tracing output format.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum TracingFormat {
    /// Human-readable colored output (default).
    #[default]
    Pretty,
    /// Compact single-line output.
    Compact,
    /// JSON structured output for log aggregation.
    Json,
}

// ─────────────────────────────────────────────────────────────────────────────
// TracingConfig Resource
// ─────────────────────────────────────────────────────────────────────────────

/// Tracing configuration resource.
///
/// Global resource exposing the tracing configuration. Systems can read
/// this to adapt their logging behavior based on the configured log level.
///
/// # Fields
///
/// - `level` - The configured maximum log level
/// - `format` - The output format (Pretty, Compact, or Json)
///
/// # Example
///
/// ```
/// use polaris_system::param::Res;
/// use polaris_system::system;
/// use polaris_core_plugins::TracingConfig;
/// use tracing::Level;
///
/// #[system]
/// async fn adaptive_logging(config: Res<TracingConfig>) {
///     // Always log at info level
///     tracing::info!("Processing request");
///
///     // Only emit debug logs if debug level is enabled
///     if config.level <= Level::DEBUG {
///         tracing::debug!(
///             format = ?config.format,
///             "Debug mode active, using {:?} format",
///             config.format
///         );
///     }
/// }
/// ```
#[derive(Debug, Clone, Copy)]
pub struct TracingConfig {
    /// The configured log level.
    pub level: Level,
    /// The configured output format.
    pub format: TracingFormat,
}

impl GlobalResource for TracingConfig {}

// ─────────────────────────────────────────────────────────────────────────────
// TracingPlugin
// ─────────────────────────────────────────────────────────────────────────────

/// Tracing and logging plugin.
///
/// Configures the `tracing` subscriber and provides observability resources.
/// Uses the [`tracing`] and [`tracing_subscriber`] crates under the hood.
///
/// # Resources Provided
///
/// | Resource | Scope | Description |
/// |----------|-------|-------------|
/// | [`TracingConfig`] | Global | Tracing configuration (read-only) |
///
/// # Dependencies
///
/// - [`ServerInfoPlugin`]
///
/// # Example
///
/// ```
/// use polaris_system::server::Server;
/// use polaris_core_plugins::{ServerInfoPlugin, TracingPlugin, TracingFormat};
/// use tracing::Level;
///
/// let mut server = Server::new();
/// server.add_plugins(ServerInfoPlugin);
/// server.add_plugins(
///     TracingPlugin::default()
///         .with_level(Level::DEBUG)
///         .with_format(TracingFormat::Json)
/// );
/// server.finish();
/// ```
///
/// # Configuration Options
///
/// ```
/// use polaris_core_plugins::{TracingPlugin, TracingFormat};
/// use tracing::Level;
///
/// // Development: Pretty colored output with debug level
/// let dev_plugin = TracingPlugin::default()
///     .with_level(Level::DEBUG)
///     .with_format(TracingFormat::Pretty)
///     .with_span_events(true);  // Show span enter/exit
///
/// // Production: JSON output for log aggregation
/// let prod_plugin = TracingPlugin::default()
///     .with_level(Level::INFO)
///     .with_format(TracingFormat::Json)
///     .with_env_filter("polaris=info,hyper=warn,tower=warn");
/// ```
///
/// # Environment Filter
///
/// Use `with_env_filter` to set target-specific log levels:
///
/// ```
/// use polaris_core_plugins::TracingPlugin;
///
/// TracingPlugin::default()
///     .with_env_filter("polaris=debug,hyper=warn,tower=info")
/// # ;
/// ```
#[derive(Clone)]
pub struct TracingPlugin {
    /// Maximum log level.
    level: Level,
    /// Output format.
    format: TracingFormat,
    /// Environment filter (e.g., "polaris=debug,hyper=warn").
    env_filter: Option<String>,
    /// Whether to include span events (enter/exit).
    span_events: bool,
}

impl Default for TracingPlugin {
    fn default() -> Self {
        Self {
            level: Level::INFO,
            format: TracingFormat::Pretty,
            env_filter: None,
            span_events: false,
        }
    }
}

impl TracingPlugin {
    /// Creates a new `TracingPlugin` with default settings.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets the maximum log level.
    #[must_use]
    pub fn with_level(mut self, level: Level) -> Self {
        self.level = level;
        self
    }

    /// Sets the output format.
    #[must_use]
    pub fn with_format(mut self, format: TracingFormat) -> Self {
        self.format = format;
        self
    }

    /// Sets a custom environment filter string.
    ///
    /// Format: `target=level,target=level,...`
    ///
    /// # Example
    ///
    /// ```
    /// use polaris_core_plugins::TracingPlugin;
    ///
    /// TracingPlugin::new()
    ///     .with_env_filter("polaris=debug,hyper=warn,tower=info");
    /// ```
    #[must_use]
    pub fn with_env_filter(mut self, filter: impl Into<String>) -> Self {
        self.env_filter = Some(filter.into());
        self
    }

    /// Enables span enter/exit events in output.
    #[must_use]
    pub fn with_span_events(mut self, enabled: bool) -> Self {
        self.span_events = enabled;
        self
    }
}

impl Plugin for TracingPlugin {
    const ID: &'static str = "polaris::tracing";
    const VERSION: Version = Version::new(0, 0, 1);

    fn build(&self, server: &mut Server) {
        // Expose configuration as global resource.
        // Subscriber initialization happens at ready() so that
        // DefaultPlugins (or other configuration) can influence settings
        // before the subscriber is installed.
        server.insert_global(TracingConfig {
            level: self.level,
            format: self.format,
        });
    }

    fn ready(&self, _server: &mut Server) {
        // Build the environment filter
        let env_filter = match &self.env_filter {
            Some(filter) => {
                EnvFilter::try_new(filter).unwrap_or_else(|_| EnvFilter::new(self.level.as_str()))
            }
            None => EnvFilter::new(self.level.as_str()),
        };

        // Build span events configuration
        let span_events = if self.span_events {
            FmtSpan::ENTER | FmtSpan::EXIT
        } else {
            FmtSpan::NONE
        };

        // Initialize subscriber based on format
        // Note: try_init().ok() ignores errors if already initialized
        match self.format {
            TracingFormat::Pretty => {
                tracing_subscriber::registry()
                    .with(env_filter)
                    .with(
                        tracing_subscriber::fmt::layer()
                            .pretty()
                            .with_span_events(span_events),
                    )
                    .try_init()
                    .ok();
            }
            TracingFormat::Compact => {
                tracing_subscriber::registry()
                    .with(env_filter)
                    .with(
                        tracing_subscriber::fmt::layer()
                            .compact()
                            .with_span_events(span_events),
                    )
                    .try_init()
                    .ok();
            }
            TracingFormat::Json => {
                tracing_subscriber::registry()
                    .with(env_filter)
                    .with(
                        tracing_subscriber::fmt::layer()
                            .json()
                            .with_span_events(span_events),
                    )
                    .try_init()
                    .ok();
            }
        }

        tracing::info!(
            level = %self.level,
            format = ?self.format,
            "TracingPlugin initialized"
        );
    }

    fn cleanup(&self, _server: &mut Server) {
        tracing::info!("TracingPlugin shutting down");
    }

    fn dependencies(&self) -> Vec<PluginId> {
        vec![PluginId::of::<ServerInfoPlugin>()]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tracing_format_default_is_pretty() {
        let format = TracingFormat::default();
        assert_eq!(format, TracingFormat::Pretty);
    }

    #[test]
    fn tracing_plugin_default_level_is_info() {
        let plugin = TracingPlugin::default();
        assert_eq!(plugin.level, Level::INFO);
    }

    #[test]
    fn tracing_plugin_with_level() {
        let plugin = TracingPlugin::new().with_level(Level::DEBUG);
        assert_eq!(plugin.level, Level::DEBUG);
    }

    #[test]
    fn tracing_plugin_with_format() {
        let plugin = TracingPlugin::new().with_format(TracingFormat::Json);
        assert_eq!(plugin.format, TracingFormat::Json);
    }

    #[test]
    fn tracing_plugin_with_env_filter() {
        let plugin = TracingPlugin::new().with_env_filter("polaris=debug");
        assert_eq!(plugin.env_filter, Some("polaris=debug".to_string()));
    }

    #[test]
    fn tracing_plugin_with_span_events() {
        let plugin = TracingPlugin::new().with_span_events(true);
        assert!(plugin.span_events);
    }

    #[test]
    fn tracing_plugin_registers_resource() {
        let mut server = Server::new();
        server.add_plugins(ServerInfoPlugin);
        server.add_plugins(TracingPlugin::default());
        server.finish();

        let ctx = server.create_context();
        assert!(ctx.contains_resource::<TracingConfig>());
    }
}
