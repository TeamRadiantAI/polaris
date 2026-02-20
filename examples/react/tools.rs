//! Tool definitions using the `polaris_tools` framework.

use crate::config::AgentConfig;
use polaris::system::plugin::{Plugin, PluginId, Version};
use polaris::system::server::Server;
use polaris::tools::{ToolError, ToolRegistry, ToolsPlugin, toolset};

/// File operation tools for the sandboxed working directory.
pub struct FileTools {
    /// Agent configuration for path resolution.
    config: AgentConfig,
}

impl FileTools {
    /// Creates a new `FileTools` with the given agent configuration.
    pub fn new(config: AgentConfig) -> Self {
        Self { config }
    }
}

#[toolset]
impl FileTools {
    /// List files in a directory.
    #[tool]
    async fn list_files(
        &self,
        /// Directory path (relative to working directory).
        path: String,
    ) -> Result<String, ToolError> {
        let resolved = self
            .config
            .resolve_path(&path)
            .ok_or_else(|| ToolError::execution_error(format!("path '{path}' escapes sandbox")))?;

        let entries = std::fs::read_dir(&resolved)
            .map_err(|err| ToolError::execution_error(err.to_string()))?;

        let files: Vec<String> = entries
            .filter_map(Result::ok)
            .map(|entry| entry.file_name().to_string_lossy().to_string())
            .collect();

        Ok(if files.is_empty() {
            "(empty directory)".to_string()
        } else {
            files.join("\n")
        })
    }

    /// Read the contents of a file.
    #[tool]
    async fn read_file(
        &self,
        /// File path (relative to working directory).
        path: String,
    ) -> Result<String, ToolError> {
        let resolved = self
            .config
            .resolve_path(&path)
            .ok_or_else(|| ToolError::execution_error(format!("path '{path}' escapes sandbox")))?;

        std::fs::read_to_string(&resolved)
            .map_err(|err| ToolError::execution_error(err.to_string()))
    }

    /// Write content to a file.
    #[tool]
    async fn write_file(
        &self,
        /// File path (relative to working directory).
        path: String,
        /// Content to write.
        content: String,
    ) -> Result<String, ToolError> {
        let resolved = self
            .config
            .resolve_path(&path)
            .ok_or_else(|| ToolError::execution_error(format!("path '{path}' escapes sandbox")))?;

        std::fs::write(&resolved, &content)
            .map_err(|err| ToolError::execution_error(err.to_string()))?;

        Ok(format!("Wrote to {}", resolved.display()))
    }
}

/// Plugin that registers [`FileTools`] with the [`ToolRegistry`].
pub struct FileToolsPlugin {
    /// Agent configuration to capture into the tools.
    config: AgentConfig,
}

impl FileToolsPlugin {
    /// Creates a new `FileToolsPlugin` with the given agent configuration.
    pub fn new(config: AgentConfig) -> Self {
        Self { config }
    }
}

impl Plugin for FileToolsPlugin {
    const ID: &'static str = "examples::file_tools";
    const VERSION: Version = Version::new(0, 0, 1);

    fn dependencies(&self) -> Vec<PluginId> {
        vec![PluginId::of::<ToolsPlugin>()]
    }

    fn build(&self, server: &mut Server) {
        let mut registry = server
            .get_resource_mut::<ToolRegistry>()
            .expect("ToolsPlugin must be added before FileToolsPlugin");
        registry.register_toolset(FileTools::new(self.config.clone()));
    }
}
