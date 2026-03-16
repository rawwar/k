---
title: MCP Resource Servers
description: Implement MCP client support for resource servers that provide contextual data like documentation, database schemas, and API references.
---

# MCP Resource Servers

> **What you'll learn:**
> - How MCP resources differ from tools -- resources provide read-only contextual data that can be injected into the agent's context window
> - Techniques for implementing resource discovery, URI-based resource fetching, and subscription-based resource updates
> - How to integrate MCP resources into your agent's context management system, including caching and freshness strategies

MCP tools let the LLM take actions. MCP resources let the LLM access data. While tools are invoked during the agentic loop in response to model decisions, resources are typically loaded before or during a conversation to provide context. Think of a database schema, API documentation, a project's coding conventions, or a list of open issues -- these are all data the LLM needs to reason effectively, and MCP resources deliver them in a standardized way.

The distinction matters architecturally. Tools are model-controlled (the LLM decides when to call them). Resources are typically application-controlled or user-controlled (your agent or the user decides which resources to load into context). This different control flow affects how you design discovery, caching, and context injection.

## Discovering Resources

Like tools, resources are discovered through a list request. Each resource has a URI, a human-readable name, an optional MIME type, and a description:

```rust
use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
pub struct McpResource {
    /// URI identifying this resource, e.g., "postgres://mydb/users/schema"
    pub uri: String,
    /// Human-readable name displayed in the UI
    pub name: String,
    /// MIME type of the resource content
    #[serde(rename = "mimeType")]
    pub mime_type: Option<String>,
    /// Description for the user (not typically sent to the LLM)
    pub description: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ResourcesListResult {
    resources: Vec<McpResource>,
}

impl McpClient {
    /// Discover all resources provided by the connected MCP server.
    pub async fn list_resources(&mut self) -> Result<Vec<McpResource>> {
        let request = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: serde_json::json!(self.next_id()),
            method: "resources/list".to_string(),
            params: None,
        };

        let result = self.send_request(request).await?;
        let list_result: ResourcesListResult = serde_json::from_value(result)?;
        Ok(list_result.resources)
    }
}
```

A PostgreSQL MCP server might list resources like:

```
- uri: "postgres://mydb/tables"         name: "Database Tables"
- uri: "postgres://mydb/users/schema"   name: "Users Table Schema"
- uri: "postgres://mydb/orders/schema"  name: "Orders Table Schema"
```

## Reading Resources

To fetch a resource's content, send a `resources/read` request with the resource URI:

```rust
#[derive(Debug, Deserialize)]
pub struct ResourceContent {
    pub uri: String,
    #[serde(rename = "mimeType")]
    pub mime_type: Option<String>,
    pub text: Option<String>,
    /// Base64-encoded binary content (for images, PDFs, etc.)
    pub blob: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ResourceReadResult {
    contents: Vec<ResourceContent>,
}

impl McpClient {
    /// Read the content of a specific resource by URI.
    pub async fn read_resource(&mut self, uri: &str) -> Result<Vec<ResourceContent>> {
        let request = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: serde_json::json!(self.next_id()),
            method: "resources/read".to_string(),
            params: Some(serde_json::json!({
                "uri": uri,
            })),
        };

        let result = self.send_request(request).await?;
        let read_result: ResourceReadResult = serde_json::from_value(result)?;
        Ok(read_result.contents)
    }
}
```

::: python Coming from Python
Python's MCP SDK makes resource reading straightforward:
```python
resources = await session.list_resources()
for resource in resources:
    content = await session.read_resource(resource.uri)
    print(f"{resource.name}: {content[0].text[:100]}...")
```
The Rust version is structurally identical -- the difference is that Rust forces you to handle the `Result` at every step, making error handling explicit. A server that returns malformed JSON, a resource that does not exist, or a network timeout all produce typed errors you must handle.
:::

## Resource Templates

Some MCP servers expose resource templates -- parameterized URIs that generate resources dynamically. For example, a GitHub MCP server might expose a template like `github://repos/{owner}/{repo}/issues` where the user fills in the owner and repo.

```rust
#[derive(Debug, Clone, Deserialize)]
pub struct McpResourceTemplate {
    /// URI template with parameter placeholders, e.g., "github://{owner}/{repo}/readme"
    #[serde(rename = "uriTemplate")]
    pub uri_template: String,
    pub name: String,
    pub description: Option<String>,
    #[serde(rename = "mimeType")]
    pub mime_type: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ResourceTemplatesListResult {
    #[serde(rename = "resourceTemplates")]
    resource_templates: Vec<McpResourceTemplate>,
}

impl McpClient {
    pub async fn list_resource_templates(&mut self) -> Result<Vec<McpResourceTemplate>> {
        let request = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: serde_json::json!(self.next_id()),
            method: "resources/templates/list".to_string(),
            params: None,
        };

        let result = self.send_request(request).await?;
        let list_result: ResourceTemplatesListResult = serde_json::from_value(result)?;
        Ok(list_result.resource_templates)
    }
}

/// Expand a URI template by substituting parameters.
pub fn expand_uri_template(
    template: &str,
    params: &std::collections::HashMap<String, String>,
) -> String {
    let mut result = template.to_string();
    for (key, value) in params {
        result = result.replace(&format!("{{{key}}}"), value);
    }
    result
}
```

## Subscribing to Resource Updates

Some resources change over time. A database schema might be altered, documentation might be updated, or new issues might be created. MCP supports subscriptions so the client is notified when a resource changes:

```rust
impl McpClient {
    /// Subscribe to updates for a specific resource.
    /// The server will send notifications when the resource changes.
    pub async fn subscribe_resource(&mut self, uri: &str) -> Result<()> {
        let request = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: serde_json::json!(self.next_id()),
            method: "resources/subscribe".to_string(),
            params: Some(serde_json::json!({
                "uri": uri,
            })),
        };

        self.send_request(request).await?;
        Ok(())
    }

    /// Unsubscribe from resource updates.
    pub async fn unsubscribe_resource(&mut self, uri: &str) -> Result<()> {
        let request = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: serde_json::json!(self.next_id()),
            method: "resources/unsubscribe".to_string(),
            params: Some(serde_json::json!({
                "uri": uri,
            })),
        };

        self.send_request(request).await?;
        Ok(())
    }
}
```

When a subscribed resource changes, the server sends a `notifications/resources/updated` notification. Your client needs to listen for these and re-fetch the resource:

```rust
/// Handle incoming notifications from an MCP server.
/// This runs in a background task, reading from the server's stdout.
pub async fn handle_server_notifications(
    mut reader: BufReader<tokio::process::ChildStdout>,
    resource_cache: Arc<RwLock<ResourceCache>>,
    client: Arc<tokio::sync::Mutex<McpClient>>,
) {
    let mut line = String::new();
    loop {
        line.clear();
        match reader.read_line(&mut line).await {
            Ok(0) => break, // EOF: server disconnected
            Ok(_) => {
                if let Ok(notification) =
                    serde_json::from_str::<JsonRpcNotification>(&line)
                {
                    match notification.method.as_str() {
                        "notifications/resources/updated" => {
                            if let Some(params) = notification.params {
                                if let Some(uri) = params.get("uri")
                                    .and_then(|u| u.as_str())
                                {
                                    // Invalidate cached version and re-fetch
                                    resource_cache.write().await.invalidate(uri);
                                    let mut client = client.lock().await;
                                    if let Ok(contents) =
                                        client.read_resource(uri).await
                                    {
                                        resource_cache.write().await
                                            .update(uri, contents);
                                    }
                                }
                            }
                        }
                        _ => {
                            // Other notification types
                        }
                    }
                }
            }
            Err(e) => {
                eprintln!("Error reading from MCP server: {e}");
                break;
            }
        }
    }
}
```

## Resource Caching

Resources are often read multiple times during a conversation. A database schema does not change between tool calls, and re-fetching it on every reference wastes time and MCP server resources. A caching layer with configurable freshness solves this:

```rust
use std::time::{Duration, Instant};

struct CachedResource {
    contents: Vec<ResourceContent>,
    fetched_at: Instant,
}

pub struct ResourceCache {
    entries: HashMap<String, CachedResource>,
    default_ttl: Duration,
    /// URIs with active subscriptions get invalidated by
    /// notifications instead of TTL.
    subscribed: HashSet<String>,
}

impl ResourceCache {
    pub fn new(default_ttl: Duration) -> Self {
        Self {
            entries: HashMap::new(),
            default_ttl,
            subscribed: HashSet::new(),
        }
    }

    /// Get a cached resource if it is still fresh.
    pub fn get(&self, uri: &str) -> Option<&Vec<ResourceContent>> {
        let entry = self.entries.get(uri)?;
        if self.subscribed.contains(uri) {
            // Subscribed resources are always considered fresh
            // (invalidated by notifications)
            Some(&entry.contents)
        } else if entry.fetched_at.elapsed() < self.default_ttl {
            Some(&entry.contents)
        } else {
            None
        }
    }

    /// Store a fetched resource in the cache.
    pub fn update(&mut self, uri: &str, contents: Vec<ResourceContent>) {
        self.entries.insert(uri.to_string(), CachedResource {
            contents,
            fetched_at: Instant::now(),
        });
    }

    /// Invalidate a cached resource (called when a subscription notification arrives).
    pub fn invalidate(&mut self, uri: &str) {
        self.entries.remove(uri);
    }

    pub fn mark_subscribed(&mut self, uri: &str) {
        self.subscribed.insert(uri.to_string());
    }
}
```

## Injecting Resources into Context

The final piece is getting resource content into the LLM's context window. Unlike tools (which the LLM invokes during the loop), resources are typically injected as system message content or as user-message attachments before the LLM starts reasoning:

```rust
/// Build a context block from cached MCP resources.
pub fn build_resource_context(
    cache: &ResourceCache,
    resource_uris: &[String],
) -> String {
    let mut context_parts = Vec::new();

    for uri in resource_uris {
        if let Some(contents) = cache.get(uri) {
            for content in contents {
                if let Some(text) = &content.text {
                    context_parts.push(format!(
                        "--- Resource: {} ---\n{}",
                        uri, text
                    ));
                }
            }
        }
    }

    context_parts.join("\n\n")
}

/// Inject MCP resources into the system prompt.
pub fn build_system_prompt_with_resources(
    base_prompt: &str,
    resource_context: &str,
) -> String {
    if resource_context.is_empty() {
        base_prompt.to_string()
    } else {
        format!(
            "{base_prompt}\n\n\
             ## Available Context\n\
             The following contextual information has been loaded from external sources:\n\n\
             {resource_context}"
        )
    }
}
```

::: wild In the Wild
Claude Code uses MCP resources to provide project-specific context to the LLM. For example, an MCP server for your project might expose your API documentation, database schema, and coding style guide as resources. When you start a conversation, Claude Code loads these resources into context so the LLM understands your project's conventions without you having to explain them every time. This pattern turns static documentation into live, always-current context that evolves with your project.
:::

## Key Takeaways

- MCP **resources** provide read-only contextual data (schemas, docs, API references) that is injected into the LLM's context, while **tools** provide executable actions the LLM invokes during the agentic loop.
- **Resource templates** with URI parameters let servers expose dynamic resource sets without pre-enumerating every possible resource.
- **Subscriptions** let the client receive notifications when resources change, enabling real-time cache invalidation instead of polling or fixed TTLs.
- A **caching layer** with TTL-based expiry for non-subscribed resources and notification-based invalidation for subscribed resources keeps the agent responsive without overwhelming MCP servers.
- Resource content is typically **injected into the system prompt** or attached as context before the LLM starts reasoning, making it available for the entire conversation.
