---
title: Telemetry Considerations
description: Design privacy-respecting telemetry that provides actionable product insights while giving users full transparency and control over data collection.
---

# Telemetry Considerations

> **What you'll learn:**
> - How to design a telemetry system that collects only actionable metrics (feature usage, error rates, performance) without capturing personal or code content
> - Techniques for implementing opt-in/opt-out telemetry with clear disclosure, following industry best practices and respecting user trust
> - How to aggregate and analyze telemetry data to drive product decisions like which features to improve, deprecate, or prioritize

Telemetry is one of the most contentious topics in developer tooling. Done right, it gives you the data to build a better product. Done wrong, it destroys user trust and triggers backlash. This subchapter covers how to implement telemetry that is genuinely useful, transparently disclosed, and fully under the user's control. The technical implementation is straightforward -- the hard part is making the right ethical decisions about what to collect and how to communicate it.

## Why Telemetry Matters

Without telemetry, you are flying blind. You do not know which features people actually use, where they encounter errors, or how long operations take in real-world conditions. Your understanding of the product is limited to your own usage, GitHub issues (which represent only frustrated users, not satisfied ones), and guesswork.

Useful telemetry answers questions like:

- Which tools does the agent invoke most frequently? (Prioritize improving those.)
- What percentage of sessions encounter an error? (Track reliability.)
- How long does the average session last? (Understand engagement.)
- Which LLM providers are most popular? (Inform integration priorities.)
- What is the P95 latency for tool execution? (Find performance bottlenecks.)

None of these questions require capturing code content, file paths, prompts, or any personally identifiable information.

## The Ethical Framework

Before writing any telemetry code, establish principles that guide every decision:

1. **Collect what you need, not what you can.** Every data point must answer a specific product question. If you cannot articulate why you need it, do not collect it.

2. **Never collect code content.** The user's source code, prompts, and file contents are off-limits. Period. No "anonymized" snippets, no "aggregated" samples.

3. **No personally identifiable information.** No usernames, email addresses, IP addresses, or file paths that reveal directory structures.

4. **Opt-in by default.** The telemetry system should be disabled until the user explicitly enables it. This is more conservative than the industry norm (many tools default to opt-out) but earns more trust.

5. **Full transparency.** Document exactly what is collected and provide a way for users to inspect the data before it is sent.

6. **Respect the decision.** When telemetry is disabled, do not collect anything. Do not even phone home to check if the user might want to re-enable it.

::: python Coming from Python
The Python community has been burned by telemetry controversies. The `pip` project considered adding telemetry and faced significant pushback. The `pipenv` project added telemetry without clear disclosure and had to backtrack. Homebrew added opt-out analytics (reporting to Google Analytics) and was criticized for it. The Rust community tends to be privacy-conscious, and tools that handle telemetry transparently (like rustup, which asks during installation) have set a positive standard.
:::

## What to Collect

Design your telemetry events around the product questions you need to answer. Here is a responsible set of events for a coding agent:

```rust
use serde::Serialize;
use std::time::Duration;

#[derive(Debug, Serialize)]
#[serde(tag = "event_type")]
pub enum TelemetryEvent {
    /// Sent once when the agent starts
    SessionStart {
        agent_version: String,
        os: String,
        arch: String,
        provider: String,
        model: String,
    },

    /// Sent once when the agent exits
    SessionEnd {
        duration_secs: u64,
        turns: u32,
        tools_invoked: u32,
    },

    /// Sent when a tool is invoked (no content, just the tool name)
    ToolInvocation {
        tool_name: String,
        duration_ms: u64,
        success: bool,
    },

    /// Sent when an unrecoverable error occurs
    Error {
        error_kind: String,  // e.g., "api_timeout", "parse_error"
        // NOT the error message (might contain file paths or code)
    },

    /// Sent periodically with performance data
    PerformanceSnapshot {
        memory_mb: u64,
        active_duration_secs: u64,
    },
}
```

Notice what is **not** collected:

- No prompts or responses (could contain secrets or personal information)
- No file paths (reveal project structure)
- No error messages (could contain code snippets or paths)
- No IP addresses (the transport layer may log these, but you do not send them as event fields)
- No tool arguments (a shell command could contain anything)

The `error_kind` field is an enumerated category like `"api_timeout"` or `"parse_error"`, not the raw error string. This ensures you can track error rates without accidentally leaking sensitive context.

## The Telemetry Client

Implement a simple telemetry client that batches events and sends them asynchronously:

```rust
use std::sync::Mutex;
use tokio::sync::mpsc;

pub struct TelemetryClient {
    enabled: bool,
    sender: Option<mpsc::UnboundedSender<TelemetryEvent>>,
    install_id: String,
}

impl TelemetryClient {
    pub fn new(config: &TelemetryConfig) -> Self {
        if !config.enabled {
            return Self {
                enabled: false,
                sender: None,
                install_id: String::new(),
            };
        }

        let install_id = config.install_id.clone()
            .unwrap_or_else(generate_install_id);

        let (sender, receiver) = mpsc::unbounded_channel();

        // Spawn the background sender
        tokio::spawn(telemetry_sender_loop(receiver, install_id.clone()));

        Self {
            enabled: true,
            sender: Some(sender),
            install_id,
        }
    }

    pub fn record(&self, event: TelemetryEvent) {
        if !self.enabled {
            return;
        }
        if let Some(ref sender) = self.sender {
            let _ = sender.send(event);  // Ignore send errors (channel closed)
        }
    }
}

fn generate_install_id() -> String {
    // Generate a random UUID that is not tied to any hardware or user identity
    uuid::Uuid::new_v4().to_string()
}
```

The `install_id` is a random UUID generated at first run. It lets you count unique installations and track session patterns for a single installation over time, without knowing anything about the user. It is not derived from hardware identifiers, usernames, or MAC addresses.

### Batching and Sending

Batch events to minimize network requests and gracefully handle offline periods:

```rust
async fn telemetry_sender_loop(
    mut receiver: mpsc::UnboundedReceiver<TelemetryEvent>,
    install_id: String,
) {
    let client = reqwest::Client::new();
    let mut batch: Vec<TelemetryEvent> = Vec::new();
    let flush_interval = tokio::time::interval(Duration::from_secs(60));
    tokio::pin!(flush_interval);

    loop {
        tokio::select! {
            event = receiver.recv() => {
                match event {
                    Some(e) => {
                        batch.push(e);
                        // Flush if batch is large enough
                        if batch.len() >= 20 {
                            send_batch(&client, &install_id, &mut batch).await;
                        }
                    }
                    None => {
                        // Channel closed, send remaining events and exit
                        if !batch.is_empty() {
                            send_batch(&client, &install_id, &mut batch).await;
                        }
                        return;
                    }
                }
            }
            _ = flush_interval.tick() => {
                if !batch.is_empty() {
                    send_batch(&client, &install_id, &mut batch).await;
                }
            }
        }
    }
}

async fn send_batch(
    client: &reqwest::Client,
    install_id: &str,
    batch: &mut Vec<TelemetryEvent>,
) {
    #[derive(Serialize)]
    struct TelemetryPayload<'a> {
        install_id: &'a str,
        events: &'a [TelemetryEvent],
    }

    let payload = TelemetryPayload {
        install_id,
        events: batch,
    };

    // Fire and forget: telemetry must never affect the user experience
    let _ = client
        .post("https://telemetry.yourservice.com/v1/events")
        .json(&payload)
        .timeout(Duration::from_secs(5))
        .send()
        .await;

    batch.clear();
}
```

Key design decisions:

- **Fire and forget** -- If the send fails (network error, server down), the events are dropped. Never retry, never queue to disk, never block.
- **Short timeout** -- 5 seconds maximum. Telemetry should not consume bandwidth or CPU time.
- **Batch sends** -- Reduce the number of HTTP requests. One request every 60 seconds or every 20 events, whichever comes first.

## Disclosure and Consent

The first time the agent runs with telemetry capable code, present a clear disclosure:

```rust
fn first_run_telemetry_prompt() -> bool {
    eprintln!("=== Anonymous Usage Telemetry ===");
    eprintln!();
    eprintln!("my-agent can collect anonymous usage statistics to help");
    eprintln!("improve the tool. This includes:");
    eprintln!("  - Which features and tools are used");
    eprintln!("  - Error rates and types (not error messages)");
    eprintln!("  - Session duration and performance metrics");
    eprintln!();
    eprintln!("We NEVER collect:");
    eprintln!("  - Your code, prompts, or file contents");
    eprintln!("  - File paths or project names");
    eprintln!("  - Personal information of any kind");
    eprintln!();
    eprintln!("You can change this at any time:");
    eprintln!("  my-agent config set telemetry.enabled true");
    eprintln!("  my-agent config set telemetry.enabled false");
    eprintln!();
    eprint!("Enable anonymous telemetry? [y/N] ");

    let mut input = String::new();
    std::io::stdin().read_line(&mut input).unwrap_or(0);
    input.trim().eq_ignore_ascii_case("y")
}
```

The default answer is "No" (the `[y/N]` convention with capital N). This is a deliberate choice: requiring an explicit affirmative action to enable telemetry demonstrates respect for user privacy.

## Inspecting Telemetry Data

Let users see exactly what will be sent. Add a `--telemetry-dump` flag or subcommand:

```rust
fn dump_telemetry_events(events: &[TelemetryEvent]) {
    println!("The following telemetry data would be sent:");
    println!();
    for event in events {
        println!("{}", serde_json::to_string_pretty(event).unwrap());
        println!();
    }
}
```

This transparency builds trust. Users can verify that you are collecting exactly what you say and nothing more.

## Server-Side Considerations

Your telemetry endpoint should be minimal:

```
POST /v1/events
Content-Type: application/json

{
    "install_id": "550e8400-e29b-41d4-a716-446655440000",
    "events": [
        {
            "event_type": "SessionStart",
            "agent_version": "0.5.2",
            "os": "macos",
            "arch": "aarch64",
            "provider": "anthropic",
            "model": "claude-sonnet-4-20250514"
        }
    ]
}
```

On the server side:

- **Do not log IP addresses** in your application logs. Configure your load balancer and application server to strip or hash client IPs.
- **Set a retention policy** -- delete raw events after 90 days. Keep only aggregated statistics long-term.
- **No cross-referencing** -- do not attempt to de-anonymize install IDs by correlating with other data sources.

::: wild In the Wild
The Rust project itself handles telemetry responsibly. `rustup` asks during installation whether the user wants to participate in anonymous telemetry. The Rust compiler team uses this data to understand which targets, editions, and features are used in practice. The key lesson: transparency at the point of collection builds the trust needed to sustain a telemetry program long-term.
:::

## The "No Telemetry" Compile-Time Option

For enterprise and privacy-sensitive environments, provide a way to compile the agent with telemetry code entirely removed:

```toml
[features]
default = []
telemetry = ["dep:reqwest", "dep:uuid"]
```

```rust
#[cfg(feature = "telemetry")]
pub fn init_telemetry(config: &TelemetryConfig) -> TelemetryClient {
    TelemetryClient::new(config)
}

#[cfg(not(feature = "telemetry"))]
pub fn init_telemetry(_config: &TelemetryConfig) -> NoopTelemetry {
    NoopTelemetry
}

#[cfg(not(feature = "telemetry"))]
pub struct NoopTelemetry;

#[cfg(not(feature = "telemetry"))]
impl NoopTelemetry {
    pub fn record(&self, _event: TelemetryEvent) {
        // Compiled to nothing
    }
}
```

When built without the `telemetry` feature, the telemetry code is completely absent from the binary. There is no dead code, no network dependency, and no configuration to worry about. This is a stronger guarantee than a runtime flag and is appreciated by security-conscious organizations.

## Key Takeaways

- Default to opt-in telemetry and require an explicit affirmative action to enable it -- this builds trust and sets a positive precedent.
- Collect only what you need to answer specific product questions: feature usage counts, error rates, performance metrics -- never code content, file paths, or prompts.
- Use a random UUID as the install identifier rather than anything derived from hardware, usernames, or other personal information.
- Fire-and-forget sending with short timeouts ensures telemetry never affects the user experience, even on slow or unreliable networks.
- Provide a compile-time feature flag that removes telemetry code entirely, giving enterprise and privacy-conscious users a provable guarantee.
