---
title: Tool Use Protocol
description: How LLMs request tool execution through structured JSON, and how agents feed tool results back into the conversation.
---

# Tool Use Protocol

> **What you'll learn:**
> - The complete lifecycle of a tool use interaction: definition, invocation, execution, and result
> - How tools are defined with JSON Schema and presented to the model at API call time
> - The message flow pattern where the model requests a tool call and the agent returns the result

Tool use is the mechanism that transforms a language model from a text generator into an agent. Without tools, the model can only produce text. With tools, it can read files, run commands, search codebases, and interact with the world. The tool use protocol defines how this works: how you tell the model what tools are available, how the model requests a tool execution, and how you feed the result back. This is the single most important protocol to understand for building a coding agent.

## The Tool Use Lifecycle

Every tool interaction follows a four-step lifecycle:

**Step 1: Definition** -- You define the available tools in the API request, describing each tool's name, purpose, and expected parameters using JSON Schema.

**Step 2: Invocation** -- The model analyzes the conversation and decides to call a tool. It generates a structured tool call with the tool name and arguments.

**Step 3: Execution** -- Your agent receives the tool call, validates it, executes the corresponding operation (reading a file, running a command, etc.), and captures the result.

**Step 4: Result** -- Your agent sends the tool result back to the model in the next API request, allowing the model to process the output and decide what to do next.

This cycle can repeat multiple times. The model might read a file, see an error, run a shell command to investigate, edit the file, and run tests -- each step is one tool use cycle within the larger agentic loop.

## Defining Tools

Tools are defined in the API request as an array of tool objects. Each tool has a name, description, and an `input_schema` that specifies the expected parameters using JSON Schema.

Here is how you would define a simple file-read tool for the Anthropic API:

```json
{
  "tools": [
    {
      "name": "read_file",
      "description": "Read the contents of a file at the given path. Use this to examine source code, configuration files, or any text file in the project.",
      "input_schema": {
        "type": "object",
        "properties": {
          "path": {
            "type": "string",
            "description": "The absolute or relative path to the file to read"
          }
        },
        "required": ["path"]
      }
    }
  ]
}
```

And a shell execution tool:

```json
{
  "name": "shell",
  "description": "Execute a shell command and return its stdout, stderr, and exit code. Use this for running builds, tests, git commands, and file system operations.",
  "input_schema": {
    "type": "object",
    "properties": {
      "command": {
        "type": "string",
        "description": "The shell command to execute"
      },
      "working_directory": {
        "type": "string",
        "description": "The directory to run the command in. Defaults to the project root."
      }
    },
    "required": ["command"]
  }
}
```

Several things matter about tool definitions:

**The description is critical.** The model uses the description to decide when to use the tool. A vague description like "runs stuff" leads to poor tool selection. A specific description like "Execute a shell command and return its stdout, stderr, and exit code" tells the model exactly what the tool does and what it returns.

**JSON Schema defines the contract.** The `input_schema` tells the model what arguments the tool expects. The model generates arguments that conform to this schema. If a parameter is in the `required` array, the model will always include it. Optional parameters are included when the model deems them necessary.

**Tool names should be concise and descriptive.** The model references tools by name, and short, clear names like `read_file`, `write_file`, `shell` work better than verbose names.

## How the Model Invokes a Tool

When the model decides to use a tool, it generates a structured tool call instead of (or alongside) text. Here is what a complete API response looks like when the model calls a tool:

```json
{
  "id": "msg_01XFDUDYJgAACzvnptvVer6C",
  "type": "message",
  "role": "assistant",
  "content": [
    {
      "type": "text",
      "text": "Let me read the main source file to understand the current implementation."
    },
    {
      "type": "tool_use",
      "id": "toolu_01A09q90qw90lq917835lq9",
      "name": "read_file",
      "input": {
        "path": "src/main.rs"
      }
    }
  ],
  "stop_reason": "tool_use",
  "usage": {
    "input_tokens": 1523,
    "output_tokens": 87
  }
}
```

Key details:

- The `stop_reason` is `"tool_use"` (not `"end_turn"`), signaling that the model wants you to execute a tool before it continues.
- The tool call has a unique `id` that you will reference when returning the result.
- The `input` object conforms to the `input_schema` you defined for the tool.
- The model can include text alongside the tool call, explaining its reasoning.

## Executing the Tool

Your agent receives this response, extracts the tool call, and executes it. In pseudocode:

```rust
// Parse the response and find tool_use blocks
for block in response.content {
    match block {
        ContentBlock::ToolUse { id, name, input } => {
            let result = match name.as_str() {
                "read_file" => {
                    let path = input["path"].as_str().unwrap();
                    std::fs::read_to_string(path)
                        .unwrap_or_else(|e| format!("Error: {}", e))
                }
                "shell" => {
                    let command = input["command"].as_str().unwrap();
                    execute_shell_command(command)
                }
                _ => format!("Unknown tool: {}", name),
            };
            // Store the result to send back
            tool_results.push(ToolResult { id, content: result });
        }
        _ => {}
    }
}
```

The execution step is entirely your code. The model generates the request; your agent decides how to fulfill it. This is where you apply safety checks, sandboxing, timeouts, and permission systems.

## Returning the Result

After executing the tool, you send the result back to the model in the next API request. The result is included in the messages array, linked to the original tool call by its ID:

```json
{
  "model": "claude-sonnet-4-20250514",
  "max_tokens": 4096,
  "system": "You are an expert coding assistant...",
  "tools": [...],
  "messages": [
    {"role": "user", "content": "Read the main file and tell me what it does"},
    {
      "role": "assistant",
      "content": [
        {"type": "text", "text": "Let me read the main source file."},
        {
          "type": "tool_use",
          "id": "toolu_01A09q90qw90lq917835lq9",
          "name": "read_file",
          "input": {"path": "src/main.rs"}
        }
      ]
    },
    {
      "role": "user",
      "content": [
        {
          "type": "tool_result",
          "tool_use_id": "toolu_01A09q90qw90lq917835lq9",
          "content": "use std::io;\n\nfn main() {\n    println!(\"Hello, world!\");\n}"
        }
      ]
    }
  ]
}
```

The model receives the entire conversation history including the tool result, and generates its next response. It might respond with text ("The main file contains a simple Hello World program"), or it might call another tool to continue its work.

::: python Coming from Python
If you have used OpenAI's function calling in Python, the flow is similar: define functions, model returns a function call, you execute and return the result. The Rust implementation differs in that you will deserialize JSON into typed structs using `serde`, which catches malformed tool calls at parse time rather than at runtime. This is one of the advantages of building an agent in a typed language -- the compiler helps you handle edge cases in the tool use protocol.
:::

## Tool Use Stop Reasons

The `stop_reason` in the API response tells you why the model stopped generating:

| Stop Reason | Meaning | Agent Action |
|---|---|---|
| `end_turn` | Model finished its response | Display to user, wait for next input |
| `tool_use` | Model wants to execute tool(s) | Execute tool(s), send results, continue loop |
| `max_tokens` | Response hit the token limit | Handle truncation, possibly request continuation |

Your agent loop checks `stop_reason` on every response to decide what to do next. The core loop is:

1. Send messages to the API
2. Check `stop_reason`
3. If `tool_use`: execute tools, append results, go to step 1
4. If `end_turn`: display response to user
5. If `max_tokens`: handle truncation

This is the agentic loop, and the tool use protocol is what drives it.

## Error Handling in Tool Results

When a tool execution fails -- the file does not exist, the command returns a non-zero exit code, the operation times out -- you still need to return a result. The model needs to know about the failure to decide what to do next.

For Anthropic, you can mark the result as an error:

```json
{
  "type": "tool_result",
  "tool_use_id": "toolu_01A09q90qw90lq917835lq9",
  "is_error": true,
  "content": "Error: file not found: src/mian.rs (did you mean src/main.rs?)"
}
```

The `is_error` flag helps the model understand that this is a failure, not a successful result that happens to contain the word "Error." The model typically responds by adjusting its approach -- fixing a typo in the path, trying a different command, or asking the user for clarification.

::: wild In the Wild
Claude Code returns detailed error information in tool results, including exit codes for shell commands, file system error descriptions, and suggestions when a path looks like a typo. This rich error context helps the model self-correct. A common pattern is to include the error type, a description, and the original request that caused it, giving the model everything it needs to retry intelligently.
:::

## Multiple Tools in a Single Turn

The model can request multiple tool calls in a single response. For example, it might want to read two files simultaneously:

```json
{
  "content": [
    {"type": "text", "text": "I'll read both files to compare them."},
    {
      "type": "tool_use",
      "id": "toolu_01ABC",
      "name": "read_file",
      "input": {"path": "src/old_impl.rs"}
    },
    {
      "type": "tool_use",
      "id": "toolu_01DEF",
      "name": "read_file",
      "input": {"path": "src/new_impl.rs"}
    }
  ]
}
```

When this happens, you should execute both tools and return both results in a single user message:

```json
{
  "role": "user",
  "content": [
    {
      "type": "tool_result",
      "tool_use_id": "toolu_01ABC",
      "content": "// old implementation..."
    },
    {
      "type": "tool_result",
      "tool_use_id": "toolu_01DEF",
      "content": "// new implementation..."
    }
  ]
}
```

Your agent can execute multiple tool calls in parallel for better performance, as long as they are independent (e.g., reading two different files). We will cover this in detail in the [Function Calling Deep Dive](/linear/03-understanding-llms/08-function-calling-deep-dive).

## Key Takeaways

- Tool use follows a four-step lifecycle: define tools with JSON Schema, receive tool calls from the model, execute them in your agent code, and return results linked by ID
- Tool definitions include a name, description, and `input_schema` -- the description is the most important part because it guides when the model chooses to use the tool
- The `stop_reason` field drives the agent loop: `tool_use` means execute tools and continue, `end_turn` means the model is done, `max_tokens` means the response was truncated
- Error handling in tool results is critical -- use the `is_error` flag and include enough detail for the model to self-correct and retry
- Multiple tool calls in a single turn are common and can be executed in parallel for performance, with all results returned in one response message
