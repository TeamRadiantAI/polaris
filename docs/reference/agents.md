# Agent Trait

The `polaris_agent` crate defines a minimal abstraction for defining reusable agent behavior patterns. An agent is a type that knows how to build a graph.

## Overview

The `Agent` trait has one required method: `build`, which receives a mutable reference to a `Graph` and populates it with systems and control flow. This provides a standard interface for packaging a behavior pattern — such as ReAct, ReWOO, or a custom design — as a self-contained, reusable unit

```rust
pub trait Agent: Send + Sync + 'static {
    fn build(&self, graph: &mut Graph);

    fn name(&self) -> &str {
        core::any::type_name::<Self>()
    }
}
```

See [graph.md](./graph.md) for further details on building graphs.

The `AgentExt` extension trait is implemented for all `Agent` types and provides `to_graph()`, which constructs a new `Graph` and passes it to `build()`:

```rust
pub trait AgentExt: Agent {
    fn to_graph(&self) -> Graph {
        let mut graph = Graph::new();
        self.build(&mut graph);
        graph
    }
}

impl<T: Agent> AgentExt for T {}
```

## Usage

Defining an agent means implementing `build` to describe the desired graph topology. The following example implements a ReAct agent that loops through reasoning, tool use, and observation until the task is complete:

```rust
struct ReActAgent;

impl Agent for ReActAgent {
    fn build(&self, graph: &mut Graph) {
        graph.add_system(init);

        graph.add_loop::<ReactState, _, _>(
            "react_loop",
            |state| state.is_complete,
            |g| {
                g.add_system(reason);
                g.add_conditional_branch::<ReasoningResult, _, _, _>(
                    "action",
                    |result| result.action == Action::UseTool,
                    |tool_branch| {
                        tool_branch.add_system(select_tool);
                        tool_branch.add_system(execute_tool);
                        tool_branch.add_system(observe);
                    },
                    |respond_branch| {
                        respond_branch.add_system(respond);
                    },
                );
            },
        );
    }

    fn name(&self) -> &str { "ReActAgent" }
}
```

Executing an agent is the responsibility of the caller. The typical pattern is to build the graph, create a context from the server, and pass both to a `GraphExecutor`:

```rust
let mut server = Server::new();
server.add_plugins(DefaultPlugins);
server.add_plugins(MyModelPlugin);
server.finish();

let graph = ReActAgent.to_graph();
let mut ctx = server.create_context();

let executor = GraphExecutor::new();
executor.execute(&graph, &mut ctx, None).await?;
```

## Packaging as Plugins

To deliver a concrete agent implementation as a distributable unit, Agents are packaged as a plugin. The plugin registers the resources the agent's systems depend on (LLM providers, tool registries, memory) and declares its dependencies on other plugins.

See [plugins.md](./plugins.md) for plugin structure and lifecycle, and `crates/example` for a complete ReAct agent.
