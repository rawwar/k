---
title: "MCP Deep Dive"
---

# MCP Deep Dive — Model Context Protocol

Comprehensive guide to the open protocol standardizing how LLM applications
connect to external tools, data sources, and workflows.

---

## 1. What MCP Is and Why It Exists

### The N×M Integration Problem

Before MCP, every AI application that needed to talk to an external system had
to build a bespoke integration. If you had **N** AI apps (Claude, ChatGPT,
Cursor, custom agents) and **M** external systems (GitHub, Jira, databases,
file systems, monitoring), you needed **N × M** individual connectors. Each
connector had its own authentication model, data format, and error handling.

```
Without MCP                          With MCP

  App A ──┬── System 1                App A ──┐
          ├── System 2                App B ──┤     ┌── System 1
          └── System 3                App C ──┼─MCP─┤── System 2
  App B ──┬── System 1                App D ──┘     └── System 3
          ├── System 2
          └── System 3                N + M connectors (linear)
                                      instead of N × M (quadratic)
  N × M connectors (quadratic)
```

This is the same problem the software industry has solved before — with USB for
peripherals, ODBC/JDBC for databases, and LSP for code editors.

### The USB-C Analogy

MCP is frequently described as **"USB-C for AI applications."** Just as USB-C
provides a universal physical and logical interface between devices and
peripherals — so that any laptop can use any monitor, any phone can use any
charger — MCP provides a universal protocol interface between AI applications
and external capabilities. A tool server written once can be used by any
MCP-compatible host, and a host supporting MCP can instantly leverage any
MCP-compatible server.

### History and Governance

MCP was introduced by **Anthropic** in late 2024 as an open-source protocol.
The specification and reference implementations were published under permissive
licenses. In early 2025 the protocol gained rapid traction across the AI
ecosystem. By mid-2025, governance was being transitioned to the
**Linux Foundation** to ensure vendor-neutral stewardship.

Key milestones:

| Date         | Event                                              |
|--------------|----------------------------------------------------|
| Nov 2024     | Anthropic publishes MCP specification               |
| Dec 2024     | TypeScript and Python SDKs released                  |
| Jan 2025     | Claude Desktop ships with MCP client support         |
| Mar 2025     | Protocol revision 2025-03-26 (Streamable HTTP)      |
| Apr 2025     | OpenAI announces MCP support in ChatGPT agents       |
| Mid-2025     | Linux Foundation governance announced                |

### Relationship to LSP

MCP explicitly draws inspiration from the **Language Server Protocol (LSP)**
created by Microsoft for VS Code. LSP solved the N×M problem for programming
language support in editors — instead of each editor implementing support for
each language, each language implements one LSP server and each editor
implements one LSP client.

MCP applies the same architectural insight to AI tool integration:

| Aspect        | LSP                          | MCP                          |
|---------------|------------------------------|------------------------------|
| Domain        | Code intelligence            | AI tool integration          |
| Message format| JSON-RPC 2.0                 | JSON-RPC 2.0                 |
| Roles         | Editor ↔ Language Server     | Host/Client ↔ MCP Server    |
| Discovery     | Capabilities negotiation     | Capabilities negotiation     |
| Transport     | stdio, TCP, pipes            | stdio, Streamable HTTP       |

---

## 2. Protocol Specification

### JSON-RPC 2.0 Foundation

All MCP communication uses **JSON-RPC 2.0** as the wire format. Every message
is one of three types:

```json
// Request (expects a response)
{
  "jsonrpc": "2.0",
  "id": 1,
  "method": "tools/call",
  "params": {
    "name": "search_files",
    "arguments": { "pattern": "*.py" }
  }
}

// Success Response
{
  "jsonrpc": "2.0",
  "id": 1,
  "result": {
    "content": [{ "type": "text", "text": "Found 42 files" }]
  }
}

// Error Response
{
  "jsonrpc": "2.0",
  "id": 1,
  "error": {
    "code": -32602,
    "message": "Invalid params: pattern must be a string"
  }
}

// Notification (no response expected, no "id" field)
{
  "jsonrpc": "2.0",
  "method": "notifications/progress",
  "params": {
    "progressToken": "abc-123",
    "progress": 50,
    "total": 100
  }
}
```

### The Three Roles

MCP defines three distinct roles in the architecture:

**Host** — The user-facing LLM application (e.g., Claude Desktop, VS Code
Copilot, a custom AI agent). The Host manages the overall conversation, decides
which MCP servers to connect to, and enforces security policies. A Host
contains one or more Clients.

**Client** — A protocol-level connector maintained by the Host. Each Client
holds a 1:1 stateful session with a single Server. The Client handles protocol
negotiation, message routing, and capability tracking. Multiple Clients can
exist within one Host, each connected to a different Server.

**Server** — A lightweight process or service that exposes specific
capabilities (tools, resources, prompts) via the MCP protocol. Servers are
designed to be focused: one server for file operations, another for Git,
another for database queries.

```
┌─────────────────────────────────────────────┐
│                    HOST                       │
│  ┌──────────┐  ┌──────────┐  ┌──────────┐   │
│  │ Client 1 │  │ Client 2 │  │ Client 3 │   │
│  └────┬─────┘  └────┬─────┘  └────┬─────┘   │
│       │              │              │         │
└───────┼──────────────┼──────────────┼─────────┘
        │              │              │
   ┌────▼────┐   ┌────▼────┐   ┌────▼────┐
   │ Server  │   │ Server  │   │ Server  │
   │ (Files) │   │  (Git)  │   │  (DB)   │
   └─────────┘   └─────────┘   └─────────┘
```

### Capability Negotiation

During initialization, both Client and Server declare what features they
support. This allows progressive enhancement — a minimal Server can start with
just Tools and add Resources or Prompts later.

```json
// Server capabilities declaration
{
  "capabilities": {
    "tools":     { "listChanged": true },
    "resources": { "subscribe": true, "listChanged": true },
    "prompts":   { "listChanged": true },
    "logging":   {}
  }
}

// Client capabilities declaration
{
  "capabilities": {
    "sampling": {},
    "roots":    { "listChanged": true },
    "elicitation": {}
  }
}
```

### Protocol Lifecycle

Every MCP session follows a defined lifecycle:

```
Client                              Server
  │                                    │
  │─── initialize ────────────────────▶│  Phase 1: Initialization
  │◀── initialize result ─────────────│  (exchange capabilities)
  │─── notifications/initialized ────▶│
  │                                    │
  │◀── tools/list, resources/list ────│  Phase 2: Operation
  │─── tools/call ────────────────────▶│  (normal message exchange)
  │◀── result ─────────────────────────│
  │                                    │
  │─── ping ──────────────────────────▶│  Phase 3: Keep-alive
  │◀── pong ───────────────────────────│  (optional)
  │                                    │
  │─── shutdown (or close transport) ─▶│  Phase 4: Shutdown
  │                                    │
```

1. **Initialize** — Client sends `initialize` with its name, version, and
   capabilities. Server responds with its own capabilities. Client confirms
   with `notifications/initialized`.

2. **Operation** — Normal request/response and notification flow. Either side
   can send requests (if the other side advertised the capability).

3. **Shutdown** — Either side can close the connection. For stdio transport,
   the Client typically terminates the Server subprocess.

### Server Features

**Resources** — Contextual data that Servers expose for the LLM to read.
Resources have URIs (like `file:///path/to/doc.md` or `db://users/schema`),
MIME types, and can be static or dynamic. Clients discover them with
`resources/list` and read them with `resources/read`.

**Prompts** — Reusable prompt templates that Servers define. These are
parameterized templates a user or host can invoke to produce structured
messages. Listed via `prompts/list`, retrieved via `prompts/get`.

**Tools** — Functions that the LLM can invoke through the Client. Each tool has
a name, description, and a JSON Schema for its parameters. Discovered via
`tools/list`, invoked via `tools/call`. Tool results contain content blocks
(text, images, embedded resources).

### Client Features

**Sampling** — Allows Servers to request LLM completions from the Client. This
enables "agentic" server behaviors where the server can ask the model to
generate text. The Client mediates this — it can modify or reject requests.

**Roots** — Filesystem boundaries the Client exposes so Servers know which
directories/files they are allowed to operate within. Declared as URI lists.

**Elicitation** — Allows Servers to request additional information from the
user through the Client. The Client presents the request as a UI prompt.

### Additional Utilities

- **Progress tracking**: Long operations report progress via `notifications/progress`
- **Cancellation**: Clients can cancel in-flight requests via `notifications/cancelled`
- **Logging**: Servers emit structured log messages via `notifications/message`
- **Ping/pong**: Both sides can verify connectivity with `ping` requests
- **Pagination**: List operations support cursor-based pagination

---

## 3. Transport Mechanisms

MCP defines two standard transport mechanisms. Implementations may support
additional transports, but these two are required by the specification.

### stdio Transport

The simplest transport. The Client spawns the Server as a **child process** and
communicates over standard I/O streams:

- **Client → Server**: Write JSON-RPC messages to the Server's **stdin**
- **Server → Client**: Write JSON-RPC messages to the Server's **stdout**
- **Logging**: Server writes human-readable diagnostics to **stderr**
- **Framing**: Messages are delimited by **newlines** (`\n`)

```
┌─────────────┐         stdin          ┌─────────────┐
│             │ ──── JSON-RPC ──────▶  │             │
│   Client    │                         │   Server    │
│   Process   │ ◀─── JSON-RPC ───────  │  (child)    │
│             │         stdout          │             │
└─────────────┘                         └──────┬──────┘
                                               │ stderr
                                               ▼
                                        (diagnostic logs)
```

**Advantages**: Zero configuration, no networking, inherits process lifecycle.
Server dies when Client kills the subprocess.

**Disadvantages**: Server must run on the same machine as the Client. Cannot
share one Server across multiple Clients.

**Example startup** (conceptual):

```bash
# Client spawns server as subprocess
$ my-mcp-server --stdio

# Server reads JSON-RPC from stdin, writes to stdout
# Each message is a single line of JSON followed by \n
{"jsonrpc":"2.0","id":1,"method":"initialize","params":{...}}
```

### Streamable HTTP Transport

For remote and shared Servers. The Server runs as an **independent HTTP
service** (not a subprocess). Introduced in protocol revision 2025-03-26 to
replace the earlier SSE-only transport.

**Message flow**:

- **Client → Server**: HTTP **POST** to the MCP endpoint with JSON-RPC body
- **Server → Client**: Can respond with:
  - A single JSON-RPC response (Content-Type: application/json)
  - An SSE stream (Content-Type: text/event-stream) for streaming responses
- **Server-initiated messages**: Server can open SSE streams on **GET**
  requests to push notifications to the Client

```
┌─────────────┐                          ┌─────────────┐
│             │  POST /mcp               │             │
│   Client    │  ── JSON-RPC body ─────▶ │   Server    │
│             │                          │  (HTTP)     │
│             │  ◀── JSON response ───── │             │
│             │    or SSE stream          │             │
│             │                          │             │
│             │  GET /mcp                │             │
│             │  ◀── SSE stream ──────── │             │
│             │  (server-initiated msgs) │             │
└─────────────┘                          └─────────────┘
```

### Session Management

Streamable HTTP uses the **`Mcp-Session-Id`** header for session tracking:

1. Client sends `initialize` POST (no session ID yet)
2. Server responds with `Mcp-Session-Id: <uuid>` header
3. Client includes this header in all subsequent requests
4. Server can reject requests with missing or invalid session IDs (HTTP 404)
5. Client can terminate a session with HTTP DELETE to the endpoint

```
Client                                  Server
  │                                       │
  │── POST /mcp (initialize) ───────────▶│
  │◀── 200 OK ───────────────────────────│
  │    Mcp-Session-Id: sess-abc-123       │
  │                                       │
  │── POST /mcp ─────────────────────────▶│
  │   Mcp-Session-Id: sess-abc-123        │
  │◀── 200 OK (SSE stream) ──────────────│
  │                                       │
  │── DELETE /mcp ───────────────────────▶│
  │   Mcp-Session-Id: sess-abc-123        │
  │◀── 200 OK ───────────────────────────│
  │                                       │
```

### Resumability

The SSE-based streaming supports reconnection via the standard
**`Last-Event-ID`** header:

1. Server includes `id:` fields in SSE events
2. If the connection drops, Client reconnects with `Last-Event-ID` header
3. Server replays missed events from that point forward

This is critical for reliability over unreliable networks.

### Security Considerations for HTTP Transport

- **DNS rebinding protection**: Servers binding to localhost must validate the
  `Origin` header and reject requests from unexpected origins
- **Authentication**: Servers should implement OAuth 2.1 or other auth when
  exposed beyond localhost
- **CORS**: Servers should set appropriate CORS headers for browser-based
  clients
- **TLS**: Required for any non-localhost deployment
- **Binding**: Production servers should bind to `127.0.0.1` (not `0.0.0.0`)
  unless explicitly configured for remote access

---

## 4. MCP vs Function Calling

Function calling (also called "tool use") is a feature built into LLM APIs
from providers like OpenAI, Anthropic, and Google. MCP and function calling
solve related but distinct problems.

### Comparison Table

| Aspect              | MCP                                | Function Calling                    |
|---------------------|------------------------------------|-------------------------------------|
| **What it is**      | Open protocol / standard           | LLM API feature                     |
| **Wire format**     | JSON-RPC 2.0                       | Provider-specific JSON              |
| **Transport**       | stdio, Streamable HTTP             | HTTP API (provider endpoint)        |
| **Discovery**       | Dynamic (`tools/list` at runtime)  | Static (defined per API request)    |
| **State**           | Stateful sessions                  | Stateless per request               |
| **Who executes**    | MCP Server (separate process)      | Your application code               |
| **Extensibility**   | Add any MCP server at runtime      | Requires code changes               |
| **Multi-provider**  | Works with any LLM                 | Tied to specific LLM provider       |
| **Resources**       | First-class (URIs, subscriptions)  | Not supported                       |
| **Prompts**         | First-class (templates)            | Not supported                       |
| **Sampling**        | Bidirectional (server→LLM)         | Unidirectional only                 |
| **Ecosystem**       | Growing registry of servers        | Per-application                     |

### How They Work Together

MCP does **not replace** function calling — it builds on top of it:

1. Your Host application connects to MCP Servers
2. It discovers available tools via `tools/list`
3. It translates MCP tool definitions into the LLM provider's function calling
   format
4. When the LLM invokes a function, the Host routes the call through the
   appropriate MCP Client to the Server
5. The Server executes the tool and returns results
6. The Host feeds results back to the LLM

```
User ─▶ Host ─▶ LLM API (with function definitions from MCP)
                    │
                    ▼ (LLM returns function_call)
         Host routes call to MCP Client
                    │
                    ▼
              MCP Server executes tool
                    │
                    ▼
         Host sends result back to LLM
```

### When to Use Which

**Use function calling alone** when:
- You have a small, fixed set of tools
- Tools are tightly coupled to your application logic
- You don't need cross-application reuse

**Use MCP** when:
- You want tools to be reusable across multiple AI applications
- You need dynamic tool discovery
- You want a standard ecosystem of pre-built integrations
- You need resources (context) and prompts alongside tools

---

## 5. MCP SDKs

The official MCP organization on GitHub (`modelcontextprotocol`) maintains
reference SDKs for multiple languages. The ecosystem also includes
community-maintained SDKs.

### TypeScript SDK

The most mature SDK. Packages:

- `@modelcontextprotocol/sdk` — Combined client and server library
- Works with Node.js, Bun, and Deno

```typescript
import { McpServer } from "@modelcontextprotocol/sdk/server/mcp.js";
import { StdioServerTransport } from "@modelcontextprotocol/sdk/server/stdio.js";
import { z } from "zod";

const server = new McpServer({
  name: "example-server",
  version: "1.0.0",
});

// Register a tool
server.tool(
  "search_files",
  "Search for files matching a glob pattern",
  {
    pattern: z.string().describe("Glob pattern to match"),
    directory: z.string().default(".").describe("Root directory"),
  },
  async ({ pattern, directory }) => {
    // Implementation here
    const results = await findFiles(pattern, directory);
    return {
      content: [{ type: "text", text: results.join("\n") }],
    };
  }
);

// Register a resource
server.resource(
  "config",
  "config://app",
  async (uri) => ({
    contents: [{
      uri: uri.href,
      mimeType: "application/json",
      text: JSON.stringify(loadConfig()),
    }],
  })
);

// Start with stdio transport
const transport = new StdioServerTransport();
await server.connect(transport);
```

### Python SDK

The Python SDK provides the **FastMCP** high-level API, inspired by FastAPI's
decorator pattern:

```python
from mcp.server import FastMCP

# Create server
app = FastMCP("my-tools")

@app.tool()
def search_files(pattern: str, directory: str = ".") -> str:
    """Search for files matching a glob pattern.

    Args:
        pattern: Glob pattern to match against filenames
        directory: Root directory to search from
    """
    import glob
    matches = glob.glob(pattern, root_dir=directory, recursive=True)
    return "\n".join(matches) if matches else "No files found"

@app.resource("config://app")
def get_config() -> str:
    """Return application configuration."""
    import json
    return json.dumps({"version": "1.0", "debug": False})

@app.prompt()
def review_prompt(code: str) -> str:
    """Generate a code review prompt."""
    return f"Please review the following code:\n\n```\n{code}\n```"

if __name__ == "__main__":
    app.run(transport="stdio")
```

Installation:

```bash
pip install "mcp[cli]"

# Run a server
mcp run my_server.py

# Development mode with MCP Inspector
mcp dev my_server.py
```

### Other SDKs

| Language   | Package / Repository                          | Status     |
|------------|-----------------------------------------------|------------|
| Rust       | `modelcontextprotocol/rust-sdk`               | Stable     |
| Go         | `modelcontextprotocol/go-sdk`                 | Stable     |
| Java       | `modelcontextprotocol/java-sdk`               | Stable     |
| Kotlin     | `modelcontextprotocol/kotlin-sdk`             | Stable     |
| C#         | `modelcontextprotocol/csharp-sdk`             | Stable     |
| Swift      | `modelcontextprotocol/swift-sdk`              | Stable     |
| Ruby       | `modelcontextprotocol/ruby-sdk`               | Beta       |
| PHP        | `modelcontextprotocol/php-sdk`                | Community  |

All SDKs aim for parity on core protocol features. Language-idiomatic APIs
vary — for example, Go uses interfaces, Rust uses traits and async, Java uses
Spring-style annotations.

---

## 6. MCP Server Ecosystem

### Official Reference Servers

Anthropic and the MCP community maintain a set of reference servers:

| Server       | Description                              | Transport |
|--------------|------------------------------------------|-----------|
| `filesystem` | Read/write/search local files            | stdio     |
| `git`        | Git repository operations                | stdio     |
| `fetch`      | HTTP fetching with robots.txt respect    | stdio     |
| `memory`     | Knowledge graph-based persistent memory  | stdio     |
| `postgres`   | PostgreSQL database queries              | stdio     |
| `sqlite`     | SQLite database operations               | stdio     |
| `slack`      | Slack workspace integration              | stdio     |
| `github`     | GitHub API operations                    | stdio     |
| `sentry`     | Sentry error tracking                    | stdio     |
| `puppeteer`  | Browser automation                       | stdio     |

### MCP Server Registry

The official registry at **https://registry.modelcontextprotocol.io** provides
a searchable catalog of MCP servers. Servers can be published and discovered
through this registry, similar to npm for Node packages.

### Community Servers

The community has built hundreds of MCP servers covering:

- Cloud providers (AWS, GCP, Azure)
- Databases (MongoDB, Redis, Elasticsearch)
- Communication (Email, Discord, Teams)
- Developer tools (Docker, Kubernetes, Terraform)
- Knowledge bases (Notion, Confluence, Obsidian)
- APIs (Stripe, Twilio, SendGrid)

---

## 7. How Agents Integrate MCP

Different AI coding agents have taken varied approaches to MCP integration.
Understanding these patterns reveals both the flexibility of the protocol and
the design decisions teams face.

### Goose (Block / Anthropic)

Goose is **MCP-native from the ground up**. Every extension in Goose is an MCP
server — there is no separate "plugin" or "extension" API. The `ExtensionManager`
supports seven transport types:

```
Transport types in Goose:
1. Platform extensions (in-process, via DuplexStream)
2. Builtin servers (DuplexStream)
3. Stdio (child process)
4. StreamableHTTP (remote server)
5. SSE (legacy remote transport)
6. Custom (user-defined)
7. Bundled WASM modules
```

Goose's architecture means that even core functionality (developer tools,
file operations) runs as MCP servers. This dogfooding ensures the MCP
integration is robust and battle-tested.

```rust
// Goose ExtensionConfig (simplified)
pub enum TransportType {
    Stdio { cmd: String, args: Vec<String>, env: HashMap<String, String> },
    StreamableHttp { url: String },
    Sse { url: String },
    Builtin { name: String },
}

pub struct ExtensionConfig {
    pub name: String,
    pub transport: TransportType,
    pub enabled: bool,
    pub timeout: Duration,
}
```

### Claude Code

Claude Code integrates MCP in two directions:

1. **As an MCP client** — Connects to external MCP servers configured in
   project or user settings. Tools from MCP servers appear alongside built-in
   tools like Read, Write, and Bash.

2. **As an MCP server** — Claude Code can itself be exposed as an MCP server,
   allowing other applications to use Claude Code's capabilities
   programmatically.

Configuration in `.claude/settings.json`:

```json
{
  "mcpServers": {
    "github": {
      "command": "npx",
      "args": ["-y", "@modelcontextprotocol/server-github"],
      "env": { "GITHUB_TOKEN": "..." }
    },
    "database": {
      "type": "streamable-http",
      "url": "http://localhost:8080/mcp"
    }
  }
}
```

### Gemini CLI

Google's Gemini CLI supports MCP as an extension mechanism alongside its
built-in tools. MCP servers are configured in a settings file and their tools
appear in the model's available functions.

### Codex (OpenAI)

OpenAI's Codex agent supports MCP both as a **client** and as a **server**:

- **Client**: Connects to MCP servers, discovers tools via `tools/list`, and
  converts them to OpenAI-format `FunctionCall` objects that the model can
  invoke.
- **Server**: Exposes Codex's own capabilities (code execution, file editing)
  as MCP tools for other applications to consume.

MCP tools are first-class citizens — they are indistinguishable from built-in
tools from the model's perspective.

### OpenCode

OpenCode treats MCP as a pure extension mechanism. MCP server tools are
namespaced with a `{server}_{tool}` prefix to avoid name collisions. All MCP
tool invocations require explicit user permission — there is no auto-approve
mode. This is a deliberate security choice.

```
Tool naming in OpenCode:
  github_create_issue      (server: github, tool: create_issue)
  filesystem_read_file     (server: filesystem, tool: read_file)
  memory_store_fact        (server: memory, tool: store_fact)
```

### OpenHands

OpenHands integrates MCP through its **microagent** system. MCP servers are
wrapped as microagents, allowing them to participate in OpenHands' multi-agent
orchestration framework. This is a higher-level integration than direct tool
exposure.

### Ante

Ante uses a custom Rust MCP SDK with **bidirectional support** — Ante servers
can both expose tools and consume tools from other MCP servers. This enables
MCP server composition, where one server delegates to another.

---

## 8. Building a Custom MCP Server

### Python Example: A Complete Weather Server

```python
"""MCP server that provides weather information."""
from mcp.server import FastMCP
import httpx

app = FastMCP("weather")

BASE_URL = "https://api.open-meteo.com/v1/forecast"

@app.tool()
async def get_weather(
    latitude: float,
    longitude: float,
    units: str = "celsius"
) -> str:
    """Get current weather for a location.

    Args:
        latitude: Latitude of the location (-90 to 90)
        longitude: Longitude of the location (-180 to 180)
        units: Temperature units - 'celsius' or 'fahrenheit'
    """
    temp_unit = "celsius" if units == "celsius" else "fahrenheit"
    params = {
        "latitude": latitude,
        "longitude": longitude,
        "current_weather": True,
        "temperature_unit": temp_unit,
    }
    async with httpx.AsyncClient() as client:
        resp = await client.get(BASE_URL, params=params)
        data = resp.json()

    weather = data["current_weather"]
    return (
        f"Temperature: {weather['temperature']}°"
        f"{'C' if units == 'celsius' else 'F'}\n"
        f"Wind speed: {weather['windspeed']} km/h\n"
        f"Wind direction: {weather['winddirection']}°"
    )

@app.resource("weather://locations")
def list_saved_locations() -> str:
    """Return a list of saved favorite locations."""
    import json
    locations = [
        {"name": "San Francisco", "lat": 37.7749, "lon": -122.4194},
        {"name": "New York", "lat": 40.7128, "lon": -74.0060},
        {"name": "London", "lat": 51.5074, "lon": -0.1278},
    ]
    return json.dumps(locations, indent=2)

if __name__ == "__main__":
    app.run(transport="stdio")
```

### TypeScript Example: A Database Query Server

```typescript
import { McpServer } from "@modelcontextprotocol/sdk/server/mcp.js";
import { StdioServerTransport } from "@modelcontextprotocol/sdk/server/stdio.js";
import { z } from "zod";
import Database from "better-sqlite3";

const server = new McpServer({
  name: "sqlite-query",
  version: "1.0.0",
});

const db = new Database("./data.db", { readonly: true });

server.tool(
  "query",
  "Execute a read-only SQL query",
  {
    sql: z.string().describe("SQL SELECT query to execute"),
    limit: z.number().default(100).describe("Max rows to return"),
  },
  async ({ sql, limit }) => {
    // Security: only allow SELECT statements
    if (!sql.trim().toUpperCase().startsWith("SELECT")) {
      return {
        content: [{ type: "text", text: "Error: Only SELECT queries allowed" }],
        isError: true,
      };
    }

    const limitedSql = `${sql} LIMIT ${limit}`;
    const rows = db.prepare(limitedSql).all();
    return {
      content: [{
        type: "text",
        text: JSON.stringify(rows, null, 2),
      }],
    };
  }
);

server.tool(
  "list_tables",
  "List all tables in the database",
  {},
  async () => {
    const tables = db.prepare(
      "SELECT name FROM sqlite_master WHERE type='table'"
    ).all();
    return {
      content: [{
        type: "text",
        text: tables.map((t: any) => t.name).join("\n"),
      }],
    };
  }
);

const transport = new StdioServerTransport();
await server.connect(transport);
```

### Testing with MCP Inspector

The MCP Inspector is a browser-based tool for testing and debugging MCP
servers interactively:

```bash
# Install and run the inspector
npx @modelcontextprotocol/inspector

# It opens a web UI where you can:
# - Connect to any MCP server (stdio or HTTP)
# - Browse available tools, resources, and prompts
# - Invoke tools with custom arguments
# - View raw JSON-RPC messages
# - Test error handling
```

### Deployment Considerations

**stdio servers**: Distributed as executables or scripts. Users configure their
Host to spawn the server. No networking required. Best for local tools.

**HTTP servers**: Deployed as web services. Can be shared across users and
applications. Require authentication, TLS, and proper security hardening.

**Configuration distribution**: Most hosts support a JSON configuration file:

```json
{
  "mcpServers": {
    "my-server": {
      "command": "python",
      "args": ["-m", "my_mcp_server"],
      "env": {
        "API_KEY": "${MY_API_KEY}"
      }
    }
  }
}
```

---

## 9. MCP Security Considerations

Security is a first-class concern in the MCP specification. The protocol
involves executing code, accessing data, and bridging trust boundaries.

### Tool Descriptions Are Untrusted

Tool names and descriptions come from the MCP Server. A malicious server could
craft descriptions designed to manipulate the LLM into harmful behavior (prompt
injection via tool descriptions). Hosts should:

- Display tool descriptions to users before first use
- Allow users to review and approve tool invocations
- Sanitize or validate descriptions before passing them to the LLM

### User Consent Requirements

The specification mandates that Hosts must obtain user consent before:

- Connecting to a new MCP server
- Sending data to an MCP server
- Executing tools that have side effects
- Allowing sampling (server-initiated LLM calls)

Different hosts implement consent differently — from per-tool approval dialogs
to allowlists to "yolo mode" auto-approve (not recommended for production).

### Server Validation

Hosts should validate MCP servers through:

- Code review of server source code
- Signature verification of server binaries
- Registry trust scores and community reviews
- Sandboxing server processes (minimal filesystem/network access)

### DNS Rebinding Attacks

HTTP-based MCP servers binding to localhost are vulnerable to DNS rebinding:

1. Attacker controls `evil.com` which initially resolves to their IP
2. User visits `evil.com` in a browser
3. Attacker changes DNS to resolve `evil.com` → `127.0.0.1`
4. Browser-based JavaScript can now make requests to the local MCP server

**Mitigation**: Always validate the `Origin` and `Host` headers. Reject
requests where the Origin doesn't match expected values.

### Transport Security

- stdio: Inherently local, but server process should run with minimal
  privileges
- HTTP: Must use TLS for any non-localhost deployment. Implement proper
  authentication (OAuth 2.1 recommended).

---

## 10. Future of MCP

### Standardization

MCP is on a path toward formal standardization under the Linux Foundation. This
mirrors the trajectory of other successful protocols like HTTP, WebSocket, and
LSP. The goal is a stable, versioned specification that multiple organizations
can independently implement.

### OAuth 2.1 Integration

The protocol specification includes provisions for **OAuth 2.1** as the
standard authentication mechanism for HTTP-based MCP servers. This enables:

- Token-based authentication
- Scoped permissions per server
- Standard token refresh flows
- Integration with enterprise identity providers

### Broader Ecosystem Adoption

As of mid-2025, MCP support is shipping or announced in:

- **Anthropic**: Claude Desktop, Claude Code (native)
- **OpenAI**: ChatGPT agents, Codex
- **Google**: Gemini CLI
- **Microsoft**: VS Code, GitHub Copilot
- **Cursor**: Built-in MCP client
- **Windsurf**: MCP integration
- **JetBrains**: IDE plugin support
- **Zed**: Editor-level MCP support
- **Amazon**: Q Developer MCP support

### Composable Workflows

A key area of development is **server composition** — where MCP servers can
themselves be MCP clients, connecting to other servers. This enables:

- **Pipelines**: Data flows through a chain of specialized servers
- **Orchestration**: A coordinator server delegates to specialist servers
- **Federation**: Multiple servers present a unified interface

```
                    ┌──────────────────┐
                    │  Orchestrator    │
                    │  MCP Server      │
                    └─────┬──────┬─────┘
                          │      │
               ┌──────────┘      └──────────┐
               ▼                             ▼
     ┌─────────────────┐          ┌─────────────────┐
     │  Analysis Server │          │  Storage Server  │
     │  (MCP Server)    │          │  (MCP Server)    │
     └─────────────────┘          └─────────────────┘
```

### Protocol Evolution

The specification uses dated revisions (e.g., `2025-03-26`) rather than semver.
Future revisions are expected to add:

- Richer media types in tool results (audio, video)
- Batch tool invocation for efficiency
- Server-to-server authentication standards
- Formal capability versioning
- Improved error taxonomy

---

## References

- [MCP Specification](https://spec.modelcontextprotocol.io/)
- [MCP GitHub Organization](https://github.com/modelcontextprotocol)
- [MCP Server Registry](https://registry.modelcontextprotocol.io)
- [TypeScript SDK](https://github.com/modelcontextprotocol/typescript-sdk)
- [Python SDK](https://github.com/modelcontextprotocol/python-sdk)
- [MCP Inspector](https://github.com/modelcontextprotocol/inspector)
- [Anthropic MCP Announcement](https://www.anthropic.com/news/model-context-protocol)
