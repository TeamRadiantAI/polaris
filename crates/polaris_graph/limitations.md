# Graph Topology Limitations Analysis

This document captures current limitations and potential improvements in the `polaris_graph` topology system.

---

## 1. Loops

### Current Implementation

- `LoopNode` with `body_entry`, `exit`, termination predicate, and `max_iterations`
- Termination checked BEFORE each iteration
- Body executed via `execute_subgraph`

### Critical Bug

- `loop_node.exit` is never set by the builder (`graph.rs:add_loop()`)
- All loops fail with `MissingExit` on normal completion
- Tests work around this by verifying iteration counts via error paths

### Conceptual Limitations

| Issue | Description |
|-------|-------------|
| Pre-check only | Termination checked before iteration, no post-iteration check option |
| No break/continue | Can't exit early from within loop body based on intermediate results |
| Single body structure | No support for loops with multiple phases (e.g., setup → main → cleanup) |
| No loop state accumulation | Each iteration starts fresh; no built-in pattern for accumulating state across iterations |
| Nested loop data flow | Inner loop outputs don't cleanly propagate to outer loops |

### Questions to Resolve

- Should loops support do-while (post-check) semantics?
- Should there be explicit break and continue control flow edges?
- How should loop iteration state accumulate? Via `LocalResource`? Via special `LoopState<T>`?

---

## 2. Parallel / Fork Branches

### Current Implementation

- `ParallelNode` with `branches: Vec<NodeId>` and `join: Option<NodeId>`
- `JoinNode` with `sources: Vec<NodeId>`
- Executes via `try_join_all` — all branches start simultaneously
- Each branch gets isolated child context

### Critical Limitation: No Merge Strategy

Currently:

```markdown
                    ┌── Branch A ──┐
Main ─→ Parallel ───┼── Branch B ──┼─→ Join ─→ Next
                    └── Branch C ──┘
```

- Outputs from branches are isolated in child contexts
- Nothing is merged back to the parent context after join
- The `JoinNode` is purely a synchronization point, not a result aggregator

### What's Missing

| Missing Feature | Description |
|-----------------|-------------|
| Merge strategy | How to combine outputs from branches |
| Partial failure handling | What if 2/3 branches succeed? |
| First-wins / All-wins semantics | Race vs. barrier completion modes |
| Dynamic fan-out | Number of branches determined at runtime |
| Branch result aggregation | Collecting outputs into a `Vec` or merged type |

### Potential Merge Strategies

```rust
pub enum MergeStrategy {
    /// Discard all branch outputs (current behavior)
    None,

    /// Collect all outputs into Vec<T> where all branches return T
    Collect,

    /// Use first completed result, cancel others
    Race,

    /// Custom merge function: Vec<Output> -> MergedOutput
    Custom(BoxedMerger),

    /// Require all branches to return same type, keep last
    LastWins,

    /// Each branch writes to named slot, merged into struct
    Named { slots: Vec<&'static str> },
}
```

### Questions to Resolve

- Should `JoinNode` have a merge function?
- How to type-check heterogeneous branch outputs?
- What about partial failure — allow n of m success patterns?
- Should there be a `select!`-style first-completion mode?

---

## 3. Switches

### Current Implementation

- `SwitchNode` with discriminator returning `&'static str` case key
- Linear search through `cases: Vec<(&'static str, NodeId)>`
- Optional default fallback
- Each case is a subgraph

### Limitations

| Issue | Description |
|-------|-------------|
| Static keys only | Case keys must be `'static str`, can't be runtime values |
| No fallthrough | Each case is isolated, no C-style fallthrough |
| No multi-match | Can't route to same handler for multiple keys |
| Convergence unclear | After switch, where does each branch go? |
| Limited discriminator return | Can only return single key, not pattern matching |

### Questions to Resolve

- Should switch support pattern matching (ranges, wildcards)?
- Should cases be able to fall through or share handlers?
- How should switch branches converge after execution?
- Should there be a typed discriminator variant (enum instead of string)?

---

## 4. Other Topology Issues

### 4.1 Branch Convergence (Critical)

After Decision, Switch, or Parallel nodes, where do branches go?

```markdown
Current:           What users expect:

  ┌─→ A            ┌─→ A ─┐
  │                │      │
──┼                │      ├─→ Continue
  │                │      │
  └─→ B            └─→ B ─┘

  (A and B are     (Both converge
   terminal)        to Continue)
```

**Problem**: The builder doesn't automatically wire branch exits to the next node.

### 4.2 Dynamic Graph Modification

- Graphs are static after construction
- No way to add nodes/edges during execution
- Some agent patterns need runtime graph expansion (e.g., tool discovery)

### 4.3 Subgraph Composition

- No first-class "subgraph" type for reuse
- ID offset strategy (1000, 2000, 3000...) is fragile for deep nesting
- Can't easily compose pre-built graph fragments

### 4.4 Output/Data Flow Model

- Outputs stored by `TypeId` — only one output per type globally
- If two systems return same type, later overwrites earlier
- No explicit data flow edges (data flows via `Out<T>` lookup)
- Parallel branches can't see each other's outputs

### 4.5 Error Propagation in Nested Structures

- Error edges only handle immediate node failures
- No propagation of errors through nested loops/parallels
- No way to catch errors at a higher level

### 4.6 Cancellation / Interruption

- No way to cancel a running graph mid-execution
- Timeout is per-node, not per-subgraph or per-graph
- No interruption points for user intervention

---

## Suggested Plan for Improvements

### Phase 1: Fix Critical Bugs

1. Fix loop exit wiring — `add_loop()` should track and set exit when next node added
2. Add explicit convergence — After branches, auto-wire to next sequential node

### Phase 2: Parallel Merge Semantics

1. Add `MergeStrategy` enum to `ParallelNode`
2. Implement `Collect` strategy (homogeneous outputs → `Vec`)
3. Implement `Race` strategy (first completion wins)
4. Add `JoinResult<T>` type for accessing branch outputs

### Phase 3: Enhanced Loop Constructs

1. Add post-check loop variant (`do_while`)
2. Add `BreakEdge` and `ContinueEdge` types
3. Add `LoopAccumulator<T>` for state across iterations
4. Consider `for_each` pattern for iterating over collections

### Phase 4: Improved Switch/Branch

1. Add multi-key matching (multiple keys → same handler)
2. Add explicit convergence node after switch
3. Consider typed discriminators (enums vs strings)
4. Document branch convergence patterns

### Phase 5: Data Flow Model

1. Add named outputs: `Out<T, "name">` or output slots
2. Add explicit data flow edges (make dependency visible in graph)
3. Consider output scoping (per-subgraph vs global)

### Phase 6: Subgraph Composition

1. First-class `Subgraph` type that can be reused
2. Better ID allocation (use hierarchical IDs, not offsets)
3. Subgraph imports/exports for explicit interfaces

---

## Philosophy Considerations

From `philosophy.md`:

> "Graphs are design artifacts ... changes are structural, not textual"

This suggests the graph topology should be:

- **Visually representable** — Can you draw it?
- **Statically analyzable** — Can tools validate it?
- **Composable** — Can you build from smaller pieces?

The current limitations around merge strategies, convergence, and data flow make the "design artifact" less clear. When reading a graph, you can't tell:

- Where data flows between nodes
- How parallel results combine
- Where branches converge

**Recommendation**: Make these explicit in the graph structure, not implicit in execution semantics.
