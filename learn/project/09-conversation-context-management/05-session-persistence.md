---
title: Session Persistence
description: Save and restore conversation sessions to disk so users can resume work across agent restarts.
---

# Session Persistence

> **What you'll learn:**
> - How to serialize a complete session state including messages, metadata, and tool state to disk
> - How to implement atomic file writes to prevent session corruption on crashes
> - How to support session listing, selection, and deletion for multi-session management

A coding agent session can span hours of work -- reading files, making edits, running tests, iterating on solutions. If the user closes their terminal or the agent crashes, all that context vanishes. Session persistence fixes this by saving the conversation to disk so it can be resumed later.

## What Goes Into a Session?

A session is more than just the message history. You need to capture everything required to resume the conversation meaningfully:

```rust
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::time::SystemTime;

/// A unique session identifier.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SessionId(pub String);

impl SessionId {
    /// Generate a new session ID based on timestamp and random suffix.
    pub fn generate() -> Self {
        let timestamp = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_millis();
        let random: u32 = rand_bytes();
        Self(format!("{}-{:08x}", timestamp, random))
    }
}

/// Simple random bytes without pulling in the rand crate.
fn rand_bytes() -> u32 {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    let mut h = DefaultHasher::new();
    SystemTime::now().hash(&mut h);
    std::process::id().hash(&mut h);
    h.finish() as u32
}

/// Complete session state that gets persisted to disk.
#[derive(Debug, Serialize, Deserialize)]
pub struct Session {
    /// Unique session identifier
    pub id: SessionId,
    /// Human-readable session name (optional)
    pub name: Option<String>,
    /// When the session was created
    pub created_at: SystemTime,
    /// When the session was last modified
    pub updated_at: SystemTime,
    /// The conversation history
    pub history: ConversationHistory,
    /// Which model was being used
    pub model: String,
    /// The working directory when the session started
    pub working_directory: PathBuf,
    /// Session-level metadata (project name, git branch, etc.)
    pub metadata: SessionMetadata,
    /// Schema version for forward compatibility
    pub schema_version: u32,
}

/// Additional metadata attached to a session.
#[derive(Debug, Default, Serialize, Deserialize)]
pub struct SessionMetadata {
    /// The project name (derived from directory name or config)
    pub project_name: Option<String>,
    /// Git branch at session start
    pub git_branch: Option<String>,
    /// Total API calls made in this session
    pub api_call_count: u32,
    /// Total tokens consumed (input + output)
    pub total_tokens_used: u64,
    /// Any tags the user has applied to this session
    pub tags: Vec<String>,
}

impl Session {
    /// Create a new empty session.
    pub fn new(model: String, working_directory: PathBuf) -> Self {
        let now = SystemTime::now();
        Self {
            id: SessionId::generate(),
            name: None,
            created_at: now,
            updated_at: now,
            history: ConversationHistory::new(),
            model,
            working_directory,
            metadata: SessionMetadata::default(),
            schema_version: 1,
        }
    }

    /// Mark the session as modified (updates the timestamp).
    pub fn touch(&mut self) {
        self.updated_at = SystemTime::now();
    }
}

fn main() {
    let session = Session::new(
        "claude-sonnet-4-20250514".to_string(),
        PathBuf::from("/home/user/my-project"),
    );
    println!("New session: {:?}", session.id);
    println!("Model: {}", session.model);
    println!("Working dir: {}", session.working_directory.display());
}
```

The `schema_version` field is critical for long-term compatibility. When you change the session format in a future release, you can detect old files and migrate them.

## Atomic File Writes

The single most important principle of session persistence is: **never corrupt the user's data**. If the agent crashes mid-write, the session file should either contain the old data or the new data, never a mix. Atomic writes achieve this:

```rust
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

/// Manages session files on disk with atomic writes.
pub struct SessionStore {
    /// Directory where session files are stored
    base_dir: PathBuf,
}

impl SessionStore {
    pub fn new(base_dir: PathBuf) -> std::io::Result<Self> {
        fs::create_dir_all(&base_dir)?;
        Ok(Self { base_dir })
    }

    /// Default session storage location (~/.config/agent/sessions/).
    pub fn default_location() -> std::io::Result<Self> {
        let dir = dirs_path().join("sessions");
        Self::new(dir)
    }

    /// Get the file path for a given session ID.
    fn session_path(&self, id: &SessionId) -> PathBuf {
        self.base_dir.join(format!("{}.json", id.0))
    }

    /// Save a session to disk atomically.
    ///
    /// Writes to a temporary file first, then renames it to the final path.
    /// On most filesystems, rename is atomic -- if the process crashes during
    /// the write, the old file is still intact.
    pub fn save(&self, session: &Session) -> std::io::Result<()> {
        let final_path = self.session_path(&session.id);
        let temp_path = final_path.with_extension("json.tmp");

        // Serialize to JSON
        let json = serde_json::to_string_pretty(session)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;

        // Write to temp file
        let mut file = fs::File::create(&temp_path)?;
        file.write_all(json.as_bytes())?;
        file.sync_all()?; // Ensure data is flushed to disk

        // Atomic rename
        fs::rename(&temp_path, &final_path)?;

        Ok(())
    }

    /// Load a session from disk.
    pub fn load(&self, id: &SessionId) -> std::io::Result<Session> {
        let path = self.session_path(id);
        let json = fs::read_to_string(&path)?;
        let mut session: Session = serde_json::from_str(&json)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;

        // Rebuild any transient state (like the message index)
        session.history.rebuild_index();

        Ok(session)
    }

    /// Delete a session from disk.
    pub fn delete(&self, id: &SessionId) -> std::io::Result<()> {
        let path = self.session_path(id);
        fs::remove_file(&path)
    }

    /// List all available sessions with their metadata.
    pub fn list(&self) -> std::io::Result<Vec<SessionSummary>> {
        let mut sessions = Vec::new();

        for entry in fs::read_dir(&self.base_dir)? {
            let entry = entry?;
            let path = entry.path();

            if path.extension().map_or(false, |ext| ext == "json") {
                match self.load_summary(&path) {
                    Ok(summary) => sessions.push(summary),
                    Err(e) => {
                        eprintln!("Warning: skipping corrupt session file {:?}: {}",
                            path, e);
                    }
                }
            }
        }

        // Sort by last modified, most recent first
        sessions.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
        Ok(sessions)
    }

    /// Load just the summary metadata without deserializing the full history.
    /// This is efficient for listing sessions.
    fn load_summary(&self, path: &Path) -> std::io::Result<SessionSummary> {
        let json = fs::read_to_string(path)?;
        // Parse just enough to get the summary fields
        let session: Session = serde_json::from_str(&json)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;

        Ok(SessionSummary {
            id: session.id,
            name: session.name,
            created_at: session.created_at,
            updated_at: session.updated_at,
            message_count: session.history.len(),
            model: session.model,
            working_directory: session.working_directory,
        })
    }
}

/// Lightweight summary of a session for listing purposes.
#[derive(Debug)]
pub struct SessionSummary {
    pub id: SessionId,
    pub name: Option<String>,
    pub created_at: SystemTime,
    pub updated_at: SystemTime,
    pub message_count: usize,
    pub model: String,
    pub working_directory: PathBuf,
}

/// Helper to get the config directory path.
fn dirs_path() -> PathBuf {
    // Use $HOME/.config/agent on all platforms for simplicity
    let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".to_string());
    PathBuf::from(home).join(".config").join("agent")
}

fn main() -> std::io::Result<()> {
    // Create a temporary store for demonstration
    let temp_dir = std::env::temp_dir().join("agent-session-demo");
    let store = SessionStore::new(temp_dir)?;

    // Create and save a session
    let mut session = Session::new(
        "claude-sonnet-4-20250514".to_string(),
        PathBuf::from("/home/user/project"),
    );
    session.name = Some("Refactoring auth module".to_string());
    session.history.push(
        Role::User,
        "Help me refactor the auth module".to_string(),
        9,
    );
    session.touch();
    store.save(&session)?;
    println!("Saved session: {:?}", session.id);

    // Load it back
    let loaded = store.load(&session.id)?;
    println!("Loaded session: {} messages", loaded.history.len());
    println!("Name: {:?}", loaded.name);

    // List all sessions
    let sessions = store.list()?;
    println!("\nAll sessions:");
    for s in &sessions {
        println!("  {} - {:?} ({} msgs)",
            s.id.0,
            s.name.as_deref().unwrap_or("unnamed"),
            s.message_count);
    }

    // Clean up
    store.delete(&session.id)?;
    println!("\nDeleted session");

    Ok(())
}
```

::: python Coming from Python
In Python, atomic writes often use `tempfile.NamedTemporaryFile` with a rename:
```python
import tempfile, os, json
with tempfile.NamedTemporaryFile(mode='w', dir=session_dir,
                                  suffix='.tmp', delete=False) as f:
    json.dump(session_data, f)
    f.flush()
    os.fsync(f.fileno())
    temp_path = f.name
os.rename(temp_path, final_path)
```
The Rust version does the same thing but with explicit `sync_all()` instead of
`os.fsync()`. The key difference is error handling -- in Rust, every file
operation returns a `Result` that you must handle. There is no way to
accidentally ignore a write failure.
:::

## Auto-Save Strategy

You do not want to save after every single message -- that would be too much I/O. But you also cannot wait until the session ends -- the user might close the terminal. A good strategy is to save at natural breakpoints:

```rust
/// Determines when to auto-save the session.
pub struct AutoSavePolicy {
    /// Save after this many new messages
    pub message_threshold: usize,
    /// Save after this many seconds since last save
    pub time_threshold_secs: u64,
    /// Always save after tool execution completes
    pub save_after_tool: bool,
    /// Messages since last save
    messages_since_save: usize,
    /// Time of last save
    last_save: SystemTime,
}

impl AutoSavePolicy {
    pub fn new() -> Self {
        Self {
            message_threshold: 5,
            time_threshold_secs: 30,
            save_after_tool: true,
            messages_since_save: 0,
            last_save: SystemTime::now(),
        }
    }

    /// Record that a message was added. Returns true if we should save.
    pub fn on_message(&mut self, is_tool_result: bool) -> bool {
        self.messages_since_save += 1;

        if is_tool_result && self.save_after_tool {
            return self.should_save();
        }

        self.should_save()
    }

    /// Check if it is time to save.
    fn should_save(&self) -> bool {
        if self.messages_since_save >= self.message_threshold {
            return true;
        }

        let elapsed = SystemTime::now()
            .duration_since(self.last_save)
            .unwrap_or_default();

        elapsed.as_secs() >= self.time_threshold_secs
    }

    /// Record that a save was performed.
    pub fn did_save(&mut self) {
        self.messages_since_save = 0;
        self.last_save = SystemTime::now();
    }
}

fn main() {
    let mut policy = AutoSavePolicy::new();

    // Simulate 10 messages
    for i in 1..=10 {
        let is_tool = i % 3 == 0; // Every 3rd message is a tool result
        let should_save = policy.on_message(is_tool);
        println!("Message {}{}: should_save={}",
            i,
            if is_tool { " (tool)" } else { "" },
            should_save);
        if should_save {
            println!("  -> Saving!");
            policy.did_save();
        }
    }
}
```

::: wild In the Wild
Claude Code persists session state after every completed tool execution and assistant response. This ensures that if the process is killed, the user loses at most the current in-flight request. The session files are stored in a platform-specific data directory and include enough metadata to display a meaningful session list for the `--resume` command. OpenCode takes a similar approach, saving after each agentic loop iteration.
:::

## Handling Schema Evolution

As your agent evolves, the session format will change. You need to handle old session files gracefully:

```rust
use serde_json::Value;

/// Migrate a session file from an older schema version to the current one.
pub fn migrate_session(json: &str) -> Result<Session, String> {
    // First, parse as generic JSON to check the version
    let value: Value = serde_json::from_str(json)
        .map_err(|e| format!("Invalid JSON: {}", e))?;

    let version = value.get("schema_version")
        .and_then(|v| v.as_u64())
        .unwrap_or(0) as u32;

    match version {
        0 => {
            // Version 0: legacy format without schema_version field
            // Add default metadata and set version to 1
            let mut value = value;
            if let Some(obj) = value.as_object_mut() {
                obj.insert("schema_version".to_string(), serde_json::json!(1));
                obj.entry("metadata".to_string())
                    .or_insert(serde_json::json!({}));
            }
            let updated = serde_json::to_string(&value)
                .map_err(|e| format!("Serialization failed: {}", e))?;
            serde_json::from_str(&updated)
                .map_err(|e| format!("Migration from v0 failed: {}", e))
        }
        1 => {
            // Current version -- deserialize directly
            serde_json::from_str(json)
                .map_err(|e| format!("Deserialization failed: {}", e))
        }
        other => {
            Err(format!(
                "Session file is from a newer version ({}) than this agent supports (1). \
                 Please update your agent.",
                other
            ))
        }
    }
}

fn main() {
    // Simulate loading a legacy session without schema_version
    let legacy_json = r#"{
        "id": {"0": "12345"},
        "name": null,
        "created_at": {"secs_since_epoch": 1700000000, "nanos_since_epoch": 0},
        "updated_at": {"secs_since_epoch": 1700000000, "nanos_since_epoch": 0},
        "history": {"messages": [], "total_tokens": 0, "next_id": 1},
        "model": "claude-3-sonnet",
        "working_directory": "/tmp"
    }"#;

    match migrate_session(legacy_json) {
        Ok(session) => println!("Migrated successfully: v{}", session.schema_version),
        Err(e) => println!("Migration failed: {}", e),
    }
}
```

## Key Takeaways

- A session includes more than message history -- capture the model, working directory, metadata, and schema version for full resume capability
- Use atomic writes (write to temp file, sync, rename) to prevent session corruption from crashes or power loss
- Auto-save at natural breakpoints (after tool execution, every N messages, or after time thresholds) rather than on every message
- Include a `schema_version` field from day one and implement migration logic for forward compatibility
- Build a `SessionStore` abstraction that handles file I/O, listing, and cleanup so the rest of your code never touches the filesystem directly
