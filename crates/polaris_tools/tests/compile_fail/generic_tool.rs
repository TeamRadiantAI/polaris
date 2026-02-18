use polaris_tools::{tool, ToolError};

#[tool]
/// A generic tool should be rejected.
async fn generic_tool<T: ToString>(name: T) -> Result<String, ToolError> {
    Ok(name.to_string())
}

fn main() {}
