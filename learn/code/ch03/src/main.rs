// Chapter 3: The Agentic Loop — Code snapshot

use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
struct Message {
    role: String,
    content: String,
}

/// Run the agentic loop: send messages, check for tool use, execute tools, repeat.
async fn agentic_loop(_messages: &mut Vec<Message>) {
    // TODO: Send conversation to the LLM
    // TODO: Check if the response contains a tool_use block
    // TODO: If tool_use, execute the tool and append the result
    // TODO: If no tool_use (end_turn), break and return the final response
    // TODO: Loop until the agent stops calling tools

    println!("TODO: Implement the agentic loop");
}

#[tokio::main]
async fn main() {
    println!("Chapter 3: The Agentic Loop");

    let mut messages = vec![Message {
        role: "user".to_string(),
        content: "Hello, agent!".to_string(),
    }];

    // TODO: Run the agentic loop with the conversation
    agentic_loop(&mut messages).await;
}
