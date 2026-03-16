---
title: Event Bus Design
description: Build an internal event bus that enables decoupled communication between the agent core, plugins, and UI components.
---

# Event Bus Design

> **What you'll learn:**
> - How to design a typed event system in Rust where events are structs and handlers are registered by event type using trait objects or enums
> - The tradeoffs between synchronous event dispatch (handlers block the caller) and async dispatch (handlers run concurrently)
> - How to implement event ordering guarantees, error propagation from handlers, and backpressure when handlers are slow

Once you have plugins loaded and initialized, they need a way to communicate with the agent core and with each other. Directly calling functions across plugin boundaries creates tight coupling -- exactly what a plugin system is supposed to avoid. An event bus solves this by letting components publish events without knowing who is listening, and subscribe to events without knowing who publishes them.

Think of the event bus as the nervous system of your agent. When a tool executes, it fires a `ToolExecuted` event. A logging plugin picks it up and writes to a file. A metrics plugin picks it up and increments a counter. A UI plugin picks it up and updates the display. None of these know about each other. They just know about the event.

## Defining Events with Enums

Rust's enums are perfect for a typed event system. Unlike stringly-typed event systems (common in Python and JavaScript), a Rust enum makes it impossible to subscribe to an event that does not exist or to publish an event with the wrong payload:

```rust
use std::time::{Duration, Instant};
use serde_json::Value;

/// Every event in the system is a variant of this enum.
/// Adding a new event variant forces all match arms to be updated.
#[derive(Debug, Clone)]
pub enum AgentEvent {
    /// The agent has started and is ready to accept input.
    AgentStarted { session_id: String },

    /// A user message was received.
    UserMessage { content: String },

    /// The LLM returned a response (possibly partial during streaming).
    LlmResponse { content: String, is_final: bool },

    /// A tool is about to be executed.
    ToolInvocationStarted {
        tool_name: String,
        args: Value,
        invocation_id: String,
    },

    /// A tool has finished executing.
    ToolInvocationCompleted {
        tool_name: String,
        invocation_id: String,
        result: Result<String, String>,
        duration: Duration,
    },

    /// An MCP server connected or disconnected.
    McpServerStateChanged {
        server_name: String,
        connected: bool,
    },

    /// A plugin was loaded or unloaded.
    PluginStateChanged {
        plugin_name: String,
        new_state: String,
    },

    /// The agent is shutting down.
    AgentShutdown { reason: String },
}
```

::: python Coming from Python
Python event systems typically use strings for event names and dictionaries for payloads:
```python
bus.emit("tool.executed", {"tool": "read_file", "duration": 0.5})
bus.on("tool.executed", lambda data: print(data["tool"]))  # Runtime errors if key missing
```
Rust's enum approach catches typos and missing fields at compile time. You cannot subscribe to `"tool.executd"` (note the typo) because the variant is `ToolInvocationCompleted`, and the compiler verifies every field. The tradeoff is that adding a new event requires recompilation, but for an agent's internal events this is almost always acceptable.
:::

## The Event Bus Core

The event bus needs to do three things: accept subscriber registrations, dispatch events to matching subscribers, and handle the lifecycle of subscriptions (including unsubscription). Here is a minimal async implementation:

```rust
use std::sync::Arc;
use tokio::sync::RwLock;

/// A handler that receives events. Handlers must be Send + Sync
/// because they may be called from any task.
#[async_trait::async_trait]
pub trait EventHandler: Send + Sync {
    /// Handle an event. Return Ok(()) to indicate success.
    /// Returning an error logs the failure but does not stop dispatch.
    async fn handle(&self, event: &AgentEvent) -> Result<(), Box<dyn std::error::Error + Send + Sync>>;

    /// An optional filter: return true for events this handler cares about.
    /// Default implementation accepts all events.
    fn accepts(&self, event: &AgentEvent) -> bool {
        let _ = event;
        true
    }
}

/// Subscription handle returned when registering a handler.
/// Drop it to unsubscribe.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SubscriptionId(u64);

struct Subscription {
    id: SubscriptionId,
    handler: Arc<dyn EventHandler>,
    priority: i32,
}

pub struct EventBus {
    subscriptions: RwLock<Vec<Subscription>>,
    next_id: std::sync::atomic::AtomicU64,
}

impl EventBus {
    pub fn new() -> Self {
        Self {
            subscriptions: RwLock::new(Vec::new()),
            next_id: std::sync::atomic::AtomicU64::new(1),
        }
    }

    /// Subscribe a handler. Lower priority numbers run first.
    pub async fn subscribe(
        &self,
        handler: Arc<dyn EventHandler>,
        priority: i32,
    ) -> SubscriptionId {
        let id = SubscriptionId(
            self.next_id.fetch_add(1, std::sync::atomic::Ordering::SeqCst)
        );
        let mut subs = self.subscriptions.write().await;
        subs.push(Subscription { id, handler, priority });
        subs.sort_by_key(|s| s.priority);
        id
    }

    /// Unsubscribe a handler by its ID.
    pub async fn unsubscribe(&self, id: SubscriptionId) {
        let mut subs = self.subscriptions.write().await;
        subs.retain(|s| s.id != id);
    }

    /// Dispatch an event to all matching subscribers.
    pub async fn emit(&self, event: &AgentEvent) {
        let subs = self.subscriptions.read().await;
        for sub in subs.iter() {
            if sub.handler.accepts(event) {
                if let Err(e) = sub.handler.handle(event).await {
                    eprintln!(
                        "Event handler error (sub {:?}): {e}",
                        sub.id
                    );
                }
            }
        }
    }
}
```

## Synchronous vs. Async Dispatch

The implementation above uses sequential async dispatch: each handler runs to completion before the next one starts. This gives you deterministic ordering but means a slow handler blocks all subsequent handlers. Let's look at the alternatives.

### Sequential Dispatch (shown above)

- Handlers run in priority order, one at a time.
- A slow handler delays everything after it.
- Useful when handlers have ordering dependencies (e.g., a logging handler should see the same state as a metrics handler).

### Concurrent Dispatch

All handlers for an event run simultaneously:

```rust
impl EventBus {
    /// Dispatch an event to all matching subscribers concurrently.
    pub async fn emit_concurrent(&self, event: &AgentEvent) {
        let subs = self.subscriptions.read().await;

        let futures: Vec<_> = subs.iter()
            .filter(|s| s.handler.accepts(event))
            .map(|s| {
                let handler = s.handler.clone();
                let event = event.clone();
                let sub_id = s.id;
                tokio::spawn(async move {
                    if let Err(e) = handler.handle(&event).await {
                        eprintln!("Event handler error (sub {sub_id:?}): {e}");
                    }
                })
            })
            .collect();

        // Wait for all handlers to complete
        for future in futures {
            let _ = future.await;
        }
    }
}
```

Concurrent dispatch is faster when handlers are independent (logging and metrics do not interact), but it makes ordering guarantees impossible.

### Fire-and-Forget Dispatch

For events where the emitter does not need to wait for handlers to complete, you can dispatch and move on:

```rust
impl EventBus {
    /// Dispatch an event without waiting for handlers.
    /// Useful for non-critical notifications like analytics.
    pub fn emit_fire_and_forget(&self, event: AgentEvent) {
        let subs = self.subscriptions.clone();
        tokio::spawn(async move {
            let subs = subs.read().await;
            for sub in subs.iter() {
                if sub.handler.accepts(&event) {
                    let handler = sub.handler.clone();
                    let event = event.clone();
                    tokio::spawn(async move {
                        let _ = handler.handle(&event).await;
                    });
                }
            }
        });
    }
}
```

## Channel-Based Alternative

Instead of trait objects with callbacks, you can use Tokio's `broadcast` channel for a simpler design. This approach works well when you have few event types and many listeners:

```rust
use tokio::sync::broadcast;

pub struct ChannelEventBus {
    sender: broadcast::Sender<AgentEvent>,
}

impl ChannelEventBus {
    pub fn new(capacity: usize) -> Self {
        let (sender, _) = broadcast::channel(capacity);
        Self { sender }
    }

    pub fn emit(&self, event: AgentEvent) {
        // Returns Err if no receivers, which is fine
        let _ = self.sender.send(event);
    }

    pub fn subscribe(&self) -> broadcast::Receiver<AgentEvent> {
        self.sender.subscribe()
    }
}

// A plugin subscribes and processes events in its own task:
async fn logging_plugin(bus: &ChannelEventBus) {
    let mut rx = bus.subscribe();
    tokio::spawn(async move {
        while let Ok(event) = rx.recv().await {
            match &event {
                AgentEvent::ToolInvocationCompleted {
                    tool_name, duration, ..
                } => {
                    println!("[LOG] Tool {tool_name} completed in {duration:?}");
                }
                _ => {} // Ignore events we do not care about
            }
        }
    });
}
```

The channel approach gives you natural backpressure (the channel has a bounded capacity), decoupled lifetimes (subscribers run independently), and easier reasoning about concurrency. The tradeoff is that every subscriber receives every event, so filtering happens on the receive side rather than the dispatch side.

::: tip In the Wild
Claude Code's internal architecture emits events at key points in the agentic loop -- tool start, tool complete, LLM response, permission check. These events drive the terminal UI rendering: when a tool completes, the UI updates to show the result. The event-driven design means the UI layer never polls the agent core, and adding new UI features (like a progress spinner or a token counter) requires only subscribing to the right events.
:::

## Error Handling and Backpressure

Two practical concerns arise in any event bus:

**Error propagation**: When a handler returns an error, the bus should log it but not stop dispatching to other handlers. One misbehaving plugin should not break the entire event flow. The exception is if you implement a "veto" pattern where a handler can cancel an event -- but that is better handled by the hook system (covered in the next subchapter).

**Backpressure**: If handlers are slow and events arrive faster than they can be processed, you need a strategy. Bounded channels handle this naturally -- once the buffer fills, the sender blocks (or drops events, depending on your policy). With the callback approach, you need to add explicit timeout or queue logic.

```rust
impl EventBus {
    /// Dispatch with a timeout per handler.
    pub async fn emit_with_timeout(
        &self,
        event: &AgentEvent,
        handler_timeout: Duration,
    ) {
        let subs = self.subscriptions.read().await;
        for sub in subs.iter() {
            if sub.handler.accepts(event) {
                let result = tokio::time::timeout(
                    handler_timeout,
                    sub.handler.handle(event),
                ).await;

                match result {
                    Ok(Ok(())) => {}
                    Ok(Err(e)) => {
                        eprintln!("Handler {:?} error: {e}", sub.id);
                    }
                    Err(_) => {
                        eprintln!("Handler {:?} timed out after {handler_timeout:?}", sub.id);
                    }
                }
            }
        }
    }
}
```

## Key Takeaways

- A **typed event enum** makes it impossible to misspell event names or omit required fields -- the compiler catches mistakes that would be runtime errors in dynamically-typed event systems.
- Choose your dispatch strategy based on handler independence: **sequential** for ordering guarantees, **concurrent** for throughput, **fire-and-forget** for non-critical notifications.
- **Tokio broadcast channels** provide a simpler alternative to callback-based dispatch, with built-in backpressure and decoupled subscriber lifetimes.
- Always **log handler errors without stopping dispatch** -- one failing subscriber should not prevent other subscribers from receiving events.
- Add **timeouts** to handler dispatch so a misbehaving plugin cannot stall the entire agent's event processing.
