# OpenCode — Context Management

## Overview

OpenCode's context management centers around three strategies: token usage tracking per session, auto-compaction when approaching the context window limit, and session persistence via SQLite. Unlike some agents that implement sophisticated token counting or sliding window approaches, OpenCode takes a pragmatic approach—letting the LLM providers handle most token management while providing conversation summarization as the primary context reduction mechanism.

## Token Usage Tracking

Token usage is tracked at the session level after each LLM response:

```go
// internal/llm/agent/agent.go
func (a *agent) TrackUsage(ctx context.Context, sessionID string,
    model models.Model, usage provider.TokenUsage) error {

    sess, _ := a.sessions.Get(ctx, sessionID)

    // Calculate cost from token counts and model pricing
    cost := model.CostPer1MInCached/1e6*float64(usage.CacheCreationTokens) +
        model.CostPer1MOutCached/1e6*float64(usage.CacheReadTokens) +
        model.CostPer1MIn/1e6*float64(usage.InputTokens) +
        model.CostPer1MOut/1e6*float64(usage.OutputTokens)

    sess.Cost += cost
    sess.CompletionTokens = usage.OutputTokens + usage.CacheReadTokens
    sess.PromptTokens = usage.InputTokens + usage.CacheCreationTokens

    a.sessions.Save(ctx, sess)
    return nil
}
```

### TokenUsage Structure

The provider layer reports token usage via:

```go
// internal/llm/provider/provider.go
type TokenUsage struct {
    InputTokens         int64
    OutputTokens        int64
    CacheCreationTokens int64
    CacheReadTokens     int64
}
```

This tracks four dimensions:
1. **InputTokens**: Standard input tokens consumed
2. **OutputTokens**: Tokens generated in the response
3. **CacheCreationTokens**: Tokens used to create prompt caches (Anthropic)
4. **CacheReadTokens**: Tokens served from cache (Anthropic)

### Session-Level Tracking

Each session stores cumulative token usage:

```go
// internal/session/session.go
type Session struct {
    ID               string
    Title            string
    MessageCount     int64
    PromptTokens     int64      // Cumulative input tokens
    CompletionTokens int64      // Cumulative output tokens
    Cost             float64    // Cumulative USD cost
    // ...
}
```

### Cost Calculation

Costs are calculated per-turn using the model's pricing data:

```go
// internal/llm/models/models.go
type Model struct {
    CostPer1MIn         float64  // $/1M input tokens
    CostPer1MOut        float64  // $/1M output tokens
    CostPer1MInCached   float64  // $/1M cache creation tokens
    CostPer1MOutCached  float64  // $/1M cache read tokens
    ContextWindow       int64    // Max context window size
    DefaultMaxTokens    int64    // Default max output tokens
    // ...
}
```

Example pricing from the codebase (Anthropic):
- Claude 3.7 Sonnet: $3/1M input, $15/1M output
- Claude 3.5 Haiku: $0.80/1M input, $4/1M output

## Context Window Management

### Model Context Windows

Each model definition includes its context window size:

```go
// Example from models/anthropic.go
Claude37Sonnet: {
    ContextWindow:    200000,  // 200K tokens
    DefaultMaxTokens: 16384,
},
```

### Auto-Compact Feature

OpenCode includes an auto-compact feature that triggers summarization when approaching the context limit:

```json
{
  "autoCompact": true  // default is true
}
```

When enabled:
- Monitors token usage during conversation
- Automatically triggers summarization when usage reaches **95% of the model's context window**
- Creates a summary message in the current session
- Subsequent turns start from the summary rather than the full history

### How Compaction Works

The summarization flow (from `agent.Summarize()`):

```
1. Load all messages from the session
       │
2. Append summarization prompt:
   "Provide a detailed but concise summary of our conversation above.
    Focus on information that would be helpful for continuing..."
       │
3. Send entire history + prompt to summarize provider
   (non-streaming, synchronous call)
       │
4. Create summary message in the session (role: assistant)
       │
5. Set session.SummaryMessageID = summary message ID
       │
6. On next processGeneration():
   - Find summary message index in history
   - Truncate all messages before it
   - Re-role summary as "user" message
   - Continue from there
```

The key implementation in `processGeneration()`:

```go
session, _ := a.sessions.Get(ctx, sessionID)
if session.SummaryMessageID != "" {
    summaryMsgIndex := -1
    for i, msg := range msgs {
        if msg.ID == session.SummaryMessageID {
            summaryMsgIndex = i
            break
        }
    }
    if summaryMsgIndex != -1 {
        msgs = msgs[summaryMsgIndex:]
        msgs[0].Role = message.User  // Re-role as user message
    }
}
```

### Summarization Prompt

The summarizer uses a focused prompt:

```go
// internal/llm/prompt/summarizer.go
func SummarizerPrompt(_ models.ModelProvider) string {
    return `You are a helpful AI assistant tasked with summarizing conversations.

When asked to summarize, provide a detailed but concise summary of the conversation.
Focus on information that would be helpful for continuing the conversation, including:
- What was done
- What is currently being worked on
- Which files are being modified
- What needs to be done next

Your summary should be comprehensive enough to provide context but concise enough
to be quickly understood.`
}
```

The in-session trigger prompt is:
```
"Provide a detailed but concise summary of our conversation above. Focus on
information that would be helpful for continuing the conversation, including
what we did, what we're doing, which files we're working on, and what we're
going to do next."
```

## Session Persistence

### SQLite Storage

All conversations are persisted to SQLite, stored in `.opencode/` within the project directory:

```
.opencode/
└── opencode.db          # SQLite database
```

The database schema (managed by Goose migrations) includes:

**Sessions table**:
```sql
CREATE TABLE sessions (
    id TEXT PRIMARY KEY,
    parent_session_id TEXT,
    title TEXT NOT NULL DEFAULT '',
    message_count INTEGER NOT NULL DEFAULT 0,
    prompt_tokens INTEGER NOT NULL DEFAULT 0,
    completion_tokens INTEGER NOT NULL DEFAULT 0,
    summary_message_id TEXT,
    cost REAL NOT NULL DEFAULT 0,
    created_at INTEGER NOT NULL DEFAULT (unixepoch()),
    updated_at INTEGER NOT NULL DEFAULT (unixepoch())
);
```

**Messages table**:
```sql
CREATE TABLE messages (
    id TEXT PRIMARY KEY,
    session_id TEXT NOT NULL REFERENCES sessions(id) ON DELETE CASCADE,
    role TEXT NOT NULL,
    parts TEXT NOT NULL,       -- JSON-serialized ContentPart array
    model TEXT,
    finished_at INTEGER,
    created_at INTEGER NOT NULL DEFAULT (unixepoch()),
    updated_at INTEGER NOT NULL DEFAULT (unixepoch())
);
```

**Files and file_versions tables** (for undo/redo):
```sql
CREATE TABLE files (
    id TEXT PRIMARY KEY,
    session_id TEXT NOT NULL,
    path TEXT NOT NULL,
    content TEXT NOT NULL DEFAULT '',
    -- ...
);

CREATE TABLE file_versions (
    id TEXT PRIMARY KEY,
    file_id TEXT NOT NULL REFERENCES files(id),
    content TEXT NOT NULL DEFAULT '',
    -- ...
);
```

### Message Serialization

Messages use a typed wrapper pattern for serializing heterogeneous content parts to JSON:

```go
type partWrapper struct {
    Type partType    `json:"type"`
    Data ContentPart `json:"data"`
}
```

Content part types:
- `reasoning` — Extended thinking/reasoning content
- `text` — Plain text content
- `image_url` — Image URL reference
- `binary` — Binary data (images, attachments)
- `tool_call` — Tool invocation with parameters
- `tool_result` — Tool execution result
- `finish` — Finish marker with reason and timestamp

### Session Types

OpenCode has several session types:

| Type | Created By | Purpose |
|------|------------|---------|
| Regular | User (new chat) | Main conversation sessions |
| Task | Agent tool | Sub-agent search sessions |
| Title | Title generator | Ephemeral title generation sessions |
| Summary | Compact command | Sessions created from summarization |

Task sessions use `parent_session_id` to maintain the parent-child relationship, enabling cost rollup from sub-agents to the parent session.

## Memory System: OpenCode.md

OpenCode supports a project-level memory file called `OpenCode.md` (or `AGENTS.md` in newer versions):

```
# Memory
If the current working directory contains a file called OpenCode.md,
it will be automatically added to your context. This file serves
multiple purposes:
1. Storing frequently used bash commands (build, test, lint, etc.)
2. Recording the user's code style preferences
3. Maintaining useful information about the codebase structure
```

This file is loaded into the system prompt context automatically, providing persistent project-specific instructions across sessions. The `/init` command analyzes the project and creates this file.

## Context Usage Patterns

### What's Sent to the LLM

On each turn, the agent sends:
1. **System prompt**: Provider-specific base prompt + environment info + LSP info + OpenCode.md content
2. **Message history**: All messages since session start (or summary point)
3. **Tool definitions**: JSON Schema for all available tools

### No Client-Side Token Counting

OpenCode does **not** perform client-side token counting or estimation. It relies on:
1. The provider's reported usage (via `TokenUsage`) after each response
2. The model's context window metadata for auto-compact thresholds
3. Provider-side errors when context is exceeded

This is simpler than approaches like tiktoken-based counting but means context overflow is only detected after it happens (provider returns an error).

### Environment Info Injection

The system prompt includes dynamic environment information:

```go
func getEnvironmentInfo() string {
    cwd := config.WorkingDirectory()
    isGit := isGitRepo(cwd)
    platform := runtime.GOOS
    date := time.Now().Format("1/2/2006")
    ls := tools.NewLsTool()
    r, _ := ls.Run(context.Background(), tools.ToolCall{Input: `{"path":"."}`})
    return fmt.Sprintf(`
<env>
Working directory: %s
Is directory a git repo: %s
Platform: %s
Today's date: %s
</env>
<project>
%s
</project>`, cwd, boolToYesNo(isGit), platform, date, r.Content)
}
```

This gives the LLM immediate awareness of:
- Current working directory
- Whether it's a git repo
- Operating system platform
- Today's date
- Directory listing of the project root

## Comparison with Other Agents

| Feature | OpenCode | Claude Code | Aider |
|---------|----------|-------------|-------|
| Token counting | Server-reported only | Client + server | tiktoken |
| Context compaction | Summarize → re-role | Compact (summary) | Map/refine |
| Persistence | SQLite per-project | JSON files | Git-based |
| Memory file | OpenCode.md | CLAUDE.md | .aider.conf.yml |
| Auto-compact trigger | 95% of context window | Configurable | Manual |
| Message truncation | After summary only | Sliding window | By cost |
