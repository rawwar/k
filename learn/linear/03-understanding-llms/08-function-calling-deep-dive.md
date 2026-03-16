---
title: Function Calling Deep Dive
description: Advanced function calling patterns including parallel tool calls, nested tool use, and handling ambiguous or invalid tool requests.
---

# Function Calling Deep Dive

> **What you'll learn:**
> - How parallel tool calling works and when models choose to invoke multiple tools simultaneously
> - Strategies for handling malformed tool calls, missing parameters, and type mismatches
> - Advanced patterns like tool choice forcing, tool-only responses, and conditional tool availability

The previous subchapter covered the tool use protocol fundamentals. Now let's dig into the advanced patterns that separate a basic agent from a robust one. In production, the model will generate malformed tool calls, pass wrong types, call tools that do not make sense in context, and occasionally hallucinate tool names that do not exist. Your agent needs to handle all of these gracefully.

## Parallel Tool Calls

When the model needs information from multiple independent sources, it can request several tool calls in a single response. This is not just a convenience -- it is a significant performance optimization.

Consider a scenario where the model needs to understand a project's structure. Instead of reading files one at a time:

```
Turn 1: Model calls read_file("Cargo.toml")
Turn 2: Agent returns Cargo.toml contents
Turn 3: Model calls read_file("src/main.rs")
Turn 4: Agent returns main.rs contents
Turn 5: Model calls read_file("src/lib.rs")
Turn 6: Agent returns lib.rs contents
```

The model can request all three in one turn:

```json
{
  "content": [
    {"type": "text", "text": "Let me examine the project structure."},
    {
      "type": "tool_use",
      "id": "toolu_01AAA",
      "name": "read_file",
      "input": {"path": "Cargo.toml"}
    },
    {
      "type": "tool_use",
      "id": "toolu_01BBB",
      "name": "read_file",
      "input": {"path": "src/main.rs"}
    },
    {
      "type": "tool_use",
      "id": "toolu_01CCC",
      "name": "read_file",
      "input": {"path": "src/lib.rs"}
    }
  ]
}
```

This reduces a 6-turn interaction to 2 turns (request + results), cutting latency by two-thirds. Your agent should execute these in parallel:

```rust
use tokio::task::JoinSet;

let mut tasks = JoinSet::new();
for tool_call in tool_calls {
    tasks.spawn(async move {
        let result = execute_tool(&tool_call.name, &tool_call.input).await;
        (tool_call.id.clone(), result)
    });
}

let mut results = Vec::new();
while let Some(Ok((id, result))) = tasks.join_next().await {
    results.push(ToolResult { tool_use_id: id, content: result });
}
```

However, not all parallel tool calls are truly independent. If the model calls `write_file` and then `shell("cargo check")` in the same turn, the shell command depends on the file write completing first. Your agent needs to be smart about execution order. A safe heuristic: execute read operations in parallel, but execute write operations and their dependent checks sequentially.

## The `tool_choice` Parameter

Both Anthropic and OpenAI let you control whether the model must use a tool, and optionally which tool.

**Anthropic's `tool_choice`:**

```json
{
  "tool_choice": {"type": "auto"}
}
```

| Value | Behavior |
|---|---|
| `{"type": "auto"}` | Model decides whether to use tools (default) |
| `{"type": "any"}` | Model must use at least one tool |
| `{"type": "tool", "name": "read_file"}` | Model must call this specific tool |

**OpenAI's `tool_choice`:**

```json
{
  "tool_choice": "auto"
}
```

| Value | Behavior |
|---|---|
| `"auto"` | Model decides whether to use tools (default) |
| `"required"` | Model must use at least one tool |
| `{"type": "function", "function": {"name": "read_file"}}` | Model must call this specific tool |
| `"none"` | Model must not use tools |

Forcing tool use is valuable in specific situations:

- **First turn of an agent session**: force a `read_file` or `list_directory` tool call to ensure the model inspects the codebase before answering.
- **After a failed tool call**: force the same tool with corrected parameters to ensure the model retries rather than giving up.
- **Pipeline steps**: when your agent has a fixed workflow (e.g., "always run tests after editing"), force the `shell` tool.

::: python Coming from Python
Python agent frameworks like LangChain use the concept of "tool binding" where you can force the next step in a chain to use a specific tool. In Rust, you control this by setting the `tool_choice` parameter in your API request struct. The pattern is the same -- you are overriding the model's autonomous decision-making when you need deterministic behavior.
:::

## Handling Malformed Tool Calls

Models generate tool call arguments as JSON, and that JSON can be malformed in several ways:

**Missing required parameters:**
```json
{
  "name": "write_file",
  "input": {
    "path": "src/main.rs"
    // "content" is missing but required
  }
}
```

**Wrong types:**
```json
{
  "name": "shell",
  "input": {
    "command": ["ls", "-la"]  // Array instead of string
  }
}
```

**Extra unexpected parameters:**
```json
{
  "name": "read_file",
  "input": {
    "path": "src/main.rs",
    "encoding": "utf-8"  // Not in the schema
  }
}
```

**Hallucinated tool names:**
```json
{
  "name": "search_code",  // This tool doesn't exist
  "input": {"query": "fn main"}
}
```

Your agent should handle each of these:

1. **Missing parameters**: Return an error result explaining which parameter is missing. The model will typically retry with the correct parameters.

2. **Wrong types**: Attempt a reasonable coercion (e.g., join an array into a string). If coercion is not possible, return an error.

3. **Extra parameters**: Ignore them. The tool does not need them, and rejecting the call would be needlessly strict.

4. **Hallucinated tools**: Return an error listing the available tools. This is surprisingly common when the model is trying to accomplish something that the available tools do not directly support.

Here is a robust error result for a hallucinated tool:

```json
{
  "type": "tool_result",
  "tool_use_id": "toolu_01XYZ",
  "is_error": true,
  "content": "Error: Unknown tool 'search_code'. Available tools are: read_file, write_file, shell, list_directory. To search for code, use the shell tool with grep or ripgrep."
}
```

This error message does three things: identifies the problem, lists alternatives, and suggests a workaround. The model almost always recovers from this.

## Conditional Tool Availability

Sometimes you want to change which tools are available based on context. For example:

- **Dangerous operations**: Remove the `shell` tool when the user has not granted execution permissions.
- **Workflow phases**: In the "planning" phase, provide only read tools. In the "implementation" phase, add write tools.
- **Cost optimization**: Remove expensive tools (like web search) when the task does not need them.

Since tool definitions are part of the API request, you can change them on every call:

```rust
fn get_tools_for_phase(phase: &AgentPhase) -> Vec<Tool> {
    let mut tools = vec![read_file_tool(), list_directory_tool()];

    match phase {
        AgentPhase::Planning => {
            // Read-only tools during planning
        }
        AgentPhase::Implementation => {
            tools.push(write_file_tool());
            tools.push(shell_tool());
        }
        AgentPhase::Review => {
            tools.push(shell_tool()); // For running tests
        }
    }

    tools
}
```

Be aware that changing available tools mid-conversation can confuse the model if previous messages reference tools that are no longer available. If you remove a tool, consider whether the conversation history contains calls to that tool, and if so, whether the model might try to call it again.

## Tool Descriptions That Improve Accuracy

The quality of tool descriptions directly affects how reliably the model uses tools. Here are patterns that work well:

**Include return value descriptions:**
```json
{
  "description": "Read a file's contents. Returns the full text content of the file, or an error message if the file does not exist or cannot be read."
}
```

**Include usage guidance:**
```json
{
  "description": "Execute a shell command. Returns stdout, stderr, and exit code. Use this for build commands (cargo check, cargo test), file operations (ls, find, grep), and version control (git status, git diff). Commands run in the project root directory by default."
}
```

**Include negative guidance (when not to use):**
```json
{
  "description": "Write content to a file, creating it if it doesn't exist or overwriting if it does. IMPORTANT: Always read the file first to understand its current contents before writing. Do not use this to append to files — read the current content, modify it, and write the complete new content."
}
```

The description is essentially a mini system prompt for each tool. It shapes when the model decides to use the tool and how it constructs the arguments.

## The Tool Result Feedback Loop

One of the most powerful patterns in agent design is using tool results to guide subsequent tool calls. The model reads the result, reasons about it, and decides what to do next:

```
Model: read_file("src/main.rs")
Result: [file contents with a compilation error]
Model: "I see a type mismatch on line 45. Let me fix it."
Model: write_file("src/main.rs", [corrected contents])
Result: "File written successfully"
Model: shell("cargo check")
Result: "error[E0308]: mismatched types on line 52..."
Model: "There's another error. Let me fix that too."
```

This iterative cycle of observe-reason-act is the core of agent intelligence. The quality of your tool results directly determines how well the model can reason. Include enough context for the model to understand the situation:

- For file reads: the complete contents, not a summary
- For shell commands: stdout, stderr, and the exit code
- For errors: the full error message, not just "failed"

::: wild In the Wild
Production agents like Claude Code invest heavily in the quality of tool results. Shell command results include the exit code, truncated but informative output, and timing information. File read results include line numbers. Error results include suggestions for common fixes. This rich feedback is what enables the model to self-correct through multiple iterations rather than failing on the first error.
:::

## Key Takeaways

- Parallel tool calls reduce interaction turns and latency -- execute independent calls concurrently, but maintain ordering for dependent operations like write-then-verify
- The `tool_choice` parameter lets you force tool use (`any`/`required`), force a specific tool, or prevent tool use entirely -- use this for deterministic workflow steps
- Handle malformed tool calls gracefully: tolerate extra parameters, attempt type coercion, return informative errors for missing parameters and hallucinated tool names
- Tool descriptions are mini system prompts -- include what the tool returns, when to use it, and when not to use it for the best model behavior
- Rich, detailed tool results enable the model's observe-reason-act loop, which is the foundation of agent intelligence
