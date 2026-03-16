---
title: Memory Management
description: Optimize Rust memory usage for long-running agent sessions with large conversation histories and frequent allocations.
---

# Memory Management

> **What you'll learn:**
> - How to profile and reduce memory allocations in the conversation history hot path
> - How to use arena allocation or string interning for repeated content patterns
> - How to implement lazy loading of session data to keep memory footprint low at startup

A coding agent can run for hours, accumulating hundreds of messages, thousands of tool results, and tens of thousands of string allocations. In Python, you would rely on the garbage collector to clean this up. In Rust, you have direct control over memory layout and lifecycle -- which means you can build an agent that stays responsive even after a marathon session.

## Understanding the Allocation Profile

Before optimizing, you need to know where memory goes. In a conversation-heavy agent, the biggest consumers are:

1. **Message content strings** -- file contents read by tools, assistant responses, user inputs
2. **Serialization buffers** -- temporary allocations during JSON/MessagePack encoding
3. **The message index** -- `HashMap` overhead for message lookup
4. **Compaction intermediaries** -- temporary vectors during compaction and summarization

Let's build a memory profiler that tracks allocations in the conversation path:

```rust
use std::collections::HashMap;

/// Simple memory accounting for conversation components.
#[derive(Debug, Default)]
pub struct MemoryProfile {
    /// Bytes used by message content strings
    pub content_bytes: usize,
    /// Bytes used by metadata (role strings, IDs, etc.)
    pub metadata_bytes: usize,
    /// Number of separate heap allocations
    pub allocation_count: usize,
    /// Number of messages tracked
    pub message_count: usize,
}

impl MemoryProfile {
    /// Profile a message list to understand memory usage.
    pub fn profile(messages: &[(String, String, usize)]) -> Self {
        let mut profile = Self::default();

        for (role, content, _tokens) in messages {
            // Each String has 24 bytes of stack overhead (ptr, len, capacity)
            // plus the heap allocation for the data
            profile.content_bytes += content.len();
            profile.metadata_bytes += role.len();
            profile.allocation_count += 2; // Two String heap allocations per message
            profile.message_count += 1;
        }

        // HashMap overhead: ~72 bytes per entry for a typical HashMap
        profile.metadata_bytes += messages.len() * 72;
        profile.allocation_count += 1; // The HashMap's backing allocation

        profile
    }

    /// Total memory used.
    pub fn total_bytes(&self) -> usize {
        self.content_bytes + self.metadata_bytes
    }

    /// Print a human-readable summary.
    pub fn print_summary(&self) {
        println!("Memory Profile:");
        println!("  Messages:     {}", self.message_count);
        println!("  Content:      {:.1} KB", self.content_bytes as f64 / 1024.0);
        println!("  Metadata:     {:.1} KB", self.metadata_bytes as f64 / 1024.0);
        println!("  Total:        {:.1} KB", self.total_bytes() as f64 / 1024.0);
        println!("  Allocations:  {}", self.allocation_count);
        println!("  Avg per msg:  {:.0} bytes", self.total_bytes() as f64 / self.message_count.max(1) as f64);
    }
}

fn main() {
    // Simulate a conversation with varying message sizes
    let messages: Vec<(String, String, usize)> = (0..500).map(|i| {
        let role = if i % 3 == 0 { "user" } else if i % 3 == 1 { "assistant" } else { "tool" };
        let content = if role == "tool" {
            // Tool results are large
            "x".repeat(5000)
        } else {
            "x".repeat(200)
        };
        (role.to_string(), content, 0)
    }).collect();

    let profile = MemoryProfile::profile(&messages);
    profile.print_summary();
}
```

::: python Coming from Python
In Python, every string object carries significant overhead:
```python
import sys
s = "hello"
sys.getsizeof(s)  # 54 bytes for a 5-char string!
```
Python strings have a 49-byte header (type pointer, reference count, hash,
length, etc.). A Rust `String` has only 24 bytes of stack overhead (pointer,
length, capacity). For a conversation with 500 messages, the per-string overhead
difference is substantial: Python uses ~49KB just for string headers, while
Rust uses ~12KB. This is before counting the actual content.
:::

## String Interning for Repeated Content

Many strings in a conversation are repeated: role names ("user", "assistant", "tool"), tool names, file paths. String interning stores each unique string once and uses cheap references everywhere else:

```rust
use std::collections::HashSet;
use std::sync::Arc;

/// A string interner that deduplicates strings across the conversation.
/// Uses Arc<str> so interned strings can be cheaply cloned.
pub struct StringInterner {
    pool: HashSet<Arc<str>>,
}

impl StringInterner {
    pub fn new() -> Self {
        Self {
            pool: HashSet::new(),
        }
    }

    /// Intern a string, returning a reference-counted handle.
    /// If the string was already interned, returns the existing handle.
    pub fn intern(&mut self, s: &str) -> Arc<str> {
        if let Some(existing) = self.pool.get(s) {
            existing.clone()
        } else {
            let interned: Arc<str> = Arc::from(s);
            self.pool.insert(interned.clone());
            interned
        }
    }

    /// How many unique strings are interned?
    pub fn unique_count(&self) -> usize {
        self.pool.len()
    }

    /// Total bytes stored in the interner.
    pub fn total_bytes(&self) -> usize {
        self.pool.iter().map(|s| s.len()).sum()
    }
}

/// A message that uses interned strings for common fields.
pub struct InternedMessage {
    pub role: Arc<str>,
    pub content: String, // Content is unique, so not interned
    pub tool_name: Option<Arc<str>>,
    pub token_count: usize,
}

fn main() {
    let mut interner = StringInterner::new();

    // Intern role names -- these get reused hundreds of times
    let user_role = interner.intern("user");
    let assistant_role = interner.intern("assistant");
    let tool_role = interner.intern("tool");

    // Simulate 300 messages
    let mut messages = Vec::new();
    for i in 0..300 {
        let role = match i % 3 {
            0 => user_role.clone(),     // Cheap Arc clone, not a string copy
            1 => assistant_role.clone(),
            _ => tool_role.clone(),
        };
        let tool_name = if i % 3 == 2 {
            Some(interner.intern("read_file")) // "read_file" interned once
        } else {
            None
        };
        messages.push(InternedMessage {
            role,
            content: format!("Message content {}", i),
            tool_name,
            token_count: 10,
        });
    }

    println!("Messages: {}", messages.len());
    println!("Unique interned strings: {}", interner.unique_count());
    println!("Interner total bytes: {}", interner.total_bytes());

    // Without interning: 300 * "assistant".len() = 2700 bytes just for role strings
    // With interning: 3 * ~10 bytes = 30 bytes + 300 * 8 bytes (Arc pointers)
    let without = 300 * "assistant".len();
    let with = interner.total_bytes() + 300 * std::mem::size_of::<Arc<str>>();
    println!("Role strings without interning: {} bytes", without);
    println!("Role strings with interning:    {} bytes", with);
}
```

The savings from interning role names alone are modest, but the same technique applies to file paths (which can be long and appear in many messages) and tool names (which repeat in every tool call and result).

## Lazy Loading for Session Data

When the agent starts, it should not load all session data into memory. A user might have dozens of saved sessions, each with thousands of messages. Lazy loading defers the expensive work until a session is actually accessed:

```rust
use std::path::PathBuf;

/// A session handle that only loads the full session data on demand.
pub enum LazySession {
    /// Only the metadata is loaded (from a quick header read)
    Unloaded {
        id: String,
        name: String,
        message_count: usize,
        file_path: PathBuf,
    },
    /// Full session data is in memory
    Loaded {
        id: String,
        name: String,
        messages: Vec<(String, String)>,
        file_path: PathBuf,
    },
}

impl LazySession {
    /// Get the session name without loading full data.
    pub fn name(&self) -> &str {
        match self {
            LazySession::Unloaded { name, .. } => name,
            LazySession::Loaded { name, .. } => name,
        }
    }

    /// Get the session ID.
    pub fn id(&self) -> &str {
        match self {
            LazySession::Unloaded { id, .. } => id,
            LazySession::Loaded { id, .. } => id,
        }
    }

    /// Ensure the session data is fully loaded.
    /// In production, this would read from disk.
    pub fn ensure_loaded(&mut self) -> Result<(), String> {
        if let LazySession::Unloaded { id, name, file_path, .. } = self {
            // Simulate loading from disk
            println!("Loading session '{}' from {:?}...", name, file_path);

            // In production: read and deserialize the file
            let messages = vec![
                ("user".to_string(), "Hello".to_string()),
                ("assistant".to_string(), "Hi there!".to_string()),
            ];

            *self = LazySession::Loaded {
                id: id.clone(),
                name: name.clone(),
                messages,
                file_path: file_path.clone(),
            };
        }
        Ok(())
    }

    /// Get messages (loads from disk if needed).
    pub fn messages(&mut self) -> Result<&[(String, String)], String> {
        self.ensure_loaded()?;
        match self {
            LazySession::Loaded { messages, .. } => Ok(messages),
            _ => unreachable!("ensure_loaded guarantees Loaded state"),
        }
    }

    /// Check if this session is fully loaded.
    pub fn is_loaded(&self) -> bool {
        matches!(self, LazySession::Loaded { .. })
    }
}

/// A session list that uses lazy loading.
pub struct LazySessionList {
    sessions: Vec<LazySession>,
}

impl LazySessionList {
    /// Load just the session headers from a directory.
    /// This is fast because it only reads metadata, not full histories.
    pub fn scan_directory(dir: &PathBuf) -> Self {
        // Simulate scanning a directory of session files
        let sessions = vec![
            LazySession::Unloaded {
                id: "sess-001".into(),
                name: "Debug auth".into(),
                message_count: 45,
                file_path: dir.join("sess-001.json"),
            },
            LazySession::Unloaded {
                id: "sess-002".into(),
                name: "API design".into(),
                message_count: 120,
                file_path: dir.join("sess-002.json"),
            },
            LazySession::Unloaded {
                id: "sess-003".into(),
                name: "Write tests".into(),
                message_count: 230,
                file_path: dir.join("sess-003.json"),
            },
        ];

        Self { sessions }
    }

    /// List session summaries (no disk I/O needed).
    pub fn list(&self) -> Vec<(&str, &str, bool)> {
        self.sessions.iter()
            .map(|s| (s.id(), s.name(), s.is_loaded()))
            .collect()
    }

    /// Get a session by index, loading it if necessary.
    pub fn get_mut(&mut self, index: usize) -> Option<&mut LazySession> {
        self.sessions.get_mut(index)
    }

    /// Memory usage: only count loaded sessions.
    pub fn loaded_memory_estimate(&self) -> usize {
        self.sessions.iter()
            .filter(|s| s.is_loaded())
            .count() * 10_000 // Rough estimate per loaded session
    }
}

fn main() {
    let dir = PathBuf::from("/tmp/sessions");
    let mut list = LazySessionList::scan_directory(&dir);

    // List sessions -- no disk I/O
    println!("Available sessions:");
    for (id, name, loaded) in list.list() {
        println!("  {} {} {}", id, name, if loaded { "(loaded)" } else { "" });
    }
    println!("Memory estimate: {} bytes\n", list.loaded_memory_estimate());

    // Access session 1 -- triggers disk load
    if let Some(session) = list.get_mut(1) {
        let msgs = session.messages().unwrap();
        println!("Session messages: {}", msgs.len());
    }

    // Now session 1 is loaded, others are not
    println!("\nAfter accessing session 1:");
    for (id, name, loaded) in list.list() {
        println!("  {} {} {}", id, name, if loaded { "(loaded)" } else { "" });
    }
    println!("Memory estimate: {} bytes", list.loaded_memory_estimate());
}
```

The `LazySession` enum uses Rust's enum discrimination to represent the two states (unloaded and loaded) in a type-safe way. There is no risk of accessing message data on an unloaded session -- the type system prevents it.

## Reducing Allocation Churn During Compaction

Compaction creates a lot of temporary allocations: filtered vectors, new messages, rebuilt indices. You can reduce churn by reusing buffers:

```rust
/// A reusable buffer pool for compaction operations.
pub struct CompactionBuffers {
    /// Reusable buffer for message IDs during filtering
    id_buffer: Vec<u64>,
    /// Reusable buffer for building output messages
    message_buffer: Vec<(String, String, usize)>,
    /// Reusable string buffer for building summaries
    string_buffer: String,
}

impl CompactionBuffers {
    pub fn new() -> Self {
        Self {
            id_buffer: Vec::with_capacity(256),
            message_buffer: Vec::with_capacity(256),
            string_buffer: String::with_capacity(4096),
        }
    }

    /// Clear all buffers for reuse (retains allocated capacity).
    pub fn clear(&mut self) {
        self.id_buffer.clear();
        self.message_buffer.clear();
        self.string_buffer.clear();
    }

    /// Get the current allocated capacity of all buffers.
    pub fn capacity_bytes(&self) -> usize {
        self.id_buffer.capacity() * std::mem::size_of::<u64>()
            + self.message_buffer.capacity() * std::mem::size_of::<(String, String, usize)>()
            + self.string_buffer.capacity()
    }
}

fn main() {
    let mut buffers = CompactionBuffers::new();

    // Simulate multiple compaction cycles
    for cycle in 0..5 {
        buffers.clear(); // Clears data, keeps allocated memory

        // Fill buffers as compaction would
        for i in 0..100 {
            buffers.id_buffer.push(i);
            buffers.message_buffer.push((
                "user".to_string(),
                format!("msg {}", i),
                10,
            ));
        }
        buffers.string_buffer.push_str("Summary of compacted messages...");

        println!("Cycle {}: {} IDs, capacity = {} bytes",
            cycle, buffers.id_buffer.len(), buffers.capacity_bytes());
    }

    // After 5 cycles, the buffers are right-sized for the workload
    // No new heap allocations needed for future compactions of similar size
    println!("Final capacity: {} bytes (stabilized)", buffers.capacity_bytes());
}
```

The key insight is `Vec::clear()` and `String::clear()` -- they set the length to zero but retain the allocated capacity. After a few compaction cycles, the buffers are right-sized for your workload and no new heap allocations are needed.

::: wild In the Wild
Claude Code is careful about memory management in long-running sessions. It avoids holding full file contents in memory after they have been sent to the API -- once a tool result is in the conversation history, the original file buffer is dropped. It also uses streaming deserialization for large session files, parsing messages one at a time rather than loading the entire JSON into memory at once. These patterns keep the agent responsive even during sessions with hundreds of tool calls.
:::

## Key Takeaways

- Profile your memory usage before optimizing -- the biggest consumer in a coding agent is message content strings, especially tool results
- Use string interning (`Arc<str>`) for frequently repeated values like role names, tool names, and file paths to eliminate redundant copies
- Implement lazy loading for session data so startup stays fast regardless of how many saved sessions exist on disk
- Reuse allocation buffers across compaction cycles using `clear()` which retains capacity but frees the data
- Rust's ownership model is an advantage here -- when you drop a message from context, its memory is freed immediately with no GC pause
