//! Tests for Graph validation.
//!
//! These tests verify the `Graph::validate()` functionality:
//! - Entry point validation
//! - Node reference validation
//! - Decision node requirements
//! - Parallel node requirements
//! - Loop node requirements
//! - Error display formatting

use polaris_graph::graph::{Graph, ValidationError, ValidationWarning};
use polaris_graph::node::NodeId;

// ─────────────────────────────────────────────────────────────────────────────
// Test Systems
// ─────────────────────────────────────────────────────────────────────────────

async fn first_step() -> i32 {
    1
}

async fn second_step() -> i32 {
    2
}

async fn true_path_system() -> String {
    "true".to_string()
}

async fn false_path_system() -> String {
    "false".to_string()
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

// ─────────────────────────────────────────────────────────────────────────────
// Entry Point Validation
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn validate_empty_graph_fails() {
    let graph = Graph::new();
    let result = graph.validate();

    assert!(result.is_err());
    let errors = result.unwrap_err();
    assert!(
        errors
            .iter()
            .any(|err| matches!(err, ValidationError::NoEntryPoint))
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// Valid Graph Structures
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn validate_simple_graph_succeeds() {
    let mut graph = Graph::new();
    graph.add_system(first_step).add_system(second_step);

    let result = graph.validate();
    assert!(result.is_ok(), "Validation failed: {:?}", result.err());
}

#[test]
fn validate_graph_with_conditional_branch_succeeds() {
    #[derive(Debug)]
    struct DecisionOutput {
        should_branch: bool,
    }

    async fn decision_system() -> DecisionOutput {
        DecisionOutput {
            should_branch: true,
        }
    }

    let mut graph = Graph::new();
    graph
        .add_system(decision_system)
        .add_conditional_branch::<DecisionOutput, _, _, _>(
            "branch",
            |output| output.should_branch,
            |g| {
                g.add_system(true_path_system);
            },
            |g| {
                g.add_system(false_path_system);
            },
        );

    let result = graph.validate();
    assert!(result.is_ok(), "Validation failed: {:?}", result.err());
}

#[test]
fn validate_graph_with_parallel_succeeds() {
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

    let result = graph.validate();
    assert!(result.is_ok(), "Validation failed: {:?}", result.err());
}

#[test]
fn validate_graph_with_loop_succeeds() {
    let mut graph = Graph::new();
    graph.add_loop_n("loop", 5, |g| {
        g.add_system(loop_body);
    });

    let result = graph.validate();
    assert!(result.is_ok(), "Validation failed: {:?}", result.err());
}

// ─────────────────────────────────────────────────────────────────────────────
// Error Display Formatting
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn validation_error_no_entry_point_display() {
    let err = ValidationError::NoEntryPoint;
    assert_eq!(format!("{err}"), "graph has no entry point");
}

#[test]
fn validation_error_invalid_entry_point_display() {
    let err = ValidationError::InvalidEntryPoint(NodeId::from_string("5"));
    let msg = format!("{err}");
    assert!(msg.contains("invalid node"));
    assert!(msg.contains("node_5"));
}

#[test]
fn validation_error_missing_predicate_display() {
    let err = ValidationError::MissingPredicate {
        node: NodeId::from_string("3"),
        name: "decision",
    };
    let msg = format!("{err}");
    assert!(msg.contains("decision"));
    assert!(msg.contains("missing predicate"));
}

#[test]
fn validation_error_missing_branch_display() {
    let err = ValidationError::MissingBranch {
        node: NodeId::from_string("2"),
        name: "choice",
        branch: "true",
    };
    let msg = format!("{err}");
    assert!(msg.contains("choice"));
    assert!(msg.contains("true branch"));
}

#[test]
fn validation_error_no_termination_condition_display() {
    let err = ValidationError::NoTerminationCondition {
        node: NodeId::from_string("1"),
        name: "infinite_loop",
    };
    let msg = format!("{err}");
    assert!(msg.contains("termination condition"));
    assert!(msg.contains("infinite_loop"));
}

// ─────────────────────────────────────────────────────────────────────────────
// Error Trait Implementation
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn validation_error_implements_error_trait() {
    fn assert_error<E: core::error::Error>() {}
    assert_error::<ValidationError>();
}

// ─────────────────────────────────────────────────────────────────────────────
// Parallel Output Conflict Detection
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn validate_parallel_conflicting_outputs_warns() {
    // Both branches produce the same output type (i32)
    let mut graph = Graph::new();
    graph.add_parallel(
        "conflict",
        vec![
            |g: &mut Graph| {
                g.add_system(branch_a);
            },
            |g: &mut Graph| {
                g.add_system(branch_b);
            },
        ],
    );

    let warnings = graph
        .validate()
        .expect("graph should be structurally valid");
    assert!(
        warnings
            .iter()
            .any(|w| matches!(w, ValidationWarning::ConflictingParallelOutputs { .. })),
        "expected ConflictingParallelOutputs warning, got: {warnings:?}"
    );
}

#[test]
fn validate_parallel_different_outputs_no_warning() {
    // Branches produce different output types (i32 vs String)
    async fn string_branch() -> String {
        "hello".to_string()
    }

    let mut graph = Graph::new();
    graph.add_parallel(
        "no_conflict",
        vec![
            |g: &mut Graph| {
                g.add_system(branch_a);
            },
            |g: &mut Graph| {
                g.add_system(string_branch);
            },
        ],
    );

    let warnings = graph
        .validate()
        .expect("graph should be structurally valid");
    assert!(
        warnings.is_empty(),
        "expected no warnings, got: {warnings:?}"
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// Loop Predicate Output Validation
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn validate_loop_predicate_output_not_produced() {
    #[derive(Debug)]
    struct LoopState {
        done: bool,
    }

    // Loop predicate reads LoopState, but body only produces i32
    let mut graph = Graph::new();
    graph.add_loop::<LoopState, _, _>(
        "bad_loop",
        |state| state.done,
        |g| {
            g.add_system(loop_body); // produces i32, not LoopState
        },
    );

    let errors = graph.validate().unwrap_err();
    assert!(
        errors
            .iter()
            .any(|err| matches!(err, ValidationError::LoopPredicateOutputNotProduced { .. })),
        "expected LoopPredicateOutputNotProduced error, got: {errors:?}"
    );
}

#[test]
fn validate_loop_predicate_output_produced() {
    #[derive(Debug)]
    struct LoopState {
        done: bool,
    }

    async fn produce_loop_state() -> LoopState {
        LoopState { done: true }
    }

    // Loop predicate reads LoopState, body produces LoopState
    let mut graph = Graph::new();
    graph.add_loop::<LoopState, _, _>(
        "good_loop",
        |state| state.done,
        |g| {
            g.add_system(produce_loop_state);
        },
    );

    let result = graph.validate();
    assert!(result.is_ok(), "Validation failed: {:?}", result.err());
}
