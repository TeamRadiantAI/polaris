//! # Polaris Internal Library
//!
//! Re-exports the core Polaris crates for convenience.

/// Layer 1: ECS-inspired system framework.
pub use polaris_system;

/// Layer 2: Graph-based execution primitives.
pub use polaris_graph;

/// Layer 2: Agent pattern definition.
pub use polaris_agent;

/// Re-export all common types for easy access.
pub mod prelude {
    pub use polaris_agent::{Agent, AgentExt};
    pub use polaris_graph::prelude::*;
    pub use polaris_system::prelude::*;
}
