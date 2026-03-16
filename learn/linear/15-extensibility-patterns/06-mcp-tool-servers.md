---
title: MCP Tool Servers
description: Implement MCP client support for connecting to tool servers that expose executable actions to your coding agent.
---

# MCP Tool Servers

> **What you'll learn:**
> - How to implement the MCP client-side protocol for discovering and invoking tools exposed by external MCP servers
> - Techniques for managing MCP server lifecycle -- spawning subprocess servers, connecting to remote servers, and handling disconnections
> - How to merge MCP-provided tools into your agent's existing tool registry so the LLM can use them alongside built-in tools seamlessly

In the previous subchapter, you built a minimal MCP client that handles the initialization handshake. Now it is time to make it useful. Tool servers are the most common type of MCP server -- they expose actions that your agent's LLM can invoke. A PostgreSQL MCP server might expose `query` and `describe_table` tools. A GitHub MCP server might expose `create_issue`, `list_pull_requests`, and `merge_pr`. By connecting to these servers, your agent gains capabilities without you writing a single line of tool-specific code.

## Discovering Tools

After initialization, you ask the server what tools it provides by sending a `tools/list` request. The server responds with an array of tool definitions, each containing a name, description, and JSON Schema for the input parameters:

```rust
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize)]
pub struct McpToolDefinition {
    pub name: String,
    pub description: String,
    #[serde(rename = "inputSchema")]
    pub input_schema: serde_json::Value,
}

#[derive(Debug, Deserialize)]
struct ToolsListResult {
    tools: Vec<McpToolDefinition>,
}

impl McpClient {
    /// Discover all tools provided by the connected MCP server.
    pub async fn list_tools(&mut self) -> Result<Vec<McpToolDefinition>> {
        let request = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: serde_json::json!(self.next_id()),
            method: "tools/list".to_string(),
            params: None,
        };

        let result = self.send_request(request).await?;
        let tools_result: ToolsListResult = serde_json::from_value(result)?;
        Ok(tools_result.tools)
    }
}
```

The tool descriptions are critical because the LLM reads them to decide when to use each tool. A well-written description like "Query a PostgreSQL database. Returns results as a JSON array of rows." helps the LLM use the tool correctly. A vague description like "Run a query" leads to misuse.

## Invoking Tools

When the LLM decides to use an MCP tool, your agent sends a `tools/call` request with the tool name and arguments. The server executes the action and returns the result:

```rust
#[derive(Debug, Serialize)]
struct ToolCallParams {
    name: String,
    arguments: serde_json::Value,
}

#[derive(Debug, Deserialize)]
pub struct ToolCallResult {
    pub content: Vec<ContentBlock>,
    #[serde(rename = "isError")]
    pub is_error: Option<bool>,
}

#[derive(Debug, Deserialize)]
pub struct ContentBlock {
    #[serde(rename = "type")]
    pub content_type: String,
    pub text: Option<String>,
    pub data: Option<String>,       // Base64-encoded for images
    #[serde(rename = "mimeType")]
    pub mime_type: Option<String>,
}

impl McpClient {
    /// Invoke a tool on the connected MCP server.
    pub async fn call_tool(
        &mut self,
        tool_name: &str,
        arguments: serde_json::Value,
    ) -> Result<ToolCallResult> {
        let request = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: serde_json::json!(self.next_id()),
            method: "tools/call".to_string(),
            params: Some(serde_json::json!({
                "name": tool_name,
                "arguments": arguments,
            })),
        };

        let result = self.send_request(request).await?;
        let call_result: ToolCallResult = serde_json::from_value(result)?;
        Ok(call_result)
    }
}
```

The response uses a `content` array rather than a plain string because MCP tools can return mixed content -- text, images, or other data types. Your agent needs to handle each content type appropriately when constructing the response for the LLM.

## Managing MCP Server Lifecycle

Each MCP server connection has a lifecycle that your agent must manage: spawning, initializing, monitoring, and cleaning up. The MCP server manager handles all of this:

```rust
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Configuration for a single MCP server.
#[derive(Debug, Clone, Deserialize)]
pub struct McpServerConfig {
    pub name: String,
    pub command: String,
    pub args: Vec<String>,
    #[serde(default)]
    pub env: HashMap<String, String>,
    /// Timeout for the initialization handshake.
    #[serde(default = "default_init_timeout_secs")]
    pub init_timeout_secs: u64,
}

fn default_init_timeout_secs() -> u64 { 30 }

/// Tracks the state of a connected MCP server.
pub struct McpServerConnection {
    pub config: McpServerConfig,
    pub client: McpClient,
    pub tools: Vec<McpToolDefinition>,
    pub connected_at: std::time::Instant,
}

/// Manages all MCP server connections.
pub struct McpServerManager {
    connections: RwLock<HashMap<String, McpServerConnection>>,
}

impl McpServerManager {
    pub fn new() -> Self {
        Self {
            connections: RwLock::new(HashMap::new()),
        }
    }

    /// Connect to an MCP server, perform the handshake, and discover tools.
    pub async fn connect(&self, config: McpServerConfig) -> Result<Vec<McpToolDefinition>> {
        let server_name = config.name.clone();

        // Apply timeout to the entire connection + initialization process
        let timeout_duration =
            std::time::Duration::from_secs(config.init_timeout_secs);

        let result = tokio::time::timeout(timeout_duration, async {
            let mut client = McpClient::connect_stdio(
                &config.command,
                &config.args,
            ).await?;

            let tools = client.list_tools().await?;

            println!(
                "Connected to MCP server '{}': {} tools available",
                server_name,
                tools.len()
            );

            for tool in &tools {
                println!("  - {}: {}", tool.name, tool.description);
            }

            Ok::<_, anyhow::Error>((client, tools))
        }).await;

        match result {
            Ok(Ok((client, tools))) => {
                let connection = McpServerConnection {
                    config,
                    client,
                    tools: tools.clone(),
                    connected_at: std::time::Instant::now(),
                };
                self.connections
                    .write()
                    .await
                    .insert(server_name, connection);
                Ok(tools)
            }
            Ok(Err(e)) => Err(anyhow!("Failed to connect to MCP server: {e}")),
            Err(_) => Err(anyhow!(
                "MCP server '{}' initialization timed out after {}s",
                server_name, timeout_duration.as_secs()
            )),
        }
    }

    /// Disconnect from an MCP server and clean up.
    pub async fn disconnect(&self, server_name: &str) -> Result<()> {
        let mut connections = self.connections.write().await;
        if let Some(mut conn) = connections.remove(server_name) {
            // Send shutdown notification if the process is still running
            let _ = conn.client.send_notification(JsonRpcNotification {
                jsonrpc: "2.0".to_string(),
                method: "notifications/cancelled".to_string(),
                params: None,
            }).await;
        }
        Ok(())
    }

    /// Disconnect from all MCP servers.
    pub async fn disconnect_all(&self) {
        let server_names: Vec<String> = self
            .connections
            .read()
            .await
            .keys()
            .cloned()
            .collect();

        for name in server_names {
            if let Err(e) = self.disconnect(&name).await {
                eprintln!("Error disconnecting MCP server '{}': {e}", name);
            }
        }
    }
}
```

::: python Coming from Python
Python MCP clients (like the `mcp` package) provide a similar abstraction:
```python
from mcp import ClientSession, StdioServerParameters

async with ClientSession(
    StdioServerParameters(command="npx", args=["mcp-server-postgres"])
) as session:
    tools = await session.list_tools()
    result = await session.call_tool("query", {"sql": "SELECT 1"})
```
The Rust version is more explicit about lifecycle management -- you own the connection object and must handle cleanup. In Python, the async context manager handles this. In Rust, you implement `Drop` or use the server manager to ensure proper cleanup.
:::

## Merging MCP Tools into the Tool Registry

The key to seamless MCP integration is making MCP tools indistinguishable from built-in tools. The LLM should not need to know whether a tool is built-in or provided by an MCP server. You achieve this by wrapping MCP tools in an adapter that implements your agent's `Tool` trait:

```rust
/// Wraps an MCP tool as a native agent tool.
/// The LLM sees it alongside built-in tools like read_file and shell.
pub struct McpToolAdapter {
    server_name: String,
    tool_def: McpToolDefinition,
    server_manager: Arc<McpServerManager>,
}

impl McpToolAdapter {
    pub fn new(
        server_name: String,
        tool_def: McpToolDefinition,
        server_manager: Arc<McpServerManager>,
    ) -> Self {
        Self { server_name, tool_def, server_manager }
    }
}

#[async_trait::async_trait]
impl Tool for McpToolAdapter {
    fn name(&self) -> &str {
        &self.tool_def.name
    }

    fn description(&self) -> &str {
        &self.tool_def.description
    }

    fn parameters_schema(&self) -> &serde_json::Value {
        &self.tool_def.input_schema
    }

    async fn execute(
        &self,
        arguments: serde_json::Value,
    ) -> Result<String> {
        let mut connections = self.server_manager.connections.write().await;
        let conn = connections
            .get_mut(&self.server_name)
            .ok_or_else(|| anyhow!(
                "MCP server '{}' not connected", self.server_name
            ))?;

        let result = conn.client.call_tool(&self.tool_def.name, arguments).await?;

        if result.is_error.unwrap_or(false) {
            let error_text = result.content.iter()
                .filter_map(|c| c.text.as_deref())
                .collect::<Vec<_>>()
                .join("\n");
            return Err(anyhow!("MCP tool error: {error_text}"));
        }

        // Concatenate all text content blocks
        let output = result.content.iter()
            .filter_map(|c| c.text.as_deref())
            .collect::<Vec<_>>()
            .join("\n");

        Ok(output)
    }
}

/// Register all tools from all connected MCP servers into the tool registry.
pub async fn register_mcp_tools(
    server_manager: &Arc<McpServerManager>,
    tool_registry: &mut ToolRegistry,
) {
    let connections = server_manager.connections.read().await;
    for (server_name, conn) in connections.iter() {
        for tool_def in &conn.tools {
            let adapter = McpToolAdapter::new(
                server_name.clone(),
                tool_def.clone(),
                server_manager.clone(),
            );
            tool_registry.register(Box::new(adapter));
        }
    }
}
```

## Handling Tool Name Conflicts

When MCP tools merge with built-in tools, name collisions can occur. Two MCP servers might both expose a `search` tool, or an MCP tool might collide with a built-in tool. You need a resolution strategy:

```rust
/// Strategies for resolving tool name conflicts.
pub enum ConflictResolution {
    /// Built-in tools always win. MCP tools with conflicting names are skipped.
    BuiltinFirst,
    /// Prefix MCP tools with the server name: "postgres.query".
    PrefixWithServer,
    /// Last registered wins (MCP tools override built-ins).
    LastWins,
}

pub fn resolve_tool_name(
    tool_name: &str,
    server_name: &str,
    existing_tools: &[String],
    strategy: &ConflictResolution,
) -> Option<String> {
    let has_conflict = existing_tools.contains(&tool_name.to_string());

    match strategy {
        ConflictResolution::BuiltinFirst => {
            if has_conflict {
                None // Skip this MCP tool
            } else {
                Some(tool_name.to_string())
            }
        }
        ConflictResolution::PrefixWithServer => {
            if has_conflict {
                Some(format!("{server_name}.{tool_name}"))
            } else {
                Some(tool_name.to_string())
            }
        }
        ConflictResolution::LastWins => {
            Some(tool_name.to_string()) // Will replace existing
        }
    }
}
```

::: wild In the Wild
Claude Code handles MCP tool naming by using the server name as a namespace when needed. If you configure a "postgres" MCP server, its tools appear with their original names unless they conflict with built-in tools. Claude Code also prefixes tool descriptions with the server name so the LLM knows where the tool comes from. This approach balances clean tool names for the common case with disambiguation for edge cases.
:::

## Reconnection and Error Recovery

MCP servers can crash, hang, or become unresponsive. Your agent needs to handle these failures gracefully without crashing itself:

```rust
impl McpServerManager {
    /// Call a tool with automatic reconnection on failure.
    pub async fn call_tool_with_retry(
        &self,
        server_name: &str,
        tool_name: &str,
        arguments: serde_json::Value,
        max_retries: u32,
    ) -> Result<ToolCallResult> {
        let mut last_error = None;

        for attempt in 0..=max_retries {
            if attempt > 0 {
                // Reconnect before retrying
                let config = {
                    let connections = self.connections.read().await;
                    connections.get(server_name)
                        .map(|c| c.config.clone())
                };

                if let Some(config) = config {
                    self.disconnect(server_name).await?;
                    if let Err(e) = self.connect(config).await {
                        last_error = Some(e);
                        continue;
                    }
                } else {
                    return Err(anyhow!("Server '{}' not configured", server_name));
                }
            }

            let mut connections = self.connections.write().await;
            if let Some(conn) = connections.get_mut(server_name) {
                match conn.client.call_tool(tool_name, arguments.clone()).await {
                    Ok(result) => return Ok(result),
                    Err(e) => {
                        last_error = Some(e);
                        // Connection may be broken, drop and retry
                        continue;
                    }
                }
            }
        }

        Err(last_error.unwrap_or_else(|| anyhow!("Failed after {max_retries} retries")))
    }
}
```

## Key Takeaways

- Use `tools/list` to **discover tools** at connection time, then wrap each MCP tool in an adapter that implements your agent's `Tool` trait so the LLM sees a unified tool catalog.
- The **MCP server manager** handles the full lifecycle -- spawning, connecting, monitoring, and cleaning up server processes -- so the rest of the agent does not deal with transport details.
- **Name conflict resolution** (prefix with server name, built-in-first, or last-wins) is essential when merging tools from multiple MCP servers and built-in tools.
- Implement **reconnection logic** to handle MCP server crashes gracefully -- the agent should retry or report the error without crashing itself.
- The adapter pattern makes MCP tools **indistinguishable from built-in tools**, which is critical for a smooth LLM experience.
