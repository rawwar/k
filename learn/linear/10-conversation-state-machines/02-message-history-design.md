---
title: Message History Design
description: Data structure design for conversation message history supporting efficient append, truncation, token counting, search, and serialization.
---

# Message History Design

> **What you'll learn:**
> - How to design a message history data structure with O(1) append, efficient prefix truncation, and cached token counts
> - The role and content types in a message: system, user, assistant, tool_call, and tool_result with their structural requirements
> - Indexing strategies that enable fast lookup by message ID, role, or tool call ID within long conversation histories

The previous subchapter gave you a state machine that governs *when* messages can be added or modified. Now you need the data structure that holds those messages. This is more than a `Vec<Message>` -- you need efficient append, prefix truncation (for sliding window compaction), cached token counts, and indexing by role or tool call ID. A poor choice here shows up as latency in every LLM call, because you rebuild the message array on every request.

## The Message Struct

Let's start with what a single message looks like. LLM APIs define messages with a role and content, but real agent messages carry more metadata:

```rust
use std::time::SystemTime;
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq)]
enum Role {
    System,
    User,
    Assistant,
    ToolCall,
    ToolResult,
}

#[derive(Debug, Clone)]
enum ContentBlock {
    Text(String),
    ToolUse {
        id: String,
        name: String,
        input: serde_json::Value,
    },
    ToolResult {
        tool_use_id: String,
        content: String,
        is_error: bool,
    },
}

#[derive(Debug, Clone)]
struct Message {
    id: Uuid,
    role: Role,
    content: Vec<ContentBlock>,
    timestamp: SystemTime,
    token_count: Option<u32>,
    metadata: MessageMetadata,
}

#[derive(Debug, Clone, Default)]
struct MessageMetadata {
    /// The model that generated this message (for assistant messages)
    model: Option<String>,
    /// How long the LLM took to generate this response
    latency_ms: Option<u64>,
    /// Whether this message was created by compaction/summarization
    is_synthetic: bool,
    /// If synthetic, which original message IDs it replaced
    replaces: Vec<Uuid>,
}
```

Every message gets a UUID on creation. This is critical for several operations you'll build later: branching conversations need stable references to fork points, compaction needs to track which messages were replaced, and persistence needs to detect duplicates on resume.

The `content` field is a `Vec<ContentBlock>` rather than a plain `String`. This matches the Anthropic API's content block model, where a single assistant message can contain both text and tool use requests. If you flatten everything to strings, you lose the ability to selectively process or compact individual blocks.

::: python Coming from Python
In Python, you might represent messages as dictionaries: `{"role": "assistant", "content": "..."}`. This is flexible but gives you zero compile-time guarantees. You can accidentally set `role` to `"assitant"` (typo) or forget the `tool_use_id` field on a tool result. Rust's type system catches all of these at compile time. The tradeoff is more upfront code, but you eliminate an entire class of runtime bugs.
:::

## The MessageHistory Data Structure

A naive `Vec<Message>` works for small conversations but has problems at scale. Prefix truncation (removing old messages for sliding window compaction) is O(n) because `Vec::drain(0..k)` shifts all remaining elements. Let's build a structure optimized for the operations agents actually perform:

```rust
use std::collections::HashMap;

struct MessageHistory {
    /// The backing store. We use VecDeque for efficient front removal.
    messages: std::collections::VecDeque<Message>,
    /// Running total of tokens across all messages.
    total_tokens: u32,
    /// Index: message ID -> position in the deque.
    id_index: HashMap<Uuid, usize>,
    /// Index: tool_use_id -> message ID of the ToolCall message.
    tool_call_index: HashMap<String, Uuid>,
}

impl MessageHistory {
    fn new() -> Self {
        Self {
            messages: std::collections::VecDeque::new(),
            total_tokens: 0,
            id_index: HashMap::new(),
            tool_call_index: HashMap::new(),
        }
    }

    /// Append a message. O(1) amortized.
    fn push(&mut self, message: Message) {
        let id = message.id;
        let tokens = message.token_count.unwrap_or(0);

        // Update tool call index
        for block in &message.content {
            if let ContentBlock::ToolUse { id: tool_id, .. } = block {
                self.tool_call_index.insert(tool_id.clone(), id);
            }
        }

        let position = self.messages.len();
        self.id_index.insert(id, position);
        self.total_tokens += tokens;
        self.messages.push_back(message);
    }

    /// Remove the oldest n messages. O(n) for the removal, O(m) to rebuild indexes.
    fn truncate_front(&mut self, n: usize) {
        let removed: Vec<Message> = self.messages.drain(..n).collect();

        // Subtract tokens from removed messages
        for msg in &removed {
            self.total_tokens -= msg.token_count.unwrap_or(0);
        }

        // Rebuild indexes (positions shifted)
        self.rebuild_indexes();
    }

    /// Get total token count without recalculating.
    fn total_tokens(&self) -> u32 {
        self.total_tokens
    }

    /// Find a message by ID. O(1) via index.
    fn get_by_id(&self, id: &Uuid) -> Option<&Message> {
        self.id_index.get(id)
            .and_then(|&pos| self.messages.get(pos))
    }

    /// Find the tool call message for a given tool_use_id. O(1) via index.
    fn get_tool_call(&self, tool_use_id: &str) -> Option<&Message> {
        self.tool_call_index.get(tool_use_id)
            .and_then(|msg_id| self.get_by_id(msg_id))
    }

    /// Get all messages as a slice-like iterator for API serialization.
    fn iter(&self) -> impl Iterator<Item = &Message> {
        self.messages.iter()
    }

    /// Get the last n messages (useful for "recent context" extraction).
    fn last_n(&self, n: usize) -> impl Iterator<Item = &Message> {
        let start = self.messages.len().saturating_sub(n);
        self.messages.iter().skip(start)
    }

    fn rebuild_indexes(&mut self) {
        self.id_index.clear();
        self.tool_call_index.clear();
        for (pos, msg) in self.messages.iter().enumerate() {
            self.id_index.insert(msg.id, pos);
            for block in &msg.content {
                if let ContentBlock::ToolUse { id: tool_id, .. } = block {
                    self.tool_call_index.insert(tool_id.clone(), msg.id);
                }
            }
        }
    }

    fn len(&self) -> usize {
        self.messages.len()
    }
}
```

The key design decisions here:

**`VecDeque` instead of `Vec`**: Front removal is O(1) amortized with `VecDeque`, compared to O(n) with `Vec`. Since sliding window compaction removes messages from the front, this matters for long conversations.

**Cached `total_tokens`**: Every time you add or remove a message, you update the running total. This avoids iterating all messages to check if you're near the context limit -- a check that happens on every LLM call.

**HashMap indexes**: Looking up a message by UUID or a tool call by its ID are both O(1) operations. Without these, you'd do linear scans through potentially thousands of messages.

## Serializing Messages for API Calls

Your internal `Message` struct carries metadata that the LLM API doesn't need. You need a serialization layer that converts your rich messages into the format the API expects:

```rust
use serde::Serialize;

#[derive(Serialize)]
struct ApiMessage {
    role: String,
    content: Vec<ApiContentBlock>,
}

#[derive(Serialize)]
#[serde(untagged)]
enum ApiContentBlock {
    Text {
        #[serde(rename = "type")]
        block_type: String,
        text: String,
    },
    ToolUse {
        #[serde(rename = "type")]
        block_type: String,
        id: String,
        name: String,
        input: serde_json::Value,
    },
    ToolResult {
        #[serde(rename = "type")]
        block_type: String,
        tool_use_id: String,
        content: String,
        is_error: bool,
    },
}

impl Message {
    fn to_api_message(&self) -> ApiMessage {
        let role = match self.role {
            Role::System => "system",
            Role::User => "user",
            Role::Assistant => "assistant",
            Role::ToolCall => "assistant",  // Tool calls come from assistant
            Role::ToolResult => "user",     // Tool results are sent as user role
        };

        let content = self.content.iter().map(|block| match block {
            ContentBlock::Text(text) => ApiContentBlock::Text {
                block_type: "text".into(),
                text: text.clone(),
            },
            ContentBlock::ToolUse { id, name, input } => ApiContentBlock::ToolUse {
                block_type: "tool_use".into(),
                id: id.clone(),
                name: name.clone(),
                input: input.clone(),
            },
            ContentBlock::ToolResult { tool_use_id, content, is_error } => {
                ApiContentBlock::ToolResult {
                    block_type: "tool_result".into(),
                    tool_use_id: tool_use_id.clone(),
                    content: content.clone(),
                    is_error: *is_error,
                }
            }
        }).collect();

        ApiMessage {
            role: role.to_string(),
            content,
        }
    }
}

impl MessageHistory {
    /// Convert the full history to API-ready format.
    fn to_api_messages(&self) -> Vec<ApiMessage> {
        self.messages.iter()
            .map(|msg| msg.to_api_message())
            .collect()
    }
}
```

Notice the role mapping: `ToolCall` becomes `"assistant"` and `ToolResult` becomes `"user"` in the API. Internally you track five distinct roles for clarity, but the API only understands three roles plus content block types. This translation layer keeps your internal model clean while conforming to API requirements.

::: wild In the Wild
Claude Code structures its messages with content blocks that match the Anthropic API format directly. Each assistant message can contain interleaved text and `tool_use` blocks, and the tool results are sent back as `tool_result` content blocks in the next user turn. OpenCode, targeting the OpenAI API, uses a different structure where tool calls and results are separate messages with explicit `tool_call_id` fields. If you plan to support multiple providers (which Chapter 14 covers), designing your internal message format as a superset of all providers' formats saves painful translation work later.
:::

## Token Count Caching Strategy

Every message should cache its token count after the first calculation. This is essential because token counting involves running the tokenizer -- not expensive for one message, but prohibitive when repeated across thousands of messages on every API call:

```rust
impl Message {
    fn ensure_token_count(&mut self, tokenizer: &dyn Tokenizer) {
        if self.token_count.is_none() {
            let count = self.content.iter().map(|block| {
                match block {
                    ContentBlock::Text(text) => tokenizer.count(text),
                    ContentBlock::ToolUse { name, input, .. } => {
                        tokenizer.count(name)
                            + tokenizer.count(&input.to_string())
                            + 20 // Overhead for JSON structure
                    }
                    ContentBlock::ToolResult { content, .. } => {
                        tokenizer.count(content) + 10
                    }
                }
            }).sum::<u32>()
                + 4; // Per-message overhead (role, delimiters)

            self.token_count = Some(count);
        }
    }
}

trait Tokenizer {
    fn count(&self, text: &str) -> u32;
}
```

The `+ 4` and `+ 20` overhead values account for the structural tokens that the API adds around each message and tool use block. These aren't part of your text but still consume context window space. We'll cover exact overhead accounting in the next subchapter on token counting strategies.

## Key Takeaways

- Use `VecDeque<Message>` for O(1) amortized append and front removal, critical for sliding window compaction in long conversations.
- Every message gets a UUID for stable referencing across branches, compaction, and persistence -- never rely on positional indexes alone.
- Maintain cached `total_tokens` and HashMap indexes (by ID, by tool call ID) so that context window checks and message lookups are O(1) instead of O(n).
- Separate your internal `Message` representation (with metadata, UUIDs, cached tokens) from the API serialization format -- translation happens at the boundary, not in the core data structure.
- Cache token counts per message on first calculation and update the running total incrementally to avoid re-tokenizing the entire history on every API call.
