//! Agent trait for defining reusable behavior patterns.
//!
//! The `Agent` trait provides a way to encapsulate agent behavior as a
//! reusable graph structure. Layer 3 implementations (`ReAct`, `ReWOO`, etc.)
//! implement this trait to define specific agent patterns.
//!
//! # Architecture
//!
//! This crate provides the pattern definition layer:
//!
//! - **`polaris_graph`**: Core graph primitives (Graph, Node, Edge, `GraphExecutor`)
//! - **`polaris_agent`**: Agent pattern definition (this crate)
//! - **Layer 3 plugins**: Concrete agent implementations (`ReAct`, `ReWOO`, etc.)
//!
//! # Example
//!
//! ```
//! use polaris_agent::{Agent, AgentExt};
//! use polaris_graph::Graph;
//! use polaris_system::system;
//!
//! # async fn reason() {}
//! # async fn decide() {}
//! # async fn respond() {}
//!
//! struct SimpleAgent {
//!     max_iterations: usize,
//! }
//!
//! impl Agent for SimpleAgent {
//!     fn build(&self, graph: &mut Graph) {
//!         graph
//!             .add_system(reason)
//!             .add_system(decide)
//!             .add_system(respond);
//!     }
//!
//!     fn name(&self) -> &str {
//!         "SimpleAgent"
//!     }
//! }
//!
//! // Convert agent to graph
//! let agent = SimpleAgent { max_iterations: 10 };
//! let graph = agent.to_graph();
//! ```

use polaris_graph::graph::Graph;

/// Defines an agent's behavior as a graph of systems.
///
/// Implement this trait to create reusable agent patterns. Each agent
/// defines its behavior by building a graph of systems and control flow
/// constructs.
///
/// # Example
///
/// ```
/// use polaris_agent::{Agent, AgentExt};
/// use polaris_graph::Graph;
/// use polaris_system::system;
///
/// # async fn reason() {}
/// # async fn decide() {}
/// # async fn respond() {}
///
/// struct SimpleAgent {
///     max_iterations: usize,
/// }
///
/// impl Agent for SimpleAgent {
///     fn build(&self, graph: &mut Graph) {
///         graph
///             .add_system(reason)
///             .add_system(decide)
///             .add_system(respond);
///     }
///
///     fn name(&self) -> &str {
///        "SimpleAgent"
///     }
/// }
/// ```
///
/// # Design Notes
///
/// - Agents are **builders**, not executors. They construct graphs that
///   will be executed by a separate executor component.
/// - Agents should be `Send + Sync` to allow concurrent graph building.
/// - The `build` method receives a mutable reference to allow agents to
///   conditionally construct different graph structures based on config.
pub trait Agent: Send + Sync + 'static {
    /// Builds the directed graph of systems that defines this agent's behavior.
    ///
    /// This method is called once when the agent is registered with the server.
    /// The graph structure becomes the source of truth for the agent's behavior.
    ///
    /// # Arguments
    ///
    /// * `graph` - The graph builder to construct the agent's behavior.
    fn build(&self, graph: &mut Graph);

    /// Returns the agent's name for debugging and tracing.
    ///
    /// Defaults to the type name.
    fn name(&self) -> &str {
        core::any::type_name::<Self>()
    }
}

/// Extension trait for creating graphs from agents.
pub trait AgentExt: Agent {
    /// Builds and returns the agent's graph.
    ///
    /// Convenience method that creates a new graph and calls `build`.
    fn to_graph(&self) -> Graph {
        let mut graph = Graph::new();
        self.build(&mut graph);
        graph
    }
}

// Blanket implementation for all agents
impl<T: Agent> AgentExt for T {}

#[cfg(test)]
mod tests {
    use super::*;

    // Test system functions
    async fn step_one() -> i32 {
        1
    }

    async fn step_two() -> i32 {
        2
    }

    async fn step_three() -> i32 {
        3
    }

    async fn single_step() -> String {
        "step".to_string()
    }

    struct ThreeStepAgent;

    impl Agent for ThreeStepAgent {
        fn build(&self, graph: &mut Graph) {
            graph
                .add_system(step_one)
                .add_system(step_two)
                .add_system(step_three);
        }

        fn name(&self) -> &str {
            "ThreeStepAgent"
        }
    }

    #[test]
    fn agent_builds_graph() {
        let agent = ThreeStepAgent;
        let graph = agent.to_graph();

        assert_eq!(graph.node_count(), 3);
        assert!(graph.entry().is_some());
    }

    #[test]
    fn agent_name() {
        let agent = ThreeStepAgent;
        assert_eq!(agent.name(), "ThreeStepAgent");
    }

    #[test]
    fn agent_default_name() {
        struct UnnamedAgent;

        impl Agent for UnnamedAgent {
            fn build(&self, graph: &mut Graph) {
                graph.add_system(single_step);
            }
        }

        let agent = UnnamedAgent;
        assert!(agent.name().contains("UnnamedAgent"));
    }
}
