//! Compile-fail tests for `#[tool]` and `#[toolset]` macro validation.

#[test]
fn compile_fail() {
    let t = trybuild::TestCases::new();
    t.compile_fail("tests/compile_fail/*.rs");
}
