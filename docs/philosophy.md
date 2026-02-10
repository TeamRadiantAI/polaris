# Polaris Core Philosophy

## Why Polaris Exists

We believe that building performant AI agents is a design problem, not a technical problem. The bottleneck in agent development is not compute, APIs, or infrastructure; it's discovering the right design, the right control flow, the right tool selection strategy, the right memory architecture, the right interaction protocol.

Most agent frameworks ship with a fixed execution loop and a set of opinions about how agents should work. When a use case doesn't fit those opinions, developers end up fighting the framework. Polaris takes a different approach: it provides composable primitives and stays out of the way.

Finding the right design requires rapid experimentation: designing an agent, observing its behavior, adjusting the approach, and iterating. This principle governs every design decision in Polaris. If a feature does not enable faster iteration on agent design, it does not belong in the framework.

## Core Architecture

### ECS-Inspired Systems

In most agent frameworks, an agent is an object — it has methods, holds internal state, and controls its own behavior. This makes agents hard to test, hard to inspect, and hard to compose.

Polaris separates behavior from state, borrowing from the [Entity Component System (ECS)](https://en.wikipedia.org/wiki/Entity_component_system) pattern used in game engines like [Bevy](https://bevy.org). The idea is simple: state lives in shared **resources**, and behavior lives in **systems** — pure functions that declare what resources they need. The framework resolves those dependencies and runs the systems.

This means:

- **State is always visible.** Resources live in a central registry. There is nothing hidden inside an object.
- **Behavior is always testable.** Systems are pure functions with explicit inputs. They can be tested like any other function.
- **Composition is straightforward.** Adding behavior means adding another system. No inheritance, no method overrides.

The `SystemParam` trait enables this by automatically resolving function parameters at runtime.

For multi-agent scenarios, Polaris extends the single-world ECS model with hierarchical contexts. Each agent gets its own context with isolated state, while retaining access to shared global resources through a parent-child context chain. This allows multiple agents to run concurrently without interfering with one another.

### Graph-Based Execution

Most frameworks express agent logic as imperative code — hardcoded loops, scattered conditionals, control flow buried across methods and callbacks. This makes behavior hard to see, hard to change, and hard to reason about.

In Polaris, an agent is a directed graph of async functions:

- **Nodes** are units of work: an LLM call, a tool invocation, a decision point.
- **Edges** define control flow: sequential, conditional, parallel, or looping.
- **Execution** traverses the graph, passing data from node to node.

This model makes agent behavior explicit and inspectable. It can be inspected, modified, and replaced — in whole or in part. Adding a step means adding a node. Changing the flow means changing an edge. Trying a different pattern means swapping the graph.

### Everything Is a Plugin

The server is a plugin orchestrator — nothing more. Every capability (logging, tracing, I/O, tool execution, memory, LLM providers) is delivered through plugins.

This means that every component is replaceable, testable in isolation, and optional. Any implementation can be swapped with minimal conflicts. This makes it straightforward to experiment with alternatives, run different configurations in parallel, and package proven designs as reusable modules.

### Type Safety First

Polaris prioritizes catching errors at compile time. Input/output types on systems enforce valid data flow, graph connections are verified before execution begins, and resource access patterns are validated through the type system.

Runtime errors from type mismatches should be impossible in a well-typed Polaris program. This compile-time safety is crucial for building reliable agents that behave predictably.
