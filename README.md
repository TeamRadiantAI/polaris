# Polaris

> **Pre-Alpha** — Layers 1-2 are functional. Layer 3 (LLM providers, tools, agent plugins) is not yet implemented. APIs may change. See [Project Status](#project-status).

A Rust framework for building AI agents as composable directed graphs.

## Why Polaris?

**Building performant AI agents is a design problem, not a technical problem.**

The bottleneck isn't compute or APIs — it's discovering the right control flow, tool strategy, and memory architecture. That requires rapid experimentation: try a design, observe, adjust, repeat.

Most frameworks give you a fixed loop and ask you to fill in the blanks. Polaris gives you **building blocks** and lets you design the loop itself.

- **Agents are designs, not programs** — graphs you iterate on, not code you debug
- **Composition over inheritance** — swap reasoning, tools, or memory independently
- **Everything is a plugin** — no built-in behavior, everything replaceable
- **Type-safe at compile time** — invalid designs fail fast, not in production

For the full rationale, see [philosophy.md](docs/philosophy.md).

## How It Works

Agents are directed graphs of async functions. The graph *is* the design.

```rust
struct ReActAgent;

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
                     |g| { g.add_system(invoke_tool).add_system(observe); },
                     |g| { g.add_system(respond); },
                 );
            },
        );
    }
}
```

Same pattern, different tools, different models — the graph structure is portable. Systems are pure async functions with dependency injection. See [agents.md](docs/design_patterns/agents.md), [system.md](docs/design_patterns/system.md), and [graph.md](docs/design_patterns/graph.md).

## Project Status

| Component | Status |
|-----------|--------|
| Layer 1: System Framework (Systems, Resources, Plugins, Server) | **Implemented** |
| Layer 2: Graph Execution (Nodes, Edges, Executor, Hooks) | **Implemented** |
| Layer 2: Agent Trait | **Implemented** |
| Layer 3: LLM Providers, Tool Registry, Agent Plugins | Planned |
| Sessions, Groups, CLI/HTTP Interfaces | Planned |

## Getting Started

> Not yet on crates.io — install from git.

```toml
[dependencies]
polaris = { git = "https://github.com/TeamRadiantAI/polaris" }
```

**Requires** Rust 1.93.0+ (Edition 2024). Run `cargo make test` for the full suite.

## Documentation

| | |
|-|-|
| [philosophy.md](docs/philosophy.md) | Design principles and rationale |

## License

Apache-2.0 — [Repository](https://github.com/TeamRadiantAI/polaris)
