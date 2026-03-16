---
title: Integration Testing the Loop
description: Test the complete agentic loop end-to-end using mock providers, verifying that tool calls, results, and conversation turns flow correctly.
---

# Integration Testing the Loop

> **What you'll learn:**
> - How to set up integration test environments with mock providers, real tools, and temporary workspaces that simulate actual agent sessions
> - Techniques for asserting on the full sequence of agent actions — verifying not just the final output but the intermediate tool calls and their ordering
> - How to test loop termination conditions including max turns, stop tokens, tool errors, and user interrupts

Unit tests verify that individual tools work. Mock providers give you scripted LLM responses. Now you combine both to test the full agentic loop — the central loop that receives a user message, calls the LLM, executes tools, feeds results back, and repeats until the task is done.

Integration tests for the agentic loop are the most important tests in your agent. They verify that all the pieces work together: message construction, LLM calling, response parsing, tool dispatch, result formatting, and turn management. When these tests pass, you can be confident that a real LLM interaction would flow through the same code paths correctly.

## Setting Up the Test Environment

An integration test needs three things: a mock provider with scripted responses, a set of real tools, and a temporary workspace for the tools to operate in. Let's build a test harness:

```rust
use tempfile::TempDir;
use std::sync::Arc;

pub struct AgentLoop {
    provider: Arc<dyn LlmProvider>,
    tools: Vec<Box<dyn Tool>>,
    max_turns: usize,
}

pub trait Tool: Send + Sync {
    fn name(&self) -> &str;
    fn definition(&self) -> ToolDefinition;
    fn execute(&self, input: serde_json::Value) -> Result<String, ToolError>;
}

#[derive(Debug)]
pub struct AgentAction {
    pub turn: usize,
    pub kind: ActionKind,
}

#[derive(Debug)]
pub enum ActionKind {
    LlmResponse(String),
    ToolCall { name: String, input: serde_json::Value },
    ToolResult { name: String, output: String },
    Finished(String),
    Error(String),
}

impl AgentLoop {
    pub fn new(
        provider: Arc<dyn LlmProvider>,
        tools: Vec<Box<dyn Tool>>,
        max_turns: usize,
    ) -> Self {
        Self {
            provider,
            tools,
            max_turns,
        }
    }

    pub async fn run(&self, user_message: &str) -> Vec<AgentAction> {
        let mut actions = Vec::new();
        let mut messages = vec![Message {
            role: "user".to_string(),
            content: MessageContent::Text(user_message.to_string()),
        }];

        let tool_defs: Vec<_> = self.tools.iter().map(|t| t.definition()).collect();

        for turn in 0..self.max_turns {
            let response = match self.provider.send(&messages, &tool_defs).await {
                Ok(r) => r,
                Err(e) => {
                    actions.push(AgentAction {
                        turn,
                        kind: ActionKind::Error(format!("{:?}", e)),
                    });
                    break;
                }
            };

            for content in &response.content {
                match content {
                    MessageContent::Text(text) => {
                        actions.push(AgentAction {
                            turn,
                            kind: ActionKind::LlmResponse(text.clone()),
                        });
                    }
                    MessageContent::ToolUse { id, name, input } => {
                        actions.push(AgentAction {
                            turn,
                            kind: ActionKind::ToolCall {
                                name: name.clone(),
                                input: input.clone(),
                            },
                        });

                        // Execute the tool
                        let result = self
                            .tools
                            .iter()
                            .find(|t| t.name() == name)
                            .map(|t| t.execute(input.clone()))
                            .unwrap_or(Err(ToolError::InvalidInput(
                                format!("Unknown tool: {}", name),
                            )));

                        let output = match result {
                            Ok(output) => output,
                            Err(e) => format!("Tool error: {:?}", e),
                        };

                        actions.push(AgentAction {
                            turn,
                            kind: ActionKind::ToolResult {
                                name: name.clone(),
                                output: output.clone(),
                            },
                        });

                        messages.push(Message {
                            role: "assistant".to_string(),
                            content: content.clone(),
                        });
                        messages.push(Message {
                            role: "user".to_string(),
                            content: MessageContent::ToolResult {
                                tool_use_id: id.clone(),
                                content: output,
                            },
                        });
                    }
                    _ => {}
                }
            }

            if response.stop_reason == StopReason::EndTurn {
                if let Some(text) = response.content.iter().find_map(|c| {
                    if let MessageContent::Text(t) = c { Some(t.clone()) } else { None }
                }) {
                    actions.push(AgentAction {
                        turn,
                        kind: ActionKind::Finished(text),
                    });
                }
                break;
            }
        }

        actions
    }
}
```

## Writing the First Integration Test

Here is a complete integration test that verifies a two-turn conversation: the agent reads a file and then summarizes it.

```rust
#[cfg(test)]
mod integration_tests {
    use super::*;
    use serde_json::json;
    use std::sync::Arc;

    struct FakeReadFileTool {
        base_dir: String,
    }

    impl Tool for FakeReadFileTool {
        fn name(&self) -> &str {
            "read_file"
        }

        fn definition(&self) -> ToolDefinition {
            ToolDefinition {
                name: "read_file".into(),
                description: "Read a file".into(),
                input_schema: json!({"type": "object", "properties": {"path": {"type": "string"}}}),
            }
        }

        fn execute(&self, input: serde_json::Value) -> Result<String, ToolError> {
            let path = input["path"].as_str().unwrap();
            let full_path = std::path::Path::new(&self.base_dir).join(path);
            std::fs::read_to_string(full_path)
                .map_err(|e| ToolError::ExecutionFailed(e.to_string()))
        }
    }

    #[tokio::test]
    async fn agent_reads_file_and_summarizes() {
        // Arrange: create a workspace with a file
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("main.rs"),
            "fn main() {\n    println!(\"Hello\");\n}\n",
        )
        .unwrap();

        // Arrange: script the LLM responses
        let provider = Arc::new(MockProvider::new(vec![
            // Turn 1: model asks to read the file
            ResponseBuilder::new()
                .text("Let me read the file.")
                .tool_use("read_file", json!({"path": "main.rs"}))
                .build(),
            // Turn 2: model gives its summary
            ResponseBuilder::new()
                .text("The file is a simple Rust program that prints Hello.")
                .stop_reason(StopReason::EndTurn)
                .build(),
        ]));

        let tools: Vec<Box<dyn Tool>> = vec![Box::new(FakeReadFileTool {
            base_dir: dir.path().to_str().unwrap().to_string(),
        })];

        // Act
        let agent = AgentLoop::new(provider.clone(), tools, 10);
        let actions = agent.run("What does main.rs do?").await;

        // Assert: verify the sequence of actions
        let tool_calls: Vec<_> = actions
            .iter()
            .filter_map(|a| {
                if let ActionKind::ToolCall { name, .. } = &a.kind {
                    Some(name.as_str())
                } else {
                    None
                }
            })
            .collect();
        assert_eq!(tool_calls, vec!["read_file"]);

        // Assert: the loop finished with a text response
        let finished = actions.iter().any(|a| {
            matches!(&a.kind, ActionKind::Finished(text) if text.contains("Hello"))
        });
        assert!(finished, "Agent should finish with a summary mentioning Hello");
    }
}
```

::: python Coming from Python
In pytest, you might use fixtures and async test decorators:
```python
@pytest.fixture
def workspace(tmp_path):
    (tmp_path / "main.rs").write_text('fn main() { println!("Hello"); }')
    return tmp_path

@pytest.mark.asyncio
async def test_reads_and_summarizes(workspace):
    provider = MockProvider([response1, response2])
    agent = AgentLoop(provider, tools, max_turns=10)
    actions = await agent.run("What does main.rs do?")
    assert any(a.tool_name == "read_file" for a in actions)
```
Rust's `#[tokio::test]` macro handles the async runtime setup. Instead of pytest fixtures, you create helper functions or use the `setup` pattern shown above. The test structure — arrange, act, assert — is identical across both languages.
:::

## Testing Tool Call Ordering

Some tasks require tools to be called in a specific order. For example, writing a file should happen after reading the existing content. Assert on the order:

```rust
#[tokio::test]
async fn tools_called_in_correct_order() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("lib.rs"), "// old code").unwrap();

    let provider = Arc::new(MockProvider::new(vec![
        // Turn 1: read the file
        ResponseBuilder::new()
            .tool_use("read_file", json!({"path": "lib.rs"}))
            .build(),
        // Turn 2: write updated content
        ResponseBuilder::new()
            .tool_use("write_file", json!({
                "path": "lib.rs",
                "content": "// new code"
            }))
            .build(),
        // Turn 3: confirm
        ResponseBuilder::new()
            .text("Done! I updated lib.rs.")
            .stop_reason(StopReason::EndTurn)
            .build(),
    ]));

    let tools: Vec<Box<dyn Tool>> = vec![
        // ... read_file and write_file tools
    ];

    let agent = AgentLoop::new(provider, tools, 10);
    let actions = agent.run("Update lib.rs").await;

    let tool_order: Vec<_> = actions
        .iter()
        .filter_map(|a| match &a.kind {
            ActionKind::ToolCall { name, .. } => Some(name.as_str()),
            _ => None,
        })
        .collect();

    assert_eq!(tool_order, vec!["read_file", "write_file"]);
}
```

## Testing Loop Termination

Your loop must terminate under all conditions. Test each termination path:

```rust
#[tokio::test]
async fn stops_at_max_turns() {
    // Script responses that always request another tool call
    let infinite_responses: Vec<_> = (0..20)
        .map(|_| {
            ResponseBuilder::new()
                .tool_use("read_file", json!({"path": "test.txt"}))
                .build()
        })
        .collect();

    let provider = Arc::new(MockProvider::new(infinite_responses));
    let tools: Vec<Box<dyn Tool>> = vec![
        // ... tools that always succeed
    ];

    let agent = AgentLoop::new(provider.clone(), tools, 5);
    let actions = agent.run("Do something").await;

    // The loop should stop after max_turns, not run forever
    let tool_calls = actions
        .iter()
        .filter(|a| matches!(&a.kind, ActionKind::ToolCall { .. }))
        .count();
    assert!(tool_calls <= 5, "Loop should respect max_turns limit");
}

#[tokio::test]
async fn handles_provider_error_gracefully() {
    let provider = Arc::new(FailingProvider::server_error("Internal Server Error"));
    let tools: Vec<Box<dyn Tool>> = vec![];

    let agent = AgentLoop::new(Arc::new(provider) as Arc<dyn LlmProvider>, tools, 10);
    let actions = agent.run("Hello").await;

    let has_error = actions
        .iter()
        .any(|a| matches!(&a.kind, ActionKind::Error(_)));
    assert!(has_error, "Loop should record the error");
}
```

## Testing Tool Error Recovery

When a tool fails, the agent should report the error to the LLM and let it try a different approach. Test this recovery path:

```rust
#[tokio::test]
async fn recovers_from_tool_error() {
    let dir = tempfile::tempdir().unwrap();
    // Note: "missing.rs" does NOT exist

    let provider = Arc::new(MockProvider::new(vec![
        // Turn 1: model tries to read a nonexistent file
        ResponseBuilder::new()
            .tool_use("read_file", json!({"path": "missing.rs"}))
            .build(),
        // Turn 2: model sees the error and recovers
        ResponseBuilder::new()
            .text("The file doesn't exist. Let me create it instead.")
            .stop_reason(StopReason::EndTurn)
            .build(),
    ]));

    let tools: Vec<Box<dyn Tool>> = vec![
        // ... read_file tool that will fail on missing.rs
    ];

    let agent = AgentLoop::new(provider, tools, 10);
    let actions = agent.run("Read missing.rs").await;

    // Verify the error was captured and the loop continued
    let has_tool_error = actions.iter().any(|a| {
        matches!(&a.kind, ActionKind::ToolResult { output, .. } if output.contains("error"))
    });
    assert!(has_tool_error);

    // Verify the loop eventually finished
    let finished = actions
        .iter()
        .any(|a| matches!(&a.kind, ActionKind::Finished(_)));
    assert!(finished);
}
```

::: wild In the Wild
Claude Code's integration tests exercise complete conversation flows including error recovery. A typical test scripts a sequence where the model tries a command that fails, observes the error, and tries an alternative approach. This ensures the agentic loop handles real-world failure patterns where the model's first attempt does not always succeed. The test asserts on the full action sequence, not just the final output.
:::

## Integration Tests Live in `tests/`

Rust separates unit tests (inside `src/`) from integration tests (in `tests/` at the crate root). Integration tests have access to your crate's public API but not private internals, which is exactly the right boundary for agentic loop tests:

```
my-agent/
  src/
    lib.rs          # pub fn, pub struct
    tools/
    provider/
  tests/
    agent_loop.rs   # Integration tests go here
    tool_chain.rs   # More integration test files
```

Each file in `tests/` is compiled as a separate crate. Run them with `cargo test --test agent_loop`.

```rust
// tests/agent_loop.rs
use my_agent::{AgentLoop, MockProvider, ResponseBuilder, StopReason};
use serde_json::json;
use std::sync::Arc;

#[tokio::test]
async fn simple_question_no_tools() {
    let provider = Arc::new(MockProvider::new(vec![
        ResponseBuilder::new()
            .text("Rust is a systems programming language.")
            .stop_reason(StopReason::EndTurn)
            .build(),
    ]));

    let agent = AgentLoop::new(provider, vec![], 10);
    let actions = agent.run("What is Rust?").await;

    assert_eq!(actions.len(), 2); // LlmResponse + Finished
}
```

## Key Takeaways

- Integration tests wire up the full agentic loop with a mock provider and real tools operating in a temporary workspace, testing the complete flow from user message to final response
- Assert on the full sequence of actions (tool calls, their order, tool results, final output) rather than just checking the final text
- Test every termination path: normal completion, max turns reached, provider errors, and tool failures
- Test error recovery by scripting a sequence where the first tool call fails and the model adapts
- Place integration tests in the `tests/` directory at the crate root, keeping them separate from unit tests and ensuring they only use the crate's public API
