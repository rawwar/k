# Tools and Projects

Open-source ecosystem for building streaming UIs in coding agents.

For each tool, include: URL, description, language, stars (approximate), and agent relevance.

---

## 1. TUI Frameworks

### Ink (TypeScript)

- **URL**: https://github.com/vadimdemedes/ink
- **Description**: React for CLIs — build and test CLI output using React components
- **Language**: TypeScript
- **Stars**: ~28k
- **Architecture**: React renderer → Yoga flexbox → ANSI output
- **Key Features**: JSX components, `<Static>` for streaming, hooks, flexbox layout
- **Used By**: Claude Code, Gemini CLI, GitHub Copilot CLI, Cloudflare Wrangler, Shopify CLI, Prisma
- **Agent Relevance**: The dominant TUI framework for TypeScript/Node.js coding agents.
  React mental model is widely known, making it easy to recruit contributors.

Ink renders React components to terminal output using a custom reconciler. The
`<Static>` component is particularly important for streaming — it writes output
once and never re-renders it, which is exactly the pattern needed for
token-by-token LLM output. The component tree is laid out with Yoga (Facebook's
flexbox engine compiled to WASM), then serialized to ANSI escape sequences.

Key architectural details:
- **Reconciler**: Uses `react-reconciler` to translate React tree → internal nodes
- **Layout**: Yoga computes flexbox, producing (x, y, width, height) per node
- **Rendering**: Nodes serialize to ANSI strings, diff against previous frame
- **Static output**: `<Static>` items are appended above the dynamic region,
  making them immune to re-renders — ideal for streaming chat messages
- **Testing**: `render()` in test mode returns helpers for assertions, no TTY needed

### Bubble Tea (Go)

- **URL**: https://github.com/charmbracelet/bubbletea
- **Description**: Fun, functional, stateful terminal apps in Go using The Elm Architecture
- **Language**: Go
- **Stars**: ~30k
- **Architecture**: Model → Update → View (Elm Architecture)
- **Key Features**: Type-safe messages, commands for async I/O, cell-based renderer
- **Ecosystem**: Bubbles (components), Lip Gloss (styling), Glamour (markdown), Harmonica (animation)
- **Used By**: OpenCode, gh-dash, chezmoi, and 18k+ dependents
- **Agent Relevance**: The standard for Go-based coding agents. Single binary, fast startup.

Bubble Tea implements The Elm Architecture (TEA) for terminals. The program loop
is: receive a `Msg`, call `Update()` to produce a new `Model` and optional `Cmd`,
then call `View()` to render the model as a string. This functional approach
makes state management predictable and testable.

Key architectural details:
- **Messages**: Typed values dispatched to `Update()`. Async results arrive as messages.
- **Commands**: Functions returning `Msg`. Used for I/O (HTTP, file reads, timers).
- **Sub-processes**: `tea.Exec()` hands over the terminal to a child process (e.g., editor).
- **Renderer**: Two modes — standard (full reflow) and cell-based (for advanced layouts).
- **Composability**: Models embed other models; parent routes messages to children.
- **Bubbles library**: Pre-built components — text input, viewport, list, table, paginator.

For streaming LLM output, agents typically define a `StreamChunkMsg` carrying the
delta text, dispatch it on each SSE event, and let `View()` re-render the
accumulated response. The cell-based renderer (v2) enables partial screen updates
for more complex layouts.

### Ratatui (Rust)

- **URL**: https://github.com/ratatui/ratatui
- **Description**: Library for building rich terminal user interfaces and dashboards in Rust
- **Language**: Rust
- **Stars**: ~12k
- **Architecture**: Immediate-mode rendering with widgets
- **Key Features**: Widget system, constraint-based layout, multiple backends (crossterm, termion, termwiz)
- **Used By**: Codex CLI, and many Rust tools
- **Agent Relevance**: For Rust-based agents. Zero-cost abstractions, same binary as agent core.

Ratatui uses immediate-mode rendering: every frame, the application constructs a
full widget tree and renders it to a `Buffer`. The library diffs the current
buffer against the previous one and emits only the changed cells as ANSI escape
sequences. This makes partial updates efficient despite the "redraw everything"
programming model.

Key architectural details:
- **Frame/Buffer**: Each render pass writes to a `Buffer` (2D grid of `Cell`s)
- **Layout**: Constraint solver (similar to Cassowary) splits areas into rects
- **Widgets**: Stateless `Widget` trait renders into a `Rect` area
- **Backends**: Pluggable — crossterm (cross-platform), termion (Unix), termwiz (advanced)
- **Event loop**: User-managed. Typically uses crossterm's event stream with tokio.

For streaming agents, the pattern is: accumulate tokens in application state,
trigger a re-render on each chunk, and let the widget diff handle efficient output.
Ratatui pairs well with `tui-textarea` for input and `syntect` for highlighting.

### Textual (Python)

- **URL**: https://github.com/Textualize/textual
- **Description**: Modern terminal application framework for Python with CSS-like styling
- **Language**: Python
- **Stars**: ~26k
- **Architecture**: Async component model with TCSS styling
- **Key Features**: CSS for terminals, widget system, reactive properties, web deployment
- **Used By**: Python-based tools
- **Agent Relevance**: Best option for Python agents needing full TUI. Async-first for LLM streaming.

Textual is a full application framework with a CSS-like styling system (TCSS),
a widget tree, message passing, and reactive properties. It runs on asyncio,
making it naturally compatible with async LLM streaming. Widgets can be updated
from async tasks, and the framework handles efficient re-rendering.

Key architectural details:
- **TCSS**: Subset of CSS for terminal styling (colors, borders, padding, grid)
- **Widgets**: Tree of composable widgets with message bubbling
- **Reactive**: Properties that automatically trigger re-renders on change
- **Workers**: Built-in async task management for background operations
- **Web mode**: Same app can run in a browser via textual-web (websocket bridge)

### Rich (Python)

- **URL**: https://github.com/Textualize/rich
- **Description**: Library for rich text and beautiful formatting in the terminal
- **Language**: Python
- **Stars**: ~50k
- **Architecture**: Renderable library (not full TUI)
- **Key Features**: `Live` display, Markdown rendering, syntax highlighting, progress bars, tables
- **Used By**: Aider, many Python tools
- **Agent Relevance**: Perfect for formatted LLM output without a full TUI. Pair with
  Textual for interactivity.

Rich is a rendering library, not a TUI framework. Its `Live` display is the key
feature for streaming: it re-renders a renderable object in-place at a configurable
refresh rate. For coding agents, the typical pattern is to accumulate markdown text,
wrap it in a `Markdown` renderable, and let `Live` handle the terminal updates.

Key renderables for agents:
- **`Markdown`**: Full CommonMark with syntax-highlighted code blocks
- **`Syntax`**: Standalone syntax highlighting with line numbers
- **`Panel`**: Bordered box for grouping output (tool calls, results)
- **`Live`**: Context manager that re-renders content in-place
- **`Console.status()`**: Spinner with message for "thinking" states

### prompt_toolkit (Python)

- **URL**: https://github.com/prompt-toolkit/python-prompt-toolkit
- **Description**: Library for building powerful interactive command line applications
- **Language**: Python
- **Stars**: ~9k
- **Key Features**: Syntax highlighting, auto-completion, history, multi-line editing
- **Used By**: Aider (for terminal interaction), IPython, pgcli, mycli
- **Agent Relevance**: Input handling for Python agents. Not a renderer but great for
  REPL-style prompt UIs with completions and history.

prompt_toolkit is the standard Python library for building interactive prompts.
It handles input independently from output rendering, so it pairs well with Rich
for display. Key features include vi/emacs key bindings, multi-line editing with
syntax highlighting, and async support. Aider uses it for its chat input alongside
Rich for output rendering.

---

## 2. LLM Streaming SDKs

### LiteLLM

- **URL**: https://github.com/BerriAI/litellm
- **Description**: Call 100+ LLM APIs in OpenAI format. Unified streaming across providers.
- **Language**: Python
- **Stars**: ~16k
- **Key Feature**: `stream=True` normalizes streaming across ALL providers
- **Used By**: Aider, many multi-provider agents
- **Agent Relevance**: Essential for multi-provider coding agents. Write one streaming
  handler for any model — OpenAI, Anthropic, Google, Bedrock, Ollama, etc.

LiteLLM wraps every provider's streaming format into OpenAI-compatible
`ChatCompletionChunk` objects. This means agents can write one streaming consumer
and it works across 100+ providers. The library also handles:
- **Fallbacks**: Automatic provider failover on errors
- **Rate limiting**: Token-aware throttling across models
- **Cost tracking**: Per-token cost calculation during streaming
- **Caching**: Semantic caching to skip repeated calls

### Vercel AI SDK

- **URL**: https://github.com/vercel/ai
- **Description**: Provider-agnostic TypeScript toolkit for AI streaming
- **Language**: TypeScript
- **Stars**: ~12k
- **Key Features**: `streamText()`, `streamObject()`, UI hooks (`useChat`), provider adapters
- **Agent Relevance**: Most feature-rich TypeScript streaming option. Unified across providers.

The AI SDK provides both server-side streaming primitives and client-side React hooks.
For CLI agents, the server-side `streamText()` and `streamObject()` are the relevant
APIs. They return `AsyncIterable` streams that yield typed chunks. The provider
system supports OpenAI, Anthropic, Google, Mistral, and custom providers.

Key streaming features:
- **`streamText()`**: Returns stream of text deltas + tool calls
- **`streamObject()`**: Streams partial JSON objects (validated with Zod)
- **Tool streaming**: Tool calls arrive incrementally (partial arguments)
- **`onChunk` callback**: Process each chunk as it arrives

### OpenAI Python SDK

- **URL**: https://github.com/openai/openai-python
- **Description**: Official Python library for the OpenAI API
- **Language**: Python
- **Stars**: ~25k
- **Key Features**: SSE streaming, Chat Completions + Responses API, async support
- **Agent Relevance**: Reference implementation. Most agents build on or mirror this
  streaming format. Both sync and async streaming supported.

The SDK uses `httpx` under the hood and parses SSE via `httpx-sse`. When
`stream=True`, the API returns an iterator of `ChatCompletionChunk` objects.
Each chunk carries a `choices[].delta` with either `content` (text) or
`tool_calls` (function arguments). The Responses API adds a richer event
stream with typed events like `response.output_text.delta`.

### OpenAI Node SDK

- **URL**: https://github.com/openai/openai-node
- **Description**: Official Node.js/TypeScript library for the OpenAI API
- **Language**: TypeScript
- **Stars**: ~8k
- **Key Features**: `stream: true` returns `Stream<ChatCompletionChunk>`, async iteration,
  `.on()` event emitter pattern, `.toReadableStream()` for web compatibility
- **Agent Relevance**: Standard for TypeScript agents calling OpenAI directly.

### Anthropic Python SDK

- **URL**: https://github.com/anthropics/anthropic-sdk-python
- **Description**: Official Python SDK for Anthropic's Claude API
- **Language**: Python
- **Stars**: ~2k
- **Key Features**: Content block streaming, extended thinking, `.stream()` helpers
- **Agent Relevance**: Mirrors Anthropic's unique content-block streaming protocol.

Anthropic's streaming format differs from OpenAI: instead of a flat delta, events
are structured around content blocks. The event stream includes:
- `message_start` → `content_block_start` → `content_block_delta`* → `content_block_stop`
- Tool use arrives as its own content block type with `input_json_delta` events
- Extended thinking has `thinking` block type with separate delta events

The SDK's `.stream()` helper provides a high-level API with callbacks:
`on_text()`, `on_input_json()`, `on_message()`, etc.

### Anthropic TypeScript SDK

- **URL**: https://github.com/anthropics/anthropic-sdk-typescript
- **Description**: Official TypeScript SDK for Anthropic's Claude API
- **Language**: TypeScript
- **Stars**: ~1k
- **Key Features**: Same content-block streaming model as the Python SDK, with
  `MessageStream` helper, `.on()` event emitter, async iteration support
- **Agent Relevance**: Used by TypeScript agents (Claude Code, etc.) for direct Anthropic API calls.

### Google Gen AI SDK

- **URL**: https://github.com/googleapis/python-genai
- **Description**: Official Python SDK for Google Gemini API
- **Language**: Python
- **Stars**: ~1k
- **Key Features**: `generate_content_stream()`, multimodal support, function calling
- **Agent Relevance**: Required for Gemini-based agents. Streaming yields `GenerateContentResponse`
  chunks with `text` parts and function call parts.

---

## 3. SSE and Transport Libraries

### @microsoft/fetch-event-source

- **URL**: https://github.com/Azure/fetch-event-source
- **Description**: Fetch-based SSE supporting POST + custom headers
- **Language**: TypeScript
- **Stars**: ~2k
- **Key Feature**: Required for LLM APIs (native EventSource is GET-only)
- **Agent Relevance**: Critical library — most LLM SSE requires POST with auth headers.
  Native `EventSource` only supports GET requests with no custom headers, making it
  useless for authenticated LLM API calls. This library bridges the gap.

Key advantages over native EventSource:
- POST method support (required for chat completions)
- Custom headers (Authorization, Content-Type)
- Request body (the messages payload)
- Better error handling with `onclose` and `onerror` callbacks
- Automatic reconnection with backoff

### eventsource (Node.js)

- **URL**: https://github.com/EventSource/eventsource
- **Description**: W3C-compliant SSE client for Node.js
- **Language**: TypeScript
- **Stars**: ~1.5k
- **Key Features**: Auto-reconnection, custom `fetch` support, Last-Event-ID tracking
- **Agent Relevance**: Used when you need standard SSE behavior in Node.js. For LLM
  APIs specifically, most agents use the SDK's built-in HTTP client instead.

### httpx-sse (Python)

- **URL**: https://pypi.org/project/httpx-sse/
- **Description**: SSE support for httpx
- **Language**: Python
- **Key Feature**: Used internally by OpenAI and Anthropic Python SDKs to parse SSE
- **Agent Relevance**: You rarely use this directly — the LLM SDKs handle it. But useful
  if building custom streaming from scratch.

### r3labs/sse (Go)

- **URL**: https://github.com/r3labs/sse
- **Description**: SSE server and client for Go
- **Language**: Go
- **Stars**: ~2k
- **Key Features**: Both client and server, channel-based subscriptions, custom event types
- **Agent Relevance**: Used by Go agents that need SSE parsing. Go's `net/http` can also
  handle chunked responses directly, so some agents skip this.

### reqwest (Rust)

- **URL**: https://github.com/seanmonstar/reqwest
- **Description**: Rust HTTP client (SSE parsed manually from chunked responses)
- **Language**: Rust
- **Stars**: ~10k
- **Key Features**: async/await, streaming response bodies, TLS, connection pooling
- **Agent Relevance**: Standard Rust HTTP client. SSE is typically parsed line-by-line
  from the `bytes_stream()` or via the `eventsource-stream` crate on top.

---

## 4. Incremental Parsing Libraries

### Jiter

- **URL**: https://github.com/pydantic/jiter
- **Description**: Fast iterable JSON parser in Rust with Python bindings
- **Language**: Rust + Python
- **Stars**: ~2k
- **Key Features**: 4-10x faster than serde_json for certain workloads, iterator mode, zero-copy
- **Agent Relevance**: Used by Pydantic V2 (which underlies OpenAI SDK, Anthropic SDK,
  LiteLLM). Every streamed chunk is deserialized through Jiter in the Python ecosystem.

Jiter is significant because it sits at the foundation of the Python LLM stack.
Pydantic V2 uses it for JSON parsing, and the OpenAI/Anthropic SDKs use Pydantic
for response models. So every `ChatCompletionChunk` deserialized in Python goes
through Jiter. Its iterator mode is particularly useful for streaming — it can
parse tokens incrementally from a buffer without requiring the full JSON upfront.

### @streamparser/json

- **URL**: https://www.npmjs.com/package/@streamparser/json
- **Description**: SAX/pull-based streaming JSON parser for TypeScript
- **Language**: TypeScript
- **Stars**: ~1k (npm)
- **Key Feature**: Handles incomplete JSON fragments from streaming tool calls
- **Agent Relevance**: Essential for showing partial tool-call arguments as they stream in.
  When a model streams `{"file": "/src/app.ts", "content": "..."}`, this parser can
  extract the `file` key before the full JSON arrives.

### partial-json-parser

- **URL**: https://www.npmjs.com/package/partial-json-parser
- **Description**: Parse incomplete JSON strings
- **Language**: TypeScript
- **Key Feature**: Show partial tool parameters during streaming. Attempts to "close"
  incomplete JSON by adding missing brackets/quotes.
- **Agent Relevance**: Complementary to @streamparser/json. Simpler API — pass in a
  partial JSON string, get back a best-effort parsed object.

### encoding/json.Decoder (Go stdlib)

- **Description**: Go's built-in streaming JSON decoder
- **Key Feature**: Reads tokens from `io.Reader`, no external dependency needed
- **Agent Relevance**: Go agents typically use this directly. Call `Decoder.Token()`
  in a loop to parse SSE data fields incrementally. No third-party dependency required.

### serde_json StreamDeserializer (Rust)

- **Description**: Streaming iterator of JSON values from serde_json
- **Key Feature**: Standard Rust JSON streaming. Deserializes a sequence of JSON values
  from a byte stream, yielding `Result<T>` for each complete value.
- **Agent Relevance**: Used in Rust agents to parse the SSE `data:` field into typed
  chunk structs. Pairs with `reqwest::Response::bytes_stream()`.

---

## 5. Terminal Markdown Renderers

### Glamour

- **URL**: https://github.com/charmbracelet/glamour
- **Description**: Stylesheet-based terminal markdown renderer for Go
- **Language**: Go
- **Stars**: ~2k
- **Key Features**: Multiple themes (dark, light, notty), syntax highlighting via Chroma,
  tables, custom stylesheets in JSON
- **Used By**: GitHub CLI, GitLab CLI, Glow, OpenCode
- **Agent Relevance**: The standard markdown renderer for Go agents. Theme-aware (detects
  light/dark terminal backgrounds). Renders full CommonMark.

### Termimad

- **URL**: https://github.com/Canop/termimad
- **Description**: Skinnable markdown renderer for Rust
- **Language**: Rust
- **Stars**: ~1k
- **Key Features**: Template syntax (inline markdown in format strings), tables, code blocks,
  scrollable views with `MadView`, skinning system
- **Agent Relevance**: Used by Rust agents for markdown output. Lighter than a full TUI.

### Rich Markdown (Python)

- Part of the Rich library (`from rich.markdown import Markdown`)
- **Key Feature**: Full CommonMark rendering with syntax-highlighted code blocks,
  integrates seamlessly with `Live` for streaming updates
- **Agent Relevance**: The standard for Python agents. Pass `Markdown(text)` to
  `Live.update()` on each chunk for streaming markdown rendering.

### marked-terminal

- **URL**: https://github.com/mikaelbr/marked-terminal
- **Description**: Custom renderer for marked that outputs ANSI-styled text
- **Language**: JavaScript
- **Stars**: ~1k
- **Key Features**: Plugs into the `marked` markdown parser, customizable styles,
  syntax highlighting via `cli-highlight`
- **Agent Relevance**: Simple option for Node.js agents. Less feature-rich than rendering
  markdown via Ink components but works without React.

---

## 6. Spinner and Progress Libraries

### ora

- **URL**: https://github.com/sindresorhus/ora
- **Description**: Elegant terminal spinner
- **Language**: JavaScript
- **Stars**: ~9k
- **Key Features**: `.start()` / `.succeed()` / `.fail()` state transitions, 80+ built-in
  spinner animations, color support, stream selection (stdout/stderr)
- **Agent Relevance**: Used for "thinking" indicators between user input and first token.
  Simple API: `ora('Thinking...').start()` then `.stop()` when streaming begins.

### indicatif

- **URL**: https://github.com/console-rs/indicatif
- **Description**: Progress bars and spinners for Rust
- **Language**: Rust
- **Stars**: ~5k
- **Key Features**: `ProgressBar` with templates, `MultiProgress` for concurrent bars,
  `ProgressStyle` with custom templates, integration with tracing/log crates
- **Agent Relevance**: Used by Rust agents for file operation progress, multi-step task
  tracking, and "thinking" spinners. The `MultiProgress` feature is useful for
  showing concurrent tool executions.

### cli-spinners

- **URL**: https://github.com/sindresorhus/cli-spinners
- **Description**: Collection of 80+ spinner animations as JSON
- **Language**: JavaScript (data package)
- **Stars**: ~2k
- **Key Feature**: Data-only — provides spinner frame arrays and intervals. Used by ora,
  Ink's `<Spinner>`, and other libraries as their animation source.

### listr2

- **URL**: https://github.com/listr2/listr2
- **Description**: Task lists with concurrent/sequential execution and streaming output
- **Language**: TypeScript
- **Stars**: ~500
- **Key Features**: Nested task groups, concurrent execution with spinner per task,
  multiple renderers (default, verbose, silent), rollback support
- **Agent Relevance**: Good for multi-step workflows (lint, test, build) shown as a task
  list. Each task can show its own spinner and status.

### Rich Progress (Python)

- Part of the Rich library (`from rich.progress import Progress`)
- **Key Features**: `Progress()` context manager for multi-bar tracking, custom columns,
  `console.status()` for simple spinners with context manager pattern
- **Agent Relevance**: Python agents use `console.status("Thinking...")` for the thinking
  indicator and `Progress()` for file processing or multi-step operations.

---

## 7. Terminal Styling Libraries

### Lip Gloss

- **URL**: https://github.com/charmbracelet/lipgloss
- **Description**: Declarative terminal styling for Go
- **Language**: Go
- **Stars**: ~9k
- **Key Features**: CSS-like properties (padding, margin, border, colors), adaptive colors
  (light/dark terminal detection), composable style inheritance, table rendering
- **Agent Relevance**: Paired with Bubble Tea for styling. Adaptive colors ensure agents
  look correct on both light and dark terminal backgrounds.

### chalk

- **URL**: https://github.com/chalk/chalk
- **Description**: Terminal string styling for JavaScript
- **Language**: JavaScript
- **Stars**: ~22k
- **Key Features**: Chainable API (`chalk.bold.red('Error')`), 256/truecolor support,
  level detection, tagged template literals
- **Agent Relevance**: Basic styling for Node.js agents not using Ink. When using Ink,
  its built-in `<Text>` color props are typically preferred.

### colorama (Python)

- **URL**: https://github.com/tartley/colorama
- **Description**: Cross-platform colored terminal text (especially Windows compatibility)
- **Language**: Python
- **Stars**: ~3k
- **Key Feature**: Makes ANSI escape sequences work on Windows by wrapping stdout
- **Agent Relevance**: Mostly superseded by Rich for new Python agents, but still used
  for simple color needs and Windows compatibility.

### colored (Rust)

- **URL**: https://github.com/colored-rs/colored
- **Description**: Simple terminal color library for Rust
- **Language**: Rust
- **Stars**: ~2k
- **Key Feature**: Trait-based API (`"text".red().bold()`)
- **Agent Relevance**: Lightweight option for Rust agents that don't need full Ratatui.

---

## 8. Syntax Highlighting

### Chroma

- **URL**: https://github.com/alecthomas/chroma
- **Description**: Pure Go syntax highlighter based on Pygments
- **Language**: Go
- **Stars**: ~4k
- **Key Features**: 300+ languages, terminal formatters (ANSI 256-color, truecolor),
  multiple output formats (HTML, SVG, terminal), Pygments-compatible styles
- **Agent Relevance**: Used by Glamour for code blocks. Go agents typically use Chroma
  directly for standalone code highlighting.

### syntect

- **URL**: https://github.com/trishume/syntect
- **Description**: Rust library for syntax highlighting using Sublime Text syntax definitions
- **Language**: Rust
- **Stars**: ~5k
- **Key Features**: Sublime Text `.sublime-syntax` files, TextMate `.tmTheme` themes,
  incremental parsing, terminal escape sequence output
- **Agent Relevance**: The standard for Rust agents. Used by `bat` (cat replacement),
  `delta` (git diff viewer), and many code tools. Incremental parsing is useful
  for highlighting code as it streams in.

### Pygments

- **URL**: https://github.com/pygments/pygments
- **Description**: Generic syntax highlighter for Python
- **Language**: Python
- **Stars**: ~2k
- **Key Features**: 500+ languages, 80+ styles, terminal formatters, foundation for many
  other highlighters (Chroma, Rouge)
- **Agent Relevance**: Used by Rich internally for code highlighting. Python agents
  rarely call Pygments directly — Rich's `Syntax` renderable is the typical interface.

---

## 9. Summary Comparison Table

| Category | Tool | Language | Stars | Used By Agents |
|---|---|---|---|---|
| TUI Framework | Ink | TypeScript | ~28k | Claude Code, Gemini CLI, GitHub Copilot CLI |
| TUI Framework | Bubble Tea | Go | ~30k | OpenCode, gh-dash |
| TUI Framework | Ratatui | Rust | ~12k | Codex CLI |
| TUI Framework | Textual | Python | ~26k | Python tools |
| TUI Framework | Rich | Python | ~50k | Aider |
| TUI Framework | prompt_toolkit | Python | ~9k | Aider, IPython |
| LLM SDK | LiteLLM | Python | ~16k | Aider, multi-provider agents |
| LLM SDK | Vercel AI SDK | TypeScript | ~12k | Next.js AI apps |
| LLM SDK | OpenAI Python | Python | ~25k | Most Python agents |
| LLM SDK | OpenAI Node | TypeScript | ~8k | Most TS agents |
| LLM SDK | Anthropic Python | Python | ~2k | Claude-based agents |
| LLM SDK | Anthropic TS | TypeScript | ~1k | Claude Code |
| LLM SDK | Google Gen AI | Python | ~1k | Gemini agents |
| SSE/Transport | fetch-event-source | TypeScript | ~2k | Custom TS agents |
| SSE/Transport | eventsource | TypeScript | ~1.5k | Node.js SSE clients |
| SSE/Transport | httpx-sse | Python | — | OpenAI/Anthropic SDKs |
| SSE/Transport | r3labs/sse | Go | ~2k | Go agents |
| SSE/Transport | reqwest | Rust | ~10k | Rust agents |
| JSON Parsing | Jiter | Rust+Python | ~2k | Pydantic V2 (all Python SDKs) |
| JSON Parsing | @streamparser/json | TypeScript | ~1k | TS agents (partial tool args) |
| JSON Parsing | partial-json-parser | TypeScript | — | TS agents (partial tool args) |
| JSON Parsing | encoding/json | Go | stdlib | All Go agents |
| JSON Parsing | serde_json | Rust | stdlib-like | All Rust agents |
| Markdown | Glamour | Go | ~2k | GitHub CLI, OpenCode |
| Markdown | Termimad | Rust | ~1k | Rust tools |
| Markdown | Rich Markdown | Python | (in Rich) | Aider |
| Markdown | marked-terminal | JavaScript | ~1k | Node.js tools |
| Spinner | ora | JavaScript | ~9k | Many Node.js CLIs |
| Spinner | indicatif | Rust | ~5k | Rust CLIs |
| Spinner | cli-spinners | JavaScript | ~2k | Data for ora, Ink |
| Spinner | listr2 | TypeScript | ~500 | Task-list UIs |
| Spinner | Rich Progress | Python | (in Rich) | Python agents |
| Styling | Lip Gloss | Go | ~9k | Bubble Tea apps |
| Styling | chalk | JavaScript | ~22k | Node.js CLIs |
| Styling | colorama | Python | ~3k | Legacy Python tools |
| Styling | colored | Rust | ~2k | Rust CLIs |
| Highlighting | Chroma | Go | ~4k | Glamour, Go tools |
| Highlighting | syntect | Rust | ~5k | bat, delta |
| Highlighting | Pygments | Python | ~2k | Rich (internal) |

---

## 10. Key Takeaways

### By Language Ecosystem

**TypeScript/Node.js**: Ink + Vercel AI SDK (or OpenAI Node SDK) + chalk + ora.
This is the most popular stack for coding agents. React mental model, large
ecosystem, async/await streaming.

**Go**: Bubble Tea + Lip Gloss + Glamour + Chroma. The Charm ecosystem provides
a complete, cohesive toolkit. Single binary distribution is a major advantage.

**Rust**: Ratatui + syntect + indicatif + reqwest. Zero-cost abstractions,
no runtime overhead. Best for agents where the TUI and core logic share a binary.

**Python**: Rich (+ Textual for full TUI) + LiteLLM + prompt_toolkit. Rich's
`Live` display is the simplest path to streaming output. LiteLLM adds
multi-provider support.

### Critical Infrastructure

The most impactful libraries are often invisible:
- **Jiter**: Deserializes every LLM chunk in the Python ecosystem
- **httpx-sse**: Parses SSE in OpenAI/Anthropic Python SDKs
- **Yoga (via Ink)**: Computes layout for most TypeScript agent UIs
- **crossterm**: Provides terminal I/O for both Ratatui and many Rust tools

### Streaming-Specific Patterns

Every framework has a "streaming primitive" for LLM output:
- **Ink**: `<Static>` component (append-only output region)
- **Bubble Tea**: Messages dispatched from goroutine → `Update()` → `View()`
- **Ratatui**: Buffer diff after state mutation on each chunk
- **Rich**: `Live.update()` with `Markdown` renderable
- **Textual**: Async worker posting widget updates

These primitives share a pattern: accumulate tokens in state, trigger re-render,
let the framework handle efficient terminal updates.
