---
title: Streaming
status: draft
---

# Streaming

Streaming is the connective tissue of every coding agent — it bridges the gap between an LLM generating tokens at ~50-100 tok/s and a user expecting responsive, flicker-free output. This document traces the full path: **protocol** (how bytes arrive), **parsing** (how they become structured data), **rendering** (how users see them), and **error recovery** (what happens when things break).

---

## Protocols

LLM API responses can take 5-60+ seconds to fully generate. Streaming protocols let agents begin processing tokens as they arrive rather than waiting for completion.

### Server-Sent Events (SSE)

SSE is the **dominant protocol** across coding agents. Every major provider (OpenAI, Anthropic, Google, together.ai) supports it.

**How it works:**
- Client sends a standard HTTP POST with `Accept: text/event-stream`
- Server holds the connection open, sending newline-delimited events
- Each event follows a simple text format: `data: {json}\n\n`
- The stream terminates with `data: [DONE]\n\n`

**Format example (OpenAI Chat Completions):**
```
data: {"id":"chatcmpl-abc","choices":[{"delta":{"content":"Hello"}}]}

data: {"id":"chatcmpl-abc","choices":[{"delta":{"content":" world"}}]}

data: [DONE]
```

**Why SSE dominates:**
- Works over standard HTTP/1.1 — no protocol upgrade needed
- Unidirectional (server → client) matches the LLM streaming model perfectly
- Firewalls and proxies handle it transparently
- Built-in reconnection semantics via `Last-Event-ID`
- Every HTTP client library supports it

**Limitations:**
- Unidirectional only — client cannot send data mid-stream (must open new request)
- No binary framing — everything is UTF-8 text
- No built-in backpressure — fast producers can overwhelm slow consumers
- Connection limits per domain in browsers (6 per domain in HTTP/1.1)

**Agents using SSE:** OpenCode (via Go channels from provider SDKs), Claude Code (Anthropic Messages API), Gemini CLI (`client.ts` SSE handler), Codex (Responses API), Goose (via provider abstraction), Junie CLI (to JetBrains backend), Ante (cloud API calls).

### Chunked Transfer Encoding

The underlying HTTP mechanism that enables SSE. With `Transfer-Encoding: chunked`, the server sends response body in pieces without knowing the total content length upfront.

- SSE rides on top of chunked encoding — they are complementary, not alternatives
- Some agents use raw chunked responses without SSE framing when talking to local models (e.g., Ollama's `/api/generate` endpoint returns newline-delimited JSON chunks)
- Aider via litellm handles both SSE and raw chunked responses transparently depending on the provider

### WebSockets

Bidirectional communication over a single TCP connection. Used when agents need **two-way streaming** — sending and receiving simultaneously.

**When WebSockets make sense:**
- **Realtime conversation mode** — Codex explicitly supports WebSocket for its realtime API (`supports_websockets` per-provider flag)
- **IDE integration** — OpenHands uses WebSocket as its primary server↔UI transport, with events flowing both directions through the `EventStream`
- **Persistent sessions** — Droid's interface-agnostic protocol uses WebSocket for web/Slack/IDE frontends that need push notifications

**Trade-offs vs SSE:**

| Aspect | SSE | WebSocket |
|--------|-----|-----------|
| Direction | Server → Client | Bidirectional |
| Protocol | HTTP/1.1 | Upgrade from HTTP |
| Reconnection | Built-in | Manual |
| Proxy support | Excellent | Sometimes problematic |
| Overhead | Minimal (text) | Minimal (binary frames) |
| Use case | LLM token streaming | Realtime/interactive |

Most agents use SSE for LLM streaming and reserve WebSockets for UI transport when a web frontend or IDE plugin needs bidirectional communication.

### OpenAI Responses API Streaming

Codex CLI is the clearest example of OpenAI's newer **Responses API** (`/v1/responses`), which replaces Chat Completions with a richer event model:

```
ResponseCreated { id }
ItemCreated(ResponseItem)      // new content item started
ItemUpdated(ResponseItem)      // delta within an item
ItemCompleted(ResponseItem)    // item finalized
ResponseCompleted { usage }    // stream done
Error { message }              // stream error
```

Key differences from Chat Completions streaming:
- **Item-level granularity** — responses contain multiple typed items (`Message`, `LocalShellCall`, `FunctionCall`, `Reasoning`, `WebSearchCall`) rather than a flat `choices[].delta`
- **Built-in tool call lifecycle** — tool calls have explicit created/updated/completed events
- **Native reasoning tokens** — `Reasoning` items stream extended thinking separately
- Codex fully committed to this API, dropping Chat Completions entirely

### Protocol Selection Guide

| Scenario | Best Protocol | Example |
|----------|---------------|---------|
| Standard LLM token streaming | SSE | Most agents |
| Local model (Ollama, llama.cpp) | Chunked JSON lines | Aider, OpenCode |
| Bidirectional realtime | WebSocket | Codex realtime mode |
| Web UI ↔ backend | WebSocket | OpenHands, Droid |
| IDE plugin ↔ agent | WebSocket or stdio | Junie, Droid |
| MCP server transport | stdio or SSE | Ante, Codex, Goose |

---

## Parsing

Raw stream chunks must be assembled into structured data — text content, tool calls, thinking tokens. This is harder than it sounds because chunks arrive at arbitrary boundaries.

### Delta vs Snapshot Streaming Models

The two major providers use fundamentally different streaming approaches:

**OpenAI — Delta model:**
- Each chunk contains only the **change** from the previous state
- `choices[0].delta.content` = new text to append
- Tool calls arrive as `tool_calls[i].function.arguments` delta strings — partial JSON fragments
- Agent must maintain running state and apply deltas incrementally

**Anthropic — Content block model:**
- Events structured around content blocks: `content_block_start`, `content_block_delta`, `content_block_stop`
- Each block has a type (`text`, `tool_use`, `thinking`) and an index
- Deltas arrive within block context, making it clear what they belong to
- `message_start` → `content_block_start` → N × `content_block_delta` → `content_block_stop` → `message_stop`

**OpenAI Responses API — Item model (Codex):**
- `ItemCreated` → N × `ItemUpdated` → `ItemCompleted`
- Each item is typed (`Message`, `FunctionCall`, `LocalShellCall`, `Reasoning`)
- Richer lifecycle than Chat Completions deltas

**How agents normalize these differences:**

| Agent | Strategy |
|-------|----------|
| **OpenCode** | Provider layer emits unified `ProviderEvent` types (`EventContentDelta`, `EventToolUseStart`, etc.) regardless of upstream provider |
| **Aider** | litellm normalizes across providers; Coder subclass parses the edit format from the final response |
| **Goose** | `stream_response_from_provider()` yields `(Option<Message>, Option<ProviderUsage>)` pairs — provider differences hidden |
| **Codex** | Only supports Responses API — no normalization needed |
| **Gemini CLI** | `contentGenerator.ts` bridges agent loop and LLM client, normalizing Gemini's response format |

### Incremental JSON Parsing

Tool call arguments arrive as partial JSON strings across multiple chunks. A typical sequence:

```
Chunk 1: {"function": {"arguments": "{\"path\": \"/sr"}}
Chunk 2: {"function": {"arguments": "c/main.rs\", \"co"}}
Chunk 3: {"function": {"arguments": "ntent\": \"fn main()\"}"}}
```

The agent must concatenate argument fragments and parse only when complete. Strategies:

1. **String accumulation** (most common): Buffer argument string fragments, attempt `JSON.parse()` / `serde_json::from_str()` only after `ItemCompleted` or `content_block_stop`. Used by OpenCode, Goose, Codex.

2. **Streaming JSON parser**: Libraries like `ijson` (Python) or `serde_json::StreamDeserializer` (Rust) can parse incrementally. Rarely used in practice because tool call JSON is small enough that buffering is simpler.

3. **Argument coercion**: Even after successful parsing, the JSON may not match the schema. Goose runs a coercion pass — converting `"42"` (string) to `42` (number) when the schema expects an integer — to handle common LLM mistakes.

### Partial Tool Call Assembly

Tool calls are the most complex streaming artifact to assemble. The lifecycle varies by provider:

**OpenCode's approach (Go channels):**
```
EventToolUseStart  →  creates new tool call entry on message
  (stream continues with content deltas)
EventToolUseStop   →  finalizes tool call by ID
EventComplete      →  reconciles with SetToolCalls() as final truth
```
Each event immediately persists to SQLite and publishes via pub/sub — the TUI sees tool calls materialize in real time.

**Codex's approach (Responses API items):**
```
ItemCreated(FunctionCall)   →  tool call shell created
ItemUpdated(FunctionCall)   →  arguments accumulating
ItemCompleted(FunctionCall) →  ready for execution
```
The `ToolRouter.build_tool_call()` maps completed items to executable `ToolCall` structs, routing to MCP servers, built-in handlers, or the local shell.

**Goose's approach (categorized dispatch):**
After the stream completes, `categorize_tools()` splits tool requests into **frontend tools** (executed in the UI) and **remaining tools** (executed server-side). This two-tier routing enables UI-specific tools like confirmation dialogs.

### Thinking / Reasoning Tokens

Extended thinking (Claude) and chain-of-thought (o1/o3) introduce a separate token stream:

- **Anthropic**: `thinking` content blocks with their own `content_block_start/delta/stop` lifecycle. Claude Code toggles visibility with `Ctrl+O`.
- **OpenAI Responses API**: `Reasoning` items in the response. Codex surfaces these via `EQ::Reasoning` events.
- **OpenCode**: `EventThinkingDelta` events append to `ReasoningContent` on the message, published via the same pub/sub path as regular content.
- **Aider**: Detects reasoning-capable models in `models.py` and routes them to `ArchitectCoder` (two-model pipeline) where the reasoning model plans and a faster model edits.

Thinking tokens are typically **not billed** in the same way but consume streaming time — agents must decide whether to display them (adds latency to perceived responsiveness) or hide them (user wonders why there's a pause).

### Error Handling During Parsing

Malformed chunks and interrupted streams are common in production:

- **Truncated JSON**: If the stream terminates mid-argument, the accumulated partial JSON is discarded. OpenCode finalizes the message with `FinishReasonCanceled`.
- **Invalid UTF-8**: SSE is text-only; binary artifacts in responses (rare) cause parse failures. Agents typically skip the offending chunk.
- **Out-of-order events**: The item model (Responses API) can theoretically deliver `ItemUpdated` before `ItemCreated` during network reordering. Codex handles this by buffering updates for unknown item IDs.
- **Provider-specific quirks**: Gemini sometimes produces tool calls in non-standard formats. Aider's model-specific edit format routing (`Gemini → diff-fenced`) works around provider-specific parsing issues.

---

## Rendering

Parsing produces structured data; rendering turns it into something a human can follow in real time. This is where agents diverge most dramatically — from 350-line scripts with raw `print()` to GPU-accelerated rendering pipelines.

### Token-by-Token Display

The simplest approach: print each token as it arrives.

```python
for chunk in stream:
    print(chunk.content, end="", flush=True)
```

**Who uses it:** mini-SWE-agent (~350 lines total, no TUI), simple scripts, headless/CI modes.

**Problems:**
- Terminal flicker on fast token arrival
- Markdown formatting impossible mid-stream (you can't render `**bold**` until you've seen the closing `**`)
- No place for status indicators, progress bars, or permission dialogs
- Interleaved output if multiple streams are active

### Buffered Rendering

Accumulate tokens, then render at intervals or on semantic boundaries (sentence end, newline, code fence).

- **Pi Coding Agent**: Explicit **differential rendering** — only re-renders changed portions of the screen. "Critical for a coding agent where LLM responses stream in token-by-token." Retains a component tree and triggers targeted re-renders.
- **Aider**: Processes full responses rather than token-by-token display. The edit format parser operates on complete output, rendering diffs after full content is available.
- **Gemini CLI headless mode**: Buffers output and emits in three formats — `text` (human-readable), `JSON` (structured), `stream-json` (newline-delimited JSON events).

### TUI Frameworks

Most production agents use a TUI framework for structured terminal rendering:

#### Ink (React for CLI) — Claude Code, Gemini CLI

Ink brings React's component model to the terminal. The entire CLI is a React component tree rendered to ANSI output.

- **Architecture**: JSX/TSX components handle input, diff rendering, spinners, permission dialogs
- **Reactive updates**: State changes trigger re-renders (React reconciliation against terminal output)
- **Keyboard handling**: Component-level event handlers (`Esc` = interrupt, `Shift+Tab` = cycle modes)
- **Rich components**: Inline diffs, progress bars (native iTerm2/Windows Terminal protocols), code blocks
- **Strengths**: Familiar React mental model; composable; handles layout, state, and input elegantly
- **Weaknesses**: Node.js dependency; React reconciliation overhead; limited to what React can express

Gemini CLI uses the same framework, with `packages/cli/src/ui/` containing Ink components and a separate `packages/core/` that remains UI-agnostic.

#### Bubble Tea (Go) — OpenCode

Bubble Tea follows The Elm Architecture: `Model` (state) → `Update` (message handler) → `View` (renderer).

- **Architecture**: `internal/tui/components/` (reusable widgets), `internal/tui/page/` (chat, logs), `internal/tui/layout/`
- **Styling**: Lip Gloss for declarative style definitions
- **Streaming integration**: Services publish events via a generic typed `Broker[T]` pub/sub system. Every streaming delta hits SQLite, triggers pub/sub, and reaches the TUI as a Bubble Tea message.
- **Themes**: Catppuccin and others via `internal/tui/theme/`
- **Image rendering**: `internal/tui/image/` for inline terminal images
- **Strengths**: Single binary (Go); Elm Architecture keeps state management clean; fast startup
- **Weaknesses**: Less composable than React; custom layout logic needed; smaller ecosystem than Ink

#### Ratatui (Rust) — Codex CLI

Ratatui is a Rust TUI library providing immediate-mode rendering with widgets.

- **Architecture**: `codex-rs/tui/` consumes `EventMsg` from the Event Queue (EQ)
- **SQ/EQ separation**: TUI is just one frontend — the same EQ events power exec mode (JSONL output), app-server (IDE), and MCP server
- **Non-interactive mode**: `codex-rs/exec/` is a simpler event processor supporting human-readable, JSONL, and ephemeral output
- **Strengths**: Zero-cost abstractions; memory safety; same binary as the agent core
- **Weaknesses**: Steeper learning curve; less mature ecosystem than Ink

#### Rust + Metal GPU — Warp

Warp takes a fundamentally different approach: **GPU-accelerated rendering** via Metal shaders.

**Pipeline:** Rust Application Logic → Element Tree (Flutter-inspired) → GPU Primitives (`rect`, `image`, `glyph`) → Metal Shaders (~250 lines MSL) → Screen Output

**Why GPU rendering for a terminal?**
- **Performance**: 400+ fps, ~1.9ms average redraw — enables smooth animations, transitions, and rich UI elements
- **Rich UI elements impossible in text-grid terminals**: Inline accept/reject buttons, syntax-highlighted diff views with rich color, smooth progress bar animations, inline images, multi-font rendering
- **Agent conversation view**: Dedicated workspace with plans, diffs, task tracking — layouts that exceed what character-cell terminals can express
- **Block-based model**: Each command+output is a discrete Block with its own grid (forked from Alacritty), enabling per-block scrolling, independent rendering, and metadata overlays

The framework was co-developed with Nathan Sobo (Atom/Zed co-founder), using a Flutter-inspired widget model with declarative element tree and flexbox-inspired layout.

#### Raw ANSI / Minimal Rendering

Agents targeting simplicity or CI/headless environments skip TUI frameworks entirely:

- **mini-SWE-agent**: Raw `subprocess.run` with `stdout=subprocess.PIPE`. No streaming, no progress indicators. Completion detected via magic string sentinel.
- **ForgeCode**: ZSH-native integration with `:` prefix commands. No custom TUI — uses the shell itself.
- **Aider**: `prompt_toolkit` for rich terminal interaction — syntax highlighting, tab completion, color-coded output — but not a full TUI framework.

#### Framework Comparison

| Framework | Language | Architecture | Streaming Model | Rich UI | Complexity |
|-----------|----------|-------------|-----------------|---------|------------|
| **Ink** | TypeScript | React component tree | State → re-render | High (components) | Medium |
| **Bubble Tea** | Go | Elm Architecture | Pub/sub → messages | Medium | Medium |
| **Ratatui** | Rust | Immediate-mode widgets | EQ events → draw | Medium | Medium-High |
| **Metal GPU** | Rust | Flutter widget tree | Element tree → GPU | Very High | Very High |
| **pi-tui** | TypeScript | Custom retained-mode | Differential render | High | High (custom) |
| **prompt_toolkit** | Python | Input/output library | Full response | Low-Medium | Low |
| **Raw ANSI** | Any | print() | Token-by-token | Minimal | Minimal |

### Markdown Rendering

LLM output is typically markdown — code fences, headers, bold, lists. Rendering this live during streaming is a challenge because markdown is context-dependent (you need the closing ``` to know a code block ended).

**Approaches:**
- **Ink/React agents** (Claude Code, Gemini CLI): Markdown→component pipeline renders formatted output. Code blocks get syntax highlighting, diffs render as colored inline components.
- **Aider**: Full markdown rendering via `io.py` after response completion — syntax-highlighted code display with color-coded sections (user input, AI output, tool output, errors).
- **OpenCode**: Lip Gloss styles applied to rendered markdown content in the Bubble Tea view.
- **Warp**: GPU-rendered markdown with full syntax highlighting, multi-font support, and rich diff views.
- **Headless modes**: Strip markdown to plain text (Gemini CLI `text` format) or preserve as structured data (`JSON`, `stream-json`).

### Progress Indicators

During tool execution (which can take seconds to minutes), agents need to signal activity:

| Agent | Approach |
|-------|----------|
| **Claude Code** | Customizable spinner verbs (`spinnerVerbs` setting); native progress bar protocols for iTerm2/Windows Terminal |
| **OpenCode** | Spinner and formatting utilities in `internal/format/`; pub/sub events drive status updates |
| **Codex** | Turn lifecycle events: `TurnStarted` → `ExecApprovalRequest` → `TurnComplete` displayed in TUI |
| **Warp** | GPU-rendered smooth animated progress bars; inline action buttons; Active AI overlay |
| **Junie** | Emoji status indicators: ✅ ✏️ ➕ ⏭️ ❌ in plan presentation |
| **Pi** | Event streaming (`tool:start`, `tool:complete`) drives TUI status lines; extensions add custom indicators |
| **Droid** | OpenTelemetry-based metrics; progress updates across any interface (Slack, web, CLI) |

---

## Error Recovery

Streams break. Networks drop. Rate limits hit mid-generation. Robust agents handle all of this gracefully.

### Reconnection Strategies

SSE has built-in reconnection via `Last-Event-ID`, but most LLM APIs don't support resuming a generation mid-stream. In practice, agents treat a dropped stream as a failed turn:

- **Codex**: `stream_max_retries = 5` with `stream_idle_timeout_ms = 300,000` (5 min). If the stream goes idle or drops, retry the entire request up to 5 times.
- **OpenCode**: `maxRetries = 8` at the provider layer. Failed streams finalize the message with `FinishReasonCanceled` and surface the error.
- **Gemini CLI**: Retry logic in `baseLlmClient.ts` with a dedicated `fallback/` module for routing to alternative models when the primary is unavailable.

### Retry with Backoff

| Agent | Max Retries | Backoff Strategy | Timeout |
|-------|-------------|------------------|---------|
| **Codex** | 5 (stream) / 4 (request) | Provider-configured | 300s idle timeout |
| **OpenCode** | 8 | Provider layer | Context-based cancellation |
| **Goose** | 2 (compaction attempts) | Immediate retry | 1000 turns max |
| **ForgeCode** | `max_tool_failure_per_turn` | Configurable | `FORGE_TOOL_TIMEOUT` = 300s |
| **Gemini CLI** | Configurable | Fallback module | Turn iteration limits |

### Partial Response Recovery

When a stream terminates mid-generation, agents must decide what to do with the partial content:

1. **Discard and retry** (most common): The incomplete response is discarded, and the request is retried from scratch. OpenCode, Codex, and Gemini CLI all take this approach.

2. **Preserve partial content**: Some agents save the partial response for debugging or user inspection. OpenCode persists every delta to SQLite — even if the stream fails, the partial message is in the database.

3. **Emergency compaction**: Goose handles `ContextLengthExceeded` mid-stream by triggering `compact_messages()`, replacing the conversation with a summary, and retrying. Up to 2 compaction attempts before giving up.

4. **Checkpoint-based recovery**: Claude Code takes file snapshots before every edit. If a stream fails during a multi-tool turn, the user can rewind to any previous state via `Esc + Esc`. This isn't stream-level recovery but turn-level — the granularity that matters to users.

### Handling Rate Limits Mid-Stream

Rate limits can hit before streaming starts (HTTP 429) or during streaming (connection terminated):

- **Goose**: `CreditsExhausted` yields a notification with `top_up_url` and breaks the loop cleanly
- **Codex**: Provider-level retry handles 429s; `stream_max_retries` covers mid-stream termination
- **Aider**: litellm handles rate limiting and retries transparently across providers
- **Gemini CLI**: Routing module with fallback handling when primary model is rate-limited

### Network Interruption Graceful Degradation

When the network fails entirely:

- **Goose**: Yields "please resend your last message" and breaks — preserving conversation state for when connectivity returns
- **OpenCode**: Context-based cancellation at 4 levels (session, loop, stream, tool). When cancelled mid-stream, the message is finalized using `context.Background()` to ensure the DB write succeeds even though the parent context is cancelled.
- **Codex**: `Op::Interrupt` cancels active streams, aborts pending tools, emits `TurnAborted` — clean state for retry
- **OpenHands**: The entire event stream is persistent and append-only. `ReplayManager` can reconstruct state from any point. Network failures lose nothing.
- **Ante**: Lock-free scheduler ensures no sub-agent blocks another — if one sub-agent's stream fails, others continue unaffected

### User-Initiated Interruption

Users pressing `Ctrl+C` or `Esc` mid-stream is the most common "error" case:

| Agent | Interrupt Mechanism | State After |
|-------|---------------------|-------------|
| **Claude Code** | `Esc` (context preserved) | Can continue conversation |
| **OpenCode** | Context cancellation cascade | Message finalized as canceled |
| **Codex** | `Op::Interrupt` via SQ | `TurnAborted` event, clean state |
| **Goose** | `CancellationToken` checked per iteration | Loop exits cleanly |
| **Pi** | `Enter` = steering message; `Alt+Enter` = queued follow-up | Conversation continues with injection |

Pi's approach is notable: rather than canceling, the user can **steer** the agent mid-stream by pressing Enter to inject a message after the current tool call but before the next LLM inference.

---

## Tools & Projects

Key open-source tools and libraries for building streaming UIs in coding agents, organized by category.

### TUI Frameworks

Frameworks for building interactive terminal user interfaces that render streaming LLM output.

| Framework | Language | Architecture | URL | Why It Matters for Streaming |
|-----------|----------|-------------|-----|------------------------------|
| **Ink** | TypeScript | React (declarative) | [github.com/vadimdemedes/ink](https://github.com/vadimdemedes/ink) | React reconciliation diffs efficiently on each token; `<Static>` freezes finalized output above live content. Used by Claude Code, Gemini CLI, Copilot CLI. |
| **Bubble Tea** | Go | Elm Architecture (TEA) | [github.com/charmbracelet/bubbletea](https://github.com/charmbracelet/bubbletea) | Message-passing maps naturally to SSE chunks — each chunk dispatched as a `tea.Msg`. Used by OpenCode. Ecosystem: Bubbles (components), Lip Gloss (styling), Glamour (markdown). |
| **Ratatui** | Rust | Immediate-mode | [github.com/ratatui/ratatui](https://github.com/ratatui/ratatui) | Redraws full state each frame — no diffing needed. Pair with tokio channels for streaming chunk ingestion. Used by Codex CLI. |
| **Textual** | Python | Component + CSS (async) | [github.com/Textualize/textual](https://github.com/Textualize/textual) | Async-first (asyncio) integrates directly with async LLM streaming APIs. CSS theming for consistent styling. |
| **Rich** | Python | Renderable library | [github.com/Textualize/rich](https://github.com/Textualize/rich) | `Live` display for real-time updates, `Markdown` renderable for incremental rendering, `Progress` for multi-step workflows. Not a full TUI — pair with Textual for interactivity. |

### LLM Streaming Libraries

SDKs that handle HTTP connections to LLM APIs and normalize streaming across providers.

- **LiteLLM** — Unified Python API calling 100+ LLMs in OpenAI-compatible format. Normalizes streaming across all providers with `stream=True`. [github.com/BerriAI/litellm](https://github.com/BerriAI/litellm). Essential for multi-provider coding agents — write one streaming handler for any model.
- **Vercel AI SDK** — Provider-agnostic TypeScript toolkit with `streamText()` and `streamObject()` primitives. [github.com/vercel/ai](https://github.com/vercel/ai). Unified streaming across OpenAI, Anthropic, Google with UI hooks like `useChat`. The most feature-rich TS option.
- **OpenAI Python/Node SDKs** — Official clients with SSE-based streaming via `stream=True`. Support both Chat Completions and Responses API. [openai-python](https://github.com/openai/openai-python) / [openai-node](https://github.com/openai/openai-node). Reference implementations that most agents build on.
- **Anthropic Python SDK** — Official Claude SDK with content-block event streaming and extended thinking support. [github.com/anthropics/anthropic-sdk-python](https://github.com/anthropics/anthropic-sdk-python). Mirrors Anthropic's unique content-block-based protocol.

| SDK | Language | Providers | Abstraction Level |
|-----|----------|-----------|-------------------|
| **LiteLLM** | Python | 100+ (all major) | Highest — normalizes to OpenAI format |
| **Vercel AI SDK** | TypeScript | All major | High — unified stream primitives |
| **OpenAI SDK** | Python / TS | OpenAI only | Low — provider-specific |
| **Anthropic SDK** | Python / TS | Anthropic only | Low — provider-specific |

### SSE & Transport Libraries

SSE (Server-Sent Events) is the dominant transport for LLM streaming. These libraries handle the low-level connection.

| Library | Language | URL | Notes |
|---------|----------|-----|-------|
| **eventsource** | TypeScript | [github.com/EventSource/eventsource](https://github.com/EventSource/eventsource) | W3C-compliant SSE client for Node.js. Auto-reconnection, custom `fetch` support. |
| **@microsoft/fetch-event-source** | TypeScript | [github.com/Azure/fetch-event-source](https://github.com/Azure/fetch-event-source) | Fetch-based SSE supporting POST + custom headers — required for LLM APIs (native EventSource is GET-only). |
| **httpx-sse** | Python | [pypi.org/project/httpx-sse](https://pypi.org/project/httpx-sse/) | SSE support for httpx (used internally by OpenAI/Anthropic SDKs). |
| **r3labs/sse** | Go | [github.com/r3labs/sse](https://github.com/r3labs/sse) | SSE server and client for Go. |
| **reqwest** | Rust | (built-in) | Rust typically parses SSE manually from chunked HTTP responses via reqwest/hyper. |

SSE dominates because LLM inference is inherently unidirectional (model → client). WebSocket is only used for bidirectional use cases like OpenAI's Realtime audio API.

### Incremental Parsing

Streaming LLM responses require parsing incomplete data — partial JSON from tool calls and token-by-token text.

- **Jiter** — Fast iterable JSON parser in Rust with Python bindings. 4-10x faster than serde_json. Iterator mode processes key-value pairs as they arrive without waiting for complete JSON. [github.com/pydantic/jiter](https://github.com/pydantic/jiter). Used by Pydantic V2 (which underlies OpenAI SDK, Anthropic SDK, and LiteLLM).
- **@streamparser/json** — SAX/pull-based streaming JSON parser for TypeScript. Handles incomplete JSON fragments from streaming tool calls. [npmjs.com/package/@streamparser/json](https://www.npmjs.com/package/@streamparser/json).
- **partial-json-parser** — TypeScript library that parses incomplete JSON strings. Useful for showing partial tool parameters during streaming.
- **encoding/json.Decoder** — Go's built-in streaming JSON decoder. Reads tokens from an `io.Reader` without buffering the entire response.

| Language | Recommended Parser | Approach |
|----------|--------------------|----------|
| **Rust** | Jiter | Iterator-based, zero-copy |
| **Python** | Pydantic partial parsing | Type-coerced validation of partial JSON |
| **TypeScript** | @streamparser/json | SAX-like streaming events |
| **Go** | encoding/json.Decoder | Built-in token-level streaming |

### Terminal Markdown Renderers

Render LLM markdown output with syntax highlighting, tables, and formatting in the terminal.

- **Glamour** — Stylesheet-based terminal markdown renderer for Go. Multiple themes (dark, light, custom). [github.com/charmbracelet/glamour](https://github.com/charmbracelet/glamour). Used by GitHub CLI, GitLab CLI, Glow. The standard for Go coding agents alongside Bubble Tea.
- **Termimad** — Skinnable markdown renderer for Rust with template support (`${placeholder}` syntax). [github.com/Canop/termimad](https://github.com/Canop/termimad). Supports tables, code blocks, scrollable views. The go-to for Rust coding agents.
- **Rich Markdown** — Part of Rich (Python). Renders markdown with syntax highlighting and integrates with `Live` display for streaming. [github.com/Textualize/rich](https://github.com/Textualize/rich). Zero extra dependencies for Python agents.
- **marked-terminal** — Custom renderer for `marked` that outputs ANSI-styled text. [github.com/mikaelbr/marked-terminal](https://github.com/mikaelbr/marked-terminal). Used by some Node.js coding agents.

### Spinner & Progress Libraries

Visual feedback during LLM inference latency (before tokens start streaming) and multi-step agent workflows.

| Library | Language | URL | Key Feature |
|---------|----------|-----|-------------|
| **ora** | JavaScript | [github.com/sindresorhus/ora](https://github.com/sindresorhus/ora) | Elegant spinner with `.start()` / `.succeed()` / `.fail()` states. Pairs with Ink. |
| **listr2** | TypeScript | [github.com/listr2/listr2](https://github.com/listr2/listr2) | Task lists with concurrent/sequential execution and progress reporting. |
| **indicatif** | Rust | [github.com/console-rs/indicatif](https://github.com/console-rs/indicatif) | Progress bars + spinners with multi-progress support. Integrates with `tracing` via tracing-indicatif. |
| **Rich Progress** | Python | [github.com/Textualize/rich](https://github.com/Textualize/rich) | `console.status("Thinking...")` for spinners; `Progress()` for multi-step tracking. Built into Rich. |
| **cli-spinners** | JavaScript | [github.com/sindresorhus/cli-spinners](https://github.com/sindresorhus/cli-spinners) | Collection of 80+ spinner animations as JSON data. Used by ora. |

---

## Real-World Implementations

| Agent | Protocol | Parsing | Rendering | Error Recovery | Reference |
|-------|----------|---------|-----------|----------------|-----------|
| **OpenCode** | SSE → Go channels | Delta events → SQLite → pub/sub | Bubble Tea + Lip Gloss | 8 retries; context cancellation cascade | [`../agents/opencode/architecture.md`](../agents/opencode/architecture.md) |
| **Claude Code** | SSE (Anthropic Messages API) | Content block start/delta/stop | Ink (React for CLI) | Checkpoint snapshots; rewind menu | [`../agents/claude-code/architecture.md`](../agents/claude-code/architecture.md) |
| **Warp** | N/A (terminal-native) | Block metadata + PTY buffer | Rust + Metal GPU (~1.9ms redraws) | Active AI proactive error detection | [`../agents/warp/architecture.md`](../agents/warp/architecture.md) |
| **Codex** | SSE + WebSocket (Responses API) | ItemCreated/Updated/Completed → SQ/EQ | Ratatui TUI + exec JSONL mode | 5 stream retries; 300s idle timeout | [`../agents/codex/architecture.md`](../agents/codex/architecture.md) |
| **Goose** | Async stream (`BoxStream<AgentEvent>`) | Stream chunks → categorize_tools() | Event-driven progressive UI | 2 compaction attempts; CancellationToken | [`../agents/goose/agentic-loop.md`](../agents/goose/agentic-loop.md) |
| **Gemini CLI** | SSE (`client.ts`) | contentGenerator.ts pipeline | Ink + headless modes (text/JSON/stream-json) | Retry + fallback module | [`../agents/gemini-cli/architecture.md`](../agents/gemini-cli/architecture.md) |
| **Aider** | litellm (provider-agnostic) | Edit format parsers (diff/whole/udiff) | prompt_toolkit (full response) | Git-based undo; model-specific routing | [`../agents/aider/architecture.md`](../agents/aider/architecture.md) |
| **Pi** | Provider-specific | Event streaming API | Custom pi-tui (differential rendering) | Extension-based recovery | [`../agents/pi-coding-agent/architecture.md`](../agents/pi-coding-agent/architecture.md) |
| **Ante** | HTTP streaming (Rust) | `fn stream() → TokenStream` | Rust native terminal rendering | Lock-free scheduler; sub-agent isolation | [`../agents/ante/architecture.md`](../agents/ante/architecture.md) |
| **OpenHands** | WebSocket/REST → EventStream | Append-only event bus with subscribers | Web UI (EventStream subscriber) | ReplayManager; full state reconstruction | [`../agents/openhands/architecture.md`](../agents/openhands/architecture.md) |
