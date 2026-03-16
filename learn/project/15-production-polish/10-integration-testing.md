---
title: Integration Testing
description: Writing end-to-end integration tests that exercise the full agent loop with recorded API responses, verifying tool execution, conversation flow, and error handling.
---

# Integration Testing

> **What you'll learn:**
> - How to write integration tests that run the full agent loop against recorded LLM responses
> - Techniques for testing multi-turn conversations with tool calls and file system side effects
> - How to build a test fixture system that sets up and tears down project directories cleanly

Unit tests verify individual functions. Integration tests verify that all the pieces work together. For a coding agent, this means testing the full loop: receive a prompt, call the LLM, parse the response, execute tools, feed results back, and produce the correct final output. The challenge is that real LLM calls are slow, expensive, and non-deterministic. The solution is recorded responses -- you capture real API interactions once and replay them in tests.

## Test Organization in Rust

Cargo has built-in support for integration tests. Files in the `tests/` directory are compiled as separate crates that depend on your library:

```
src/
  main.rs
  lib.rs          # Library code that tests import
  agent.rs
  tools/
tests/
  integration/
    mod.rs
    agent_loop.rs
    tool_execution.rs
    error_recovery.rs
  fixtures/
    recorded_responses/
      simple_greeting.json
      tool_call_write_file.json
      multi_turn_conversation.json
    projects/
      sample_rust_project/
        Cargo.toml
        src/main.rs
```

The key insight is putting your agent logic in `lib.rs` (exported from the library) so integration tests can import it. Your `main.rs` becomes a thin wrapper that calls into the library.

::: python Coming from Python
In Python, integration tests are typically files in a `tests/` directory run by pytest. Rust's approach is similar but with a structural difference: integration test files in `tests/` are compiled as separate crates. This means they can only access your public API, which enforces the same boundaries that external users of your library would face. It is like having `from agent import public_api` instead of accessing internal modules.
:::

## Building a Mock LLM Server

Instead of mocking at the function level, build a lightweight HTTP server that returns recorded responses. This tests the full HTTP path including serialization and deserialization.

```rust
use std::net::TcpListener;
use std::sync::{Arc, Mutex};
use tokio::sync::oneshot;

/// A mock LLM server that returns pre-configured responses in sequence.
pub struct MockLlmServer {
    pub url: String,
    shutdown_tx: Option<oneshot::Sender<()>>,
}

#[derive(Clone)]
struct ServerState {
    responses: Arc<Mutex<Vec<serde_json::Value>>>,
    request_log: Arc<Mutex<Vec<serde_json::Value>>>,
}

impl MockLlmServer {
    /// Start a mock server that returns the given responses in order.
    pub async fn start(responses: Vec<serde_json::Value>) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        let url = format!("http://127.0.0.1:{port}");

        let state = ServerState {
            responses: Arc::new(Mutex::new(responses)),
            request_log: Arc::new(Mutex::new(Vec::new())),
        };

        let (shutdown_tx, shutdown_rx) = oneshot::channel();

        let state_clone = state.clone();
        tokio::spawn(async move {
            let app = axum::Router::new()
                .route("/v1/messages", axum::routing::post(handle_request))
                .with_state(state_clone);

            let listener = tokio::net::TcpListener::from_std(listener).unwrap();
            axum::serve(listener, app)
                .with_graceful_shutdown(async {
                    shutdown_rx.await.ok();
                })
                .await
                .unwrap();
        });

        MockLlmServer {
            url,
            shutdown_tx: Some(shutdown_tx),
        }
    }

    pub fn api_url(&self) -> String {
        format!("{}/v1/messages", self.url)
    }
}

impl Drop for MockLlmServer {
    fn drop(&mut self) {
        if let Some(tx) = self.shutdown_tx.take() {
            let _ = tx.send(());
        }
    }
}

async fn handle_request(
    axum::extract::State(state): axum::extract::State<ServerState>,
    axum::Json(body): axum::Json<serde_json::Value>,
) -> axum::Json<serde_json::Value> {
    // Log the request
    state.request_log.lock().unwrap().push(body);

    // Return the next response
    let response = state
        .responses
        .lock()
        .unwrap()
        .remove(0);

    axum::Json(response)
}
```

## Recording Real API Responses

To create fixtures, record real API interactions and save them:

```rust
use std::path::Path;

/// Record an API response to a fixture file for later replay.
pub fn record_response(
    fixture_name: &str,
    request: &serde_json::Value,
    response: &serde_json::Value,
) {
    let fixture = serde_json::json!({
        "request": request,
        "response": response,
    });

    let path = format!("tests/fixtures/recorded_responses/{fixture_name}.json");
    let content = serde_json::to_string_pretty(&fixture).unwrap();
    std::fs::write(&path, content).unwrap();
}

/// Load a recorded response fixture.
pub fn load_fixture(fixture_name: &str) -> serde_json::Value {
    let path = format!("tests/fixtures/recorded_responses/{fixture_name}.json");
    let content = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("Failed to load fixture {path}: {e}"));
    serde_json::from_str(&content)
        .unwrap_or_else(|e| panic!("Failed to parse fixture {path}: {e}"))
}
```

A fixture file looks like this:

```json
{
  "request": {
    "model": "claude-sonnet-4-20250514",
    "messages": [
      {"role": "user", "content": "Say hello"}
    ]
  },
  "response": {
    "content": "Hello! How can I help you today?",
    "tool_calls": null,
    "stop_reason": "end_turn"
  }
}
```

## Writing Integration Tests

Here is a complete integration test that exercises the agent loop:

```rust
// tests/integration/agent_loop.rs
use agent::{Agent, AgentConfig, ProviderConfig};

mod common;
use common::MockLlmServer;

#[tokio::test]
async fn test_simple_greeting() {
    let response = serde_json::json!({
        "content": "Hello! How can I help you with your code today?",
        "tool_calls": null,
        "stop_reason": "end_turn"
    });

    let server = MockLlmServer::start(vec![response]).await;

    let config = AgentConfig {
        provider: ProviderConfig {
            name: "anthropic".to_string(),
            model: "test-model".to_string(),
            api_url: Some(server.api_url()),
            max_tokens: 1024,
            temperature: 0.0,
        },
        ..Default::default()
    };

    let mut agent = Agent::new(config);
    let result = agent.run_prompt("Say hello").await;

    assert!(result.is_ok());
    let output = result.unwrap();
    assert!(output.contains("Hello"));
}

#[tokio::test]
async fn test_tool_call_write_file() {
    // The LLM responds with a tool call, then a final message
    let tool_response = serde_json::json!({
        "content": null,
        "tool_calls": [{
            "name": "write_file",
            "arguments": {
                "path": "hello.txt",
                "content": "Hello, world!"
            }
        }],
        "stop_reason": "tool_use"
    });

    let final_response = serde_json::json!({
        "content": "I've created hello.txt with the greeting.",
        "tool_calls": null,
        "stop_reason": "end_turn"
    });

    let server = MockLlmServer::start(vec![tool_response, final_response]).await;

    // Set up a temporary project directory
    let temp_dir = tempfile::tempdir().unwrap();

    let config = AgentConfig {
        provider: ProviderConfig {
            api_url: Some(server.api_url()),
            ..Default::default()
        },
        ..Default::default()
    };

    let mut agent = Agent::new_with_workdir(config, temp_dir.path().to_path_buf());
    let result = agent.run_prompt("Create a hello.txt file").await;

    assert!(result.is_ok());

    // Verify the file was actually created
    let file_path = temp_dir.path().join("hello.txt");
    assert!(file_path.exists(), "Tool should have created hello.txt");

    let content = std::fs::read_to_string(&file_path).unwrap();
    assert_eq!(content, "Hello, world!");
}
```

## Testing Error Recovery

Verify that error recovery works end to end:

```rust
#[tokio::test]
async fn test_retry_on_network_error() {
    // First two requests fail, third succeeds
    let responses = vec![
        // The mock server can simulate errors by returning error status codes
        serde_json::json!({"error": "rate_limited", "status": 429}),
        serde_json::json!({"error": "rate_limited", "status": 429}),
        serde_json::json!({
            "content": "Hello after retries!",
            "tool_calls": null,
            "stop_reason": "end_turn"
        }),
    ];

    let server = MockLlmServer::start(responses).await;

    let config = AgentConfig {
        provider: ProviderConfig {
            api_url: Some(server.api_url()),
            ..Default::default()
        },
        ..Default::default()
    };

    let mut agent = Agent::new(config);
    let result = agent.run_prompt("Hello").await;

    // Should succeed after retries
    assert!(result.is_ok());
    let output = result.unwrap();
    assert!(output.contains("Hello after retries"));
}

#[tokio::test]
async fn test_malformed_response_recovery() {
    // LLM returns invalid JSON that our recovery should handle
    let responses = vec![
        serde_json::json!({
            "content": "```json\n{\"name\": \"read_file\", \"path\": \"src/main.rs\"}\n```",
            "tool_calls": null,
            "stop_reason": "end_turn"
        }),
    ];

    let server = MockLlmServer::start(responses).await;

    let config = AgentConfig {
        provider: ProviderConfig {
            api_url: Some(server.api_url()),
            ..Default::default()
        },
        ..Default::default()
    };

    let mut agent = Agent::new(config);
    let result = agent.run_prompt("Read main.rs").await;

    // Should not crash even with malformed tool calls
    assert!(result.is_ok());
}
```

## Test Fixtures for File System Operations

Create reusable project fixtures for tests that involve reading or modifying code:

```rust
use std::path::{Path, PathBuf};
use tempfile::TempDir;

/// A test fixture that creates a temporary Rust project.
pub struct ProjectFixture {
    pub dir: TempDir,
}

impl ProjectFixture {
    /// Create a minimal Rust project in a temp directory.
    pub fn new_rust_project() -> Self {
        let dir = TempDir::new().unwrap();

        // Create Cargo.toml
        std::fs::write(
            dir.path().join("Cargo.toml"),
            r#"[package]
name = "test-project"
version = "0.1.0"
edition = "2021"
"#,
        )
        .unwrap();

        // Create src directory and main.rs
        std::fs::create_dir_all(dir.path().join("src")).unwrap();
        std::fs::write(
            dir.path().join("src/main.rs"),
            r#"fn main() {
    println!("Hello, world!");
}
"#,
        )
        .unwrap();

        ProjectFixture { dir }
    }

    /// Create a project with a failing test for the agent to fix.
    pub fn with_failing_test() -> Self {
        let fixture = Self::new_rust_project();

        std::fs::write(
            fixture.dir.path().join("src/lib.rs"),
            r#"pub fn add(a: i32, b: i32) -> i32 {
    a - b  // Bug: should be a + b
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_add() {
        assert_eq!(add(2, 3), 5);
    }
}
"#,
        )
        .unwrap();

        fixture
    }

    pub fn path(&self) -> &Path {
        self.dir.path()
    }
}
```

::: wild In the Wild
Claude Code's test suite includes end-to-end tests that exercise the full agent loop with recorded API responses. OpenCode uses a similar approach with fixture files that capture real LLM interactions. Both projects emphasize testing tool execution side effects (was the file actually written?) rather than just checking the output text, because tool reliability is what determines whether users trust the agent.
:::

## Key Takeaways

- Build a mock LLM server that returns pre-recorded responses in sequence, testing the full HTTP path rather than just mocking functions -- this catches serialization bugs that unit tests miss.
- Record real API interactions as JSON fixture files so tests are deterministic, fast (no network calls), and free (no API costs), while still reflecting realistic LLM behavior.
- Test tool execution side effects (file creation, content modification) by using `tempfile::TempDir` for isolated project directories that are automatically cleaned up after each test.
- Include error recovery tests that verify the agent handles network failures, malformed responses, and tool errors without crashing -- these are the paths that break most often in production.
- Create reusable `ProjectFixture` helpers that set up realistic project directories (with Cargo.toml, source files, and intentional bugs) for consistent and maintainable integration tests.
