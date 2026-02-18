use polaris_tools::{tool, ToolError};

#[tool]
/// An extern tool should be rejected.
async extern "C" fn extern_tool(name: String) -> Result<String, ToolError> {
    Ok(name)
}

fn main() {}
