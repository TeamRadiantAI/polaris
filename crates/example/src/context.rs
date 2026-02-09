//! Conversation context management.

use polaris_models::llm::Message;
use polaris_system::resource::LocalResource;

/// Manages conversation history for the agent.
#[derive(Debug, Clone, Default)]
pub struct ContextManager {
    /// The conversation message history.
    pub messages: Vec<Message>,
}

impl ContextManager {
    /// Creates a new context with an initial user request.
    #[must_use]
    pub fn new(request: impl Into<String>) -> Self {
        Self {
            messages: vec![Message::user(request)],
        }
    }

    /// Adds a message to the history.
    pub fn push(&mut self, message: Message) {
        self.messages.push(message);
    }
}

impl LocalResource for ContextManager {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_context_has_user_message() {
        let ctx = ContextManager::new("What is 2+2?");
        assert_eq!(ctx.messages.len(), 1);
    }

    #[test]
    fn push_messages() {
        let mut ctx = ContextManager::new("test");
        ctx.push(Message::assistant("response"));
        assert_eq!(ctx.messages.len(), 2);
    }
}
