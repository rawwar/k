# Session Persistence and Checkpointing in Coding Agents

## 1. Introduction

Context management doesn't end when the session closes. The most sophisticated coding agents
maintain state across sessions — preserving conversation history, tool call results, modified
file lists, and task progress so that work can be resumed without starting from scratch.

Session persistence sits at a fascinating intersection of concerns. Agents aggressively compact
context windows to stay within token limits, discarding older messages and summarizing outputs.
But persistence mechanisms want to retain *everything* — the full history of what happened,
what was tried, and what succeeded. This **persistence-context tension** is one of the defining
architectural challenges in modern coding agents.

The approaches vary dramatically. Some use SQLite databases with ACID transactions. Others
append JSON events to flat files. Gemini CLI maintains a shadow git repository, auto-committing
after each turn. And the most ambitious systems persist conversations across devices and teams.

---

## 2. SQLite for Agent State (OpenCode Deep-Dive)

OpenCode stores ALL conversation data in SQLite within the project directory (`.opencode/state.db`).

### Full Schema

```sql
CREATE TABLE sessions (
    id TEXT PRIMARY KEY,
    title TEXT NOT NULL DEFAULT '',
    message_count INTEGER NOT NULL DEFAULT 0,
    prompt_tokens INTEGER NOT NULL DEFAULT 0,
    completion_tokens INTEGER NOT NULL DEFAULT 0,
    summary_message_id TEXT,
    cost REAL NOT NULL DEFAULT 0,
    created_at INTEGER NOT NULL DEFAULT (unixepoch()),
    updated_at INTEGER NOT NULL DEFAULT (unixepoch())
);

CREATE TABLE messages (
    id TEXT PRIMARY KEY,
    session_id TEXT NOT NULL REFERENCES sessions(id),
    role TEXT NOT NULL,
    parts TEXT NOT NULL,  -- JSON-serialized ContentPart array
    model TEXT,
    created_at INTEGER NOT NULL DEFAULT (unixepoch())
);
```

### Schema Design Decisions

- **`parts TEXT NOT NULL`**: Content stored as JSON-serialized `ContentPart` array, preserving
  full structure of multi-modal messages (tool calls, results, images, text blocks).
- **`summary_message_id TEXT`**: The critical compaction integration field. Marks the boundary —
  messages before the referenced message have been summarized and can be excluded on restore.
- **Token and cost tracking**: `prompt_tokens`, `completion_tokens`, and `cost` tracked at
  session level, enabling cost accounting without re-parsing message content.
- **Unix epoch timestamps**: `unixepoch()` enables efficient range queries and sorting.

### Key Query Patterns

```sql
-- Resume a session: get messages after the summary boundary
SELECT * FROM messages
WHERE session_id = ? AND created_at >= (
    SELECT created_at FROM messages WHERE id = (
        SELECT summary_message_id FROM sessions WHERE id = ?
    )
)
ORDER BY created_at;

-- Session listing with cost summary
SELECT id, title, message_count, cost, created_at
FROM sessions ORDER BY updated_at DESC;
```

### Why SQLite

- **Zero configuration**: No database server to install or manage
- **Single-file portability**: Entire database in one file, trivially backed up
- **ACID transactions**: Safe concurrent reads/writes, crash-resistant
- **Universal support**: Every major language has SQLite bindings

Aider also uses SQLite indirectly through `diskcache` for repository map tag caching with
`mtime` invalidation — a different use case but the same embedded-database pattern.

---

## 3. File-Based Session Dumps

### OpenHands EventStream

OpenHands takes an append-only approach inspired by event sourcing:

- **EventStream**: Append-only log of all events. Events are never modified or deleted.
- **FileStore backend**: Events serialized as JSON, paginated for efficient retrieval.
- **Immutable history**: Context management decisions are *reversible*. Original events
  persist; only the "view" presented to the LLM changes.
- **Reconstruction**: Can reconstruct exactly what the agent saw at any point.
- **Event types**: Observations, actions, and agent messages — a complete session record.

The event sourcing pattern cleanly separates persisted state from runtime state. The
EventStream is the source of truth; the condensed context window is a derived view.

### Gemini CLI Shadow Git Repository

```
~/.gemini/history/<project_hash>/
├── .git/                    # Shadow git repository
├── conversation.json        # Full conversation history
├── tool_calls.json          # All tool calls and results
├── metadata.json            # Session metadata
└── files/                   # Snapshots of modified files
```

- **Auto-commit per turn**: Every conversational turn becomes a git commit checkpoint.
- **Git-based inspection**: Standard `git log`, `git diff`, `git show` work on history.
- **Restore via `/restore`**: Presents checkpoint list, user picks a restore point.
- **Leveraging existing infrastructure**: Uses git — a tool developers already understand.

### Pi Coding Agent Tree-Structured History

Pi uses JSONL with `id` and `parentId` fields forming a **tree structure**:

```json
{"id": "msg_001", "parentId": null, "role": "user", "content": "Fix the auth bug"}
{"id": "msg_002", "parentId": "msg_001", "role": "assistant", "content": "Looking..."}
{"id": "msg_003", "parentId": "msg_002", "role": "assistant", "content": "Found it..."}
{"id": "msg_004", "parentId": "msg_001", "role": "assistant", "content": "Alternative..."}
```

Here `msg_004` branches from `msg_001`, creating an alternative path without deleting
the original chain. Features include `/tree` navigation and `/fork` without history loss.
This maps naturally to how developers work — trying approaches, backing up, trying another.

---

## 4. Conversation Checkpointing

Checkpointing goes beyond persistence. It creates named, restorable snapshots that users
can navigate between.

### Claude Code's Approach

- **Session naming**: `--name "feature-auth"` for human-readable identifiers
- **Resume**: `--resume` continues last session; `--resume "feature-auth"` for named
- **State scope**: Conversation history, modified files list, task progress
- **Persistence**: Sessions survive process termination and system restarts

### Gemini CLI's Git-Based Checkpointing

- **Every turn = git commit**: Fine-grained restore points without explicit user action
- **`/restore` workflow**: Checkpoint list with timestamps, user picks restore point
- **Undo semantics**: Like "undo" for the entire conversation
- **Diffable**: Inspect changes between checkpoints with `git diff`

### Warp Drive

- **Cross-device persistence**: Conversations sync across devices via cloud storage
- **Team-shared sessions**: Conversations shared with team members
- **`/fork-and-compact`**: New conversation branch with compacted history
- **Persistent team knowledge**: Solutions discovered by one member accessible to others

### Droid's Cross-Interface Persistence

- **Interface-agnostic**: Single conversation spans web, CLI, GitHub, and Slack
- **Unified state**: Same session identity regardless of interface
- **Transition continuity**: Switching interfaces doesn't lose state

This is the most challenging architecture — requires centralized session store with
real-time synchronization accessible from multiple clients.

---

## 5. Resume from Checkpoint Patterns

- **Full restore**: Load entire conversation history, replay from beginning. Simple but
  impractical for long sessions that would exceed token limits.
- **Summary restore**: Load summary message + messages after summary (OpenCode's approach
  via `summary_message_id`). Solves token limits while preserving continuity.
- **Selective restore**: User chooses which checkpoint to resume from (Gemini CLI).
  Maximum flexibility but requires user to understand checkpoint history.

### The Cold-Start Problem

After restore, the agent faces "cold start" — it has history but may lack implicit context
accumulated during the original session. Agents handle this differently:

- **Claude Code**: Re-reads `CLAUDE.md` on every session start, ensuring project context
  is always available regardless of conversation history.
- **OpenCode**: Summary message is designed to capture essential context, but subtle
  understanding may be lost.
- **Gemini CLI**: Full checkpoint includes file snapshots, so the agent can re-examine
  codebase state at the checkpoint rather than current state.

---

## 6. Checkpoint Storage Strategies

| Strategy   | Queryable | Versioned | Portable | Complexity | Best For                       |
|------------|-----------|-----------|----------|------------|--------------------------------|
| SQLite     | ✅         | No        | ✅        | Low        | Single-agent, structured data  |
| JSON/JSONL | Limited   | No        | ✅        | Lowest     | Simple persistence             |
| Git        | Via diff  | ✅         | ✅        | Medium     | Checkpoint/restore workflows   |
| Cloud      | ✅         | Varies    | ✅✅       | High       | Team collaboration             |

**SQLite**: Best for structured queries over session metadata. Single-file, no infrastructure.
Limitation: no built-in versioning.

**JSON/JSONL**: Best for simplicity and human readability. JSONL suits append-only patterns.
Limitation: query efficiency degrades with large histories.

**Git**: Best when checkpoint/restore is primary workflow. Free versioning and diffing.
Limitation: storage efficiency — git isn't designed for large JSON blobs.

**Cloud**: Best for team collaboration and cross-device use. Limitation: complexity, privacy
concerns (data leaves local machine).

---

## 7. Multi-Session Management

As agents accumulate sessions, management becomes its own challenge:

- **Session listing**: Metadata display (title, timestamps, message count, cost) helps
  users find the right session without remembering arbitrary IDs.
- **Session naming**: Named sessions (Claude Code's `--name`) are valuable for long tasks.
  Common patterns: feature branch names, ticket numbers, task descriptions.
- **Cleanup and archival**: Agents rarely implement automatic cleanup. Better approaches
  would include age-based archival, cost-based retention, or git-branch-linked lifecycle.
- **Cost tracking**: OpenCode's per-session cost tracking enables queries like "how much
  did feature X cost?" — increasingly important as LLM costs become budget line items.

---

## 8. The Persistence-Context Tension

The fundamental tension: compaction discards information to free context space, but
persistence wants to retain everything. Agents resolve this differently:

### Goose: Visibility Flags

`agent_visible: false` makes messages invisible to the LLM but preserved in persisted
history. Cleanly separates "what the LLM sees" from "what is stored."

### OpenHands: Immutable EventStream + Condensed View

Two parallel representations: the immutable EventStream (complete history) and the
condensed view (LLM-visible subset). Context management only affects the view.
Any condensation decision is reversible.

### Codex: GhostSnapshot Items

`GhostSnapshot` items are preserved through compaction cycles — references to previous
states enabling undo/redo even after context has been compacted. Invisible to the LLM
during normal generation, materializable when needed for state restoration.

### mini-SWE-agent: No Compaction

Never compacts. The saved trajectory exactly matches what the LLM saw, making it
directly usable as training data. Sessions must stay within context window limits,
but for research and training-data generation, this trade-off is acceptable.

---

## 9. Best Practices

- **Separate LLM view from persistent state**: Persisted state should be a superset of
  the LLM view, never a subset. Enables aggressive context management without sacrificing
  future restore or analysis capability.

- **Use structured storage for metadata**: Session-level metadata (timestamps, costs,
  token counts) benefits from SQLite. "Show sessions from last week costing >$1" matters.

- **Implement checkpoint/restore for long tasks**: Any task taking more than a few minutes
  needs checkpointing. Losing an hour of agent work to an interruption is unacceptable.

- **Track costs per session**: Enables budget management, cost optimization, accountability,
  and ROI analysis comparing agent-assisted vs. manual development.

- **Consider cross-session learning**: Auto-memory patterns, instruction file updates
  based on observed patterns, and team knowledge extraction represent the frontier of
  persistent agent memory.

- **Design for debuggability**: The persistence layer is your forensic tool. Design it to
  answer: "What did the agent see when it made that decision?" Event sourcing and git-based
  checkpointing excel here because they preserve complete, ordered history.

---

## 10. Summary

Session persistence is a surprisingly deep problem space. The simplest approaches — dumping
conversation JSON to a file — work for basic use cases but fail to address the
persistence-context tension, multi-session management, cost tracking, and collaboration.

The most sophisticated approaches recognize that persistence and context management are two
sides of the same coin. They maintain immutable, complete session histories at the storage
layer while presenting compact, optimized views to the LLM. They provide checkpoint/restore
workflows that give users control over session navigation. And they track metadata that makes
sessions queryable, accountable, and debuggable.

The trajectory is clear: persistence is evolving from "save the chat log" toward durable,
structured, queryable agent memory that spans sessions, devices, and teams. The agents that
get persistence right will be the ones that feel like they truly *remember* — picking up
exactly where they left off, retaining lessons learned, and building on past work rather
than starting from scratch every time.
