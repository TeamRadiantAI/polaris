use polaris_tools::{tool, toolset, ToolError};

struct MyTools;

#[toolset]
impl MyTools {
    #[tool]
    /// &mut self is not supported.
    async fn bad_method(&mut self, name: String) -> Result<String, ToolError> {
        Ok(name)
    }
}

fn main() {}
