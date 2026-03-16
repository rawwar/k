---
title: Summary
description: Review of streaming protocols, parsing techniques, and real-time rendering patterns with a look ahead to terminal UI integration in the next chapter.
---

# Summary

> **What you'll learn:**
> - How the streaming concepts from this chapter -- SSE parsing, backpressure, incremental rendering -- form the foundation for the terminal UI in Chapter 9
> - Which streaming patterns are universal across LLM providers and which are provider-specific implementation details
> - Key architectural decisions for streaming pipelines and their impact on agent responsiveness and reliability

You have traveled from the raw bytes on the wire to a fully-architected event-driven streaming pipeline. This chapter covered a lot of ground, and it is worth stepping back to see how the pieces fit together and how they prepare you for what comes next.

## The Complete Picture

Let's trace a token's journey from the LLM server to the user's screen, referencing every layer you built in this chapter:

**1. The LLM generates a token** and the server wraps it in an SSE event:
```
event: content_block_delta
data: {"type":"content_block_delta","index":0,"delta":{"type":"text_delta","text":"Hello"}}
```

**2. Chunked transfer encoding** ([subchapter 4](/linear/08-streaming-and-realtime/04-chunked-encoding)) delivers this as part of an HTTP response body. The hex size prefix and CRLF delimiters are stripped by your HTTP library (reqwest/hyper), giving you raw bytes.

**3. Your SSE parser** ([subchapter 5](/linear/08-streaming-and-realtime/05-parsing-sse-in-rust)) feeds these bytes into a line buffer, splits on newlines, processes fields, and dispatches a complete `SseEvent` when it encounters a blank line.

**4. The JSON payload is parsed.** For text deltas, this is straightforward -- the `data` field contains valid JSON. For tool call deltas, the **partial JSON accumulator** ([subchapter 6](/linear/08-streaming-and-realtime/06-partial-json-handling)) collects fragments and speculatively completes them to extract field values early.

**5. The event flows through bounded channels** ([subchapter 8](/linear/08-streaming-and-realtime/08-backpressure-and-flow-control)) with backpressure ensuring that the network reader pauses if the renderer falls behind.

**6. The renderer** ([subchapter 7](/linear/08-streaming-and-realtime/07-incremental-rendering)) displays the token, using a **buffering strategy** ([subchapter 11](/linear/08-streaming-and-realtime/11-buffering-patterns)) that balances latency against throughput -- perhaps time-based flushing at 50ms intervals.

**7. If the connection drops**, the **reconnection logic** ([subchapter 10](/linear/08-streaming-and-realtime/10-reconnection-strategies)) kicks in with exponential backoff and jitter, preserving accumulated text and resuming the stream.

**8. If the user presses Ctrl+C**, the **cancellation system** ([subchapter 9](/linear/08-streaming-and-realtime/09-interrupt-and-cancel)) propagates a `CancellationToken` through every pipeline stage, dropping the HTTP connection (stopping token generation and billing) while preserving what was already rendered.

**9. All of this is coordinated** by the **event-driven architecture** ([subchapter 12](/linear/08-streaming-and-realtime/12-event-driven-architecture)), where every component subscribes to the events it needs and operates independently.

## What Is Universal vs. Provider-Specific

As you build your agent, it helps to know which parts of the streaming stack are universal and which are tied to a specific LLM provider:

### Universal (implement once, use everywhere)

- **SSE parsing.** The SSE protocol is standardized (W3C specification). Your parser works with any SSE server, regardless of the LLM provider.
- **Chunked transfer encoding.** This is HTTP standard. Your HTTP library handles it.
- **Backpressure and flow control.** Bounded channels and TCP flow control work the same regardless of what data flows through them.
- **Cancellation.** CancellationToken patterns and Ctrl+C handling are application-level concerns independent of the API.
- **Buffering strategies.** Time-based, size-based, and adaptive buffering are universal rendering techniques.
- **Reconnection with exponential backoff.** The retry pattern is the same for any HTTP endpoint.

### Provider-Specific (varies between APIs)

- **Event types.** Anthropic uses `message_start`, `content_block_delta`, `message_stop`. OpenAI uses `[DONE]` as a stream terminator and different delta structures. Google Gemini has its own event format.
- **Tool call streaming format.** Anthropic streams tool call arguments as `partial_json` deltas. OpenAI streams them as `function` deltas with different field names.
- **Usage reporting.** Anthropic includes token usage in `message_delta` events. OpenAI includes it in the final chunk. Google includes it in the response metadata.
- **Error formats.** Each provider has different error response structures and HTTP status code conventions.

A well-designed agent isolates the provider-specific code behind a trait:

```rust
pub trait StreamEventMapper {
    /// Map a provider-specific SSE event into a universal agent event.
    fn map_event(&mut self, sse: &SseEvent) -> Option<AgentEvent>;
}

pub struct AnthropicMapper {
    tool_accumulator: Option<ToolCallAccumulator>,
}

impl StreamEventMapper for AnthropicMapper {
    fn map_event(&mut self, sse: &SseEvent) -> Option<AgentEvent> {
        let data: serde_json::Value = serde_json::from_str(&sse.data).ok()?;

        match sse.event_type() {
            "content_block_delta" => {
                let delta = data.get("delta")?;
                match delta.get("type")?.as_str()? {
                    "text_delta" => {
                        let text = delta.get("text")?.as_str()?;
                        Some(AgentEvent::TextDelta {
                            text: text.to_string(),
                        })
                    }
                    "input_json_delta" => {
                        let json = delta.get("partial_json")?.as_str()?;
                        // Handle tool call delta...
                        None
                    }
                    _ => None,
                }
            }
            "message_stop" => {
                Some(AgentEvent::StreamCompleted {
                    stop_reason: "end_turn".to_string(),
                    input_tokens: 0,
                    output_tokens: 0,
                })
            }
            _ => None,
        }
    }
}

// Types referenced from earlier subchapters
struct SseEvent {
    event_type: Option<String>,
    data: String,
}

impl SseEvent {
    fn event_type(&self) -> &str {
        self.event_type.as_deref().unwrap_or("message")
    }
}

#[derive(Debug, Clone)]
enum AgentEvent {
    TextDelta { text: String },
    StreamCompleted { stop_reason: String, input_tokens: u64, output_tokens: u64 },
}

struct ToolCallAccumulator;
```

::: python Coming from Python
Python SDK libraries like `anthropic` and `openai` provide high-level streaming abstractions that hide the SSE parsing and event mapping:
```python
with client.messages.stream(model="claude-sonnet-4-20250514", messages=messages) as stream:
    for text in stream.text_stream:
        print(text, end="")
```
In Rust, you typically build these abstractions yourself (or use a lower-level client library). This is more work upfront but gives you complete control over buffering, backpressure, and error handling -- control that matters when you are building a production agent rather than a script.
:::

## Architectural Decisions Recap

Here are the key decisions you made in this chapter and their trade-offs:

| Decision | Choice | Trade-off |
|----------|--------|-----------|
| Streaming protocol | SSE over HTTP | Simple and universal, but unidirectional |
| SSE parser | Custom line-based state machine | Full control over allocation, but more code than a library |
| Partial JSON | Speculative completion | Early field extraction, but some false positives on truncated values |
| Backpressure | Bounded mpsc channels (16-64 capacity) | Smooth flow control, but requires capacity tuning |
| Cancellation | CancellationToken + select! | Cooperative and explicit, but must be checked at every await point |
| Reconnection | Exponential backoff with equal jitter | Good retry spread, but adds latency on first retry |
| Buffering | Time-based with content triggers | Smooth output, but adds up to 50ms latency |
| Architecture | Event-driven with broadcast channels | Clean separation, but more indirection than procedural |

None of these choices are the only correct ones, but they represent a well-balanced set of trade-offs for a CLI coding agent.

## What Comes Next

Chapter 9 takes the streaming infrastructure you built here and connects it to a terminal user interface. You will use a TUI library to render streaming content in structured layouts: a response panel that shows text as it arrives, a status bar with token counts and timing, and a tool execution panel that displays progress. The event-driven architecture from [subchapter 12](/linear/08-streaming-and-realtime/12-event-driven-architecture) becomes the backbone of the TUI -- each UI component subscribes to the events it needs and re-renders when they arrive.

The backpressure and buffering work you did in this chapter directly applies to TUI rendering. A TUI typically renders at 60 frames per second, so you need to batch token arrivals into frame-sized updates. The timed flush strategy from [subchapter 11](/linear/08-streaming-and-realtime/11-buffering-patterns) maps directly to TUI frame timing.

The cancellation infrastructure from [subchapter 9](/linear/08-streaming-and-realtime/09-interrupt-and-cancel) becomes the foundation for TUI-specific interrupts: pressing Escape to cancel a stream, pressing a key to scroll back through the response, or pressing Ctrl+C to exit the application entirely.

::: wild In the Wild
Both Claude Code and OpenCode build their TUI directly on top of their streaming event system. Claude Code's terminal rendering receives the same `AgentEvent` stream that the state manager processes, ensuring that what the user sees is always in sync with the internal conversation state. This architecture -- a single event stream feeding multiple independent consumers -- is the pattern you have built in this chapter and will extend in the next.
:::

## Chapter Checklist

Before moving on, verify you understand:

- [ ] Why TTFT matters more than total response time for perceived responsiveness
- [ ] How SSE events are structured (data, event, id, retry fields) and delimited (blank lines)
- [ ] How to parse SSE streams in Rust with correct handling of multi-line data and chunk boundaries
- [ ] Why tool call arguments arrive as partial JSON and how speculative completion extracts early field values
- [ ] How bounded channels provide backpressure between pipeline stages
- [ ] How CancellationToken coordinates multi-stage cancellation
- [ ] The exponential backoff with jitter pattern for reconnection
- [ ] When to use event-driven architecture versus a simple procedural loop

## Key Takeaways

- A streaming pipeline has **five layers** -- network, protocol (SSE), data (JSON), application (state), and rendering -- each with distinct concerns and buffering characteristics.
- **SSE parsing, backpressure, cancellation, and buffering** are universal infrastructure that works across all LLM providers. Event type mapping and error handling are provider-specific.
- The **event-driven architecture** built in this chapter becomes the backbone of the terminal UI in Chapter 9, with each UI component subscribing to the events it renders.
- Isolate provider-specific streaming logic behind a **`StreamEventMapper` trait** so your pipeline works with any LLM API without modifying the core infrastructure.
- The streaming infrastructure is among the most user-visible code in your agent -- **TTFT, smooth rendering, and instant cancellation** are what make the difference between an agent that feels professional and one that feels like a prototype.
