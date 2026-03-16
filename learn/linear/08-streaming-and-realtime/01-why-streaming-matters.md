---
title: Why Streaming Matters
description: The user experience and technical motivations for streaming LLM responses instead of waiting for complete responses, with latency analysis and perception psychology.
---

# Why Streaming Matters

> **What you'll learn:**
> - Why time-to-first-token is the most important latency metric for perceived agent responsiveness
> - How streaming reduces memory pressure by processing data incrementally instead of buffering entire responses
> - The psychological impact of progressive disclosure on user trust and engagement with AI tools

Imagine you type a question into your coding agent and wait. One second. Two seconds. Five seconds. Then suddenly, a wall of text appears all at once. Now imagine a different experience: you type the same question, and within 200 milliseconds, text starts flowing onto the screen word by word, finishing in those same five seconds. The total wait time is identical, but the second experience *feels* dramatically faster. That perception gap is why streaming matters, and it is the foundation of everything you will build in this chapter.

## The Latency That Users Actually Feel

When you measure the performance of an LLM-powered agent, two metrics dominate:

- **Time to first token (TTFT):** How long from the moment the user presses Enter until the first character of the response appears on screen.
- **Total generation time:** How long until the complete response is available.

Of these two, TTFT is by far the more important one for user perception. Research in human-computer interaction consistently shows that users perceive a system as responsive when it provides feedback within 100-200 milliseconds. After about one second of silence, users start wondering if something is broken. After three seconds, they begin losing focus.

An LLM API like Anthropic's Claude or OpenAI's GPT might take 5-30 seconds to generate a complete response, depending on length. Without streaming, your agent must wait for the entire response to arrive before showing anything. With streaming, you typically get the first token within 200-500 milliseconds, and the user sees a continuous flow of text for the remainder.

Let's look at the numbers in a simple comparison:

```rust
use std::time::Instant;

// Simulating non-streaming: wait for everything, then display
async fn non_streaming_request(client: &reqwest::Client) -> String {
    let start = Instant::now();

    let response = client
        .post("https://api.anthropic.com/v1/messages")
        .header("x-api-key", std::env::var("ANTHROPIC_API_KEY").unwrap())
        .header("anthropic-version", "2023-06-01")
        .header("content-type", "application/json")
        .body(r#"{
            "model": "claude-sonnet-4-20250514",
            "max_tokens": 1024,
            "messages": [{"role": "user", "content": "Explain ownership in Rust"}]
        }"#)
        .send()
        .await
        .expect("request failed");

    let body = response.text().await.expect("failed to read body");

    let elapsed = start.elapsed();
    println!("Time to display: {:?}", elapsed); // ~5-15 seconds
    body
}
```

In this non-streaming approach, the user stares at a blank screen for the entire generation time. Now compare with a streaming approach where you process tokens as they arrive:

```rust
use std::time::Instant;
use futures::StreamExt;

async fn streaming_request(client: &reqwest::Client) {
    let start = Instant::now();
    let mut first_token_time = None;

    let response = client
        .post("https://api.anthropic.com/v1/messages")
        .header("x-api-key", std::env::var("ANTHROPIC_API_KEY").unwrap())
        .header("anthropic-version", "2023-06-01")
        .header("content-type", "application/json")
        .body(r#"{
            "model": "claude-sonnet-4-20250514",
            "max_tokens": 1024,
            "stream": true,
            "messages": [{"role": "user", "content": "Explain ownership in Rust"}]
        }"#)
        .send()
        .await
        .expect("request failed");

    let mut stream = response.bytes_stream();

    while let Some(chunk) = stream.next().await {
        let chunk = chunk.expect("stream error");
        if first_token_time.is_none() {
            first_token_time = Some(start.elapsed());
            println!("Time to first token: {:?}", first_token_time.unwrap());
            // Typically 200-500ms
        }
        // Process and display the chunk immediately
        print!("{}", String::from_utf8_lossy(&chunk));
    }

    println!("\nTotal time: {:?}", start.elapsed());
}
```

The total time is nearly the same in both cases, but the streaming version gives feedback within hundreds of milliseconds instead of seconds.

::: python Coming from Python
In Python, you might stream responses with `httpx`:
```python
import httpx

with httpx.stream("POST", url, json=payload, headers=headers) as response:
    for chunk in response.iter_text():
        print(chunk, end="", flush=True)
```
Or with the `anthropic` Python SDK:
```python
with client.messages.stream(model="claude-sonnet-4-20250514", messages=messages) as stream:
    for text in stream.text_stream:
        print(text, end="", flush=True)
```
The Rust approach is structurally similar -- you iterate over a stream of chunks -- but uses `async`/`await` with `futures::StreamExt` instead of Python's synchronous context managers or `async for` loops. The key difference is that Rust's stream processing is zero-cost at runtime: there is no garbage collector pause that might cause a stutter in your token rendering.
:::

## Memory Efficiency

Streaming is not just about perceived latency. It also fundamentally changes how your application uses memory.

Consider an LLM generating a long code refactoring response -- perhaps 50KB of text with explanations. Without streaming, your agent must allocate a buffer large enough to hold the entire response before it can process any of it. With streaming, you process each chunk as it arrives, and you only need to buffer what has not yet been displayed or committed to your conversation state.

This matters more than you might think. A coding agent often has multiple conversations in flight, and each might have responses arriving concurrently. If each response buffers completely before processing, your memory usage scales with `number_of_responses * average_response_size`. With streaming, it scales with `number_of_responses * chunk_size`, which is typically orders of magnitude smaller.

```rust
// Without streaming: entire response in memory at once
let full_response: String = response.text().await?;  // Allocates for entire body
process_response(&full_response);

// With streaming: only one chunk in memory at a time
let mut stream = response.bytes_stream();
while let Some(chunk) = stream.next().await {
    let chunk = chunk?;
    process_chunk(&chunk);  // chunk is typically a few hundred bytes
    // Previous chunk is dropped here, memory freed
}
```

## Progressive Disclosure and User Trust

There is a psychological dimension to streaming that goes beyond raw performance. When users see text appearing progressively, several things happen:

**They can read as the response generates.** A typical human reads at 200-300 words per minute, and an LLM generates at 30-80 tokens per second. The generation speed far outpaces reading speed, so by the time the user finishes reading the first paragraph, several more have already arrived. Streaming turns waiting time into reading time.

**They can interrupt early.** If the LLM starts heading in the wrong direction, a user watching a streaming response can hit Ctrl+C within the first sentence rather than waiting for a complete wrong answer. This saves API tokens and user time. We will build this interrupt capability in the [Interrupt and Cancel](/linear/08-streaming-and-realtime/09-interrupt-and-cancel) subchapter.

**They build a mental model of the LLM's "thinking."** Seeing text flow creates a sense of the model working through the problem. This is not just an illusion -- it genuinely helps users understand the response's structure as it builds up, rather than being confronted with a complete wall of text.

::: wild In the Wild
Every major coding agent streams by default. Claude Code streams tokens directly to the terminal as they arrive, which is why its responses feel immediate even when generating long code blocks. OpenCode similarly streams through its TUI, updating the markdown rendering in real-time. Codex streams responses and even provides a visual indicator of which tool calls are currently in progress. None of these agents use the non-streaming API in their default mode -- the UX difference is simply too large.
:::

## The Streaming Pipeline

Building a streaming agent is not just about calling a streaming API endpoint. You need a multi-layered pipeline:

1. **Network layer:** Receives raw bytes from the HTTP response, handling chunked transfer encoding and connection management.
2. **Protocol layer:** Parses the SSE (Server-Sent Events) protocol to extract individual events from the raw byte stream.
3. **Data layer:** Parses the JSON payload within each SSE event, handling partial JSON when tool call arguments arrive in fragments.
4. **Application layer:** Updates conversation state, accumulates tool call arguments, and tracks the response structure.
5. **Rendering layer:** Displays tokens to the user, handling markdown formatting, code highlighting, and smooth text flow.

Each layer in this pipeline has its own challenges -- backpressure, buffering, error handling, cancellation. The rest of this chapter walks through each one in detail, giving you the tools to build a streaming pipeline that is both correct and performant.

## Key Takeaways

- **Time to first token (TTFT) drives user perception** more than total generation time. Streaming typically achieves TTFT of 200-500ms versus 5-30 seconds for non-streaming requests.
- **Streaming reduces memory usage** by processing data incrementally rather than buffering entire responses. Memory scales with chunk size instead of response size.
- **Progressive disclosure transforms waiting time into reading time** and allows users to interrupt bad responses early, saving both time and API tokens.
- **A streaming agent needs a multi-layered pipeline** -- network, protocol, data, application, and rendering -- each with its own concerns around buffering, error handling, and cancellation.
- **Every production coding agent streams by default** because the UX improvement is too significant to leave on the table.
