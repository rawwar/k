---
title: Message History Design
description: Design an efficient message history data structure that supports fast appending, token tracking, and selective retrieval.
---

# Message History Design

> **What you'll learn:**
> - How to represent conversation messages with role, content, token count, and metadata fields
> - How to design an append-optimized data structure with O(1) token total tracking
> - How to support message tagging and priority levels for intelligent compaction decisions

The conversation history is the central data structure of your context management system. Every other feature -- persistence, compaction, summarization, forking -- operates on this structure. Getting it right matters. A naive `Vec<String>` will not cut it when you need to track token counts, tag messages for priority-based pruning, and support efficient serialization.

## Designing the Message Type

Each message in the conversation carries more than just role and content. You need metadata for context management decisions:

```rust
use serde::{Deserialize, Serialize};
use std::time::SystemTime;

/// Unique identifier for a message within a conversation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct MessageId(pub u64);

/// The role of a message in the conversation.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Role {
    System,
    User,
    Assistant,
    /// Tool results with the tool name and call ID
    Tool { name: String, call_id: String },
}

/// Priority level for compaction decisions.
/// Higher priority messages are kept longer during compaction.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum Priority {
    /// Can be dropped without summarization (e.g., verbose tool output)
    Low = 0,
    /// Normal conversation messages
    Normal = 1,
    /// Important context (e.g., user instructions, key decisions)
    High = 2,
    /// Must never be removed (e.g., system prompt, pinned messages)
    Pinned = 3,
}

/// A single message in the conversation history.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    /// Unique identifier within this conversation
    pub id: MessageId,
    /// Who sent this message
    pub role: Role,
    /// The message content
    pub content: String,
    /// Pre-computed token count for this message (including overhead)
    pub token_count: usize,
    /// When this message was added
    pub timestamp: SystemTime,
    /// Priority for compaction decisions
    pub priority: Priority,
    /// Whether this message has been summarized/compacted
    pub is_summary: bool,
    /// If this is a summary, which message IDs did it replace?
    pub summarizes: Vec<MessageId>,
}

impl Message {
    /// Create a new message with a pre-computed token count.
    pub fn new(
        id: MessageId,
        role: Role,
        content: String,
        token_count: usize,
    ) -> Self {
        let priority = match &role {
            Role::System => Priority::Pinned,
            Role::User => Priority::High,
            Role::Assistant => Priority::Normal,
            Role::Tool { .. } => Priority::Low,
        };

        Self {
            id,
            role,
            content,
            token_count,
            timestamp: SystemTime::now(),
            priority,
            is_summary: false,
            summarizes: Vec::new(),
        }
    }

    /// Create a summary message that replaces a range of older messages.
    pub fn summary(
        id: MessageId,
        content: String,
        token_count: usize,
        replaced_ids: Vec<MessageId>,
    ) -> Self {
        Self {
            id,
            role: Role::Assistant,
            content,
            token_count,
            timestamp: SystemTime::now(),
            priority: Priority::Normal,
            is_summary: true,
            summarizes: replaced_ids,
        }
    }
}

fn main() {
    let msg = Message::new(
        MessageId(1),
        Role::User,
        "Read the file src/main.rs and explain what it does".to_string(),
        15,
    );
    println!("Message {:?}: role={:?}, tokens={}, priority={:?}",
        msg.id, msg.role, msg.token_count, msg.priority);
}
```

A few design decisions worth noting. The `Priority` enum derives `Ord`, which means you can compare priorities directly with `<` and `>` -- this simplifies compaction logic. Token counts are stored at creation time, not recomputed on access. The `summarizes` field creates a paper trail so you can always trace what a summary replaced.

::: python Coming from Python
In Python, you would likely use a dataclass or dictionary:
```python
@dataclass
class Message:
    role: str
    content: str
    token_count: int
    priority: str = "normal"
```
Rust's enum-based `Role` and `Priority` types are stricter than Python strings --
you cannot accidentally set priority to `"hgih"` (a typo). The compiler catches
it. The `#[derive(Serialize, Deserialize)]` attributes give you free JSON
serialization, similar to how `dataclasses_json` works but with zero runtime
overhead.
:::

## The Conversation History Structure

Now let's build the history container that holds messages and maintains aggregate state:

```rust
use std::collections::HashMap;

/// A conversation history optimized for context management.
///
/// Maintains O(1) token counting by tracking a running total,
/// and supports efficient lookup by message ID.
#[derive(Debug, Serialize, Deserialize)]
pub struct ConversationHistory {
    /// All messages in chronological order
    messages: Vec<Message>,
    /// Quick lookup from message ID to index in the messages vec
    #[serde(skip)]
    index: HashMap<MessageId, usize>,
    /// Running total of tokens across all messages
    total_tokens: usize,
    /// Next message ID to assign
    next_id: u64,
}

impl ConversationHistory {
    pub fn new() -> Self {
        Self {
            messages: Vec::new(),
            index: HashMap::new(),
            total_tokens: 0,
            next_id: 1,
        }
    }

    /// Append a message to the history. Returns the assigned MessageId.
    pub fn push(&mut self, role: Role, content: String, token_count: usize) -> MessageId {
        let id = MessageId(self.next_id);
        self.next_id += 1;

        let msg = Message::new(id, role, content, token_count);
        self.total_tokens += msg.token_count;
        let idx = self.messages.len();
        self.index.insert(id, idx);
        self.messages.push(msg);

        id
    }

    /// Total tokens across all messages (O(1) lookup).
    pub fn total_tokens(&self) -> usize {
        self.total_tokens
    }

    /// Number of messages in the history.
    pub fn len(&self) -> usize {
        self.messages.len()
    }

    /// Is the history empty?
    pub fn is_empty(&self) -> bool {
        self.messages.is_empty()
    }

    /// Get a message by its ID.
    pub fn get(&self, id: MessageId) -> Option<&Message> {
        self.index.get(&id).map(|&idx| &self.messages[idx])
    }

    /// Get all messages as a slice (for serialization to the API).
    pub fn messages(&self) -> &[Message] {
        &self.messages
    }

    /// Get the last N messages.
    pub fn recent(&self, n: usize) -> &[Message] {
        let start = self.messages.len().saturating_sub(n);
        &self.messages[start..]
    }

    /// Remove messages by their IDs and return the freed token count.
    /// This is used during compaction.
    pub fn remove_messages(&mut self, ids: &[MessageId]) -> usize {
        let id_set: std::collections::HashSet<_> = ids.iter().collect();
        let mut freed = 0;

        self.messages.retain(|msg| {
            if id_set.contains(&msg.id) {
                freed += msg.token_count;
                false
            } else {
                true
            }
        });

        // Rebuild the index after removal
        self.index.clear();
        for (idx, msg) in self.messages.iter().enumerate() {
            self.index.insert(msg.id, idx);
        }

        self.total_tokens -= freed;
        freed
    }

    /// Replace a range of messages with a summary message.
    /// Returns the net token change (negative means tokens were freed).
    pub fn replace_with_summary(
        &mut self,
        ids_to_replace: &[MessageId],
        summary_content: String,
        summary_token_count: usize,
    ) -> i64 {
        let freed = self.remove_messages(ids_to_replace) as i64;

        let id = MessageId(self.next_id);
        self.next_id += 1;

        let summary = Message::summary(
            id,
            summary_content,
            summary_token_count,
            ids_to_replace.to_vec(),
        );

        // Insert summary at the position of the first removed message
        // For simplicity, we append it (compacted summaries go at the start
        // of the remaining history)
        self.total_tokens += summary.token_count;
        let idx = self.messages.len();
        self.index.insert(id, idx);
        self.messages.push(summary);

        summary_token_count as i64 - freed
    }

    /// Get messages below a given priority level, sorted by age (oldest first).
    /// Useful for finding compaction candidates.
    pub fn compaction_candidates(&self, below_priority: Priority) -> Vec<MessageId> {
        self.messages
            .iter()
            .filter(|msg| msg.priority < below_priority && !msg.is_summary)
            .map(|msg| msg.id)
            .collect()
    }

    /// Rebuild the index after deserialization.
    pub fn rebuild_index(&mut self) {
        self.index.clear();
        for (idx, msg) in self.messages.iter().enumerate() {
            self.index.insert(msg.id, idx);
        }
    }
}

fn main() {
    let mut history = ConversationHistory::new();

    // Build up a conversation
    let _sys = history.push(
        Role::System,
        "You are a helpful coding assistant.".to_string(),
        10,
    );
    let _u1 = history.push(
        Role::User,
        "Read src/main.rs".to_string(),
        6,
    );
    let _a1 = history.push(
        Role::Assistant,
        "I'll read that file for you.".to_string(),
        8,
    );
    let t1 = history.push(
        Role::Tool {
            name: "read_file".to_string(),
            call_id: "call_1".to_string(),
        },
        "fn main() {\n    println!(\"hello\");\n}".to_string(),
        12,
    );
    let _a2 = history.push(
        Role::Assistant,
        "This is a simple Rust program that prints hello.".to_string(),
        11,
    );

    println!("Messages: {}", history.len());
    println!("Total tokens: {}", history.total_tokens());
    println!("Recent 2 messages:");
    for msg in history.recent(2) {
        println!("  {:?}: {} ({} tokens)", msg.role, &msg.content[..30.min(msg.content.len())], msg.token_count);
    }

    // Find compaction candidates (Low priority, not summaries)
    let candidates = history.compaction_candidates(Priority::Normal);
    println!("\nCompaction candidates (below Normal priority): {:?}", candidates);

    // Replace tool result with a summary
    let net = history.replace_with_summary(
        &[t1],
        "[Summary: main.rs contains a hello world program]".to_string(),
        8,
    );
    println!("After summarization: {} tokens (net change: {})", history.total_tokens(), net);
}
```

## Why This Design Works

Several aspects of this design are worth calling out:

**O(1) token tracking**: The `total_tokens` field is maintained incrementally. Adding a message adds its tokens; removing messages subtracts them. You never need to iterate the entire history just to check the total.

**Indexed lookup**: The `index` HashMap lets you find any message by ID in O(1), which is essential when compaction needs to remove specific messages. After bulk removal, we rebuild the index -- this is O(n) but only happens during compaction, not on every append.

**Priority-based filtering**: The `compaction_candidates` method finds messages that can be compacted, respecting the priority system. Pinned messages (like the system prompt) are never candidates. Tool results start as Low priority since they are often large and become less relevant over time.

**Summary tracking**: When messages are replaced by a summary, the `summarizes` field records what was lost. This creates an audit trail and enables features like "expand summary" in a UI.

::: wild In the Wild
Claude Code maintains a rich message structure that includes metadata beyond just role and content. Each message tracks its token count, whether it has been compacted, and references to related messages (like which tool call a tool result belongs to). This metadata drives intelligent compaction -- when the context fills up, Claude Code can identify the least valuable messages to compact first, preserving the most important context.
:::

## Iterating Over History for API Calls

When building the actual API request, you need to convert your rich `Message` structs into the format the API expects:

```rust
use serde_json::json;

/// Convert the conversation history into the format expected by the Claude API.
pub fn to_api_messages(history: &ConversationHistory) -> Vec<serde_json::Value> {
    history
        .messages()
        .iter()
        .filter(|msg| msg.role != Role::System) // System goes in a separate field
        .map(|msg| {
            let role_str = match &msg.role {
                Role::User => "user",
                Role::Assistant => "assistant",
                Role::Tool { .. } => "user", // Tool results are sent as user messages
                Role::System => unreachable!(), // Filtered above
            };

            match &msg.role {
                Role::Tool { call_id, .. } => {
                    json!({
                        "role": role_str,
                        "content": [{
                            "type": "tool_result",
                            "tool_use_id": call_id,
                            "content": msg.content,
                        }]
                    })
                }
                _ => {
                    json!({
                        "role": role_str,
                        "content": msg.content,
                    })
                }
            }
        })
        .collect()
}

fn main() {
    let mut history = ConversationHistory::new();
    history.push(Role::User, "Hello!".to_string(), 3);
    history.push(Role::Assistant, "Hi there!".to_string(), 4);

    let api_msgs = to_api_messages(&history);
    for msg in &api_msgs {
        println!("{}", serde_json::to_string_pretty(msg).unwrap());
    }
}
```

## Key Takeaways

- Store token counts on each message at creation time for O(1) total token tracking -- never recount the entire history
- Use an enum-based priority system (`Low`, `Normal`, `High`, `Pinned`) to guide compaction decisions, with `Ord` derivation for easy comparison
- Maintain a `HashMap` index for O(1) message lookup by ID, rebuilding it only during bulk operations like compaction
- Track which messages a summary replaces via the `summarizes` field to maintain an audit trail
- Separate the internal message representation from the API wire format -- your history needs richer metadata than the API accepts
