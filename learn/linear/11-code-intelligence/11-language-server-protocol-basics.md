---
title: Language Server Protocol Basics
description: The Language Server Protocol architecture — JSON-RPC transport, capability negotiation, and core request types that provide IDE-level code intelligence.
---

# Language Server Protocol Basics

> **What you'll learn:**
> - The LSP client-server architecture: JSON-RPC over stdio/TCP, initialization handshake, and capability negotiation
> - Core LSP requests — textDocument/definition, textDocument/references, textDocument/completion — and their request/response schemas
> - When to use LSP versus tree-sitter for code intelligence and how they complement each other in an agent's tool stack

Tree-sitter gives you syntactic structure. The grep/glob toolchain gives you text-level search. But some questions require deeper understanding: "What is the fully resolved type of this variable?" "Which trait implementation does this method call dispatch to?" "What completions are valid at this cursor position?" These questions require a language-specific analyzer that understands types, generics, trait resolution, macro expansion, and the full module graph.

The Language Server Protocol (LSP) provides a standard interface to these analyzers. Developed by Microsoft for VS Code and now adopted universally, LSP defines a JSON-RPC protocol that any editor or tool can use to communicate with language-specific servers like `rust-analyzer`, `pyright`, `typescript-language-server`, or `gopls`. For a coding agent, LSP is the gateway to full semantic code intelligence.

## The LSP Architecture

LSP uses a client-server model:

- The **server** is a language-specific process (e.g., `rust-analyzer`) that understands one or more programming languages deeply. It loads the project, builds an internal semantic model, and answers queries about the code.

- The **client** is the editor, IDE, or agent that sends requests and processes responses. The client does not need to understand the language — it just speaks the LSP protocol.

Communication uses **JSON-RPC 2.0** over **stdin/stdout** (most common) or TCP sockets. Each message is a JSON object with a `Content-Length` header:

```
Content-Length: 85\r\n
\r\n
{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"capabilities":{},"rootUri":"file:///project"}}
```

The protocol defines three kinds of messages:

1. **Requests** — the client asks for something and expects a response. Example: "What is the definition of the symbol at line 10, column 15?"
2. **Responses** — the server answers a request. Example: "The definition is in file `src/config.rs` at line 42."
3. **Notifications** — one-way messages that do not expect a response. Example: "The file `src/main.rs` was saved."

## The Initialization Handshake

Before sending any requests, the client and server negotiate capabilities through an initialization handshake:

```rust
use serde::{Deserialize, Serialize};
use serde_json::json;

#[derive(Serialize)]
struct LspRequest {
    jsonrpc: String,
    id: u64,
    method: String,
    params: serde_json::Value,
}

fn build_initialize_request(project_root: &str) -> LspRequest {
    LspRequest {
        jsonrpc: "2.0".into(),
        id: 1,
        method: "initialize".into(),
        params: json!({
            "processId": std::process::id(),
            "rootUri": format!("file://{}", project_root),
            "capabilities": {
                "textDocument": {
                    "definition": {
                        "dynamicRegistration": false
                    },
                    "references": {
                        "dynamicRegistration": false
                    },
                    "completion": {
                        "completionItem": {
                            "snippetSupport": false
                        }
                    },
                    "hover": {
                        "contentFormat": ["plaintext"]
                    }
                }
            },
            "workspaceFolders": [{
                "uri": format!("file://{}", project_root),
                "name": "project"
            }]
        }),
    }
}

fn build_initialized_notification() -> serde_json::Value {
    json!({
        "jsonrpc": "2.0",
        "method": "initialized",
        "params": {}
    })
}

fn main() {
    let init = build_initialize_request("/home/user/my-project");
    let json = serde_json::to_string_pretty(&init).unwrap();
    println!("Initialize request:\n{}", json);
}
```

The server responds with its own capabilities — which features it supports. A typical `rust-analyzer` response includes capabilities for definition, references, completion, hover, rename, code actions, and more. The agent should check these capabilities before making requests that the server might not support.

::: python Coming from Python
Python developers may be familiar with Jedi or Pylance for code intelligence. LSP wraps these analyzers in a standard protocol. Pyright (Microsoft's Python type checker) is also an LSP server — it is the same engine whether you use it from VS Code, Neovim, or a coding agent. The protocol is the same; only the client changes. If you have used `python-lsp-server` or `pyright`, you have already used LSP.
:::

## Core LSP Requests

The most useful LSP requests for a coding agent are:

### Go to Definition

```rust
use serde_json::json;

fn go_to_definition_request(id: u64, file_uri: &str, line: u32, character: u32) -> serde_json::Value {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "method": "textDocument/definition",
        "params": {
            "textDocument": {
                "uri": file_uri
            },
            "position": {
                "line": line,        // 0-indexed
                "character": character // 0-indexed
            }
        }
    })
}

// Response format:
// {
//   "jsonrpc": "2.0",
//   "id": 2,
//   "result": {
//     "uri": "file:///project/src/config.rs",
//     "range": {
//       "start": { "line": 41, "character": 0 },
//       "end": { "line": 41, "character": 25 }
//     }
//   }
// }

fn main() {
    let req = go_to_definition_request(
        2,
        "file:///project/src/main.rs",
        10,  // line (0-indexed)
        15,  // character (0-indexed)
    );
    println!("{}", serde_json::to_string_pretty(&req).unwrap());
}
```

### Find References

```rust
use serde_json::json;

fn find_references_request(
    id: u64,
    file_uri: &str,
    line: u32,
    character: u32,
    include_declaration: bool,
) -> serde_json::Value {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "method": "textDocument/references",
        "params": {
            "textDocument": {
                "uri": file_uri
            },
            "position": {
                "line": line,
                "character": character
            },
            "context": {
                "includeDeclaration": include_declaration
            }
        }
    })
}

// Response is an array of locations:
// {
//   "jsonrpc": "2.0",
//   "id": 3,
//   "result": [
//     { "uri": "file:///project/src/main.rs", "range": { "start": { "line": 10, "character": 4 }, "end": { "line": 10, "character": 20 } } },
//     { "uri": "file:///project/src/server.rs", "range": { "start": { "line": 55, "character": 8 }, "end": { "line": 55, "character": 24 } } }
//   ]
// }

fn main() {
    let req = find_references_request(
        3,
        "file:///project/src/config.rs",
        15,
        10,
        true, // Include the declaration itself in results
    );
    println!("{}", serde_json::to_string_pretty(&req).unwrap());
}
```

### Hover Information

```rust
use serde_json::json;

fn hover_request(id: u64, file_uri: &str, line: u32, character: u32) -> serde_json::Value {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "method": "textDocument/hover",
        "params": {
            "textDocument": {
                "uri": file_uri
            },
            "position": {
                "line": line,
                "character": character
            }
        }
    })
}

// Response includes type information and documentation:
// {
//   "jsonrpc": "2.0",
//   "id": 4,
//   "result": {
//     "contents": {
//       "kind": "markdown",
//       "value": "```rust\nfn connect(addr: &str) -> Result<TcpStream, io::Error>\n```\n\nEstablishes a TCP connection to the given address."
//     },
//     "range": {
//       "start": { "line": 10, "character": 4 },
//       "end": { "line": 10, "character": 11 }
//     }
//   }
// }

fn main() {
    let req = hover_request(4, "file:///project/src/main.rs", 10, 4);
    println!("{}", serde_json::to_string_pretty(&req).unwrap());
}
```

Hover is particularly valuable for agents because it returns the fully resolved type of a symbol, including inferred types that do not appear in the source code. This fills the gap that tree-sitter cannot: when a variable has no type annotation, hover tells you its type.

## Communicating with an LSP Server

Here is a basic LSP client that launches `rust-analyzer` and sends requests:

```rust
use std::io::{BufRead, BufReader, Write};
use std::process::{Command, Stdio};
use serde_json::Value;

struct LspClient {
    child: std::process::Child,
    next_id: u64,
}

impl LspClient {
    fn start(command: &str, project_root: &str) -> Result<Self, String> {
        let child = Command::new(command)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .current_dir(project_root)
            .spawn()
            .map_err(|e| format!("Failed to start {}: {}", command, e))?;

        Ok(LspClient { child, next_id: 1 })
    }

    fn send_request(&mut self, method: &str, params: Value) -> Result<u64, String> {
        let id = self.next_id;
        self.next_id += 1;

        let request = serde_json::json!({
            "jsonrpc": "2.0",
            "id": id,
            "method": method,
            "params": params,
        });

        let body = serde_json::to_string(&request)
            .map_err(|e| format!("JSON error: {}", e))?;

        let message = format!("Content-Length: {}\r\n\r\n{}", body.len(), body);

        let stdin = self.child.stdin.as_mut()
            .ok_or("Failed to access stdin")?;
        stdin.write_all(message.as_bytes())
            .map_err(|e| format!("Write error: {}", e))?;
        stdin.flush()
            .map_err(|e| format!("Flush error: {}", e))?;

        Ok(id)
    }

    fn send_notification(&mut self, method: &str, params: Value) -> Result<(), String> {
        let notification = serde_json::json!({
            "jsonrpc": "2.0",
            "method": method,
            "params": params,
        });

        let body = serde_json::to_string(&notification)
            .map_err(|e| format!("JSON error: {}", e))?;

        let message = format!("Content-Length: {}\r\n\r\n{}", body.len(), body);

        let stdin = self.child.stdin.as_mut()
            .ok_or("Failed to access stdin")?;
        stdin.write_all(message.as_bytes())
            .map_err(|e| format!("Write error: {}", e))?;
        stdin.flush()
            .map_err(|e| format!("Flush error: {}", e))?;

        Ok(())
    }

    fn read_response(&mut self) -> Result<Value, String> {
        let stdout = self.child.stdout.as_mut()
            .ok_or("Failed to access stdout")?;
        let mut reader = BufReader::new(stdout);

        // Read headers
        let mut content_length: usize = 0;
        loop {
            let mut header = String::new();
            reader.read_line(&mut header)
                .map_err(|e| format!("Read error: {}", e))?;

            let header = header.trim();
            if header.is_empty() {
                break; // Empty line signals end of headers
            }

            if let Some(length_str) = header.strip_prefix("Content-Length: ") {
                content_length = length_str.parse()
                    .map_err(|e| format!("Parse error: {}", e))?;
            }
        }

        // Read body
        let mut body = vec![0u8; content_length];
        std::io::Read::read_exact(&mut reader, &mut body)
            .map_err(|e| format!("Read error: {}", e))?;

        serde_json::from_slice(&body)
            .map_err(|e| format!("JSON parse error: {}", e))
    }
}

fn main() {
    println!("LSP client example (requires rust-analyzer to be installed)");

    // In practice, you would:
    // 1. Start the LSP server
    // 2. Send initialize request
    // 3. Wait for initialize response
    // 4. Send initialized notification
    // 5. Send textDocument/didOpen for files you want to analyze
    // 6. Send queries (definition, references, hover)
    // 7. Process responses
    // 8. Send shutdown request when done
}
```

::: wild In the Wild
Most coding agents do not run their own LSP servers. Instead, they rely on text search (ripgrep) and the LLM's own reasoning for code navigation. However, agents that integrate LSP can answer questions that are otherwise impossible without reading large amounts of code — "what type does this function return?" or "show me every implementation of this trait." The Zed editor's AI features use LSP data to provide the model with precise type information, reducing hallucination. As agents become more sophisticated, LSP integration is becoming a differentiating feature.
:::

## LSP vs Tree-Sitter: When to Use Each

The two tools have complementary strengths:

| Capability | Tree-Sitter | LSP |
|-----------|-------------|-----|
| Startup time | Instant (no project loading) | Seconds to minutes (must index project) |
| Syntax structure | Full CST for any language | Not its purpose |
| Type resolution | Type annotations only | Full type inference |
| Cross-file references | Heuristic (name matching) | Precise (semantic analysis) |
| Error tolerance | Always produces a tree | May not work with broken code |
| Multi-language | Same API for all languages | One server per language |
| Resource usage | Minimal (in-process) | Heavy (separate process, memory) |
| Query language | S-expression patterns | Request/response only |

The practical guidance for agent design:

**Use tree-sitter for:**
- File outlines and structure extraction
- Finding definitions within a single file
- Syntax-aware search (filtering out comments and strings)
- Fast, lightweight analysis that runs on every file touch

**Use LSP for:**
- Resolving inferred types
- Finding all references across the project
- Precise go-to-definition (especially for trait methods and generics)
- Code completion suggestions
- Rename refactoring that understands all usages

**Use both together:**
- Tree-sitter for initial file analysis (fast, no setup required)
- LSP for follow-up questions when tree-sitter's syntactic analysis is insufficient
- Tree-sitter queries to pre-filter before sending targeted LSP requests

## Key Takeaways

- LSP provides a standard JSON-RPC protocol for communicating with language-specific analyzers — `textDocument/definition`, `textDocument/references`, and `textDocument/hover` are the most useful requests for agents
- The initialization handshake negotiates capabilities between client and server, determining which features are available
- LSP servers like rust-analyzer provide full semantic analysis including type inference, trait resolution, and cross-file references — capabilities beyond what tree-sitter can offer
- Tree-sitter and LSP are complementary: tree-sitter provides instant, lightweight syntactic analysis while LSP provides deep semantic analysis at the cost of startup time and resource usage
- The pragmatic agent strategy is tree-sitter first for speed, LSP on demand for precision — matching the pattern of fast heuristics with precise fallbacks
