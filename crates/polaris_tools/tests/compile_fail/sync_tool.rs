use polaris_tools::{tool, ToolError};

#[tool]
/// A sync tool should be rejected.
fn sync_tool(name: String) -> Result<String, ToolError> {
    Ok(name)
}

fn main() {}
