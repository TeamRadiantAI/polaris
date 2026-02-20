//! Compile-time tests for the `#[system]` macro.
//!
//! Uses `trybuild` to verify that the macro produces correct compile errors
//! for invalid usage and compiles successfully for valid usage.

#[test]
fn compile_fail() {
    let t = trybuild::TestCases::new();
    t.compile_fail("tests/compile_fail/*.rs");
}

#[test]
fn compile_pass() {
    let t = trybuild::TestCases::new();
    t.pass("tests/compile_pass/*.rs");
}
