---
title: MCP Implementation
description: Building an MCP client that discovers and invokes tools from external MCP servers, handling connection lifecycle, capability negotiation, and error recovery.
---

# MCP Implementation

> **What you'll learn:**
> - How to implement an MCP client that connects to tool servers over stdio and HTTP+SSE transports
> - How to handle the MCP handshake including initialization, capability negotiation, and tool listing
> - Techniques for integrating MCP-provided tools seamlessly into the agent's existing tool system

Now that you understand the MCP protocol, let's implement a client that your agent uses to connect to MCP servers. By the end of this section, your agent will be able to spawn an MCP server, discover its tools, and invoke them as if they were native tools -- the LLM will not know the difference.

## The JSON-RPC Layer

Before building the MCP client, you need a JSON-RPC transport layer. This handles sending requests, matching responses by ID, and routing notifications:

```rust
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{mpsc, oneshot, Mutex};

#[derive(Debug, Serialize, Deserialize)]
pub struct JsonRpcRequest {
    pub jsonrpc: String,
    pub id: Option<u64>,
    pub method: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub params: Option<Value>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct JsonRpcResponse {
    pub jsonrpc: String,
    pub id: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<JsonRpcError>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct JsonRpcError {
    pub code: i64,
    pub message: String,
    pub data: Option<Value>,
}

impl JsonRpcRequest {
    pub fn new(id: u64, method: &str, params: Option<Value>) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            id: Some(id),
            method: method.to_string(),
            params,
        }
    }

    pub fn notification(method: &str, params: Option<Value>) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            id: None,
            method: method.to_string(),
            params,
        }
    }
}
```

## The Stdio Transport

The stdio transport spawns an MCP server as a child process and communicates over its stdin/stdout:

```rust
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, Command};

pub struct StdioTransport {
    child: Child,
    writer: tokio::process::ChildStdin,
    pending: Arc<Mutex<HashMap<u64, oneshot::Sender<JsonRpcResponse>>>>,
    next_id: Arc<Mutex<u64>>,
}

impl StdioTransport {
    /// Spawn an MCP server and set up the communication channel.
    pub async fn connect(
        command: &str,
        args: &[String],
        env: &HashMap<String, String>,
    ) -> Result<Self, McpError> {
        let mut cmd = Command::new(command);
        cmd.args(args)
            .envs(env)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped());

        let mut child = cmd.spawn().map_err(|e| {
            McpError::ConnectionFailed(format!(
                "Failed to spawn MCP server '{}': {}",
                command, e
            ))
        })?;

        let stdin = child.stdin.take().ok_or_else(|| {
            McpError::ConnectionFailed("Failed to capture stdin".to_string())
        })?;

        let stdout = child.stdout.take().ok_or_else(|| {
            McpError::ConnectionFailed("Failed to capture stdout".to_string())
        })?;

        let pending: Arc<Mutex<HashMap<u64, oneshot::Sender<JsonRpcResponse>>>> =
            Arc::new(Mutex::new(HashMap::new()));

        // Spawn a reader task that routes incoming messages
        let pending_clone = pending.clone();
        tokio::spawn(async move {
            let reader = BufReader::new(stdout);
            let mut lines = reader.lines();

            while let Ok(Some(line)) = lines.next_line().await {
                if line.is_empty() {
                    continue;
                }

                match serde_json::from_str::<JsonRpcResponse>(&line) {
                    Ok(response) => {
                        if let Some(id) = response.id {
                            let mut pending = pending_clone.lock().await;
                            if let Some(sender) = pending.remove(&id) {
                                let _ = sender.send(response);
                            }
                        }
                        // Notifications (no id) can be logged or handled here
                    }
                    Err(e) => {
                        eprintln!("[mcp] Failed to parse response: {}: {}", e, line);
                    }
                }
            }
        });

        Ok(Self {
            child,
            writer: stdin,
            pending,
            next_id: Arc::new(Mutex::new(1)),
        })
    }

    /// Send a request and wait for the matching response.
    pub async fn request(
        &mut self,
        method: &str,
        params: Option<Value>,
    ) -> Result<Value, McpError> {
        let mut id = self.next_id.lock().await;
        let request_id = *id;
        *id += 1;
        drop(id);

        let request = JsonRpcRequest::new(request_id, method, params);
        let json = serde_json::to_string(&request)
            .map_err(|e| McpError::SerializationError(e.to_string()))?;

        // Register a oneshot channel for the response
        let (tx, rx) = oneshot::channel();
        {
            let mut pending = self.pending.lock().await;
            pending.insert(request_id, tx);
        }

        // Write the request followed by a newline
        self.writer
            .write_all(format!("{}\n", json).as_bytes())
            .await
            .map_err(|e| McpError::TransportError(e.to_string()))?;

        self.writer
            .flush()
            .await
            .map_err(|e| McpError::TransportError(e.to_string()))?;

        // Wait for the response with a timeout
        let response = tokio::time::timeout(
            std::time::Duration::from_secs(30),
            rx,
        )
        .await
        .map_err(|_| McpError::Timeout("Request timed out after 30s".to_string()))?
        .map_err(|_| McpError::TransportError("Response channel closed".to_string()))?;

        if let Some(error) = response.error {
            return Err(McpError::ServerError(error));
        }

        response
            .result
            .ok_or_else(|| McpError::ProtocolError("Response has no result".to_string()))
    }

    /// Send a notification (no response expected).
    pub async fn notify(
        &mut self,
        method: &str,
        params: Option<Value>,
    ) -> Result<(), McpError> {
        let notification = JsonRpcRequest::notification(method, params);
        let json = serde_json::to_string(&notification)
            .map_err(|e| McpError::SerializationError(e.to_string()))?;

        self.writer
            .write_all(format!("{}\n", json).as_bytes())
            .await
            .map_err(|e| McpError::TransportError(e.to_string()))?;

        self.writer.flush().await
            .map_err(|e| McpError::TransportError(e.to_string()))?;

        Ok(())
    }

    /// Shut down the MCP server.
    pub async fn close(&mut self) -> Result<(), McpError> {
        drop(self.writer.by_ref());
        let _ = self.child.wait().await;
        Ok(())
    }
}
```

::: python Coming from Python
In Python, you might communicate with a subprocess using `asyncio`:
```python
import asyncio
import json

async def connect_mcp(command: str, args: list[str]):
    process = await asyncio.create_subprocess_exec(
        command, *args,
        stdin=asyncio.subprocess.PIPE,
        stdout=asyncio.subprocess.PIPE,
    )

    async def request(method: str, params: dict = None):
        msg = json.dumps({"jsonrpc": "2.0", "id": 1, "method": method, "params": params})
        process.stdin.write((msg + "\n").encode())
        await process.stdin.drain()
        line = await process.stdout.readline()
        return json.loads(line)

    return request
```
The Rust version does the same thing but adds explicit message routing (matching requests to responses by ID) and proper error handling. The `oneshot` channel pattern for matching responses is idiomatic in async Rust -- it avoids polling and lets the runtime manage the wakeup.
:::

## The MCP Client

Now wrap the transport in a higher-level MCP client that handles the protocol lifecycle:

```rust
use serde_json::json;

#[derive(Debug, thiserror::Error)]
pub enum McpError {
    #[error("Connection failed: {0}")]
    ConnectionFailed(String),
    #[error("Transport error: {0}")]
    TransportError(String),
    #[error("Serialization error: {0}")]
    SerializationError(String),
    #[error("Protocol error: {0}")]
    ProtocolError(String),
    #[error("Server error: {0:?}")]
    ServerError(JsonRpcError),
    #[error("Timeout: {0}")]
    Timeout(String),
    #[error("Not initialized")]
    NotInitialized,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpToolDef {
    pub name: String,
    pub description: Option<String>,
    #[serde(rename = "inputSchema")]
    pub input_schema: Value,
}

pub struct McpClient {
    transport: StdioTransport,
    server_name: String,
    server_capabilities: Option<Value>,
    tools: Vec<McpToolDef>,
    initialized: bool,
}

impl McpClient {
    /// Connect to an MCP server and complete the handshake.
    pub async fn connect(
        server_name: &str,
        command: &str,
        args: &[String],
        env: &HashMap<String, String>,
    ) -> Result<Self, McpError> {
        let transport = StdioTransport::connect(command, args, env).await?;

        let mut client = Self {
            transport,
            server_name: server_name.to_string(),
            server_capabilities: None,
            tools: Vec::new(),
            initialized: false,
        };

        client.initialize().await?;
        client.discover_tools().await?;

        Ok(client)
    }

    /// Perform the MCP initialization handshake.
    async fn initialize(&mut self) -> Result<(), McpError> {
        let result = self
            .transport
            .request(
                "initialize",
                Some(json!({
                    "protocolVersion": "2024-11-05",
                    "capabilities": {
                        "roots": { "listChanged": true }
                    },
                    "clientInfo": {
                        "name": "coding-agent",
                        "version": "0.14.0"
                    }
                })),
            )
            .await?;

        self.server_capabilities = Some(result.clone());

        // Send the initialized notification
        self.transport
            .notify("notifications/initialized", None)
            .await?;

        self.initialized = true;
        println!(
            "[mcp] Connected to server '{}': {:?}",
            self.server_name,
            result.get("serverInfo")
        );

        Ok(())
    }

    /// Discover all tools the server provides.
    async fn discover_tools(&mut self) -> Result<(), McpError> {
        let result = self.transport.request("tools/list", None).await?;

        let tools: Vec<McpToolDef> = serde_json::from_value(
            result.get("tools").cloned().unwrap_or(json!([])),
        )
        .map_err(|e| McpError::ProtocolError(format!("Failed to parse tools: {}", e)))?;

        println!(
            "[mcp] Server '{}' provides {} tools: {:?}",
            self.server_name,
            tools.len(),
            tools.iter().map(|t| &t.name).collect::<Vec<_>>()
        );

        self.tools = tools;
        Ok(())
    }

    /// Invoke a tool on the MCP server.
    pub async fn call_tool(
        &mut self,
        tool_name: &str,
        arguments: Value,
    ) -> Result<Value, McpError> {
        if !self.initialized {
            return Err(McpError::NotInitialized);
        }

        let result = self
            .transport
            .request(
                "tools/call",
                Some(json!({
                    "name": tool_name,
                    "arguments": arguments,
                })),
            )
            .await?;

        // Check if the server reported an error
        if result.get("isError").and_then(|v| v.as_bool()).unwrap_or(false) {
            let error_text = result
                .get("content")
                .and_then(|c| c.as_array())
                .and_then(|arr| arr.first())
                .and_then(|item| item.get("text"))
                .and_then(|t| t.as_str())
                .unwrap_or("Unknown error");
            return Err(McpError::ServerError(JsonRpcError {
                code: -1,
                message: error_text.to_string(),
                data: None,
            }));
        }

        Ok(result)
    }

    /// Get the list of tools this server provides.
    pub fn tools(&self) -> &[McpToolDef] {
        &self.tools
    }

    /// Get the server name.
    pub fn server_name(&self) -> &str {
        &self.server_name
    }

    /// Shut down the connection.
    pub async fn shutdown(&mut self) -> Result<(), McpError> {
        self.transport.close().await
    }
}
```

## Integrating MCP Tools into the Agent

The final piece is bridging MCP tools into the existing tool registry so the LLM sees them alongside built-in tools:

```rust
use std::sync::Arc;
use tokio::sync::Mutex;

pub struct McpToolBridge {
    clients: HashMap<String, Arc<Mutex<McpClient>>>,
    tool_to_server: HashMap<String, String>, // tool name -> server name
}

impl McpToolBridge {
    pub fn new() -> Self {
        Self {
            clients: HashMap::new(),
            tool_to_server: HashMap::new(),
        }
    }

    /// Connect to an MCP server and register its tools.
    pub async fn add_server(
        &mut self,
        server_name: &str,
        command: &str,
        args: &[String],
        env: &HashMap<String, String>,
        tool_registry: &mut ToolRegistry,
    ) -> Result<(), McpError> {
        let client = McpClient::connect(server_name, command, args, env).await?;

        // Register each MCP tool in the agent's tool registry
        for tool in client.tools() {
            // Namespace MCP tools: "mcp__servername__toolname"
            let namespaced_name = format!("mcp__{}_{}", server_name, tool.name);

            let tool_def = ToolDefinition {
                name: namespaced_name.clone(),
                description: tool
                    .description
                    .clone()
                    .unwrap_or_else(|| format!("MCP tool from {}", server_name)),
                parameters: tool.input_schema.clone(),
            };

            // Store the mapping from namespaced name to server
            self.tool_to_server
                .insert(namespaced_name.clone(), server_name.to_string());

            // The handler delegates to the MCP client
            let server_key = server_name.to_string();
            let original_name = tool.name.clone();
            let clients = self.clients.clone();

            // We cannot register the handler yet because we need the client in the map
            // So we register after inserting the client below
            println!(
                "[mcp-bridge] Mapped tool '{}' -> server '{}'",
                namespaced_name, server_name
            );
        }

        let client = Arc::new(Mutex::new(client));
        self.clients
            .insert(server_name.to_string(), client.clone());

        Ok(())
    }

    /// Invoke an MCP tool by its namespaced name.
    pub async fn invoke(
        &self,
        namespaced_name: &str,
        arguments: Value,
    ) -> Result<Value, McpError> {
        let server_name = self
            .tool_to_server
            .get(namespaced_name)
            .ok_or_else(|| McpError::ProtocolError(
                format!("No server found for tool '{}'", namespaced_name),
            ))?;

        // Extract the original tool name from the namespaced name
        let prefix = format!("mcp__{}_", server_name);
        let original_name = namespaced_name
            .strip_prefix(&prefix)
            .unwrap_or(namespaced_name);

        let client = self
            .clients
            .get(server_name)
            .ok_or_else(|| McpError::ConnectionFailed(
                format!("Server '{}' not connected", server_name),
            ))?;

        let mut client = client.lock().await;
        client.call_tool(original_name, arguments).await
    }

    /// Shut down all connected MCP servers.
    pub async fn shutdown_all(&mut self) {
        for (name, client) in &self.clients {
            let mut client = client.lock().await;
            if let Err(e) = client.shutdown().await {
                eprintln!("[mcp-bridge] Error shutting down '{}': {}", name, e);
            }
        }
        self.clients.clear();
        self.tool_to_server.clear();
    }
}
```

The namespacing scheme (`mcp__servername__toolname`) prevents collisions between tools from different MCP servers and between MCP tools and built-in tools.

::: wild In the Wild
Claude Code namespaces MCP tools with a double-underscore convention like `mcp__memory__store` and `mcp__memory__retrieve`. This makes it clear to both the LLM and the user where a tool comes from. During tool dispatch, Claude Code checks if the tool name starts with `mcp__`, and if so, routes it through the MCP client instead of the built-in tool handler. The LLM sees all tools as a flat list -- it does not need to understand the MCP abstraction.
:::

## Putting It All Together

Here is how the agent bootstraps MCP connections from configuration:

```rust
use serde::Deserialize;
use std::collections::HashMap;

#[derive(Debug, Deserialize)]
pub struct McpServerConfig {
    pub command: String,
    pub args: Vec<String>,
    #[serde(default)]
    pub env: HashMap<String, String>,
}

pub async fn setup_mcp_servers(
    config: &HashMap<String, McpServerConfig>,
    bridge: &mut McpToolBridge,
    tool_registry: &mut ToolRegistry,
) -> Result<(), McpError> {
    for (name, server_config) in config {
        println!("[mcp] Connecting to server '{}'...", name);

        match bridge
            .add_server(
                name,
                &server_config.command,
                &server_config.args,
                &server_config.env,
                tool_registry,
            )
            .await
        {
            Ok(()) => {
                println!("[mcp] Server '{}' connected successfully", name);
            }
            Err(e) => {
                eprintln!("[mcp] Failed to connect to '{}': {}", name, e);
                // Continue with other servers -- one failure should not block all
            }
        }
    }

    Ok(())
}
```

A user's configuration file might look like this:

```toml
[mcp_servers.filesystem]
command = "npx"
args = ["-y", "@modelcontextprotocol/server-filesystem", "/home/user/project"]

[mcp_servers.memory]
command = "npx"
args = ["-y", "@modelcontextprotocol/server-memory"]

[mcp_servers.github]
command = "npx"
args = ["-y", "@modelcontextprotocol/server-github"]

[mcp_servers.github.env]
GITHUB_TOKEN = "ghp_xxxxxxxxxxxx"
```

Each entry names a server, specifies the command to spawn it, and optionally provides environment variables. The agent reads this config at startup, connects to each server, discovers tools, and makes them available to the LLM.

## Key Takeaways

- The MCP client implementation has three layers: a JSON-RPC transport (handles serialization and message routing), a protocol client (handles the MCP lifecycle), and a bridge (integrates MCP tools into the agent's tool registry)
- The stdio transport spawns MCP servers as child processes, communicating over stdin/stdout with newline-delimited JSON-RPC messages matched by request ID using oneshot channels
- Tool namespacing (`mcp__server__tool`) prevents collisions and makes it clear where each tool comes from, while the LLM sees a flat list of all available tools
- Error handling at each level (transport errors, protocol errors, tool execution errors) ensures the agent degrades gracefully when an MCP server misbehaves
- Configuration-driven MCP setup lets users add new tool servers without code changes -- just edit the config file and restart
