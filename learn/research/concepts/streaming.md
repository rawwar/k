# Streaming

## Overview

Streaming is the mechanism that makes coding agents feel responsive. Without streaming, the user stares at a blank screen while the model generates its entire response before any text appears. With streaming, tokens appear incrementally, giving immediate feedback and allowing the user to read as the response forms. Streaming also enables detecting tool calls as they arrive and preparing for execution before the full response is complete.

The dominant protocol for LLM streaming is Server-Sent Events (SSE) over HTTP. Every major LLM API (Anthropic, OpenAI, Google) uses SSE to deliver incremental responses.

## The Pattern

An LLM streaming pipeline has four stages connected in sequence: the HTTP connection, the SSE parser, the application event processor, and the renderer.

**The HTTP connection** is established as a POST request with `"stream": true`. The server responds with `Content-Type: text/event-stream` and sends SSE events over a long-lived connection using chunked transfer encoding. Data arrives in arbitrary-sized byte chunks that do not align with event boundaries -- a single event might split across chunks, or one chunk might contain several events.

**The SSE parser** transforms raw bytes into structured events. The SSE wire format is line-based: each event consists of one or more fields (`data:`, `event:`, `id:`, `retry:`) followed by a blank line that triggers dispatch. Multiple `data:` lines within a single event are concatenated with newlines. Comment lines (starting with `:`) are ignored and often used as heartbeat keepalives. The parser must handle events split across chunk boundaries by maintaining a line buffer that accumulates partial data between chunks. In Rust, this is implemented as a state machine with an `EventBuilder` that processes lines one at a time and emits complete `SseEvent` structs when blank lines are encountered.

**The application event processor** interprets LLM-specific event types. Anthropic's API uses a lifecycle: `message_start`, then content blocks (each with `start`, `delta` events carrying incremental text or JSON fragments, and `stop`), then `message_delta` (with usage and stop reason), and `message_stop`. Text blocks carry `text_delta` events; tool use blocks carry `input_json_delta` events with JSON argument fragments. The processor accumulates fragments and signals when a complete tool call is available.

**The renderer** displays tokens to the user as they arrive -- printing text deltas immediately, showing progress indicators for accumulating tool calls, then transitioning to execution status. The display must remain coherent even when events arrive in bursts after a network stall.

## Implementation Approaches

**SSE parsing in Rust** requires explicit buffer management. The parser maintains a line buffer; incoming byte chunks are appended to it, then scanned for newlines. Complete lines are processed through the event builder; partial lines remain in the buffer until the next chunk completes them. This correctly handles events split across HTTP chunks. BOM stripping is applied once at the stream's start. Integration with `reqwest` is straightforward: iterate over `response.bytes_stream()`, feed each chunk into the parser, and collect emitted events.

**Handling tool_use events in streams** is more complex than text. When `content_block_start` arrives with `type: "tool_use"`, the agent accumulates JSON arguments from subsequent `input_json_delta` events. These deltas carry JSON fragments that cannot be parsed until `content_block_stop` signals completion. The agent then deserializes the full JSON, validates it, and dispatches the tool call. Tool execution cannot begin until the model finishes generating the entire call, even though text tokens may already be rendering.

**Backpressure and buffering** prevent memory exhaustion when the producer is faster than the consumer. Bounded `tokio::sync::mpsc` channels with capacities of 16-64 events provide the right balance. When a channel fills, the sender's `await` pauses the producer, propagating backpressure to TCP flow control -- the network reader stops reading, the TCP buffer fills, and the server slows transmission. For metadata like token counts, `tokio::sync::watch` channels provide "latest value" semantics where intermediate values are safely overwritten.

**Multi-stage pipeline architecture** connects network reading, SSE parsing, and rendering as independent async tasks joined by bounded channels. Backpressure propagates automatically: a slow renderer fills the parser channel, which stalls the parser, which fills the network channel, which pauses the reader. Claude Code takes an alternative approach, processing events synchronously within a single async context, simplifying flow control at the cost of less parallelism.

**Reconnection and resilience** handle long-running streams that drop. The SSE `id` field and `Last-Event-ID` header enable stream resumption; the `retry` field specifies reconnection delay. While not all LLM APIs support mid-stream resumption, implementing ID tracking is low-cost insurance.

## Key Considerations

**First-token latency defines perceived responsiveness.** Users judge an agent's speed by how quickly the first token appears, not by total generation time. Streaming reduces perceived latency from the full response time (often 5-30 seconds) to the first-token time (typically under 1 second). This psychological effect is disproportionately important for user experience.

**Chunked transfer encoding is not SSE.** HTTP chunked encoding delivers bytes in pieces; SSE is the application protocol on top that gives those bytes structure. Chunk boundaries do not align with event boundaries, so treating each HTTP chunk as a complete SSE event will produce parse errors.

**Partial JSON is unavoidable for tool calls.** The agent receives JSON fragments that are syntactically invalid until the stream completes. Accumulate fragments in a buffer and parse only when `content_block_stop` signals completeness.

**Interrupt handling interacts with streaming.** When the user presses Ctrl+C, the agent must cancel the HTTP connection and unwind the pipeline. Dropping a channel receiver propagates cancellation through all stages. The agent should preserve whatever partial response has already been appended to the conversation history.

**Token counting from streams requires care.** The `message_start` and `message_delta` events carry authoritative usage statistics. Always prefer the API's reported usage over local estimates based on character counting.

## Cross-References
- [Server-Sent Events](/linear/08-streaming-and-realtime/03-server-sent-events) -- The SSE wire format and protocol specification
- [Parsing SSE in Rust](/linear/08-streaming-and-realtime/05-parsing-sse-in-rust) -- Building a streaming SSE parser with buffer management
- [Incremental Rendering](/linear/08-streaming-and-realtime/07-incremental-rendering) -- Displaying tokens as they arrive
- [Backpressure and Flow Control](/linear/08-streaming-and-realtime/08-backpressure-and-flow-control) -- Bounded channels and pipeline architecture
- [Partial JSON Handling](/linear/08-streaming-and-realtime/06-partial-json-handling) -- Accumulating tool call arguments from stream fragments
- [Interrupt and Cancel](/linear/08-streaming-and-realtime/09-interrupt-and-cancel) -- Handling user cancellation during streaming
