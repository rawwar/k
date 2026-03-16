---
title: Interrupt Handling
description: Allow users to cancel in-progress streaming responses with Ctrl+C while preserving partial results and system state.
---

# Interrupt Handling

> **What you'll learn:**
> - How to catch Ctrl+C signals and propagate cancellation to the streaming task
> - How to use Tokio cancellation tokens to cleanly abort an in-flight stream
> - How to preserve partial response content for the conversation history after interruption

One of the primary reasons for streaming is to let users interrupt early. If the model is generating a long response heading in the wrong direction, the user should be able to press Ctrl+C and stop it immediately. But "immediately" is deceptively hard. You need to stop reading from the network, stop rendering to the terminal, preserve whatever text was already generated, and leave the conversation history in a consistent state for the next turn. Let's build all of that.

## The problem with naive Ctrl+C

If you do nothing special, pressing Ctrl+C during a streaming response terminates your entire process. The operating system delivers a `SIGINT` signal, and by default Rust's runtime aborts. That means the user loses their entire session -- conversation history, working directory state, everything.

What you want instead is cooperative cancellation: Ctrl+C signals "stop this stream" but not "kill the process." The agent should:

1. Stop reading new chunks from the network.
2. Stop rendering tokens to the terminal.
3. Preserve the text that was already received.
4. Add the partial response to the conversation history.
5. Return control to the REPL prompt so the user can continue.

## Tokio cancellation tokens

Tokio provides `CancellationToken`, a lightweight mechanism for cooperative cancellation. You create a token, share it with a task, and when you cancel the token, the task can detect it and stop gracefully:

```rust
use tokio_util::sync::CancellationToken;

// Create a cancellation token
let cancel_token = CancellationToken::new();

// Clone it for the signal handler
let cancel_on_signal = cancel_token.clone();

// Spawn a task that listens for Ctrl+C
tokio::spawn(async move {
    tokio::signal::ctrl_c().await.expect("failed to listen for Ctrl+C");
    cancel_on_signal.cancel();
});
```

Now the streaming loop can check the token on each iteration:

```rust
use tokio::select;
use futures::StreamExt;

async fn stream_with_cancellation(
    mut byte_stream: impl futures::Stream<Item = Result<bytes::Bytes, reqwest::Error>> + Unpin,
    cancel_token: CancellationToken,
) -> Result<StreamOutput, Box<dyn std::error::Error>> {
    let mut splitter = LineSplitter::new();
    let mut parser = SseParser::new();
    let mut renderer = TokenRenderer::new();
    let mut tool_accumulator = ToolCallAccumulator::new();
    let mut stop_reason = None;

    loop {
        // Race between the next chunk and cancellation
        let chunk = select! {
            chunk = byte_stream.next() => {
                match chunk {
                    Some(Ok(bytes)) => bytes,
                    Some(Err(e)) => return Err(e.into()),
                    None => break, // Stream ended naturally
                }
            }
            _ = cancel_token.cancelled() => {
                // User pressed Ctrl+C
                stop_reason = Some("user_interrupt".to_string());
                break;
            }
        };

        for line in splitter.feed(&chunk) {
            let Some(sse_event) = parser.feed_line(&line) else {
                continue;
            };

            if sse_event.event_type == "ping" {
                continue;
            }

            let stream_event: StreamEvent = serde_json::from_str(&sse_event.data)?;

            match stream_event {
                StreamEvent::ContentBlockDelta {
                    delta: Delta::TextDelta { text },
                    ..
                } => {
                    renderer.render_delta(&text)?;
                }
                StreamEvent::ContentBlockDelta {
                    index,
                    delta: Delta::InputJsonDelta { partial_json },
                } => {
                    tool_accumulator.append_json(index, &partial_json);
                }
                StreamEvent::ContentBlockStart {
                    index,
                    content_block: ContentBlockStub::ToolUse { id, name },
                } => {
                    tool_accumulator.start_tool_call(index, id, name);
                }
                StreamEvent::ContentBlockStop { index } => {
                    tool_accumulator.finish_block(index).ok();
                }
                StreamEvent::MessageDelta { delta, .. } => {
                    stop_reason = delta.stop_reason;
                }
                StreamEvent::MessageStop => break,
                _ => {}
            }
        }
    }

    let render_result = renderer.finish()?;

    Ok(StreamOutput {
        text: render_result.text,
        tool_calls: tool_accumulator.take_completed(),
        stop_reason,
    })
}
```

The critical part is the `select!` macro. It races two futures: the next chunk from the network and the cancellation signal. Whichever completes first wins. If the user presses Ctrl+C, the loop breaks immediately without waiting for the next network chunk.

::: python Coming from Python
In Python, you handle interrupts with a try/except around KeyboardInterrupt:
```python
try:
    with client.messages.stream(...) as stream:
        for text in stream.text_stream:
            print(text, end="", flush=True)
except KeyboardInterrupt:
    print("\n[Interrupted]")
    # stream is automatically closed by the context manager
```
Rust's approach with `CancellationToken` and `select!` is more explicit but also more powerful. You can cancel from any source (Ctrl+C, timeout, user button click), and the cancellation propagates through async tasks cleanly without unwinding the stack.
:::

## Managing the signal handler lifecycle

You need to set up and tear down the Ctrl+C handler correctly for each streaming request. If you leave the handler active between requests, pressing Ctrl+C at the REPL prompt might not behave as expected.

Here is a pattern that scopes the signal handler to a single streaming operation:

```rust
use tokio_util::sync::CancellationToken;

/// Manages the lifecycle of a streaming request with interrupt support.
pub struct StreamSession {
    cancel_token: CancellationToken,
}

impl StreamSession {
    /// Create a new stream session. Sets up Ctrl+C handling.
    pub fn new() -> Self {
        let cancel_token = CancellationToken::new();

        let token_clone = cancel_token.clone();
        tokio::spawn(async move {
            tokio::signal::ctrl_c().await.ok();
            token_clone.cancel();
        });

        Self { cancel_token }
    }

    /// Get a clone of the cancellation token for the stream processor.
    pub fn cancel_token(&self) -> CancellationToken {
        self.cancel_token.clone()
    }

    /// Check if the session was interrupted.
    pub fn was_interrupted(&self) -> bool {
        self.cancel_token.is_cancelled()
    }
}
```

Use it in the agentic loop like this:

```rust
loop {
    let session = StreamSession::new();
    let byte_stream = start_streaming_request(&client, &api_key, &messages).await?;

    let output = stream_with_cancellation(byte_stream, session.cancel_token()).await?;

    if session.was_interrupted() {
        eprintln!("\n[Response interrupted by user]");
        // Add partial text to conversation history
        if !output.text.is_empty() {
            messages.push(assistant_message_from_text(&output.text));
        }
        // Don't execute any partially-received tool calls
        continue;
    }

    // Normal completion: process tool calls, update history, etc.
    messages.push(assistant_message_from_output(&output));

    match output.stop_reason.as_deref() {
        Some("tool_use") => {
            let results = execute_tools(&output.tool_calls).await?;
            messages.push(tool_results_message(results));
        }
        Some("end_turn") | _ => break,
    }
}
```

## Preserving partial content

When the user interrupts, you have two choices for the partial response:

1. **Keep it in conversation history.** The model will see its partial response in the next turn. This is usually the right choice because it maintains context continuity.
2. **Discard it.** The model starts fresh as if the interrupted response never happened. This is appropriate if the response was completely wrong.

The recommended default is option 1 -- keep the partial content. Here is how to build a conversation message from a partial response:

```rust
use serde_json::json;

pub fn assistant_message_from_partial(text: &str) -> serde_json::Value {
    json!({
        "role": "assistant",
        "content": [
            {
                "type": "text",
                "text": format!("{}\n\n[Response interrupted by user]", text)
            }
        ]
    })
}
```

Appending `[Response interrupted by user]` makes it clear to both the human reader and the model that this response was cut short.

## Handling interrupts during tool calls

If the stream was delivering a tool call when the user interrupted, the JSON arguments are likely incomplete. You should never attempt to parse or execute a partial tool call:

```rust
if session.was_interrupted() {
    // Any active (incomplete) tool calls are silently discarded.
    // Only completed tool calls (those that received content_block_stop
    // before the interrupt) are kept.
    let completed_before_interrupt = tool_accumulator.take_completed();

    // But even completed tool calls should not be executed after an
    // interrupt -- the user wanted to stop.
    eprintln!(
        "[Discarded {} pending tool call(s)]",
        completed_before_interrupt.len()
    );
}
```

::: wild In the Wild
Claude Code handles Ctrl+C with a two-tier system. The first Ctrl+C during streaming cancels the current response and returns to the prompt. A second Ctrl+C within a short window (about 500ms) exits the application entirely. This gives users a predictable escape hatch without making it too easy to accidentally close the session. OpenCode implements a similar pattern in its Bubble Tea event loop, catching the `tea.KeyCtrlC` event and routing it to either "cancel stream" or "quit app" depending on the current state.
:::

## Key Takeaways

- Never let Ctrl+C kill the entire process during streaming -- use cooperative cancellation to stop the stream gracefully.
- Tokio's `CancellationToken` combined with `select!` lets you race between the next network chunk and a cancellation signal, exiting the loop immediately when interrupted.
- Always preserve partial text after an interrupt and add it to the conversation history so the model maintains context continuity.
- Never execute partially-received tool calls -- only tool calls that completed before the interrupt should be considered, and even those should typically be discarded after a user interrupt.
- Scope the signal handler to each streaming operation using a `StreamSession` struct to avoid interference with REPL-level input handling.
