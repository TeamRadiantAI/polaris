//! Integration tests for the `#[system]` macro.
//!
//! These tests are in a separate integration test file because the macro
//! generates code using `::polaris_system::` paths, which only work when
//! the crate is used as an external dependency.

use polaris_system::param::{Out, Res, ResMut, SystemContext};
use polaris_system::resource::LocalResource;
use polaris_system::system;
use polaris_system::system::System;

#[derive(Debug, PartialEq, Clone)]
struct TestOutput {
    value: i32,
}

#[derive(Debug, PartialEq)]
struct Counter {
    count: i32,
}

impl LocalResource for Counter {}

#[derive(Debug, PartialEq)]
struct Config {
    multiplier: i32,
}

impl LocalResource for Config {}

// ─────────────────────────────────────────────────────────────────────────────
// Single parameter system
// ─────────────────────────────────────────────────────────────────────────────

#[system]
async fn macro_read_counter(counter: Res<Counter>) -> TestOutput {
    TestOutput {
        value: counter.count,
    }
}

#[tokio::test]
async fn macro_single_param_system() {
    let system = macro_read_counter();
    let ctx = SystemContext::new().with(Counter { count: 42 });

    let result = system.run(&ctx).await.unwrap();
    assert_eq!(result.value, 42);
}

// ─────────────────────────────────────────────────────────────────────────────
// Multiple parameter system
// ─────────────────────────────────────────────────────────────────────────────

#[system]
async fn macro_compute(counter: Res<Counter>, config: Res<Config>) -> TestOutput {
    TestOutput {
        value: counter.count * config.multiplier,
    }
}

#[tokio::test]
async fn macro_multi_param_system() {
    let system = macro_compute();
    let ctx = SystemContext::new()
        .with(Counter { count: 7 })
        .with(Config { multiplier: 6 });

    let result = system.run(&ctx).await.unwrap();
    assert_eq!(result.value, 42);
}

// ─────────────────────────────────────────────────────────────────────────────
// Mutable resource system
// ─────────────────────────────────────────────────────────────────────────────

#[system]
async fn macro_increment(mut counter: ResMut<Counter>) -> TestOutput {
    counter.count += 1;
    TestOutput {
        value: counter.count,
    }
}

#[tokio::test]
async fn macro_mutable_resource_system() {
    let system = macro_increment();
    let ctx = SystemContext::new().with(Counter { count: 0 });

    let result = system.run(&ctx).await.unwrap();
    assert_eq!(result.value, 1);

    let result2 = system.run(&ctx).await.unwrap();
    assert_eq!(result2.value, 2);
}

// ─────────────────────────────────────────────────────────────────────────────
// Output chain system
// ─────────────────────────────────────────────────────────────────────────────

#[system]
async fn macro_double_output(prev: Out<TestOutput>) -> TestOutput {
    TestOutput {
        value: prev.value * 2,
    }
}

#[tokio::test]
async fn macro_output_chain_system() {
    let system = macro_double_output();
    let mut ctx = SystemContext::new();
    ctx.insert_output(TestOutput { value: 21 });

    let result = system.run(&ctx).await.unwrap();
    assert_eq!(result.value, 42);
}

// ─────────────────────────────────────────────────────────────────────────────
// System name
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn macro_system_has_correct_name() {
    let system = macro_read_counter();
    assert_eq!(System::name(&system), "macro_read_counter");
}
