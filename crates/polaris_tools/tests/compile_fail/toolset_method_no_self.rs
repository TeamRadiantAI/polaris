use polaris_tools::{tool, toolset, ToolError};

struct MyTools;

#[toolset]
impl MyTools {
    #[tool]
    /// Missing &self receiver.
    async fn bad_method(name: String) -> Result<String, ToolError> {
        Ok(name)
    }
}

fn main() {}
