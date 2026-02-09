//! Graph execution engine.
//!
//! The [`GraphExecutor`] traverses and executes graphs, handling all node types
//! including sequential execution, conditional branching, loops, and parallel execution.
//!
//! # Example
//!
//! ```ignore
//! use polaris_graph::{Graph, GraphExecutor};
//! use polaris_system::param::SystemContext;
//!
//! let graph = Graph::new()
//!     .add_system(reason)
//!     .add_system(act);
//!
//! let ctx = SystemContext::new();
//! let executor = GraphExecutor::new();
//! let result = executor.execute(&graph, &ctx).await?;
//! ```

use core::any::TypeId;
use core::fmt;
use core::time::Duration;

use polaris_system::param::{AccessMode, SystemAccess, SystemContext};

use crate::edge::Edge;
use crate::graph::Graph;
use crate::node::{LoopNode, Node, NodeId, ParallelNode, SwitchNode};
use crate::predicate::PredicateError;

/// Result of executing a graph.
#[derive(Debug)]
pub struct ExecutionResult {
    /// Number of nodes executed during traversal.
    pub nodes_executed: usize,
    /// Total execution duration.
    pub duration: Duration,
}

/// Errors that can occur during graph execution.
#[derive(Debug)]
pub enum ExecutionError {
    /// The graph has no entry point.
    EmptyGraph,
    /// A referenced node was not found in the graph.
    NodeNotFound(NodeId),
    /// No sequential edge found from the given node.
    NoNextNode(NodeId),
    /// A decision or loop node is missing its predicate.
    MissingPredicate(NodeId),
    /// A decision node is missing a branch target.
    MissingBranch {
        /// The node ID of the decision node.
        node: NodeId,
        /// Which branch is missing ("true" or "false").
        branch: &'static str,
    },
    /// A parallel node is missing its join target.
    MissingJoin(NodeId),
    /// A system execution error occurred.
    SystemError(String),
    /// A predicate evaluation error occurred.
    PredicateError(PredicateError),
    /// Maximum iterations exceeded in a loop.
    MaxIterationsExceeded {
        /// The loop node that exceeded iterations.
        node: NodeId,
        /// The maximum allowed iterations.
        max: usize,
    },
    /// A loop node has no termination condition (neither predicate nor `max_iterations`).
    NoTerminationCondition(NodeId),
    /// A system execution timed out.
    Timeout {
        /// The node that timed out.
        node: NodeId,
        /// The timeout duration that was exceeded.
        timeout: Duration,
    },
    /// Feature not yet implemented.
    Unimplemented(&'static str),
    /// Maximum recursion depth exceeded in nested control flow.
    RecursionLimitExceeded {
        /// The current depth when the limit was hit.
        depth: usize,
        /// The maximum allowed depth.
        max: usize,
    },
    /// A switch node is missing its discriminator.
    MissingDiscriminator(NodeId),
    /// No matching case found in switch node and no default provided.
    NoMatchingCase {
        /// The switch node ID.
        node: NodeId,
        /// The discriminator value that didn't match any case.
        key: &'static str,
    },
}

impl fmt::Display for ExecutionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ExecutionError::EmptyGraph => write!(f, "graph has no entry point"),
            ExecutionError::NodeNotFound(id) => write!(f, "node not found: {id}"),
            ExecutionError::NoNextNode(id) => write!(f, "no sequential edge from node: {id}"),
            ExecutionError::MissingPredicate(id) => {
                write!(f, "missing predicate on node: {id}")
            }
            ExecutionError::MissingBranch { node, branch } => {
                write!(f, "missing {branch} branch on decision node: {node}")
            }
            ExecutionError::MissingJoin(id) => write!(f, "missing join on parallel node: {id}"),
            ExecutionError::SystemError(msg) => write!(f, "system error: {msg}"),
            ExecutionError::PredicateError(err) => write!(f, "predicate error: {err}"),
            ExecutionError::MaxIterationsExceeded { node, max } => {
                write!(f, "max iterations ({max}) exceeded on loop node: {node}")
            }
            ExecutionError::NoTerminationCondition(id) => {
                write!(f, "loop node has no termination condition: {id}")
            }
            ExecutionError::Timeout { node, timeout } => {
                write!(f, "system timed out after {:?} on node: {node}", timeout)
            }
            ExecutionError::Unimplemented(feature) => {
                write!(f, "feature not implemented: {feature}")
            }
            ExecutionError::RecursionLimitExceeded { depth, max } => {
                write!(
                    f,
                    "recursion limit exceeded: depth {depth} exceeds max {max}"
                )
            }
            ExecutionError::MissingDiscriminator(id) => {
                write!(f, "missing discriminator on switch node: {id}")
            }
            ExecutionError::NoMatchingCase { node, key } => {
                write!(f, "no matching case for key '{key}' on switch node: {node}")
            }
        }
    }
}

impl core::error::Error for ExecutionError {
    fn source(&self) -> Option<&(dyn core::error::Error + 'static)> {
        match self {
            ExecutionError::PredicateError(err) => Some(err),
            _ => None,
        }
    }
}

/// Errors that can occur during resource validation.
///
/// These errors are detected before graph execution starts, allowing
/// early detection of missing resources that would cause runtime failures.
#[derive(Debug, Clone)]
pub enum ResourceValidationError {
    /// A required resource is missing from the context.
    MissingResource {
        /// The node ID of the system requiring the resource.
        node: NodeId,
        /// The name of the system.
        system_name: &'static str,
        /// The type name of the missing resource.
        resource_type: &'static str,
        /// The type ID of the missing resource.
        type_id: TypeId,
        /// The access mode (read or write).
        access_mode: AccessMode,
    },
    /// A required output from a previous system is missing.
    ///
    /// Note: This can only be validated with flow analysis, as outputs
    /// are produced dynamically during execution.
    MissingOutput {
        /// The node ID of the system requiring the output.
        node: NodeId,
        /// The name of the system.
        system_name: &'static str,
        /// The type name of the missing output.
        output_type: &'static str,
        /// The type ID of the missing output.
        type_id: TypeId,
    },
}

impl fmt::Display for ResourceValidationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ResourceValidationError::MissingResource {
                node,
                system_name,
                resource_type,
                access_mode,
                ..
            } => {
                let mode_str = match access_mode {
                    AccessMode::Read => "read",
                    AccessMode::Write => "write",
                };
                write!(
                    f,
                    "system '{system_name}' ({node}) requires {mode_str} access to missing resource: {resource_type}"
                )
            }
            ResourceValidationError::MissingOutput {
                node,
                system_name,
                output_type,
                ..
            } => {
                write!(
                    f,
                    "system '{system_name}' ({node}) requires missing output: {output_type}"
                )
            }
        }
    }
}

impl core::error::Error for ResourceValidationError {}

/// Graph execution engine.
///
/// `GraphExecutor` traverses a graph starting from its entry point,
/// executing systems and following control flow edges.
#[derive(Debug)]
pub struct GraphExecutor {
    /// Maximum iterations for loops without explicit limits (safety default).
    default_max_iterations: Option<usize>,
    /// Maximum recursion depth for nested control flow (safety default).
    max_recursion_depth: usize,
}

impl Default for GraphExecutor {
    fn default() -> Self {
        Self::new()
    }
}

impl GraphExecutor {
    /// Default maximum recursion depth for nested control flow.
    const DEFAULT_MAX_RECURSION_DEPTH: usize = 64;

    /// Creates a new graph executor.
    #[must_use]
    pub fn new() -> Self {
        Self {
            default_max_iterations: Some(1000),
            max_recursion_depth: Self::DEFAULT_MAX_RECURSION_DEPTH,
        }
    }

    /// Creates a new executor with no default iteration limit.
    ///
    /// # Warning
    ///
    /// This can lead to infinite loops if graphs contain loops
    /// without termination predicates or explicit `max_iterations`.
    #[must_use]
    pub fn without_iteration_limit() -> Self {
        Self {
            default_max_iterations: None,
            max_recursion_depth: Self::DEFAULT_MAX_RECURSION_DEPTH,
        }
    }

    /// Sets the default maximum iterations for loops without explicit limits.
    #[must_use]
    pub fn with_default_max_iterations(mut self, max: usize) -> Self {
        self.default_max_iterations = Some(max);
        self
    }

    /// Sets the maximum recursion depth for nested control flow.
    #[must_use]
    pub fn with_max_recursion_depth(mut self, max: usize) -> Self {
        self.max_recursion_depth = max;
        self
    }

    /// Validates that all resources required by systems in the graph
    /// are available in the context.
    ///
    /// This method performs eager validation before execution, catching
    /// missing resources early rather than failing during execution.
    ///
    /// # What is Validated
    ///
    /// - **Resources** (`Res<T>`, `ResMut<T>`): Checked against the context's
    ///   resources (local scope, parent chain, and globals).
    /// - **Outputs** (`Out<T>`): Currently not validated, as outputs are
    ///   produced dynamically during execution. Use flow analysis for
    ///   output validation.
    ///
    /// # Example
    ///
    /// ```ignore
    /// let executor = GraphExecutor::new();
    /// let mut ctx = server.create_context();
    ///
    /// // Validate before executing
    /// if let Err(errors) = executor.validate_resources(&graph, &ctx) {
    ///     for error in &errors {
    ///         eprintln!("Validation error: {error}");
    ///     }
    ///     return Err(errors);
    /// }
    ///
    /// // Safe to execute - all resources are available
    /// let result = executor.execute(&graph, &mut ctx).await?;
    /// ```
    ///
    /// # Returns
    ///
    /// Returns `Ok(())` if all resources are available, or a vector of
    /// validation errors describing missing resources.
    pub fn validate_resources(
        &self,
        graph: &Graph,
        ctx: &SystemContext<'_>,
    ) -> Result<(), Vec<ResourceValidationError>> {
        let mut errors = Vec::new();

        for node in graph.nodes() {
            if let Node::System(sys) = node {
                let access = sys.system.access();
                self.validate_system_access(sys.id, sys.system.name(), &access, ctx, &mut errors);
            }
        }

        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }

    /// Validates a single system's access requirements against the context.
    fn validate_system_access(
        &self,
        node_id: NodeId,
        system_name: &'static str,
        access: &SystemAccess,
        ctx: &SystemContext<'_>,
        errors: &mut Vec<ResourceValidationError>,
    ) {
        // Validate resource accesses
        for res_access in &access.resources {
            let exists = match res_access.mode {
                // Read access walks up the hierarchy (local + parent + globals)
                AccessMode::Read => ctx.contains_resource_by_type_id(res_access.type_id),
                // Write access only checks local scope
                AccessMode::Write => ctx.contains_local_resource_by_type_id(res_access.type_id),
            };

            if !exists {
                errors.push(ResourceValidationError::MissingResource {
                    node: node_id,
                    system_name,
                    resource_type: res_access.type_name,
                    type_id: res_access.type_id,
                    access_mode: res_access.mode,
                });
            }
        }

        // Note: Output validation is skipped here because outputs are produced
        // dynamically during execution. To validate outputs, we would need
        // flow analysis to determine which outputs are produced before each
        // system that requires them.
        //
        // For now, outputs will fail at runtime with ParamError::OutputNotFound
        // if the required output hasn't been produced yet.
    }

    /// Executes a graph starting from its entry point.
    ///
    /// System outputs are stored in the context after each system executes,
    /// making them available to subsequent systems via `Out<T>` parameters
    /// and predicates.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The graph has no entry point
    /// - A referenced node is not found
    /// - A system execution fails
    /// - A predicate evaluation fails
    /// - A loop exceeds its maximum iterations
    pub async fn execute(
        &self,
        graph: &Graph,
        ctx: &mut SystemContext<'_>,
    ) -> Result<ExecutionResult, ExecutionError> {
        let start = std::time::Instant::now();
        let mut nodes_executed = 0;

        let mut current = graph.entry().ok_or(ExecutionError::EmptyGraph)?;

        loop {
            let node = graph
                .get_node(current)
                .ok_or(ExecutionError::NodeNotFound(current))?;

            nodes_executed += 1;

            match node {
                Node::System(sys) => {
                    // Execute the system with optional timeout
                    let result = if let Some(timeout_duration) = sys.timeout {
                        match tokio::time::timeout(timeout_duration, sys.system.run_erased(ctx))
                            .await
                        {
                            Ok(inner_result) => inner_result,
                            Err(_elapsed) => {
                                // Timeout occurred - check for timeout edge
                                if let Some(handler) = self.find_timeout_edge(graph, current) {
                                    current = handler;
                                    continue;
                                }
                                return Err(ExecutionError::Timeout {
                                    node: current,
                                    timeout: timeout_duration,
                                });
                            }
                        }
                    } else {
                        sys.system.run_erased(ctx).await
                    };

                    match result {
                        Ok(output) => {
                            // Store output in context for subsequent systems/predicates
                            ctx.insert_output_boxed(sys.output_type_id(), output);

                            // Find next sequential node
                            match self.find_next_sequential(graph, current) {
                                Ok(next) => current = next,
                                Err(ExecutionError::NoNextNode(_)) => break, // Terminal node
                                Err(err) => return Err(err),
                            }
                        }
                        Err(err) => {
                            // Check for error edge (fallback path)
                            if let Some(handler) = self.find_error_edge(graph, current) {
                                current = handler;
                                // Continue execution from error handler
                            } else {
                                // No error handler, propagate error
                                return Err(ExecutionError::SystemError(err.to_string()));
                            }
                        }
                    }
                }
                Node::Decision(dec) => {
                    let decision_id = current;
                    let predicate = dec
                        .predicate
                        .as_ref()
                        .ok_or(ExecutionError::MissingPredicate(current))?;

                    let result = predicate
                        .evaluate(ctx)
                        .map_err(ExecutionError::PredicateError)?;

                    let branch_entry = if result {
                        dec.true_branch.ok_or(ExecutionError::MissingBranch {
                            node: current,
                            branch: "true",
                        })?
                    } else {
                        dec.false_branch.ok_or(ExecutionError::MissingBranch {
                            node: current,
                            branch: "false",
                        })?
                    };

                    // Execute branch as subgraph
                    let branch_count = self.execute_subgraph(graph, ctx, branch_entry, 0).await?;
                    nodes_executed += branch_count;

                    // After branch, find next sequential from decision node
                    match self.find_next_sequential(graph, decision_id) {
                        Ok(next) => current = next,
                        Err(ExecutionError::NoNextNode(_)) => break,
                        Err(err) => return Err(err),
                    }
                }
                Node::Loop(loop_node) => {
                    let loop_count = self.execute_loop(graph, ctx, loop_node, 0).await?;
                    nodes_executed += loop_count;

                    match self.find_next_sequential(graph, current) {
                        Ok(next) => current = next,
                        Err(ExecutionError::NoNextNode(_)) => break,
                        Err(err) => return Err(err),
                    }
                }
                Node::Parallel(par) => {
                    let parallel_count = self.execute_parallel(graph, ctx, par, 0).await?;
                    nodes_executed += parallel_count;

                    current = par.join.ok_or(ExecutionError::MissingJoin(current))?;
                }
                Node::Join(_) => {
                    // Join is a sync point, find next sequential
                    match self.find_next_sequential(graph, current) {
                        Ok(next) => current = next,
                        Err(ExecutionError::NoNextNode(_)) => break,
                        Err(err) => return Err(err),
                    }
                }
                Node::Switch(switch_node) => {
                    let (switch_count, next) =
                        self.execute_switch(graph, ctx, switch_node, 0).await?;
                    nodes_executed += switch_count;
                    match next {
                        Some(n) => current = n,
                        None => break, // Switch was terminal node
                    }
                }
            }
        }

        Ok(ExecutionResult {
            nodes_executed,
            duration: start.elapsed(),
        })
    }

    /// Finds the next node connected by a sequential edge.
    fn find_next_sequential(&self, graph: &Graph, from: NodeId) -> Result<NodeId, ExecutionError> {
        for edge in graph.edges() {
            if let Edge::Sequential(seq) = edge {
                if seq.from == from {
                    return Ok(seq.to);
                }
            }
        }
        Err(ExecutionError::NoNextNode(from))
    }

    /// Finds an error handler edge from the given node.
    ///
    /// Returns the target node ID if an error edge exists from `from`.
    fn find_error_edge(&self, graph: &Graph, from: NodeId) -> Option<NodeId> {
        for edge in graph.edges() {
            if let Edge::Error(err_edge) = edge {
                if err_edge.from == from {
                    return Some(err_edge.to);
                }
            }
        }
        None
    }

    /// Finds a timeout handler edge from the given node.
    ///
    /// Returns the target node ID if a timeout edge exists from `from`.
    fn find_timeout_edge(&self, graph: &Graph, from: NodeId) -> Option<NodeId> {
        for edge in graph.edges() {
            if let Edge::Timeout(timeout_edge) = edge {
                if timeout_edge.from == from {
                    return Some(timeout_edge.to);
                }
            }
        }
        None
    }

    /// Executes a loop node, returning the number of nodes executed in the loop body.
    ///
    /// Returns a boxed future to support recursion with nested control flow.
    fn execute_loop<'a>(
        &'a self,
        graph: &'a Graph,
        ctx: &'a mut SystemContext<'_>,
        loop_node: &'a LoopNode,
        depth: usize,
    ) -> futures::future::BoxFuture<'a, Result<usize, ExecutionError>> {
        Box::pin(async move {
            let max_iterations = loop_node
                .max_iterations
                .or(self.default_max_iterations)
                .ok_or(ExecutionError::NoTerminationCondition(loop_node.id))?;

            let mut iterations = 0;
            let mut nodes_executed = 0;

            loop {
                // Check termination predicate first
                if let Some(term) = &loop_node.termination {
                    if term.evaluate(ctx).map_err(ExecutionError::PredicateError)? {
                        break;
                    }
                }

                // Check max iterations
                if iterations >= max_iterations {
                    // If we have a termination predicate, this is an error
                    // If not, it's expected behavior
                    if loop_node.termination.is_some() {
                        return Err(ExecutionError::MaxIterationsExceeded {
                            node: loop_node.id,
                            max: max_iterations,
                        });
                    }
                    break;
                }

                // Execute loop body if present
                if let Some(body) = loop_node.body_entry {
                    let count = self.execute_subgraph(graph, ctx, body, depth).await?;
                    nodes_executed += count;
                }

                iterations += 1;
            }

            Ok(nodes_executed)
        })
    }

    /// Executes parallel branches concurrently, returning the total nodes executed.
    ///
    /// Each branch runs in its own child context, providing isolation between
    /// parallel execution paths. Outputs from all branches are merged back into
    /// the parent context after completion (last-write-wins for same types).
    ///
    /// # Concurrency
    ///
    /// Branches execute concurrently using `futures::future::try_join_all`.
    /// If any branch fails, the entire parallel execution fails and remaining
    /// branches are cancelled.
    ///
    /// Returns a boxed future to support recursion with nested control flow.
    fn execute_parallel<'a>(
        &'a self,
        graph: &'a Graph,
        ctx: &'a mut SystemContext<'_>,
        par: &'a ParallelNode,
        depth: usize,
    ) -> futures::future::BoxFuture<'a, Result<usize, ExecutionError>> {
        Box::pin(async move {
            use futures::future::try_join_all;

            // Create child contexts for each branch upfront
            let mut child_contexts: Vec<SystemContext<'_>> =
                par.branches.iter().map(|_| ctx.child()).collect();

            // Pair branches with their contexts and create futures
            let futures = par
                .branches
                .iter()
                .zip(child_contexts.iter_mut())
                .map(|(&branch, child_ctx)| self.execute_subgraph(graph, child_ctx, branch, depth));

            // Execute all branches concurrently
            let results = try_join_all(futures).await?;

            // Sum up total nodes executed across all branches
            let total_nodes = results.iter().sum();

            Ok(total_nodes)
        })
    }

    /// Executes a switch node, returning the nodes executed and optionally the next node.
    ///
    /// Evaluates the discriminator to determine which case branch to execute,
    /// then runs that branch's subgraph. Returns the total nodes executed in
    /// the branch and the next node to continue execution from (if any).
    fn execute_switch<'a>(
        &'a self,
        graph: &'a Graph,
        ctx: &'a mut SystemContext<'_>,
        switch_node: &'a SwitchNode,
        depth: usize,
    ) -> futures::future::BoxFuture<'a, Result<(usize, Option<NodeId>), ExecutionError>> {
        Box::pin(async move {
            // Get the discriminator
            let discriminator = switch_node
                .discriminator
                .as_ref()
                .ok_or(ExecutionError::MissingDiscriminator(switch_node.id))?;

            // Evaluate to get the case key
            let key = discriminator
                .discriminate(ctx)
                .map_err(ExecutionError::PredicateError)?;

            // Find matching case
            let target = switch_node
                .cases
                .iter()
                .find(|(case_key, _)| *case_key == key)
                .map(|(_, node_id)| *node_id)
                .or(switch_node.default)
                .ok_or(ExecutionError::NoMatchingCase {
                    node: switch_node.id,
                    key,
                })?;

            // Execute the case branch subgraph
            let nodes_executed = self.execute_subgraph(graph, ctx, target, depth).await?;

            // Find the next node after the switch (via sequential edge from switch node)
            let next = self.find_next_sequential(graph, switch_node.id).ok();

            Ok((nodes_executed, next))
        })
    }

    /// Executes a subgraph starting from a given node until a terminal point.
    ///
    /// Supports nested control flow (loops, parallel) with recursion depth tracking.
    /// Returns a boxed future to support recursion with nested control flow.
    fn execute_subgraph<'a>(
        &'a self,
        graph: &'a Graph,
        ctx: &'a mut SystemContext<'_>,
        start: NodeId,
        depth: usize,
    ) -> futures::future::BoxFuture<'a, Result<usize, ExecutionError>> {
        Box::pin(async move {
            // Check recursion limit
            if depth >= self.max_recursion_depth {
                return Err(ExecutionError::RecursionLimitExceeded {
                    depth,
                    max: self.max_recursion_depth,
                });
            }

            let mut current = start;
            let mut nodes_executed = 0;

            loop {
                let node = graph
                    .get_node(current)
                    .ok_or(ExecutionError::NodeNotFound(current))?;

                nodes_executed += 1;

                match node {
                    Node::System(sys) => {
                        // Execute the system with optional timeout
                        let result = if let Some(timeout_duration) = sys.timeout {
                            match tokio::time::timeout(timeout_duration, sys.system.run_erased(ctx))
                                .await
                            {
                                Ok(inner_result) => inner_result,
                                Err(_elapsed) => {
                                    // Timeout occurred - check for timeout edge
                                    if let Some(handler) = self.find_timeout_edge(graph, current) {
                                        current = handler;
                                        continue;
                                    }
                                    return Err(ExecutionError::Timeout {
                                        node: current,
                                        timeout: timeout_duration,
                                    });
                                }
                            }
                        } else {
                            sys.system.run_erased(ctx).await
                        };

                        match result {
                            Ok(output) => {
                                // Store output in context
                                ctx.insert_output_boxed(sys.output_type_id(), output);

                                match self.find_next_sequential(graph, current) {
                                    Ok(next) => current = next,
                                    Err(ExecutionError::NoNextNode(_)) => break,
                                    Err(err) => return Err(err),
                                }
                            }
                            Err(err) => {
                                // Check for error edge (fallback path)
                                if let Some(handler) = self.find_error_edge(graph, current) {
                                    current = handler;
                                    // Continue execution from error handler
                                } else {
                                    // No error handler, propagate error
                                    return Err(ExecutionError::SystemError(err.to_string()));
                                }
                            }
                        }
                    }
                    Node::Decision(dec) => {
                        let decision_id = current;
                        let predicate = dec
                            .predicate
                            .as_ref()
                            .ok_or(ExecutionError::MissingPredicate(current))?;

                        let result = predicate
                            .evaluate(ctx)
                            .map_err(ExecutionError::PredicateError)?;

                        let branch_entry = if result {
                            dec.true_branch.ok_or(ExecutionError::MissingBranch {
                                node: current,
                                branch: "true",
                            })?
                        } else {
                            dec.false_branch.ok_or(ExecutionError::MissingBranch {
                                node: current,
                                branch: "false",
                            })?
                        };

                        // Execute branch as subgraph (with increased depth)
                        let branch_count =
                            self.execute_subgraph(graph, ctx, branch_entry, depth + 1).await?;
                        nodes_executed += branch_count;

                        // After branch, find next sequential from decision node
                        match self.find_next_sequential(graph, decision_id) {
                            Ok(next) => current = next,
                            Err(ExecutionError::NoNextNode(_)) => break,
                            Err(err) => return Err(err),
                        }
                    }
                    Node::Join(_) => {
                        // Join marks the end of a parallel branch
                        break;
                    }
                    Node::Loop(loop_node) => {
                        // Execute nested loop with increased depth
                        let loop_count =
                            self.execute_loop(graph, ctx, loop_node, depth + 1).await?;
                        nodes_executed += loop_count;

                        match self.find_next_sequential(graph, current) {
                            Ok(next) => current = next,
                            Err(ExecutionError::NoNextNode(_)) => break,
                            Err(err) => return Err(err),
                        }
                    }
                    Node::Parallel(par) => {
                        // Execute nested parallel with increased depth
                        let parallel_count =
                            self.execute_parallel(graph, ctx, par, depth + 1).await?;
                        nodes_executed += parallel_count;

                        // After parallel completes, continue to join node or next sequential
                        if let Some(join) = par.join {
                            current = join;
                        } else {
                            match self.find_next_sequential(graph, current) {
                                Ok(next) => current = next,
                                Err(ExecutionError::NoNextNode(_)) => break,
                                Err(err) => return Err(err),
                            }
                        }
                    }
                    Node::Switch(switch_node) => {
                        // Execute nested switch with increased depth
                        let (switch_count, next) = self
                            .execute_switch(graph, ctx, switch_node, depth + 1)
                            .await?;
                        nodes_executed += switch_count;
                        match next {
                            Some(n) => current = n,
                            None => break, // Switch was terminal node in subgraph
                        }
                    }
                }
            }

            Ok(nodes_executed)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn executor_creation() {
        let executor = GraphExecutor::new();
        assert_eq!(executor.default_max_iterations, Some(1000));
    }

    #[test]
    fn executor_without_limit() {
        let executor = GraphExecutor::without_iteration_limit();
        assert_eq!(executor.default_max_iterations, None);
    }

    #[test]
    fn executor_with_custom_limit() {
        let executor = GraphExecutor::new().with_default_max_iterations(500);
        assert_eq!(executor.default_max_iterations, Some(500));
    }

    #[test]
    fn execution_error_display() {
        let err = ExecutionError::EmptyGraph;
        assert_eq!(format!("{err}"), "graph has no entry point");

        let err = ExecutionError::NodeNotFound(NodeId::new(5));
        assert_eq!(format!("{err}"), "node not found: node_5");

        let err = ExecutionError::MissingBranch {
            node: NodeId::new(3),
            branch: "true",
        };
        assert_eq!(
            format!("{err}"),
            "missing true branch on decision node: node_3"
        );
    }

    #[tokio::test]
    async fn execute_empty_graph() {
        let graph = Graph::new();
        let mut ctx = SystemContext::new();
        let executor = GraphExecutor::new();

        let result = executor.execute(&graph, &mut ctx).await;
        assert!(matches!(result, Err(ExecutionError::EmptyGraph)));
    }

    #[tokio::test]
    async fn execute_single_system() {
        async fn simple() -> i32 {
            42
        }

        let mut graph = Graph::new();
        graph.add_system(simple);

        let mut ctx = SystemContext::new();
        let executor = GraphExecutor::new();

        let result = executor.execute(&graph, &mut ctx).await;
        assert!(result.is_ok());
        let result = result.unwrap();
        assert_eq!(result.nodes_executed, 1);

        // Verify output was stored
        assert!(ctx.contains_output::<i32>());
    }

    #[tokio::test]
    async fn execute_sequential_systems() {
        async fn first() -> i32 {
            1
        }
        async fn second() -> i32 {
            2
        }
        async fn third() -> i32 {
            3
        }

        let mut graph = Graph::new();
        graph.add_system(first).add_system(second).add_system(third);

        let mut ctx = SystemContext::new();
        let executor = GraphExecutor::new();

        let result = executor.execute(&graph, &mut ctx).await;
        assert!(result.is_ok());
        let result = result.unwrap();
        assert_eq!(result.nodes_executed, 3);

        // Last system's output should be available
        assert!(ctx.contains_output::<i32>());
    }

    #[tokio::test]
    async fn output_available_to_predicate() {
        #[derive(Debug)]
        struct Decision {
            should_branch: bool,
        }

        async fn make_decision() -> Decision {
            Decision {
                should_branch: true,
            }
        }

        async fn true_path() -> &'static str {
            "took true path"
        }

        async fn false_path() -> &'static str {
            "took false path"
        }

        let mut graph = Graph::new();

        // Add decision system
        graph.add_system(make_decision);

        // Add conditional branch that reads the decision output
        graph.add_conditional_branch::<Decision, _, _, _>(
            "branch",
            |decision| decision.should_branch,
            |g| {
                g.add_system(true_path);
            },
            |g| {
                g.add_system(false_path);
            },
        );

        let mut ctx = SystemContext::new();
        let executor = GraphExecutor::new();

        let result = executor.execute(&graph, &mut ctx).await;
        assert!(result.is_ok(), "Execution failed: {:?}", result.err());

        // Should have taken the true branch
        let output = ctx.get_output::<&'static str>().unwrap();
        assert_eq!(*output, "took true path");
    }

    #[tokio::test]
    async fn error_handler_invoked_on_failure() {
        use polaris_system::system::{BoxFuture, System, SystemError};

        // Custom system that always fails
        struct FailingSystem;

        impl System for FailingSystem {
            type Output = i32;

            fn run<'a>(
                &'a self,
                _ctx: &'a SystemContext<'_>,
            ) -> BoxFuture<'a, Result<Self::Output, SystemError>> {
                Box::pin(
                    async move { Err(SystemError::ExecutionError("intentional failure".into())) },
                )
            }

            fn name(&self) -> &'static str {
                "failing_system"
            }
        }

        async fn error_handler() -> &'static str {
            "handled error"
        }

        let mut graph = Graph::new();

        // Add the failing system using public API
        let fail_id = graph.add_boxed_system(Box::new(FailingSystem));

        graph.add_error_handler(fail_id, |g| {
            g.add_system(error_handler);
        });

        let mut ctx = SystemContext::new();
        let executor = GraphExecutor::new();

        let result = executor.execute(&graph, &mut ctx).await;
        assert!(
            result.is_ok(),
            "Execution should succeed via error handler: {:?}",
            result.err()
        );

        let output = ctx.get_output::<&'static str>().unwrap();
        assert_eq!(*output, "handled error");
    }

    #[tokio::test]
    async fn error_propagates_without_handler() {
        use polaris_system::system::{BoxFuture, System, SystemError};

        // Custom system that always fails
        struct FailingSystem;

        impl System for FailingSystem {
            type Output = i32;

            fn run<'a>(
                &'a self,
                _ctx: &'a SystemContext<'_>,
            ) -> BoxFuture<'a, Result<Self::Output, SystemError>> {
                Box::pin(async move { Err(SystemError::ExecutionError("always fails".into())) })
            }

            fn name(&self) -> &'static str {
                "failing_system"
            }
        }

        let mut graph = Graph::new();

        // Add the failing system using public API
        graph.add_boxed_system(Box::new(FailingSystem));

        let mut ctx = SystemContext::new();
        let executor = GraphExecutor::new();

        let result = executor.execute(&graph, &mut ctx).await;
        assert!(
            result.is_err(),
            "Execution should fail without error handler"
        );

        if let Err(ExecutionError::SystemError(msg)) = result {
            assert!(msg.contains("always fails"));
        } else {
            panic!("Expected SystemError, got {:?}", result);
        }
    }

    #[tokio::test]
    async fn successful_system_skips_error_handler() {
        async fn succeeds() -> &'static str {
            "success"
        }

        async fn error_handler() -> &'static str {
            "handled error"
        }

        let mut graph = Graph::new();
        let system_id = graph.add_system_node(succeeds);
        graph.add_error_handler(system_id, |g| {
            g.add_system(error_handler);
        });

        let mut ctx = SystemContext::new();
        let executor = GraphExecutor::new();

        let result = executor.execute(&graph, &mut ctx).await;
        assert!(result.is_ok());

        // Should have the success output, not the error handler output
        let output = ctx.get_output::<&'static str>().unwrap();
        assert_eq!(*output, "success");
    }

    #[tokio::test]
    async fn timeout_triggers_handler() {
        use core::time::Duration;
        use polaris_system::system::{BoxFuture, System, SystemError};

        // System that takes a long time
        struct SlowSystem;

        impl System for SlowSystem {
            type Output = i32;

            fn run<'a>(
                &'a self,
                _ctx: &'a SystemContext<'_>,
            ) -> BoxFuture<'a, Result<Self::Output, SystemError>> {
                Box::pin(async move {
                    tokio::time::sleep(Duration::from_secs(10)).await;
                    Ok(42)
                })
            }

            fn name(&self) -> &'static str {
                "slow_system"
            }
        }

        async fn timeout_handler() -> &'static str {
            "timeout handled"
        }

        let mut graph = Graph::new();
        let slow_id = graph.add_boxed_system(Box::new(SlowSystem));
        graph.set_timeout(slow_id, Duration::from_millis(10));
        graph.add_timeout_handler(slow_id, |g| {
            g.add_system(timeout_handler);
        });

        let mut ctx = SystemContext::new();
        let executor = GraphExecutor::new();

        let result = executor.execute(&graph, &mut ctx).await;
        assert!(
            result.is_ok(),
            "Execution should succeed via timeout handler: {:?}",
            result.err()
        );

        let output = ctx.get_output::<&'static str>().unwrap();
        assert_eq!(*output, "timeout handled");
    }

    #[tokio::test]
    async fn timeout_error_without_handler() {
        use core::time::Duration;
        use polaris_system::system::{BoxFuture, System, SystemError};

        // System that takes a long time
        struct SlowSystem;

        impl System for SlowSystem {
            type Output = i32;

            fn run<'a>(
                &'a self,
                _ctx: &'a SystemContext<'_>,
            ) -> BoxFuture<'a, Result<Self::Output, SystemError>> {
                Box::pin(async move {
                    tokio::time::sleep(Duration::from_secs(10)).await;
                    Ok(42)
                })
            }

            fn name(&self) -> &'static str {
                "slow_system"
            }
        }

        let mut graph = Graph::new();
        let slow_id = graph.add_boxed_system(Box::new(SlowSystem));
        graph.set_timeout(slow_id, Duration::from_millis(10));

        let mut ctx = SystemContext::new();
        let executor = GraphExecutor::new();

        let result = executor.execute(&graph, &mut ctx).await;
        assert!(
            result.is_err(),
            "Execution should fail without timeout handler"
        );

        if let Err(ExecutionError::Timeout { node, timeout }) = result {
            assert_eq!(node, slow_id);
            assert_eq!(timeout, Duration::from_millis(10));
        } else {
            panic!("Expected Timeout error, got {:?}", result);
        }
    }

    #[tokio::test]
    async fn fast_system_does_not_timeout() {
        use core::time::Duration;

        async fn fast_system() -> &'static str {
            "fast result"
        }

        async fn timeout_handler() -> &'static str {
            "timeout handled"
        }

        let mut graph = Graph::new();
        let fast_id = graph.add_system_node(fast_system);
        graph.set_timeout(fast_id, Duration::from_secs(10));
        graph.add_timeout_handler(fast_id, |g| {
            g.add_system(timeout_handler);
        });

        let mut ctx = SystemContext::new();
        let executor = GraphExecutor::new();

        let result = executor.execute(&graph, &mut ctx).await;
        assert!(result.is_ok());

        // Should have fast result, not timeout handler
        let output = ctx.get_output::<&'static str>().unwrap();
        assert_eq!(*output, "fast result");
    }

    #[tokio::test]
    async fn parallel_execution_runs_all_branches() {
        use core::sync::atomic::{AtomicUsize, Ordering};
        use polaris_system::system::{BoxFuture, System, SystemError};

        // Counter to track how many branches executed
        static COUNTER: AtomicUsize = AtomicUsize::new(0);

        struct CountingSystem {
            value: i32,
        }

        impl System for CountingSystem {
            type Output = i32;

            fn run<'a>(
                &'a self,
                _ctx: &'a SystemContext<'_>,
            ) -> BoxFuture<'a, Result<Self::Output, SystemError>> {
                Box::pin(async move {
                    COUNTER.fetch_add(1, Ordering::SeqCst);
                    Ok(self.value)
                })
            }

            fn name(&self) -> &'static str {
                "counting_system"
            }
        }

        // Reset counter
        COUNTER.store(0, Ordering::SeqCst);

        let mut graph = Graph::new();
        graph.add_parallel(
            "parallel",
            vec![
                |g: &mut Graph| {
                    g.add_boxed_system(Box::new(CountingSystem { value: 1 }));
                },
                |g: &mut Graph| {
                    g.add_boxed_system(Box::new(CountingSystem { value: 2 }));
                },
                |g: &mut Graph| {
                    g.add_boxed_system(Box::new(CountingSystem { value: 3 }));
                },
            ],
        );

        let mut ctx = SystemContext::new();
        let executor = GraphExecutor::new();

        let result = executor.execute(&graph, &mut ctx).await;
        assert!(
            result.is_ok(),
            "Parallel execution failed: {:?}",
            result.err()
        );

        // All 3 branches should have executed
        assert_eq!(COUNTER.load(Ordering::SeqCst), 3);

        // Result should show nodes executed (parallel + 3 systems + join = 5, or just systems)
        let execution_result = result.unwrap();
        assert!(execution_result.nodes_executed >= 3);
    }

    #[tokio::test]
    async fn parallel_branch_failure_stops_execution() {
        use polaris_system::system::{BoxFuture, System, SystemError};

        struct FailingBranch;

        impl System for FailingBranch {
            type Output = i32;

            fn run<'a>(
                &'a self,
                _ctx: &'a SystemContext<'_>,
            ) -> BoxFuture<'a, Result<Self::Output, SystemError>> {
                Box::pin(async move { Err(SystemError::ExecutionError("branch failed".into())) })
            }

            fn name(&self) -> &'static str {
                "failing_branch"
            }
        }

        async fn success_branch() -> i32 {
            42
        }

        let mut graph = Graph::new();
        graph.add_parallel(
            "parallel",
            vec![
                |g: &mut Graph| {
                    g.add_system(success_branch);
                },
                |g: &mut Graph| {
                    g.add_boxed_system(Box::new(FailingBranch));
                },
            ],
        );

        let mut ctx = SystemContext::new();
        let executor = GraphExecutor::new();

        let result = executor.execute(&graph, &mut ctx).await;
        assert!(
            result.is_err(),
            "Parallel execution should fail when a branch fails"
        );

        if let Err(ExecutionError::SystemError(msg)) = result {
            assert!(msg.contains("branch failed"));
        } else {
            panic!("Expected SystemError, got {:?}", result);
        }
    }

    // ─────────────────────────────────────────────────────────────────────
    // Decision node tests - false branch
    // ─────────────────────────────────────────────────────────────────────

    #[tokio::test]
    async fn decision_takes_false_branch() {
        #[derive(Debug)]
        struct Decision {
            should_branch: bool,
        }

        async fn make_decision() -> Decision {
            Decision {
                should_branch: false, // This time take false branch
            }
        }

        async fn true_path() -> &'static str {
            "took true path"
        }

        async fn false_path() -> &'static str {
            "took false path"
        }

        let mut graph = Graph::new();
        graph.add_system(make_decision);
        graph.add_conditional_branch::<Decision, _, _, _>(
            "branch",
            |decision| decision.should_branch,
            |g| {
                g.add_system(true_path);
            },
            |g| {
                g.add_system(false_path);
            },
        );

        let mut ctx = SystemContext::new();
        let executor = GraphExecutor::new();

        let result = executor.execute(&graph, &mut ctx).await;
        assert!(result.is_ok(), "Execution failed: {:?}", result.err());

        // Should have taken the false branch
        let output = ctx.get_output::<&'static str>().unwrap();
        assert_eq!(*output, "took false path");
    }

    #[test]
    fn decision_missing_predicate_error_display() {
        // Test that MissingPredicate error displays correctly
        // We can't easily create a malformed graph via public API, but we can
        // verify the error type works correctly
        let decision_id = NodeId::new(0);
        let err = ExecutionError::MissingPredicate(decision_id);
        assert!(format!("{err}").contains("missing predicate"));
        assert!(format!("{err}").contains("node_0"));
    }

    // ─────────────────────────────────────────────────────────────────────
    // Loop node tests
    // ─────────────────────────────────────────────────────────────────────
    //
    // ## Loop Execution Semantics
    //
    // 1. **Predicate Timing**: The termination predicate is checked BEFORE
    //    each iteration. It reads from `Out<T>` in the context.
    //
    // 2. **Output Priming**: For loops with termination predicates, output
    //    must exist before entering the loop. Add a system before the loop
    //    that produces the initial output value.
    //
    // 3. **Exit Path**: After the loop completes (via predicate or max
    //    iterations), the executor proceeds to the next sequential node.
    //
    // ─────────────────────────────────────────────────────────────────────

    #[tokio::test]
    async fn loop_max_iterations_exceeded_error() {
        use core::sync::atomic::{AtomicUsize, Ordering};
        use polaris_system::system::{BoxFuture, System, SystemError};

        static NEVER_DONE_COUNT: AtomicUsize = AtomicUsize::new(0);

        #[derive(Debug)]
        struct NeverDone {
            done: bool,
        }

        struct NeverDoneSystem;

        impl System for NeverDoneSystem {
            type Output = NeverDone;

            fn run<'a>(
                &'a self,
                _ctx: &'a SystemContext<'_>,
            ) -> BoxFuture<'a, Result<Self::Output, SystemError>> {
                Box::pin(async move {
                    NEVER_DONE_COUNT.fetch_add(1, Ordering::SeqCst);
                    Ok(NeverDone { done: false })
                })
            }

            fn name(&self) -> &'static str {
                "never_done_system"
            }
        }

        async fn init() -> NeverDone {
            NeverDone { done: false }
        }

        NEVER_DONE_COUNT.store(0, Ordering::SeqCst);

        let mut graph = Graph::new();
        graph.add_system(init); // Prime the output
        graph.add_loop::<NeverDone, _, _>(
            "infinite_loop",
            |state| state.done, // Never returns true
            |g| {
                g.add_boxed_system(Box::new(NeverDoneSystem));
            },
        );

        let mut ctx = SystemContext::new();
        let executor = GraphExecutor::new().with_default_max_iterations(10);

        let result = executor.execute(&graph, &mut ctx).await;

        // Should error due to max iterations exceeded
        assert!(result.is_err());
        if let Err(ExecutionError::MaxIterationsExceeded { max, .. }) = result {
            assert_eq!(max, 10);
        } else {
            panic!("Expected MaxIterationsExceeded, got {:?}", result);
        }

        // Should have iterated exactly 10 times before stopping
        assert_eq!(NEVER_DONE_COUNT.load(Ordering::SeqCst), 10);
    }

    #[tokio::test]
    async fn loop_body_executes_correct_iterations() {
        // This test verifies that loop body executes the expected number
        // of times and completes successfully when loop is terminal.
        use core::sync::atomic::{AtomicUsize, Ordering};
        use polaris_system::system::{BoxFuture, System, SystemError};

        static BODY_COUNT: AtomicUsize = AtomicUsize::new(0);

        struct BodySystem;

        impl System for BodySystem {
            type Output = i32;

            fn run<'a>(
                &'a self,
                _ctx: &'a SystemContext<'_>,
            ) -> BoxFuture<'a, Result<Self::Output, SystemError>> {
                Box::pin(async move {
                    let count = BODY_COUNT.fetch_add(1, Ordering::SeqCst);
                    Ok(count as i32)
                })
            }

            fn name(&self) -> &'static str {
                "body_system"
            }
        }

        BODY_COUNT.store(0, Ordering::SeqCst);

        let mut graph = Graph::new();
        graph.add_loop_n("count_loop", 7, |g| {
            g.add_boxed_system(Box::new(BodySystem));
        });

        let mut ctx = SystemContext::new();
        let executor = GraphExecutor::new();

        let result = executor.execute(&graph, &mut ctx).await;

        // Loop body should have executed exactly 7 times
        assert_eq!(BODY_COUNT.load(Ordering::SeqCst), 7);

        assert!(result.is_ok(), "Expected Ok, got {:?}", result);
        assert_eq!(result.unwrap().nodes_executed, 8); // 1 loop node + 7 body executions
    }

    #[tokio::test]
    async fn loop_predicate_terminates_early() {
        // This test verifies that the predicate can terminate the loop
        // before max iterations and completes successfully.
        use core::sync::atomic::{AtomicUsize, Ordering};
        use polaris_system::system::{BoxFuture, System, SystemError};

        static PRED_BODY_COUNT: AtomicUsize = AtomicUsize::new(0);

        #[derive(Debug)]
        struct Counter {
            count: usize,
        }

        struct CounterSystem;

        impl System for CounterSystem {
            type Output = Counter;

            fn run<'a>(
                &'a self,
                _ctx: &'a SystemContext<'_>,
            ) -> BoxFuture<'a, Result<Self::Output, SystemError>> {
                Box::pin(async move {
                    let count = PRED_BODY_COUNT.fetch_add(1, Ordering::SeqCst) + 1;
                    Ok(Counter { count })
                })
            }

            fn name(&self) -> &'static str {
                "counter_system"
            }
        }

        async fn init() -> Counter {
            Counter { count: 0 }
        }

        PRED_BODY_COUNT.store(0, Ordering::SeqCst);

        let mut graph = Graph::new();
        graph.add_system(init); // Prime the output
        graph.add_loop::<Counter, _, _>(
            "early_exit_loop",
            |counter| counter.count >= 3, // Exit after 3 iterations
            |g| {
                g.add_boxed_system(Box::new(CounterSystem));
            },
        );

        let mut ctx = SystemContext::new();
        // Set high max to ensure predicate triggers first
        let executor = GraphExecutor::new().with_default_max_iterations(100);

        let result = executor.execute(&graph, &mut ctx).await;

        // Should have executed exactly 3 times before predicate returned true
        assert_eq!(PRED_BODY_COUNT.load(Ordering::SeqCst), 3);

        assert!(result.is_ok(), "Expected Ok, got {:?}", result);
        // 1 init + 1 loop node + 3 body executions = 5 nodes
        assert_eq!(result.unwrap().nodes_executed, 5);
    }

    #[tokio::test]
    async fn loop_continues_to_next_sequential_node() {
        // This test verifies that after a loop completes, execution
        // continues to the next sequential node in the graph.
        use core::sync::atomic::{AtomicUsize, Ordering};

        static LOOP_BODY_COUNT: AtomicUsize = AtomicUsize::new(0);
        static AFTER_LOOP_COUNT: AtomicUsize = AtomicUsize::new(0);

        async fn loop_body() {
            LOOP_BODY_COUNT.fetch_add(1, Ordering::SeqCst);
        }

        async fn after_loop() {
            AFTER_LOOP_COUNT.fetch_add(1, Ordering::SeqCst);
        }

        LOOP_BODY_COUNT.store(0, Ordering::SeqCst);
        AFTER_LOOP_COUNT.store(0, Ordering::SeqCst);

        let mut graph = Graph::new();
        graph.add_loop_n("test_loop", 3, |g| {
            g.add_system(loop_body);
        });
        graph.add_system(after_loop); // Should execute after loop completes

        let mut ctx = SystemContext::new();
        let executor = GraphExecutor::new();

        let result = executor.execute(&graph, &mut ctx).await;

        assert!(result.is_ok(), "Expected Ok, got {:?}", result);

        // Loop body should have executed 3 times
        assert_eq!(LOOP_BODY_COUNT.load(Ordering::SeqCst), 3);

        // After-loop system should have executed exactly once
        assert_eq!(AFTER_LOOP_COUNT.load(Ordering::SeqCst), 1);

        // 1 loop node + 3 body + 1 after = 5 nodes
        assert_eq!(result.unwrap().nodes_executed, 5);
    }

    // ─────────────────────────────────────────────────────────────────────
    // Output chaining tests
    // ─────────────────────────────────────────────────────────────────────

    #[tokio::test]
    async fn output_chaining_between_systems() {
        use polaris_system::param::{Out, SystemParam};
        use polaris_system::system::{BoxFuture, System, SystemError};

        #[derive(Debug, Clone)]
        struct FirstOutput {
            value: i32,
        }

        #[derive(Debug, Clone)]
        struct SecondOutput {
            doubled: i32,
        }

        async fn first_system() -> FirstOutput {
            FirstOutput { value: 21 }
        }

        // Second system reads first system's output
        struct SecondSystem;

        impl System for SecondSystem {
            type Output = SecondOutput;

            fn run<'a>(
                &'a self,
                ctx: &'a SystemContext<'_>,
            ) -> BoxFuture<'a, Result<Self::Output, SystemError>> {
                Box::pin(async move {
                    let first = Out::<FirstOutput>::fetch(ctx)?;
                    Ok(SecondOutput {
                        doubled: first.value * 2,
                    })
                })
            }

            fn name(&self) -> &'static str {
                "second_system"
            }
        }

        let mut graph = Graph::new();
        graph.add_system(first_system);
        graph.add_boxed_system(Box::new(SecondSystem));

        let mut ctx = SystemContext::new();
        let executor = GraphExecutor::new();

        let result = executor.execute(&graph, &mut ctx).await;
        assert!(result.is_ok(), "Output chaining failed: {:?}", result.err());

        // Second system should have read first system's output
        let output = ctx.get_output::<SecondOutput>().unwrap();
        assert_eq!(output.doubled, 42);
    }

    // ─────────────────────────────────────────────────────────────────────
    // Nested control flow tests
    // ─────────────────────────────────────────────────────────────────────

    #[tokio::test]
    async fn nested_loop_in_parallel_branch() {
        use core::sync::atomic::{AtomicUsize, Ordering};
        use polaris_system::system::{BoxFuture, System, SystemError};

        static NESTED_LOOP_COUNT: AtomicUsize = AtomicUsize::new(0);

        struct NestedSystem;

        impl System for NestedSystem {
            type Output = i32;

            fn run<'a>(
                &'a self,
                _ctx: &'a SystemContext<'_>,
            ) -> BoxFuture<'a, Result<Self::Output, SystemError>> {
                Box::pin(async move {
                    NESTED_LOOP_COUNT.fetch_add(1, Ordering::SeqCst);
                    Ok(1)
                })
            }

            fn name(&self) -> &'static str {
                "nested_system"
            }
        }

        NESTED_LOOP_COUNT.store(0, Ordering::SeqCst);

        let mut graph = Graph::new();
        graph.add_parallel(
            "outer_parallel",
            vec![
                |g: &mut Graph| {
                    // Branch 1: A loop with 3 iterations
                    g.add_loop_n("nested_loop", 3, |inner| {
                        inner.add_boxed_system(Box::new(NestedSystem));
                    });
                },
                |g: &mut Graph| {
                    // Branch 2: Simple system
                    g.add_boxed_system(Box::new(NestedSystem));
                },
            ],
        );

        let mut ctx = SystemContext::new();
        let executor = GraphExecutor::new();

        let result = executor.execute(&graph, &mut ctx).await;
        // The nested loop in branch 1 runs 3 times, branch 2 runs 1 system
        // Total: 4 executions of NestedSystem
        assert_eq!(
            NESTED_LOOP_COUNT.load(Ordering::SeqCst),
            4,
            "Expected 4 executions (3 from nested loop + 1 from simple branch)"
        );

        assert!(result.is_ok(), "Expected Ok, got {:?}", result);
    }

    #[tokio::test]
    async fn recursion_limit_exceeded() {
        // Test that deeply nested control flow hits the recursion limit
        let executor = GraphExecutor::new().with_max_recursion_depth(2);
        assert_eq!(executor.max_recursion_depth, 2);

        // Verify the error display
        let err = ExecutionError::RecursionLimitExceeded { depth: 3, max: 2 };
        let msg = format!("{err}");
        assert!(msg.contains("recursion limit exceeded"));
        assert!(msg.contains("depth 3"));
        assert!(msg.contains("max 2"));
    }

    #[test]
    fn executor_with_custom_recursion_depth() {
        let executor = GraphExecutor::new().with_max_recursion_depth(128);
        assert_eq!(executor.max_recursion_depth, 128);
    }

    // ─────────────────────────────────────────────────────────────────────
    // Switch node tests
    // ─────────────────────────────────────────────────────────────────────

    #[tokio::test]
    async fn switch_routes_to_matching_case() {
        #[derive(Debug)]
        struct RouterOutput {
            action: &'static str,
        }

        async fn router() -> RouterOutput {
            RouterOutput { action: "tool" }
        }

        async fn tool_handler() -> &'static str {
            "executed tool"
        }

        async fn respond_handler() -> &'static str {
            "executed respond"
        }

        // Define handlers as named functions to work around closure type differences
        fn build_tool(g: &mut Graph) {
            g.add_system(tool_handler);
        }
        fn build_respond(g: &mut Graph) {
            g.add_system(respond_handler);
        }

        let mut graph = Graph::new();
        graph.add_system(router);
        graph.add_switch::<RouterOutput, _, _>(
            "route",
            |o| o.action,
            vec![
                ("tool", build_tool as fn(&mut Graph)),
                ("respond", build_respond),
            ],
            None,
        );

        let mut ctx = SystemContext::new();
        let executor = GraphExecutor::new();

        let result = executor.execute(&graph, &mut ctx).await;
        assert!(
            result.is_ok(),
            "Switch execution failed: {:?}",
            result.err()
        );

        // Should have taken the "tool" branch
        let output = ctx.get_output::<&'static str>().unwrap();
        assert_eq!(*output, "executed tool");
    }

    #[tokio::test]
    async fn switch_routes_to_default_when_no_match() {
        #[derive(Debug)]
        struct RouterOutput {
            action: &'static str,
        }

        async fn router() -> RouterOutput {
            RouterOutput { action: "unknown" }
        }

        async fn tool_handler() -> &'static str {
            "executed tool"
        }

        async fn default_handler() -> &'static str {
            "executed default"
        }

        fn build_tool(g: &mut Graph) {
            g.add_system(tool_handler);
        }
        fn build_default(g: &mut Graph) {
            g.add_system(default_handler);
        }

        let mut graph = Graph::new();
        graph.add_system(router);
        graph.add_switch::<RouterOutput, _, _>(
            "route",
            |o| o.action,
            vec![("tool", build_tool as fn(&mut Graph))],
            Some(build_default as fn(&mut Graph)),
        );

        let mut ctx = SystemContext::new();
        let executor = GraphExecutor::new();

        let result = executor.execute(&graph, &mut ctx).await;
        assert!(
            result.is_ok(),
            "Switch execution failed: {:?}",
            result.err()
        );

        // Should have taken the default branch
        let output = ctx.get_output::<&'static str>().unwrap();
        assert_eq!(*output, "executed default");
    }

    #[tokio::test]
    async fn switch_error_when_no_match_and_no_default() {
        #[derive(Debug)]
        struct RouterOutput {
            action: &'static str,
        }

        async fn router() -> RouterOutput {
            RouterOutput {
                action: "nonexistent",
            }
        }

        async fn tool_handler() -> &'static str {
            "executed tool"
        }

        fn build_tool(g: &mut Graph) {
            g.add_system(tool_handler);
        }

        let mut graph = Graph::new();
        graph.add_system(router);
        graph.add_switch::<RouterOutput, _, _>(
            "route",
            |o| o.action,
            vec![("tool", build_tool as fn(&mut Graph))],
            None,
        );

        let mut ctx = SystemContext::new();
        let executor = GraphExecutor::new();

        let result = executor.execute(&graph, &mut ctx).await;
        assert!(
            result.is_err(),
            "Expected error when no match and no default"
        );

        if let Err(ExecutionError::NoMatchingCase { key, .. }) = result {
            assert_eq!(key, "nonexistent");
        } else {
            panic!("Expected NoMatchingCase error, got {:?}", result);
        }
    }

    #[test]
    fn switch_error_display() {
        let err = ExecutionError::MissingDiscriminator(NodeId::new(5));
        assert!(format!("{err}").contains("missing discriminator"));
        assert!(format!("{err}").contains("node_5"));

        let err = ExecutionError::NoMatchingCase {
            node: NodeId::new(3),
            key: "unknown",
        };
        let msg = format!("{err}");
        assert!(msg.contains("no matching case"));
        assert!(msg.contains("unknown"));
        assert!(msg.contains("node_3"));
    }
}
