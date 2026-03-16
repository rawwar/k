---
title: Model Context Protocol
description: Understand the Model Context Protocol (MCP) specification and its role as a standard interface between AI agents and external tool/resource providers.
---

# Model Context Protocol

> **What you'll learn:**
> - The MCP specification structure -- transports, message types, capabilities negotiation, and the client-server interaction model
> - How MCP standardizes the way agents discover, invoke, and receive results from external tools and data sources
> - The difference between MCP tools (actions the model can take), resources (data the model can read), and prompts (templates the model can use)

Up to now, every extension mechanism we have discussed is specific to your agent. Your plugin trait, your event bus, your hook system -- a plugin written for your agent only works with your agent. The Model Context Protocol (MCP) changes this. MCP is an open standard that defines how AI agents communicate with external tool and resource servers, and it is rapidly becoming the universal connector for the AI ecosystem.

Think of MCP as "USB for AI agents." Just as USB lets any device work with any computer regardless of the manufacturer, MCP lets any MCP-compatible server work with any MCP-compatible agent. You build your agent's MCP client once, and it gains access to every MCP server in the ecosystem -- database connectors, web scrapers, code analysis tools, documentation servers, and more.

## The MCP Architecture

MCP follows a client-server model built on JSON-RPC 2.0. Your coding agent is the **client** (also called the "host"). External tools and data sources run as **servers**. The protocol defines how they discover each other's capabilities, exchange requests and responses, and manage their lifecycle.

```
┌─────────────────────┐         ┌──────────────────────┐
│    Your Agent        │         │   MCP Server         │
│    (MCP Client)      │         │   (e.g., PostgreSQL) │
│                      │         │                      │
│  ┌────────────────┐  │ JSON-RPC│  ┌────────────────┐  │
│  │ MCP Client Lib │◄─┼────────►│  │ MCP Server Lib │  │
│  └────────────────┘  │  over   │  └────────────────┘  │
│                      │ stdio / │                      │
│  ┌────────────────┐  │ HTTP+SSE│  ┌────────────────┐  │
│  │ Tool Registry  │  │         │  │ Tool Handlers  │  │
│  └────────────────┘  │         │  └────────────────┘  │
│                      │         │                      │
│  ┌────────────────┐  │         │  ┌────────────────┐  │
│  │ Agentic Loop   │  │         │  │ Resource Store │  │
│  └────────────────┘  │         │  └────────────────┘  │
└─────────────────────┘         └──────────────────────┘
```

## Transport Layer

MCP defines two transport mechanisms:

**stdio transport**: The agent spawns the MCP server as a child process and communicates over stdin/stdout. Each message is a single line of JSON followed by a newline. This is the most common transport for local MCP servers.

**HTTP+SSE transport**: The MCP server runs as a standalone HTTP server. The client sends requests as HTTP POST, and the server streams responses back using Server-Sent Events (SSE). This is used for remote MCP servers and shared server instances.

```rust
/// Represents the transport layer for an MCP connection.
pub enum McpTransport {
    /// Child process with stdin/stdout communication.
    Stdio {
        command: String,
        args: Vec<String>,
        env: HashMap<String, String>,
    },
    /// HTTP server with SSE for streaming.
    Http {
        url: String,
        headers: HashMap<String, String>,
    },
}
```

## The JSON-RPC Protocol

All MCP messages follow the JSON-RPC 2.0 format. There are three message types:

**Requests** have an `id`, a `method`, and optional `params`. The sender expects a response.

**Responses** have the same `id` as the request, plus either a `result` or an `error`.

**Notifications** have a `method` and optional `params` but no `id`. They are fire-and-forget.

```rust
use serde::{Deserialize, Serialize};

/// A JSON-RPC 2.0 request.
#[derive(Debug, Serialize, Deserialize)]
pub struct JsonRpcRequest {
    pub jsonrpc: String, // Always "2.0"
    pub id: serde_json::Value, // Usually a number or string
    pub method: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub params: Option<serde_json::Value>,
}

/// A JSON-RPC 2.0 response.
#[derive(Debug, Serialize, Deserialize)]
pub struct JsonRpcResponse {
    pub jsonrpc: String,
    pub id: serde_json::Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<JsonRpcError>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct JsonRpcError {
    pub code: i64,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<serde_json::Value>,
}

/// A JSON-RPC 2.0 notification (no id, no response expected).
#[derive(Debug, Serialize, Deserialize)]
pub struct JsonRpcNotification {
    pub jsonrpc: String,
    pub method: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub params: Option<serde_json::Value>,
}
```

::: python Coming from Python
If you have used Python's JSON-RPC libraries like `jsonrpcserver`, the message format is identical. The difference is that MCP layers a specific set of methods on top of JSON-RPC:
```python
# Python MCP server using the official SDK
from mcp.server import Server
app = Server("my-tool-server")

@app.tool()
async def search_code(query: str) -> str:
    """Search the codebase for matching patterns."""
    # MCP handles the JSON-RPC wrapping automatically
    results = do_search(query)
    return format_results(results)
```
In Rust, you will build the JSON-RPC layer yourself (or use a crate), and the MCP-specific methods are strongly typed with serde structs.
:::

## The MCP Lifecycle

An MCP session follows a defined lifecycle:

### 1. Initialization

The client sends an `initialize` request declaring its capabilities. The server responds with its own capabilities. This handshake establishes what each side supports.

```rust
/// The client's initialize request.
fn build_initialize_request(id: u64) -> JsonRpcRequest {
    JsonRpcRequest {
        jsonrpc: "2.0".to_string(),
        id: serde_json::json!(id),
        method: "initialize".to_string(),
        params: Some(serde_json::json!({
            "protocolVersion": "2024-11-05",
            "capabilities": {
                "tools": {},
                "resources": { "subscribe": true },
                "prompts": {}
            },
            "clientInfo": {
                "name": "my-coding-agent",
                "version": "0.1.0"
            }
        })),
    }
}

/// The server's initialize response (parsed).
#[derive(Debug, Deserialize)]
pub struct InitializeResult {
    #[serde(rename = "protocolVersion")]
    pub protocol_version: String,
    pub capabilities: ServerCapabilities,
    #[serde(rename = "serverInfo")]
    pub server_info: ServerInfo,
}

#[derive(Debug, Deserialize)]
pub struct ServerCapabilities {
    pub tools: Option<serde_json::Value>,
    pub resources: Option<serde_json::Value>,
    pub prompts: Option<serde_json::Value>,
}

#[derive(Debug, Deserialize)]
pub struct ServerInfo {
    pub name: String,
    pub version: String,
}
```

### 2. Initialized Notification

After receiving the server's capabilities, the client sends an `initialized` notification to confirm the handshake is complete. The server should not accept tool calls until this notification is received.

### 3. Normal Operation

During normal operation, the client can:
- Call `tools/list` to discover available tools
- Call `tools/call` to invoke a tool
- Call `resources/list` to discover available resources
- Call `resources/read` to read a resource
- Call `prompts/list` to discover available prompts
- Call `prompts/get` to retrieve a prompt template

### 4. Shutdown

The client sends a shutdown notification or simply closes the transport. For stdio transport, this means closing stdin and waiting for the process to exit.

## MCP Primitives

MCP defines three primitives that servers can provide:

### Tools

Tools are actions the model can take. They have a name, a description (which the LLM reads to decide when to use them), and a JSON Schema for their parameters:

```rust
#[derive(Debug, Deserialize)]
pub struct McpTool {
    pub name: String,
    pub description: String,
    #[serde(rename = "inputSchema")]
    pub input_schema: serde_json::Value, // JSON Schema
}
```

When the LLM decides to use an MCP tool, your agent sends a `tools/call` request to the appropriate server and returns the result to the LLM.

### Resources

Resources are read-only data that can be injected into the agent's context. They have a URI, a name, and a MIME type:

```rust
#[derive(Debug, Deserialize)]
pub struct McpResource {
    pub uri: String,        // e.g., "file:///docs/api.md"
    pub name: String,
    #[serde(rename = "mimeType")]
    pub mime_type: Option<String>,
    pub description: Option<String>,
}
```

Resources are useful for providing documentation, database schemas, API references, and other contextual data to the agent.

### Prompts

Prompts are templates that the server provides for common interactions. They can have parameters that the user fills in:

```rust
#[derive(Debug, Deserialize)]
pub struct McpPrompt {
    pub name: String,
    pub description: Option<String>,
    pub arguments: Option<Vec<McpPromptArgument>>,
}

#[derive(Debug, Deserialize)]
pub struct McpPromptArgument {
    pub name: String,
    pub description: Option<String>,
    pub required: Option<bool>,
}
```

::: wild In the Wild
Claude Code is one of the primary consumers of MCP servers. It supports both stdio and HTTP+SSE transports, configured through its settings file. Users can add MCP servers for database access, API documentation, project-specific tools, and more. The MCP ecosystem is growing rapidly -- there are MCP servers for PostgreSQL, GitHub, Slack, Google Drive, web browsing, and dozens of other integrations. By implementing MCP client support, your agent immediately gains access to this entire ecosystem.
:::

## A Minimal MCP Client

Here is the skeleton of an MCP client that connects to a stdio server:

```rust
use tokio::process::Command;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

pub struct McpClient {
    stdin: tokio::process::ChildStdin,
    stdout: BufReader<tokio::process::ChildStdout>,
    next_id: u64,
    server_capabilities: Option<ServerCapabilities>,
}

impl McpClient {
    /// Spawn an MCP server process and perform the initialization handshake.
    pub async fn connect_stdio(command: &str, args: &[String]) -> Result<Self> {
        let mut child = Command::new(command)
            .args(args)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()?;

        let stdin = child.stdin.take().unwrap();
        let stdout = BufReader::new(child.stdout.take().unwrap());

        let mut client = Self {
            stdin,
            stdout,
            next_id: 1,
            server_capabilities: None,
        };

        // Perform initialization handshake
        client.initialize().await?;

        Ok(client)
    }

    async fn initialize(&mut self) -> Result<()> {
        let request = build_initialize_request(self.next_id());
        let response = self.send_request(request).await?;
        let init_result: InitializeResult =
            serde_json::from_value(response)?;
        self.server_capabilities = Some(init_result.capabilities);

        // Send initialized notification
        let notification = JsonRpcNotification {
            jsonrpc: "2.0".to_string(),
            method: "notifications/initialized".to_string(),
            params: None,
        };
        self.send_notification(notification).await?;

        Ok(())
    }

    async fn send_request(&mut self, request: JsonRpcRequest) -> Result<serde_json::Value> {
        let mut json = serde_json::to_string(&request)?;
        json.push('\n');
        self.stdin.write_all(json.as_bytes()).await?;
        self.stdin.flush().await?;

        let mut response_line = String::new();
        self.stdout.read_line(&mut response_line).await?;
        let response: JsonRpcResponse = serde_json::from_str(&response_line)?;

        if let Some(error) = response.error {
            return Err(anyhow!("MCP error {}: {}", error.code, error.message));
        }

        response.result.ok_or_else(|| anyhow!("Empty MCP response"))
    }

    async fn send_notification(&mut self, notification: JsonRpcNotification) -> Result<()> {
        let mut json = serde_json::to_string(&notification)?;
        json.push('\n');
        self.stdin.write_all(json.as_bytes()).await?;
        self.stdin.flush().await?;
        Ok(())
    }

    fn next_id(&mut self) -> u64 {
        let id = self.next_id;
        self.next_id += 1;
        id
    }
}
```

In the next two subchapters, you will build out the tool-specific and resource-specific client methods on top of this foundation.

## Key Takeaways

- MCP is the **emerging standard protocol** for connecting AI agents to external tool and resource servers, built on JSON-RPC 2.0 with two transport options (stdio and HTTP+SSE).
- The protocol follows an **initialize-operate-shutdown lifecycle** where client and server negotiate capabilities before any tool calls occur.
- MCP defines three primitives: **tools** (actions), **resources** (read-only data), and **prompts** (templates) -- each discovered dynamically from the server.
- The **stdio transport** (spawning the server as a child process) is most common for local development and provides natural process isolation.
- Implementing MCP client support gives your agent instant access to a **growing ecosystem** of servers for databases, APIs, documentation, and specialized tools.
