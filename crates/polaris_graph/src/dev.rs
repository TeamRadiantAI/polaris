//! Development tools for graph execution.
//!
//! The [`DevToolsPlugin`] injects [`SystemInfo`] before each system runs,
//! providing execution context for debugging and observability.
//!
//! # Example
//!
//! ```ignore
//! use polaris_graph::dev::SystemInfo;
//! use polaris_system::param::Res;
//! use polaris_system::system;
//!
//! #[system]
//! async fn my_system(info: Res<SystemInfo>) {
//!     tracing::info!(
//!         "Executing system '{}' on node {:?}",
//!         info.system_name(),
//!         info.node_id()
//!     );
//! }
//! ```
//!
//! # Setup
//!
//! Add `DevToolsPlugin` to your server:
//!
//! ```ignore
//! use polaris_graph::dev::DevToolsPlugin;
//! use polaris_system::server::Server;
//!
//! let mut server = Server::new();
//! server.add_plugins(DevToolsPlugin);
//! ```
//!
//! # Validation
//!
//! `SystemInfo` is recognized as a hook-provided resource. Systems that declare
//! `Res<SystemInfo>` will not fail resource validation, as we leverage
//! `register_provider` api to track provided resource types.

use crate::hooks::HooksAPI;
use crate::hooks::events::GraphEvent;
use crate::hooks::schedule::OnSystemStart;
use crate::node::NodeId;
use polaris_system::plugin::Plugin;
use polaris_system::resource::LocalResource;
use polaris_system::server::Server;

/// Execution context injected by [`DevToolsPlugin`] before each system runs.
///
/// This resource provides information about the currently executing system,
/// useful for logging, debugging, and observability.
///
/// # Thread Safety
///
/// `SystemInfo` is a [`LocalResource`], meaning each execution context has
/// its own copy. It is updated by the [`DevToolsPlugin`] hook before each
/// system call.
#[derive(Debug, Clone)]
pub struct SystemInfo {
    /// The ID of the node currently being executed.
    node_id: NodeId,
    /// The name of the system being executed.
    system_name: &'static str,
}

impl LocalResource for SystemInfo {}

impl SystemInfo {
    /// Creates a new `SystemInfo` with the given execution context.
    fn new(node_id: NodeId, system_name: &'static str) -> Self {
        Self {
            node_id,
            system_name,
        }
    }

    /// Returns the ID of the node currently being executed.
    #[must_use]
    pub fn node_id(&self) -> NodeId {
        self.node_id.clone()
    }

    /// Returns the name of the system currently being executed.
    #[must_use]
    pub fn system_name(&self) -> &'static str {
        self.system_name
    }
}

/// Plugin that injects [`SystemInfo`] before each system execution.
///
/// This plugin registers a hook on [`OnSystemStart`] that injects a
/// [`SystemInfo`] resource into the context, making execution metadata
/// available to systems via `Res<SystemInfo>`.
///
/// # Example
///
/// ```ignore
/// use polaris_graph::dev::DevToolsPlugin;
/// use polaris_system::server::Server;
///
/// let mut server = Server::new();
/// server.add_plugins(DevToolsPlugin);
/// ```
pub struct DevToolsPlugin;

impl Plugin for DevToolsPlugin {
    fn build(&self, server: &mut Server) {
        // Initialize HooksAPI if not present
        if !server.contains_api::<HooksAPI>() {
            server.insert_api(HooksAPI::new());
        }

        // Register provider hook to inject SystemInfo before each system.
        server
            .api::<HooksAPI>()
            .expect("HooksAPI should be present after initialization")
            .register_provider::<OnSystemStart, SystemInfo, _>(
                "devtools_system_info",
                |event: &GraphEvent| {
                    if let GraphEvent::SystemStart {
                        node_id,
                        system_name,
                    } = event
                    {
                        Some(SystemInfo::new(node_id.clone(), system_name))
                    } else {
                        None
                    }
                },
            )
            .expect("DevToolsPlugin hook registration should not fail");
    }
}
