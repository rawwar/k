# OpenCode — Agentic Loop

## Overview

OpenCode implements a straightforward **streaming agentic loop** in `internal/llm/agent/agent.go`. The loop follows the standard pattern: send messages to the LLM, stream the response, execute any requested tool calls, append results to the conversation, and repeat until the LLM produces a final response without tool calls.

What makes OpenCode's loop notable is its clean Go implementation using channels, context cancellation, and a pub/sub system for reactive TUI updates.

## Entry Point: `agent.Run()`

The public entry point is the `Run()` method on the agent service:

```go
func (a *agent) Run(ctx context.Context, sessionID string, content string,
    attachments ...message.Attachment) (<-chan AgentEvent, error) {

    events := make(chan AgentEvent)
    if a.IsSessionBusy(sessionID) {
        return nil, ErrSessionBusy
    }

    genCtx, cancel := context.WithCancel(ctx)
    a.activeRequests.Store(sessionID, cancel)

    go func() {
        result := a.processGeneration(genCtx, sessionID, content, attachmentParts)
        a.activeRequests.Delete(sessionID)
        cancel()
        a.Publish(pubsub.CreatedEvent, result)
        events <- result
        close(events)
    }()
    return events, nil
}
```

Key design decisions:
1. **One request per session**: Uses `sync.Map` (`activeRequests`) to enforce that each session can only have one active generation at a time
2. **Cancellation support**: Stores the `context.CancelFunc` so the TUI can call `Cancel(sessionID)` to abort
3. **Async via goroutine**: The generation runs in a goroutine, returning a channel for the caller to await
4. **Panic recovery**: Uses `logging.RecoverPanic()` to prevent panics from crashing the TUI

## The Core Loop: `processGeneration()`

This is the heart of the agentic loop:

```go
func (a *agent) processGeneration(ctx context.Context, sessionID, content string,
    attachmentParts []message.ContentPart) AgentEvent {

    // 1. Load existing conversation history
    msgs, _ := a.messages.List(ctx, sessionID)

    // 2. If first message, generate title asynchronously
    if len(msgs) == 0 {
        go a.generateTitle(context.Background(), sessionID, content)
    }

    // 3. Handle summarization - if session has a summary, truncate history
    session, _ := a.sessions.Get(ctx, sessionID)
    if session.SummaryMessageID != "" {
        // Find summary message and use it as conversation start
        // Re-role it as "user" message for the LLM
        msgs = msgs[summaryMsgIndex:]
        msgs[0].Role = message.User
    }

    // 4. Create and persist the user message
    userMsg, _ := a.createUserMessage(ctx, sessionID, content, attachmentParts)
    msgHistory := append(msgs, userMsg)

    // 5. THE LOOP
    for {
        select {
        case <-ctx.Done():
            return a.err(ctx.Err())
        default:
        }

        agentMessage, toolResults, err := a.streamAndHandleEvents(
            ctx, sessionID, msgHistory)

        if (agentMessage.FinishReason() == message.FinishReasonToolUse) &&
            toolResults != nil {
            // Not done — append assistant + tool results, continue
            msgHistory = append(msgHistory, agentMessage, *toolResults)
            continue
        }

        // Done — return the final response
        return AgentEvent{
            Type:    AgentEventTypeResponse,
            Message: agentMessage,
            Done:    true,
        }
    }
}
```

### Loop Termination Conditions

The loop terminates when **any** of these conditions are met:

1. **No tool calls**: The LLM's `FinishReason` is `EndTurn` (not `ToolUse`) — the model is done
2. **Context cancelled**: User pressed Ctrl+X or the context was cancelled externally
3. **Error**: A non-recoverable error occurred during streaming or tool execution
4. **Permission denied**: If a tool's permission is denied, the loop finishes with `FinishReasonPermissionDenied`

There is **no explicit turn limit** — the loop continues as long as the model keeps requesting tool calls. This is a deliberate design choice that trusts the LLM to know when to stop.

## Streaming: `streamAndHandleEvents()`

This method handles one turn of the conversation:

```go
func (a *agent) streamAndHandleEvents(ctx context.Context, sessionID string,
    msgHistory []message.Message) (message.Message, *message.Message, error) {

    // 1. Start streaming from the provider
    ctx = context.WithValue(ctx, tools.SessionIDContextKey, sessionID)
    eventChan := a.provider.StreamResponse(ctx, msgHistory, a.tools)

    // 2. Create assistant message in DB (empty initially)
    assistantMsg, _ := a.messages.Create(ctx, sessionID, message.CreateMessageParams{
        Role:  message.Assistant,
        Parts: []message.ContentPart{},
        Model: a.provider.Model().ID,
    })

    // 3. Add message ID to context for tool use
    ctx = context.WithValue(ctx, tools.MessageIDContextKey, assistantMsg.ID)

    // 4. Process streaming events
    for event := range eventChan {
        if processErr := a.processEvent(ctx, sessionID, &assistantMsg, event); processErr != nil {
            a.finishMessage(ctx, &assistantMsg, message.FinishReasonCanceled)
            return assistantMsg, nil, processErr
        }
        if ctx.Err() != nil {
            a.finishMessage(context.Background(), &assistantMsg, message.FinishReasonCanceled)
            return assistantMsg, nil, ctx.Err()
        }
    }

    // 5. Execute tool calls SEQUENTIALLY
    toolResults := make([]message.ToolResult, len(assistantMsg.ToolCalls()))
    for i, toolCall := range assistantMsg.ToolCalls() {
        select {
        case <-ctx.Done():
            // Cancel remaining tool calls
            for j := i; j < len(toolCalls); j++ {
                toolResults[j] = message.ToolResult{
                    ToolCallID: toolCalls[j].ID,
                    Content:    "Tool execution canceled by user",
                    IsError:    true,
                }
            }
            goto out
        default:
            // Find tool by name (linear search)
            var tool tools.BaseTool
            for _, availableTool := range a.tools {
                if availableTool.Info().Name == toolCall.Name {
                    tool = availableTool
                    break
                }
            }

            if tool == nil {
                toolResults[i] = message.ToolResult{
                    ToolCallID: toolCall.ID,
                    Content:    fmt.Sprintf("Tool not found: %s", toolCall.Name),
                    IsError:    true,
                }
                continue
            }

            toolResult, toolErr := tool.Run(ctx, tools.ToolCall{
                ID: toolCall.ID, Name: toolCall.Name, Input: toolCall.Input,
            })

            // Permission denied → cancel all remaining
            if errors.Is(toolErr, permission.ErrorPermissionDenied) {
                toolResults[i] = message.ToolResult{
                    ToolCallID: toolCall.ID,
                    Content:    "Permission denied",
                    IsError:    true,
                }
                // Fill remaining with cancelled
                break
            }

            toolResults[i] = message.ToolResult{
                ToolCallID: toolCall.ID,
                Content:    toolResult.Content,
                Metadata:   toolResult.Metadata,
                IsError:    toolResult.IsError,
            }
        }
    }
    // Create tool results message and return
}
```

### Event Processing

The `processEvent()` method handles each streaming event type:

```go
func (a *agent) processEvent(ctx context.Context, sessionID string,
    assistantMsg *message.Message, event provider.ProviderEvent) error {
    switch event.Type {
    case provider.EventThinkingDelta:
        assistantMsg.AppendReasoningContent(event.Content)
        return a.messages.Update(ctx, *assistantMsg)

    case provider.EventContentDelta:
        assistantMsg.AppendContent(event.Content)
        return a.messages.Update(ctx, *assistantMsg)

    case provider.EventToolUseStart:
        assistantMsg.AddToolCall(*event.ToolCall)
        return a.messages.Update(ctx, *assistantMsg)

    case provider.EventToolUseStop:
        assistantMsg.FinishToolCall(event.ToolCall.ID)
        return a.messages.Update(ctx, *assistantMsg)

    case provider.EventComplete:
        assistantMsg.SetToolCalls(event.Response.ToolCalls)
        assistantMsg.AddFinish(event.Response.FinishReason)
        a.messages.Update(ctx, *assistantMsg)
        return a.TrackUsage(ctx, sessionID, a.provider.Model(), event.Response.Usage)

    case provider.EventError:
        return event.Error
    }
    return nil
}
```

Each event **immediately** updates the message in the database and publishes the change via pub/sub. This is what allows the TUI to show real-time streaming text — the message service publishes an `UpdatedEvent` on every delta, and the TUI subscribes to these events.

## Tool Execution Model

### Sequential, Not Parallel

OpenCode executes tool calls **sequentially** in a single `for` loop. When the LLM returns multiple tool calls in one response, they are processed one at a time. This is a simpler approach than agents like Claude Code or Copilot CLI which may execute tools in parallel. The sequential model avoids race conditions (e.g., two file edits conflicting) at the cost of latency.

### Tool Lookup

Tool dispatch is a simple linear search by name through the tool slice. There is no hashmap or registry — the tool list is small enough (~12 tools) that linear search is fine.

### Permission Blocking

When a tool requires permission (bash commands, file writes), the tool's `Run()` method calls `permission.Request()`, which **blocks the goroutine** until the user responds in the TUI:

```go
// In bash.go Run():
p := b.permissions.Request(permission.CreatePermissionRequest{
    SessionID:   sessionID,
    ToolName:    BashToolName,
    Action:      "execute",
    Description: fmt.Sprintf("Execute command: %s", params.Command),
})
if !p {
    return ToolResponse{}, permission.ErrorPermissionDenied
}
```

The flow:
1. Tool calls `permission.Request()` → publishes request via pub/sub
2. TUI receives the event → renders permission dialog
3. User presses 'a' (allow), 'A' (allow for session), or 'd' (deny)
4. Permission service sends `true`/`false` on the blocked channel
5. Tool execution continues or aborts

### Safe Read-Only Commands

Bash tool has a whitelist of "safe read-only commands" that skip permission checks:

```go
var safeReadOnlyCommands = []string{
    "ls", "echo", "pwd", "date", "cal", "uptime", "whoami", ...
    "git status", "git log", "git diff", "git show", ...
    "go version", "go help", "go list", "go env", "go doc", ...
}
```

## Cancellation Handling

Cancellation is handled at multiple levels:

1. **Session level**: `Cancel(sessionID)` loads and calls the stored `context.CancelFunc`
2. **Loop level**: Each iteration checks `ctx.Done()` before proceeding
3. **Stream level**: The streaming event loop checks `ctx.Err()` after each event
4. **Tool level**: Between tool executions, `ctx.Done()` is checked; remaining tools get "canceled" results

When cancelled mid-stream, the message is properly finalized using `context.Background()`:
```go
if ctx.Err() != nil {
    a.finishMessage(context.Background(), &assistantMsg, message.FinishReasonCanceled)
    return assistantMsg, nil, ctx.Err()
}
```

## Error Recovery and Retry

The loop has basic error handling:

- **Streaming errors**: Propagated up, causing the loop to terminate
- **Tool errors**: Returned as `ToolResult.IsError = true`, sent back to the LLM for self-correction
- **Permission denied**: Aborts remaining tools, finishes with `FinishReasonPermissionDenied`
- **Panics**: Caught by `logging.RecoverPanic()` at the goroutine level
- **Provider retries**: The provider layer has `maxRetries = 8` for API call retries

When a tool fails, the error message is fed back to the LLM as a tool result, allowing the model to self-correct:

```go
// Example: file not found
toolResults[i] = message.ToolResult{
    ToolCallID: toolCall.ID,
    Content:    "file not found: /path/to/missing.go",
    IsError:    true,
}
```

The LLM sees this in the next iteration and can adjust its approach.

## Sub-Agent Delegation

The `agent` tool creates a new agent instance for delegated search tasks:

```go
func (b *agentTool) Run(ctx context.Context, call tools.ToolCall) (tools.ToolResponse, error) {
    // Create a new task agent with read-only tools only
    agent, _ := NewAgent(config.AgentTask, b.sessions, b.messages,
        TaskAgentTools(b.lspClients))

    // Create a dedicated session for the sub-agent
    session, _ := b.sessions.CreateTaskSession(ctx, call.ID, sessionID, "New Agent Session")

    // Run the sub-agent synchronously (blocks until complete)
    done, _ := agent.Run(ctx, session.ID, params.Prompt)
    result := <-done

    // Propagate cost to parent session
    parentSession.Cost += updatedSession.Cost

    return tools.NewTextResponse(response.Content().String()), nil
}
```

Key characteristics:
- Sub-agent gets **read-only tools only**: glob, grep, ls, view, sourcegraph
- Runs through the **same agentic loop** but can't modify files
- Gets its own session (ID = tool call ID of the parent's agent tool use)
- Returns a single text response
- Is **stateless** — no multi-turn interaction with the parent
- Cost is tracked and propagated to the parent session

## Automatic Title Generation

When the first message is sent to a session, title generation runs in a separate goroutine:

```go
if len(msgs) == 0 {
    go func() {
        a.generateTitle(context.Background(), sessionID, content)
    }()
}
```

This uses a dedicated `titleProvider` configured under `agents.title` in the config. It runs independently from the main loop and updates the session title via the session service, which triggers a pub/sub event for the TUI.

## Conversation Summarization

The `Summarize()` method compacts a long conversation into a summary:

1. Loads all messages from the session
2. Appends a summarization prompt asking for key details
3. Sends to the dedicated `summarizeProvider` (synchronous, no streaming)
4. Creates a summary message in the same session
5. Sets `SummaryMessageID` on the session

On the next `processGeneration()` call, messages before the summary are **truncated**, and the summary message is re-roled as a "user" message. This effectively restarts the conversation from the summary while preserving context.

## Sequence Diagram

```
User       TUI         Agent         Provider      Tool         Permission
  │         │            │              │            │              │
  │─type──▶│            │              │            │              │
  │         │──Run()───▶│              │            │              │
  │         │            │──StreamResp─▶│            │              │
  │         │            │◀──events─────│            │              │
  │◀─render─│◀─pubsub───│              │            │              │
  │         │            │              │            │              │
  │         │            │──tool.Run()─────────────▶│              │
  │         │            │              │            │──Request()──▶│
  │◀──permission dialog──│              │            │              │
  │──allow──▶│           │              │            │◀─grant──────│
  │         │            │◀──ToolResponse──────────│              │
  │         │            │              │            │              │
  │         │            │──StreamResp─▶│ (loop)    │              │
  │         │            │◀──events─────│            │              │
  │◀─render─│◀─pubsub───│              │            │              │
  │         │            │──Done────────▶            │              │
```

## Comparison with Other Agents

| Feature | OpenCode | Claude Code | Aider |
|---------|----------|-------------|-------|
| Tool execution | Sequential | Parallel | Sequential |
| Turn limit | None | Configurable | None |
| Cancellation | Context-based | SIGINT | SIGINT |
| Sub-agents | Yes (read-only) | No | No |
| Permission model | Per-action, per-session | Allowlist-based | Trust-based |
| Error recovery | Feed back to LLM | Feed back + retry | Feed back to LLM |
| Streaming | Token-by-token via pub/sub | Token-by-token | Full responses |
