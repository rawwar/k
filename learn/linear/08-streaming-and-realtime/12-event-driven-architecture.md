---
title: Event Driven Architecture
description: Structuring the agent around an event bus where streaming tokens, tool results, user input, and system events flow through a unified event-driven pipeline.
---

# Event Driven Architecture

> **What you'll learn:**
> - How an event-driven architecture decouples producers (network, user input) from consumers (renderer, state manager)
> - Designing an event enum that represents all agent events: stream tokens, tool calls, user actions, and system signals
> - Implementing an event bus with typed channels that allows components to subscribe to specific event types

Up to this point, you have built individual pieces of a streaming pipeline: SSE parsing, partial JSON handling, backpressure, cancellation, and buffering. Now it is time to connect them into a coherent architecture. An event-driven design gives you a clean way to structure the entire agent: every interesting thing that happens -- a token arriving, a user pressing a key, a tool call completing, a network error occurring -- is represented as an event that flows through a central bus. Components subscribe to the events they care about and ignore the rest.

## Why Events?

Consider the interactions in a coding agent during a single turn:

1. The user types a message and presses Enter.
2. The agent sends the message to the LLM API.
3. SSE events start arriving: `message_start`, then text deltas.
4. The renderer displays tokens as they arrive.
5. A tool call block starts streaming.
6. The renderer shows "calling write_file...".
7. The tool call completes; the agent executes the tool.
8. The tool result is sent back to the LLM.
9. More text deltas arrive as the LLM responds to the tool result.
10. The user presses Ctrl+C to interrupt.

In a procedural design, you would write a single giant function that handles all of these cases with nested `match` statements and complex control flow. Every time you add a new feature (syntax highlighting, progress bars, logging), you modify this central function.

In an event-driven design, each of these interactions is an event. The SSE parser emits `TokenReceived` events. The tool executor emits `ToolCompleted` events. The signal handler emits `UserInterrupt` events. The renderer *subscribes* to the events it cares about and renders accordingly. Adding a new feature means adding a new subscriber, not modifying existing code.

## The Event Enum

Start by defining all possible events in your system as a Rust enum:

```rust
use serde_json::Value;

#[derive(Debug, Clone)]
pub enum AgentEvent {
    // === Stream events ===
    /// A new message has started streaming from the LLM
    StreamStarted {
        message_id: String,
        model: String,
    },
    /// A text token arrived
    TextDelta {
        text: String,
    },
    /// A tool call has started streaming
    ToolCallStarted {
        tool_id: String,
        tool_name: String,
    },
    /// A fragment of tool call JSON arguments arrived
    ToolCallDelta {
        tool_id: String,
        json_fragment: String,
    },
    /// A tool call has finished streaming (arguments complete)
    ToolCallReady {
        tool_id: String,
        tool_name: String,
        arguments: Value,
    },
    /// The stream has completed normally
    StreamCompleted {
        stop_reason: String,
        input_tokens: u64,
        output_tokens: u64,
    },
    /// The stream encountered an error
    StreamError {
        error: String,
        retryable: bool,
    },

    // === Tool events ===
    /// A tool has started executing
    ToolExecutionStarted {
        tool_id: String,
        tool_name: String,
    },
    /// A tool has finished executing
    ToolExecutionCompleted {
        tool_id: String,
        result: ToolResult,
    },

    // === User events ===
    /// The user submitted a new message
    UserMessage {
        content: String,
    },
    /// The user requested cancellation (Ctrl+C)
    UserInterrupt,
    /// The user requested exit (Ctrl+D or /exit)
    UserExit,

    // === System events ===
    /// Reconnecting after a network failure
    Reconnecting {
        attempt: u32,
        delay_ms: u64,
    },
    /// The agent is ready for input
    Ready,
}

#[derive(Debug, Clone)]
pub struct ToolResult {
    pub success: bool,
    pub output: String,
}
```

This enum is the central contract of your application. Every component communicates through these events. The enum is `Clone` so events can be distributed to multiple subscribers, and `Debug` so they can be logged.

::: python Coming from Python
In Python, events are often represented as dataclasses or dictionaries:
```python
from dataclasses import dataclass
from typing import Union

@dataclass
class TextDelta:
    text: str

@dataclass
class ToolCallReady:
    tool_id: str
    tool_name: str
    arguments: dict

AgentEvent = Union[TextDelta, ToolCallReady, ...]
```
Rust's enum approach is more powerful than Python's Union type because the compiler enforces exhaustive matching. When you add a new event variant, every `match` on `AgentEvent` must handle it -- the compiler tells you every place in your code that needs updating. With Python's Union, adding a new type is silent, and you only discover missing handlers at runtime.
:::

## The Event Bus

The event bus is the central hub that routes events from producers to consumers. A simple implementation uses Tokio's `broadcast` channel, which allows multiple receivers to each get a copy of every event:

```rust
use tokio::sync::broadcast;

pub struct EventBus {
    sender: broadcast::Sender<AgentEvent>,
}

impl EventBus {
    pub fn new(capacity: usize) -> Self {
        let (sender, _) = broadcast::channel(capacity);
        Self { sender }
    }

    /// Publish an event to all subscribers
    pub fn publish(&self, event: AgentEvent) {
        // Ignore the error if there are no subscribers
        let _ = self.sender.send(event);
    }

    /// Subscribe to all events
    pub fn subscribe(&self) -> broadcast::Receiver<AgentEvent> {
        self.sender.subscribe()
    }

    /// Get a sender handle for producers to use
    pub fn sender(&self) -> broadcast::Sender<AgentEvent> {
        self.sender.clone()
    }
}
```

Now each component can independently subscribe and react:

```rust
/// The renderer subscribes to events and updates the display
async fn renderer_loop(bus: &EventBus) {
    let mut rx = bus.subscribe();

    loop {
        match rx.recv().await {
            Ok(AgentEvent::TextDelta { text }) => {
                print!("{}", text);
                std::io::stdout().flush().ok();
            }
            Ok(AgentEvent::ToolCallStarted { tool_name, .. }) => {
                println!("\n[Calling {}...]", tool_name);
            }
            Ok(AgentEvent::ToolExecutionCompleted { tool_id, result }) => {
                if result.success {
                    println!("[Tool {} completed]", tool_id);
                } else {
                    println!("[Tool {} failed: {}]", tool_id, result.output);
                }
            }
            Ok(AgentEvent::StreamCompleted { input_tokens, output_tokens, .. }) => {
                println!("\n[{} input, {} output tokens]", input_tokens, output_tokens);
                break;
            }
            Ok(AgentEvent::UserInterrupt) => {
                println!("\n[Interrupted]");
                break;
            }
            Ok(AgentEvent::StreamError { error, .. }) => {
                eprintln!("\n[Error: {}]", error);
                break;
            }
            Ok(_) => {} // Ignore events we don't care about
            Err(broadcast::error::RecvError::Lagged(n)) => {
                eprintln!("[Renderer lagged, missed {} events]", n);
            }
            Err(broadcast::error::RecvError::Closed) => break,
        }
    }
}

use std::io::Write;
```

## State Management as an Event Subscriber

The conversation state manager is another subscriber that updates the agent's internal model based on events:

```rust
pub struct ConversationState {
    messages: Vec<Message>,
    current_response: Option<ResponseBuilder>,
    token_usage: TokenUsage,
}

struct ResponseBuilder {
    text: String,
    tool_calls: Vec<ToolCallBuilder>,
}

struct ToolCallBuilder {
    id: String,
    name: String,
    json_buffer: String,
}

#[derive(Default)]
struct TokenUsage {
    total_input: u64,
    total_output: u64,
}

struct Message {
    role: String,
    content: String,
}

impl ConversationState {
    pub fn new() -> Self {
        Self {
            messages: Vec::new(),
            current_response: None,
            token_usage: TokenUsage::default(),
        }
    }

    /// Process an event and update state
    pub fn handle_event(&mut self, event: &AgentEvent) {
        match event {
            AgentEvent::StreamStarted { .. } => {
                self.current_response = Some(ResponseBuilder {
                    text: String::new(),
                    tool_calls: Vec::new(),
                });
            }
            AgentEvent::TextDelta { text } => {
                if let Some(ref mut response) = self.current_response {
                    response.text.push_str(text);
                }
            }
            AgentEvent::ToolCallStarted { tool_id, tool_name } => {
                if let Some(ref mut response) = self.current_response {
                    response.tool_calls.push(ToolCallBuilder {
                        id: tool_id.clone(),
                        name: tool_name.clone(),
                        json_buffer: String::new(),
                    });
                }
            }
            AgentEvent::ToolCallDelta { tool_id, json_fragment } => {
                if let Some(ref mut response) = self.current_response {
                    if let Some(tc) = response
                        .tool_calls
                        .iter_mut()
                        .find(|tc| tc.id == *tool_id)
                    {
                        tc.json_buffer.push_str(json_fragment);
                    }
                }
            }
            AgentEvent::StreamCompleted { input_tokens, output_tokens, .. } => {
                self.token_usage.total_input += input_tokens;
                self.token_usage.total_output += output_tokens;

                // Finalize the response and add it to the conversation
                if let Some(response) = self.current_response.take() {
                    self.messages.push(Message {
                        role: "assistant".to_string(),
                        content: response.text,
                    });
                }
            }
            AgentEvent::UserMessage { content } => {
                self.messages.push(Message {
                    role: "user".to_string(),
                    content: content.clone(),
                });
            }
            _ => {}
        }
    }
}
```

The state manager processes the same events as the renderer, but instead of displaying them, it builds up the conversation history. This separation means you can change how events are displayed without touching the state logic, and vice versa.

## Wiring It All Together

Here is how the complete event-driven agent loop looks:

```rust
async fn agent_main_loop() {
    let bus = EventBus::new(256);

    // Spawn the renderer
    let bus_for_renderer = bus.sender();
    let renderer_handle = tokio::spawn({
        let bus = EventBus { sender: bus_for_renderer };
        async move {
            renderer_loop(&bus).await;
        }
    });

    // State manager runs in the main loop
    let mut state = ConversationState::new();

    // Simulated: the SSE parser publishes events to the bus
    let events = vec![
        AgentEvent::StreamStarted {
            message_id: "msg_01".into(),
            model: "claude-sonnet-4-20250514".into(),
        },
        AgentEvent::TextDelta { text: "Hello, ".into() },
        AgentEvent::TextDelta { text: "world!".into() },
        AgentEvent::StreamCompleted {
            stop_reason: "end_turn".into(),
            input_tokens: 10,
            output_tokens: 5,
        },
    ];

    for event in &events {
        state.handle_event(event);
        bus.publish(event.clone());
        // Small delay to simulate streaming
        tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
    }

    // Wait for renderer to finish
    let _ = renderer_handle.await;
}
```

::: wild In the Wild
Claude Code uses an event-driven architecture internally where streaming events, tool executions, and user interactions all flow through a central event system. This allows features like logging, token counting, and UI updates to be implemented as independent event handlers rather than being tangled into the core streaming logic. OpenCode similarly structures its agent around an event bus (using Go's channel patterns), where the TUI, state manager, and tool executor each subscribe to the events they need.
:::

## When Not to Use Event-Driven Architecture

Event-driven design is not always the right choice. For simple agents with a single streaming path and no concurrent operations, a straightforward procedural loop is simpler and easier to debug:

```rust
// Sometimes simple is better
async fn simple_stream_loop(mut events: tokio::sync::mpsc::Receiver<SseEvent>) {
    let mut text = String::new();
    while let Some(event) = events.recv().await {
        if event.event_type() == "content_block_delta" {
            if let Some(delta) = extract_text_delta(&event.data) {
                print!("{}", delta);
                text.push_str(&delta);
            }
        }
    }
    println!();
}

fn extract_text_delta(data: &str) -> Option<String> {
    let v: serde_json::Value = serde_json::from_str(data).ok()?;
    v.get("delta")?.get("text")?.as_str().map(|s| s.to_string())
}

// SseEvent type from earlier
struct SseEvent {
    event_type: Option<String>,
    data: String,
}

impl SseEvent {
    fn event_type(&self) -> &str {
        self.event_type.as_deref().unwrap_or("message")
    }
}
```

Reach for the event bus when you have multiple consumers that need to react to the same events, when you want to add features (logging, metrics, analytics) without modifying existing code, or when you need to coordinate concurrent operations. For a prototype or a simple CLI tool, the procedural approach is fine.

## Key Takeaways

- An **event-driven architecture** decouples producers from consumers, allowing the renderer, state manager, and logger to each process events independently.
- The **`AgentEvent` enum** serves as the central contract for your application. Rust's exhaustive matching ensures that every component handles every event type it should.
- **`broadcast` channels** distribute events to multiple subscribers, each receiving a copy. Use this when multiple components need to observe the same events.
- Separating **rendering from state management** lets you change the display logic without affecting conversation history, and vice versa.
- Event-driven design adds complexity -- use a **simple procedural loop** for prototypes and reach for the event bus when you need multiple independent consumers or extensibility.
