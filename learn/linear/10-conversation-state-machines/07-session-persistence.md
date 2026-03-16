---
title: Session Persistence
description: Saving and restoring conversation sessions across process restarts, including atomic writes, crash recovery, and efficient incremental persistence.
---

# Session Persistence

> **What you'll learn:**
> - How to persist conversation state to disk with atomic writes that prevent corruption from crashes or power loss
> - Incremental persistence strategies that append new messages instead of rewriting the entire history on each change
> - Session resume logic that validates persisted state, handles schema migrations, and recovers from partial writes

A coding agent that loses its conversation when the terminal closes is frustrating. A coding agent that loses its conversation because the process crashed in the middle of saving is unforgivable. Session persistence lets users close their terminal, sleep their laptop, and come back hours or days later to continue exactly where they left off. But persistence introduces failure modes that don't exist in memory-only conversations: partial writes, corrupt files, schema changes between agent versions, and the challenge of keeping disk state in sync with memory state.

## Session Identity

Every session needs a unique identifier and metadata. This is the "envelope" that wraps the conversation:

```rust
use chrono::{DateTime, Utc};
use uuid::Uuid;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
struct SessionMetadata {
    /// Unique session identifier
    id: Uuid,
    /// Human-readable session title (often derived from first user message)
    title: String,
    /// When the session was created
    created_at: DateTime<Utc>,
    /// When the session was last modified
    updated_at: DateTime<Utc>,
    /// Working directory when the session started
    working_directory: PathBuf,
    /// Agent version that created this session
    agent_version: String,
    /// Schema version for migration handling
    schema_version: u32,
    /// Total messages in the session
    message_count: usize,
    /// Total tokens consumed (for cost tracking)
    total_tokens_used: u64,
}

impl SessionMetadata {
    fn new(working_directory: PathBuf) -> Self {
        Self {
            id: Uuid::new_v4(),
            title: String::new(),
            created_at: Utc::now(),
            updated_at: Utc::now(),
            working_directory,
            agent_version: env!("CARGO_PKG_VERSION").to_string(),
            schema_version: 1,
            message_count: 0,
            total_tokens_used: 0,
        }
    }
}
```

The `schema_version` is critical for long-lived agents. When you change your message format (adding a field, changing an enum variant), older session files won't match the new schema. The version number lets you detect this and run a migration path.

::: python Coming from Python
Python developers often use `pickle` for quick persistence, but it's fragile across versions and a security risk for loading untrusted data. Rust's `serde` gives you explicit serialization to known formats (JSON, MessagePack, etc.) with compile-time guarantees that all fields are handled. If you add a field to a struct and forget to update the serialization, the compiler tells you.
:::

## Atomic Writes

The most common persistence bug is a corrupt file from a crash during writing. If the process dies halfway through writing the session file, you have a file that's neither the old version nor the new version -- it's garbage. The solution is atomic writes:

```rust
use std::fs;
use std::io::Write;

struct AtomicFileWriter;

impl AtomicFileWriter {
    /// Write data to a file atomically using write-to-temp-then-rename
    fn write_atomic(path: &std::path::Path, data: &[u8]) -> std::io::Result<()> {
        // Create a temporary file in the same directory
        // (same filesystem ensures rename is atomic)
        let temp_path = path.with_extension("tmp");

        // Write to the temporary file
        let mut file = fs::File::create(&temp_path)?;
        file.write_all(data)?;
        file.sync_all()?; // Flush to disk, not just OS buffer

        // Atomic rename: this is guaranteed atomic on POSIX systems
        fs::rename(&temp_path, path)?;

        Ok(())
    }
}
```

The key insight: `fs::rename` on the same filesystem is an atomic operation on POSIX systems (macOS, Linux). Either the old file exists or the new one does -- never a partial state. The `sync_all()` call before the rename ensures data is actually on disk, not just in the OS write buffer. Without it, a power failure could still lose data even though the rename succeeded.

## Full Session Persistence

The simplest persistence strategy: serialize the entire session to a file after every change. This is fine for short sessions but becomes expensive for long ones:

```rust
#[derive(Serialize, Deserialize)]
struct PersistedSession {
    metadata: SessionMetadata,
    conversation_state: String, // Serialized ConversationState
    messages: Vec<PersistedMessage>,
}

#[derive(Serialize, Deserialize)]
struct PersistedMessage {
    id: String,
    role: String,
    content: Vec<PersistedContentBlock>,
    timestamp: String,
    token_count: Option<u32>,
    is_synthetic: bool,
    replaces: Vec<String>,
}

struct FullSessionPersister {
    session_dir: PathBuf,
}

impl FullSessionPersister {
    fn new(session_dir: PathBuf) -> std::io::Result<Self> {
        fs::create_dir_all(&session_dir)?;
        Ok(Self { session_dir })
    }

    fn session_path(&self, session_id: &Uuid) -> PathBuf {
        self.session_dir.join(format!("{}.json", session_id))
    }

    fn save(
        &self,
        metadata: &SessionMetadata,
        state: &ConversationState,
        history: &MessageHistory,
    ) -> Result<(), PersistenceError> {
        let persisted = PersistedSession {
            metadata: metadata.clone(),
            conversation_state: format!("{:?}", state),
            messages: history.iter()
                .map(|m| self.serialize_message(m))
                .collect(),
        };

        let json = serde_json::to_string_pretty(&persisted)
            .map_err(PersistenceError::Serialization)?;

        let path = self.session_path(&metadata.id);
        AtomicFileWriter::write_atomic(&path, json.as_bytes())
            .map_err(PersistenceError::Io)?;

        Ok(())
    }

    fn load(&self, session_id: &Uuid) -> Result<PersistedSession, PersistenceError> {
        let path = self.session_path(session_id);
        let data = fs::read_to_string(&path)
            .map_err(PersistenceError::Io)?;
        let session: PersistedSession = serde_json::from_str(&data)
            .map_err(PersistenceError::Deserialization)?;
        Ok(session)
    }

    fn serialize_message(&self, msg: &Message) -> PersistedMessage {
        PersistedMessage {
            id: msg.id.to_string(),
            role: format!("{:?}", msg.role),
            content: msg.content.iter().map(|b| self.serialize_block(b)).collect(),
            timestamp: format!("{:?}", msg.timestamp),
            token_count: msg.token_count,
            is_synthetic: msg.metadata.is_synthetic,
            replaces: msg.metadata.replaces.iter().map(|id| id.to_string()).collect(),
        }
    }

    fn serialize_block(&self, block: &ContentBlock) -> PersistedContentBlock {
        match block {
            ContentBlock::Text(t) => PersistedContentBlock::Text(t.clone()),
            ContentBlock::ToolUse { id, name, input } => {
                PersistedContentBlock::ToolUse {
                    id: id.clone(),
                    name: name.clone(),
                    input: input.clone(),
                }
            }
            ContentBlock::ToolResult { tool_use_id, content, is_error } => {
                PersistedContentBlock::ToolResult {
                    tool_use_id: tool_use_id.clone(),
                    content: content.clone(),
                    is_error: *is_error,
                }
            }
        }
    }
}

#[derive(Serialize, Deserialize)]
enum PersistedContentBlock {
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
```

## Incremental Persistence with Append Logs

For long sessions, rewriting the entire file after every message is wasteful. An append log writes only new messages:

```rust
struct AppendLogPersister {
    session_dir: PathBuf,
}

impl AppendLogPersister {
    fn metadata_path(&self, session_id: &Uuid) -> PathBuf {
        self.session_dir.join(format!("{}.meta.json", session_id))
    }

    fn log_path(&self, session_id: &Uuid) -> PathBuf {
        self.session_dir.join(format!("{}.log.jsonl", session_id))
    }

    fn append_message(
        &self,
        session_id: &Uuid,
        message: &Message,
    ) -> Result<(), PersistenceError> {
        let path = self.log_path(session_id);
        let mut file = fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)
            .map_err(PersistenceError::Io)?;

        let json = serde_json::to_string(&self.serialize_message(message))
            .map_err(PersistenceError::Serialization)?;

        writeln!(file, "{}", json).map_err(PersistenceError::Io)?;
        file.sync_all().map_err(PersistenceError::Io)?;

        Ok(())
    }

    fn load_messages(&self, session_id: &Uuid) -> Result<Vec<PersistedMessage>, PersistenceError> {
        let path = self.log_path(session_id);
        let data = fs::read_to_string(&path).map_err(PersistenceError::Io)?;

        let mut messages = Vec::new();
        for (line_num, line) in data.lines().enumerate() {
            if line.trim().is_empty() {
                continue;
            }
            match serde_json::from_str::<PersistedMessage>(line) {
                Ok(msg) => messages.push(msg),
                Err(e) => {
                    eprintln!(
                        "Warning: Skipping corrupt line {} in session log: {}",
                        line_num + 1, e
                    );
                    // Continue loading -- partial data is better than no data
                }
            }
        }

        Ok(messages)
    }

    fn serialize_message(&self, msg: &Message) -> PersistedMessage {
        PersistedMessage {
            id: msg.id.to_string(),
            role: format!("{:?}", msg.role),
            content: msg.content.iter().map(|b| match b {
                ContentBlock::Text(t) => PersistedContentBlock::Text(t.clone()),
                ContentBlock::ToolUse { id, name, input } => {
                    PersistedContentBlock::ToolUse {
                        id: id.clone(), name: name.clone(), input: input.clone(),
                    }
                }
                ContentBlock::ToolResult { tool_use_id, content, is_error } => {
                    PersistedContentBlock::ToolResult {
                        tool_use_id: tool_use_id.clone(),
                        content: content.clone(),
                        is_error: *is_error,
                    }
                }
            }).collect(),
            timestamp: format!("{:?}", msg.timestamp),
            token_count: msg.token_count,
            is_synthetic: msg.metadata.is_synthetic,
            replaces: msg.metadata.replaces.iter().map(|id| id.to_string()).collect(),
        }
    }
}
```

The JSON Lines format (`.jsonl`) is perfect for append logs: each line is a self-contained JSON object. If the process crashes mid-write, at worst you lose one incomplete line. The loader skips corrupt lines and recovers everything else. This is a huge advantage over a single JSON file, where a missing closing bracket corrupts the entire file.

::: tip In the Wild
Claude Code persists session data to `~/.claude/projects/<project-hash>/` using a JSONL-based approach. Each session has a metadata file and a messages log. When resuming a session, Claude Code validates the message sequence (checking role alternation and tool call pairing) and repairs any inconsistencies from interrupted writes. OpenCode stores sessions in its config directory with separate files for metadata and message history, using JSON for the metadata and a binary format for message content to reduce disk usage.
:::

## Session Resume and Validation

Loading a session from disk is not just deserialization. You need to validate the data, migrate schemas if needed, and rebuild in-memory indexes:

```rust
struct SessionManager {
    persister: AppendLogPersister,
    schema_version: u32,
}

impl SessionManager {
    fn resume_session(
        &self,
        session_id: &Uuid,
    ) -> Result<(SessionMetadata, MessageHistory), PersistenceError> {
        // Load metadata
        let meta_path = self.persister.metadata_path(session_id);
        let meta_json = fs::read_to_string(&meta_path)
            .map_err(PersistenceError::Io)?;
        let mut metadata: SessionMetadata = serde_json::from_str(&meta_json)
            .map_err(PersistenceError::Deserialization)?;

        // Check schema version and migrate if needed
        if metadata.schema_version < self.schema_version {
            self.migrate_session(&mut metadata, session_id)?;
        }

        // Load messages
        let persisted_messages = self.persister.load_messages(session_id)?;
        let mut history = MessageHistory::new();

        for pmsg in persisted_messages {
            match self.deserialize_message(pmsg) {
                Ok(msg) => history.push(msg),
                Err(e) => {
                    eprintln!("Warning: Skipping invalid message: {}", e);
                }
            }
        }

        // Validate the loaded conversation
        let conversation = Conversation {
            state: ConversationState::Idle,
            messages: Vec::new(),
        };

        if let Err(violations) = conversation.validate_invariants() {
            eprintln!("Warning: Loaded session has {} invariant violations",
                violations.len());
            // Attempt repair: remove orphaned tool results, fix role alternation
            // This is better than refusing to load
        }

        metadata.updated_at = Utc::now();
        Ok((metadata, history))
    }

    fn migrate_session(
        &self,
        metadata: &mut SessionMetadata,
        session_id: &Uuid,
    ) -> Result<(), PersistenceError> {
        // Apply migrations sequentially
        if metadata.schema_version < 2 {
            // v1 -> v2: Added token_count field to messages
            // Old messages won't have token counts; they'll be None and
            // recalculated on load
            eprintln!("Migrating session {} from schema v1 to v2", session_id);
        }

        metadata.schema_version = self.schema_version;
        Ok(())
    }

    fn deserialize_message(&self, pmsg: PersistedMessage) -> Result<Message, PersistenceError> {
        let id = Uuid::parse_str(&pmsg.id)
            .map_err(|e| PersistenceError::InvalidData(format!("Bad UUID: {}", e)))?;

        let role = match pmsg.role.as_str() {
            "System" => Role::System,
            "User" => Role::User,
            "Assistant" => Role::Assistant,
            "ToolCall" => Role::ToolCall,
            "ToolResult" => Role::ToolResult,
            other => return Err(PersistenceError::InvalidData(
                format!("Unknown role: {}", other)
            )),
        };

        Ok(Message {
            id,
            role,
            content: pmsg.content.into_iter()
                .map(|b| self.deserialize_block(b))
                .collect(),
            timestamp: std::time::SystemTime::now(), // Simplified
            token_count: pmsg.token_count,
            metadata: MessageMetadata {
                is_synthetic: pmsg.is_synthetic,
                replaces: pmsg.replaces.iter()
                    .filter_map(|s| Uuid::parse_str(s).ok())
                    .collect(),
                ..Default::default()
            },
        })
    }

    fn deserialize_block(&self, block: PersistedContentBlock) -> ContentBlock {
        match block {
            PersistedContentBlock::Text(t) => ContentBlock::Text(t),
            PersistedContentBlock::ToolUse { id, name, input } => {
                ContentBlock::ToolUse { id, name, input }
            }
            PersistedContentBlock::ToolResult { tool_use_id, content, is_error } => {
                ContentBlock::ToolResult { tool_use_id, content, is_error }
            }
        }
    }
}
```

The philosophy here is "load what you can, warn about what you can't." A session with one corrupt message out of 200 is still valuable. Refusing to load it because of one bad line would be terrible user experience.

## Key Takeaways

- Use atomic writes (write to temp file, sync to disk, rename) to prevent session corruption from crashes or power loss.
- JSON Lines (JSONL) provides natural append-only persistence where each line is independent -- a corrupt line doesn't destroy the whole session.
- Every session needs a schema version number so you can detect and migrate old session formats when your message structure evolves.
- Session resume must validate the loaded data (role alternation, tool call pairing) and gracefully handle corruption by skipping invalid entries rather than refusing to load.
- Separate metadata (small, rewritten atomically) from message history (large, appended incrementally) to get the best of both persistence strategies.
