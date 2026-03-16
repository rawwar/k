---
title: Storage Formats
description: Comparing storage formats for conversation persistence — JSON Lines, SQLite, MessagePack, and custom binary formats — with tradeoffs for size, speed, and queryability.
---

# Storage Formats

> **What you'll learn:**
> - The tradeoffs between JSON Lines (human-readable, appendable), SQLite (queryable, ACID), and binary formats (compact, fast)
> - How JSON Lines provides a simple append-only log that naturally supports incremental persistence and crash recovery
> - When SQLite's query capabilities justify its complexity for features like conversation search and analytics

The previous subchapter covered *how* to persist sessions. This subchapter covers *what format* to persist them in. The choice of storage format affects everything: disk usage, load times, queryability, human debuggability, and the complexity of your persistence code. There is no universally best format -- the right choice depends on your agent's priorities.

## JSON Lines: The Simple Default

JSON Lines (JSONL) stores one JSON object per line. It's the format we used in the persistence subchapter, and it's the best starting point for most agents:

```rust
use std::io::{BufRead, BufWriter, Write};
use std::fs;

struct JsonLinesStore {
    base_dir: std::path::PathBuf,
}

impl JsonLinesStore {
    fn append(&self, session_id: &str, message: &PersistedMessage) -> std::io::Result<()> {
        let path = self.base_dir.join(format!("{}.jsonl", session_id));
        let file = fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)?;
        let mut writer = BufWriter::new(file);

        let json = serde_json::to_string(message)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
        writeln!(writer, "{}", json)?;
        writer.flush()?;

        Ok(())
    }

    fn load_all(&self, session_id: &str) -> std::io::Result<Vec<PersistedMessage>> {
        let path = self.base_dir.join(format!("{}.jsonl", session_id));
        let file = fs::File::open(&path)?;
        let reader = std::io::BufReader::new(file);

        let mut messages = Vec::new();
        for (line_num, line) in reader.lines().enumerate() {
            let line = line?;
            if line.trim().is_empty() {
                continue;
            }
            match serde_json::from_str::<PersistedMessage>(&line) {
                Ok(msg) => messages.push(msg),
                Err(e) => {
                    eprintln!("Skipping corrupt line {}: {}", line_num + 1, e);
                }
            }
        }

        Ok(messages)
    }

    fn list_sessions(&self) -> std::io::Result<Vec<String>> {
        let mut sessions = Vec::new();
        for entry in fs::read_dir(&self.base_dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.extension().map_or(false, |ext| ext == "jsonl") {
                if let Some(stem) = path.file_stem() {
                    sessions.push(stem.to_string_lossy().into_owned());
                }
            }
        }
        Ok(sessions)
    }
}
```

**Strengths**: Human-readable (open the file in any text editor), append-only (natural crash recovery), zero dependencies beyond `serde_json`, line-independent (one corrupt line doesn't affect others).

**Weaknesses**: No indexing (searching requires scanning all lines), no compression (verbose JSON means large files), no random access (loading message #500 requires reading messages #1-499 first).

::: python Coming from Python
JSONL is widely used in the Python ML ecosystem for training data. If you've worked with Hugging Face datasets or logged data for fine-tuning, you've used this format. The Rust version is nearly identical -- `serde_json::to_string` instead of `json.dumps`, file operations with explicit error handling instead of `with open(...)`.
:::

## SQLite: When You Need Queries

Once your agent needs to search across sessions ("which session discussed the auth refactor?"), list sessions by date, or compute usage analytics, flat files become painful. SQLite gives you a full relational database in a single file with no server process:

```rust
use rusqlite::{Connection, params};

struct SqliteStore {
    conn: Connection,
}

impl SqliteStore {
    fn open(path: &std::path::Path) -> Result<Self, rusqlite::Error> {
        let conn = Connection::open(path)?;

        // Enable WAL mode for better concurrent read performance
        conn.execute_batch("PRAGMA journal_mode=WAL;")?;

        // Create tables if they don't exist
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS sessions (
                id TEXT PRIMARY KEY,
                title TEXT NOT NULL,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL,
                working_directory TEXT NOT NULL,
                agent_version TEXT NOT NULL,
                schema_version INTEGER NOT NULL,
                total_tokens INTEGER DEFAULT 0
            );

            CREATE TABLE IF NOT EXISTS messages (
                id TEXT PRIMARY KEY,
                session_id TEXT NOT NULL,
                position INTEGER NOT NULL,
                role TEXT NOT NULL,
                content_json TEXT NOT NULL,
                timestamp TEXT NOT NULL,
                token_count INTEGER,
                is_synthetic BOOLEAN DEFAULT FALSE,
                FOREIGN KEY (session_id) REFERENCES sessions(id)
            );

            CREATE INDEX IF NOT EXISTS idx_messages_session
                ON messages(session_id, position);

            CREATE INDEX IF NOT EXISTS idx_messages_role
                ON messages(session_id, role);

            CREATE VIRTUAL TABLE IF NOT EXISTS message_fts
                USING fts5(content_text, content='messages', content_rowid='rowid');"
        )?;

        Ok(Self { conn })
    }

    fn insert_message(
        &self,
        session_id: &str,
        message: &PersistedMessage,
        position: usize,
    ) -> Result<(), rusqlite::Error> {
        let content_json = serde_json::to_string(&message.content)
            .unwrap_or_default();

        self.conn.execute(
            "INSERT INTO messages (id, session_id, position, role, content_json, \
             timestamp, token_count, is_synthetic) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                message.id,
                session_id,
                position,
                message.role,
                content_json,
                message.timestamp,
                message.token_count,
                message.is_synthetic,
            ],
        )?;

        // Update FTS index
        let text_content = self.extract_text_content(&message.content);
        self.conn.execute(
            "INSERT INTO message_fts(rowid, content_text) \
             VALUES (last_insert_rowid(), ?1)",
            params![text_content],
        )?;

        Ok(())
    }

    fn search_sessions(&self, query: &str) -> Result<Vec<SearchResult>, rusqlite::Error> {
        let mut stmt = self.conn.prepare(
            "SELECT m.session_id, s.title, snippet(message_fts, 0, '<mark>', '</mark>', '...', 32)
             FROM message_fts
             JOIN messages m ON m.rowid = message_fts.rowid
             JOIN sessions s ON s.id = m.session_id
             WHERE message_fts MATCH ?1
             ORDER BY rank
             LIMIT 20"
        )?;

        let results = stmt.query_map(params![query], |row| {
            Ok(SearchResult {
                session_id: row.get(0)?,
                session_title: row.get(1)?,
                snippet: row.get(2)?,
            })
        })?.filter_map(|r| r.ok()).collect();

        Ok(results)
    }

    fn load_session_messages(
        &self,
        session_id: &str,
    ) -> Result<Vec<PersistedMessage>, rusqlite::Error> {
        let mut stmt = self.conn.prepare(
            "SELECT id, role, content_json, timestamp, token_count, is_synthetic \
             FROM messages WHERE session_id = ?1 ORDER BY position"
        )?;

        let messages = stmt.query_map(params![session_id], |row| {
            let content_json: String = row.get(2)?;
            let content: Vec<PersistedContentBlock> = serde_json::from_str(&content_json)
                .unwrap_or_default();
            Ok(PersistedMessage {
                id: row.get(0)?,
                role: row.get(1)?,
                content,
                timestamp: row.get(3)?,
                token_count: row.get(4)?,
                is_synthetic: row.get(5)?,
                replaces: Vec::new(),
            })
        })?.filter_map(|r| r.ok()).collect();

        Ok(messages)
    }

    fn get_usage_stats(&self, session_id: &str) -> Result<UsageStats, rusqlite::Error> {
        let mut stmt = self.conn.prepare(
            "SELECT COUNT(*), SUM(token_count), MIN(timestamp), MAX(timestamp) \
             FROM messages WHERE session_id = ?1"
        )?;

        stmt.query_row(params![session_id], |row| {
            Ok(UsageStats {
                message_count: row.get(0)?,
                total_tokens: row.get::<_, Option<i64>>(1)?.unwrap_or(0) as u64,
                first_message: row.get(2)?,
                last_message: row.get(3)?,
            })
        })
    }

    fn extract_text_content(&self, content: &[PersistedContentBlock]) -> String {
        content.iter().map(|block| match block {
            PersistedContentBlock::Text(t) => t.clone(),
            PersistedContentBlock::ToolUse { name, .. } => format!("[tool: {}]", name),
            PersistedContentBlock::ToolResult { content, .. } => content.clone(),
        }).collect::<Vec<_>>().join(" ")
    }
}

#[derive(Debug)]
struct SearchResult {
    session_id: String,
    session_title: String,
    snippet: String,
}

#[derive(Debug)]
struct UsageStats {
    message_count: i64,
    total_tokens: u64,
    first_message: String,
    last_message: String,
}
```

The `fts5` virtual table gives you full-text search across all messages. A user can search for "authentication middleware" and find every session where that topic was discussed, with highlighted snippets showing the matching context.

**Strengths**: Full SQL queries, FTS search, ACID transactions, single-file storage, great library support via `rusqlite`.

**Weaknesses**: Heavier dependency, more complex code, not human-readable (need a SQLite client to inspect), append-only patterns require more care (WAL mode helps but inserts are slower than file appends).

::: tip In the Wild
Claude Code stores its session data in project-specific directories using file-based formats. The session history lives alongside project metadata, making it easy to associate conversations with specific codebases. For cross-session search, Claude Code uses the conversation index that maps session IDs to key topics and file paths. OpenCode uses SQLite for its session storage, which gives it built-in query capabilities for listing and searching past sessions.
:::

## MessagePack: Binary Compactness

MessagePack is a binary serialization format that's compatible with JSON data types but more compact and faster to parse:

```rust
use rmp_serde;

struct MessagePackStore {
    base_dir: std::path::PathBuf,
}

impl MessagePackStore {
    fn save_session(
        &self,
        session_id: &str,
        messages: &[PersistedMessage],
    ) -> Result<(), Box<dyn std::error::Error>> {
        let path = self.base_dir.join(format!("{}.msgpack", session_id));
        let data = rmp_serde::to_vec(messages)?;

        AtomicFileWriter::write_atomic(&path, &data)?;
        Ok(())
    }

    fn load_session(
        &self,
        session_id: &str,
    ) -> Result<Vec<PersistedMessage>, Box<dyn std::error::Error>> {
        let path = self.base_dir.join(format!("{}.msgpack", session_id));
        let data = fs::read(&path)?;
        let messages: Vec<PersistedMessage> = rmp_serde::from_slice(&data)?;
        Ok(messages)
    }

    fn file_size(&self, session_id: &str) -> std::io::Result<u64> {
        let path = self.base_dir.join(format!("{}.msgpack", session_id));
        Ok(fs::metadata(&path)?.len())
    }
}
```

**Strengths**: 30-50% smaller than JSON, faster serialization/deserialization, same data model (easy conversion from JSON).

**Weaknesses**: Not human-readable (need special tools to inspect), no append-only mode (must rewrite entire file), less ecosystem support for debugging.

## Choosing the Right Format

The decision matrix comes down to your priorities:

| Feature | JSON Lines | SQLite | MessagePack |
|---------|-----------|--------|-------------|
| Human readable | Yes | No (need client) | No |
| Append-only | Native | Via WAL | No |
| Crash recovery | Excellent (line-level) | Excellent (ACID) | Must rewrite fully |
| Search/query | Scan required | FTS + SQL | Scan required |
| Disk size | Large | Medium | Small |
| Parse speed | Moderate | Fast (indexed) | Fast |
| Dependencies | serde_json only | rusqlite | rmp-serde |
| Complexity | Low | High | Low |

For most coding agents, start with JSON Lines. It's the simplest to implement, easiest to debug, and good enough for sessions with hundreds of messages. Move to SQLite when you need cross-session search or usage analytics. Use MessagePack as a compression optimization for agents that store many large sessions and disk space is a concern.

You can also combine formats: JSON Lines for the active session (fast appends), with periodic compaction into SQLite for completed sessions (efficient search and storage).

```rust
enum StorageBackend {
    JsonLines(JsonLinesStore),
    Sqlite(SqliteStore),
    MessagePack(MessagePackStore),
}

impl StorageBackend {
    fn recommended_for(use_case: &str) -> Self {
        match use_case {
            "development" => StorageBackend::JsonLines(
                JsonLinesStore { base_dir: ".sessions/".into() }
            ),
            "production" => StorageBackend::Sqlite(
                SqliteStore::open(&".sessions/agent.db".into()).unwrap()
            ),
            "embedded" => StorageBackend::MessagePack(
                MessagePackStore { base_dir: ".sessions/".into() }
            ),
            _ => StorageBackend::JsonLines(
                JsonLinesStore { base_dir: ".sessions/".into() }
            ),
        }
    }
}
```

## Key Takeaways

- JSON Lines is the best default for conversation persistence: human-readable, naturally append-only, crash-resilient at the line level, and trivial to implement with `serde_json`.
- SQLite earns its complexity when you need cross-session search (via FTS5), usage analytics, or ACID transactions -- features that flat files cannot efficiently provide.
- MessagePack provides 30-50% size reduction over JSON with faster parsing, but sacrifices human readability and append-only capability.
- Consider combining formats: JSON Lines for active sessions (fast appends) and SQLite for archived sessions (efficient queries and search).
- The format choice is not permanent -- design your persistence layer behind a trait so you can swap implementations or migrate data between formats as your agent's needs evolve.
