# Agent Streaming Comparison

Comprehensive cross-agent comparison of streaming implementations across the major
open-source and commercial coding agents. This document examines protocol choices,
TUI frameworks, parsing strategies, rendering pipelines, error recovery mechanisms,
and architectural patterns that define how each agent handles real-time LLM output.

---

## 1. Protocol Comparison

How each agent receives streamed tokens from LLM providers.

| Agent | Primary Protocol | Transport Layer | Bidirectional? | Notes |
|-------|------------------|-----------------|----------------|-------|
| **OpenCode** | SSE | Go HTTP client → channels | No | Provider layer emits unified ProviderEvent types |
| **Claude Code** | SSE | Anthropic Messages API | No | Content block streaming with start/delta/stop lifecycle |
| **Codex** | SSE + WebSocket | Responses API | Yes (realtime) | Only supports OpenAI Responses API; WebSocket for realtime mode |
| **Gemini CLI** | SSE | client.ts handler | No | Separate core and CLI packages; contentGenerator.ts bridges them |
| **Goose** | Async stream | `BoxStream<AgentEvent>` | No | Rust async streams yielding (Message, Usage) tuples |
| **Aider** | SSE (via litellm) | litellm abstraction | No | Provider-agnostic through litellm; supports 100+ providers |
| **Warp** | N/A (terminal-native) | PTY buffer | N/A | GPU-accelerated terminal, not an API client |
| **Pi Coding Agent** | Provider-specific | Event streaming | No | Extension-based providers with tool:start/tool:complete events |
| **OpenHands** | WebSocket/REST | EventStream | Yes | Append-only event bus with full replay capability |
| **Droid** | WebSocket | Interface-agnostic | Yes | Multi-frontend architecture (Slack, web, CLI) |
| **Ante** | HTTP streaming | Rust native | No | Lock-free scheduler for concurrent sub-agent streaming |
| **Junie CLI** | WebSocket/HTTP | JetBrains backend | Yes | Connects to JetBrains IDE backend for tool execution |
| **ForgeCode** | ZSH-native | Shell integration | N/A | `:` prefix commands in ZSH; no separate streaming layer |
| **mini-SWE-agent** | None (no streaming) | subprocess.run | N/A | ~350 lines total; synchronous request/response only |

### Protocol Details

- **SSE (Server-Sent Events)**: The dominant protocol. Unidirectional server→client text
  stream over HTTP. Each event is a `data:` line followed by `\n\n`. Most providers
  (Anthropic, OpenAI completions, Google) support this natively.

- **WebSocket**: Used by Codex for OpenAI Realtime API and by OpenHands for its
  event-driven architecture. Enables bidirectional communication but adds complexity
  in reconnection and state management.

- **Async Streams**: Goose and Ante use Rust's native async streaming (`Stream` trait)
  rather than HTTP-level protocols. The provider adapter handles HTTP internally and
  exposes a typed async iterator.

---

## 2. TUI Framework Comparison

The terminal user interface technology each agent uses for rendering streamed output.

| Agent | TUI Framework | Language | Rendering Model | Rich UI Level |
|-------|---------------|----------|-----------------|---------------|
| **Claude Code** | Ink (React) | TypeScript | Virtual DOM diff | High |
| **Gemini CLI** | Ink (React) | TypeScript | Virtual DOM diff | High |
| **OpenCode** | Bubble Tea | Go | Elm Architecture (MVU) | Medium-High |
| **Codex** | Ratatui | Rust | Immediate-mode widgets | Medium |
| **Warp** | Custom (Metal GPU) | Rust | Flutter-style element tree | Very High |
| **Pi Coding Agent** | Custom (pi-tui) | TypeScript | Differential rendering | High |
| **Aider** | prompt_toolkit | Python | Input/output library | Low-Medium |
| **Goose** | Event-driven UI | Rust | Progressive rendering | Medium |
| **OpenHands** | Web UI | TypeScript | React (browser) | Very High |
| **Droid** | Multi-frontend | Various | Interface-agnostic | Varies |
| **ForgeCode** | ZSH shell | Shell | No custom rendering | Minimal |
| **mini-SWE-agent** | None (raw print) | Python | print() | Minimal |

### Framework Architecture Notes

**Ink (React for CLI)**: Claude Code and Gemini CLI both use Ink, which brings
React's component model to the terminal. Components like `<Box>`, `<Text>`, and
custom elements render via virtual DOM diffing. This enables complex layouts
(permission dialogs, multi-pane views) but adds startup overhead (~200ms).

**Bubble Tea (Elm Architecture)**: OpenCode uses the MVU pattern: Model holds state,
Update processes messages, View renders. Messages arrive as `tea.Msg` from the
streaming pub/sub system. Well-suited for Go's concurrency model.

**Ratatui (Immediate Mode)**: Codex renders the entire frame each tick. Widgets
describe what to draw; the framework handles diffing against the terminal buffer.
Efficient for Rust's ownership model—no retained widget tree to manage.

**Metal GPU Rendering**: Warp bypasses traditional terminal rendering entirely.
Text is rendered via Metal shaders at ~1.9ms per frame. This enables true rich
rendering: inline images, animated progress bars, multi-font layouts.

---

## 3. Streaming Parsing Comparison

How each agent parses incoming stream chunks into structured data.

| Agent | Parsing Strategy | Provider Normalization | JSON Assembly | Tool Call Handling |
|-------|------------------|------------------------|---------------|-------------------|
| **OpenCode** | Delta events → SQLite → pub/sub | ProviderEvent types (EventContentDelta, EventToolUseStart, etc.) | String accumulation, parse at EventComplete | Per-tool entry tracking by ID |
| **Claude Code** | Content block lifecycle | Anthropic-only (native format) | Buffer until content_block_stop | Anthropic block lifecycle events |
| **Codex** | Item lifecycle (Responses API) | OpenAI Responses API only | ItemCreated → ItemUpdated → ItemCompleted | ToolRouter.build_tool_call() dispatches |
| **Gemini CLI** | contentGenerator.ts pipeline | Gemini format normalization | Provider-specific parsing in core | Gemini function call format |
| **Goose** | Stream chunks → categorize_tools() | stream_response_from_provider() yields (Message, Usage) | Accumulate strings, coerce types | categorize_tools() splits frontend/backend |
| **Aider** | Edit format parsers | litellm normalizes to OpenAI format | litellm handles JSON assembly | Edit format routing (diff/whole/udiff) |
| **Pi** | Event streaming API | Extension-based providers | Provider-specific in extensions | tool:start, tool:complete lifecycle events |
| **Ante** | fn stream() → TokenStream | HTTP streaming in Rust | Rust native serde parsing | Lock-free per-sub-agent dispatch |

### Parsing Deep Dive

**Delta Accumulation**: Most agents accumulate text deltas into a growing string buffer.
The critical question is when to parse. OpenCode parses tool call JSON only at
`EventComplete`, avoiding partial-JSON errors. Codex uses `ItemCompleted` similarly.

**Tool Call JSON Assembly**: Tool calls arrive as partial JSON fragments across
multiple SSE events. Example sequence:
```
data: {"type":"tool_use","id":"tool_1","name":"read_file","input":""}
data: {"type":"input_json_delta","partial_json":"{\"path\":"}
data: {"type":"input_json_delta","partial_json":"\"src/main.rs\""}
data: {"type":"input_json_delta","partial_json":"}"}
data: {"type":"tool_use_stop"}
```
Agents must accumulate these fragments and parse the complete JSON only after the
stop event. Premature parsing causes `SyntaxError` on partial JSON.

**Type Coercion** (Goose): When a tool schema expects `integer` but the LLM produces
`"42"` (string), Goose's argument coercion layer automatically converts. This handles
a common LLM failure mode where numbers are quoted.

---

## 4. Rendering Approach Comparison

How streamed tokens become visible output in the user's terminal.

| Agent | Markdown Rendering | Syntax Highlighting | Diff Display | Progress Indicators |
|-------|-------------------|---------------------|--------------|---------------------|
| **Claude Code** | Ink component pipeline | Within Ink components | Colored inline diffs | Custom spinner verbs, iTerm2 progress bar |
| **OpenCode** | Lip Gloss styled | Via Bubble Tea components | Styled diff view with line numbers | Spinner + format utilities |
| **Codex** | Ratatui widgets | Within TUI cells | TUI-based side-by-side diff | Turn lifecycle events as status |
| **Gemini CLI** | Ink components | Ink component tree | Component-based diff rendering | Standard Ink spinner patterns |
| **Warp** | GPU-rendered multi-font | GPU syntax highlighting shaders | Rich diff views with color blending | GPU-animated progress bars |
| **Aider** | Full markdown via Rich/io.py | Color-coded terminal output | Git-based diff display (unified) | prompt_toolkit bottom toolbar |
| **Goose** | Event-driven progressive | Provider-dependent rendering | Event-based diff display | CancellationToken-based spinners |
| **Pi** | Custom pi-tui differential | Extension-dependent | Component-based diff views | Extension event progress indicators |
| **OpenHands** | Web React components | Monaco editor highlighting | Web diff viewer (side-by-side) | Event-driven web progress bars |

### Rendering Pipeline Details

**Claude Code Pipeline**:
1. SSE event arrives → content block delta
2. Text accumulated in conversation state
3. React (Ink) component re-renders with new text
4. Ink diffs virtual DOM → minimal terminal escape sequences
5. Markdown parsed inline; code blocks get syntax highlighting

**OpenCode Pipeline**:
1. Provider emits `EventContentDelta`
2. Persisted to SQLite row
3. `Broker[T]` publishes to subscribers
4. Bubble Tea model receives `tea.Msg`
5. View function renders with Lip Gloss styling
6. Catppuccin theme applied to code blocks

**Warp Pipeline**:
1. PTY output captured in Block grid buffer
2. Element tree updated (Flutter-style layout)
3. Metal shader compiles glyph atlases
4. GPU renders at ~1.9ms including syntax highlighting
5. Inline images, buttons rendered as GPU textures

---

## 5. Error Recovery Comparison

How each agent handles failures during streaming.

| Agent | Max Retries | Reconnection | Partial Response | Rate Limit | User Interrupt | Unique Feature |
|-------|-------------|--------------|------------------|------------|----------------|----------------|
| **Codex** | 5 (stream) / 4 (request) | Retry from scratch | Discard + retry | Provider-level 429 handling | Op::Interrupt → TurnAborted | 300s idle timeout auto-abort |
| **OpenCode** | 8 | Retry from scratch | Persist to SQLite (even partial) | Provider layer handles | Context cancellation cascade | 4-level cancellation hierarchy |
| **Claude Code** | Provider SDK default | Provider SDK retry logic | Checkpoint snapshots preserved | SDK handles transparently | Esc preserves context | File snapshot rewind menu |
| **Goose** | 2 (compaction) | Break + preserve state | Emergency compaction triggers | CreditsExhausted with top_up_url | CancellationToken propagation | compact_messages() for overflow |
| **Gemini CLI** | Configurable | Retry + fallback module | Discard + retry fresh | Fallback to alternative model | Standard Ink interrupt | Fallback model routing |
| **Aider** | Via litellm defaults | litellm handles reconnection | Discard + retry | litellm transparent retry | Standard SIGINT | Git-based undo for edits |
| **OpenHands** | Event replay | EventStream persistent | Full state reconstruction | Backend handles | Web UI cancel button | ReplayManager for recovery |
| **Ante** | Sub-agent isolation | Per sub-agent retry | Sub-agents independent | Per sub-agent limits | Lock-free scheduler abort | Sub-agent fault isolation |
| **Pi** | Extension-based | Extension handles retry | Extension-dependent | Extension handles | Enter = steer, not cancel | Mid-stream steering |

### Error Recovery Deep Dive

**OpenCode's 4-Level Cancellation**:
1. **Session level**: Cancels everything, closes session
2. **Loop level**: Cancels current agent loop iteration
3. **Stream level**: Cancels active SSE connection, preserves loop
4. **Tool level**: Cancels running tool execution only

Each level has its own `context.Context` with proper parent-child relationships.
Cancelling a parent automatically cancels all children.

**Claude Code's File Snapshots**:
Before every file edit, Claude Code captures a snapshot. If the user presses
Esc twice, they get a menu to rewind any or all file changes. This is a
coarse-grained but highly reliable recovery mechanism.

**Goose's Emergency Compaction**:
When context exceeds the model's window, Goose triggers `compact_messages()`.
This summarizes the conversation history into a shorter form, preserving
critical context while freeing token budget. Limited to 2 retries before
surfacing the error to the user.

**Codex's Idle Timeout**:
If the LLM stops producing tokens for 300 seconds, Codex automatically
aborts the turn. This prevents hung connections from blocking the user
indefinitely. The `Op::Interrupt` signal cleanly transitions to `TurnAborted`.

---

## 6. Detailed Agent Profiles

### OpenCode (Go)

- **Protocol**: SSE via Go HTTP client with goroutine-per-stream
- **Parsing**: Provider layer emits unified `ProviderEvent` types regardless of upstream
  - `EventContentDelta` — text token arrived
  - `EventToolUseStart` — tool call begins (includes name, ID)
  - `EventToolUseStop` — tool call JSON complete
  - `EventThinkingDelta` — reasoning/thinking token
  - `EventComplete` — stream finished, includes usage stats
- **State**: Every delta persisted to SQLite immediately via `store` package
- **Pub/Sub**: Generic typed `Broker[T]` system publishes events to multiple subscribers
- **TUI**: Bubble Tea receives events as `tea.Msg` via subscription
- **Rendering**: Lip Gloss styling with Catppuccin Mocha/Latte themes
- **Images**: Inline image rendering via Kitty/iTerm2 protocols
- **Error Recovery**: 8 retries with exponential backoff; 4-level context cancellation

### Claude Code (TypeScript)

- **Protocol**: SSE via Anthropic Messages API (`@anthropic-ai/sdk`)
- **Parsing**: Content block start → delta → stop lifecycle
  - `content_block_start` — new text or tool_use block
  - `content_block_delta` — incremental text or JSON fragment
  - `content_block_stop` — block complete
  - `message_stop` — entire message complete
- **State**: Conversation state held in-memory; file snapshots on disk
- **TUI**: Full Ink (React) application with JSX components
- **Rendering**: Rich component tree: diffs, spinners, permission dialogs, markdown
- **Thinking**: Extended thinking blocks toggleable with Ctrl+O
- **Error Recovery**: File snapshots before edits; Esc+Esc for rewind menu
- **Unique**: Multi-turn tool use with automatic permission escalation

### Codex (Rust)

- **Protocol**: SSE + WebSocket via OpenAI Responses API exclusively
- **Parsing**: Item-based lifecycle
  - `ItemCreated` — new response item (text, tool call, etc.)
  - `ItemUpdated` — delta for existing item
  - `ItemCompleted` — item fully received
- **Architecture**: SQ (Submission Queue) / EQ (Event Queue) separation
  - SQ: User and system submit operations
  - EQ: Events flow back (turn updates, tool results)
- **TUI**: Ratatui (one of multiple frontends)
- **Other Frontends**: exec mode (JSONL stdout), app-server (IDE integration), MCP server
- **Tool Routing**: `ToolRouter` maps items to MCP servers, built-in handlers, local shell
- **Error Recovery**: 5 stream retries, 4 request retries, 300s idle timeout
- **Unique**: `Op::Interrupt` signal for clean abort mid-stream

### Goose (Rust)

- **Protocol**: Async `BoxStream<AgentEvent>` from provider adapters
- **Parsing**: `stream_response_from_provider()` yields `(Option<Message>, Option<ProviderUsage>)`
- **Tool Routing**: `categorize_tools()` splits tools into:
  - **Frontend tools**: UI-facing (display, prompt user)
  - **Backend tools**: System-facing (file ops, shell commands)
- **Argument Coercion**: Converts `"42"` string to `42` number when schema expects integer
- **Context Management**: `compact_messages()` for emergency context window overflow
- **Error Recovery**: CancellationToken propagation, CreditsExhausted with `top_up_url`
- **Unique**: Built-in `platform` tool for cross-platform system interaction

### Gemini CLI (TypeScript)

- **Architecture**: Monorepo with `packages/core/` (UI-agnostic) and `packages/cli/` (Ink UI)
- **Parsing**: `contentGenerator.ts` bridges the agent loop and LLM client
- **Headless Modes**: Three output formats for non-interactive use:
  - `text` — human-readable plain text
  - `json` — structured JSON output
  - `stream-json` — newline-delimited JSON (NDJSON) for piping
- **Model Fallback**: Retry logic with fallback module for routing to alternative models
- **Error Recovery**: Configurable retry count; automatic model fallback on failure
- **Unique**: First-party Google integration with Gemini-specific optimizations

### Aider (Python)

- **Protocol**: litellm normalizes SSE across 100+ LLM providers
- **Parsing**: Edit format parsers handle multiple output styles:
  - `EditBlockCoder` — search/replace blocks
  - `WholeFileCoder` — complete file replacement
  - `UnifiedDiffCoder` — unified diff format
  - `ArchitectCoder` — two-model pipeline (architect + editor)
- **TUI**: prompt_toolkit for rich input; Rich library for formatted output
- **Voice Input**: `--voice` flag activates OpenAI Whisper for speech-to-code
- **Model Detection**: Automatically detects reasoning models → routes to ArchitectCoder
- **Error Recovery**: Git-based undo; every edit creates a commit for easy revert
- **Unique**: Repository map via tree-sitter for intelligent context selection

### Warp (Rust + Metal)

- **Architecture**: GPU-accelerated terminal application (not an LLM API client)
- **Rendering**: Metal shaders render text at ~1.9ms per frame
  - Glyph atlas caching for font rendering
  - Multi-font support within single view
  - Flutter-inspired element tree for layout
- **Block Model**: Each command + output is a discrete `Block` with its own grid
  - Blocks are individually selectable, copyable, shareable
  - AI can analyze individual block output
- **AI Features**:
  - Proactive error detection on command failure
  - Natural language → command translation
  - Conversation view for multi-turn interaction
- **Unique**: Only agent that renders via GPU; diffs, progress bars, buttons all GPU-rendered

### OpenHands (TypeScript + Python)

- **Architecture**: Full-stack web application with event-driven backend
- **Protocol**: WebSocket for real-time events; REST for state queries
- **EventStream**: Append-only event bus — the core abstraction
  - All actions and observations are events
  - Events are immutable once appended
  - Full replay from any point via `ReplayManager`
- **State**: Complete state reconstructible from event log
- **UI**: React web application with Monaco editor integration
- **Error Recovery**: Full state reconstruction from event replay
- **Unique**: Most robust recovery model; can resume from any failure point

### Pi Coding Agent (TypeScript)

- **Architecture**: Extension-based with `pi-tui` custom terminal UI
- **Providers**: Implemented as extensions, each with own streaming protocol
- **Tool Lifecycle**: `tool:start` → progress events → `tool:complete`
- **Rendering**: Differential rendering — only updates changed terminal regions
- **Interrupt Model**: Enter key steers the agent mid-stream rather than cancelling
- **Unique**: Mid-stream steering lets users redirect without losing context

---

## 7. Key Architectural Patterns

### Provider Normalization Spectrum

```
Full Normalization          Partial Normalization          Single Provider
     │                            │                             │
  OpenCode                     Goose                      Claude Code
  Aider/litellm             Gemini CLI                      Codex
                                Pi
```

- **Full normalization** (OpenCode, Aider/litellm): All providers emit identical event
  types. Adding a new provider requires only implementing the adapter interface. Consumer
  code is completely provider-agnostic.

- **Partial normalization** (Goose, Gemini CLI): Stream handler abstracts away most
  provider differences, but some provider-specific behavior leaks through. Adding
  providers requires moderate effort.

- **Single provider** (Codex, Claude Code): Tightly coupled to one API format. No
  normalization layer exists. Maximum optimization for that provider's features, but
  zero portability.

### State Management During Streaming

| Strategy | Agent(s) | Durability | Recovery | Performance |
|----------|----------|------------|----------|-------------|
| SQLite persistence | OpenCode | Survives crashes | Full replay | ~1ms write overhead |
| In-memory | Claude Code, Codex | Lost on crash | None (restart) | Zero overhead |
| Append-only event log | OpenHands | Survives crashes | Full replay | Append-only writes |
| Checkpoint files | Claude Code | File-level snapshots | Rewind to checkpoint | File I/O per edit |
| Git commits | Aider | Full history | Git revert | Git overhead per edit |

### Tool Call Architecture Patterns

- **Inline execution** (most agents): Tool calls execute immediately as they complete
  during the stream. Simple but can block the stream if tool execution is slow.

- **Categorized dispatch** (Goose): `categorize_tools()` routes to frontend (UI) or
  backend (system) handlers. Frontend tools can update the UI while backend tools
  execute silently.

- **Queued execution** (Codex): SQ/EQ model separates submission from execution.
  Operations are queued and processed in order, enabling clean cancellation and
  prioritization.

- **Extension-based** (Pi): Tools implemented as extensions with their own streaming
  event lifecycle. Each extension manages its own state and error handling.

- **Lock-free scheduling** (Ante): Sub-agents run concurrently with lock-free
  coordination. Each sub-agent has independent streaming and tool execution.

### Cancellation Patterns

```
Simple SIGINT              Structured Cancellation         Event-Driven
     │                            │                             │
   Aider                      OpenCode                     OpenHands
  ForgeCode                     Codex                        Goose
 mini-SWE-agent              Claude Code                      Pi
```

- **Simple SIGINT**: Standard Unix signal handling. Process stops; state may be lost.
- **Structured**: Hierarchical cancellation with context propagation. Parent cancels children.
- **Event-Driven**: Cancellation is itself an event in the stream. Observers react accordingly.

---

## 8. Summary Matrix

| Agent | Language | Protocol | TUI | Provider Support | State Persistence | Error Recovery | Complexity |
|-------|----------|----------|-----|------------------|-------------------|----------------|------------|
| **OpenCode** | Go | SSE | Bubble Tea | Multi (normalized) | SQLite | 8 retries, 4-level cancel | High |
| **Claude Code** | TypeScript | SSE | Ink (React) | Anthropic only | Memory + snapshots | SDK retry + file rewind | High |
| **Codex** | Rust | SSE+WS | Ratatui | OpenAI only | Memory | 5+4 retries, idle timeout | Very High |
| **Gemini CLI** | TypeScript | SSE | Ink (React) | Google only | Memory | Retry + model fallback | Medium |
| **Goose** | Rust | Async stream | Event UI | Multi (partial) | Memory | Cancel token + compaction | High |
| **Aider** | Python | SSE/litellm | prompt_toolkit | 100+ via litellm | Git commits | litellm retry + git undo | Medium |
| **Warp** | Rust | PTY | Metal GPU | N/A (terminal) | Block grid | N/A | Very High |
| **Pi** | TypeScript | Extension | pi-tui | Extension-based | Extension-based | Extension-based | Medium |
| **OpenHands** | TS+Python | WebSocket | Web React | Multi | Event log | Full replay | Very High |
| **Droid** | Various | WebSocket | Multi-frontend | Multi | Interface-dependent | Interface-dependent | High |
| **Ante** | Rust | HTTP stream | Minimal | Multi | Sub-agent scoped | Sub-agent isolation | Medium |
| **ForgeCode** | Shell | ZSH native | ZSH | Shell-based | None | None | Low |
| **mini-SWE-agent** | Python | None | None | Single | None | None | Minimal |

---

## 9. Key Takeaways

1. **SSE dominates**: Nearly every agent uses Server-Sent Events as the primary streaming
   protocol. WebSocket is used only when bidirectional communication is essential
   (OpenHands, Codex realtime mode).

2. **Ink (React) is the most popular TUI**: Claude Code and Gemini CLI both chose Ink,
   suggesting the React component model maps well to complex terminal UIs.

3. **Provider normalization is a spectrum**: Agents that support multiple providers
   (OpenCode, Aider) invest heavily in normalization layers. Single-provider agents
   (Claude Code, Codex) skip this entirely for tighter integration.

4. **Error recovery correlates with complexity**: The most sophisticated agents (OpenCode,
   Codex, OpenHands) have the most elaborate recovery mechanisms. Simpler agents rely
   on external tools (git, litellm) for recovery.

5. **State persistence varies wildly**: From SQLite (OpenCode) to append-only logs
   (OpenHands) to nothing at all (mini-SWE-agent). The choice reflects each agent's
   reliability requirements.

6. **Tool call handling is the hardest streaming problem**: Assembling partial JSON,
   routing to correct handlers, managing concurrent tool execution, and handling
   failures mid-tool-call are the most complex aspects of streaming implementation.

7. **GPU rendering is an outlier**: Warp's Metal-based approach is fundamentally
   different from every other agent. It achieves rendering performance impossible
   with traditional terminal escape sequences but at enormous implementation cost.
