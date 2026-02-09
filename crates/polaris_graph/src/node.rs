//! Node types for agent graphs.
//!
//! Nodes are the vertices in an agent graph, representing units of computation
//! or control flow decisions.

use core::any::TypeId;
use core::fmt;

use polaris_system::system::{BoxedSystem, ErasedSystem};

use crate::predicate::BoxedPredicate;

/// Unique identifier for a node in the graph.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct NodeId(pub(crate) usize);

impl NodeId {
    /// Creates a new node ID.
    #[must_use]
    pub fn new(id: usize) -> Self {
        Self(id)
    }

    /// Returns the raw ID value.
    #[must_use]
    pub fn index(&self) -> usize {
        self.0
    }
}

impl fmt::Display for NodeId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "node_{}", self.0)
    }
}

/// A node in the agent graph.
///
/// Each node represents either a computation unit (system) or a control flow
/// construct (decision, loop, parallel execution).
#[derive(Debug)]
pub enum Node {
    /// Executes a system function.
    System(SystemNode),
    /// Routes flow based on predicate (binary branch).
    Decision(DecisionNode),
    /// Routes flow based on discriminator (multi-way branch).
    Switch(SwitchNode),
    /// Executes multiple paths concurrently.
    Parallel(ParallelNode),
    /// Repeats subgraph until termination condition.
    Loop(LoopNode),
    /// Aggregates results from parallel paths.
    Join(JoinNode),
}

impl Node {
    /// Returns the node's ID.
    #[must_use]
    pub fn id(&self) -> NodeId {
        match self {
            Node::System(n) => n.id,
            Node::Decision(n) => n.id,
            Node::Switch(n) => n.id,
            Node::Parallel(n) => n.id,
            Node::Loop(n) => n.id,
            Node::Join(n) => n.id,
        }
    }

    /// Returns the node's name.
    #[must_use]
    pub fn name(&self) -> &str {
        match self {
            Node::System(n) => n.name(),
            Node::Decision(n) => n.name,
            Node::Switch(n) => n.name,
            Node::Parallel(n) => n.name,
            Node::Loop(n) => n.name,
            Node::Join(n) => n.name,
        }
    }
}

/// A node that executes a system function.
///
/// This is the most common node type, wrapping an async system function
/// that performs computation (LLM calls, tool invocations, etc.).
pub struct SystemNode {
    /// Unique identifier for this node.
    pub id: NodeId,
    /// The boxed system to execute.
    pub system: BoxedSystem,
    /// Optional timeout for this system's execution.
    /// If set and exceeded, the executor will follow any timeout edge if present.
    pub timeout: Option<core::time::Duration>,
}

impl SystemNode {
    /// Creates a new system node from any type implementing [`ErasedSystem`].
    #[must_use]
    pub fn new<S: ErasedSystem>(id: NodeId, system: S) -> Self {
        Self {
            id,
            system: Box::new(system),
            timeout: None,
        }
    }

    /// Creates a new system node from an already-boxed system.
    #[must_use]
    pub fn new_boxed(id: NodeId, system: BoxedSystem) -> Self {
        Self {
            id,
            system,
            timeout: None,
        }
    }

    /// Sets the timeout for this system node.
    #[must_use]
    pub fn with_timeout(mut self, timeout: core::time::Duration) -> Self {
        self.timeout = Some(timeout);
        self
    }

    /// Returns the system's name for debugging and tracing.
    #[must_use]
    pub fn name(&self) -> &'static str {
        self.system.name()
    }

    /// Returns the [`TypeId`] of this system's output type.
    #[must_use]
    pub fn output_type_id(&self) -> TypeId {
        self.system.output_type_id()
    }

    /// Returns the output type name for error messages.
    #[must_use]
    pub fn output_type_name(&self) -> &'static str {
        self.system.output_type_name()
    }
}

impl fmt::Debug for SystemNode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SystemNode")
            .field("id", &self.id)
            .field("name", &self.name())
            .field("output_type", &self.output_type_name())
            .finish()
    }
}

/// A node that routes flow based on a boolean predicate.
///
/// Decision nodes implement binary branching: if the predicate returns true,
/// flow continues to the "true" branch; otherwise to the "false" branch.
pub struct DecisionNode {
    /// Unique identifier for this node.
    pub id: NodeId,
    /// Human-readable name for debugging and tracing.
    pub name: &'static str,
    /// The predicate that determines which branch to take.
    pub predicate: Option<BoxedPredicate>,
    /// Node ID for the true branch.
    pub true_branch: Option<NodeId>,
    /// Node ID for the false branch.
    pub false_branch: Option<NodeId>,
}

impl DecisionNode {
    /// Creates a new decision node.
    #[must_use]
    pub fn new(id: NodeId, name: &'static str) -> Self {
        Self {
            id,
            name,
            predicate: None,
            true_branch: None,
            false_branch: None,
        }
    }

    /// Creates a new decision node with a predicate.
    #[must_use]
    pub fn with_predicate(id: NodeId, name: &'static str, predicate: BoxedPredicate) -> Self {
        Self {
            id,
            name,
            predicate: Some(predicate),
            true_branch: None,
            false_branch: None,
        }
    }
}

impl fmt::Debug for DecisionNode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("DecisionNode")
            .field("id", &self.id)
            .field("name", &self.name)
            .field("has_predicate", &self.predicate.is_some())
            .field("true_branch", &self.true_branch)
            .field("false_branch", &self.false_branch)
            .finish()
    }
}

/// A node that routes flow based on a discriminator value (multi-way branch).
///
/// Switch nodes generalize decision nodes to handle multiple cases,
/// similar to a match/switch statement.
pub struct SwitchNode {
    /// Unique identifier for this node.
    pub id: NodeId,
    /// Human-readable name for debugging and tracing.
    pub name: &'static str,
    /// The discriminator that determines which case to take.
    pub discriminator: Option<crate::predicate::BoxedDiscriminator>,
    /// Node IDs for each case, keyed by case name.
    pub cases: Vec<(&'static str, NodeId)>,
    /// Default case if no match.
    pub default: Option<NodeId>,
}

impl SwitchNode {
    /// Creates a new switch node.
    #[must_use]
    pub fn new(id: NodeId, name: &'static str) -> Self {
        Self {
            id,
            name,
            discriminator: None,
            cases: Vec::new(),
            default: None,
        }
    }

    /// Creates a new switch node with a discriminator.
    #[must_use]
    pub fn with_discriminator(
        id: NodeId,
        name: &'static str,
        discriminator: crate::predicate::BoxedDiscriminator,
    ) -> Self {
        Self {
            id,
            name,
            discriminator: Some(discriminator),
            cases: Vec::new(),
            default: None,
        }
    }
}

impl fmt::Debug for SwitchNode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SwitchNode")
            .field("id", &self.id)
            .field("name", &self.name)
            .field("has_discriminator", &self.discriminator.is_some())
            .field("cases", &self.cases)
            .field("default", &self.default)
            .finish()
    }
}

/// A node that executes multiple paths concurrently.
///
/// Parallel nodes fork execution into multiple branches that run
/// simultaneously. Results are collected at a corresponding Join node.
#[derive(Debug)]
pub struct ParallelNode {
    /// Unique identifier for this node.
    pub id: NodeId,
    /// Human-readable name for debugging and tracing.
    pub name: &'static str,
    /// Node IDs for each parallel branch.
    pub branches: Vec<NodeId>,
    /// Node ID of the join node that collects results.
    pub join: Option<NodeId>,
}

impl ParallelNode {
    /// Creates a new parallel node.
    #[must_use]
    pub fn new(id: NodeId, name: &'static str) -> Self {
        Self {
            id,
            name,
            branches: Vec::new(),
            join: None,
        }
    }
}

/// A node that repeats a subgraph until a termination condition.
///
/// Loop nodes implement iterative execution patterns, repeating the
/// loop body until a termination predicate returns true or max iterations
/// is reached.
pub struct LoopNode {
    /// Unique identifier for this node.
    pub id: NodeId,
    /// Human-readable name for debugging and tracing.
    pub name: &'static str,
    /// The termination predicate (loop exits when this returns true).
    pub termination: Option<BoxedPredicate>,
    /// Maximum number of iterations (safety limit).
    pub max_iterations: Option<usize>,
    /// Entry point of the loop body.
    pub body_entry: Option<NodeId>,
}

impl LoopNode {
    /// Creates a new loop node.
    #[must_use]
    pub fn new(id: NodeId, name: &'static str) -> Self {
        Self {
            id,
            name,
            termination: None,
            max_iterations: None,
            body_entry: None,
        }
    }

    /// Creates a new loop node with a termination predicate.
    #[must_use]
    pub fn with_termination(id: NodeId, name: &'static str, termination: BoxedPredicate) -> Self {
        Self {
            id,
            name,
            termination: Some(termination),
            max_iterations: None,
            body_entry: None,
        }
    }

    /// Creates a new loop node with a maximum iteration count.
    #[must_use]
    pub fn with_max_iterations(id: NodeId, name: &'static str, max_iterations: usize) -> Self {
        Self {
            id,
            name,
            termination: None,
            max_iterations: Some(max_iterations),
            body_entry: None,
        }
    }
}

impl fmt::Debug for LoopNode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("LoopNode")
            .field("id", &self.id)
            .field("name", &self.name)
            .field("has_termination", &self.termination.is_some())
            .field("max_iterations", &self.max_iterations)
            .field("body_entry", &self.body_entry)
            .finish()
    }
}

/// A node that aggregates results from parallel paths.
///
/// Join nodes are the counterpart to Parallel nodes, collecting
/// results from all parallel branches before continuing execution.
#[derive(Debug)]
pub struct JoinNode {
    /// Unique identifier for this node.
    pub id: NodeId,
    /// Human-readable name for debugging and tracing.
    pub name: &'static str,
    /// Node IDs of the parallel branches being joined.
    pub sources: Vec<NodeId>,
}

impl JoinNode {
    /// Creates a new join node.
    #[must_use]
    pub fn new(id: NodeId, name: &'static str) -> Self {
        Self {
            id,
            name,
            sources: Vec::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use polaris_system::system::IntoSystem;

    // Test system functions
    async fn test_system() -> String {
        "hello".to_string()
    }

    async fn sys_fn() -> i32 {
        42
    }

    #[test]
    fn node_id_display() {
        let id = NodeId::new(42);
        assert_eq!(format!("{id}"), "node_42");
    }

    #[test]
    fn node_id_equality() {
        let id1 = NodeId::new(1);
        let id2 = NodeId::new(1);
        let id3 = NodeId::new(2);

        assert_eq!(id1, id2);
        assert_ne!(id1, id3);
    }

    #[test]
    fn system_node_creation() {
        let system = test_system.into_system();
        let node = SystemNode::new(NodeId::new(0), system);
        assert_eq!(node.id.index(), 0);
        assert!(node.name().contains("test_system"));
    }

    #[test]
    fn node_enum_accessors() {
        let system = Node::System(SystemNode::new(NodeId::new(1), sys_fn.into_system()));
        assert_eq!(system.id().index(), 1);
        assert!(system.name().contains("sys_fn"));

        let decision = Node::Decision(DecisionNode::new(NodeId::new(2), "dec"));
        assert_eq!(decision.id().index(), 2);
        assert_eq!(decision.name(), "dec");
    }

    #[test]
    fn system_node_preserves_type_info() {
        let system = sys_fn.into_system();
        let node = SystemNode::new(NodeId::new(0), system);

        assert_eq!(node.output_type_id(), TypeId::of::<i32>());
        assert!(node.output_type_name().contains("i32"));
    }

    #[test]
    fn system_node_debug() {
        let system = test_system.into_system();
        let node = SystemNode::new(NodeId::new(42), system);
        let debug_str = format!("{node:?}");

        assert!(debug_str.contains("SystemNode"));
        assert!(debug_str.contains("42"));
        assert!(debug_str.contains("test_system"));
    }
}
