---
title: From Chatbot to Agent
description: The architectural transformation that turns a simple chat interface into an autonomous agent with tool use and iterative execution.
---

# From Chatbot to Agent

> **What you'll learn:**
> - The three key additions that transform a chatbot into an agent: tool use, inner loops, and state
> - How the "inner loop" of tool execution creates multi-step autonomy within a single user turn
> - Why the transition from chatbot to agent is more about architecture than model capability

In the previous subchapter, you built a REPL that sends messages to an LLM and prints responses. That is a chatbot. It can answer questions, write code, and hold a conversation -- but it cannot *do* anything. It cannot read your files, run your tests, or edit your code. The leap from chatbot to agent is not about getting a smarter model. It is about giving the model the ability to act on the world and observe the results.

This subchapter walks you through the three architectural changes that turn a chatbot into a coding agent. By the end, you will understand the exact structure of an agentic loop and why each piece exists.

## The Three Missing Pieces

A chatbot has a simple cycle: user speaks, model responds, repeat. An agent adds three things:

1. **Tool definitions** -- You tell the model what actions it can take (read a file, run a command, write code). The model can then request these actions in its response.

2. **An inner loop** -- When the model requests a tool, your code executes it, feeds the result back to the model, and lets the model continue. This loop runs inside a single user turn, potentially many times.

3. **Structured state tracking** -- The agent tracks more than conversation history. It monitors tool results, iteration counts, token budgets, and error states to manage the inner loop.

Let's add each piece, one at a time.

## Adding Tool Definitions

The first step is telling the model what tools it has. In the Anthropic API (and most modern LLM APIs), you pass tool definitions alongside your messages. Each tool definition includes a name, a description, and a JSON schema for its parameters:

```rust
use serde_json::json;

fn get_tool_definitions() -> serde_json::Value {
    json!([
        {
            "name": "read_file",
            "description": "Read the contents of a file at the given path",
            "input_schema": {
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "The file path to read"
                    }
                },
                "required": ["path"]
            }
        },
        {
            "name": "run_command",
            "description": "Execute a shell command and return its output",
            "input_schema": {
                "type": "object",
                "properties": {
                    "command": {
                        "type": "string",
                        "description": "The shell command to execute"
                    }
                },
                "required": ["command"]
            }
        }
    ])
}
```

You send these definitions with every API call. The model does not "remember" its tools between calls -- your code provides them fresh each time. This means you can dynamically add or remove tools based on context, permissions, or the current state of the task.

::: python Coming from Python
If you have used the OpenAI or Anthropic Python SDK, this should look familiar. In Python, you would pass a `tools` list to the API call:
```python
response = client.messages.create(
    model="claude-sonnet-4-20250514",
    messages=messages,
    tools=[
        {"name": "read_file", "description": "...", "input_schema": {...}},
    ],
)
```
The Rust equivalent is structurally identical -- you build the same JSON structure. The difference is that Rust's type system can help you validate tool definitions at compile time if you define them as typed structs rather than raw JSON.
:::

## Adding the Inner Loop

Here is where the real transformation happens. When the model decides it needs to use a tool, it signals this through its response. Instead of returning plain text, it returns a **tool use block** that specifies which tool to call and with what parameters.

Your code must detect this, execute the tool, and send the result back as a new message. Then the model gets another chance to respond -- it might produce text, or it might request another tool. This cycle continues until the model decides it has enough information to give a final answer.

Here is the chatbot from the previous subchapter, transformed into an agent:

```rust
use std::io::{self, BufRead, Write};

struct Message {
    role: String,
    content: MessageContent,
}

enum MessageContent {
    Text(String),
    ToolUse { id: String, name: String, input: serde_json::Value },
    ToolResult { tool_use_id: String, content: String },
}

struct LlmResponse {
    content: Vec<ContentBlock>,
    stop_reason: StopReason,
}

enum ContentBlock {
    Text(String),
    ToolUse { id: String, name: String, input: serde_json::Value },
}

enum StopReason {
    EndTurn,    // Model is done, show response to user
    ToolUse,    // Model wants to use a tool, keep looping
}

fn call_llm(messages: &[Message], tools: &serde_json::Value) -> Result<LlmResponse, AgentError> {
    todo!("API call implementation")
}

fn execute_tool(name: &str, input: &serde_json::Value) -> Result<String, AgentError> {
    todo!("Tool execution implementation")
}

struct AgentError(String);

fn agent_turn(history: &mut Vec<Message>, tools: &serde_json::Value) -> Result<String, AgentError> {
    let mut final_text = String::new();

    // The inner loop: keep going until the model stops requesting tools
    loop {
        let response = call_llm(history, tools)?;

        // Process each content block in the response
        for block in &response.content {
            match block {
                ContentBlock::Text(text) => {
                    final_text.push_str(text);
                }
                ContentBlock::ToolUse { id, name, input } => {
                    // Add the assistant's tool request to history
                    history.push(Message {
                        role: "assistant".to_string(),
                        content: MessageContent::ToolUse {
                            id: id.clone(),
                            name: name.clone(),
                            input: input.clone(),
                        },
                    });

                    // Execute the tool
                    let result = execute_tool(name, input)
                        .unwrap_or_else(|e| format!("Error: {}", e.0));

                    // Add the tool result to history
                    history.push(Message {
                        role: "user".to_string(),
                        content: MessageContent::ToolResult {
                            tool_use_id: id.clone(),
                            content: result,
                        },
                    });
                }
            }
        }

        // Check the stop reason
        match response.stop_reason {
            StopReason::EndTurn => break,  // Model is done
            StopReason::ToolUse => continue, // Model wants another tool call
        }
    }

    Ok(final_text)
}
```

Look at the `agent_turn` function carefully. It contains the inner loop -- the `loop { ... }` block that keeps calling the LLM, executing tools, and feeding results back until the model signals it is done. This is the agentic loop. The outer REPL calls `agent_turn` for each user message, and `agent_turn` handles any number of tool calls internally.

## The Two-Loop Architecture

Let's visualize the complete structure:

```text
OUTER LOOP (REPL - user-driven):
  +--> Read user input
  |    |
  |    v
  |    INNER LOOP (Agentic - model-driven):
  |      +--> Call LLM with history + tools
  |      |    |
  |      |    v
  |      |    Stop reason = end_turn? --YES--> Collect final text
  |      |    |                                     |
  |      |    NO (tool_use)                         |
  |      |    |                                     |
  |      |    v                                     |
  |      |    Execute tool                          |
  |      |    |                                     |
  |      |    v                                     |
  |      |    Add tool result to history            |
  |      |    |                                     |
  |      +----+                                     |
  |                                                 |
  |    <------- Print final text <------------------+
  |    |
  +----+
```

The outer loop is controlled by the user -- they decide when to send a message and when to stop. The inner loop is controlled by the model -- it decides when to use tools and when it has enough information to respond. Your code provides the infrastructure for both loops, but the decision-making is split between the human and the LLM.

## What Actually Changed

Let's be precise about what changed from the chatbot to the agent:

| Aspect | Chatbot | Agent |
|--------|---------|-------|
| LLM calls per user turn | Exactly 1 | 1 to many |
| Model can take actions | No | Yes, via tools |
| Evaluation step | Single call | Inner loop |
| History contents | Text messages only | Text + tool calls + tool results |
| Stop condition | Always after one response | When model says "end_turn" |

Notice what did *not* change: the model itself. You can use the exact same Claude model for a chatbot and an agent. The difference is entirely in your code -- the loop structure, the tool definitions, and the message routing.

This is a crucial insight. Agency is not a property of the model. It is a property of the system you build around the model. A capable model makes a better agent, but the architecture is what makes it an agent at all.

::: wild In the Wild
Claude Code uses the exact same Claude model that powers the chat interface at claude.ai. The difference is that Claude Code wraps the model in an agentic loop with tools for file reading, file writing, shell execution, and web search. The model's ability to use these tools comes from the tool definitions passed in the API call and the inner loop that executes them -- not from any special model capability.
:::

## A Concrete Example

Let's trace through a real interaction to see both loops in action. The user asks: "Read the file src/main.rs and tell me what it does."

```text
OUTER LOOP iteration 1:
  Read: User types "Read the file src/main.rs and tell me what it does"

  INNER LOOP iteration 1:
    Call LLM -> model returns: tool_use(read_file, {path: "src/main.rs"})
    Execute read_file("src/main.rs") -> "fn main() { println!(\"hello\"); }"
    Add tool result to history

  INNER LOOP iteration 2:
    Call LLM -> model returns: text("This is a simple Rust program that prints
    'hello' to the console. The main() function is the entry point...")
    Stop reason: end_turn

  Print: Display the model's explanation to the user

OUTER LOOP iteration 2:
  Read: User types next message (or "exit")
```

The user sent one message, but the LLM made two API calls and one tool execution happened in between. From the user's perspective, they asked a question and got an answer. From the system's perspective, a multi-step process unfolded: the model decided it needed to read a file, your code executed that read, the result went back to the model, and the model synthesized a response.

## The Cost of the Inner Loop

There is an important practical consideration: every iteration of the inner loop costs money and time. Each LLM call consumes tokens (input tokens for the growing history, output tokens for the response), and each call takes hundreds of milliseconds to seconds.

A simple question like "What is 2 + 2?" should take one LLM call. But a complex task like "Set up a new Rust project with a CI pipeline" might take 20+ inner loop iterations, each with an LLM call and tool execution. This is why stop conditions, which we cover later in this chapter, are not optional -- they are essential safety rails.

## Key Takeaways

- The transformation from chatbot to agent requires three additions: tool definitions (what the model can do), an inner loop (model-driven execution), and structured state tracking (monitoring the process)
- The inner loop is the core of agency: it lets the model call tools, observe results, and continue reasoning without waiting for user input at each step
- Agency is a property of the system architecture, not the model -- the same LLM powers both chatbots and agents
- The two-loop architecture (outer REPL + inner agentic loop) gives users control over the conversation while giving the model autonomy within each turn
- Each inner loop iteration costs time and money, making stop conditions and iteration limits essential safety mechanisms
