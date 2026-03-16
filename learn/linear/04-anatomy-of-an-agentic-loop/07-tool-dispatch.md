---
title: Tool Dispatch
description: Routing detected tool calls to the correct handler, executing them, and managing the execution lifecycle.
---

# Tool Dispatch

> **What you'll learn:**
> - How a tool registry maps tool names to handler functions for dynamic dispatch
> - The execution lifecycle of a tool call from parameter validation through execution to result capture
> - How to implement timeout, cancellation, and permission checks before tool execution

Tool dispatch is the transition from ToolDetected to ToolExecuting in our state machine. You have parsed the LLM's tool calls, validated their parameters, and now you need to actually run them. This phase maps tool names to concrete code and manages the execution lifecycle: parameter extraction, permission checks, execution, timeout enforcement, and result capture.

The design of your dispatch system determines how easy it is to add new tools to your agent. A well-designed dispatcher makes adding a new tool a matter of writing one function and registering it. A poorly designed one requires touching multiple files and understanding complex control flow.

## The Tool Registry Pattern

The simplest dispatcher is a `match` statement:

```rust
fn execute_tool(name: &str, input: &serde_json::Value) -> ToolResult {
    match name {
        "read_file" => tools::read_file(input),
        "write_file" => tools::write_file(input),
        "run_command" => tools::run_command(input),
        _ => ToolResult::error(format!("Unknown tool: {}", name)),
    }
}
```

This works for a small number of tools, but it has a problem: every time you add a tool, you must modify this function. For a coding agent with 10+ tools, the `match` gets unwieldy. More importantly, it hard-codes the tool list, making it impossible to add tools dynamically (e.g., user-defined tools or plugin systems).

The alternative is a **tool registry** -- a data structure that maps tool names to handler functions at runtime:

```rust
use std::collections::HashMap;

type ToolHandler = Box<dyn Fn(&serde_json::Value) -> ToolResult + Send + Sync>;

struct ToolRegistry {
    tools: HashMap<String, RegisteredTool>,
}

struct RegisteredTool {
    definition: ToolDefinition,
    handler: ToolHandler,
}

impl ToolRegistry {
    fn new() -> Self {
        Self {
            tools: HashMap::new(),
        }
    }

    fn register<F>(&mut self, definition: ToolDefinition, handler: F)
    where
        F: Fn(&serde_json::Value) -> ToolResult + Send + Sync + 'static,
    {
        self.tools.insert(
            definition.name.clone(),
            RegisteredTool {
                definition,
                handler: Box::new(handler),
            },
        );
    }

    fn dispatch(&self, name: &str, input: &serde_json::Value) -> ToolResult {
        match self.tools.get(name) {
            Some(tool) => (tool.handler)(input),
            None => ToolResult::error(format!(
                "Unknown tool: '{}'. Available tools: {}",
                name,
                self.tool_names().join(", ")
            )),
        }
    }

    fn definitions(&self) -> Vec<&ToolDefinition> {
        self.tools.values().map(|t| &t.definition).collect()
    }

    fn tool_names(&self) -> Vec<String> {
        self.tools.keys().cloned().collect()
    }
}
```

With the registry, adding a new tool means calling `register` once during startup. The dispatcher does not change. The tool definitions (sent to the LLM) and the handlers (executed on tool calls) live together, so they cannot get out of sync.

::: python Coming from Python
In Python, you might implement a tool registry with a decorator pattern:
```python
tools = {}

def tool(name, description, schema):
    def decorator(func):
        tools[name] = {"handler": func, "definition": {...}}
        return func
    return decorator

@tool("read_file", "Read a file", {"path": {"type": "string"}})
def read_file(params):
    return open(params["path"]).read()
```
Rust cannot use decorators, but the `register` method serves the same purpose. The key difference is that Rust's `Box<dyn Fn>` provides type-safe dynamic dispatch -- the compiler guarantees that every registered handler has the correct signature.
:::

## Setting Up the Registry

Here is how you would register tools at agent startup:

```rust
fn build_tool_registry() -> ToolRegistry {
    let mut registry = ToolRegistry::new();

    registry.register(
        ToolDefinition {
            name: "read_file".to_string(),
            description: "Read the contents of a file at the given path".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "The file path to read"
                    }
                },
                "required": ["path"]
            }),
        },
        |input| {
            let path = input["path"].as_str().unwrap_or("");
            match std::fs::read_to_string(path) {
                Ok(content) => ToolResult::success(content),
                Err(e) => ToolResult::error(format!("Failed to read {}: {}", path, e)),
            }
        },
    );

    registry.register(
        ToolDefinition {
            name: "run_command".to_string(),
            description: "Execute a shell command and return its output".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "command": {
                        "type": "string",
                        "description": "The shell command to execute"
                    }
                },
                "required": ["command"]
            }),
        },
        |input| {
            let command = input["command"].as_str().unwrap_or("");
            match std::process::Command::new("sh")
                .arg("-c")
                .arg(command)
                .output()
            {
                Ok(output) => {
                    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
                    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
                    let combined = if stderr.is_empty() {
                        stdout
                    } else {
                        format!("stdout:\n{}\nstderr:\n{}", stdout, stderr)
                    };
                    ToolResult::success(combined)
                }
                Err(e) => ToolResult::error(format!("Failed to execute command: {}", e)),
            }
        },
    );

    registry
}
```

Each registration bundles the tool's definition (what the LLM sees) with its handler (what your code executes). When you need to send tool definitions to the API, call `registry.definitions()`. When you need to execute a tool, call `registry.dispatch(name, input)`.

## The Execution Lifecycle

Dispatching a tool is not just calling the handler function. A production agent wraps each tool call in a lifecycle that includes several stages:

```text
ToolCall received from LLM
    |
    v
[1. Parameter extraction]  -- Pull typed values from JSON
    |
    v
[2. Permission check]     -- Does the user allow this action?
    |
    v
[3. Pre-execution hooks]  -- Logging, telemetry, safety checks
    |
    v
[4. Execution]            -- Run the actual tool handler
    |
    v
[5. Timeout enforcement]  -- Kill if execution exceeds time limit
    |
    v
[6. Result capture]       -- Collect output, exit codes, errors
    |
    v
[7. Post-execution hooks] -- Logging, cost tracking, cleanup
    |
    v
ToolResult ready for observation
```

Let's implement this lifecycle:

```rust
use std::time::{Duration, Instant};

struct ExecutionConfig {
    timeout: Duration,
    require_permission: bool,
}

impl Default for ExecutionConfig {
    fn default() -> Self {
        Self {
            timeout: Duration::from_secs(120),
            require_permission: true,
        }
    }
}

fn execute_with_lifecycle(
    registry: &ToolRegistry,
    call: &ToolCall,
    config: &ExecutionConfig,
    permission_handler: &dyn PermissionHandler,
) -> ToolResult {
    // Step 1: Check tool exists
    if !registry.tools.contains_key(&call.name) {
        return ToolResult::error(format!(
            "Unknown tool: '{}'. Available: {}",
            call.name,
            registry.tool_names().join(", ")
        ));
    }

    // Step 2: Permission check
    if config.require_permission {
        match permission_handler.check(&call.name, &call.input) {
            Permission::Allowed => {}
            Permission::Denied(reason) => {
                return ToolResult::error(format!(
                    "Permission denied for tool '{}': {}",
                    call.name, reason
                ));
            }
            Permission::NeedsApproval => {
                match permission_handler.request_approval(&call.name, &call.input) {
                    true => {}
                    false => {
                        return ToolResult::error(format!(
                            "User denied permission for tool '{}'",
                            call.name
                        ));
                    }
                }
            }
        }
    }

    // Step 3: Pre-execution logging
    let start = Instant::now();
    log::info!("Executing tool '{}' with input: {}", call.name, call.input);

    // Step 4-5: Execute with timeout
    let result = execute_with_timeout(registry, &call.name, &call.input, config.timeout);

    // Step 6-7: Post-execution logging
    let elapsed = start.elapsed();
    log::info!(
        "Tool '{}' completed in {:?}: {}",
        call.name,
        elapsed,
        if result.is_error { "ERROR" } else { "OK" }
    );

    result
}

fn execute_with_timeout(
    registry: &ToolRegistry,
    name: &str,
    input: &serde_json::Value,
    timeout: Duration,
) -> ToolResult {
    let start = Instant::now();

    // For synchronous tools, we execute directly
    // A production implementation would use tokio::time::timeout for async tools
    let result = registry.dispatch(name, input);

    if start.elapsed() > timeout {
        ToolResult::error(format!(
            "Tool '{}' timed out after {:?}",
            name, timeout
        ))
    } else {
        result
    }
}
```

## Permission Checks

Coding agents execute real actions on the user's system. A tool that runs shell commands can `rm -rf /`. A tool that writes files can overwrite important code. Permission checks are not optional -- they are a safety requirement.

There are several permission models:

```rust
trait PermissionHandler {
    fn check(&self, tool_name: &str, input: &serde_json::Value) -> Permission;
    fn request_approval(&self, tool_name: &str, input: &serde_json::Value) -> bool;
}

enum Permission {
    Allowed,      // Always allowed (e.g., read_file)
    Denied(String), // Never allowed (e.g., blocked by policy)
    NeedsApproval,  // Requires user confirmation
}

struct DefaultPermissionHandler;

impl PermissionHandler for DefaultPermissionHandler {
    fn check(&self, tool_name: &str, _input: &serde_json::Value) -> Permission {
        match tool_name {
            // Read-only tools are always allowed
            "read_file" | "list_directory" | "search_files" => Permission::Allowed,
            // Write tools need approval
            "write_file" | "run_command" => Permission::NeedsApproval,
            // Unknown tools are denied
            _ => Permission::Denied("Unknown tool".to_string()),
        }
    }

    fn request_approval(&self, tool_name: &str, input: &serde_json::Value) -> bool {
        println!("The agent wants to execute: {} with {:?}", tool_name, input);
        println!("Allow? [y/N] ");
        let mut response = String::new();
        std::io::stdin().read_line(&mut response).unwrap();
        response.trim().eq_ignore_ascii_case("y")
    }
}
```

::: tip In the Wild
Claude Code implements a tiered permission system. Read-only operations (reading files, listing directories) are always allowed. Write operations (editing files, running commands) require user approval on first use, with options to allow specific patterns for the rest of the session. When you approve a command like `cargo test`, Claude Code remembers this and does not ask again for similar commands. OpenCode takes a different approach with an explicit permission configuration file where users pre-approve specific tool patterns before starting a session.
:::

## Sequential vs. Parallel Dispatch

When the model requests multiple tool calls in a single response, you have a choice:

**Sequential execution** runs tools one after another. It is simpler and guarantees that tools do not interfere with each other (e.g., one tool writing a file that another tool reads):

```rust
fn dispatch_sequential(
    registry: &ToolRegistry,
    calls: &[ToolCall],
    config: &ExecutionConfig,
    permissions: &dyn PermissionHandler,
) -> Vec<ToolResult> {
    calls
        .iter()
        .map(|call| execute_with_lifecycle(registry, call, config, permissions))
        .collect()
}
```

**Parallel execution** runs independent tools concurrently. It is faster (if you need to read three files, read them all at once) but requires that the tools are safe to run in parallel:

```rust
async fn dispatch_parallel(
    registry: &ToolRegistry,
    calls: &[ToolCall],
    config: &ExecutionConfig,
    permissions: &dyn PermissionHandler,
) -> Vec<ToolResult> {
    let futures: Vec<_> = calls
        .iter()
        .map(|call| {
            // In a real implementation, execute_with_lifecycle would be async
            let result = execute_with_lifecycle(registry, call, config, permissions);
            async move { result }
        })
        .collect();

    futures::future::join_all(futures).await
}
```

The right choice depends on the tools. File reads are safe to parallelize. Shell commands might not be (one might create a file that another expects to exist). A practical approach is to parallelize read-only tools and serialize write tools.

## The ToolResult Type

Every tool execution produces a `ToolResult`:

```rust
struct ToolResult {
    tool_use_id: String,
    content: String,
    is_error: bool,
}

impl ToolResult {
    fn success(content: String) -> Self {
        Self {
            tool_use_id: String::new(), // Set by the caller
            content,
            is_error: false,
        }
    }

    fn error(message: String) -> Self {
        Self {
            tool_use_id: String::new(),
            content: message,
            is_error: true,
        }
    }

    fn with_id(mut self, id: String) -> Self {
        self.tool_use_id = id;
        self
    }
}
```

The `is_error` flag tells the model whether the tool succeeded or failed. This distinction matters because the model handles errors differently from successes. On error, the model might try an alternative approach, fix its parameters and retry, or explain the failure to the user. On success, it uses the result to continue its task.

The `content` field is always a string. Even if the tool produces structured data (like a JSON response from an API), it gets serialized to a string before going back to the model. The model processes all tool results as text.

## Key Takeaways

- A tool registry maps tool names to handler functions, making it easy to add new tools without modifying the dispatch logic -- register once at startup and the dispatcher handles routing
- The tool execution lifecycle includes parameter extraction, permission checks, pre-execution hooks, execution with timeout, result capture, and post-execution logging
- Permission checks are a safety requirement for coding agents: read-only tools can be auto-approved, but write tools (file edits, shell commands) should require user confirmation
- Multiple tool calls can be dispatched sequentially (simpler, safer) or in parallel (faster, requires tools to be independent) -- a practical approach is to parallelize reads and serialize writes
- Every tool execution produces a `ToolResult` with a success/error flag and content string, which the model uses to decide its next action
