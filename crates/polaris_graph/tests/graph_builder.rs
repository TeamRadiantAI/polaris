//! Tests for the Graph builder API.
//!
//! These tests verify the graph construction functionality:
//! - Creating empty graphs
//! - Adding system nodes
//! - Sequential chaining
//! - Conditional branches
//! - Parallel branches
//! - Loops (predicate-based and iteration-based)
//! - Complex graph compositions

use polaris_graph::graph::Graph;
use polaris_graph::node::Node;

// ─────────────────────────────────────────────────────────────────────────────
// Test Systems
// ─────────────────────────────────────────────────────────────────────────────

async fn test_system() -> String {
    "hello".to_string()
}

async fn first_step() -> i32 {
    1
}

async fn second_step() -> i32 {
    2
}

async fn third_step() -> i32 {
    3
}

async fn before_decision() -> bool {
    true
}

async fn true_path_system() -> String {
    "true".to_string()
}

async fn false_path_system() -> String {
    "false".to_string()
}

async fn after_decision() -> String {
    "after".to_string()
}

async fn branch_a() -> i32 {
    1
}

async fn branch_b() -> i32 {
    2
}

async fn loop_body() -> i32 {
    42
}

async fn reason() -> String {
    "reasoning".to_string()
}

async fn invoke_tool() -> String {
    "tool_result".to_string()
}

async fn observe() -> String {
    "observed".to_string()
}

async fn respond() -> String {
    "response".to_string()
}

async fn finalize() -> String {
    "done".to_string()
}

// ─────────────────────────────────────────────────────────────────────────────
// Basic Graph Creation
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn new_graph_is_empty() {
    let graph = Graph::new();
    assert!(graph.is_empty());
    assert_eq!(graph.node_count(), 0);
    assert_eq!(graph.edge_count(), 0);
    assert!(graph.entry().is_none());
}

#[test]
fn add_single_system() {
    let mut graph = Graph::new();
    graph.add_system(test_system);

    assert_eq!(graph.node_count(), 1);
    assert_eq!(graph.edge_count(), 0);
    assert!(graph.entry().is_some());

    let node = graph.get_node(graph.entry().unwrap()).unwrap();
    // Name contains the function path
    assert!(node.name().contains("test_system"));
}

#[test]
fn add_sequential_systems() {
    let mut graph = Graph::new();
    graph
        .add_system(first_step)
        .add_system(second_step)
        .add_system(third_step);

    assert_eq!(graph.node_count(), 3);
    assert_eq!(graph.edge_count(), 2); // first->second, second->third
}

#[test]
fn system_node_stores_type_info() {
    use core::any::TypeId;

    let mut graph = Graph::new();
    graph.add_system(first_step); // returns i32

    let node = graph.get_node(graph.entry().unwrap()).unwrap();
    if let Node::System(sys_node) = node {
        assert_eq!(sys_node.output_type_id(), TypeId::of::<i32>());
        assert!(sys_node.output_type_name().contains("i32"));
    } else {
        panic!("Expected SystemNode");
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Conditional Branches
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
struct DecisionOutput {
    should_branch: bool,
}

async fn decision_system() -> DecisionOutput {
    DecisionOutput {
        should_branch: true,
    }
}

#[test]
fn add_conditional_branch() {
    let mut graph = Graph::new();
    graph
        .add_system(before_decision)
        .add_system(decision_system)
        .add_conditional_branch::<DecisionOutput, _, _, _>(
            "decision",
            |output| output.should_branch,
            |g| {
                g.add_system(true_path_system);
            },
            |g| {
                g.add_system(false_path_system);
            },
        )
        .add_system(after_decision);

    // Nodes: before, decision_system, decision, true_path, false_path, after
    assert!(graph.node_count() >= 5);
}

// ─────────────────────────────────────────────────────────────────────────────
// Parallel Branches
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn add_parallel_branches() {
    let mut graph = Graph::new();
    graph.add_parallel(
        "parallel",
        vec![
            |g: &mut Graph| {
                g.add_system(branch_a);
            },
            |g: &mut Graph| {
                g.add_system(branch_b);
            },
        ],
    );

    // Nodes: parallel, branch_a, branch_b, join
    assert!(graph.node_count() >= 4);
}

// ─────────────────────────────────────────────────────────────────────────────
// Loops
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
struct LoopState {
    #[expect(dead_code, reason = "used for testing struct completeness")]
    iteration: i32,
    done: bool,
}

async fn loop_init() -> LoopState {
    LoopState {
        iteration: 0,
        done: false,
    }
}

#[test]
fn add_loop_with_predicate() {
    let mut graph = Graph::new();
    graph.add_system(loop_init).add_loop::<LoopState, _, _>(
        "loop",
        |state| state.done,
        |g| {
            g.add_system(loop_body);
        },
    );

    // Nodes: loop_init, loop, loop_body
    assert!(graph.node_count() >= 3);
}

#[test]
fn add_loop_with_iterations() {
    let mut graph = Graph::new();
    graph.add_loop_n("loop", 10, |g| {
        g.add_system(loop_body);
    });

    // Nodes: loop, loop_body
    assert!(graph.node_count() >= 2);
}

// ─────────────────────────────────────────────────────────────────────────────
// Complex Graphs
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
struct ReasoningResult {
    needs_tool: bool,
}

async fn reasoning() -> ReasoningResult {
    ReasoningResult { needs_tool: true }
}

#[test]
fn complex_graph() {
    let mut graph = Graph::new();
    graph
        .add_system(reason)
        .add_system(reasoning)
        .add_conditional_branch::<ReasoningResult, _, _, _>(
            "needs_tool",
            |result| result.needs_tool,
            |g| {
                g.add_system(invoke_tool).add_system(observe);
            },
            |g| {
                g.add_system(respond);
            },
        )
        .add_system(finalize);

    assert!(!graph.is_empty());
    assert!(graph.entry().is_some());
}

// ─────────────────────────────────────────────────────────────────────────────
// ID Allocation Tests
// ─────────────────────────────────────────────────────────────────────────────

/// Verifies that the shared `IdAllocator` ensures unique IDs across all
/// subgraphs, regardless of nesting depth.
#[test]
fn no_id_collision_in_deep_nesting() {
    use polaris_graph::edge::Edge;
    use polaris_graph::node::Node;
    use std::collections::HashSet;

    let mut graph = Graph::new();

    // Build a deeply nested structure:
    // parallel -> [
    //   loop -> conditional -> [true_branch, false_branch],
    //   loop -> system
    // ]
    graph.add_parallel(
        "outer_parallel",
        vec![
            |g: &mut Graph| {
                g.add_loop_n("inner_loop_1", 3, |g| {
                    g.add_system(first_step)
                        .add_conditional_branch::<i32, _, _, _>(
                            "nested_decision",
                            |_| true,
                            |g| {
                                g.add_system(true_path_system);
                            },
                            |g| {
                                g.add_system(false_path_system);
                            },
                        );
                });
            },
            |g: &mut Graph| {
                g.add_loop_n("inner_loop_2", 2, |g| {
                    g.add_system(second_step);
                });
            },
            |g: &mut Graph| {
                g.add_system(third_step);
            },
        ],
    );

    // Collect all node IDs
    let node_ids: HashSet<_> = graph.nodes().iter().map(Node::id).collect();

    // All node IDs should be unique (set size equals node count)
    assert_eq!(
        node_ids.len(),
        graph.node_count(),
        "Node ID collision detected! Expected {} unique IDs but found {}",
        graph.node_count(),
        node_ids.len()
    );

    // Collect all edge IDs
    let edge_ids: HashSet<_> = graph.edges().iter().map(Edge::id).collect();

    // All edge IDs should be unique
    assert_eq!(
        edge_ids.len(),
        graph.edge_count(),
        "Edge ID collision detected! Expected {} unique IDs but found {}",
        graph.edge_count(),
        edge_ids.len()
    );
}

/// Verifies sequential ID allocation across subgraphs.
#[test]
fn ids_are_sequential_across_subgraphs() {
    let mut graph = Graph::new();

    // Add systems and a conditional branch
    graph
        .add_system(first_step)
        .add_system(second_step)
        .add_conditional_branch::<i32, _, _, _>(
            "branch",
            |_| true,
            |g| {
                g.add_system(true_path_system);
            },
            |g| {
                g.add_system(false_path_system);
            },
        )
        .add_system(third_step);

    // Collect and sort node IDs
    let mut node_ids: Vec<_> = graph.nodes().iter().map(|n| n.id().index()).collect();
    node_ids.sort();

    // Verify IDs are sequential (no gaps from old offset system)
    for i in 1..node_ids.len() {
        let gap = node_ids[i] - node_ids[i - 1];
        assert!(
            gap == 1,
            "Non-sequential node IDs: {} and {} (gap of {})",
            node_ids[i - 1],
            node_ids[i],
            gap
        );
    }
}
