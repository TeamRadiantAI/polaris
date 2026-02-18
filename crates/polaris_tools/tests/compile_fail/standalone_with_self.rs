use polaris_tools::{tool, ToolError};

struct MyStruct;

impl MyStruct {
    #[tool]
    /// A standalone tool should not have &self.
    async fn bad_tool(&self, name: String) -> Result<String, ToolError> {
        Ok(name)
    }
}

fn main() {}
