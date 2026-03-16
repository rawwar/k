---
title: Memory Patterns
description: Implementing short-term, long-term, and episodic memory systems that allow the agent to recall information across and within conversation sessions.
---

# Memory Patterns

> **What you'll learn:**
> - The distinction between working memory (current context window), short-term memory (session-level), and long-term memory (cross-session)
> - Implementing a key-value memory store that the agent can explicitly read from and write to during conversations
> - Episodic memory patterns that automatically extract and store reusable facts, preferences, and project-specific knowledge

Everything we've built so far -- message history, compaction, summarization, persistence -- deals with the *current session's* memory. But real productivity comes from memory that persists across sessions. When a user tells the agent "I prefer explicit error types over anyhow" in session 1, the agent should remember this in session 47. This is the difference between a tool you configure once and a tool you configure every time you use it.

## The Memory Hierarchy

Agent memory parallels the human memory hierarchy and computer memory architecture:

```rust
use std::collections::HashMap;

/// Working memory: the current context window contents.
/// Lost when the context window is compacted.
struct WorkingMemory {
    /// The active message history (what the LLM sees)
    context_window: MessageHistory,
    /// Token count of the current context
    tokens_used: u32,
}

/// Short-term memory: session-level information that survives compaction.
/// Lost when the session ends (unless promoted to long-term).
struct ShortTermMemory {
    /// Key facts learned during this session
    facts: Vec<MemoryEntry>,
    /// Files the agent has read or modified
    file_interactions: HashMap<String, FileMemory>,
    /// Errors encountered and their resolutions
    error_patterns: Vec<ErrorPattern>,
}

/// Long-term memory: cross-session knowledge stored on disk.
/// Survives process restarts and persists indefinitely.
struct LongTermMemory {
    /// User preferences (coding style, tools, conventions)
    preferences: HashMap<String, String>,
    /// Project-specific knowledge
    project_knowledge: HashMap<String, ProjectMemory>,
    /// Frequently accessed file summaries
    file_summaries: HashMap<String, FileSummary>,
    /// Storage backend
    store: Box<dyn MemoryStore>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct MemoryEntry {
    key: String,
    value: String,
    source: MemorySource,
    created_at: chrono::DateTime<chrono::Utc>,
    access_count: u32,
    last_accessed: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
enum MemorySource {
    UserExplicit,      // User directly told the agent
    AgentInferred,     // Agent extracted from conversation
    ToolResult,        // Learned from tool output
    CrossSession,      // Carried over from another session
}
```

::: python Coming from Python
This hierarchy maps directly to Python's variable scopes, but for an LLM. Working memory is like local variables (current function scope). Short-term memory is like instance variables (`self.x`). Long-term memory is like module-level globals or data stored in a database. The difference is that working memory has a hard size limit (the context window), so you need explicit strategies for what to keep and what to evict.
:::

## The Key-Value Memory Store

The simplest effective memory pattern is an explicit key-value store that the agent can read from and write to as a tool:

```rust
use std::path::PathBuf;

struct MemoryStore {
    memories: HashMap<String, MemoryEntry>,
    storage_path: PathBuf,
}

impl MemoryStore {
    fn new(storage_path: PathBuf) -> Result<Self, std::io::Error> {
        let memories = if storage_path.exists() {
            let data = std::fs::read_to_string(&storage_path)?;
            serde_json::from_str(&data).unwrap_or_default()
        } else {
            HashMap::new()
        };

        Ok(Self { memories, storage_path })
    }

    /// Store a memory (called by the agent via tool)
    fn remember(&mut self, key: String, value: String, source: MemorySource) {
        let now = chrono::Utc::now();
        let entry = MemoryEntry {
            key: key.clone(),
            value,
            source,
            created_at: now,
            access_count: 0,
            last_accessed: now,
        };
        self.memories.insert(key, entry);
        let _ = self.persist();
    }

    /// Recall a memory (called by the agent via tool)
    fn recall(&mut self, key: &str) -> Option<&MemoryEntry> {
        if let Some(entry) = self.memories.get_mut(key) {
            entry.access_count += 1;
            entry.last_accessed = chrono::Utc::now();
            Some(entry)
        } else {
            None
        }
    }

    /// Search memories by keyword
    fn search(&self, query: &str) -> Vec<&MemoryEntry> {
        let query_lower = query.to_lowercase();
        let mut results: Vec<&MemoryEntry> = self.memories.values()
            .filter(|entry| {
                entry.key.to_lowercase().contains(&query_lower)
                    || entry.value.to_lowercase().contains(&query_lower)
            })
            .collect();

        // Sort by access count (most frequently accessed first)
        results.sort_by(|a, b| b.access_count.cmp(&a.access_count));
        results
    }

    /// Get all memories as context for system prompt injection
    fn to_context_string(&self, max_tokens: u32, tokenizer: &dyn Tokenizer) -> String {
        let mut entries: Vec<&MemoryEntry> = self.memories.values().collect();
        // Sort by access frequency and recency
        entries.sort_by(|a, b| {
            b.access_count.cmp(&a.access_count)
                .then(b.last_accessed.cmp(&a.last_accessed))
        });

        let mut context = String::from("Known facts and preferences:\n");
        let mut tokens_used = tokenizer.count_tokens(&context);

        for entry in entries {
            let line = format!("- {}: {}\n", entry.key, entry.value);
            let line_tokens = tokenizer.count_tokens(&line);
            if tokens_used + line_tokens > max_tokens {
                break;
            }
            context.push_str(&line);
            tokens_used += line_tokens;
        }

        context
    }

    fn persist(&self) -> Result<(), std::io::Error> {
        let json = serde_json::to_string_pretty(&self.memories)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
        AtomicFileWriter::write_atomic(&self.storage_path, json.as_bytes())
    }

    /// Remove rarely accessed memories to prevent unbounded growth
    fn prune(&mut self, max_entries: usize) {
        if self.memories.len() <= max_entries {
            return;
        }

        let mut entries: Vec<(String, u32, chrono::DateTime<chrono::Utc>)> =
            self.memories.iter()
                .map(|(k, v)| (k.clone(), v.access_count, v.last_accessed))
                .collect();

        // Sort by access count ascending, then by last access ascending
        entries.sort_by(|a, b| {
            a.1.cmp(&b.1).then(a.2.cmp(&b.2))
        });

        // Remove the least accessed entries
        let to_remove = self.memories.len() - max_entries;
        for (key, _, _) in entries.iter().take(to_remove) {
            self.memories.remove(key);
        }

        let _ = self.persist();
    }
}
```

The memory store exposes `remember` and `recall` as tool-callable operations. The agent decides when to store and retrieve information, making memory management an explicit part of its reasoning:

```rust
/// Tool definitions for memory operations
fn memory_tool_definitions() -> Vec<ToolDefinition> {
    vec![
        ToolDefinition {
            name: "memory_store".into(),
            description: "Store a fact, preference, or piece of knowledge \
                          for future reference. Use this when the user tells \
                          you something you should remember across conversations.".into(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "key": {
                        "type": "string",
                        "description": "A descriptive key for the memory"
                    },
                    "value": {
                        "type": "string",
                        "description": "The information to remember"
                    }
                },
                "required": ["key", "value"]
            }),
        },
        ToolDefinition {
            name: "memory_recall".into(),
            description: "Search for previously stored memories. Use this \
                          to check if you already know something about the \
                          user's preferences or project.".into(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "query": {
                        "type": "string",
                        "description": "Search query for memories"
                    }
                },
                "required": ["query"]
            }),
        },
    ]
}
```

## Automatic Memory Extraction

Relying solely on the agent to decide what to remember is unreliable. You can automatically extract memories from the conversation:

```rust
struct MemoryExtractor {
    /// Patterns that indicate memorizable information
    patterns: Vec<ExtractionPattern>,
}

struct ExtractionPattern {
    /// Regex or keyword that triggers extraction
    trigger: String,
    /// Category of memory this pattern produces
    category: String,
    /// Function to extract the memory value from the message
    extractor: Box<dyn Fn(&str) -> Option<(String, String)>>,
}

impl MemoryExtractor {
    fn new() -> Self {
        let patterns = vec![
            ExtractionPattern {
                trigger: "prefer".into(),
                category: "preference".into(),
                extractor: Box::new(|text| {
                    // Extract preference statements like "I prefer X over Y"
                    if let Some(idx) = text.to_lowercase().find("prefer") {
                        let context = &text[idx..];
                        let end = context.find('.').unwrap_or(context.len()).min(200);
                        let preference = &context[..end];
                        Some(("user_preference".into(), preference.to_string()))
                    } else {
                        None
                    }
                }),
            },
            ExtractionPattern {
                trigger: "always use".into(),
                category: "convention".into(),
                extractor: Box::new(|text| {
                    if let Some(idx) = text.to_lowercase().find("always use") {
                        let context = &text[idx..];
                        let end = context.find('.').unwrap_or(context.len()).min(200);
                        Some(("convention".into(), context[..end].to_string()))
                    } else {
                        None
                    }
                }),
            },
        ];

        Self { patterns }
    }

    fn extract_from_message(&self, msg: &Message) -> Vec<(String, String)> {
        let mut extracted = Vec::new();
        for block in &msg.content {
            if let ContentBlock::Text(text) = block {
                for pattern in &self.patterns {
                    if text.to_lowercase().contains(&pattern.trigger) {
                        if let Some((key, value)) = (pattern.extractor)(text) {
                            extracted.push((
                                format!("{}_{}", pattern.category, key),
                                value,
                            ));
                        }
                    }
                }
            }
        }
        extracted
    }
}
```

::: wild In the Wild
Claude Code implements a memory system through its `CLAUDE.md` files. Project-level memories are stored in `CLAUDE.md` at the project root, user-level memories in `~/.claude/CLAUDE.md`, and the agent can read and update these files during conversations. This is an elegant approach because the memory is human-readable and editable -- users can review, modify, and version-control their agent's memories alongside their code. The Pi coding agent takes a different approach with an explicit `/memory` command that lets users manage stored facts through a structured interface.
:::

## Episodic Memory

Episodic memory stores summaries of past sessions -- what was done, what was learned, and how problems were solved. This is more structured than raw session persistence:

```rust
#[derive(Debug, serde::Serialize, serde::Deserialize)]
struct Episode {
    session_id: String,
    timestamp: chrono::DateTime<chrono::Utc>,
    /// One-sentence summary of what was done
    summary: String,
    /// Key files involved
    files: Vec<String>,
    /// Technologies/patterns used
    tags: Vec<String>,
    /// Problems solved and how
    solutions: Vec<ProblemSolution>,
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
struct ProblemSolution {
    problem: String,
    solution: String,
    files_involved: Vec<String>,
}

struct EpisodicMemory {
    episodes: Vec<Episode>,
    storage_path: PathBuf,
}

impl EpisodicMemory {
    fn record_episode(&mut self, session: &PersistedSession) {
        let episode = Episode {
            session_id: session.metadata.id.to_string(),
            timestamp: chrono::Utc::now(),
            summary: self.generate_summary(&session.messages),
            files: self.extract_files(&session.messages),
            tags: self.extract_tags(&session.messages),
            solutions: self.extract_solutions(&session.messages),
        };
        self.episodes.push(episode);
        let _ = self.persist();
    }

    /// Find relevant past episodes for the current task
    fn find_relevant(&self, query: &str, limit: usize) -> Vec<&Episode> {
        let query_lower = query.to_lowercase();
        let mut scored: Vec<(&Episode, f32)> = self.episodes.iter()
            .map(|ep| {
                let mut score = 0.0f32;
                if ep.summary.to_lowercase().contains(&query_lower) {
                    score += 3.0;
                }
                for tag in &ep.tags {
                    if tag.to_lowercase().contains(&query_lower) {
                        score += 2.0;
                    }
                }
                for file in &ep.files {
                    if file.to_lowercase().contains(&query_lower) {
                        score += 1.5;
                    }
                }
                for solution in &ep.solutions {
                    if solution.problem.to_lowercase().contains(&query_lower) {
                        score += 2.5;
                    }
                }
                (ep, score)
            })
            .filter(|(_, score)| *score > 0.0)
            .collect();

        scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());
        scored.into_iter()
            .take(limit)
            .map(|(ep, _)| ep)
            .collect()
    }

    fn generate_summary(&self, messages: &[PersistedMessage]) -> String {
        // In practice, use an LLM call for better summaries
        messages.iter()
            .filter(|m| m.role == "User")
            .take(1)
            .map(|m| {
                let text: String = m.content.iter().filter_map(|b| match b {
                    PersistedContentBlock::Text(t) => Some(t.clone()),
                    _ => None,
                }).collect();
                if text.len() > 100 {
                    format!("{}...", &text[..100])
                } else {
                    text
                }
            })
            .next()
            .unwrap_or_else(|| "Unknown task".to_string())
    }

    fn extract_files(&self, messages: &[PersistedMessage]) -> Vec<String> {
        let mut files = std::collections::HashSet::new();
        for msg in messages {
            for block in &msg.content {
                if let PersistedContentBlock::ToolUse { name, input, .. } = block {
                    if name == "read_file" || name == "write_file" {
                        if let Some(path) = input.get("path").and_then(|v| v.as_str()) {
                            files.insert(path.to_string());
                        }
                    }
                }
            }
        }
        files.into_iter().collect()
    }

    fn extract_tags(&self, _messages: &[PersistedMessage]) -> Vec<String> {
        Vec::new() // Would use LLM extraction in practice
    }

    fn extract_solutions(&self, _messages: &[PersistedMessage]) -> Vec<ProblemSolution> {
        Vec::new() // Would use LLM extraction in practice
    }

    fn persist(&self) -> Result<(), std::io::Error> {
        let json = serde_json::to_string_pretty(&self.episodes)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
        AtomicFileWriter::write_atomic(&self.storage_path, json.as_bytes())
    }
}
```

Episodic memory is queried at the start of each new session. When a user says "fix the authentication bug," the agent searches episodic memory for past sessions involving authentication, finding relevant solutions and file paths. This context is injected into the system prompt, giving the agent a head start.

## Key Takeaways

- Agent memory has three tiers: working memory (context window, volatile), short-term memory (session-level, survives compaction), and long-term memory (cross-session, persisted to disk).
- A key-value memory store with `remember` and `recall` tool operations lets the agent explicitly manage its long-term knowledge, with pruning based on access frequency to prevent unbounded growth.
- Automatic memory extraction detects preference statements and conventions from user messages, supplementing the agent's explicit memory decisions.
- Episodic memory records structured summaries of past sessions (task, files, solutions) that can be searched when starting new sessions for relevant context.
- Store memories in human-readable, editable formats (like Claude Code's `CLAUDE.md`) so users can review, correct, and version-control what their agent remembers.
