---
title: Event System
description: Implementing a publish-subscribe event system that broadcasts agent lifecycle events like tool execution, message receipt, and session start to interested plugins.
---

# Event System

> **What you'll learn:**
> - How to design an event taxonomy covering agent lifecycle, tool execution, and conversation events
> - How to implement a pub-sub dispatcher that delivers events to registered plugin handlers
> - Patterns for async event handling that does not block the main agent loop

Plugins need to know what the agent is doing. When the user sends a message, when a tool executes, when an error occurs -- plugins may want to react to any of these. An event system lets plugins subscribe to the events they care about and respond asynchronously, without the core agent knowing which plugins are listening. This is the publish-subscribe pattern, and it forms the backbone of any extensible system.

## Designing the Event Taxonomy

Before writing any code, you need to decide what events the agent emits. Think about every meaningful state change in the agent lifecycle:

```rust
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::time::Instant;

/// Every event the agent can emit.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "data")]
pub enum AgentEvent {
    // Session lifecycle
    SessionStarted {
        session_id: String,
    },
    SessionEnded {
        session_id: String,
        duration_secs: f64,
    },

    // Conversation events
    UserMessageReceived {
        content: String,
    },
    AssistantResponseStarted,
    AssistantResponseCompleted {
        content: String,
        token_count: usize,
    },

    // Tool events
    ToolCallRequested {
        tool_name: String,
        arguments: Value,
    },
    ToolCallCompleted {
        tool_name: String,
        result: Value,
        duration_ms: u64,
    },
    ToolCallFailed {
        tool_name: String,
        error: String,
    },

    // Provider events
    ProviderSwitched {
        from: String,
        to: String,
    },
    ApiRequestSent {
        provider: String,
        model: String,
    },
    ApiResponseReceived {
        provider: String,
        tokens_used: usize,
    },

    // Plugin events
    PluginLoaded {
        plugin_name: String,
    },
    PluginUnloaded {
        plugin_name: String,
    },
    PluginError {
        plugin_name: String,
        error: String,
    },

    // Custom events from plugins
    Custom {
        name: String,
        payload: Value,
    },
}

impl AgentEvent {
    /// Returns the event type as a string for subscription filtering.
    pub fn event_type(&self) -> &'static str {
        match self {
            AgentEvent::SessionStarted { .. } => "session.started",
            AgentEvent::SessionEnded { .. } => "session.ended",
            AgentEvent::UserMessageReceived { .. } => "message.user",
            AgentEvent::AssistantResponseStarted => "message.assistant.start",
            AgentEvent::AssistantResponseCompleted { .. } => "message.assistant.complete",
            AgentEvent::ToolCallRequested { .. } => "tool.requested",
            AgentEvent::ToolCallCompleted { .. } => "tool.completed",
            AgentEvent::ToolCallFailed { .. } => "tool.failed",
            AgentEvent::ProviderSwitched { .. } => "provider.switched",
            AgentEvent::ApiRequestSent { .. } => "api.request",
            AgentEvent::ApiResponseReceived { .. } => "api.response",
            AgentEvent::PluginLoaded { .. } => "plugin.loaded",
            AgentEvent::PluginUnloaded { .. } => "plugin.unloaded",
            AgentEvent::PluginError { .. } => "plugin.error",
            AgentEvent::Custom { .. } => "custom",
        }
    }
}
```

The `Custom` variant is crucial -- it lets plugins define their own events without modifying the core enum. Plugins can emit events that other plugins consume, enabling plugin-to-plugin communication.

::: python Coming from Python
In Python, event systems often use string-based event names with flexible payloads:
```python
from typing import Callable, Any

class EventEmitter:
    def __init__(self):
        self._handlers: dict[str, list[Callable]] = {}

    def on(self, event: str, handler: Callable):
        self._handlers.setdefault(event, []).append(handler)

    async def emit(self, event: str, data: Any = None):
        for handler in self._handlers.get(event, []):
            await handler(data)
```
Rust's enum-based approach is more rigid but much safer -- you cannot subscribe to a misspelled event name because the compiler catches it. The `serde` tag gives you string-based flexibility when serializing events for logging or external systems.
:::

## Building the Event Bus

The event bus is the central dispatcher. It maintains a list of subscribers and delivers events to each one:

```rust
use std::sync::Arc;
use tokio::sync::{broadcast, RwLock};

/// A subscriber callback that handles an event.
pub type EventHandler = Arc<
    dyn Fn(AgentEvent) -> std::pin::Pin<
        Box<dyn std::future::Future<Output = ()> + Send>,
    > + Send + Sync,
>;

/// Subscription metadata, linking a handler to its owner and filter.
struct Subscription {
    id: u64,
    owner: String,
    filter: Option<Vec<String>>, // Event types to receive, or None for all
    handler: EventHandler,
}

pub struct EventBus {
    subscriptions: RwLock<Vec<Subscription>>,
    next_id: RwLock<u64>,
    // Broadcast channel for fan-out delivery
    broadcast_tx: broadcast::Sender<AgentEvent>,
}

impl EventBus {
    pub fn new(capacity: usize) -> Self {
        let (broadcast_tx, _) = broadcast::channel(capacity);
        Self {
            subscriptions: RwLock::new(Vec::new()),
            next_id: RwLock::new(0),
            broadcast_tx,
        }
    }

    /// Subscribe to specific event types. Pass None for the filter to receive all events.
    pub async fn subscribe(
        &self,
        owner: &str,
        filter: Option<Vec<String>>,
        handler: EventHandler,
    ) -> u64 {
        let mut subs = self.subscriptions.write().await;
        let mut next = self.next_id.write().await;
        let id = *next;
        *next += 1;

        subs.push(Subscription {
            id,
            owner: owner.to_string(),
            filter,
            handler,
        });

        println!(
            "[events] Plugin '{}' subscribed (id={})",
            owner, id
        );
        id
    }

    /// Unsubscribe a specific handler by its ID.
    pub async fn unsubscribe(&self, subscription_id: u64) {
        let mut subs = self.subscriptions.write().await;
        subs.retain(|s| s.id != subscription_id);
    }

    /// Remove all subscriptions for a given plugin.
    pub async fn unsubscribe_all(&self, owner: &str) {
        let mut subs = self.subscriptions.write().await;
        let before = subs.len();
        subs.retain(|s| s.owner != owner);
        let removed = before - subs.len();
        if removed > 0 {
            println!(
                "[events] Removed {} subscriptions for plugin '{}'",
                removed, owner
            );
        }
    }

    /// Emit an event to all matching subscribers.
    pub async fn emit(&self, event: AgentEvent) {
        // Also send through broadcast channel for channel-based subscribers
        let _ = self.broadcast_tx.send(event.clone());

        let subs = self.subscriptions.read().await;
        let event_type = event.event_type().to_string();

        for sub in subs.iter() {
            // Check if this subscription matches the event type
            let matches = match &sub.filter {
                None => true, // No filter = receive all
                Some(types) => types.iter().any(|t| {
                    // Support prefix matching: "tool.*" matches "tool.requested"
                    if t.ends_with(".*") {
                        let prefix = &t[..t.len() - 2];
                        event_type.starts_with(prefix)
                    } else {
                        t == &event_type
                    }
                }),
            };

            if matches {
                let handler = sub.handler.clone();
                let event_clone = event.clone();
                // Spawn each handler so a slow subscriber does not block others
                tokio::spawn(async move {
                    handler(event_clone).await;
                });
            }
        }
    }

    /// Get a broadcast receiver for channel-based consumption.
    pub fn receiver(&self) -> broadcast::Receiver<AgentEvent> {
        self.broadcast_tx.subscribe()
    }
}
```

There are two consumption models here. The handler-based model (`subscribe`/`emit`) fires callbacks -- good for plugins that react to events. The broadcast channel model (`receiver`) returns a `tokio::sync::broadcast::Receiver` -- good for background tasks that need to process events in a loop.

## Emitting Events from the Agent

Now instrument the agent to emit events at key lifecycle points. Here is how the agentic loop integrates with the event bus:

```rust
use std::sync::Arc;
use std::time::Instant;

pub struct Agent {
    event_bus: Arc<EventBus>,
    // ... other fields ...
}

impl Agent {
    pub async fn handle_user_message(&self, content: &str) -> Result<String> {
        // Emit that we received a user message
        self.event_bus
            .emit(AgentEvent::UserMessageReceived {
                content: content.to_string(),
            })
            .await;

        // ... build API request, call LLM ...

        self.event_bus
            .emit(AgentEvent::AssistantResponseStarted)
            .await;

        let response = self.call_llm(content).await?;

        self.event_bus
            .emit(AgentEvent::AssistantResponseCompleted {
                content: response.clone(),
                token_count: response.split_whitespace().count(), // Simplified
            })
            .await;

        Ok(response)
    }

    pub async fn execute_tool(&self, name: &str, args: Value) -> Result<Value> {
        self.event_bus
            .emit(AgentEvent::ToolCallRequested {
                tool_name: name.to_string(),
                arguments: args.clone(),
            })
            .await;

        let start = Instant::now();
        let result = self.tool_registry.read().await.invoke(name, args).await;
        let duration_ms = start.elapsed().as_millis() as u64;

        match &result {
            Ok(value) => {
                self.event_bus
                    .emit(AgentEvent::ToolCallCompleted {
                        tool_name: name.to_string(),
                        result: value.clone(),
                        duration_ms,
                    })
                    .await;
            }
            Err(e) => {
                self.event_bus
                    .emit(AgentEvent::ToolCallFailed {
                        tool_name: name.to_string(),
                        error: e.to_string(),
                    })
                    .await;
            }
        }

        result.map_err(|e| anyhow::anyhow!(e))
    }
}
```

## A Plugin That Uses Events

Here is a practical example: a metrics plugin that tracks tool execution times and reports statistics:

```rust
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;

pub struct MetricsPlugin {
    manifest: PluginManifest,
    metrics: Arc<Mutex<ToolMetrics>>,
    subscription_id: Option<u64>,
}

struct ToolMetrics {
    call_counts: HashMap<String, u64>,
    total_duration_ms: HashMap<String, u64>,
    errors: HashMap<String, u64>,
}

impl MetricsPlugin {
    pub fn new() -> Self {
        Self {
            manifest: PluginManifest {
                name: "metrics".to_string(),
                version: "1.0.0".to_string(),
                description: "Tracks tool execution metrics".to_string(),
                author: "Agent Core".to_string(),
                dependencies: vec![],
                capabilities: vec![PluginCapability::Events, PluginCapability::Commands],
            },
            metrics: Arc::new(Mutex::new(ToolMetrics {
                call_counts: HashMap::new(),
                total_duration_ms: HashMap::new(),
                errors: HashMap::new(),
            })),
            subscription_id: None,
        }
    }
}

#[async_trait]
impl Plugin for MetricsPlugin {
    fn manifest(&self) -> &PluginManifest {
        &self.manifest
    }

    async fn initialize(&mut self, _ctx: &mut PluginContext) -> Result<(), PluginError> {
        Ok(())
    }

    async fn activate(&mut self, ctx: &mut PluginContext) -> Result<(), PluginError> {
        let metrics = self.metrics.clone();

        // Subscribe to tool events using wildcard filter
        let id = ctx
            .event_bus
            .subscribe(
                "metrics",
                Some(vec!["tool.*".to_string()]),
                Arc::new(move |event| {
                    let metrics = metrics.clone();
                    Box::pin(async move {
                        let mut m = metrics.lock().await;
                        match event {
                            AgentEvent::ToolCallCompleted {
                                tool_name,
                                duration_ms,
                                ..
                            } => {
                                *m.call_counts.entry(tool_name.clone()).or_insert(0) += 1;
                                *m.total_duration_ms
                                    .entry(tool_name)
                                    .or_insert(0) += duration_ms;
                            }
                            AgentEvent::ToolCallFailed { tool_name, .. } => {
                                *m.errors.entry(tool_name).or_insert(0) += 1;
                            }
                            _ => {}
                        }
                    })
                }),
            )
            .await;

        self.subscription_id = Some(id);
        Ok(())
    }

    async fn deactivate(&mut self) -> Result<(), PluginError> {
        // Subscription cleanup happens via unsubscribe_all
        Ok(())
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}
```

This plugin silently records tool usage stats. You could extend it to expose a `/metrics` command that prints a summary, or to emit a warning when a tool's average execution time exceeds a threshold.

::: wild In the Wild
Claude Code's hook system emits events at specific lifecycle points: `PreToolUse`, `PostToolUse`, `Notification`, and `Stop`. Each hook can run arbitrary shell commands, making it effectively an event-to-shell bridge. The events carry context about what tool is being called, its input, and its output. This design keeps the core agent simple -- it just emits events at the right moments, and external scripts decide how to react.
:::

## Avoiding Blocking the Agent Loop

A critical design choice: event handlers must not block the main agent loop. If a handler takes 5 seconds to complete, you do not want the agent to freeze. The `tokio::spawn` in the `emit` method handles this -- each handler runs as its own task.

But this creates a new problem: what if you need to wait for all handlers to complete before proceeding? For that, add a synchronous emit variant:

```rust
impl EventBus {
    /// Emit and wait for all handlers to complete.
    /// Use sparingly -- only when handler completion is required before proceeding.
    pub async fn emit_and_wait(&self, event: AgentEvent) {
        let _ = self.broadcast_tx.send(event.clone());

        let subs = self.subscriptions.read().await;
        let event_type = event.event_type().to_string();

        let mut handles = Vec::new();

        for sub in subs.iter() {
            let matches = match &sub.filter {
                None => true,
                Some(types) => types.iter().any(|t| t == &event_type),
            };

            if matches {
                let handler = sub.handler.clone();
                let event_clone = event.clone();
                handles.push(tokio::spawn(async move {
                    handler(event_clone).await;
                }));
            }
        }

        // Wait for all handlers with a timeout
        let timeout = tokio::time::Duration::from_secs(5);
        for handle in handles {
            let _ = tokio::time::timeout(timeout, handle).await;
        }
    }
}
```

Use `emit` for fire-and-forget events like metrics tracking. Use `emit_and_wait` for critical lifecycle events like `SessionEnded` where you need handlers to flush data before the process exits.

## Key Takeaways

- An event taxonomy based on a Rust enum provides compile-time safety for event types, with a `Custom` variant for plugin-defined events
- The `EventBus` supports both handler-based subscriptions (for reactive plugins) and broadcast channels (for background processing tasks)
- Event handlers run as spawned tasks to avoid blocking the main agent loop, with an `emit_and_wait` variant for lifecycle-critical events
- Wildcard filters like `"tool.*"` let plugins subscribe to event categories without listing every individual event type
- Instrumenting the agent loop with event emissions at key points (message received, tool executed, errors) gives plugins visibility into agent behavior without tight coupling
