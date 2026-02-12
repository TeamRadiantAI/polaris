# polaris_graph

Graph execution primitives for Polaris (Layer 2).

## Overview

`polaris_graph` provides the core abstractions for defining agents as directed graphs of systems:

- **Graph** - Directed graph structure with builder API
- **Node** - Vertices representing computation units (systems, decisions, loops, etc.)
- **Edge** - Connections defining control flow between nodes

## Core Concepts

### Node Types

| Node | Purpose |
|------|---------|
| `SystemNode` | Executes a system function |
| `DecisionNode` | Routes flow based on predicate (binary branch) |
| `SwitchNode` | Routes flow based on discriminator (multi-way branch) |
| `ParallelNode` | Executes multiple paths concurrently |
| `LoopNode` | Repeats subgraph until termination condition |
| `JoinNode` | Aggregates results from parallel paths |

### Edge Types

| Edge | Purpose |
|------|---------|
| `Sequential` | A -> B, output flows to input |
| `Conditional` | A -> B if predicate, else A -> C |
| `Parallel` | A -> [B, C, D] concurrently |
| `LoopBack` | Return to earlier node in graph |
| `Error` | Fallback path on failure |
| `Timeout` | Fallback path on timeout |

### Graph Builder

```rust
let mut graph = Graph::new();

graph
    // Sequential systems
    .add_system(step_one)
    .add_system(step_two)

    // Conditional branching
    .add_conditional_branch(
        "branch_name",
        |g| g.add_system(true_path),
        |g| g.add_system(false_path),
    )

    // Parallel execution
    .add_parallel(vec![
        |g| g.add_system(path_a),
        |g| g.add_system(path_b),
    ])

    // Loops
    .add_loop(|g| g.add_system(loop_body));
```

## Design Philosophy

Agents are really just **data flowing through a computation graph**. The graph structure IS the agent's behavior:

- **Explicit control flow** - The graph structure is the source of truth
- **Composable** - Systems are reusable across different agent patterns
- **Inspectable** - Graph structure is data you can visualize and analyze
- **Type-safe** - Node connections verified at build time

See [docs/philosophy.md](../../docs/philosophy.md) for detailed design rationale.

## Execution Semantics

The executor makes the following implicit assumptions:

1. **Implicit termination**: A node without an outgoing edge is a termination point. Execution ends when such a node completes.

2. **Subgraph return**: Control flow nodes (loops, branches, parallel) contain subgraphs. When a subgraph terminates, execution returns to the parent node and continues to its next sequential node.

These rules apply uniformly whether in the root graph or nested within control flow constructs.

## Known Limitations

### Critical

| Limitation | Description | Workaround |
|------------|-------------|------------|
| **No cycle detection** | Graph validation doesn't detect unintended cycles via conditional/parallel edges | Manually verify graph structure doesn't contain cycles |

### Design Constraints

| Limitation | Description | Workaround |
|------------|-------------|------------|
| **Loop predicate priming** | Termination predicates require output type to exist before loop starts | Add an init system before the loop to produce the required output type |
| **Parallel last-write-wins** | Same-typed outputs from parallel branches silently overwrite each other | Use distinct output types per branch, or collect via shared `ResMut<T>` resource |
| **No nested graphs** | Cannot compose graphs or add a `Graph` as a node | Inline all graph logic; no reusable subgraph patterns |
| **No output type validation** | Type mismatches between predicates and system outputs only discovered at runtime | Ensure predicates use the exact output type of the preceding system |
| **Recursion limits** | Deep graph nesting hits stack limits | Flatten deeply nested control flow structures |
| **Subgraph exit constraints** | Subgraphs can only return to their parent node, not to arbitrary points in parent graph | Structure control flow to avoid cross-subgraph jumps |
| **No explicit termination** | Subgraphs cannot terminate the entire parent graph execution | Use error propagation or state-based termination checks |

### Performance

| Limitation | Impact |
|------------|--------|
| **Sequential edge O(E) lookup** | Linear scan for each edge traversal; scales poorly for large graphs |
| **ID offset collision risk** | Hardcoded offsets (1000, 2000, etc.) can collide with deeply nested graphs (~1000 node limit per subgraph) |

### Missing Features

- **Graph introspection**: No DOT export, serialization, or path enumeration
- **Edge operations**: Cannot remove edges or query by target node

See [docs/design_patterns/graph.md](../../docs/design_patterns/graph.md#known-limitations) for detailed documentation.

## License

Apache-2.0
