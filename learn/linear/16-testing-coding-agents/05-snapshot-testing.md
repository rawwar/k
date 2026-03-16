---
title: Snapshot Testing
description: Apply snapshot testing to catch unintended changes in tool outputs, API request formatting, and rendered UI components.
---

# Snapshot Testing

> **What you'll learn:**
> - How to use snapshot testing crates (like insta) to capture and compare tool outputs, API request bodies, and formatted messages
> - When snapshot testing is appropriate for agent code and when it creates brittle tests that break on irrelevant changes
> - Techniques for writing snapshot tests that are resilient to non-deterministic fields like timestamps, IDs, and file paths

Snapshot testing captures the output of your code and saves it to a file. On subsequent runs, the test compares the current output against the saved snapshot. If they differ, the test fails and shows you exactly what changed. This approach is powerful for catching unintended regressions in formatted outputs, serialized data structures, and rendered UI components.

For a coding agent, snapshot testing shines in three areas: tool output formatting (how results are presented to the LLM), API request construction (the exact JSON you send to the model provider), and terminal UI rendering (what the user sees). In each case, you want to know when the output changes so you can verify the change is intentional.

## Getting Started with Insta

The `insta` crate is the standard snapshot testing library in Rust. Add it to your dev dependencies:

```toml
[dev-dependencies]
insta = { version = "1", features = ["yaml", "json", "redactions"] }
```

The basic workflow: call `insta::assert_snapshot!()` with a value, and insta saves it to a snapshot file. On the first run, it creates the snapshot. On subsequent runs, it compares:

```rust
use insta::assert_snapshot;

fn format_tool_result(tool_name: &str, output: &str, exit_code: i32) -> String {
    let mut result = String::new();
    result.push_str(&format!("--- {} ---\n", tool_name));
    result.push_str(output);
    if !output.ends_with('\n') {
        result.push('\n');
    }
    result.push_str(&format!("--- exit code: {} ---\n", exit_code));
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn snapshot_tool_result_success() {
        let output = format_tool_result(
            "shell",
            "Hello, world!\nCompilation successful.",
            0,
        );
        assert_snapshot!(output, @r###"
        --- shell ---
        Hello, world!
        Compilation successful.
        --- exit code: 0 ---
        "###);
    }

    #[test]
    fn snapshot_tool_result_error() {
        let output = format_tool_result(
            "shell",
            "error[E0308]: mismatched types",
            1,
        );
        assert_snapshot!(output, @r###"
        --- shell ---
        error[E0308]: mismatched types
        --- exit code: 1 ---
        "###);
    }
}
```

The `@r###"..."###` syntax is an inline snapshot. Insta fills in the expected value on first run, and you commit it with your code. If the output changes, `cargo test` fails and `cargo insta review` opens an interactive diff viewer where you accept or reject the change.

::: python Coming from Python
Python has a similar concept with `pytest-snapshot` or `syrupy`:
```python
def test_tool_output(snapshot):
    result = format_tool_result("shell", "Hello", 0)
    assert result == snapshot
```
The workflow is the same: first run creates the snapshot, subsequent runs compare. Insta's `cargo insta review` command is the equivalent of `pytest --snapshot-update`, but it shows you each change individually so you can accept or reject them one at a time. This selective approval is a major advantage over Python's all-or-nothing update flag.
:::

## Snapshot Testing Structured Data

For structured data like JSON payloads, use `assert_json_snapshot!` which pretty-prints and sorts keys for stable comparisons:

```rust
use insta::assert_json_snapshot;
use serde::Serialize;
use serde_json::json;

#[derive(Serialize)]
struct ApiRequest {
    model: String,
    messages: Vec<ApiMessage>,
    max_tokens: u32,
    tools: Vec<serde_json::Value>,
}

#[derive(Serialize)]
struct ApiMessage {
    role: String,
    content: String,
}

fn build_api_request(user_msg: &str) -> ApiRequest {
    ApiRequest {
        model: "claude-sonnet-4-20250514".to_string(),
        messages: vec![ApiMessage {
            role: "user".to_string(),
            content: user_msg.to_string(),
        }],
        max_tokens: 4096,
        tools: vec![json!({
            "name": "read_file",
            "description": "Read a file from the filesystem",
            "input_schema": {
                "type": "object",
                "properties": {
                    "path": { "type": "string" }
                },
                "required": ["path"]
            }
        })],
    }
}

#[test]
fn snapshot_api_request() {
    let request = build_api_request("Read main.rs");
    assert_json_snapshot!(request, @r###"
    {
      "model": "claude-sonnet-4-20250514",
      "messages": [
        {
          "role": "user",
          "content": "Read main.rs"
        }
      ],
      "max_tokens": 4096,
      "tools": [
        {
          "name": "read_file",
          "description": "Read a file from the filesystem",
          "input_schema": {
            "type": "object",
            "properties": {
              "path": {
                "type": "string"
              }
            },
            "required": [
              "path"
            ]
          }
        }
      ]
    }
    "###);
}
```

This catches subtle bugs like accidentally changing the model name, dropping a required field from a tool schema, or reordering message roles.

## Handling Non-Deterministic Fields with Redactions

Snapshots break when they contain values that change on every run — timestamps, unique IDs, absolute file paths, or token counts. Insta's redaction feature replaces these with stable placeholders:

```rust
use insta::{assert_json_snapshot, with_settings};

#[derive(Serialize)]
struct AgentEvent {
    timestamp: String,
    event_id: String,
    tool_name: String,
    duration_ms: u64,
    working_dir: String,
}

#[test]
fn snapshot_agent_event_with_redactions() {
    let event = AgentEvent {
        timestamp: "2026-03-16T10:30:00Z".to_string(),
        event_id: "evt_abc123".to_string(),
        tool_name: "read_file".to_string(),
        duration_ms: 42,
        working_dir: "/tmp/test-workspace-xK9f2".to_string(),
    };

    with_settings!({
        filters => vec![
            (r"\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}Z", "[TIMESTAMP]"),
            (r"evt_\w+", "[EVENT_ID]"),
            (r"/tmp/test-workspace-\w+", "[WORKSPACE]"),
        ]
    }, {
        assert_json_snapshot!(event, @r###"
        {
          "timestamp": "[TIMESTAMP]",
          "event_id": "[EVENT_ID]",
          "tool_name": "read_file",
          "duration_ms": 42,
          "working_dir": "[WORKSPACE]"
        }
        "###);
    });
}
```

Redactions use regex patterns to replace dynamic values with fixed placeholders. The snapshot stays stable regardless of when or where the test runs.

## When Snapshot Testing Helps

Snapshot testing is ideal for:

- **Tool output formatting**: catching when a formatting change affects what the LLM sees. Since the LLM interprets tool outputs as context, even small formatting changes can affect behavior.
- **API request bodies**: ensuring your HTTP client sends exactly the right payload. Accidentally dropping a field or changing a value is caught immediately.
- **System prompt construction**: verifying that your prompt builder produces the expected prompt text, including tool descriptions and instructions.
- **Error messages**: ensuring user-facing error messages remain consistent and helpful.

```rust
#[test]
fn snapshot_system_prompt() {
    let tools = vec!["read_file", "write_file", "shell"];
    let prompt = build_system_prompt(&tools);
    assert_snapshot!(prompt, @r###"
    You are a coding assistant. You have access to the following tools:
    - read_file: Read a file from the filesystem
    - write_file: Write content to a file
    - shell: Execute a shell command

    Use tools to help the user with their coding tasks. Always read files before modifying them.
    "###);
}
```

## When Snapshot Testing Hurts

Snapshot testing becomes a liability when:

- **The output is intentionally non-deterministic.** LLM responses change every time. Never snapshot test actual model output.
- **The output changes frequently for valid reasons.** If you are iterating on prompt wording daily, snapshot tests for the prompt will fail on every change and become noise.
- **The snapshot is too large.** A 500-line snapshot is hard to review. If someone changes one line, the reviewer must carefully check all 500 lines to verify nothing else was accidentally affected.
- **The test does not assert anything meaningful.** Snapshotting an entire serialized struct "just in case" does not catch specific bugs — it catches all changes, most of which are intentional.

The rule of thumb: snapshot test outputs that are consumed by external systems (the LLM, the API, the user's terminal) where unintended changes have consequences.

::: wild In the Wild
Claude Code uses snapshot-style testing for its tool schema definitions to ensure that changes to tool parameters are intentional. When a developer modifies a tool's input schema, the snapshot test fails, forcing a review of the change. This prevents accidental schema changes that could break the model's ability to use the tool correctly.
:::

## Managing Snapshots in Version Control

Insta stores snapshots in `*.snap` files alongside your test code (or inline in the test itself). Commit these files to version control. When a test fails because the output changed:

1. Run `cargo insta review` to see the diff.
2. Accept the change if it is intentional (press `a`).
3. Reject the change if it is a bug (press `r`).
4. Commit the updated snapshot files.

For inline snapshots (using the `@r###"..."###` syntax), insta updates the source file directly. For external snapshots, it creates `.snap.new` files that you review and accept.

```bash
# Run tests — some snapshot tests fail
cargo test

# Review each change interactively
cargo insta review

# Or accept all changes at once (use with caution)
cargo insta accept
```

## Key Takeaways

- Snapshot testing captures output and flags any change, making it ideal for tool output formatting, API request bodies, system prompts, and error messages
- Use `insta` with `assert_snapshot!` for text, `assert_json_snapshot!` for structured data, and inline snapshots for keeping expected values visible in the test code
- Apply redactions to replace non-deterministic values (timestamps, IDs, paths) with stable placeholders so snapshots do not break on irrelevant changes
- Avoid snapshot testing LLM responses, rapidly changing outputs, or excessively large data structures where the snapshot becomes noise rather than signal
- Commit snapshot files to version control and use `cargo insta review` to selectively accept or reject changes
