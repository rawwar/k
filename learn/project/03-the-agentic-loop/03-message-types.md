---
title: Message Types
description: Define the Rust types that represent user messages, assistant messages, tool-use requests, and tool results.
---

# Message Types

> **What you'll learn:**
> - How to model the four message roles (system, user, assistant, tool_result) as Rust enum variants
> - How content blocks within a message can be text, tool_use, or tool_result and how to represent that with enums
> - How to make your message types serialize and deserialize cleanly for both API calls and debug logging

The agentic loop moves messages back and forth between your agent and the LLM. Every message has a *role* (who said it) and *content* (what they said). But content is not always plain text. An assistant message might contain a text block followed by a tool-use request. A user message responding to a tool call contains tool-result blocks, not text. Getting these types right is the foundation for everything else in this chapter.

Let's design the type system that represents these messages in Rust.

## The Anthropic Message Format

The Anthropic Messages API uses a structured format where each message has a `role` and a `content` field. The content field is an array of *content blocks*, each with a `type` that determines its shape. Here are the four content block types you need to handle:

1. **Text block** -- Plain text from either the user or the assistant. `{ "type": "text", "text": "Hello!" }`

2. **Tool use block** -- The assistant requesting a tool call. Contains a unique ID, the tool name, and the input arguments as JSON. `{ "type": "tool_use", "id": "toolu_01A...", "name": "read_file", "input": { "path": "src/main.rs" } }`

3. **Tool result block** -- The user (your agent) responding with the output of a tool call. Contains the matching tool-use ID, the output content, and an optional error flag. `{ "type": "tool_result", "tool_use_id": "toolu_01A...", "content": "fn main() { ... }" }`

4. **Image block** -- Binary image data, which we will not use in our coding agent but should be aware of.

A critical insight: a single assistant message can contain *multiple* content blocks of different types. The model might output some explanatory text, then a tool-use request, then more text, then another tool-use request. Your types must model this as a vector of content blocks, not a single string.

## Modeling Content Blocks as an Enum

Rust enums with data are the perfect fit for content blocks. Each variant holds exactly the data that content block type needs:

```rust
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// A single content block within a message.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum ContentBlock {
    /// Plain text content from user or assistant.
    #[serde(rename = "text")]
    Text {
        text: String,
    },

    /// A tool-use request from the assistant.
    #[serde(rename = "tool_use")]
    ToolUse {
        id: String,
        name: String,
        input: Value,
    },

    /// A tool result sent back from the agent to the model.
    #[serde(rename = "tool_result")]
    ToolResult {
        tool_use_id: String,
        content: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        is_error: Option<bool>,
    },
}
```

The `#[serde(tag = "type")]` attribute tells serde to use an internally tagged representation. When this serializes to JSON, serde uses the `type` field to distinguish between variants -- exactly how the Anthropic API formats it. The `#[serde(rename = "...")]` attributes map each Rust variant name to the API's snake_case names.

Let's verify this works with a round-trip test:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_text_block_serialization() {
        let block = ContentBlock::Text {
            text: "Hello, world!".to_string(),
        };
        let json = serde_json::to_value(&block).unwrap();
        assert_eq!(json, json!({
            "type": "text",
            "text": "Hello, world!"
        }));
    }

    #[test]
    fn test_tool_use_deserialization() {
        let json = json!({
            "type": "tool_use",
            "id": "toolu_01ABC",
            "name": "read_file",
            "input": { "path": "src/main.rs" }
        });
        let block: ContentBlock = serde_json::from_value(json).unwrap();
        match block {
            ContentBlock::ToolUse { id, name, input } => {
                assert_eq!(id, "toolu_01ABC");
                assert_eq!(name, "read_file");
                assert_eq!(input["path"], "src/main.rs");
            }
            _ => panic!("Expected ToolUse variant"),
        }
    }
}
```

::: python Coming from Python
In Python, content blocks are typically plain dictionaries:
```python
content_block = {"type": "tool_use", "id": "toolu_01ABC", "name": "read_file", "input": {"path": "src/main.rs"}}
# Access with string keys -- no compile-time checking
tool_name = content_block["name"]  # KeyError at runtime if wrong
```
With Rust's `ContentBlock` enum, you pattern match on the variant:
```rust
match block {
    ContentBlock::ToolUse { name, .. } => println!("Tool: {name}"),
    ContentBlock::Text { text } => println!("Text: {text}"),
    ContentBlock::ToolResult { .. } => println!("Result"),
}
```
The compiler ensures you handle every variant. If you add a new content block type later, every `match` that does not handle it becomes a compile error.
:::

## Modeling Messages

With content blocks defined, a message is simply a role plus a vector of content blocks:

```rust
/// The role of a message sender.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    User,
    Assistant,
}

/// A single message in the conversation history.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub role: Role,
    pub content: Vec<ContentBlock>,
}
```

Notice that `Role` only has `User` and `Assistant` -- the system prompt is sent separately in the Anthropic API, not as a message with a "system" role. This is a design choice by Anthropic that simplifies the message types.

## Convenience Constructors

You will be creating messages constantly throughout the agentic loop. Adding convenience methods makes the calling code much cleaner:

```rust
impl Message {
    /// Create a user message containing a single text block.
    pub fn user(text: impl Into<String>) -> Self {
        Message {
            role: Role::User,
            content: vec![ContentBlock::Text {
                text: text.into(),
            }],
        }
    }

    /// Create an assistant message from a list of content blocks.
    /// Used when appending the API response to conversation history.
    pub fn assistant(content: Vec<ContentBlock>) -> Self {
        Message {
            role: Role::Assistant,
            content,
        }
    }

    /// Create a user message containing tool result blocks.
    /// After executing tools, we send the results back as a "user" message.
    pub fn tool_results(results: Vec<ContentBlock>) -> Self {
        Message {
            role: Role::User,
            content: results,
        }
    }
}
```

An important detail: tool results are sent as a **user** message, not a special "tool" role. The Anthropic API treats tool results as user messages whose content blocks happen to be `tool_result` types. This trips people up, so let's be explicit about it.

## The API Response Type

You also need a type to represent the full API response that comes back from the Anthropic Messages endpoint:

```rust
/// The response from the Anthropic Messages API.
#[derive(Debug, Clone, Deserialize)]
pub struct ApiResponse {
    /// Unique identifier for this response.
    pub id: String,

    /// The content blocks in the response.
    pub content: Vec<ContentBlock>,

    /// The model that generated the response.
    pub model: String,

    /// Why the model stopped generating.
    /// "end_turn" means it finished normally.
    /// "tool_use" means it wants to call a tool.
    /// "max_tokens" means it hit the token limit.
    pub stop_reason: Option<String>,

    /// Token usage information.
    pub usage: Usage,
}

/// Token usage statistics from the API.
#[derive(Debug, Clone, Deserialize)]
pub struct Usage {
    pub input_tokens: u32,
    pub output_tokens: u32,
}
```

The `stop_reason` field is the linchpin of the agentic loop. Its value tells the loop what to do next: if it is `"end_turn"`, the model is done and you break out of the loop. If it is `"tool_use"`, the model wants to call tools and you continue the loop. If it is `"max_tokens"`, the model ran out of output space and you need to decide whether to continue or stop.

## Helper Functions for Content Extraction

You will frequently need to extract specific content block types from a message. These helpers make that ergonomic:

```rust
impl Message {
    /// Extract all text content from this message, concatenated.
    pub fn text(&self) -> String {
        self.content
            .iter()
            .filter_map(|block| match block {
                ContentBlock::Text { text } => Some(text.as_str()),
                _ => None,
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// Extract all tool-use blocks from this message.
    pub fn tool_calls(&self) -> Vec<&ContentBlock> {
        self.content
            .iter()
            .filter(|block| matches!(block, ContentBlock::ToolUse { .. }))
            .collect()
    }

    /// Returns true if this message contains any tool-use requests.
    pub fn has_tool_calls(&self) -> bool {
        self.content
            .iter()
            .any(|block| matches!(block, ContentBlock::ToolUse { .. }))
    }
}
```

The `matches!` macro is a concise way to check an enum variant without destructuring it. It returns `true` if the pattern matches and `false` otherwise.

## Putting It All Together

Here is how these types work together in a typical loop iteration. The model returns a response with a tool-use block:

```rust
// The API returns this response
let response = ApiResponse {
    id: "msg_01XYZ".to_string(),
    content: vec![
        ContentBlock::Text {
            text: "I'll read the file for you.".to_string(),
        },
        ContentBlock::ToolUse {
            id: "toolu_01ABC".to_string(),
            name: "read_file".to_string(),
            input: serde_json::json!({ "path": "src/main.rs" }),
        },
    ],
    model: "claude-sonnet-4-20250514".to_string(),
    stop_reason: Some("tool_use".to_string()),
    usage: Usage { input_tokens: 100, output_tokens: 50 },
};

// Append the assistant's response to conversation history
let assistant_msg = Message::assistant(response.content.clone());
messages.push(assistant_msg);

// Execute each tool call and collect results
let tool_result = ContentBlock::ToolResult {
    tool_use_id: "toolu_01ABC".to_string(),
    content: "fn main() {\n    println!(\"Hello!\");\n}".to_string(),
    is_error: None,
};

// Feed results back as a user message
messages.push(Message::tool_results(vec![tool_result]));
```

After this, the message history contains the user's original question, the assistant's response with the tool call, and the tool result. The next API call sends all of this, and the model can see what the file contains.

## Key Takeaways

- Content blocks are modeled as a Rust enum (`ContentBlock`) with variants for text, tool use, and tool results -- serde's `#[serde(tag = "type")]` handles JSON serialization automatically
- A `Message` is a role (`User` or `Assistant`) plus a `Vec<ContentBlock>`, because a single message can contain multiple blocks of different types
- Tool results are sent as **user** messages whose content blocks have type `tool_result`, not as a separate message role
- The `stop_reason` field on the API response (`"end_turn"`, `"tool_use"`, `"max_tokens"`) drives the loop's control flow
- Convenience constructors and helper methods (`Message::user()`, `Message::tool_results()`, `.text()`, `.tool_calls()`) keep the loop code clean and readable
