---
title: Tool Dispatch
description: Route incoming tool_use requests from the model to the correct tool implementation and collect results.
---

# Tool Dispatch

> **What you'll learn:**
> - How to extract the tool name and input from a tool_use content block and look up the handler in the registry
> - How to call the tool's execute method with the parsed input and await the async result
> - How to handle the case where the model requests a tool that does not exist in the registry

With the registry holding your tools, you now need the code that connects the model's `tool_use` requests to the right tool implementation. This is the dispatch layer -- the router that takes a tool call from the model, looks it up in the registry, calls `execute`, and returns the result. Getting dispatch right is essential because it sits in the critical path of every agentic loop iteration.

## The tool_use Content Block

When the model wants to call a tool, it produces a content block that looks like this:

```json
{
  "type": "tool_use",
  "id": "toolu_01XFDUDYJgAACzvnptvVer8z",
  "name": "read_file",
  "input": {
    "path": "src/main.rs"
  }
}
```

Three pieces of information matter for dispatch:

1. **`id`** -- A unique identifier for this tool call. You must echo it back in the `tool_result` so the model can match the result to its request.
2. **`name`** -- The tool to invoke. This must match a key in your registry.
3. **`input`** -- The arguments, as a JSON object. This gets passed to the tool's `execute` method.

Let's define a Rust struct to represent this:

```rust
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Deserialize, Serialize)]
pub struct ToolUse {
    pub id: String,
    pub name: String,
    pub input: Value,
}
```

When you parse the API response, you extract `ToolUse` structs from the content blocks. Chapter 3 covered how to detect `tool_use` blocks in the response; now you act on them.

## The Dispatch Function

The dispatch function is the core of the routing logic. It takes a `ToolUse` and the registry, looks up the tool, executes it, and returns a `ToolResult`:

```rust
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::fmt;

// --- Types from previous subchapters ---

#[derive(Debug)]
pub enum ToolError {
    InvalidInput(String),
    ExecutionFailed(String),
    SystemError(String),
}

impl fmt::Display for ToolError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ToolError::InvalidInput(msg) => write!(f, "Invalid input: {}", msg),
            ToolError::ExecutionFailed(msg) => write!(f, "Execution failed: {}", msg),
            ToolError::SystemError(msg) => write!(f, "System error: {}", msg),
        }
    }
}

pub trait Tool: Send + Sync {
    fn name(&self) -> &str;
    fn description(&self) -> &str;
    fn input_schema(&self) -> Value;
    fn execute(&self, input: &Value) -> Result<String, ToolError>;
}

pub struct ToolRegistry {
    tools: HashMap<String, Box<dyn Tool>>,
}

impl ToolRegistry {
    pub fn new() -> Self {
        ToolRegistry { tools: HashMap::new() }
    }
    pub fn register(&mut self, tool: Box<dyn Tool>) {
        let name = tool.name().to_string();
        self.tools.insert(name, tool);
    }
    pub fn get(&self, name: &str) -> Option<&dyn Tool> {
        self.tools.get(name).map(|t| t.as_ref())
    }
}

// --- New types for dispatch ---

#[derive(Debug, Deserialize, Serialize)]
pub struct ToolUse {
    pub id: String,
    pub name: String,
    pub input: Value,
}

#[derive(Debug, Serialize)]
pub struct ToolResult {
    pub tool_use_id: String,
    pub content: String,
    pub is_error: bool,
}

/// Dispatch a tool call: look up the tool, execute it, return a ToolResult.
pub fn dispatch_tool_call(
    registry: &ToolRegistry,
    tool_use: &ToolUse,
) -> ToolResult {
    // Step 1: Look up the tool by name
    let tool = match registry.get(&tool_use.name) {
        Some(t) => t,
        None => {
            return ToolResult {
                tool_use_id: tool_use.id.clone(),
                content: format!(
                    "Error: Unknown tool '{}'. Available tools: {:?}",
                    tool_use.name,
                    registry.tool_names().collect::<Vec<_>>()
                ),
                is_error: true,
            };
        }
    };

    // Step 2: Execute the tool with the provided input
    match tool.execute(&tool_use.input) {
        Ok(output) => ToolResult {
            tool_use_id: tool_use.id.clone(),
            content: output,
            is_error: false,
        },
        Err(e) => ToolResult {
            tool_use_id: tool_use.id.clone(),
            content: e.to_string(),
            is_error: true,
        },
    }
}
```

Let's trace through the logic:

**Step 1: Look up the tool.** The function calls `registry.get(&tool_use.name)`. If the tool is not found, it returns a `ToolResult` with `is_error: true` and a helpful message listing the available tools. This message goes back to the model as an observation, giving it a chance to correct itself. Models occasionally hallucinate tool names, especially in early turns before they have "settled in" to the available tools.

**Step 2: Execute.** If the tool is found, call `tool.execute(&tool_use.input)`. The result is either `Ok(output)` or `Err(error)`. Both become a `ToolResult` -- the difference is the `is_error` flag.

## The ToolResult Struct

The `ToolResult` maps directly to the `tool_result` content block in the Anthropic API:

```json
{
  "type": "tool_result",
  "tool_use_id": "toolu_01XFDUDYJgAACzvnptvVer8z",
  "content": "Contents of src/main.rs:\n1: fn main() {\n2:     println!(\"Hello\");\n3: }",
  "is_error": false
}
```

The `tool_use_id` links this result back to the model's original request. The `content` is the tool's output as a string. The `is_error` flag tells the model whether the tool succeeded or failed. When `is_error` is `true`, the model knows something went wrong and can try a different approach.

::: python Coming from Python
Python dispatch is often a dictionary lookup followed by a method call:

```python
def dispatch_tool_call(registry: dict[str, Tool], tool_use: dict) -> dict:
    tool = registry.get(tool_use["name"])
    if tool is None:
        return {"tool_use_id": tool_use["id"], "content": "Unknown tool", "is_error": True}
    try:
        result = tool.execute(tool_use["input"])
        return {"tool_use_id": tool_use["id"], "content": result, "is_error": False}
    except Exception as e:
        return {"tool_use_id": tool_use["id"], "content": str(e), "is_error": True}

```

The Rust version follows the same pattern but uses `Result` instead of exceptions. The `match` on `tool.execute()` is analogous to the `try/except` block. The key difference: Rust forces you to handle both cases. You cannot forget to catch an exception because `Result` is not optional to handle -- the compiler insists you match on it.
:::

## Handling Multiple Tool Calls

The model can request multiple tool calls in a single response. Each `tool_use` block is independent. Your dispatch layer processes them sequentially:

```rust
/// Dispatch all tool calls from a single assistant response.
pub fn dispatch_all(
    registry: &ToolRegistry,
    tool_uses: &[ToolUse],
) -> Vec<ToolResult> {
    tool_uses
        .iter()
        .map(|tu| dispatch_tool_call(registry, tu))
        .collect()
}
```

This processes each tool call in order and collects the results into a `Vec<ToolResult>`. Sequential execution is the simplest approach and correct for most tools. In a later chapter, you might explore parallel dispatch for independent tools (e.g., reading two files simultaneously), but sequential dispatch is the right starting point.

## Integrating Dispatch with the Agentic Loop

Here is how dispatch fits into the agentic loop from Chapter 3. The pseudocode structure is:

```rust
fn agentic_loop(registry: &ToolRegistry) {
    let mut messages: Vec<Value> = vec![/* initial user message */];

    loop {
        // 1. Send messages + tool definitions to the API
        let response = call_api(&messages, &registry.tool_definitions());

        // 2. Check stop reason
        if response.stop_reason == "end_turn" {
            // Model is done -- print final response and break
            break;
        }

        // 3. Extract tool_use blocks from the response
        let tool_uses: Vec<ToolUse> = extract_tool_uses(&response);

        // 4. Dispatch each tool call
        let tool_results = dispatch_all(registry, &tool_uses);

        // 5. Append assistant message + tool results to conversation
        messages.push(assistant_message(&response));
        for result in &tool_results {
            messages.push(tool_result_message(result));
        }

        // 6. Loop back to step 1
    }
}
```

The dispatch layer is step 4. It receives the tool calls, executes them through the registry, and produces results that get appended to the conversation. The loop does not know or care what the tools do -- it just passes data through the dispatch layer.

## A Complete Working Example

Let's put it all together with a runnable example:

```rust
fn main() {
    // Build the registry
    let mut registry = ToolRegistry::new();
    registry.register(Box::new(EchoTool));

    // Simulate a tool_use from the model
    let tool_use = ToolUse {
        id: "toolu_01ABC".to_string(),
        name: "echo".to_string(),
        input: json!({"message": "Hello from dispatch!"}),
    };

    // Dispatch it
    let result = dispatch_tool_call(&registry, &tool_use);
    println!("Result: {:?}", result);
    println!("is_error: {}", result.is_error);
    println!("content: {}", result.content);

    // Simulate an unknown tool
    let bad_call = ToolUse {
        id: "toolu_02DEF".to_string(),
        name: "nonexistent_tool".to_string(),
        input: json!({}),
    };

    let error_result = dispatch_tool_call(&registry, &bad_call);
    println!("\nError result: {:?}", error_result);
    println!("is_error: {}", error_result.is_error);
    println!("content: {}", error_result.content);
}
```

The output shows both the success and error paths:

```
Result: ToolResult { tool_use_id: "toolu_01ABC", content: "Echo: Hello from dispatch!", is_error: false }
is_error: false
content: Echo: Hello from dispatch!

Error result: ToolResult { tool_use_id: "toolu_02DEF", content: "Error: Unknown tool 'nonexistent_tool'. Available tools: [\"echo\"]", is_error: true }
is_error: true
content: Error: Unknown tool 'nonexistent_tool'. Available tools: ["echo"]
```

The error message lists available tools so the model can self-correct. This pattern -- returning helpful errors as observations rather than crashing -- is one of the most important principles in tool system design.

## Key Takeaways

- Dispatch connects the model's `tool_use` requests to tool implementations via the registry's O(1) name lookup.
- The `dispatch_tool_call` function handles two cases: tool found (execute and return result) and tool not found (return error with available tool names).
- `ToolResult` maps directly to the API's `tool_result` content block, with `tool_use_id` for linking, `content` for the output, and `is_error` for signaling failure.
- Multiple tool calls in a single response are dispatched sequentially with `dispatch_all`.
- Unknown tool errors are returned as observations, not panics -- the model gets a chance to self-correct on the next turn.
