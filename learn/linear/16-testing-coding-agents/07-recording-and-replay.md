---
title: Recording and Replay
description: Build infrastructure to record real LLM API interactions and replay them in tests, combining real-world fidelity with test determinism.
---

# Recording and Replay

> **What you'll learn:**
> - How to implement a recording layer that captures HTTP requests and responses during real agent sessions for later replay
> - Techniques for sanitizing recorded interactions to remove API keys, personal data, and non-deterministic headers before committing to the test suite
> - How to build a replay provider that serves recorded responses, matching requests by content hash or conversation position

Mock providers are fast and deterministic, but they test against responses you wrote by hand. You might script the mock to return a clean `read_file` tool call, but the real model might return a tool call wrapped in commentary, or choose a different tool entirely. Recording and replay bridges this gap: you record a real LLM interaction once, then replay it in tests forever after. You get real-world fidelity with test-suite speed.

## The Recording Architecture

The recording system sits between your agentic loop and the real LLM provider. It passes requests through to the real API and captures both the request and response. The recordings are saved as JSON files that the replay provider loads later.

```rust
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct RecordedExchange {
    pub request: RecordedRequest,
    pub response: RecordedResponse,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct RecordedRequest {
    pub messages: Vec<RecordedMessage>,
    pub tool_count: usize,
    pub sequence_number: usize,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct RecordedMessage {
    pub role: String,
    pub content_summary: String,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct RecordedResponse {
    pub content: Vec<MessageContent>,
    pub stop_reason: String,
    pub input_tokens: u32,
    pub output_tokens: u32,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Recording {
    pub metadata: RecordingMetadata,
    pub exchanges: Vec<RecordedExchange>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct RecordingMetadata {
    pub model: String,
    pub recorded_at: String,
    pub description: String,
    pub agent_version: String,
}
```

## The Recording Provider

The recording provider wraps a real provider. It forwards every call and saves the exchange:

```rust
use std::sync::Mutex;
use async_trait::async_trait;

pub struct RecordingProvider {
    inner: Box<dyn LlmProvider>,
    exchanges: Mutex<Vec<RecordedExchange>>,
    output_path: PathBuf,
    metadata: RecordingMetadata,
}

impl RecordingProvider {
    pub fn new(
        inner: Box<dyn LlmProvider>,
        output_path: PathBuf,
        description: &str,
    ) -> Self {
        Self {
            inner,
            exchanges: Mutex::new(Vec::new()),
            output_path,
            metadata: RecordingMetadata {
                model: "claude-sonnet-4-20250514".to_string(),
                recorded_at: chrono::Utc::now().to_rfc3339(),
                description: description.to_string(),
                agent_version: env!("CARGO_PKG_VERSION").to_string(),
            },
        }
    }

    /// Save all recorded exchanges to disk.
    pub fn save(&self) -> Result<(), std::io::Error> {
        let exchanges = self.exchanges.lock().unwrap();
        let recording = Recording {
            metadata: self.metadata.clone(),
            exchanges: exchanges.clone(),
        };
        let json = serde_json::to_string_pretty(&recording)?;
        std::fs::write(&self.output_path, json)?;
        Ok(())
    }
}

#[async_trait]
impl LlmProvider for RecordingProvider {
    async fn send(
        &self,
        messages: &[Message],
        tools: &[ToolDefinition],
    ) -> Result<LlmResponse, ProviderError> {
        let sequence = self.exchanges.lock().unwrap().len();

        // Forward to the real provider
        let response = self.inner.send(messages, tools).await?;

        // Record the exchange
        let recorded = RecordedExchange {
            request: RecordedRequest {
                messages: messages
                    .iter()
                    .map(|m| RecordedMessage {
                        role: m.role.clone(),
                        content_summary: summarize_content(&m.content),
                    })
                    .collect(),
                tool_count: tools.len(),
                sequence_number: sequence,
            },
            response: RecordedResponse {
                content: response.content.clone(),
                stop_reason: format!("{:?}", response.stop_reason),
                input_tokens: response.usage.input_tokens,
                output_tokens: response.usage.output_tokens,
            },
        };

        self.exchanges.lock().unwrap().push(recorded);
        Ok(response)
    }
}

fn summarize_content(content: &MessageContent) -> String {
    match content {
        MessageContent::Text(t) => {
            if t.len() > 100 {
                format!("{}...", &t[..100])
            } else {
                t.clone()
            }
        }
        MessageContent::ToolUse { name, .. } => format!("[tool_use: {}]", name),
        MessageContent::ToolResult { tool_use_id, .. } => {
            format!("[tool_result: {}]", tool_use_id)
        }
    }
}
```

## Sanitizing Recordings

Before committing recordings to version control, strip sensitive data:

```rust
pub fn sanitize_recording(recording: &mut Recording) {
    // Remove any API keys that might have leaked into content
    for exchange in &mut recording.exchanges {
        for msg in &mut exchange.request.messages {
            msg.content_summary = redact_secrets(&msg.content_summary);
        }

        // Redact file paths that contain usernames
        for content in &mut exchange.response.content {
            if let MessageContent::Text(text) = content {
                *text = redact_paths(text);
            }
        }
    }

    // Normalize the timestamp
    recording.metadata.recorded_at = "2026-01-01T00:00:00Z".to_string();
}

fn redact_secrets(text: &str) -> String {
    // Replace patterns that look like API keys
    let re = regex::Regex::new(r"sk-[a-zA-Z0-9]{20,}").unwrap();
    re.replace_all(text, "[REDACTED_API_KEY]").to_string()
}

fn redact_paths(text: &str) -> String {
    // Replace /Users/username/ or /home/username/ with /home/user/
    let re = regex::Regex::new(r"/(Users|home)/[a-zA-Z0-9_-]+/").unwrap();
    re.replace_all(text, "/home/user/").to_string()
}

#[cfg(test)]
mod sanitize_tests {
    use super::*;

    #[test]
    fn redacts_api_keys() {
        let text = "Using key sk-abcdefghijklmnopqrstuvwxyz for auth";
        let clean = redact_secrets(text);
        assert!(!clean.contains("sk-abc"));
        assert!(clean.contains("[REDACTED_API_KEY]"));
    }

    #[test]
    fn redacts_home_directories() {
        let text = "Reading /Users/johndoe/projects/myapp/src/main.rs";
        let clean = redact_paths(text);
        assert!(!clean.contains("johndoe"));
        assert!(clean.contains("/home/user/"));
    }
}
```

::: python Coming from Python
Python's `vcrpy` library provides similar record/replay functionality for HTTP interactions:
```python
@vcr.use_cassette('fixtures/read_file.yaml')
def test_agent_reads_file():
    agent = Agent(provider=RealProvider())
    result = agent.run("Read main.rs")
    assert "fn main" in result
```
The concept is the same — record once, replay forever. The Rust approach gives you more control because you implement the recording layer yourself rather than relying on HTTP-level interception. This means you can sanitize at the semantic level (redacting tool results, normalizing paths) rather than just at the HTTP level (replacing headers).
:::

## The Replay Provider

The replay provider loads a recording file and serves responses in sequence:

```rust
pub struct ReplayProvider {
    exchanges: Vec<RecordedExchange>,
    position: Mutex<usize>,
}

impl ReplayProvider {
    pub fn from_file(path: &str) -> Result<Self, Box<dyn std::error::Error>> {
        let content = std::fs::read_to_string(path)?;
        let recording: Recording = serde_json::from_str(&content)?;
        Ok(Self {
            exchanges: recording.exchanges,
            position: Mutex::new(0),
        })
    }

    pub fn from_recording(recording: Recording) -> Self {
        Self {
            exchanges: recording.exchanges,
            position: Mutex::new(0),
        }
    }
}

#[async_trait]
impl LlmProvider for ReplayProvider {
    async fn send(
        &self,
        _messages: &[Message],
        _tools: &[ToolDefinition],
    ) -> Result<LlmResponse, ProviderError> {
        let mut pos = self.position.lock().unwrap();
        if *pos >= self.exchanges.len() {
            panic!(
                "ReplayProvider exhausted: {} exchanges recorded, \
                 but call {} was made",
                self.exchanges.len(),
                *pos + 1
            );
        }

        let exchange = &self.exchanges[*pos];
        *pos += 1;

        let stop_reason = match exchange.response.stop_reason.as_str() {
            "EndTurn" => StopReason::EndTurn,
            "ToolUse" => StopReason::ToolUse,
            "MaxTokens" => StopReason::MaxTokens,
            other => panic!("Unknown stop reason in recording: {}", other),
        };

        Ok(LlmResponse {
            content: exchange.response.content.clone(),
            stop_reason,
            usage: Usage {
                input_tokens: exchange.response.input_tokens,
                output_tokens: exchange.response.output_tokens,
            },
        })
    }
}
```

## Using Replay Tests

Replay tests live alongside your other integration tests. They load a recording fixture and run the agentic loop against it:

```rust
#[cfg(test)]
mod replay_tests {
    use super::*;
    use std::sync::Arc;

    #[tokio::test]
    async fn replay_file_read_conversation() {
        // Load the recorded conversation
        let provider = Arc::new(
            ReplayProvider::from_file("tests/fixtures/read_main_rs.json").unwrap()
        );

        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("main.rs"),
            "fn main() {\n    println!(\"Hello\");\n}\n",
        )
        .unwrap();

        let tools: Vec<Box<dyn Tool>> = vec![
            // ... real tools pointing at the temp dir
        ];

        let agent = AgentLoop::new(provider, tools, 10);
        let actions = agent.run("What does main.rs do?").await;

        // Assert the same things you would in a mock-based test
        let finished = actions
            .iter()
            .any(|a| matches!(&a.kind, ActionKind::Finished(_)));
        assert!(finished, "Agent should complete the task");
    }
}
```

## When to Re-Record

Recordings go stale. Re-record when:

- You change models (switching from Sonnet to Opus)
- You change your system prompt significantly
- You add or remove tools (the model sees different tool definitions)
- You change tool output formatting (the model sees different tool results)

Keep a script that re-records all fixtures:

```rust
// scripts/record_fixtures.rs
// Run with: cargo run --bin record_fixtures

fn main() {
    let scenarios = vec![
        ("read_file", "Read src/main.rs and tell me what it does"),
        ("write_file", "Create a new file called hello.rs with a main function"),
        ("fix_bug", "The test in tests/math.rs is failing. Fix it."),
    ];

    for (name, prompt) in scenarios {
        println!("Recording scenario: {}", name);
        // ... set up workspace, create recording provider, run agent, save
    }
}
```

::: wild In the Wild
Claude Code uses a recording infrastructure to capture real API interactions during development and replay them in CI. This approach lets the team test against real model behavior without incurring API costs on every test run. Recordings are periodically refreshed when the model or system prompt changes. The recordings are stored in a separate fixtures directory and are treated as test data in version control.
:::

## Key Takeaways

- Recording and replay captures real LLM interactions once and replays them deterministically, giving you real-world fidelity at test-suite speed
- Sanitize recordings before committing them — redact API keys, personal paths, and any sensitive data that might appear in model responses
- The replay provider serves responses in sequence, matching by position rather than by request content, which keeps the implementation simple
- Re-record fixtures when you change models, system prompts, tool definitions, or output formatting, since stale recordings will cause tests to behave differently from real interactions
- Keep a recording script that can regenerate all fixtures in one step, making it easy to refresh recordings when the agent changes
