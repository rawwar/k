---
title: "Ante Tool System"
status: complete
---

# Ante Tool System

> Ante by Antigma Labs is a Rust-built terminal coding agent that ranked #4 on
> Terminal-Bench TB1 and #17 on TB2 — benchmarks requiring sophisticated tool use
> for complex tasks like chess engines, cryptanalysis, protein assembly, and
> CoreWars. While Ante itself is closed-source, its tool system can be understood
> through its public MCP SDK, website documentation, and observed behavior in
> Terminal-Bench evaluations and blog posts.

## Overview

Ante's tool system is built on two pillars:

1. **Native Rust tools** — compiled into the agent binary, providing zero-overhead
   access to file system operations, shell execution, code analysis, and search.
2. **MCP (Model Context Protocol) integration** — via Antigma's own open-source
   `mcp-sdk` crate, enabling standardized tool exposure and consumption across
   agent boundaries.

The design philosophy, stated explicitly in the MCP SDK repository, is to
"use primitive building blocks and avoid framework if possible." This minimalism
carries through the entire tool system: tools are simple Rust traits, transport
is stdio-based, and there is no heavyweight runtime or dependency injection.

## MCP (Model Context Protocol) Integration

Antigma Labs built and open-sourced their own Rust MCP implementation at
`AntigmaLabs/mcp-sdk` on GitHub. Rather than adopting an existing SDK, they wrote
a minimal, purpose-built implementation that reflects the same engineering values
as Ante itself: small, fast, and composable.

### Why Build a Custom MCP SDK?

The official MCP specification provides SDKs in TypeScript and Python. For a
Rust-native agent like Ante, using these would mean either:

- Running a sidecar process in another language (added latency, complexity)
- Using FFI bindings (fragile, unsafe)
- Porting the SDK to Rust

Antigma chose to write a clean Rust implementation from scratch. The result is a
crate with roughly six core source files and no heavy dependencies beyond
`serde_json` and `tokio`.

### SDK Architecture

The `mcp-sdk` source tree is deliberately minimal:

```
mcp-sdk/src/
├── lib.rs          — Module exports and re-exports
├── client.rs       — MCP client implementation
├── server.rs       — MCP server with builder pattern
├── protocol.rs     — JSON-RPC 2.0 protocol handling
├── tools.rs        — Tool trait definition and registration
├── types.rs        — MCP type definitions (capabilities, resources, etc.)
└── transport/      — Transport layer
    └── stdio.rs    — Standard I/O transport (stdin/stdout)
```

Each file has a single, clear responsibility. There are no deep abstraction
hierarchies, no plugin registries, no lifecycle hooks — just the primitives
needed to implement MCP.

### The Tool Trait

At the heart of the tool system is the `Tool` trait. Every tool — whether built
into Ante or exposed via MCP to external consumers — implements this interface:

```rust
use async_trait::async_trait;
use serde_json::Value;

#[async_trait]
pub trait Tool: Send + Sync {
    /// The unique name of this tool (e.g., "read_file", "shell_exec")
    fn name(&self) -> &str;

    /// A human-readable description of what the tool does,
    /// used by the LLM to decide when to invoke it
    fn description(&self) -> &str;

    /// JSON Schema describing the tool's input parameters,
    /// enabling the LLM to generate valid invocations
    fn input_schema(&self) -> Value;

    /// Execute the tool with the given arguments and return the result
    async fn call(&self, args: Value) -> Result<Value, ToolError>;
}
```

This is a deliberately narrow interface. Four methods, no configuration objects,
no middleware chains. A tool knows its name, can describe itself, defines its
input contract, and can be called. Everything else is left to the implementor.

### Server Builder Pattern

The MCP server uses a builder pattern for construction, allowing incremental
configuration before the server starts accepting requests:

```rust
use mcp_sdk::server::Server;
use mcp_sdk::transport::StdioTransport;

let server = Server::builder(StdioTransport)
    .capabilities(ServerCapabilities {
        tools: Some(ToolsCapability { list_changed: Some(true) }),
        ..Default::default()
    })
    .request_handler(move |request| {
        // Route incoming JSON-RPC requests to appropriate handlers
        handle_request(request, &tool_registry)
    })
    .build();

server.run().await?;
```

The builder accepts a transport (currently stdio), optional capability
declarations, and a request handler. The server then runs an event loop,
reading JSON-RPC messages from stdin and writing responses to stdout.

### Client Implementation

The MCP client mirrors the server's simplicity. It connects to an MCP server
process via stdio, discovers available tools, and can invoke them:

```rust
use mcp_sdk::client::Client;
use mcp_sdk::transport::StdioTransport;

let client = Client::new(StdioTransport::connect("ante-tool-server")?);

// Discover available tools
let tools = client.list_tools().await?;

// Invoke a specific tool
let result = client.call_tool("read_file", json!({
    "path": "/src/main.rs"
})).await?;
```

This means Ante can both **expose** its tools to other MCP-compatible systems
and **consume** tools from external MCP servers — a bidirectional integration
point.

### Protocol Layer

The protocol layer handles JSON-RPC 2.0 message framing, serialization, and
deserialization. Messages flow over stdio as newline-delimited JSON:

```rust
// Outgoing request
{
    "jsonrpc": "2.0",
    "id": 1,
    "method": "tools/call",
    "params": {
        "name": "shell_exec",
        "arguments": { "command": "grep -r 'pattern' src/" }
    }
}

// Incoming response
{
    "jsonrpc": "2.0",
    "id": 1,
    "result": {
        "content": [{ "type": "text", "text": "src/main.rs:42: pattern found" }]
    }
}
```

## Inferred Core Tools

While Ante's exact tool set is not publicly documented, its Terminal-Bench
performance and the investigative blog post reveal the tools it must have.

### File System Tools

Ante was observed performing extensive file system operations during
Terminal-Bench tasks and the npm package investigation:

- **Reading files**: Analyzing `package.json`, reading JavaScript bundles (31MB,
  616K lines), examining source maps
- **Writing files**: Generating solutions for chess engines, cryptanalysis
  programs, CoreWars warriors
- **Directory traversal**: Navigating package structures, identifying relevant
  files in large codebases
- **File search**: Locating specific files by name or pattern across directory
  trees

A plausible implementation:

```rust
pub struct ReadFile;

#[async_trait]
impl Tool for ReadFile {
    fn name(&self) -> &str { "read_file" }

    fn description(&self) -> &str {
        "Read the contents of a file at the given path"
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "Absolute or relative path to the file"
                }
            },
            "required": ["path"]
        })
    }

    async fn call(&self, args: Value) -> Result<Value, ToolError> {
        let path = args["path"].as_str()
            .ok_or(ToolError::InvalidArgs("path must be a string"))?;
        let contents = tokio::fs::read_to_string(path).await
            .map_err(|e| ToolError::Execution(e.to_string()))?;
        Ok(json!({ "content": contents }))
    }
}
```

### Shell / Command Execution

The Terminal-Bench blog post shows Ante running shell commands directly:

- `strings` — extracting printable strings from binaries
- `grep` — searching for patterns in files
- Binary analysis tools — examining compiled artifacts
- Build systems — compiling Rust, C, and other languages for benchmark tasks

Shell execution is the most powerful and dangerous tool in any coding agent.
Ante likely wraps it with timeout controls and output capture:

```rust
pub struct ShellExec;

#[async_trait]
impl Tool for ShellExec {
    fn name(&self) -> &str { "shell_exec" }

    fn description(&self) -> &str {
        "Execute a shell command and return its output"
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "command": {
                    "type": "string",
                    "description": "The shell command to execute"
                },
                "timeout_secs": {
                    "type": "integer",
                    "description": "Maximum execution time in seconds",
                    "default": 30
                }
            },
            "required": ["command"]
        })
    }

    async fn call(&self, args: Value) -> Result<Value, ToolError> {
        let command = args["command"].as_str()
            .ok_or(ToolError::InvalidArgs("command required"))?;
        let timeout = args["timeout_secs"].as_u64().unwrap_or(30);

        let output = tokio::time::timeout(
            Duration::from_secs(timeout),
            tokio::process::Command::new("sh")
                .arg("-c")
                .arg(command)
                .output()
        ).await
            .map_err(|_| ToolError::Timeout)?
            .map_err(|e| ToolError::Execution(e.to_string()))?;

        Ok(json!({
            "stdout": String::from_utf8_lossy(&output.stdout),
            "stderr": String::from_utf8_lossy(&output.stderr),
            "exit_code": output.status.code()
        }))
    }
}
```

### Code Analysis

Ante demonstrated sophisticated code analysis capabilities:

- Parsing 31MB JavaScript bundles and identifying specific patterns
- Analyzing source maps to trace obfuscated code back to original sources
- Identifying XOR-encrypted strings and decoding them
- Understanding code structure across multiple languages

This likely involves a combination of shell tools (`grep`, `awk`, language-specific
parsers) and potentially built-in Rust analysis for common languages.

### Search / Grep

Pattern matching across large codebases is essential for Terminal-Bench tasks.
Ante was shown finding specific code patterns in obfuscated bundles — hundreds
of thousands of lines — quickly and accurately.

A Rust-native grep tool using the `grep` crate (from the ripgrep ecosystem)
would provide high-performance regex search without spawning external processes:

```rust
pub struct GrepSearch;

#[async_trait]
impl Tool for GrepSearch {
    fn name(&self) -> &str { "grep" }

    fn description(&self) -> &str {
        "Search for a regex pattern across files in a directory"
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "pattern": { "type": "string" },
                "path": { "type": "string", "default": "." },
                "include": {
                    "type": "string",
                    "description": "Glob pattern to filter files (e.g., '*.rs')"
                }
            },
            "required": ["pattern"]
        })
    }

    async fn call(&self, args: Value) -> Result<Value, ToolError> {
        // Rust-native regex search using the grep crate
        // Returns matching lines with file paths and line numbers
        // ...
    }
}
```

## Tool Implementation in Rust

The choice of Rust for tool implementation has specific technical consequences
that affect how tools behave at runtime.

### Compile-Time Type Safety

Tool input schemas are defined using `serde_json::Value`, but the actual
argument parsing within `call()` can use strongly-typed Rust structs with
`serde::Deserialize`. This means schema violations are caught at the point of
deserialization rather than causing runtime panics deeper in the call stack:

```rust
#[derive(Deserialize)]
struct ReadFileArgs {
    path: String,
    #[serde(default)]
    encoding: Option<String>,
}

// Inside call():
let args: ReadFileArgs = serde_json::from_value(args)
    .map_err(|e| ToolError::InvalidArgs(e.to_string()))?;
```

### Concurrent Execution

Rust's ownership model and `Send + Sync` trait bounds on `Tool` guarantee that
tools can be called concurrently from multiple sub-agents without data races.
There is no global interpreter lock, no shared mutable state by default, and
no need for runtime lock checking:

```rust
// The Tool trait requires Send + Sync
pub trait Tool: Send + Sync { ... }

// This means tools can be shared across threads safely
let tool: Arc<dyn Tool> = Arc::new(ReadFile);

// Multiple sub-agents can call the same tool concurrently
let handles: Vec<_> = tasks.iter().map(|task| {
    let tool = Arc::clone(&tool);
    tokio::spawn(async move {
        tool.call(task.args.clone()).await
    })
}).collect();
```

### Zero-Cost Abstractions

The `Tool` trait uses dynamic dispatch (`dyn Tool`) for flexibility, but the
actual tool logic compiles down to native code with no runtime overhead beyond
the vtable lookup. For tools that are called frequently (like file reads or
grep), this means tool dispatch overhead is measured in nanoseconds — negligible
compared to the I/O operations the tools perform.

## Offline Tool Execution

Ante supports offline operation, and its tool system is designed to work without
network connectivity. The core tools — file system, shell execution, code
analysis, and search — are inherently local operations.

In offline mode:

- **File system tools** work identically — they operate on the local filesystem
- **Shell execution** works identically — spawning local processes
- **Search tools** work identically — scanning local files
- **Code analysis** works identically — parsing local source code

The only component that differs in offline mode is the LLM inference backing
the agent's reasoning. Offline mode uses a local model rather than a cloud API.
The tool system itself is completely unaffected.

This design is intentional: by keeping tools as pure local operations and
separating them from the inference layer, Ante achieves a clean offline/online
boundary. The same tool implementations serve both modes without conditional
logic or feature flags.

## Sub-Agent Tool Access

In Ante's multi-agent architecture, sub-agents are spawned to handle specific
subtasks. Each sub-agent needs access to tools to do its work. There are two
plausible models for how this works:

### Shared Tool Registry

All sub-agents share the same tool registry, meaning every sub-agent can access
every tool. This is the simpler model and aligns with the `Arc<dyn Tool>`
pattern enabled by Rust's concurrency model:

```rust
struct AgentRuntime {
    tools: Vec<Arc<dyn Tool>>,
    // ...
}

impl AgentRuntime {
    fn spawn_sub_agent(&self, task: Task) -> SubAgent {
        SubAgent {
            tools: self.tools.clone(),  // Arc clone — cheap
            task,
        }
    }
}
```

### Scoped Tool Access

Alternatively, the meta-agent might restrict which tools a sub-agent can access
based on the subtask. A sub-agent writing code might get file system and search
tools but not shell execution, while a sub-agent running tests gets shell
execution but not file writes:

```rust
fn tools_for_role(role: SubAgentRole, all_tools: &[Arc<dyn Tool>]) -> Vec<Arc<dyn Tool>> {
    match role {
        SubAgentRole::Coder => all_tools.iter()
            .filter(|t| matches!(t.name(), "read_file" | "write_file" | "grep"))
            .cloned()
            .collect(),
        SubAgentRole::Tester => all_tools.iter()
            .filter(|t| matches!(t.name(), "read_file" | "shell_exec" | "grep"))
            .cloned()
            .collect(),
        SubAgentRole::Reviewer => all_tools.iter()
            .filter(|t| matches!(t.name(), "read_file" | "grep"))
            .cloned()
            .collect(),
    }
}
```

Given Ante's minimalist philosophy, the shared model is more likely — it avoids
the complexity of role-based access control while relying on the LLM's judgment
to use appropriate tools for each subtask.

## MCP as an Extensibility Layer

The MCP integration serves a dual purpose:

1. **Tool consumption**: Ante can connect to external MCP servers to gain
   additional capabilities — database access, API integrations, specialized
   analysis tools — without modifying the agent binary.

2. **Tool exposure**: Ante can expose its own tools as an MCP server, allowing
   other agents or systems to use Ante's file system, search, and analysis
   capabilities programmatically.

This bidirectional MCP support means Ante is not a closed system. It can
participate in larger agent ecosystems where multiple tools and agents
communicate via the standardized MCP protocol, all while keeping its core
tool implementations lean, fast, and Rust-native.

## Summary

| Aspect | Implementation |
|---|---|
| Language | Rust (compiled, zero-cost abstractions) |
| Tool interface | `Tool` trait with 4 methods |
| MCP SDK | Custom `AntigmaLabs/mcp-sdk` (stdio transport) |
| Transport | JSON-RPC 2.0 over stdin/stdout |
| Concurrency | `Send + Sync` traits, `Arc<dyn Tool>`, lock-free |
| Core tools | File I/O, shell exec, code analysis, grep |
| Offline support | All tools work locally; only LLM inference differs |
| Extensibility | Bidirectional MCP (consume and expose tools) |