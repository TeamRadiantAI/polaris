//! Agent loop state.

use polaris::system::resource::LocalResource;

/// Tracks whether the agent loop should continue.
#[derive(Debug, Clone, Copy, Default)]
pub struct ReactState {
    /// Whether the loop is complete.
    pub is_complete: bool,
}

impl LocalResource for ReactState {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_not_complete() {
        assert!(!ReactState::default().is_complete);
    }
}
