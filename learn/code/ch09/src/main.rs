// Chapter 9: Context Management — Code snapshot

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Message {
    role: String,
    content: serde_json::Value,
}

/// Manages the conversation context, including token counting and truncation.
struct ConversationManager {
    messages: Vec<Message>,
    max_tokens: usize,
    // TODO: Add system prompt storage
    // TODO: Add token counting
}

impl ConversationManager {
    fn new(max_tokens: usize) -> Self {
        Self {
            messages: Vec::new(),
            max_tokens,
        }
    }

    fn add_message(&mut self, message: Message) {
        self.messages.push(message);
        // TODO: Check token count and truncate if needed
    }

    fn get_messages(&self) -> &[Message] {
        &self.messages
    }

    // TODO: Implement context window truncation strategy
    // TODO: Implement conversation summarization
    // TODO: Implement token counting (estimate or exact)
}

#[tokio::main]
async fn main() {
    println!("Chapter 9: Context Management");

    let mut manager = ConversationManager::new(100_000);
    manager.add_message(Message {
        role: "user".to_string(),
        content: serde_json::Value::String("Hello!".to_string()),
    });

    println!("Messages in context: {}", manager.get_messages().len());
    println!("TODO: Implement context window management");
}
