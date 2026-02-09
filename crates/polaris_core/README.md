# polaris_core

Core infrastructure plugins for Polaris applications.

## Overview

This crate provides foundational plugins that most Polaris applications need. It is part of **Layer 1** infrastructure, sitting alongside `polaris_system`.

## Plugins

| Plugin | Resources | Scope | Purpose |
|--------|-----------|-------|---------|
| `ServerInfoPlugin` | `ServerInfo` | Global | Server metadata (version, debug mode) |
| `TimePlugin` | `Clock`, `Stopwatch` | Global, Local | Time utilities with mockable clock |
| `TracingPlugin` | `TracingConfig` | Global | Logging and observability via `tracing` |

## Quick Start

```rust
use polaris_system::server::Server;
use polaris_system::plugin::PluginGroup;
use polaris_core::DefaultPlugins;

// Use DefaultPlugins for typical applications
Server::new()
    .add_plugins(DefaultPlugins.build())
    .run();
```

## Plugin Groups

### DefaultPlugins

Includes all infrastructure plugins - use for most applications:

- `ServerInfoPlugin`
- `TimePlugin`
- `TracingPlugin`

### MinimalPlugins

Lightweight bundle for testing (no tracing output):

- `ServerInfoPlugin`
- `TimePlugin`

```rust
use polaris_core::MinimalPlugins;

// Great for unit tests
Server::new()
    .add_plugins(MinimalPlugins.build())
    .run();
```

## Individual Plugin Configuration

### ServerInfoPlugin

Provides `ServerInfo` resource with version and debug mode:

```rust
use polaris_core::{ServerInfoPlugin, ServerInfo};
use polaris_system::param::Res;

#[system]
async fn check_mode(info: Res<ServerInfo>) {
    if info.debug {
        // Enable extra diagnostics
    }
}
```

### TimePlugin

Provides `Clock` (global, mockable) and `Stopwatch` (per-agent timer):

```rust
use polaris_core::{TimePlugin, Clock, Stopwatch};
use polaris_system::param::{Res, ResMut};

#[system]
async fn timed_work(clock: Res<Clock>, mut sw: ResMut<Stopwatch>) {
    let start = clock.now();
    // ... work ...
    sw.lap();
    println!("Elapsed: {:?}", sw.elapsed());
}
```

**Testing with MockClock:**

MockClock allows deterministic time control in tests, enabling testing time-dependent systems without real delays. For example:

|Plugins Use Cases|Example|
|-----------------|-------|
|Timeout testing|Set mock time to trigger timeouts|
|Rate Limiting|Advance mock time to test rate limits|
|Cache TTL|Simulate cache expiration by advancing time|
|Scheduled tasks|Control execution timing of scheduled jobs|

```rust
use std::sync::Arc;
use std::time::{Duration, Instant};
use polaris_core::{TimePlugin, MockClock};

let mock = Arc::new(MockClock::new(Instant::now()));
let plugin = TimePlugin::with_clock(mock.clone());

// In tests, advance time without waiting
mock.advance(Duration::from_secs(60));
```

### TracingPlugin

Configures `tracing` subscriber with multiple output formats:

```rust
use polaris_core::{TracingPlugin, TracingFormat};
use tracing::Level;

// Development: colored pretty output
let dev = TracingPlugin::default()
    .with_level(Level::DEBUG)
    .with_format(TracingFormat::Pretty);

// Production: JSON for log aggregation
let prod = TracingPlugin::default()
    .with_level(Level::INFO)
    .with_format(TracingFormat::Json)
    .with_env_filter("polaris=info,hyper=warn");
```

## When to Use Each Plugin

| Scenario | Recommended Setup |
|----------|-------------------|
| Production application | `DefaultPlugins` |
| Unit tests (no logs) | `MinimalPlugins` |
| Integration tests | `MinimalPlugins` + `TracingPlugin::default().with_level(Level::WARN)` |
| Deterministic time tests | `MinimalPlugins` with `TimePlugin::with_clock(mock)` |
| Log aggregation (ELK, etc.) | `TracingPlugin::default().with_format(TracingFormat::Json)` |

## Feature Flags

| Feature | Description |
|---------|-------------|
| `test-utils` | Enables `MockClock` for library consumers |

## Planned Infrastructure Plugins

These plugins are planned for future `polaris_core` releases:

| Plugin | Resources | Purpose |
|--------|-----------|---------|
| `MetricsPlugin` | `MetricsRegistry` | Application metrics collection and export |
| `TurnCountPlugin` | `TurnCount` | Track agent execution turns/steps |
| `RandomPlugin` | `Rng` | Seedable RNG for reproducible agent behavior |

## Layer 2/3 Plugin Examples

These plugins are typically implemented at higher layers or by users. See [plugins.md](../../docs/design_patterns/plugins.md) for full documentation.

### Capability Plugins

| Plugin | Resources | Scope | Purpose |
|--------|-----------|-------|---------|
| `ToolsPlugin` | `ToolRegistry` | Global | Tool registration and execution |
| `MemoryPlugin` | `MemoryConfig`, `MemoryManager` | Global, Local | Agent memory and context management |
| `IOPlugin` | `InputBuffer`, `OutputBuffer` | Local | Input/output abstractions per agent |

### LLM Provider Plugins

| Plugin | Resources | Purpose |
|--------|-----------|---------|
| `OpenAIPlugin` | `LLM` | OpenAI API integration |
| `AnthropicPlugin` | `LLM` | Anthropic API integration |

```rust
// Swap LLM providers without changing agent code
Server::new()
    .add_plugins(DefaultPlugins.build())
    .add_plugins(OpenAIPlugin {
        api_key: env::var("OPENAI_API_KEY").unwrap(),
        model: "gpt-4".into(),
        base_url: None,
    })
    .run();
```

### Agent Pattern Plugins

| Plugin | Pattern | Description |
|--------|---------|-------------|
| `ReActAgentPlugin` | ReAct | Interleaved reasoning and acting with observation loops |
| `ReWOOAgentPlugin` | ReWOO | Plan all tools upfront, execute in parallel |
| `LLMCompilerPlugin` | LLM Compiler | Dependency-analyzed multi-stage parallel execution |

### Budget & Rate Limiting Plugins

| Plugin | Resources | Purpose |
|--------|-----------|---------|
| `TokenBudgetPlugin` | `TokenBudget` | Track/limit token consumption per agent |
| `CostTrackingPlugin` | `CostTracker` | Track API costs across LLM calls |
| `RateLimitPlugin` | `RateLimiter` | Handle API rate limits gracefully |

## Dependencies

This crate depends on:

- `polaris_system` - Core system framework
- `tracing` - Logging facade
- `tracing-subscriber` - Subscriber implementation

## Architecture

```text
Layer 1 (Infrastructure)
├── polaris_system    # Core primitives (System, Resource, Plugin, Server)
└── polaris_core      # Infrastructure plugins (this crate)

Layer 2 (Execution)
└── polaris_graph     # Graph-based agent execution

Layer 3 (Plugins)
└── (your plugins)    # Agent implementations, tools, LLMs
```

## License

Apache-2.0
