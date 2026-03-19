# OpenCode — Unique Patterns

## Overview

OpenCode stands out in the coding agent ecosystem through several distinctive design choices: being written in Go rather than Python/TypeScript, using Bubble Tea for a rich terminal UI, implementing a generic pub/sub system for reactive state management, and leveraging Go's type system for a clean provider abstraction. This document analyzes the patterns that make OpenCode architecturally interesting.

## 1. Go as an Agent Language

### Why It's Unusual

The vast majority of coding agents are built in Python (Aider, SWE-agent, Devin) or TypeScript/JavaScript (Cline, Continue). OpenCode chose Go, which has significant implications:

**Advantages**:
- **Single binary distribution**: `go install` or download a binary. No runtime, no virtual environments, no node_modules.
- **Fast startup**: Sub-second cold start vs. Python's multi-second import chains
- **Excellent concurrency**: Goroutines and channels map naturally to streaming LLM responses, parallel tool execution, and reactive UI updates
- **Static typing**: Catches many errors at compile time that Python agents find at runtime
- **Memory efficiency**: Lower overhead than Python/Node.js runtimes

**Trade-offs**:
- **Less ecosystem**: Python has langchain, llama-index, and vast ML tooling. Go has minimal LLM libraries.
- **Verbose JSON handling**: Go's lack of dynamic typing makes JSON parameter handling more boilerplate-heavy
- **Fewer contributors**: The Go AI/ML community is smaller
- **No REPL development**: Harder to prototype interactively

### Go-Specific Patterns in the Codebase

**Context-based cancellation everywhere**:
```go
func (a *agent) processGeneration(ctx context.Context, ...) AgentEvent {
    for {
        select {
        case <-ctx.Done():
            return a.err(ctx.Err())
        default:
        }
        // ... continue processing
    }
}
```

**Goroutine-per-request with panic recovery**:
```go
go func() {
    defer logging.RecoverPanic("agent.Run", func() {
        events <- a.err(fmt.Errorf("panic while running the agent"))
    })
    result := a.processGeneration(genCtx, sessionID, content, attachmentParts)
    // ...
}()
```

**sync.Map for concurrent session tracking**:
```go
type agent struct {
    activeRequests sync.Map  // sessionID → context.CancelFunc
}
```

## 2. Bubble Tea TUI Architecture

### The Elm Architecture in Go

OpenCode uses [Bubble Tea](https://github.com/charmbracelet/bubbletea), which implements the Elm Architecture (TEA) in Go:

```
Model → View → (User Input) → Update → Model → View → ...
```

This is fundamentally different from most CLI agents which simply print text to stdout. OpenCode has:
- **Full-screen terminal UI** with layout regions
- **Modal dialogs** (permission, model selection, session switching)
- **Vim-like editor** for input with keybindings
- **Real-time streaming** text rendering
- **Theme support** (Catppuccin palettes)
- **Image rendering** in the terminal

### Component Structure

```
internal/tui/
├── tui.go              # Main Bubble Tea model
├── components/         # Reusable UI components
│   ├── chat/           # Chat message display
│   ├── editor/         # Input editor (vim-like)
│   ├── dialog/         # Modal dialogs
│   ├── status/         # Status bar
│   └── ...
├── layout/             # Layout management
├── page/               # Full page views
│   ├── chat.go         # Chat page
│   └── logs.go         # Log viewer page
├── styles/             # Lip Gloss styles
├── theme/              # Theme definitions
└── image/              # Terminal image rendering
```

### Reactive Updates via Pub/Sub

The TUI subscribes to backend state changes through the generic pub/sub broker:

```go
// TUI subscribes to message updates
msgChan := app.Messages.Subscribe(ctx)
go func() {
    for event := range msgChan {
        switch event.Type {
        case pubsub.CreatedEvent:
            // New message → re-render chat
        case pubsub.UpdatedEvent:
            // Streaming delta → update current message
        }
    }
}()
```

This decouples the agent loop from the UI — the agent writes to services, services publish events, the TUI reacts. This is especially important for:
- **Streaming text**: Each `EventContentDelta` updates the message in the DB, publishes an event, and the TUI re-renders the chat
- **Permission dialogs**: The permission service publishes a request, the TUI renders the dialog, the user responds
- **Session management**: Creating/deleting sessions triggers UI updates

### Lip Gloss Styling

UI styling uses [Lip Gloss](https://github.com/charmbracelet/lipgloss), another Charm library:

```go
// Styles defined declaratively
style := lipgloss.NewStyle().
    Bold(true).
    Foreground(lipgloss.Color("#FAFAFA")).
    Background(lipgloss.Color("#7D56F4")).
    PaddingTop(2).
    PaddingLeft(4)
```

Combined with Catppuccin color palettes for consistent, beautiful theming across terminal emulators.

## 3. Generic Typed Pub/Sub Broker

### Pattern

One of the most elegant patterns in OpenCode is the generic pub/sub broker:

```go
// internal/pubsub/broker.go
type Broker[T any] struct {
    subs      map[chan Event[T]]struct{}
    mu        sync.RWMutex
    done      chan struct{}
}

func (b *Broker[T]) Subscribe(ctx context.Context) <-chan Event[T] {
    sub := make(chan Event[T], bufferSize)
    b.subs[sub] = struct{}{}
    // Auto-cleanup when context is done
    go func() {
        <-ctx.Done()
        b.mu.Lock()
        delete(b.subs, sub)
        close(sub)
        b.mu.Unlock()
    }()
    return sub
}

func (b *Broker[T]) Publish(t EventType, payload T) {
    event := Event[T]{Type: t, Payload: payload}
    for _, sub := range subscribers {
        select {
        case sub <- event:
        default: // Non-blocking: drop if buffer full
        }
    }
}
```

### Usage

This broker is embedded in every service via Go's struct embedding:

```go
type session.service struct {
    *pubsub.Broker[Session]  // Embedded — inherits Subscribe/Publish
    q db.Querier
}

type message.service struct {
    *pubsub.Broker[Message]
    q db.Querier
}

type agent struct {
    *pubsub.Broker[AgentEvent]
    // ...
}

type permissionService struct {
    *pubsub.Broker[PermissionRequest]
    // ...
}
```

Benefits:
- **Type-safe**: Each broker carries its payload type — `Broker[Session]` can only publish `Session` events
- **Context-scoped subscriptions**: Subscribers auto-cleanup when their context is done
- **Non-blocking publish**: Uses `select` with `default` to avoid blocking publishers when subscribers are slow
- **Composable**: Embed in any service to add reactive capabilities

This is a pattern that's uniquely clean in Go thanks to generics (added in Go 1.18).

## 4. Provider Abstraction with Generic Base

### The Pattern

OpenCode uses Go generics to create a base provider that delegates to provider-specific clients:

```go
type ProviderClient interface {
    send(ctx context.Context, messages []message.Message,
        tools []tools.BaseTool) (*ProviderResponse, error)
    stream(ctx context.Context, messages []message.Message,
        tools []tools.BaseTool) <-chan ProviderEvent
}

type baseProvider[C ProviderClient] struct {
    options providerClientOptions
    client  C
}

func (p *baseProvider[C]) StreamResponse(ctx context.Context,
    messages []message.Message, tools []tools.BaseTool) <-chan ProviderEvent {
    messages = p.cleanMessages(messages)
    return p.client.stream(ctx, messages, tools)
}
```

### Factory with Code Reuse

The factory function maximizes reuse:

```go
func NewProvider(providerName models.ModelProvider, ...) (Provider, error) {
    switch providerName {
    case models.ProviderAnthropic:
        return &baseProvider[AnthropicClient]{...}, nil
    case models.ProviderOpenAI:
        return &baseProvider[OpenAIClient]{...}, nil
    case models.ProviderGROQ:
        // Reuses OpenAI client with different base URL
        opts.openaiOptions = append(opts.openaiOptions,
            WithOpenAIBaseURL("https://api.groq.com/openai/v1"))
        return &baseProvider[OpenAIClient]{...}, nil
    case models.ProviderOpenRouter:
        opts.openaiOptions = append(opts.openaiOptions,
            WithOpenAIBaseURL("https://openrouter.ai/api/v1"))
        return &baseProvider[OpenAIClient]{...}, nil
    }
}
```

Only 5 client implementations cover 10+ providers:
- `AnthropicClient` — Anthropic SDK
- `OpenAIClient` — OpenAI SDK (reused for Groq, OpenRouter, xAI, local)
- `GeminiClient` — Google GenAI SDK
- `CopilotClient` — GitHub Copilot (custom OAuth flow)
- `BedrockClient` — AWS Bedrock (wraps Anthropic over AWS)
- `AzureClient` — Azure OpenAI
- `VertexAIClient` — Google Vertex AI

### Event-Based Streaming

All providers emit events through a channel:

```go
type ProviderEvent struct {
    Type EventType
    Content  string
    Thinking string
    Response *ProviderResponse
    ToolCall *message.ToolCall
    Error    error
}

const (
    EventContentStart  EventType = "content_start"
    EventToolUseStart  EventType = "tool_use_start"
    EventToolUseDelta  EventType = "tool_use_delta"
    EventToolUseStop   EventType = "tool_use_stop"
    EventContentDelta  EventType = "content_delta"
    EventThinkingDelta EventType = "thinking_delta"
    EventContentStop   EventType = "content_stop"
    EventComplete      EventType = "complete"
    EventError         EventType = "error"
)
```

This unified event model means the agent loop doesn't need to know which provider it's talking to.

## 5. Channel-Based Permission Blocking

### Pattern

The permission system uses channels to synchronize between the agent's tool execution goroutine and the TUI's user interaction:

```go
func (s *permissionService) Request(opts CreatePermissionRequest) bool {
    respCh := make(chan bool, 1)
    s.pendingRequests.Store(permission.ID, respCh)
    s.Publish(pubsub.CreatedEvent, permission)  // TUI sees this
    resp := <-respCh  // BLOCKS HERE until user responds
    return resp
}

func (s *permissionService) Grant(permission PermissionRequest) {
    respCh, ok := s.pendingRequests.Load(permission.ID)
    if ok {
        respCh.(chan bool) <- true  // Unblocks the waiting goroutine
    }
}
```

This is a textbook Go concurrency pattern: use a channel as a synchronization point between goroutines. The tool goroutine blocks on the channel, the TUI goroutine sends the response.

## 6. Dual System Prompts

### Pattern

OpenCode maintains separate system prompts for different providers:

```go
func CoderPrompt(provider models.ModelProvider) string {
    basePrompt := baseAnthropicCoderPrompt  // Default
    switch provider {
    case models.ProviderOpenAI:
        basePrompt = baseOpenAICoderPrompt
    }
    return fmt.Sprintf("%s\n\n%s\n%s", basePrompt, envInfo, lspInformation())
}
```

The Anthropic prompt is Claude-style (concise, direct, "OpenCode" identity). The OpenAI prompt follows a more structured format with explicit rules. Both include:
- Tool usage guidelines
- Git commit/PR workflow instructions
- Code style requirements
- Safety rules

## 7. SQLite via WebAssembly

### Pattern

OpenCode uses `ncruces/go-sqlite3` which embeds SQLite compiled to WebAssembly (via Wazero). This means:
- **No CGO dependency**: Pure Go compilation
- **Cross-compilation works**: Build for any platform without C toolchain
- **Single binary**: SQLite is embedded in the binary
- **Slight performance cost**: Wasm execution is slower than native, but acceptable for this use case

Combined with **sqlc** for type-safe query generation and **Goose** for migrations, this creates a robust data layer without any external database dependency.

## 8. Multi-Agent Hierarchy with Session Isolation

### Pattern

The agent tool creates isolated sub-agent sessions:

```go
// Parent agent's tool call creates a sub-agent
session, _ := b.sessions.CreateTaskSession(ctx, call.ID, sessionID, "New Agent Session")
agent, _ := NewAgent(config.AgentTask, b.sessions, b.messages, TaskAgentTools(...))
done, _ := agent.Run(ctx, session.ID, params.Prompt)
```

Key design decisions:
- **Session per sub-agent**: Each sub-agent gets its own session (ID = tool call ID)
- **Read-only tools**: Sub-agents cannot modify files or run commands
- **Cost rollup**: Sub-agent costs are propagated to the parent session
- **Shared DB**: Sub-agents share the same SQLite database, enabling cost tracking
- **No communication**: Sub-agents return a single response — no back-and-forth

## 9. File Safety via Read-Before-Write

### Pattern

The edit tool enforces a "read before write" discipline:

```go
// edit.go
if getLastReadTime(filePath).IsZero() {
    return NewTextErrorResponse(
        "you must read the file before editing it. Use the View tool first")
}

modTime := fileInfo.ModTime()
lastRead := getLastReadTime(filePath)
if modTime.After(lastRead) {
    return NewTextErrorResponse(fmt.Sprintf(
        "file %s has been modified since it was last read", filePath))
}
```

This prevents:
1. **Blind writes**: The LLM must see the file content before modifying it
2. **Stale writes**: If the file changed externally since the last read, the edit is rejected
3. **Incorrect diffs**: By requiring exact string matching, the edit is verified at application time

## 10. Non-Interactive Mode

### Pattern

OpenCode supports a headless/scripting mode:

```go
func (a *App) RunNonInteractive(ctx context.Context, prompt string, ...) error {
    sess, _ := a.Sessions.Create(ctx, title)
    a.Permissions.AutoApproveSession(sess.ID)  // Skip all permission dialogs
    done, _ := a.CoderAgent.Run(ctx, sess.ID, prompt)
    result := <-done
    fmt.Println(format.FormatOutput(content, outputFormat))
    return nil
}
```

Usage:
```bash
opencode -p "Explain the use of context in Go"
opencode -p "Fix the bug in main.go" -f json -q
```

Key differences from interactive mode:
- All permissions auto-approved
- Output goes to stdout (text or JSON format)
- Optional spinner (`-q` to suppress)
- Process exits after one response

## Summary of Differentiators

| Pattern | OpenCode | Typical Agents |
|---------|----------|----------------|
| Language | Go | Python / TypeScript |
| UI | Full Bubble Tea TUI | Plain stdout |
| State management | Generic pub/sub broker | Callbacks or events |
| Provider pattern | Generic base + factory | Class inheritance |
| Permission | Channel-based blocking | Config-based allowlist |
| Database | SQLite via Wasm (pure Go) | JSON files or Postgres |
| Binary | Single static binary | Python env / npm |
| Prompts | Per-provider system prompts | Single prompt |
| Sub-agents | Session-isolated, read-only | Same process, full access |
