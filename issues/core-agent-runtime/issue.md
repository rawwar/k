# 🏗️ Core Agent Runtime: Agentic Loop with Tool Registry and Provider-Agnostic LLM Client

## Summary

Implement the foundational runtime of Kodai: a working agentic loop backed by a trait-based tool registry, provider-agnostic LLM client, and structured conversation state management. This is the "engine" that everything else plugs into — file tools, shell execution, streaming, TUI, and provider support all depend on this core being solid.

## Motivation

Right now the codebase has chapter-level code snapshots in `learn/code/` that demonstrate individual concepts in isolation (ch01 = REPL, ch02 = API call, ch03 = agentic loop, ch04 = tool registry). These are educational scaffolding — they live in single `main.rs` files, duplicate code between chapters, and aren't structured as a real application.

The actual Kodai binary needs a properly modularized core runtime that can serve as the foundation for all subsequent features. Without this, every feature (file tools, shell execution, streaming, TUI) would be built on an ad-hoc base and require painful rewrites later.

## Scope

This issue covers **four tightly coupled subsystems** that must be designed and built together because they share types and interfaces:

### 1. Message Types & Conversation State (`src/conversation/`)

- `Role` enum (`User`, `Assistant`)
- `ContentBlock` enum with serde-tagged variants: `Text`, `ToolUse { id, name, input }`, `ToolResult { tool_use_id, content, is_error }`
- `Message` struct with `role: Role` and `content: Vec<ContentBlock>`, plus convenience constructors (`Message::user()`, `Message::assistant()`, `Message::tool_results()`)
- `ConversationState` struct holding: system prompt, message history (`Vec<Message>`), cumulative token usage, and a rough token estimator (`estimate_token_count()`) using the ~4 chars/token heuristic
- Context window overflow detection (`is_approaching_limit()`) at 80% of the configured max

### 2. Tool Trait & Registry (`src/tools/`)

- `ToolError` enum with three variants: `InvalidInput(String)`, `ExecutionFailed(String)`, `SystemError(String)` — each variant enables different error-handling behavior downstream
- `Tool` trait with `Send + Sync` bounds, requiring: `fn name() -> &str`, `fn description() -> &str`, `fn input_schema() -> Value`, `fn execute(&self, input: &Value) -> Result<String, ToolError>`
- `ToolRegistry` backed by `HashMap<String, Box<dyn Tool>>` with: `register()`, `get()`, `tool_names()`, `tool_definitions() -> Vec<Value>` (generates the JSON array for the API's `tools` parameter)
- `dispatch_tool_call()` function with: registry lookup, `panic::catch_unwind` for crash isolation, timing/logging, output truncation at 50K chars
- `dispatch_all()` for batch dispatch of multiple tool calls from a single response
- Two example tools for integration testing:
  - `EchoTool` — echoes input back (pipeline validation)
  - `GetCurrentTimeTool` — returns current datetime (demonstrates a real side-effect-free tool)

### 3. Provider-Agnostic LLM Client (`src/provider/`)

- `Provider` trait with an async method: `async fn send_message(&self, request: &CompletionRequest) -> Result<CompletionResponse, ProviderError>`
- `CompletionRequest` struct: model, max_tokens, system prompt, messages, tool definitions, temperature
- `CompletionResponse` struct: content blocks, stop_reason (as enum: `EndTurn`, `ToolUse`, `MaxTokens`, `StopSequence`), usage stats
- `ProviderError` enum: `AuthenticationError`, `RateLimited { retry_after }`, `ApiError { status, body }`, `NetworkError`, `ParseError`
- `AnthropicProvider` as the first concrete implementation: HTTP client via `reqwest`, `x-api-key` / `anthropic-version` headers, request/response serialization matching the Messages API format
- API key loaded from `ANTHROPIC_API_KEY` env var (or passed via config)

### 4. The Agentic Loop (`src/agent.rs`)

- `Agent` struct holding: provider (as `Box<dyn Provider>`), tool registry, config (max turns, max tokens, model name)
- `AgentError` enum: `ProviderError(ProviderError)`, `EmptyResponse { turn }`, `MaxTurnsReached { limit }`, `ContextOverflow`
- `LoopResult` enum: `Complete(String)`, `MaxTokens(String)`, `TurnLimitReached(String)`, `ContextOverflow`
- `async fn run(&self, state: &mut ConversationState, user_message: &str) -> Result<LoopResult, AgentError>` implementing the core cycle:
  1. Append user message to conversation state
  2. Check stop conditions (turn limit, context window)
  3. Call the provider
  4. Append assistant response to history
  5. Match on `stop_reason`:
     - `EndTurn` → extract text, return `LoopResult::Complete`
     - `ToolUse` → dispatch all tool calls via registry, append tool results, continue loop
     - `MaxTokens` → return `LoopResult::MaxTokens` with partial text
  6. Increment turn counter, loop back to step 2

## Architecture Diagram

```
┌─────────────────────────────────────────────────┐
│                   Agent                         │
│                                                 │
│  ┌──────────────┐    ┌──────────────────────┐   │
│  │ Conversation │    │    Tool Registry      │   │
│  │    State      │    │  ┌────────────────┐  │   │
│  │ ┌──────────┐ │    │  │ HashMap<String, │  │   │
│  │ │ Messages │ │    │  │  Box<dyn Tool>> │  │   │
│  │ └──────────┘ │    │  └────────────────┘  │   │
│  │ ┌──────────┐ │    │  dispatch_tool_call() │   │
│  │ │  Usage   │ │    │  dispatch_all()       │   │
│  │ └──────────┘ │    └──────────────────────┘   │
│  └──────────────┘                               │
│                                                 │
│  ┌──────────────────────────────────────────┐   │
│  │         Agentic Loop (agent.rs)          │   │
│  │                                          │   │
│  │  loop {                                  │   │
│  │    check_stop_conditions()               │   │
│  │    response = provider.send_message()    │   │
│  │    match response.stop_reason {          │   │
│  │      EndTurn => return Complete,         │   │
│  │      ToolUse => dispatch + continue,     │   │
│  │    }                                     │   │
│  │  }                                       │   │
│  └──────────────────────────────────────────┘   │
│                                                 │
│  ┌──────────────────────────────────────────┐   │
│  │     Provider (Box<dyn Provider>)         │   │
│  │  ┌──────────────────────────────────┐    │   │
│  │  │      AnthropicProvider           │    │   │
│  │  │  POST /v1/messages               │    │   │
│  │  │  x-api-key, anthropic-version    │    │   │
│  │  └──────────────────────────────────┘    │   │
│  └──────────────────────────────────────────┘   │
└─────────────────────────────────────────────────┘
```

## Module Layout

```
src/
├── main.rs                    # CLI entry point (clap), REPL, wires everything together
├── agent.rs                   # Agent struct, agentic loop, LoopResult, AgentError
├── conversation/
│   ├── mod.rs                 # ConversationState
│   └── types.rs               # Role, ContentBlock, Message
├── provider/
│   ├── mod.rs                 # Provider trait, CompletionRequest/Response, ProviderError
│   └── anthropic.rs           # AnthropicProvider implementation
└── tools/
    ├── mod.rs                 # Tool trait, ToolError, ToolRegistry, dispatch functions
    ├── echo.rs                # EchoTool (testing)
    └── time.rs                # GetCurrentTimeTool (demo)
```

## Acceptance Criteria

- [ ] `cargo build` compiles with zero warnings
- [ ] `cargo test` passes with unit tests for:
  - [ ] Message type serialization/deserialization round-trips
  - [ ] Tool registry: register, lookup, duplicate name handling
  - [ ] Tool dispatch: successful execution, unknown tool, panic recovery, output truncation
  - [ ] EchoTool and GetCurrentTimeTool execute correctly with valid and invalid inputs
  - [ ] ConversationState token estimation and limit detection
  - [ ] AnthropicProvider request serialization (mock the HTTP layer, don't hit the real API)
  - [ ] Agentic loop with a mock provider: single-turn (end_turn), multi-turn (tool_use → end_turn), max turns exceeded, empty response handling
- [ ] `cargo clippy` passes with no warnings
- [ ] Running `cargo run` with `ANTHROPIC_API_KEY` set starts a REPL that can:
  - Accept user input
  - Send it through the agentic loop
  - Execute tool calls (echo, time) when the model requests them
  - Display the final response
  - Handle `/help`, `/quit`, Ctrl+C, Ctrl+D gracefully
- [ ] The `Provider` trait is async and object-safe, so a second provider (e.g., OpenAI) can be added later by implementing the trait without touching agent.rs
- [ ] All public types and functions have doc comments

## Non-Goals (for this issue)

- File operation tools (read, write, edit) — separate issue
- Shell execution tool — separate issue
- Streaming responses / SSE — separate issue
- Terminal UI (ratatui) — separate issue
- Session persistence / history — separate issue
- Configuration file support — separate issue
- Permission/safety system — separate issue

## Technical Notes

- Use `tokio` as the async runtime (already in ch03's dependencies)
- Use `reqwest` with the `json` feature for HTTP
- Use `serde` + `serde_json` for serialization; use `#[serde(tag = "type")]` for internally-tagged ContentBlock variants
- The `Tool` trait should be synchronous (`fn execute`) — most tools do I/O but not async I/O, and `async_trait` adds complexity. If a tool needs async (e.g., HTTP-based tools), it can use `tokio::runtime::Handle::current().block_on()` internally.
- `StopReason` should be a proper enum, not a raw string — this gives exhaustive matching and catches unhandled cases at compile time
- Design the `Provider` trait so it can be mocked in tests without hitting the network

## References

- [Learn: Agentic Loop Architecture](../../learn/project/03-the-agentic-loop/02-loop-architecture.md) — state machine design
- [Learn: Tool Trait Design](../../learn/project/04-building-a-tool-system/02-tool-trait-design.md) — trait and registry patterns
- [Learn: Anthropic API Overview](../../learn/project/02-first-llm-call/02-anthropic-api-overview.md) — request/response format
- [Learn: OpenCode Analysis](../../learn/linear/01-what-is-a-coding-agent/06-opencode-analysis.md) — provider abstraction, registry pattern
- [Learn: State Machine Model](../../learn/linear/04-anatomy-of-an-agentic-loop/03-state-machine-model.md) — formal state transitions
- [Research: Agentic Loop](../../research/concepts/agentic-loop.md)
- [Research: Tool Systems](../../research/concepts/tool-systems.md)

## Labels

`core`, `architecture`, `priority: critical`
