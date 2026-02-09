//! Edge types for agent graphs.
//!
//! Edges are the connections between nodes, defining control flow
//! through the agent graph.

use core::fmt;

use crate::node::NodeId;

/// Unique identifier for an edge in the graph.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct EdgeId(pub(crate) usize);

impl EdgeId {
    /// Creates a new edge ID.
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

impl fmt::Display for EdgeId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "edge_{}", self.0)
    }
}

/// A connection between nodes defining control flow.
///
/// Edges determine how execution flows through the graph after
/// a node completes.
#[derive(Debug)]
pub enum Edge {
    /// A -> B, output flows to input.
    Sequential(SequentialEdge),
    /// A -> B if predicate, else A -> C.
    Conditional(ConditionalEdge),
    /// A -> [B, C, D] concurrently.
    Parallel(ParallelEdge),
    /// Return to earlier node in graph.
    LoopBack(LoopBackEdge),
    /// Fallback path on failure.
    Error(ErrorEdge),
    /// Fallback path on timeout.
    Timeout(TimeoutEdge),
}

impl Edge {
    /// Returns the edge's ID.
    #[must_use]
    pub fn id(&self) -> EdgeId {
        match self {
            Edge::Sequential(edge) => edge.id,
            Edge::Conditional(edge) => edge.id,
            Edge::Parallel(edge) => edge.id,
            Edge::LoopBack(edge) => edge.id,
            Edge::Error(edge) => edge.id,
            Edge::Timeout(edge) => edge.id,
        }
    }

    /// Returns the source node ID.
    #[must_use]
    pub fn from(&self) -> NodeId {
        match self {
            Edge::Sequential(edge) => edge.from,
            Edge::Conditional(edge) => edge.from,
            Edge::Parallel(edge) => edge.from,
            Edge::LoopBack(edge) => edge.from,
            Edge::Error(edge) => edge.from,
            Edge::Timeout(edge) => edge.from,
        }
    }
}

/// A sequential edge: A -> B.
///
/// The simplest edge type, connecting one node to another
/// with output flowing directly to input.
#[derive(Debug)]
pub struct SequentialEdge {
    /// Unique identifier for this edge.
    pub id: EdgeId,
    /// Source node ID.
    pub from: NodeId,
    /// Destination node ID.
    pub to: NodeId,
}

impl SequentialEdge {
    /// Creates a new sequential edge.
    #[must_use]
    pub fn new(id: EdgeId, from: NodeId, to: NodeId) -> Self {
        Self { id, from, to }
    }
}

/// A conditional edge: A -> B if true, else A -> C.
///
/// Used with `DecisionNode` to implement binary branching.
#[derive(Debug)]
pub struct ConditionalEdge {
    /// Unique identifier for this edge.
    pub id: EdgeId,
    /// Source node ID (typically a `DecisionNode`).
    pub from: NodeId,
    /// Destination if condition is true.
    pub true_target: NodeId,
    /// Destination if condition is false.
    pub false_target: NodeId,
}

impl ConditionalEdge {
    /// Creates a new conditional edge.
    #[must_use]
    pub fn new(id: EdgeId, from: NodeId, true_target: NodeId, false_target: NodeId) -> Self {
        Self {
            id,
            from,
            true_target,
            false_target,
        }
    }
}

/// A parallel edge: A -> [B, C, D] concurrently.
///
/// Used with `ParallelNode` to fork execution into multiple paths.
#[derive(Debug)]
pub struct ParallelEdge {
    /// Unique identifier for this edge.
    pub id: EdgeId,
    /// Source node ID (typically a `ParallelNode`).
    pub from: NodeId,
    /// Destination node IDs for each parallel branch.
    pub targets: Vec<NodeId>,
}

impl ParallelEdge {
    /// Creates a new parallel edge.
    #[must_use]
    pub fn new(id: EdgeId, from: NodeId, targets: Vec<NodeId>) -> Self {
        Self { id, from, targets }
    }
}

/// A loop-back edge: return to earlier node.
///
/// Used with `LoopNode` to implement iteration.
#[derive(Debug)]
pub struct LoopBackEdge {
    /// Unique identifier for this edge.
    pub id: EdgeId,
    /// Source node ID (end of loop body).
    pub from: NodeId,
    /// Target node ID (loop entry point).
    pub to: NodeId,
}

impl LoopBackEdge {
    /// Creates a new loop-back edge.
    #[must_use]
    pub fn new(id: EdgeId, from: NodeId, to: NodeId) -> Self {
        Self { id, from, to }
    }
}

/// An error edge: fallback path on failure.
///
/// Provides an alternative execution path when a node fails.
#[derive(Debug)]
pub struct ErrorEdge {
    /// Unique identifier for this edge.
    pub id: EdgeId,
    /// Source node ID (the node that may fail).
    pub from: NodeId,
    /// Target node ID (error handler).
    pub to: NodeId,
}

impl ErrorEdge {
    /// Creates a new error edge.
    #[must_use]
    pub fn new(id: EdgeId, from: NodeId, to: NodeId) -> Self {
        Self { id, from, to }
    }
}

/// A timeout edge: fallback path on timeout.
///
/// Provides an alternative execution path when a node times out.
#[derive(Debug)]
pub struct TimeoutEdge {
    /// Unique identifier for this edge.
    pub id: EdgeId,
    /// Source node ID (the node that may timeout).
    pub from: NodeId,
    /// Target node ID (timeout handler).
    pub to: NodeId,
}

impl TimeoutEdge {
    /// Creates a new timeout edge.
    #[must_use]
    pub fn new(id: EdgeId, from: NodeId, to: NodeId) -> Self {
        Self { id, from, to }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn edge_id_display() {
        let id = EdgeId::new(42);
        assert_eq!(format!("{id}"), "edge_42");
    }

    #[test]
    fn edge_id_equality() {
        let id1 = EdgeId::new(1);
        let id2 = EdgeId::new(1);
        let id3 = EdgeId::new(2);

        assert_eq!(id1, id2);
        assert_ne!(id1, id3);
    }

    #[test]
    fn sequential_edge_creation() {
        let edge = SequentialEdge::new(EdgeId::new(0), NodeId::new(1), NodeId::new(2));
        assert_eq!(edge.id.index(), 0);
        assert_eq!(edge.from.index(), 1);
        assert_eq!(edge.to.index(), 2);
    }

    #[test]
    fn edge_enum_accessors() {
        let seq = Edge::Sequential(SequentialEdge::new(
            EdgeId::new(0),
            NodeId::new(1),
            NodeId::new(2),
        ));
        assert_eq!(seq.id().index(), 0);
        assert_eq!(seq.from().index(), 1);
    }
}
