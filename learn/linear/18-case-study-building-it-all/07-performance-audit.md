---
title: Performance Audit
description: Identify and address the key performance bottlenecks in a coding agent, from startup time to streaming latency to tool execution overhead.
---

# Performance Audit

> **What you'll learn:**
> - How to profile a coding agent to identify the actual bottlenecks — typically network latency, token generation, and tool I/O rather than CPU computation
> - Techniques for reducing perceived latency through streaming, progressive rendering, speculative prefetching, and background initialization
> - How to set up performance benchmarks that track key metrics (time to first token, tool execution time, memory usage) across releases

A coding agent spends most of its time waiting. Waiting for the LLM to generate tokens. Waiting for network round-trips. Waiting for file system operations and shell commands. The actual CPU work — parsing JSON, matching patterns, rendering text — is negligible by comparison. This changes where you should focus your optimization effort. If you spend a week making your JSON parser twice as fast, you save microseconds. If you reduce one unnecessary LLM round-trip, you save seconds.

This subchapter teaches you how to profile your agent, identify the real bottlenecks, and apply targeted optimizations that the user will actually notice.

## Where Time Goes

Let's measure a typical agent interaction — the user asks "Add error handling to the parse function in src/parser.rs" and the agent reads the file, writes a modified version, and runs the tests. Here is the time breakdown:

| Phase | Duration | % of Total |
|-------|----------|-----------|
| Context assembly | 2ms | 0.02% |
| API request (network + queue) | 800ms | 8% |
| Token generation (first response) | 3,500ms | 35% |
| Tool execution: read_file | 5ms | 0.05% |
| API request (second call) | 600ms | 6% |
| Token generation (second response) | 4,200ms | 42% |
| Tool execution: write_file | 8ms | 0.08% |
| Tool execution: shell (cargo test) | 850ms | 8.5% |
| Rendering overhead | 35ms | 0.35% |
| **Total** | **~10,000ms** | **100%** |

Token generation accounts for 77% of the total time. Network latency adds another 14%. Everything else combined is under 10%. This is the fundamental performance profile of every coding agent, and it dictates where your optimization effort should go.

## Profiling with `tracing` Spans

The `tracing` crate gives you structured, hierarchical timing data without manual timestamp tracking:

```rust
use tracing::{instrument, info_span, Instrument};
use std::time::Instant;

#[instrument(skip(provider, messages))]
async fn call_provider(
    provider: &dyn Provider,
    messages: &[Message],
) -> anyhow::Result<ResponseStream> {
    let span = info_span!("provider_call", model = %provider.model_name());
    let stream = provider.stream_completion(messages)
        .instrument(span)
        .await?;
    Ok(stream)
}

pub async fn execute_tool_timed(
    registry: &ToolRegistry,
    tool_call: &ToolCall,
) -> (ToolResult, Duration) {
    let start = Instant::now();
    let result = registry.execute(tool_call).await;
    let duration = start.elapsed();

    tracing::info!(
        tool = %tool_call.name,
        duration_ms = duration.as_millis() as u64,
        success = result.is_ok(),
        "Tool execution completed"
    );

    let tool_result = match result {
        Ok(output) => ToolResult::success(tool_call.id.clone(), output),
        Err(e) => ToolResult::error(tool_call.id.clone(), e.to_string()),
    };

    (tool_result, duration)
}
```

Run the agent with `RUST_LOG=info` to see timing data for every operation. For detailed profiling, use `RUST_LOG=trace` to capture individual stream chunks and function entry/exit times.

::: python Coming from Python
In Python, you might profile with `cProfile` or `py-spy`. Rust's `tracing` crate serves a different purpose — it is instrumentation that runs in production, not just during profiling sessions. The overhead of `tracing` with a subscriber filtering at the `info` level is typically under 100 nanoseconds per span, making it safe to leave in release builds. Python's `cProfile` adds significant overhead and is typically used only during development.
:::

## Optimization 1: Streaming Reduces Perceived Latency

The single most impactful optimization in a coding agent is streaming. Without streaming, the user stares at a blank screen for the entire token generation time (often 3-5 seconds for a substantial response). With streaming, they see the first token within 200-400 milliseconds.

The *actual* total time is the same. But the *perceived* latency drops dramatically because the user sees progress immediately and can start reading the response while the rest generates.

You built streaming in Chapter 8. The performance implication is that streaming is not optional — it is the primary latency optimization for any LLM-powered application.

## Optimization 2: Parallel Tool Execution

When the model makes multiple tool calls in a single response, you can execute them in parallel rather than sequentially:

```rust
use futures::future::join_all;

async fn execute_tools_parallel(
    registry: &ToolRegistry,
    safety: &SafetyLayer,
    tool_calls: &[ToolCall],
) -> Vec<ToolResult> {
    let futures: Vec<_> = tool_calls.iter().map(|call| async {
        // Safety check first (these are fast)
        match safety.check_tool_call(call).await {
            Ok(Permission::Allowed) => {
                registry.execute(call).await.unwrap_or_else(|e| {
                    ToolResult::error(call.id.clone(), e.to_string())
                })
            }
            Ok(Permission::Denied(reason)) => {
                ToolResult::error(call.id.clone(), format!("Denied: {reason}"))
            }
            _ => {
                // NeedsApproval — must be sequential for user interaction
                ToolResult::error(
                    call.id.clone(),
                    "Requires approval (sequential execution needed)".into()
                )
            }
        }
    }).collect();

    join_all(futures).await
}
```

This matters when the model reads multiple files at once. Three sequential file reads take 15ms. Three parallel reads take 5ms. For shell commands that take longer (running tests, compiling), the savings are more substantial.

Note the caveat with approval-required tools: if the user needs to approve a tool call, you must process it sequentially to present a coherent approval flow.

## Optimization 3: Speculative Context Assembly

Context assembly involves counting tokens for every message in the conversation. For long conversations, this can take noticeable time. Cache token counts for messages that have not changed:

```rust
pub struct CachedMessage {
    pub message: Message,
    pub token_count: usize, // Computed once, cached
}

impl ContextManager {
    pub fn add_message(&mut self, message: Message) {
        let token_count = self.tokenizer.count_tokens(&message);
        self.messages.push(CachedMessage {
            message,
            token_count,
        });
        self.total_tokens += token_count;
    }

    pub fn total_tokens(&self) -> usize {
        // O(1) — no need to re-count
        self.total_tokens
    }
}
```

By caching token counts on insertion, the "is there room for another message?" check becomes a simple integer comparison rather than a full re-count of the conversation.

## Optimization 4: Background Session Persistence

If your agent persists sessions to disk, do it in the background rather than blocking the main loop:

```rust
use tokio::sync::mpsc;

pub struct BackgroundPersister {
    sender: mpsc::Sender<SessionSnapshot>,
}

impl BackgroundPersister {
    pub fn new(sessions_dir: PathBuf) -> Self {
        let (sender, mut receiver) = mpsc::channel::<SessionSnapshot>(16);

        tokio::spawn(async move {
            while let Some(snapshot) = receiver.recv().await {
                let path = sessions_dir.join(format!("{}.json", snapshot.id));
                if let Err(e) = tokio::fs::write(&path, &snapshot.data).await {
                    tracing::warn!("Failed to persist session: {}", e);
                }
            }
        });

        Self { sender }
    }

    pub async fn save(&self, snapshot: SessionSnapshot) {
        // Non-blocking — drops the snapshot if the channel is full
        let _ = self.sender.try_send(snapshot);
    }
}
```

The `try_send` is intentional. If the background task is behind, you would rather drop a snapshot than block the main loop. The next snapshot will contain the complete state.

## Optimization 5: Startup Time

You addressed startup in subchapter 3 with deferred initialization. Here are additional techniques:

**Lazy API key validation.** Don't make a network call at startup to verify the API key. Instead, let the first real API call serve as validation. If the key is invalid, the user sees the error when they make their first request.

**Compiled-in tool definitions.** Tool definitions (names, descriptions, JSON schemas) can be computed at compile time using const functions or `include_str!`:

```rust
impl ReadFileTool {
    pub fn definition() -> ToolDefinition {
        ToolDefinition {
            name: "read_file",
            description: "Read the contents of a file at the given path",
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "The file path to read"
                    }
                },
                "required": ["path"]
            }),
        }
    }
}
```

These definitions are constant and can be assembled into the tool list without any runtime computation.

::: tip In the Wild
Claude Code optimizes perceived latency by showing a "thinking" indicator immediately when the user submits a prompt, before the first API response arrives. It also displays tool call names and arguments as they stream in, so the user sees "Reading src/parser.rs..." before the file contents are fully loaded. OpenCode uses its TUI to show real-time metrics (tokens used, elapsed time, model name) in a status bar, giving users confidence that the agent is working even during long operations.
:::

## Setting Up Benchmarks

Track key performance metrics across releases with a simple benchmark harness:

```rust
#[cfg(test)]
mod benchmarks {
    use std::time::Instant;
    use super::*;

    #[tokio::test]
    async fn bench_startup_time() {
        let start = Instant::now();
        let _agent = Agent::builder()
            .with_config(Config::default())
            .build()
            .await
            .unwrap();
        let duration = start.elapsed();

        println!("Startup time: {:?}", duration);
        assert!(duration < Duration::from_millis(500), "Startup too slow");
    }

    #[tokio::test]
    async fn bench_context_assembly() {
        let mut ctx = ContextManager::new(128_000, &ContextConfig::default());
        // Add 100 messages
        for i in 0..100 {
            ctx.add_message(Message::user(format!("Message {}", i)));
            ctx.add_message(Message::assistant(format!("Response {}", i)));
        }

        let start = Instant::now();
        let _messages = ctx.assemble_messages(&[]);
        let duration = start.elapsed();

        println!("Context assembly (200 messages): {:?}", duration);
        assert!(duration < Duration::from_millis(10));
    }

    #[tokio::test]
    async fn bench_token_counting() {
        let long_text = "word ".repeat(10_000);
        let start = Instant::now();
        let _count = Tokenizer::new().count_tokens_str(&long_text);
        let duration = start.elapsed();

        println!("Token counting (10K words): {:?}", duration);
        assert!(duration < Duration::from_millis(50));
    }
}
```

Run these benchmarks in CI to catch performance regressions. The assertions serve as performance budgets — if startup exceeds 500ms, the test fails and you investigate.

## Memory Profiling

Memory usage matters for long-running sessions. The conversation history grows with every turn. Tool results (especially file reads and command output) can be large. Monitor memory with:

```rust
pub fn memory_stats(context: &ContextManager) -> MemoryStats {
    let message_count = context.message_count();
    let estimated_memory: usize = context.messages().iter()
        .map(|m| m.message.content.len() + std::mem::size_of::<CachedMessage>())
        .sum();

    MemoryStats {
        message_count,
        estimated_bytes: estimated_memory,
        estimated_mb: estimated_memory as f64 / 1_048_576.0,
    }
}
```

If memory exceeds a threshold, trigger context compaction proactively rather than waiting for token limits.

## Key Takeaways

- Token generation and network latency dominate a coding agent's time budget (90%+) — optimize these first through streaming and minimizing unnecessary LLM round-trips rather than speeding up local computation.
- Streaming is the single most impactful performance optimization because it reduces perceived latency from seconds to milliseconds, even though total time is unchanged.
- Execute independent tool calls in parallel using `join_all`, but fall back to sequential execution when user approval is required for a coherent interaction flow.
- Cache token counts on message insertion and persist sessions in the background to keep the main loop responsive, using channels for non-blocking I/O.
- Establish performance benchmarks with concrete budgets (startup under 500ms, context assembly under 10ms) and run them in CI to catch regressions before they reach users.
