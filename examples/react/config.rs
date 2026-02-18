//! Agent configuration.

use polaris::system::resource::LocalResource;
use std::path::PathBuf;

/// Configuration for the `ReAct` agent.
#[derive(Debug, Clone)]
pub struct AgentConfig {
    /// Model identifier (e.g., "anthropic/claude-sonnet-4-5-20250929").
    pub model_id: String,
    /// Working directory for sandboxed file operations.
    pub working_dir: PathBuf,
}

impl AgentConfig {
    /// Creates a new agent configuration.
    #[must_use]
    pub fn new(model_id: impl Into<String>, working_dir: impl Into<PathBuf>) -> Self {
        Self {
            model_id: model_id.into(),
            working_dir: working_dir.into(),
        }
    }

    /// Resolves a path relative to the working directory.
    /// Returns `None` if the path escapes the sandbox.
    #[must_use]
    pub fn resolve_path(&self, path: &str) -> Option<PathBuf> {
        let resolved = self.working_dir.join(path);

        match resolved.canonicalize() {
            Ok(canonical) => {
                let working_dir = self.working_dir.canonicalize().ok()?;
                canonical.starts_with(&working_dir).then_some(canonical)
            }
            Err(_) => {
                // File doesn't exist yet - check the parent directory
                let parent = resolved.parent()?;
                let parent_canonical = parent.canonicalize().ok()?;
                let working_dir = self.working_dir.canonicalize().ok()?;
                parent_canonical
                    .starts_with(&working_dir)
                    .then_some(resolved)
            }
        }
    }
}

impl LocalResource for AgentConfig {}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;

    #[test]
    fn new_config() {
        let config = AgentConfig::new("anthropic/claude-sonnet-4-5-20250929", "/tmp");
        assert_eq!(config.model_id, "anthropic/claude-sonnet-4-5-20250929");
        assert_eq!(config.working_dir, PathBuf::from("/tmp"));
    }

    #[test]
    fn resolve_path_within_sandbox() {
        let config = AgentConfig::new("test", env::temp_dir());
        assert!(config.resolve_path("test.txt").is_some());
    }

    #[test]
    fn resolve_path_escape_blocked() {
        let temp_dir = env::temp_dir();
        let config = AgentConfig::new("test", &temp_dir);
        let resolved = config.resolve_path("../../../etc/passwd");
        // Either None or still within temp_dir
        if let Some(path) = resolved {
            assert!(path.starts_with(&temp_dir));
        }
    }
}
