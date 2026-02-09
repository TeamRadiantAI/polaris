# Polaris Taxonomy

This document defines the conceptual layers and core abstractions in Polaris. Use this as a reference when deciding where new functionality belongs.

For the design philosophy behind these decisions, see [philosophy.md](./philosophy.md).

## Layered Architecture

Polaris is organized into three distinct layers, each with clear responsibilities:

```markdown
┌─────────────────────────────────────────────────────────────────────┐
│  LAYER 3: Plugin-Provided Abstractions                              │
│                                                                     │
│  Swappable, optional functionality delivered via plugins.           │
│  Users can replace, extend, or omit entirely.                       │
│                                                                     │
│  Examples: ToolsPlugin, LLMPlugin, MemoryPlugin, TracingPlugin      │
├─────────────────────────────────────────────────────────────────────┤
│  LAYER 2: Graph Execution and Agent Patterns                        │
│                                                                     │
│  polaris_graph: Graph execution primitives (Graph, Node, Edge)      │
│  polaris_agent: Agent pattern definition (Agent trait)              │
│                                                                     │
│  Fixed primitives for defining and executing behavior.              │
├─────────────────────────────────────────────────────────────────────┤
│  LAYER 1: System Framework (polaris_system)                         │
│                                                                     │
│  The foundational ECS-inspired primitives. Everything builds        │
│  on top of this. Cannot be replaced or swapped.                     │
│                                                                     │
│  Examples: System, Resource, SystemParam, Plugin, Server, API       │
└─────────────────────────────────────────────────────────────────────┘
```

## Layer 1: System Framework

**Crate**: `polaris_system`

The foundation of Polaris. These primitives define how code is organized and executed. They are **fixed** and cannot be replaced—changing them would mean using a different framework.

### Primitives

| Concept | Description | Example |
| --------- | ------------- | --------- |
| **System** | A unit of execution. Pure async function: inputs → output. | `fn reason(llm: Res<LLM>, prev: Out<Request>) -> Response` |
| **SystemContext** | Hierarchical execution context with parent chain. | `ctx.child()`, `ctx.get_resource::<T>()` |
| **Resource** | Long-lived shared state stored in Resources container. | `struct Memory { context: Vec<String> }` |
| **GlobalResource** | Server-lifetime, read-only resource shared across all agents. | `impl GlobalResource for Config {}` |
| **LocalResource** | Per-context, mutable resource isolated per agent. | `impl LocalResource for Memory {}` |
| **Output** | Ephemeral system return value stored in Outputs container. | `struct ReasoningResult { action: String }` |
| **SystemParam** | Trait enabling dependency injection into systems. | `Res<T>`, `ResMut<T>`, `Out<T>` |
| **SystemAccess** | Access descriptor for conflict detection between systems. | `Res<T>::access()`, `system.access()` |
| **Plugin** | Extension mechanism. Bundles resources, systems, and sub-plugins. | `impl Plugin for TracingPlugin { ... }` |
| **ScheduleId** | Identifier for tick schedules. Layer 2 defines schedules, plugins register. | `ScheduleId::of::<PostAgentRun>()` |
| **API** | Capability registry for plugin orchestration. Not accessed by systems. | `server.api::<AgentAPI>()` |
| **Server** | The runtime. Orchestrates plugins, executes systems, triggers ticks. | `Server::new().add_plugins(...).run()` |

### What Belongs Here

- Core traits: `System`, `Resource`, `Output`, `SystemParam`, `Plugin`, `API`
- Marker traits: `GlobalResource`, `LocalResource`
- `SystemContext`: Hierarchical execution context with parent chain
- Resource containers: `Resources` (long-lived), `Outputs` (ephemeral)
- Access types: `Res<T>`, `ResMut<T>`, `Out<T>`, `Option<...>`
- Access descriptors: `Access`, `AccessMode`, `SystemAccess`
- `ScheduleId`: Tick schedule identifier (Layer 2 defines, Layer 3 registers)
- `API` trait and Server methods: `insert_api()`, `api()`
- Server implementation with `insert_global()`, `register_local()`, `create_context()`, `tick()`
- Borrow tracking and conflict detection
- System scheduling primitives

### What Does NOT Belong Here

- Anything agent-specific (graphs, LLMs, tools)
- Anything domain-specific (memory backends, providers)
- Anything optional (if users might not need it, it's a plugin)

---

## Layer 2: Graph Execution and Agent Patterns

**Crates**: `polaris_graph`, `polaris_agent`

The graph execution model and agent pattern definitions. These crates define how behavior is structured as graphs of systems and provide the standard interface for agent patterns.

### Graph Primitives (`polaris_graph`)

| Concept | Description | Example |
| --------- | ------------- | --------- |
| **Graph** | Directed graph of systems defining behavior. | `Graph::new().add_system(reason).add_system(act)` |
| **Node** | A vertex in the graph. Wraps a system with metadata. | Decision node, parallel node, loop node |
| **Edge** | Connection between nodes defining control flow. | Sequential, conditional, parallel, loop-back |
| **GraphExecutor** | Runtime that executes graphs with context. | `executor.execute(&graph, &ctx).await` |

### Agent Pattern (`polaris_agent`)

| Concept | Description | Example |
| --------- | ------------- | --------- |
| **Agent** | Trait for defining reusable behavior patterns. | `impl Agent for ReAct { fn build(&self, graph: &mut Graph) }` |
| **AgentExt** | Extension trait for creating graphs from agents. | `agent.to_graph()` |

### Orchestration APIs (Layer 2)

| Concept | Description | Example |
| --------- | ------------- | --------- |
| **AgentAPI** | Registry for agent types. Plugins register agents here. | `server.api::<AgentAPI>().register("react", agent)` |
| **SessionAPI** | Manages session lifecycle (create, run, destroy). | `server.api::<SessionAPI>().create("react")` |
| **GroupAPI** | Manages session groups for shared state. | `server.api::<GroupAPI>().create("team")` |

### Node Types

These are the fundamental building blocks for agent graphs:

| Node Type | Purpose |
| --------- | --------- |
| **SystemNode** | Executes a system function |
| **DecisionNode** | Routes flow based on predicate (binary branch) |
| **SwitchNode** | Routes flow based on discriminator (multi-way branch) |
| **ParallelNode** | Executes multiple paths concurrently |
| **LoopNode** | Repeats subgraph until termination condition |
| **JoinNode** | Aggregates results from parallel paths |

### Edge Types

| Edge Type | Purpose |
| --------- | --------- |
| **Sequential** | A → B, output flows to input |
| **Conditional** | A → B if predicate, else A → C |
| **Parallel** | A → [B, C, D] concurrently |
| **LoopBack** | Return to earlier node in graph |
| **Error** | Fallback path on failure |
| **Timeout** | Fallback path on timeout |

### What Belongs in `polaris_graph`

- `Graph` structure and builder API
- Node and edge type definitions
- Graph validation and cycle detection
- Graph execution engine (`GraphExecutor`)
- Runtime context (iteration count, custom state)

### What Belongs in `polaris_agent`

- `Agent` trait definition
- `AgentExt` extension trait

### What Belongs in Layer 2 (either crate or separate)

- **API implementations**: `AgentAPI`, `SessionAPI`, `GroupAPI`
  - Layer 1 provides generic `API` trait and `server.api::<A>()`
  - Layer 2 defines concrete APIs for agent orchestration
- **Schedule definitions**: Marker types for tick schedules (e.g., `PostAgentRun`, `PreTurn`)
  - Layer 1 provides `ScheduleId` and `Server::tick()`
  - Layer 2 defines the actual schedules and when to trigger them
- **Scope semantics**: What "Agent", "Session", "Turn" mean in context hierarchy
  - Layer 1 provides generic hierarchical `SystemContext`
  - Layer 2 defines lifecycle and meaning of each scope level

### What Does NOT Belong Here

- Specific agent implementations (ReAct, ReWOO) → Layer 3 plugins
- Tool/LLM/Memory abstractions → Layer 3 plugins
- I/O mechanisms → Layer 3 plugins

---

## Layer 3: Plugin-Provided Abstractions

**Crates**: Various plugin crates (`polaris_tools`, `polaris_llm`, `polaris_memory`, etc.)

Optional functionality delivered via plugins. Users can mix and match, replace implementations, or omit entirely. Each plugin may define its own traits and provide default implementations.

### Standard Plugins

| Plugin | Purpose | Key Resources |
|--------|---------|---------------|
| **ToolsPlugin** | Tool registration and execution | `ToolRegistry`, `Tool` trait |
| **LLMPlugin** | LLM abstraction and providers | `LLM`, `LLMProvider` trait |
| **MemoryPlugin** | Agent memory and context | `Memory`, `MemoryBackend` trait |
| **TracingPlugin** | Logging and observability | `Tracer` |
| **IOPlugin** | Input/output abstractions | `InputBuffer`, `OutputBuffer`, `UserIO` |

See [plugins.md](./design_patterns/plugins.md#core-plugins) for implementations.

### Agent Pattern Plugins

Concrete agent implementations (ReAct, ReWOO, etc.) are delivered as plugins that depend on capability plugins above.

See [agents.md](./design_patterns/agents.md#agent-pattern-plugins) for implementations.

### What Belongs in Layer 3

- Domain-specific traits (Tool, LLMProvider, MemoryBackend)
- Implementations of those traits (OpenAI, Redis, etc.)
- SystemParams that access plugin-provided resources
- Concrete agent implementations (ReAct, ReWOO, etc.)
- Utility plugins (tracing, metrics, I/O)
- Integration plugins (external services, APIs)

---

## Decision Guide

When adding new functionality, ask:

```markdown
Is it fundamental to how code executes?
├─ YES → Layer 1 (polaris_system)
│        Examples: new SystemParam type, new resource access pattern
│
└─ NO → Is it fundamental to how agents are structured?
        ├─ YES → Layer 2 (polaris_graph)
        │        Examples: new node type, new edge type, graph feature
        │
        └─ NO → Layer 3 (Plugin)
                Examples: new tool, new LLM provider, new agent pattern
```

**Key principle**: If in doubt, make it a plugin. The framework should be minimal, with functionality delivered through composition of plugins.

### Examples

| Feature | Layer | Rationale |
| --------- | ------- | ----------- |
| `GlobalResource` | 1 | Core resource scoping marker |
| `LocalResource` | 1 | Core resource scoping marker |
| `ResMut<T>` | 1 | Core resource access pattern |
| `SystemAccess` | 1 | Core conflict detection primitive |
| `ScheduleId` | 1 | Core tick schedule identifier |
| `Server::tick()` | 1 | Core tick mechanism |
| `Graph` | 2 | Core agent execution primitive |
| `ConditionalEdge` | 2 | Core control flow primitive |
| Schedule marker types | 2 | Layer 2 defines schedules and when to trigger |
| Scope lifecycle | 2 | Agent/Session/Turn semantics |
| `Tool` trait | 3 | Optional capability, users might not use tools |
| `OpenAIProvider` | 3 | Specific implementation, swappable |
| `ReActAgent` | 3 | Specific agent pattern, optional |
| `MetricsPlugin` | 3 | Optional observability feature |
