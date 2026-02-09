# Polaris Core Philosophy

This document captures the **design philosophy and rationale** behind Polaris. For implementation details, see the referenced documents.

## Why Polaris Exists

**Building performant AI agents is a design problem, not a technical problem.**

The bottleneck in agent development is not compute, APIs, or infrastructure—it's discovering the right design. The right control flow. The right tool selection strategy. The right memory architecture. The right interaction protocol.

Finding these answers requires **rapid experimentation**. Try a design, observe behavior, adjust, repeat. The faster this loop, the faster you converge on effective agents.

## Core Philosophy

### ECS-Inspired Architecture

Polaris borrows heavily from Entity Component System (ECS) patterns, particularly Bevy's approach:

- **Systems are pure functions** - No hidden state, all dependencies explicitly declared via function parameters
- **Resources are shared state** - Stored in a central registry, accessed through typed handles
- **Dependency injection via traits** - The `SystemParam` trait enables automatic parameter resolution at runtime
- **Hierarchical contexts** - Parent-child context chains enable multi-agent isolation

This differs from traditional OOP agent frameworks where agents are objects with methods and internal state. In Polaris, agent behavior emerges from the composition of stateless systems operating on shared resources.

Unlike single-world ECS frameworks (like Bevy), Polaris is designed for **multi-agent scenarios** where multiple agents execute concurrently with isolated state. This is achieved through hierarchical `SystemContext` chains where each agent gets its own context with access to parent (global) resources.

See [system.md](./design_patterns/system.md) for implementation details.

### Graph-Based Execution Model

Agents are defined as **directed graphs of async functions**:

- **Nodes** represent units of computation (LLM calls, tool invocations, decisions)
- **Edges** define control flow (sequential, conditional, parallel, loops)
- **Execution** traverses the graph, passing data between nodes

This model makes agent behavior explicit and inspectable. The graph structure is the source of truth for what an agent does, not scattered imperative code. See [graph.md](./design_patterns/graph.md) for implementation details.

See [agents.md](./design_patterns/agents.md) for agent pattern implementations.

### Type Safety First

Polaris prioritizes compile-time correctness:

- Input/output types on systems enforce valid data flow
- `SystemParam` implementations validate resource access patterns
- Graph connections are verified before execution begins

Runtime errors from type mismatches should be impossible in a well-typed Polaris program. This compile-time safety is crucial for building reliable agents that behave predictably.

### Everything is a Plugin

There is no "built-in" functionality in Polaris. The server is a plugin orchestrator—nothing more. Every capability—logging, tracing, I/O abstractions, tool execution, memory management, LLM providers—is delivered through plugins.

This means:

- **Replaceable**: Swap any implementation without touching other code
- **Testable**: Use mock plugins in tests
- **Minimal**: Include only what you need
- **Extensible**: Add domain-specific functionality as first-class plugins

See [plugins.md](./design_patterns/plugins.md) for the plugin system and lifecycle.

This architecture directly serves rapid iteration:

- **Experiment with alternatives**: Composable plugins make it easy to try different implementations
- **A/B test designs**: Run different plugin configurations in parallel
- **Isolate changes**: Modify one capability without touching others
- **Share patterns**: Package proven designs as reusable plugins

## Design-First Philosophy

### Agents Are Designs, Not Programs

An agent is not code to be written—it's a **design to be discovered**. The code is just the representation of that design.

This perspective shift changes how we build:

| Traditional View | Polaris View |
|-----------------|--------------|
| Write code that does X | Design a flow that achieves X |
| Debug the implementation | Iterate on the design |
| Optimize the code | Experiment with alternatives |
| Ship the program | Deploy the best-performing design |

### The Design Space

Agent design has many dimensions:

- **Control Flow**: When to reason, when to act, when to loop
- **Tool Strategy**: Which tools, in what order, with what fallbacks
- **Memory Architecture**: What to remember, what to forget, how to retrieve
- **Interaction Protocol**: How to communicate with users and other agents
- **Error Handling**: How to recover, retry, or escalate

Each dimension has countless valid choices. Polaris doesn't prescribe answers—it provides primitives that let you explore the space efficiently.

### Rapid Iteration as Core Value

Every Polaris design decision is evaluated against: **Does this enable faster iteration on agent design?**

- Graph-based execution: Because flows are easier to modify than imperative code
- Plugin architecture: Because swapping components should be trivial
- Type-safe composition: Because invalid designs should fail fast, not at runtime
- Explicit control flow: Because you can't improve what you can't see

If a feature doesn't help users iterate faster on designs, it doesn't belong in the framework.

## Compositional Design

### Building Blocks, Not Blueprints

Polaris provides **building blocks**, not complete solutions:

```text
┌─────────────────────────────────────────────────────────────┐
│  Your Agent Design                                          │
│  ┌─────────┐  ┌─────────┐  ┌─────────┐  ┌─────────┐         │
│  │ Pattern │ +│ Pattern │ +│ Pattern │ +│ Custom  │ = Agent │
│  │    A    │  │    B    │  │    C    │  │  Logic  │         │
│  └─────────┘  └─────────┘  └─────────┘  └─────────┘         │
└─────────────────────────────────────────────────────────────┘
```

You don't subclass a "BaseAgent" and override methods. You compose:
- A reasoning pattern (ReAct, ReWOO, tree-of-thought)
- A tool execution strategy (sequential, parallel, validated)
- A memory system (conversation, episodic, semantic, or something entirely different)
- Custom logic specific to your domain

### Patterns Are Portable

Define a pattern once, reuse it everywhere:

```rust
// Define ReAct pattern as a reusable graph
impl Agent for ReActAgent {
    fn build(&self, graph: &mut Graph) {
        graph.add_loop::<ReActState, _, _>(
            "react_loop",
            |state| state.is_complete,
            |g| {
                g.add_system(reason)
                 .add_conditional_branch::<ReasoningResult, _, _, _>(
                     "check_action",
                     |r| r.action == Action::UseTool,
                     |g| g.add_system(invoke_tool).add_system(observe),
                     |g| g.add_system(respond),
                 );
            },
        );
    }
}

// Reuse in different contexts with different configurations
let support_agent = ReActAgent { tools: support_tools, llm: gpt4 };
let code_agent = ReActAgent { tools: code_tools, llm: claude };
let research_agent = ReActAgent { tools: search_tools, llm: gemini };
```

Same pattern, different tools, different models—the graph structure is portable.

### Experimentation Through Composition

Rapid iteration means trying alternatives:

```rust
// Monday's design
let agent_v1 = Agent::new()
    .with_pattern(ReAct)
    .with_memory(ConversationMemory);

// Tuesday's experiment
let agent_v2 = Agent::new()
    .with_pattern(ReWOO)           // Changed reasoning
    .with_memory(SemanticMemory);  // Changed memory

// Compare performance, keep what works
```

Composition makes this trivial. No refactoring, no rewrites—just swap components.

### Configuration Over Code

For rapid iteration, configuration beats code:

```yaml
# Kubernetes-style agent manifest
apiVersion: polaris.ai/v1
kind: Agent
metadata:
  name: support-bot
  labels:
    team: customer-success
    environment: production
spec:
  pattern: react
  maxIterations: 5
  tools:
    - name: search_docs
      timeout: 5s
    - name: create_ticket
    - name: escalate
  memory:
    type: conversation
    maxTokens: 4096
  validation:
    - name: check_pii
      action: redact
    - name: check_tone
      action: warn
```

Change the manifest, apply, observe. No recompilation, no redeployment.

(Note: See [plugins.md](./design_patterns/plugins.md) for configuration plugins.)

## Fundamental Design Principles

### Resource Immutability by Default

Resources are read-only unless explicitly marked mutable:

- `Res<T>` - Shared immutable borrow, multiple systems can read concurrently
- `ResMut<T>` - Exclusive mutable borrow, only one system can write

Resources are further classified by scope:

- `GlobalResource` - Server-lifetime, read-only, shared across all agents
- `LocalResource` - Per-context, mutable, isolated per agent

This classification provides **compile-time safety**: `ResMut<T>` requires `T: LocalResource`, making `ResMut<GlobalResource>` a compile error. This prevents accidental mutation of shared state and enables the runtime to parallelize independent systems safely.

### Zero-Cost Abstractions

Polaris leverages Rust's zero-cost abstraction principle:

- Trait-based polymorphism compiles to static dispatch where possible
- Graph structures are resolved at setup time, not during execution
- No runtime reflection or dynamic typing overhead

The abstractions exist for developer ergonomics but compile away to efficient code.

## Key Architectural Patterns

These patterns implement the design principles above. For detailed documentation:

- **SystemParam Pattern**: GAT-based dependency injection. See [system.md](./design_patterns/system.md#systemparam).
- **Hierarchical Resource Model**: Context chains with global/local scope. See [system.md](./design_patterns/system.md#hierarchical-resource-model).
- **Graph Construction**: Sequential, conditional, parallel, loop patterns. See [graph.md](./design_patterns/graph.md#agentgraph-construction-patterns).
- **Borrow Tracking**: Compile-time conflict detection for safe parallelization. See [system.md](./design_patterns/system.md#resources).

## Design Decisions & Rationale

### Why ECS Over Traditional OOP?

Traditional agent frameworks use objects with methods where internal state is hidden and hard to inspect, testing requires mocking object internals, and composition requires inheritance or delegation patterns.

The ECS approach inverts these problems:
- State lives in resources—visible and testable
- Systems are pure functions—easy to test in isolation
- Composition is just adding more systems

### Why Graph-Based Over Imperative Loops?

Imperative agent loops bury control flow in conditionals, making behavior hard to visualize, modify, or extend.

The graph-based approach makes control flow explicit:
- Control flow *is* the structure itself
- Visually inspectable and tooling-friendly
- Add new paths by adding edges, not rewriting loops

Critically, graphs are **design artifacts**:
- They can be visualized, compared, versioned
- Non-programmers can understand agent behavior
- Tools can analyze, validate, and optimize them
- Changes are structural, not textual

This makes agent design accessible beyond just programmers—product managers, domain experts, and designers can participate in agent design by understanding and modifying graph structures.

### Why Compile-Time Over Runtime Validation?

Runtime validation catches errors during execution—potentially in production after deployment.

Compile-time validation catches errors at build:
- Errors surface immediately during development
- Type system documents valid usage
- IDE support for correct code

## Anti-Patterns to Avoid

| Anti-Pattern | Problem | Solution |
|--------------|---------|----------|
| **Hidden Mutation** | Interior mutability (e.g., `RefCell`) bypasses borrow tracking | Use `ResMut<T>` for explicit mutable access |
| **Circular Dependencies** | Systems cannot have circular output dependencies | Break cycles by writing to shared resource, reading next iteration |
| **Blocking in Async** | Blocking ops (e.g., `std::thread::sleep`) block the executor | Use async equivalents (e.g., `tokio::time::sleep`) |
| **Premature Abstraction** | Generic frameworks before understanding real needs | Start concrete, extract patterns after repetition |
| **Premature Optimization** | Optimizing before finding the right design | First find a design that works, then optimize. A fast wrong design is still wrong. |
| **Coupling to Implementation** | Designing agents around specific LLM/tool implementations | Design against abstractions. Implementations should be swappable. |

## Extending the Framework

When extending Polaris, follow these guiding principles:

### Creating Plugins

Plugins are the primary extension mechanism:
1. Use `build()` for setup, `ready()` for post-build initialization
2. Declare dependencies explicitly via `dependencies()`
3. Keep lifecycle methods lightweight—heavy logic belongs in systems
4. Prefer composition: build complex plugins from simpler sub-plugins

See [plugins.md](./design_patterns/plugins.md) for complete plugin patterns.

### Creating New Agent Patterns

1. Study existing patterns in [agents.md](./design_patterns/agents.md)
2. Compose from existing graph primitives where possible
3. Only add new node/edge types if primitives are insufficient

### Adding New Node Types

Before adding a new node type, verify it cannot be expressed as:
- A system function with appropriate `SystemParam`s
- A composition of existing node types
- A conditional branch with specific predicate

New node types should represent fundamentally new computation models, not convenience wrappers.

## References

- [taxonomy.md](./taxonomy.md) - Layered architecture and concept classification
- [system.md](./design_patterns/system.md) - Layer 1: System primitives
- [api.md](./design_patterns/api.md) - Layer 1: API primitive for capability registration
- [graph.md](./design_patterns/graph.md) - Layer 2: Graph execution primitives
- [sessions.md](./design_patterns/sessions.md) - Sessions and multi-agent coordination
- [interfaces.md](./design_patterns/interfaces.md) - External interaction patterns
- [plugins.md](./design_patterns/plugins.md) - Plugin system and compositional architecture
- [agents.md](./design_patterns/agents.md) - Agent pattern implementations
