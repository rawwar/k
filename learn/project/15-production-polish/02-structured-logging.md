---
title: Structured Logging
description: Adding structured logging with the tracing crate for observability, including span-based request tracking, log levels, and output formatting for development and production.
---

# Structured Logging

> **What you'll learn:**
> - How to instrument the agent with the tracing crate for structured, span-based logging
> - How to configure log levels, filtering, and output formats for development versus production
> - Techniques for correlating log entries across async tasks and tool invocations

When your agent works correctly, nobody cares about logs. When it misbehaves -- and it will -- logs are the only window into what happened. The `println!` and `eprintln!` statements you have been using so far are not enough for production. You need structured logging: machine-parseable, filterable, and organized by request context. Rust's `tracing` crate gives you all of this, and it integrates beautifully with async code.

## Why tracing, Not log

Rust has two major logging ecosystems. The `log` crate provides a simple logging facade similar to Python's `logging` module. The `tracing` crate builds on that foundation with *spans* -- named, timed regions of execution that carry structured key-value data.

For a coding agent, spans are invaluable. When the agent processes a tool call, you want to know which conversation turn triggered it, how long it took, and what arguments were passed -- all without manually threading context through every function.

First, add the dependencies to your `Cargo.toml`:

```toml
[dependencies]
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter", "json"] }
```

## Setting Up the Subscriber

The subscriber is the component that receives log events and decides what to do with them. You configure it once at application startup.

```rust
use tracing_subscriber::{fmt, EnvFilter, layer::SubscriberExt, util::SubscriberInitExt};

pub fn init_logging(verbose: bool, json_output: bool) {
    let env_filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| {
            if verbose {
                EnvFilter::new("agent=debug,tower_http=debug")
            } else {
                EnvFilter::new("agent=info,tower_http=warn")
            }
        });

    let fmt_layer = if json_output {
        // JSON format for production -- machine-parseable
        fmt::layer()
            .json()
            .with_target(true)
            .with_span_list(true)
            .boxed()
    } else {
        // Pretty format for development -- human-readable
        fmt::layer()
            .pretty()
            .with_target(true)
            .with_thread_ids(false)
            .with_file(true)
            .with_line_number(true)
            .boxed()
    };

    tracing_subscriber::registry()
        .with(env_filter)
        .with(fmt_layer)
        .init();
}
```

Call `init_logging()` at the very start of `main()`:

```rust
#[tokio::main]
async fn main() -> anyhow::Result<()> {
    init_logging(false, false);

    tracing::info!("Agent starting up");
    // ... rest of your agent initialization
    Ok(())
}
```

::: python Coming from Python
Python's `logging` module uses a hierarchy of named loggers (`logging.getLogger("agent.tools")`), handlers, and formatters. Rust's `tracing` works similarly but adds spans, which are like Python's `logging` context managers on steroids. Where you might use `logging.LoggerAdapter` with `extra` fields in Python, `tracing` lets you attach structured fields to spans that automatically propagate to every event within that span.
:::

## Instrumenting the Agent with Spans

Spans create a tree of execution context. When you enter a span, every log event within it automatically carries the span's metadata. This is how you correlate logs across the agent loop.

```rust
use tracing::{info, warn, error, debug, instrument, Span};

#[instrument(skip(client, messages), fields(turn = %turn_number, model = %model))]
pub async fn send_llm_request(
    client: &reqwest::Client,
    api_url: &str,
    model: &str,
    messages: &[Message],
    turn_number: u32,
) -> Result<LlmResponse, AgentError> {
    info!(message_count = messages.len(), "Sending request to LLM");

    let start = std::time::Instant::now();
    let response = client
        .post(api_url)
        .json(&build_request_body(model, messages))
        .send()
        .await
        .map_err(|e| {
            error!(error = %e, "LLM request failed");
            AgentError::Network {
                source: e,
                url: api_url.to_string(),
            }
        })?;

    let status = response.status();
    let body = response.text().await.map_err(|e| {
        error!(error = %e, "Failed to read response body");
        AgentError::Network {
            source: e,
            url: api_url.to_string(),
        }
    })?;

    let elapsed = start.elapsed();
    info!(
        status = %status,
        body_len = body.len(),
        elapsed_ms = elapsed.as_millis() as u64,
        "LLM response received"
    );

    parse_llm_response(&body)
}
```

The `#[instrument]` attribute macro automatically creates a span when the function is entered and closes it when the function returns. The `skip` parameter excludes large arguments from the span data, and `fields` adds custom structured fields.

## Instrumenting Tool Execution

Each tool call should live in its own span so you can see exactly how long each tool takes and what arguments it received.

```rust
use tracing::{info_span, Instrument};

pub async fn execute_tool(
    tool_name: &str,
    arguments: &serde_json::Value,
    workspace: &std::path::Path,
) -> Result<String, AgentError> {
    let span = info_span!(
        "tool_execution",
        tool = %tool_name,
        workspace = %workspace.display(),
    );

    async {
        tracing::info!(args = %arguments, "Executing tool");
        let start = std::time::Instant::now();

        let result = match tool_name {
            "read_file" => execute_read_file(arguments, workspace).await,
            "write_file" => execute_write_file(arguments, workspace).await,
            "shell" => execute_shell(arguments, workspace).await,
            "grep" => execute_grep(arguments, workspace).await,
            other => {
                tracing::warn!(tool = %other, "Unknown tool requested");
                Err(AgentError::ToolExecution {
                    tool: other.to_string(),
                    source: "Unknown tool".into(),
                })
            }
        };

        let elapsed = start.elapsed();
        match &result {
            Ok(output) => {
                tracing::info!(
                    elapsed_ms = elapsed.as_millis() as u64,
                    output_len = output.len(),
                    "Tool completed successfully"
                );
            }
            Err(e) => {
                tracing::error!(
                    elapsed_ms = elapsed.as_millis() as u64,
                    error = %e,
                    "Tool execution failed"
                );
            }
        }

        result
    }
    .instrument(span)
    .await
}
```

The `.instrument(span)` call attaches the span to the async future. Even if the future is polled across multiple executor ticks, the span context follows it correctly.

## Log Levels and Filtering

Choosing the right log level for each event is critical. Too much output drowns signal in noise. Too little leaves you blind when debugging.

Here is a practical guide for agent log levels:

```rust
// ERROR: Something broke that the user or operator must know about
tracing::error!("Failed to connect to LLM API after 3 retries");

// WARN: Something unexpected happened but the agent can continue
tracing::warn!(attempt = 2, max = 3, "LLM request timed out, retrying");

// INFO: Significant events in normal operation -- the "story" of a session
tracing::info!(tool = "shell", cmd = "cargo test", "Executing tool");
tracing::info!(turn = 5, tokens = 1523, "Turn completed");

// DEBUG: Detailed information useful during development
tracing::debug!(body_len = 4096, "Raw LLM response received");
tracing::debug!(key = "model", value = "claude-sonnet", "Config loaded");

// TRACE: Very fine-grained information, usually only for library debugging
tracing::trace!(chunk_len = 128, "SSE chunk received");
```

Users control filtering through the `RUST_LOG` environment variable:

```bash
# Show only warnings and errors
RUST_LOG=warn ./agent

# Show debug output for the agent, info for everything else
RUST_LOG=info,agent=debug ./agent

# Show trace output for tool execution only
RUST_LOG=info,agent::tools=trace ./agent
```

## Writing Logs to a File

In production, you want logs going to a file rather than cluttering the terminal. The `tracing-appender` crate handles this with non-blocking file writing.

```rust
use tracing_subscriber::{fmt, layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};
use tracing_appender::rolling;

pub fn init_production_logging(log_dir: &std::path::Path) {
    let env_filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("agent=info"));

    // Rolling daily log files
    let file_appender = rolling::daily(log_dir, "agent.log");
    let (non_blocking, _guard) = tracing_appender::non_blocking(file_appender);

    let file_layer = fmt::layer()
        .json()
        .with_writer(non_blocking)
        .with_target(true)
        .with_span_list(true);

    // Also log errors to stderr for immediate visibility
    let stderr_layer = fmt::layer()
        .with_writer(std::io::stderr)
        .with_target(false)
        .with_level(true)
        .with_filter(tracing_subscriber::filter::LevelFilter::WARN);

    tracing_subscriber::registry()
        .with(env_filter)
        .with(file_layer)
        .with(stderr_layer)
        .init();
}
```

Note the `_guard` return value from `non_blocking()`. You must keep this guard alive for the duration of the program -- if it is dropped, buffered log entries may be lost. Store it in your `main()` function.

::: wild In the Wild
Claude Code logs structured events for every LLM interaction, tool execution, and error. This telemetry is essential for understanding how the agent performs across thousands of user sessions. OpenCode takes a lighter approach with configurable log levels and file-based logging. Both systems use structured fields (not just string messages) so that logs can be queried and aggregated by tools like Datadog or Grafana.
:::

## Sensitive Data in Logs

Never log API keys, user credentials, or the full content of files the user is editing. Create a sanitization layer:

```rust
/// Truncate a string to a maximum length, adding an ellipsis if truncated.
fn truncate_for_log(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        format!("{}... ({} bytes total)", &s[..max_len], s.len())
    }
}

/// Redact known sensitive patterns from log output.
fn redact_sensitive(s: &str) -> String {
    let patterns = [
        (r"sk-[a-zA-Z0-9]{20,}", "sk-***REDACTED***"),
        (r"Bearer [a-zA-Z0-9._-]+", "Bearer ***REDACTED***"),
        (r"api[_-]key[=:]\s*\S+", "api_key=***REDACTED***"),
    ];

    let mut result = s.to_string();
    for (pattern, replacement) in &patterns {
        if let Ok(re) = regex::Regex::new(pattern) {
            result = re.replace_all(&result, *replacement).to_string();
        }
    }
    result
}
```

## Key Takeaways

- Use the `tracing` crate instead of `println!` for production logging -- it provides structured key-value fields, spans for execution context, and configurable output formats.
- The `#[instrument]` attribute macro and `.instrument(span)` method automatically create spans that track function execution time and carry context across async boundaries.
- Configure different output formats for development (pretty, human-readable) and production (JSON, machine-parseable) controlled by a startup flag or environment variable.
- Use `RUST_LOG` environment variable filtering to control log verbosity at the module level, letting users dial in exactly the information they need for debugging.
- Never log sensitive data like API keys or full file contents -- build sanitization and truncation into your logging layer from day one.
