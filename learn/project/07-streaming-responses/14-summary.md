---
title: Summary
description: Review the complete streaming pipeline from SSE parsing through rendering and reflect on the architectural patterns used.
---

# Summary

> **What you'll learn:**
> - How all streaming components connect into an end-to-end pipeline
> - Which streaming patterns are reusable across different API providers
> - What performance and reliability characteristics to monitor in production

You have built a complete streaming pipeline that transforms your agent from a batch processor into a responsive, real-time assistant. Let's review the full architecture, revisit the key patterns, and look ahead to how streaming integrates with the rest of your agent.

## The complete pipeline

Here is the full data flow from HTTP bytes to terminal pixels, with every component you built in this chapter:

```
Anthropic API
     |
     v
HTTP Chunked Transfer (reqwest bytes_stream)
     |
     v
LineSplitter (chunks -> complete lines)
     |
     v
SseParser (lines -> SseEvent structs)
     |
     v
JSON Deserialization (SseEvent -> StreamEvent)
     |
     v
StreamStateMachine (events -> state transitions + actions)
     |
     +----> StreamAction::RenderToken ----> UiEvent::AppendText ----> FrameRenderer
     |
     +----> StreamAction::ExecuteToolCall ----> ToolCallAccumulator ----> Tool Executor
     |
     +----> StreamAction::ReportError ----> Error Recovery / Reconnection
     |
     v
StreamOutput (text, tool_calls, stop_reason)
     |
     v
Conversation History (for next agentic loop iteration)
```

Each layer has a single responsibility:

| Layer                  | Responsibility                                     | Built in                |
|------------------------|------------------------------------------------------|-------------------------|
| `LineSplitter`         | Buffer partial chunks, emit complete lines           | Chunked Transfer        |
| `SseParser`            | Parse SSE fields, dispatch on blank lines            | SSE Protocol            |
| `StreamEvent` types    | Type-safe deserialization of API events               | SSE Protocol            |
| `TokenRenderer`        | Accumulate and render text deltas                    | Token By Token Rendering|
| `ToolCallAccumulator`  | Buffer JSON fragments, assemble tool calls           | Partial Tool Call Assembly |
| `AdaptiveBuffer`       | Adjust flush frequency based on token rate           | Buffering Strategies    |
| `StreamSession`        | Manage Ctrl+C cancellation per streaming request     | Interrupt Handling      |
| Bounded `mpsc` channel | Backpressure between reader and renderer             | Backpressure            |
| `StreamStateMachine`   | Track lifecycle phases, emit actions                 | Streaming State Machine |
| Error classification   | Decide retry vs accept vs abandon                    | Error Recovery          |
| `BackoffStrategy`      | Exponential backoff with jitter for reconnection     | Reconnection            |
| `Spinner` / `StreamProgress` | User feedback during connection and streaming  | Progress Display        |
| `FrameRenderer`        | Batch UI updates at fixed frame rate                 | Real Time UI Updates    |

## Patterns worth remembering

Several patterns from this chapter appear repeatedly in systems programming. They are not specific to streaming or AI agents -- you will use them whenever you build real-time data pipelines in Rust.

### The layered parser

```
raw bytes -> lines -> structured events -> typed data
```

Each transformation layer handles exactly one concern. The `LineSplitter` does not know about SSE. The `SseParser` does not know about JSON. This separation makes each layer independently testable and replaceable. If you switch from the Anthropic API to OpenAI, you replace the JSON deserialization layer but keep everything else.

### Producer-consumer with bounded channels

```
[fast producer] ---> bounded channel ---> [slow consumer]
```

This pattern prevents unbounded memory growth when the producer (network) is faster than the consumer (terminal). The bounded channel provides natural backpressure without dropping data. You will use this same pattern for tool execution, log streaming, and file watching in later chapters.

### The state machine

```rust
enum State { A { ... }, B { ... }, C { ... } }

fn handle_event(&mut self, event: Event) -> Action {
    match (&self.state, event) {
        (State::A { .. }, Event::X) => { self.state = State::B { .. }; Action::DoSomething }
        // ...
    }
}
```

Explicit state machines replace boolean flag soup with clear, exhaustive state handling. The compiler tells you when you have forgotten a state/event combination. You will use this pattern for permission prompts (Chapter 12), connection management (Chapter 13), and plugin lifecycle (Chapter 14).

### Cooperative cancellation

```rust
tokio::select! {
    result = do_work() => handle(result),
    _ = cancel_token.cancelled() => cleanup(),
}
```

Racing work against a cancellation signal lets you stop any long-running operation cleanly. This is the async equivalent of checking a "should I stop?" flag in a loop, but it works even when the work is waiting on I/O.

::: python Coming from Python
Many of these patterns have Python equivalents, but they are typically hidden behind library abstractions:

| Rust pattern             | Python equivalent                         |
|--------------------------|-------------------------------------------|
| `LineSplitter`           | Built into `httpx-sse` / SDK internals    |
| `SseParser`              | `httpx-sse.EventSource` or SDK internals  |
| Bounded channel          | `asyncio.Queue(maxsize=N)`                |
| `CancellationToken`      | `asyncio.Event` + `KeyboardInterrupt`     |
| State machine enum       | Class with `state` string attribute       |
| Frame-based rendering    | `rich.Live(refresh_per_second=30)`        |

In Rust, you built each piece yourself. This is more work up front, but it means you understand exactly what happens at every layer and can optimize or customize any part. When something breaks at 3 AM, you know where to look.
:::

## What changed in the agentic loop

Let's compare your Chapter 3 agentic loop with the streaming-enabled version:

**Before (Chapter 3):**
```rust
loop {
    let response = call_api(&messages).await?;  // Blocks for full response
    messages.push(assistant_message(&response));

    match response.stop_reason.as_deref() {
        Some("tool_use") => {
            let results = execute_tools(&response.tool_calls).await?;
            messages.push(tool_results_message(results));
        }
        _ => break,
    }
}
```

**After (Chapter 7):**
```rust
loop {
    let session = StreamSession::new();
    let output = stream_with_progress(
        &client, &api_key, &messages,
        session.cancel_token(), true,
    ).await?;

    if session.was_interrupted() {
        messages.push(assistant_partial(&output.text));
        continue;
    }

    messages.push(assistant_message_from_output(&output));

    match output.stop_reason.as_deref() {
        Some("tool_use") => {
            for tool_call in &output.tool_calls {
                ToolProgressDisplay::tool_started(&tool_call.name);
                let start = Instant::now();
                let result = execute_tool(tool_call).await?;
                ToolProgressDisplay::tool_completed(
                    &tool_call.name, start.elapsed(), result.is_ok(),
                );
            }
            messages.push(tool_results_message(results));
        }
        _ => break,
    }
}
```

The core structure is the same -- it is still a loop that alternates between API calls and tool execution. But now responses stream in real time, the user can interrupt, progress is visible, and errors trigger reconnection instead of crashing.

## Performance characteristics

Here are the numbers you should expect and monitor:

| Metric                      | Typical value         | Investigate if           |
|-----------------------------|-----------------------|--------------------------|
| Time to first token         | 200 - 500ms           | > 2s consistently        |
| Token throughput            | 30 - 80 tokens/s      | < 10 tokens/s            |
| Frame render time           | < 1ms                 | > 16ms (frame drops)     |
| Memory per stream           | < 1MB                 | > 10MB (buffer leak)     |
| Reconnection success rate   | > 95%                 | < 80%                    |

Time to first token is the most important metric for perceived responsiveness. If it consistently exceeds 2 seconds, check whether you are sending too many tokens in the conversation history (context window pressure causes slower generation starts).

## Looking ahead

The streaming pipeline you built here is a foundation for the next several chapters:

- **Chapter 8 (Terminal UI):** The `UiEvent` bus and `FrameRenderer` pattern directly translate to a Ratatui application. You will replace the `FrameRenderer` with Ratatui's rendering loop and the `UiEvent` channel with Ratatui's event system.
- **Chapter 9 (Context Management):** The token counting you built into `StreamProgress` feeds into context window management decisions.
- **Chapter 13 (Multi-Provider):** The layered parser design makes it straightforward to support OpenAI and other providers -- you replace the JSON deserialization layer while keeping the SSE parsing, state machine, and rendering layers unchanged.

::: wild In the Wild
Production coding agents treat their streaming pipeline as critical infrastructure. Claude Code's streaming layer has been through extensive hardening -- it handles edge cases like empty content blocks, out-of-order events, and server-side mid-stream resets that you will rarely encounter in normal operation but that cause crashes in naive implementations. The investment in a robust streaming architecture pays off every day, because every single user interaction flows through this code path.
:::

## Exercises

1. **(Easy)** Add a `--no-stream` flag to your agent that falls back to the batch API for debugging. Compare the perceived latency between streaming and batch modes.

2. **(Medium)** Implement a "streaming replay" mode that records all SSE events to a file during a live session. Build a function that replays the file with realistic timing to reproduce streaming behavior without an API key.

3. **(Hard)** Build a streaming multiplexer that sends the same prompt to two different models simultaneously, streaming both responses side-by-side. Use the backpressure channel to keep both streams synchronized.

## Key Takeaways

- The streaming pipeline is a layered architecture where each component (chunking, SSE parsing, state machine, rendering) has a single responsibility and can be tested or replaced independently.
- The core patterns -- layered parsing, bounded channels, state machines, cooperative cancellation -- are general-purpose systems programming patterns that apply far beyond streaming.
- Streaming transforms the user experience by reducing perceived latency from 10+ seconds to under 500ms, enabling interruption, and providing real-time feedback.
- The streaming-enabled agentic loop retains the same structure as the batch version but adds interrupt handling, progress display, and error recovery around the streaming call.
- This pipeline is the foundation for Chapter 8's TUI integration and Chapter 13's multi-provider support.
