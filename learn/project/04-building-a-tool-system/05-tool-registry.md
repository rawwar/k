---
title: Tool Registry
description: Build a registry that stores tool implementations and provides lookup by name for the dispatch system.
---

# Tool Registry

> **What you'll learn:**
> - How to implement a `ToolRegistry` struct backed by a `HashMap<String, Box<dyn Tool>>`
> - How to register tools at startup and generate the complete tools array for the API request
> - How to look up a tool by name in O(1) time when the model requests a tool call

You have a `Tool` trait and you know how to define schemas. Now you need a place to *store* all your tools so the dispatch system can find them. The tool registry is that place. It is a collection that maps tool names to their implementations, provides O(1) lookup at dispatch time, and generates the `tools` array for API requests. Think of it as the agent's toolbox -- you fill it at startup, and from then on the loop reaches in and grabs whatever tool the model asks for.

## The ToolRegistry Struct

The registry is straightforward: a `HashMap` keyed by tool name, with values being boxed trait objects.

```rust
use std::collections::HashMap;
use serde_json::{json, Value};

pub struct ToolRegistry {
    tools: HashMap<String, Box<dyn Tool>>,
}
```

Why `HashMap`? Because the primary operation is "given a tool name from the model's `tool_use` block, find the corresponding tool." A `HashMap` does this in O(1) average time. You could use a `Vec` and search linearly -- with 5-10 tools that would be fine -- but a `HashMap` scales better and makes the intent clearer.

::: python Coming from Python
In Python, you might use a plain dictionary:

```python
class ToolRegistry:
    def __init__(self):
        self._tools: dict[str, Tool] = {}

    def register(self, tool: Tool) -> None:
        self._tools[tool.name()] = tool

    def get(self, name: str) -> Tool | None:
        return self._tools.get(name)
```

The Rust version is structurally identical. The key difference is that `Box<dyn Tool>` gives you dynamic dispatch (looking up methods at runtime via a vtable), which Python does by default. In Rust, you explicitly opt in to this behavior when you need a heterogeneous collection.
:::

## Implementing the Registry

Here is the complete `ToolRegistry` implementation with registration, lookup, and schema generation:

```rust
use std::collections::HashMap;
use serde_json::{json, Value};
use std::fmt;

// --- ToolError and Tool trait (from subchapter 2) ---

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

// --- The Registry ---

pub struct ToolRegistry {
    tools: HashMap<String, Box<dyn Tool>>,
}

impl ToolRegistry {
    /// Create a new, empty registry.
    pub fn new() -> Self {
        ToolRegistry {
            tools: HashMap::new(),
        }
    }

    /// Register a tool. If a tool with the same name already exists,
    /// it is replaced and the old tool is returned.
    pub fn register(&mut self, tool: Box<dyn Tool>) -> Option<Box<dyn Tool>> {
        let name = tool.name().to_string();
        self.tools.insert(name, tool)
    }

    /// Look up a tool by name. Returns None if the tool is not registered.
    pub fn get(&self, name: &str) -> Option<&dyn Tool> {
        self.tools.get(name).map(|t| t.as_ref())
    }

    /// Return the number of registered tools.
    pub fn len(&self) -> usize {
        self.tools.len()
    }

    /// Check if the registry is empty.
    pub fn is_empty(&self) -> bool {
        self.tools.is_empty()
    }

    /// Generate the `tools` array for the Anthropic API request.
    /// Each element contains `name`, `description`, and `input_schema`.
    pub fn tool_definitions(&self) -> Vec<Value> {
        self.tools
            .values()
            .map(|tool| {
                json!({
                    "name": tool.name(),
                    "description": tool.description(),
                    "input_schema": tool.input_schema()
                })
            })
            .collect()
    }

    /// Return an iterator over all tool names.
    pub fn tool_names(&self) -> impl Iterator<Item = &str> {
        self.tools.keys().map(|s| s.as_str())
    }
}
```

Let's examine each method.

### `new()`

Creates an empty registry. You call this once at application startup, then register all your tools.

### `register(tool: Box<dyn Tool>) -> Option<Box<dyn Tool>>`

Takes ownership of a boxed tool and inserts it into the map. The return type mirrors `HashMap::insert`: if a tool with the same name was already registered, the old one is returned. This lets the caller detect accidental name collisions:

```rust
let mut registry = ToolRegistry::new();
let old = registry.register(Box::new(EchoTool));
assert!(old.is_none()); // First registration: no collision

let old = registry.register(Box::new(EchoTool));
assert!(old.is_some()); // Second registration: replaced the first
```

In production, you might want to log a warning if `register` returns `Some`, since it means two tools claimed the same name.

### `get(name: &str) -> Option<&dyn Tool>`

The dispatch hot path. Given a tool name from the model's response, look up the implementation. Returns `None` if no tool with that name is registered. The caller (the dispatch function) handles the `None` case by returning an error observation to the model.

Notice the return type is `&dyn Tool` -- a reference to the trait object inside the box. The caller does not need ownership; it just needs to call the tool's methods.

### `tool_definitions()`

Generates the `tools` array for the API request. This is called once per agentic loop iteration when constructing the API request body. It iterates over all registered tools and assembles the standard `name`/`description`/`input_schema` objects.

## Wiring Up Registration

Here is how you create and populate a registry at application startup:

```rust
struct EchoTool;

impl Tool for EchoTool {
    fn name(&self) -> &str { "echo" }
    fn description(&self) -> &str { "Echoes the input message back." }
    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "message": {
                    "type": "string",
                    "description": "The message to echo back."
                }
            },
            "required": ["message"]
        })
    }
    fn execute(&self, input: &Value) -> Result<String, ToolError> {
        let msg = input.get("message").and_then(|v| v.as_str())
            .ok_or_else(|| ToolError::InvalidInput("Missing 'message'".into()))?;
        Ok(format!("Echo: {}", msg))
    }
}

fn create_registry() -> ToolRegistry {
    let mut registry = ToolRegistry::new();
    registry.register(Box::new(EchoTool));
    // In later chapters, you will add more tools here:
    // registry.register(Box::new(ReadFileTool));
    // registry.register(Box::new(WriteFileTool));
    // registry.register(Box::new(ShellTool));
    registry
}

fn main() {
    let registry = create_registry();

    println!("Registered {} tool(s):", registry.len());
    for name in registry.tool_names() {
        println!("  - {}", name);
    }

    // Generate the tools array for the API
    let definitions = registry.tool_definitions();
    println!(
        "\nAPI tools array:\n{}",
        serde_json::to_string_pretty(&definitions).unwrap()
    );

    // Look up and execute a tool
    if let Some(tool) = registry.get("echo") {
        let input = json!({"message": "Hello from the registry!"});
        match tool.execute(&input) {
            Ok(result) => println!("\nTool result: {}", result),
            Err(e) => println!("\nTool error: {}", e),
        }
    }
}
```

Running this produces:

```
Registered 1 tool(s):
  - echo

API tools array:
[
  {
    "description": "Echoes the input message back.",
    "input_schema": {
      "properties": {
        "message": {
          "description": "The message to echo back.",
          "type": "string"
        }
      },
      "required": ["message"],
      "type": "object"
    },
    "name": "echo"
  }
]

Tool result: Echo: Hello from the registry!
```

The `create_registry` function is the single point where you configure your agent's capabilities. Adding a new tool to the agent is a one-line change: `registry.register(Box::new(MyNewTool))`. No changes to the loop, the dispatcher, or any other part of the system.

::: wild In the Wild
Claude Code builds its tool registry at startup with a static list of known tools. Each tool is instantiated and registered in a central initialization function. OpenCode takes a similar approach, though its tools register themselves through a builder pattern. Both approaches establish the full set of tools before the agentic loop begins. Dynamic tool registration (adding tools mid-conversation) is possible but uncommon in production agents.
:::

## Design Decisions

A few design choices are worth calling out:

**Owned names vs. references.** The `HashMap` key is `String` (owned), not `&str` (borrowed). This is because the tools are boxed and stored inside the registry -- the borrow checker would complain if the map key borrowed from the tool's `name()` return value, since both the key and the value live inside the same struct. Using `String` avoids this self-referential borrow issue entirely.

**Immutable after setup.** In this design, you build the registry during startup (with `&mut self` methods) and then use it immutably during the loop (with `&self` methods). This is safe for concurrent access without any extra synchronization. If you ever need to add tools mid-loop, you would wrap the registry in an `Arc<RwLock<ToolRegistry>>`.

**No ordering guarantees.** `HashMap` iteration order is not deterministic. If the order of tools in the API request matters (it generally does not), use an `IndexMap` from the `indexmap` crate, which preserves insertion order.

## Key Takeaways

- `ToolRegistry` wraps a `HashMap<String, Box<dyn Tool>>` to provide O(1) lookup by tool name and convenient schema generation.
- `register()` takes ownership of boxed tools and returns the previous tool if a name collision occurs.
- `tool_definitions()` produces the `tools` array for the Anthropic API request by calling each tool's trait methods.
- The registry is built once at startup with `&mut self` methods and then used immutably during the agentic loop -- no locks or synchronization needed.
- Adding a new tool to the agent is a single line: `registry.register(Box::new(MyNewTool))`. The trait enforces that the new tool provides everything the system needs.
