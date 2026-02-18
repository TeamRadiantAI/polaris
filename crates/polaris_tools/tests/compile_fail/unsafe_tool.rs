use polaris_tools::{tool, ToolError};

#[tool]
/// An unsafe tool should be rejected.
async unsafe fn unsafe_tool(name: String) -> Result<String, ToolError> {
    Ok(name)
}

fn main() {}
