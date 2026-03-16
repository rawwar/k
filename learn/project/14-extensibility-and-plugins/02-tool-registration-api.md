---
title: Tool Registration API
description: Building a dynamic tool registration API that lets plugins contribute new tools at runtime, with schema validation, conflict resolution, and deregistration support.
---

# Tool Registration API

> **What you'll learn:**
> - How to implement a runtime tool registry that accepts new tool definitions from plugins
> - Techniques for validating tool schemas and resolving naming conflicts between plugins
> - How to support tool deregistration and replacement for plugin hot-reloading scenarios

In [Chapter 4](/project/04-building-a-tool-system/), you built a tool system with a `Tool` trait and a static set of tools known at compile time. That design works when you control all the tools, but plugins need to register new tools at runtime. Now you will build a dynamic tool registry that accepts tool definitions from plugins, validates their schemas, manages naming conflicts, and integrates seamlessly with the tool dispatch system you already have.

## From Static Dispatch to Dynamic Registry

Your current tool system likely looks something like this -- a match statement or a fixed `Vec<Box<dyn Tool>>` populated at startup:

```rust
// The old static approach
fn dispatch_tool(name: &str, params: serde_json::Value) -> Result<String> {
    match name {
        "read_file" => tools::read_file(params),
        "write_file" => tools::write_file(params),
        "shell" => tools::shell(params),
        _ => Err(anyhow!("Unknown tool: {}", name)),
    }
}
```

A dynamic registry replaces the hard-coded match with a `HashMap` lookup. The registry stores tool metadata (name, description, JSON schema) alongside the callable handler function. Let's build it.

## The Tool Registry

```rust
use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio::sync::RwLock;

/// JSON Schema describing a tool's parameters.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolDefinition {
    pub name: String,
    pub description: String,
    pub parameters: Value, // JSON Schema object
}

/// The result of invoking a tool.
pub type ToolResult = Result<Value, ToolError>;

/// A tool handler is an async function that takes parameters and returns a result.
pub type ToolHandler = Arc<
    dyn Fn(Value) -> Pin<Box<dyn Future<Output = ToolResult> + Send>> + Send + Sync,
>;

#[derive(Debug, thiserror::Error)]
pub enum ToolError {
    #[error("Tool '{0}' not found")]
    NotFound(String),
    #[error("Tool '{0}' already registered by plugin '{1}'")]
    Conflict(String, String),
    #[error("Invalid schema for tool '{0}': {1}")]
    InvalidSchema(String, String),
    #[error("Execution error in tool '{0}': {1}")]
    ExecutionError(String, String),
    #[error("Parameter validation failed for tool '{0}': {1}")]
    ValidationError(String, String),
}

/// A registered tool entry, linking metadata to the callable handler.
struct RegisteredTool {
    definition: ToolDefinition,
    handler: ToolHandler,
    owner: String, // Plugin name that registered this tool
}

/// The dynamic tool registry. Plugins register and deregister tools here.
pub struct ToolRegistry {
    tools: HashMap<String, RegisteredTool>,
}

impl ToolRegistry {
    pub fn new() -> Self {
        Self {
            tools: HashMap::new(),
        }
    }

    /// Register a new tool. Fails if the tool name is already taken.
    pub fn register(
        &mut self,
        owner: &str,
        definition: ToolDefinition,
        handler: ToolHandler,
    ) -> Result<(), ToolError> {
        // Validate the JSON schema structure
        self.validate_schema(&definition)?;

        if let Some(existing) = self.tools.get(&definition.name) {
            return Err(ToolError::Conflict(
                definition.name.clone(),
                existing.owner.clone(),
            ));
        }

        let name = definition.name.clone();
        self.tools.insert(
            name.clone(),
            RegisteredTool {
                definition,
                handler,
                owner: owner.to_string(),
            },
        );

        println!("[registry] Tool '{}' registered by plugin '{}'", name, owner);
        Ok(())
    }

    /// Deregister a tool. Only the owning plugin can remove it.
    pub fn deregister(&mut self, tool_name: &str, owner: &str) -> Result<(), ToolError> {
        match self.tools.get(tool_name) {
            Some(tool) if tool.owner == owner => {
                self.tools.remove(tool_name);
                println!("[registry] Tool '{}' deregistered", tool_name);
                Ok(())
            }
            Some(tool) => Err(ToolError::Conflict(
                format!(
                    "Tool '{}' owned by '{}', not '{}'",
                    tool_name, tool.owner, owner
                ),
                tool.owner.clone(),
            )),
            None => Err(ToolError::NotFound(tool_name.to_string())),
        }
    }

    /// Deregister all tools owned by a specific plugin.
    pub fn deregister_all_by_owner(&mut self, owner: &str) {
        let to_remove: Vec<String> = self
            .tools
            .iter()
            .filter(|(_, t)| t.owner == owner)
            .map(|(name, _)| name.clone())
            .collect();

        for name in &to_remove {
            self.tools.remove(name);
        }

        if !to_remove.is_empty() {
            println!(
                "[registry] Removed {} tools from plugin '{}'",
                to_remove.len(),
                owner
            );
        }
    }

    /// Get all tool definitions for sending to the LLM.
    pub fn list_definitions(&self) -> Vec<ToolDefinition> {
        self.tools
            .values()
            .map(|t| t.definition.clone())
            .collect()
    }

    /// Look up and invoke a tool by name.
    pub async fn invoke(&self, name: &str, params: Value) -> ToolResult {
        let tool = self
            .tools
            .get(name)
            .ok_or_else(|| ToolError::NotFound(name.to_string()))?;

        // Validate parameters against the schema before invocation
        self.validate_params(name, &tool.definition.parameters, &params)?;

        // Call the handler
        (tool.handler)(params).await
    }

    /// Validate that a tool definition has a well-formed JSON Schema.
    fn validate_schema(&self, definition: &ToolDefinition) -> Result<(), ToolError> {
        let schema = &definition.parameters;

        // Must be an object type
        if schema.get("type").and_then(|t| t.as_str()) != Some("object") {
            return Err(ToolError::InvalidSchema(
                definition.name.clone(),
                "Parameters schema must have type: 'object'".to_string(),
            ));
        }

        // Must have a properties field
        if schema.get("properties").is_none() {
            return Err(ToolError::InvalidSchema(
                definition.name.clone(),
                "Parameters schema must have a 'properties' field".to_string(),
            ));
        }

        Ok(())
    }

    /// Basic parameter validation against the schema.
    fn validate_params(
        &self,
        tool_name: &str,
        schema: &Value,
        params: &Value,
    ) -> Result<(), ToolError> {
        // Check required fields
        if let Some(required) = schema.get("required").and_then(|r| r.as_array()) {
            for field in required {
                if let Some(field_name) = field.as_str() {
                    if params.get(field_name).is_none() {
                        return Err(ToolError::ValidationError(
                            tool_name.to_string(),
                            format!("Missing required field: '{}'", field_name),
                        ));
                    }
                }
            }
        }
        Ok(())
    }
}
```

::: python Coming from Python
In Python, you might register tool functions with a simple decorator:
```python
tool_registry = {}

def tool(name: str, schema: dict):
    def decorator(func):
        tool_registry[name] = {"handler": func, "schema": schema}
        return func
    return decorator

@tool("word_count", {"text": "string"})
async def word_count(params: dict) -> dict:
    return {"count": len(params["text"].split())}
```
Rust cannot do decorators, but the pattern is the same: store a callable in a `HashMap` keyed by name. The Rust version uses `Arc<dyn Fn>` where Python uses bare functions. The explicit schema validation in Rust replaces what you might rely on Pydantic for in Python.
:::

## Wrapping Tool Handlers Ergonomically

The `ToolHandler` type signature is verbose. Let's provide a helper that lets plugin authors register tools with closures:

```rust
impl ToolRegistry {
    /// Convenience method to register a tool with a closure.
    pub fn register_tool<F, Fut>(
        &mut self,
        owner: &str,
        definition: ToolDefinition,
        handler: F,
    ) -> Result<(), ToolError>
    where
        F: Fn(Value) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = ToolResult> + Send + 'static,
    {
        let handler: ToolHandler = Arc::new(move |params| {
            Box::pin(handler(params))
        });
        self.register(owner, definition, handler)
    }
}
```

Now a plugin author can write:

```rust
registry.register_tool(
    "my-plugin",
    ToolDefinition {
        name: "line_count".to_string(),
        description: "Count lines in a file".to_string(),
        parameters: serde_json::json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "Path to the file"
                }
            },
            "required": ["path"]
        }),
    },
    |params| async move {
        let path = params["path"].as_str().unwrap_or("");
        let content = tokio::fs::read_to_string(path)
            .await
            .map_err(|e| ToolError::ExecutionError(
                "line_count".to_string(),
                e.to_string(),
            ))?;
        let count = content.lines().count();
        Ok(serde_json::json!({ "line_count": count }))
    },
)?;
```

## Integrating with the LLM

The LLM needs to know which tools are available. When building the API request, pull the tool list from the registry:

```rust
use serde_json::json;

/// Build the tools array for the LLM API request.
pub fn build_tools_for_llm(registry: &ToolRegistry) -> Vec<Value> {
    registry
        .list_definitions()
        .into_iter()
        .map(|def| {
            json!({
                "type": "function",
                "function": {
                    "name": def.name,
                    "description": def.description,
                    "parameters": def.parameters,
                }
            })
        })
        .collect()
}

/// Dispatch a tool call from the LLM to the registry.
pub async fn dispatch_tool_call(
    registry: &ToolRegistry,
    tool_name: &str,
    arguments: Value,
) -> Result<String, ToolError> {
    let result = registry.invoke(tool_name, arguments).await?;
    // Serialize the result back to a string for the LLM
    Ok(serde_json::to_string_pretty(&result)
        .unwrap_or_else(|_| "Error serializing result".to_string()))
}
```

This replaces the static `dispatch_tool` match statement entirely. The agentic loop does not need to know which tools exist -- it just asks the registry.

## Handling Naming Conflicts

When multiple plugins try to register tools with the same name, you have several strategies:

```rust
/// Conflict resolution strategies.
#[derive(Debug, Clone)]
pub enum ConflictStrategy {
    /// Reject the second registration (default).
    RejectDuplicate,
    /// Let the new registration replace the existing one.
    ReplaceExisting,
    /// Namespace tools by plugin: "plugin_name.tool_name".
    Namespace,
}

impl ToolRegistry {
    /// Register with a specific conflict resolution strategy.
    pub fn register_with_strategy(
        &mut self,
        owner: &str,
        mut definition: ToolDefinition,
        handler: ToolHandler,
        strategy: ConflictStrategy,
    ) -> Result<(), ToolError> {
        self.validate_schema(&definition)?;

        match strategy {
            ConflictStrategy::RejectDuplicate => {
                if self.tools.contains_key(&definition.name) {
                    return Err(ToolError::Conflict(
                        definition.name,
                        self.tools[&definition.name].owner.clone(),
                    ));
                }
            }
            ConflictStrategy::ReplaceExisting => {
                // Silently replace -- useful for hot-reloading
                if self.tools.contains_key(&definition.name) {
                    println!(
                        "[registry] Replacing tool '{}' (was owned by '{}')",
                        definition.name,
                        self.tools[&definition.name].owner
                    );
                }
            }
            ConflictStrategy::Namespace => {
                // Prefix with the plugin name
                definition.name = format!("{}.{}", owner, definition.name);
            }
        }

        let name = definition.name.clone();
        self.tools.insert(
            name,
            RegisteredTool {
                definition,
                handler,
                owner: owner.to_string(),
            },
        );
        Ok(())
    }
}
```

The `Namespace` strategy is the safest for large plugin ecosystems -- it prevents collisions by prefixing tool names with the plugin name. The `ReplaceExisting` strategy is useful during hot-reloading, which we will cover in a later section.

::: wild In the Wild
Claude Code takes the static approach -- all tools are compiled in and dispatched through a known set. MCP tools are the exception: they are discovered at runtime from configured MCP servers and merged into the tool list dynamically. OpenCode similarly uses a static tool registry for built-in tools but allows MCP servers to contribute additional tools that are namespaced by server name (e.g., `mcp__memory__store`).
:::

## Thread-Safe Access

Since tool registration can happen during plugin activation (which may be async) and tool invocation happens during the agentic loop, you need thread-safe access. Wrap the registry in `Arc<RwLock<_>>`:

```rust
use std::sync::Arc;
use tokio::sync::RwLock;

pub type SharedToolRegistry = Arc<RwLock<ToolRegistry>>;

/// In the agent loop, invoke tools through the shared registry.
async fn handle_tool_call(
    registry: &SharedToolRegistry,
    name: &str,
    params: Value,
) -> Result<String, ToolError> {
    let reg = registry.read().await;
    let result = reg.invoke(name, params).await?;
    Ok(serde_json::to_string(&result).unwrap_or_default())
}

/// During plugin activation, register tools through the same shared registry.
async fn plugin_register_tools(
    registry: &SharedToolRegistry,
    owner: &str,
) -> Result<(), ToolError> {
    let mut reg = registry.write().await;
    reg.register_tool(
        owner,
        ToolDefinition {
            name: "example".to_string(),
            description: "An example tool".to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {},
            }),
        },
        |_params| async move {
            Ok(serde_json::json!({ "status": "ok" }))
        },
    )
}
```

The `RwLock` allows multiple concurrent readers (tool invocations) but exclusive access for writes (registrations). This matches the access pattern: tools are registered once during activation but invoked many times during the conversation.

## Key Takeaways

- The dynamic `ToolRegistry` replaces static match-based dispatch with a `HashMap` of tool definitions and handlers, letting plugins register tools at runtime
- Each registered tool carries a JSON Schema that is validated at registration time and used for parameter checking before invocation
- Ownership tracking ensures only the registering plugin can deregister its tools, preventing plugins from interfering with each other
- Conflict resolution strategies (reject, replace, namespace) handle the case where multiple plugins register tools with the same name
- Wrapping the registry in `Arc<RwLock<_>>` provides thread-safe concurrent reads during tool invocation and exclusive writes during registration
