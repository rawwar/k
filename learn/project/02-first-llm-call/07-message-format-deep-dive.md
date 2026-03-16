---
title: Message Format Deep Dive
description: Understand the full structure of Anthropic message objects including roles, content blocks, and metadata.
---

# Message Format Deep Dive

> **What you'll learn:**
> - How user and assistant messages differ in structure and how content blocks carry text and tool use
> - How to model the complete message type hierarchy in Rust enums and structs
> - How multi-turn conversations are represented as an ordered list of messages with alternating roles

You have seen the basics of the Messages API request and response format. Now it is time to understand the message structure at a deeper level. As your agent grows -- especially when you add tool use in later chapters -- you will need a precise mental model of how messages, roles, and content blocks fit together. This subchapter gives you that model and the Rust types to match it.

## The Message Object

Every message in the Anthropic API has two fields: `role` and `content`. But `content` is more flexible than you might expect. Let's look at both forms.

### Simple String Content

For user messages, `content` can be a plain string:

```json
{
  "role": "user",
  "content": "What is the capital of France?"
}
```

This is the shorthand form. The API accepts it and internally converts it to the structured form.

### Structured Content Blocks

The full form uses an array of content blocks:

```json
{
  "role": "user",
  "content": [
    { "type": "text", "text": "What is the capital of France?" }
  ]
}
```

For user messages, the array can contain `text` blocks and `image` blocks (for multimodal input). For assistant messages, the array can contain `text` blocks and `tool_use` blocks.

The assistant's response always comes back in the structured form:

```json
{
  "role": "assistant",
  "content": [
    { "type": "text", "text": "The capital of France is Paris." }
  ]
}
```

## Roles and Their Rules

The Messages API enforces strict rules about message ordering:

1. **The first message must have role `"user"`.**
2. **Roles must alternate.** A user message is always followed by an assistant message, which is followed by a user message, and so on.
3. **The `system` prompt is not a message.** It is a separate top-level field on the request, not part of the `messages` array.

Here is a valid multi-turn conversation:

```json
{
  "model": "claude-sonnet-4-20250514",
  "max_tokens": 1024,
  "messages": [
    { "role": "user", "content": "What is Rust?" },
    { "role": "assistant", "content": "Rust is a systems programming language..." },
    { "role": "user", "content": "How does it handle memory safety?" }
  ]
}
```

And here is an **invalid** conversation that the API will reject:

```json
{
  "messages": [
    { "role": "user", "content": "Hello" },
    { "role": "user", "content": "Are you there?" }
  ]
}
```

Two consecutive user messages violate the alternation rule. If you need to combine multiple user inputs (for example, if the user sends a follow-up before the assistant responds), concatenate them into a single user message.

::: python Coming from Python
Python's `anthropic` SDK enforces the same rules but you might not notice because you are typically building the messages list manually. In Rust, you will build a type system that makes invalid message sequences harder to construct. The compiler cannot fully enforce alternation at compile time, but well-designed types can guide you toward correctness.
:::

## Modeling Messages in Rust

Let's build Rust types that capture the full flexibility of the message format. Start with the content blocks:

```rust
use serde::{Deserialize, Serialize};

/// A content block within a message. The API supports multiple block types,
/// and a single message can contain several blocks.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum ContentBlock {
    #[serde(rename = "text")]
    Text { text: String },

    #[serde(rename = "tool_use")]
    ToolUse {
        id: String,
        name: String,
        input: serde_json::Value,
    },

    #[serde(rename = "tool_result")]
    ToolResult {
        tool_use_id: String,
        content: String,
    },
}
```

This enum uses `#[serde(tag = "type")]` as you learned in the serde subchapter. Each variant corresponds to a content block type. When you add tool use to your agent later, the `ToolUse` and `ToolResult` variants will be essential.

Now model the content field itself. Remember, user messages can be a plain string or an array of blocks:

```rust
/// Message content can be a simple string or an array of content blocks.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum MessageContent {
    Text(String),
    Blocks(Vec<ContentBlock>),
}
```

The `#[serde(untagged)]` attribute tells serde to try each variant in order. If the JSON value is a string, it matches `Text(String)`. If it is an array, it matches `Blocks(Vec<ContentBlock>)`. This is a powerful serde feature that handles APIs with flexible types.

Now the message itself:

```rust
/// A single message in a conversation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub role: Role,
    pub content: MessageContent,
}

/// The role of a message sender.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    User,
    Assistant,
}
```

Using an enum for `Role` instead of a plain `String` prevents typos like `"User"` or `"assitant"`. The `#[serde(rename_all = "lowercase")]` ensures `User` serializes to `"user"` and `Assistant` to `"assistant"`.

## Helper Methods

Add convenience methods to make working with these types ergonomic:

```rust
impl Message {
    /// Create a user message with plain text content.
    pub fn user(text: impl Into<String>) -> Self {
        Message {
            role: Role::User,
            content: MessageContent::Text(text.into()),
        }
    }

    /// Create an assistant message with plain text content.
    pub fn assistant(text: impl Into<String>) -> Self {
        Message {
            role: Role::Assistant,
            content: MessageContent::Text(text.into()),
        }
    }

    /// Extract all text from this message's content blocks.
    pub fn text(&self) -> String {
        match &self.content {
            MessageContent::Text(s) => s.clone(),
            MessageContent::Blocks(blocks) => blocks
                .iter()
                .filter_map(|block| match block {
                    ContentBlock::Text { text } => Some(text.as_str()),
                    _ => None,
                })
                .collect::<Vec<_>>()
                .join("\n"),
        }
    }
}
```

Now building conversations is clean and readable:

```rust
fn main() {
    let conversation = vec![
        Message::user("What is Rust?"),
        Message::assistant("Rust is a systems programming language focused on safety and performance."),
        Message::user("How does ownership work?"),
    ];

    for msg in &conversation {
        println!("[{:?}] {}", msg.role, msg.text());
    }
}
```

## The Full Request and Response Types

With these building blocks, here is the complete request type:

```rust
#[derive(Debug, Serialize)]
pub struct ChatRequest {
    pub model: String,
    pub max_tokens: u32,
    pub messages: Vec<Message>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub system: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f64>,
}
```

And the complete response type:

```rust
#[derive(Debug, Deserialize)]
pub struct ChatResponse {
    pub id: String,
    #[serde(rename = "type")]
    pub response_type: String,
    pub role: Role,
    pub model: String,
    pub content: Vec<ContentBlock>,
    pub stop_reason: Option<String>,
    pub stop_sequence: Option<String>,
    pub usage: Usage,
}

#[derive(Debug, Deserialize)]
pub struct Usage {
    pub input_tokens: u32,
    pub output_tokens: u32,
}
```

Notice that the response `content` is always `Vec<ContentBlock>` (the structured form), even though request content can be a plain string. The API always returns the structured form.

## Multi-Turn Conversation Flow

Let's trace through what happens during a multi-turn conversation to solidify your understanding:

**Turn 1:** You send one user message.
```rust
let messages = vec![Message::user("What is Rust?")];
// Request messages: [{"role":"user","content":"What is Rust?"}]
```

**Turn 1 response:** Claude responds with text.
```json
{"content": [{"type": "text", "text": "Rust is a systems programming language..."}]}
```

**Turn 2:** You append the assistant response and a new user message.
```rust
messages.push(Message::assistant("Rust is a systems programming language..."));
messages.push(Message::user("How does ownership work?"));
// Request messages now has 3 items, alternating user/assistant/user
```

**Turn 2 response:** Claude responds with context from the entire conversation.

This accumulation pattern is the heart of a conversational agent. Each turn, your `Vec<Message>` grows by two entries (one user, one assistant). The entire vector is sent with every request, giving Claude the full context.

::: wild In the Wild
Claude Code maintains a conversation history that can grow to hundreds of messages during long coding sessions. To manage this, it implements context compaction -- summarizing older messages when the token count approaches the context window limit. OpenCode takes a similar approach. You will implement this pattern in a later chapter; for now, just be aware that the messages vector you are building is the foundation of your agent's memory.
:::

## Validating Message Sequences

Since the API requires alternating roles, it is useful to validate your message sequence before sending:

```rust
fn validate_messages(messages: &[Message]) -> Result<(), String> {
    if messages.is_empty() {
        return Err("Messages array cannot be empty".to_string());
    }

    if messages[0].role != Role::User {
        return Err("First message must be from the user".to_string());
    }

    for window in messages.windows(2) {
        if window[0].role == window[1].role {
            return Err(format!(
                "Messages must alternate roles, found two consecutive {:?} messages",
                window[0].role
            ));
        }
    }

    Ok(())
}
```

Call this before sending your request to catch alternation errors locally instead of getting a 400 error from the API.

## Key Takeaways

- Message content can be a plain string (shorthand for user messages) or an array of typed content blocks (`text`, `tool_use`, `tool_result`), which you model with `#[serde(untagged)]` and `#[serde(tag = "type")]` enums.
- Using a `Role` enum instead of raw strings prevents typos and makes message construction explicit and type-safe.
- Helper methods like `Message::user()` and `Message::assistant()` make building conversations clean and readable.
- Multi-turn conversations are stateless from the API's perspective -- you maintain a `Vec<Message>` and send the full history with every request.
- Validate the message sequence before sending to catch alternation errors locally, rather than waiting for a 400 response from the API.
