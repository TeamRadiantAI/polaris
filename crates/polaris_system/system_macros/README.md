# system_macros

Procedural macros for the `polaris_system` crate.

## Overview

This crate provides the `#[system]` attribute macro that transforms async functions into `System` implementations, solving Rust's lifetime limitations with Higher-Ranked Trait Bounds (HRTB) and async functions.

## The Problem

When defining systems with lifetime-parameterized parameters like `Res<'_, T>`, Rust's type system cannot express the relationship between input lifetimes and async return types:

```rust
// This doesn't work due to Rust's E0582 error:
// "lifetime 'w in return type doesn't appear in input types"
for<'w> F: Fn(Res<'w, T>) -> BoxFuture<'w, O>
```

## The Solution

The `#[system]` macro generates a struct that implements `System` directly, bypassing the HRTB limitation:

```rust
use polaris_system_macros::system;

#[system]
async fn read_counter(counter: Res<Counter>) -> Output {
    Output { value: counter.count }
}
```

This generates:

```rust
struct ReadCounterSystem;

impl System for ReadCounterSystem {
    type Output = Output;

    fn run<'a>(&'a self, ctx: &'a SystemContext<'_>)
        -> BoxFuture<'a, Result<Self::Output, SystemError>>
    {
        Box::pin(async move {
            let counter = Res::<Counter>::fetch(ctx)?;
            Ok({ Output { value: counter.count } })
        })
    }

    fn name(&self) -> &'static str {
        "read_counter"
    }
}

fn read_counter() -> ReadCounterSystem {
    ReadCounterSystem
}
```

## Usage

```rust
use polaris_system::param::{Res, ResMut, Out};
use polaris_system::system; // Macro is re-exported from polaris_system for convenience

// Single parameter
#[system]
async fn read_config(config: Res< Config>) -> ConfigData {
    ConfigData::from(&*config)
}

// Multiple parameters
#[system]
async fn process(
    input: Res<Input>,
    config: Res<Config>,
) -> ProcessedOutput {
    ProcessedOutput::new(&input, &config)
}

// Mutable resource
#[system]
async fn increment(mut counter: ResMut<Counter>) -> i32 {
    counter.value += 1;
    counter.value
}

// Reading previous output
#[system]
async fn transform(prev: Out<PreviousResult>) -> NextResult {
    NextResult::from(&*prev)
}
```

## Generated Code

For a function `foo_bar`, the macro generates:

| Generated Item | Description |
|----------------|-------------|
| `FooBarSystem` | Unit struct (PascalCase from snake_case) |
| `impl System for FooBarSystem` | The `System` trait implementation |
| `fn foo_bar() -> FooBarSystem` | Factory function returning the system |

## Requirements

- Functions must be `async`
- Parameters must be simple identifiers (no patterns)
- Currently designed for use within `polaris_system` crate (uses `crate::` paths)

## License

Apache-2.0
