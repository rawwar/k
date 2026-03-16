---
title: Real World Implementations
description: How Claude Code, OpenCode, and other production agents implement their agentic loops in practice, with code-level analysis.
---

# Real World Implementations

> **What you'll learn:**
> - How Claude Code's agentic loop handles streaming, tool dispatch, and permission checks in its TypeScript implementation
> - How OpenCode structures its Go-based loop with provider abstraction and tool middleware
> - The common implementation patterns and divergences across real-world agent codebases

Theory is essential, but seeing how production agents actually implement their loops grounds the abstractions in reality. In this subchapter, we examine how two prominent open-source coding agents -- Claude Code (TypeScript) and OpenCode (Go) -- build their agentic loops. You will see the same state machine patterns from earlier in this chapter, but adapted to real-world constraints: streaming protocols, concurrent operations, error recovery, and user experience.

## Claude Code: The TypeScript Implementation

Claude Code is Anthropic's official CLI coding agent. Its agentic loop follows the basic tool loop pattern we described, with streaming, permission checks, and a rich tool system layered on top.

### Loop Structure

Claude Code's core loop lives in its agent module. Simplified to its essential structure, it follows this pattern:

```typescript
// Simplified pseudocode based on Claude Code's architecture
async function agentLoop(
  messages: Message[],
  tools: Tool[],
  systemPrompt: string,
): Promise<string> {
  while (true) {
    // Call the LLM with streaming
    const response = await callAnthropicApi({
      model: "claude-sonnet-4-20250514",
      system: systemPrompt,
      messages: messages,
      tools: tools.map(t => t.definition),
      stream: true,
    });

    // Process the streamed response
    // Text is displayed in real-time as it arrives
    // Tool calls are accumulated from the stream

    if (response.stopReason === "end_turn") {
      return response.text;
    }

    // Process tool calls
    for (const toolCall of response.toolCalls) {
      // Permission check
      const approved = await checkPermission(toolCall);
      if (!approved) {
        messages.push(toolResultMessage(toolCall.id, "Permission denied by user"));
        continue;
      }

      // Execute the tool
      const result = await executeTool(toolCall);

      // Add result to messages
      messages.push(toolResultMessage(toolCall.id, result));
    }
  }
}
```

The structure matches our state machine model exactly: Processing (LLM call) -> check stop reason -> either Done (end_turn) or ToolDetected (tool_use) -> ToolExecuting -> ObservationReady -> back to Processing.

### Key Design Decisions

**Streaming first.** Claude Code always uses streaming. Text tokens appear in the terminal immediately, and tool calls are accumulated from the stream. The response assembler buffers partial JSON for tool call parameters and only parses them after the content block completes. This is the same pattern we described in the LLM Invocation subchapter.

**Permission tiers.** Tools are categorized into permission levels. Read-only tools (file reads, directory listings, grep) execute immediately. Write tools (file edits, shell commands) require user approval. The user can grant per-command approval ("allow this specific command"), pattern approval ("allow all cargo commands"), or session-wide approval. This tiered approach balances safety with usability.

**Tool result formatting.** File contents are returned with line numbers. Command output includes exit codes and separates stdout from stderr. Large outputs are truncated with clear markers indicating how much was omitted. Error messages include hints about what the model should try instead.

::: tip In the Wild
Claude Code's permission system remembers approvals within a session. If you approve `cargo test` once, subsequent `cargo test` calls execute without asking. But a different command like `cargo build` requires separate approval. This session memory significantly reduces the friction of working with an agent that makes many tool calls, while still catching unexpected or dangerous commands.
:::

### Error Recovery

Claude Code implements retry logic for API errors with exponential backoff. Rate limit responses (HTTP 429) are handled by waiting for the `retry-after` duration. Server errors trigger a backoff sequence. Context window overflow triggers automatic conversation compaction.

For tool errors, the error is formatted as a tool result with `is_error: true` and sent back to the model. The model typically self-corrects by trying a different approach. If three consecutive tool calls fail with the same error, Claude Code surfaces the issue to the user rather than letting the model keep trying.

## OpenCode: The Go Implementation

OpenCode is an open-source terminal-based coding agent written in Go. Its architecture shares the same fundamental pattern but makes different choices in several areas.

### Loop Structure

OpenCode's agent loop is structured around a provider abstraction that supports multiple LLM backends (Anthropic, OpenAI, and others). The core loop, simplified:

```go
// Simplified pseudocode based on OpenCode's architecture
func (a *Agent) Run(ctx context.Context, userMessage string) (string, error) {
    a.history = append(a.history, UserMessage(userMessage))

    for iteration := 0; iteration < a.maxIterations; iteration++ {
        // Call the LLM through the provider abstraction
        response, err := a.provider.Chat(ctx, ChatRequest{
            Messages: a.history,
            Tools:    a.tools.Definitions(),
        })
        if err != nil {
            return "", fmt.Errorf("LLM call failed: %w", err)
        }

        a.history = append(a.history, response.AssistantMessage)

        // Check if the model is done
        if response.StopReason == "end_turn" {
            return response.Text, nil
        }

        // Execute tool calls
        for _, call := range response.ToolCalls {
            result := a.tools.Execute(ctx, call.Name, call.Input)
            a.history = append(a.history, ToolResultMessage(call.ID, result))
        }
    }

    return "", fmt.Errorf("max iterations (%d) exceeded", a.maxIterations)
}
```

### Key Design Decisions

**Provider abstraction.** OpenCode supports multiple LLM providers through an interface. The agent loop does not call the Anthropic API directly -- it calls a `Provider` that could be Anthropic, OpenAI, Google, or any other LLM backend. This means the same loop handles different API formats, streaming protocols, and error codes:

```go
// OpenCode's provider interface (simplified)
type Provider interface {
    Chat(ctx context.Context, req ChatRequest) (*ChatResponse, error)
    Stream(ctx context.Context, req ChatRequest) (<-chan StreamEvent, error)
}
```

This is a significant architectural difference from Claude Code, which is tightly coupled to the Anthropic API. The trade-off: OpenCode gets multi-provider support but has to maintain a lowest-common-denominator abstraction that may not leverage provider-specific features.

**Tool middleware.** OpenCode wraps tool execution in a middleware chain. Each middleware can modify the tool call before execution or the result after execution. This is used for logging, permission checks, output formatting, and safety filters:

```go
// OpenCode's tool middleware pattern (simplified)
type ToolMiddleware func(next ToolHandler) ToolHandler

func LoggingMiddleware(next ToolHandler) ToolHandler {
    return func(ctx context.Context, name string, input json.RawMessage) (string, error) {
        log.Printf("Executing tool: %s", name)
        result, err := next(ctx, name, input)
        log.Printf("Tool %s completed: err=%v", name, err)
        return result, err
    }
}
```

::: python Coming from Python
The middleware pattern is common in Python web frameworks (Django middleware, Flask's `before_request`/`after_request`). OpenCode applies the same concept to tool execution. If you have written Python middleware, OpenCode's tool middleware will feel familiar. In Rust, you would implement this pattern using either trait objects (dynamic dispatch) or generics with the decorator pattern.
:::

**TUI integration.** OpenCode uses a terminal UI library (Bubble Tea) that runs the agent loop in a background goroutine while the UI runs in the main goroutine. Messages between them flow through Go channels. This means the loop must be designed for concurrency -- tool results, streaming text, and UI events all happen asynchronously.

### Error Recovery

OpenCode implements per-provider error handling. Each provider knows its specific error codes and retry strategies. The agent loop itself has a simpler error model: if the provider returns an error after internal retries, the loop propagates it to the user. Tool errors are wrapped in tool results, similar to Claude Code.

## Common Patterns Across Implementations

Despite the language differences (TypeScript vs. Go) and architectural differences (single-provider vs. multi-provider), both agents share these patterns:

### 1. The Loop Is Simpler Than You Expect

Both core loops are surprisingly small -- 50 to 100 lines of actual control flow logic. The complexity lives in the components the loop calls (streaming parser, tool system, permission system, context management), not in the loop itself. This validates the state machine model: the loop is a simple cycle through well-defined states.

### 2. History Is the Central Data Structure

Both agents build their entire state around the conversation history. The history is the single source of truth for what has happened, and every LLM call receives the full history. There is no separate "action log" or "tool result cache" -- everything flows through the message array.

### 3. Streaming Is Non-Negotiable

Both agents stream LLM responses. Neither offers a non-streaming mode in their production builds. The latency of waiting for complete responses is simply not acceptable for interactive use. Streaming adds complexity to the response parsing, but the user experience improvement is worth it.

### 4. Error Recovery Is Model-Driven

Both agents rely heavily on the model to recover from errors. When a tool fails, the error is formatted as a tool result and sent back to the model. The model decides what to do next -- try again, try something different, or report the failure to the user. The agent code sets boundaries (retry limits, error budgets) but does not prescribe specific recovery actions.

### 5. Permission Systems Are Tool-Specific

Both agents implement per-tool permission policies. Read-only tools run freely. Write tools require approval with various levels of persistence (per-call, per-pattern, per-session). Neither agent uses a single global "allow all tools" switch -- the granularity is always at the tool level.

## Mapping to Our State Machine

Let's map both implementations to our state machine model:

| State Machine State | Claude Code | OpenCode |
|---------------------|-------------|----------|
| Idle | REPL prompt waiting for input | TUI input box |
| Processing | `callAnthropicApi()` with streaming | `provider.Chat()` or `provider.Stream()` |
| ToolDetected | Loop over `response.toolCalls` | Loop over `response.ToolCalls` |
| ToolExecuting | `executeTool()` with permission check | `tools.Execute()` with middleware chain |
| ObservationReady | Push `toolResultMessage` to history | Append `ToolResultMessage` to history |
| Done | Return response text, show in terminal | Return text, update TUI |
| Error | Retry with backoff or surface to user | Propagate provider error or surface to user |

The mapping is direct. Despite different languages, different UI paradigms, and different levels of abstraction, the underlying state machine is identical. This is what makes the state machine model so valuable -- it describes the essential structure that all agent implementations share.

## Lessons for Our Rust Implementation

From studying these implementations, several lessons emerge for the Rust agent we will build:

1. **Keep the loop simple.** Push complexity into well-defined components (tool system, streaming parser, permission handler). The loop itself should be readable at a glance.

2. **Make the history the single source of truth.** Do not maintain separate state that could get out of sync with the history.

3. **Build streaming from the start.** It is much harder to add streaming to a non-streaming loop than to build it streaming from the beginning.

4. **Design the tool system for extensibility.** Whether you use a registry (OpenCode) or a match statement (Claude Code uses both), make it easy to add new tools without modifying the core loop.

5. **Let the model drive error recovery.** Your code provides the boundaries (retry limits, error budgets, permission checks), but the model decides the recovery strategy.

## Key Takeaways

- Claude Code implements a basic tool loop in TypeScript with streaming, tiered permissions, and session-level approval memory -- its core loop is under 100 lines of control flow
- OpenCode implements the same loop pattern in Go with a provider abstraction for multi-LLM support and a middleware chain for tool execution hooks
- Both agents share five common patterns: simple core loops, history as central data structure, mandatory streaming, model-driven error recovery, and tool-specific permissions
- The state machine model from earlier in this chapter maps directly to both implementations -- Idle, Processing, ToolDetected, ToolExecuting, ObservationReady, Done, and Error are present in both
- For our Rust implementation, the key lessons are: keep the loop simple, make history the source of truth, build streaming from the start, design tools for extensibility, and let the model drive error recovery
