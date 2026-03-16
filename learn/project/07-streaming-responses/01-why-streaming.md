---
title: Why Streaming
description: Understand the user experience and technical motivations for streaming AI responses instead of waiting for complete responses.
---

# Why Streaming

> **What you'll learn:**
> - Why batch responses create unacceptable latency in interactive coding agents
> - How streaming reduces perceived latency and enables early user feedback
> - What architectural changes are needed to support streaming in an agentic system

Up to this point, your agent sends a request to the Anthropic API and waits -- sometimes for ten, twenty, or even thirty seconds -- until the entire response is assembled on the server and delivered in one shot. During that wait, your user stares at a blinking cursor with zero feedback. They do not know if the agent is thinking hard, stuck in a loop, or if the network dropped. This chapter fixes that. You are going to make your agent stream responses token-by-token, just like ChatGPT, Claude, and every other modern AI interface.

## The batch response problem

Let's look at what happens with the non-streaming approach you built in Chapter 2. Your agent calls the Anthropic Messages API, and the server generates the entire response before sending anything back:

```
User sends prompt ──> Server generates 500 tokens ──> Full response arrives
                      [~10-15 seconds of silence]
```

For a 500-token response at roughly 30-50 tokens per second, the user waits the full generation time before seeing a single character. In a coding agent context, this is especially painful because:

1. **Responses are often long.** Code explanations, multi-file edits, and command sequences can run to thousands of tokens. A 2000-token response might take 40+ seconds.
2. **Users need early signal.** If the agent is heading in the wrong direction, the user wants to interrupt immediately -- not after waiting half a minute for a response they will discard.
3. **Tool calls compound the wait.** In an agentic loop, the model might generate a response that includes a tool call. Without streaming, you wait for the full response, execute the tool, send the result back, and wait again for another full response. Each round trip adds dead time.

## What streaming gives you

With streaming, the server starts sending tokens as soon as it generates them. The first token typically arrives within 200-500 milliseconds:

```
User sends prompt ──> First token (200ms) ──> tokens flow continuously ──> Done
                      [User sees output immediately]
```

This changes the user experience in three fundamental ways:

**Perceived latency drops by an order of magnitude.** Instead of waiting 15 seconds for a batch response, the user sees the first word in under a second. The psychological difference is enormous -- the agent feels alive and responsive.

**Users can interrupt early.** If you see the agent writing code that uses the wrong library or taking an approach you disagree with, you can press Ctrl+C two seconds into the response instead of waiting for the whole thing. This saves both time and API costs.

**Tool calls start sooner.** When you stream, you can detect a tool call as soon as the model starts emitting its JSON arguments. While you still need the complete JSON to execute the tool, you know a tool call is coming and can prepare for it. More importantly, the text content before a tool call displays immediately.

::: python Coming from Python
In Python, the difference between batch and streaming is often just adding `stream=True` to your API call:
```python
# Batch (blocking, waits for full response)
response = client.messages.create(model="claude-sonnet-4-20250514", messages=messages, max_tokens=1024)

# Streaming (yields chunks as they arrive)
with client.messages.stream(model="claude-sonnet-4-20250514", messages=messages, max_tokens=1024) as stream:
    for text in stream.text_stream:
        print(text, end="", flush=True)
```
The Python SDK hides the SSE parsing, HTTP chunking, and buffering behind a clean iterator. In Rust, you will build each of those layers yourself, which gives you much finer control over performance and error handling.
:::

## Architectural implications

Streaming is not just a UI improvement -- it requires rethinking several parts of your agent's architecture. Here is what changes:

### The HTTP layer

Your current `reqwest` call uses `.json()` on the response, which buffers the entire body into memory before deserializing. For streaming, you switch to reading the response body as a `Stream` of bytes. You will need to handle:

- **Chunked transfer encoding** -- the HTTP mechanism that lets the server send data in pieces.
- **SSE parsing** -- the Server-Sent Events format that the Anthropic API uses to structure streamed data.
- **Partial data** -- chunks can arrive at arbitrary byte boundaries, splitting events mid-line.

### The agentic loop

Your loop from Chapter 3 currently looks like this in pseudocode:

```rust
loop {
    let response = call_api(&messages).await?;  // blocks until complete
    match response.stop_reason {
        StopReason::ToolUse => {
            execute_tools(&response);
            messages.push(tool_results);
        }
        StopReason::EndTurn => break,
    }
}
```

With streaming, the loop needs to process events as they arrive rather than waiting for a complete response. The stop reason is not known until the stream ends, so you accumulate content blocks incrementally:

```rust
loop {
    let mut stream = start_stream(&messages).await?;
    let mut accumulated = StreamAccumulator::new();

    while let Some(event) = stream.next().await {
        match event? {
            StreamEvent::ContentDelta(delta) => {
                print_token(&delta.text);
                accumulated.add_delta(delta);
            }
            StreamEvent::MessageStop => break,
        }
    }

    let response = accumulated.into_message();
    match response.stop_reason {
        StopReason::ToolUse => { /* same as before */ }
        StopReason::EndTurn => break,
    }
}
```

### Content accumulation

With batch responses, you get a complete `Message` struct with fully-formed content blocks. With streaming, you receive a sequence of deltas -- small text fragments and partial JSON chunks -- that you must assemble back into the same `Message` structure. This accumulation logic is the heart of what you will build in this chapter.

### Error handling

Batch requests either succeed or fail. Streaming introduces a third possibility: partial success. The stream might deliver 80% of a response before the connection drops. You need strategies for preserving what you received and deciding whether to retry.

## The plan for this chapter

Here is the roadmap for what you will build:

1. **SSE protocol** -- understand the wire format and parse raw event streams.
2. **Chunked transfer** -- handle HTTP chunks that do not align with event boundaries.
3. **Token rendering** -- display text deltas as they arrive.
4. **Tool call assembly** -- buffer partial JSON fragments into complete tool calls.
5. **Buffering strategies** -- tune the balance between responsiveness and efficiency.
6. **Interrupt handling** -- let users cancel mid-stream with Ctrl+C.
7. **Backpressure** -- prevent memory blowout when tokens arrive faster than the terminal can render.
8. **State machine** -- model the full streaming lifecycle with Rust enums.
9. **Error recovery** -- handle mid-stream failures gracefully.
10. **Reconnection** -- retry dropped connections with exponential backoff.
11. **Progress display** -- show spinners, token counts, and timing.
12. **Real-time UI** -- integrate streaming with the terminal display layer.

Each piece builds on the last. By the end of this chapter, your agent will feel as responsive as any commercial coding assistant.

::: wild In the Wild
Every production coding agent uses streaming. Claude Code streams all responses and uses the streaming events to drive its terminal UI in real time -- tool call indicators appear as soon as the model starts generating a tool_use block, not when it finishes. OpenCode similarly streams all API responses, parsing SSE events to update its Bubble Tea TUI incrementally. The non-streaming path exists only as a fallback for debugging.
:::

## Key Takeaways

- Batch responses create unacceptable latency for interactive agents -- users wait the full generation time before seeing any output.
- Streaming delivers the first token in under a second, enabling early interruption and a dramatically better user experience.
- Adopting streaming requires changes at every layer: HTTP transport, event parsing, content accumulation, the agentic loop, and error handling.
- The rest of this chapter builds each streaming component from the ground up, starting with the SSE protocol.
