//! Conversation context management.

use polaris::models::llm::{AssistantBlock, Message, UserBlock};
use polaris::system::resource::LocalResource;

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

    /// Returns `true` if any message contains tool call or tool result blocks.
    pub fn has_tool_blocks(&self) -> bool {
        self.messages.iter().any(|msg| match msg {
            Message::User { content } => content
                .iter()
                .any(|b| matches!(b, UserBlock::ToolResult(_))),
            Message::Assistant { content, .. } => content
                .iter()
                .any(|b| matches!(b, AssistantBlock::ToolCall(_))),
        })
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
