---
title: Conversation State
description: Manage the growing message history that forms the context for each successive LLM call in the loop.
---

# Conversation State

> **What you'll learn:**
> - How to store conversation history as a `Vec<Message>` and append new messages after each loop iteration
> - How the full message history is sent with every API call and why context window limits matter
> - How to implement basic context window management by tracking token counts and truncating when necessary

The conversation state is the model's memory. Every time you call the API, you send the entire conversation history -- every user message, every assistant response, every tool result. The model has no memory between calls; it reconstructs its understanding of the task from the messages you send. This means the conversation state is not just a log of what happened. It is the *input* to each LLM call, and its quality directly affects the model's ability to do its job.

## The Message History Vector

At its simplest, conversation state is a vector of messages:

```rust
pub struct ConversationState {
    /// The system prompt, sent separately from the message history.
    pub system_prompt: String,

    /// The ordered list of messages exchanged so far.
    pub messages: Vec<Message>,

    /// Running total of tokens consumed across all API calls.
    pub total_input_tokens: u32,
    pub total_output_tokens: u32,
}

impl ConversationState {
    pub fn new(system_prompt: impl Into<String>) -> Self {
        ConversationState {
            system_prompt: system_prompt.into(),
            messages: Vec::new(),
            total_input_tokens: 0,
            total_output_tokens: 0,
        }
    }

    /// Add the user's initial message to start a conversation.
    pub fn add_user_message(&mut self, text: &str) {
        self.messages.push(Message::user(text));
    }

    /// Add the assistant's response (the full content blocks from the API).
    pub fn add_assistant_message(&mut self, content: Vec<ContentBlock>) {
        self.messages.push(Message::assistant(content));
    }

    /// Add tool results after executing tool calls.
    pub fn add_tool_results(&mut self, results: Vec<ContentBlock>) {
        self.messages.push(Message::tool_results(results));
    }

    /// Record token usage from an API response.
    pub fn record_usage(&mut self, usage: &Usage) {
        self.total_input_tokens += usage.input_tokens;
        self.total_output_tokens += usage.output_tokens;
    }
}
```

This is straightforward, but let's think about what happens as the conversation grows.

## How the State Grows

Consider a typical agent interaction where the user asks: "Add error handling to src/main.rs."

After the first loop iteration, the state looks like this:

```text
messages[0]: User    "Add error handling to src/main.rs"
messages[1]: Asst    [Text("I'll read the file first."), ToolUse(read_file)]
```

After the tool execution and second iteration:

```text
messages[0]: User    "Add error handling to src/main.rs"
messages[1]: Asst    [Text("I'll read the file first."), ToolUse(read_file)]
messages[2]: User    [ToolResult("fn main() { ... }")]
messages[3]: Asst    [Text("I'll update the error handling."), ToolUse(write_file)]
```

After the third iteration:

```text
messages[0]: User    "Add error handling to src/main.rs"
messages[1]: Asst    [Text("I'll read the file first."), ToolUse(read_file)]
messages[2]: User    [ToolResult("fn main() { ... }")]
messages[3]: Asst    [Text("I'll update the error handling."), ToolUse(write_file)]
messages[4]: User    [ToolResult("File written successfully.")]
messages[5]: Asst    [Text("I've updated the file. Let me verify it compiles."), ToolUse(run_command)]
```

And after the fourth:

```text
messages[0..5]: (same as above)
messages[6]: User    [ToolResult("cargo check: OK")]
messages[7]: Asst    [Text("Done! I've added proper error handling...")]
```

That is 8 messages for a fairly simple task. Complex tasks can easily generate 30-50 messages. Each message includes the full content -- a file read might return thousands of characters of source code. All of this is sent to the API on every call.

## The Context Window Problem

Every LLM has a context window -- a maximum number of tokens it can process in a single request. Claude's context window is large (200K tokens for Claude 3.5 Sonnet), but it is not infinite. And even well within the window, more tokens means higher latency and higher cost.

The problem is that tokens accumulate fast in an agentic loop:

- A file read tool returning a 500-line source file might use 2,000-3,000 tokens
- Each API call's prompt includes the *entire* history, so those file contents are re-sent every time
- A 10-turn interaction might accumulate 20,000+ input tokens

You need to track this. Let's add a rough token estimation method:

```rust
impl ConversationState {
    /// Estimate the total token count of the current message history.
    /// This is a rough heuristic: ~4 characters per token for English text.
    /// For precise counts, use a proper tokenizer like tiktoken.
    pub fn estimate_token_count(&self) -> usize {
        let mut chars = self.system_prompt.len();
        for msg in &self.messages {
            for block in &msg.content {
                chars += match block {
                    ContentBlock::Text { text } => text.len(),
                    ContentBlock::ToolUse { name, input, .. } => {
                        name.len() + input.to_string().len()
                    }
                    ContentBlock::ToolResult { content, .. } => content.len(),
                };
            }
        }
        // Rough estimate: 1 token per 4 characters
        chars / 4
    }

    /// Check if the conversation is approaching the context window limit.
    pub fn is_approaching_limit(&self, max_context_tokens: usize) -> bool {
        let estimated = self.estimate_token_count();
        // Warn at 80% capacity to leave room for the next response
        estimated > (max_context_tokens * 80) / 100
    }
}
```

::: python Coming from Python
In Python, you might use the `tiktoken` library for precise token counting:
```python
import tiktoken
enc = tiktoken.encoding_for_model("claude-sonnet-4-20250514")
token_count = sum(len(enc.encode(str(msg))) for msg in messages)
```
Rust does not have a built-in `tiktoken` equivalent in the standard ecosystem, so most Rust agents use character-based heuristics or call out to a tokenizer library. The 4-characters-per-token estimate is good enough for deciding when to truncate. For production use, you would use the `input_tokens` count returned by the API itself, which gives you the exact number.
:::

## Basic Truncation Strategy

When the conversation gets too long, you need to drop some messages. The simplest strategy is to keep the system prompt and the most recent messages, dropping the oldest ones:

```rust
impl ConversationState {
    /// Truncate the message history to fit within the token budget.
    /// Keeps the first message (original user request) and the most recent
    /// messages, dropping the middle.
    pub fn truncate_to_fit(&mut self, max_context_tokens: usize) {
        while self.estimate_token_count() > max_context_tokens && self.messages.len() > 2 {
            // Remove the second message (index 1), preserving the first
            // user message and the most recent messages.
            self.messages.remove(1);
        }
    }
}
```

This is a naive approach -- it can break the tool-use/tool-result pairing if you remove an assistant message but keep its corresponding tool result. A production agent needs smarter truncation that preserves message pairs. You will build a much more sophisticated context management system in Chapter 9. For now, this is enough to prevent the loop from blowing past the context window.

::: wild In the Wild
Claude Code implements a sophisticated context compaction system. When the conversation approaches the context limit, it summarizes older messages rather than simply dropping them. This preserves the model's understanding of what happened earlier without paying the full token cost. OpenCode takes a different approach, implementing a sliding window that keeps the most recent N messages plus any messages that contain important tool results (like file reads that the model is still referencing). Both approaches are significantly more sophisticated than simple truncation, and you will explore similar strategies in Chapter 9.
:::

## State Invariants

Your conversation state must maintain certain invariants for the API to accept the request:

1. **Messages must alternate correctly.** After an assistant message with a tool use, the next message must be a user message containing the corresponding tool results. You cannot send two assistant messages in a row or two user messages in a row (with one exception: the first message must always be a user message).

2. **Every tool_use must have a matching tool_result.** If the assistant's response contains three tool-use blocks with IDs `a`, `b`, `c`, the next user message must contain tool-result blocks for all three IDs.

3. **Tool results reference valid tool-use IDs.** The `tool_use_id` in each tool result must match the `id` from the corresponding tool-use block.

Let's add a validation method:

```rust
impl ConversationState {
    /// Validate that the message history is well-formed.
    /// Returns Ok(()) if valid, or an error describing the problem.
    pub fn validate(&self) -> Result<(), String> {
        if self.messages.is_empty() {
            return Err("Message history is empty".to_string());
        }

        // First message must be from the user
        if self.messages[0].role != Role::User {
            return Err("First message must be from the user".to_string());
        }

        // Check alternation (simplified -- real check would verify
        // tool_use/tool_result pairing)
        for window in self.messages.windows(2) {
            if window[0].role == Role::Assistant
                && window[1].role == Role::Assistant
            {
                return Err(
                    "Two consecutive assistant messages found".to_string()
                );
            }
        }

        Ok(())
    }
}
```

## Passing State to the API

When you call the API, you pass the system prompt separately from the messages:

```rust
use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct ApiRequest {
    pub model: String,
    pub max_tokens: u32,
    pub system: String,
    pub messages: Vec<Message>,
}

impl ConversationState {
    /// Build an API request from the current conversation state.
    pub fn to_api_request(&self, model: &str, max_tokens: u32) -> ApiRequest {
        ApiRequest {
            model: model.to_string(),
            max_tokens,
            system: self.system_prompt.clone(),
            messages: self.messages.clone(),
        }
    }
}
```

This is where you see the full picture: the conversation state is literally serialized into the API request body. Every message, every content block, every tool result -- it all goes over the wire on every call. That is why managing the size of this state matters.

## Key Takeaways

- Conversation state is a `Vec<Message>` that is sent in full with every API call -- the model has no memory between calls, so the message history is the only context it has
- The state grows rapidly in an agentic loop: a 4-turn tool interaction can easily produce 8+ messages with thousands of tokens of content
- Token counting (even rough estimates) is essential for avoiding context window overflows that cause API errors or degraded model performance
- Message history must maintain structural invariants: proper alternation between roles, matching tool-use/tool-result pairs, and a user message first
- Simple truncation works as a starting point, but production agents need smarter strategies (summarization, sliding windows) that preserve message pair integrity -- you will build these in Chapter 9
