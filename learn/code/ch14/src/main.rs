// Chapter 14: Extensibility and Plugins — Code snapshot
//
// Builds on Ch13 (multi-provider). Demonstrates:
//   - A Hook trait with lifecycle methods (on_message, on_tool_call, on_response)
//   - A HookManager that registers and runs hooks at appropriate points
//   - A Plugin trait with manifest, initialize, and activate lifecycle
//   - A logging hook that records all tool calls
//   - An MCP-style ToolRegistry that allows adding tools at runtime

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use tokio::sync::RwLock;

// ---------------------------------------------------------------------------
// Re-used from Ch13: unified message type and provider trait
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Message {
    role: String,
    content: Value,
}

#[allow(dead_code)]
#[async_trait]
trait Provider: Send + Sync {
    async fn chat(&self, messages: &[Message]) -> Result<Message, String>;
    fn name(&self) -> &str;
}

// ---------------------------------------------------------------------------
// Hook system — the core extensibility primitive
// ---------------------------------------------------------------------------

/// What a hook can decide after inspecting / modifying context.
#[derive(Debug)]
enum HookAction {
    /// Continue with (possibly modified) context.
    Continue(HookContext),
    /// Skip the operation and use this string as the result.
    Skip(String),
    /// Abort with an error message.
    Abort(String),
}

/// The lifecycle points where hooks can intercept behavior.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
enum HookPoint {
    /// Before a user message is sent to the LLM.
    PreMessage,
    /// After the LLM responds, before display.
    PostMessage,
    /// Before a tool call is executed.
    PreToolUse,
    /// After a tool call completes.
    PostToolUse,
}

/// Data flowing through a hook chain. Fields are optional because
/// different hook points carry different payloads.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct HookContext {
    tool_name: Option<String>,
    tool_input: Option<Value>,
    tool_output: Option<Value>,
    message: Option<String>,
    metadata: Value,
}

/// The trait every hook implementation must satisfy.
#[async_trait]
trait Hook: Send + Sync {
    /// A short identifier for this hook (used in logs).
    fn name(&self) -> &str;

    /// Called when a user message arrives (PreMessage / PostMessage).
    async fn on_message(&self, ctx: HookContext) -> HookAction {
        HookAction::Continue(ctx)
    }

    /// Called around tool execution (PreToolUse / PostToolUse).
    async fn on_tool_call(&self, ctx: HookContext) -> HookAction {
        HookAction::Continue(ctx)
    }

    /// Called when the assistant's response is ready (PostMessage).
    async fn on_response(&self, ctx: HookContext) -> HookAction {
        HookAction::Continue(ctx)
    }
}

/// A registered hook entry with an ordering priority.
struct RegisteredHook {
    priority: i32, // lower = runs first
    hook: Box<dyn Hook>,
}

/// Manages a set of hooks and runs them in priority order at each hook point.
struct HookManager {
    hooks: HashMap<HookPoint, Vec<RegisteredHook>>,
}

impl HookManager {
    fn new() -> Self {
        Self {
            hooks: HashMap::new(),
        }
    }

    /// Register a hook at a given point. Lower priority numbers execute first.
    fn register(&mut self, point: HookPoint, priority: i32, hook: Box<dyn Hook>) {
        let name = hook.name().to_string();
        let entries = self.hooks.entry(point.clone()).or_default();
        entries.push(RegisteredHook { priority, hook });
        entries.sort_by_key(|r| r.priority);
        println!(
            "  [hook-mgr] Registered '{}' at {:?} (priority {})",
            name, point, priority
        );
    }

    /// Execute all hooks registered at `point`, threading context through them.
    async fn run(&self, point: &HookPoint, mut ctx: HookContext) -> HookAction {
        let hooks = match self.hooks.get(point) {
            Some(h) => h,
            None => return HookAction::Continue(ctx),
        };

        for entry in hooks {
            let action = match point {
                HookPoint::PreMessage | HookPoint::PostMessage => {
                    if *point == HookPoint::PostMessage {
                        entry.hook.on_response(ctx.clone()).await
                    } else {
                        entry.hook.on_message(ctx.clone()).await
                    }
                }
                HookPoint::PreToolUse | HookPoint::PostToolUse => {
                    entry.hook.on_tool_call(ctx.clone()).await
                }
            };

            match action {
                HookAction::Continue(modified) => ctx = modified,
                other => return other, // Skip or Abort short-circuits
            }
        }

        HookAction::Continue(ctx)
    }
}

// ---------------------------------------------------------------------------
// Dynamic tool registry — MCP-style runtime tool addition
// ---------------------------------------------------------------------------

/// Metadata describing a tool that the LLM can invoke.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct ToolDefinition {
    name: String,
    description: String,
    parameters: Value, // JSON Schema
}

/// A callable async handler for a tool.
type ToolHandler = Arc<
    dyn Fn(Value) -> Pin<Box<dyn Future<Output = Result<Value, String>> + Send>> + Send + Sync,
>;

/// An entry in the registry: definition + handler + owning plugin.
struct RegisteredTool {
    definition: ToolDefinition,
    handler: ToolHandler,
    owner: String,
}

/// Runtime tool registry that plugins (and MCP servers) can add tools to.
struct ToolRegistry {
    tools: HashMap<String, RegisteredTool>,
}

impl ToolRegistry {
    fn new() -> Self {
        Self {
            tools: HashMap::new(),
        }
    }

    /// Register a tool from a plugin.
    fn register(
        &mut self,
        owner: &str,
        definition: ToolDefinition,
        handler: ToolHandler,
    ) -> Result<(), String> {
        if self.tools.contains_key(&definition.name) {
            return Err(format!("Tool '{}' already registered", definition.name));
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
        println!(
            "  [registry] Tool '{}' registered by '{}'",
            name, owner
        );
        Ok(())
    }

    /// Convenience: register with a closure instead of a pre-wrapped Arc.
    fn register_tool<F, Fut>(
        &mut self,
        owner: &str,
        definition: ToolDefinition,
        handler: F,
    ) -> Result<(), String>
    where
        F: Fn(Value) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<Value, String>> + Send + 'static,
    {
        let handler: ToolHandler = Arc::new(move |params| Box::pin(handler(params)));
        self.register(owner, definition, handler)
    }

    /// Invoke a tool by name.
    async fn invoke(&self, name: &str, params: Value) -> Result<Value, String> {
        let tool = self
            .tools
            .get(name)
            .ok_or_else(|| format!("Tool '{}' not found", name))?;
        (tool.handler)(params).await
    }

    /// Return definitions for all registered tools (sent to the LLM).
    fn list_definitions(&self) -> Vec<ToolDefinition> {
        self.tools.values().map(|t| t.definition.clone()).collect()
    }

    /// Remove all tools belonging to a given owner.
    fn deregister_all(&mut self, owner: &str) {
        self.tools.retain(|_, t| t.owner != owner);
    }
}

// ---------------------------------------------------------------------------
// Plugin system
// ---------------------------------------------------------------------------

/// Declares what a plugin provides and requires.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct PluginManifest {
    name: String,
    version: String,
    description: String,
}

/// The surface a plugin interacts with during its lifecycle.
struct PluginContext<'a> {
    tool_registry: &'a mut ToolRegistry,
    hook_manager: &'a mut HookManager,
}

/// Every plugin implements this trait.
#[async_trait]
trait Plugin: Send + Sync {
    fn manifest(&self) -> &PluginManifest;

    /// Set up state; register hooks and tools.
    async fn activate(&mut self, ctx: &mut PluginContext<'_>) -> Result<(), String>;

    /// Tear down; cleanup happens automatically via registries.
    async fn deactivate(&mut self) -> Result<(), String> {
        Ok(())
    }
}

/// Manages registration, activation, and deactivation of plugins.
struct PluginManager {
    plugins: Vec<Box<dyn Plugin>>,
}

impl PluginManager {
    fn new() -> Self {
        Self {
            plugins: Vec::new(),
        }
    }

    fn register(&mut self, plugin: Box<dyn Plugin>) {
        println!(
            "  [plugin-mgr] Registered plugin: {}",
            plugin.manifest().name
        );
        self.plugins.push(plugin);
    }

    async fn activate_all(
        &mut self,
        tool_registry: &mut ToolRegistry,
        hook_manager: &mut HookManager,
    ) -> Result<(), String> {
        for plugin in &mut self.plugins {
            let name = plugin.manifest().name.clone();
            let mut ctx = PluginContext {
                tool_registry,
                hook_manager,
            };
            plugin.activate(&mut ctx).await.map_err(|e| {
                format!("Failed to activate plugin '{}': {}", name, e)
            })?;
            println!("  [plugin-mgr] Activated: {}", name);
        }
        Ok(())
    }

    async fn deactivate_all(&mut self) {
        for plugin in self.plugins.iter_mut().rev() {
            let name = plugin.manifest().name.clone();
            if let Err(e) = plugin.deactivate().await {
                eprintln!("  [plugin-mgr] Warning: {} deactivation error: {}", name, e);
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Concrete hook: LoggingHook — records every tool call
// ---------------------------------------------------------------------------

struct LoggingHook {
    log: Arc<RwLock<Vec<String>>>,
}

impl LoggingHook {
    fn new(log: Arc<RwLock<Vec<String>>>) -> Self {
        Self { log }
    }
}

#[async_trait]
impl Hook for LoggingHook {
    fn name(&self) -> &str {
        "logging"
    }

    async fn on_tool_call(&self, ctx: HookContext) -> HookAction {
        if let Some(tool) = &ctx.tool_name {
            let entry = format!(
                "tool_call: {} | input: {}",
                tool,
                ctx.tool_input
                    .as_ref()
                    .map(|v| v.to_string())
                    .unwrap_or_default()
            );
            self.log.write().await.push(entry);
        }
        // Logging hooks never block — always continue.
        HookAction::Continue(ctx)
    }

    async fn on_message(&self, ctx: HookContext) -> HookAction {
        if let Some(msg) = &ctx.message {
            let entry = format!("user_message: {}", msg);
            self.log.write().await.push(entry);
        }
        HookAction::Continue(ctx)
    }

    async fn on_response(&self, ctx: HookContext) -> HookAction {
        if let Some(msg) = &ctx.message {
            let entry = format!("assistant_response: {}", msg);
            self.log.write().await.push(entry);
        }
        HookAction::Continue(ctx)
    }
}

// ---------------------------------------------------------------------------
// Concrete hook: SecurityHook — blocks dangerous shell commands
// ---------------------------------------------------------------------------

struct SecurityHook;

#[async_trait]
impl Hook for SecurityHook {
    fn name(&self) -> &str {
        "security"
    }

    async fn on_tool_call(&self, ctx: HookContext) -> HookAction {
        // Only inspect the "shell" tool.
        if ctx.tool_name.as_deref() != Some("shell") {
            return HookAction::Continue(ctx);
        }
        let blocked = ["rm -rf /", "mkfs", ":(){ :|:& };:"];
        if let Some(input) = &ctx.tool_input {
            if let Some(cmd) = input.get("command").and_then(|c| c.as_str()) {
                for pattern in &blocked {
                    if cmd.contains(pattern) {
                        return HookAction::Abort(format!(
                            "Blocked dangerous command: '{}'",
                            pattern
                        ));
                    }
                }
            }
        }
        HookAction::Continue(ctx)
    }
}

// ---------------------------------------------------------------------------
// Concrete plugin: LoggingPlugin — installs the logging hook + a "show_log" tool
// ---------------------------------------------------------------------------

struct LoggingPlugin {
    manifest: PluginManifest,
    log: Arc<RwLock<Vec<String>>>,
}

impl LoggingPlugin {
    fn new() -> Self {
        Self {
            manifest: PluginManifest {
                name: "logging".into(),
                version: "1.0.0".into(),
                description: "Logs every tool call and exposes a show_log tool".into(),
            },
            log: Arc::new(RwLock::new(Vec::new())),
        }
    }
}

#[async_trait]
impl Plugin for LoggingPlugin {
    fn manifest(&self) -> &PluginManifest {
        &self.manifest
    }

    async fn activate(&mut self, ctx: &mut PluginContext<'_>) -> Result<(), String> {
        // Register the logging hook at both pre and post tool-use points.
        let hook = LoggingHook::new(self.log.clone());
        // Priority 400 — logging runs late so it sees final data.
        ctx.hook_manager
            .register(HookPoint::PreToolUse, 400, Box::new(hook));

        let log_for_response = self.log.clone();
        ctx.hook_manager.register(
            HookPoint::PostMessage,
            400,
            Box::new(LoggingHook::new(log_for_response)),
        );

        // Also register a tool that lets the LLM read the log.
        let log_handle = self.log.clone();
        ctx.tool_registry.register_tool(
            "logging",
            ToolDefinition {
                name: "show_log".into(),
                description: "Show all recorded hook log entries".into(),
                parameters: serde_json::json!({
                    "type": "object",
                    "properties": {},
                }),
            },
            move |_params| {
                let log = log_handle.clone();
                async move {
                    let entries = log.read().await;
                    Ok(serde_json::json!({ "entries": *entries }))
                }
            },
        )?;

        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Concrete plugin: SecurityPlugin — installs the security hook
// ---------------------------------------------------------------------------

struct SecurityPlugin {
    manifest: PluginManifest,
}

impl SecurityPlugin {
    fn new() -> Self {
        Self {
            manifest: PluginManifest {
                name: "security".into(),
                version: "1.0.0".into(),
                description: "Blocks dangerous shell commands via a pre-tool hook".into(),
            },
        }
    }
}

#[async_trait]
impl Plugin for SecurityPlugin {
    fn manifest(&self) -> &PluginManifest {
        &self.manifest
    }

    async fn activate(&mut self, ctx: &mut PluginContext<'_>) -> Result<(), String> {
        // Priority 0 — security runs first.
        ctx.hook_manager
            .register(HookPoint::PreToolUse, 0, Box::new(SecurityHook));
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Concrete plugin: WordCountPlugin — adds a tool at runtime (MCP-style)
// ---------------------------------------------------------------------------

struct WordCountPlugin {
    manifest: PluginManifest,
}

impl WordCountPlugin {
    fn new() -> Self {
        Self {
            manifest: PluginManifest {
                name: "word-count".into(),
                version: "1.0.0".into(),
                description: "Adds a word_count tool via the runtime registry".into(),
            },
        }
    }
}

#[async_trait]
impl Plugin for WordCountPlugin {
    fn manifest(&self) -> &PluginManifest {
        &self.manifest
    }

    async fn activate(&mut self, ctx: &mut PluginContext<'_>) -> Result<(), String> {
        ctx.tool_registry.register_tool(
            "word-count",
            ToolDefinition {
                name: "word_count".into(),
                description: "Count words in a given text".into(),
                parameters: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "text": {
                            "type": "string",
                            "description": "The text to count words in"
                        }
                    },
                    "required": ["text"]
                }),
            },
            |params| async move {
                let text = params
                    .get("text")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                let count = text.split_whitespace().count();
                Ok(serde_json::json!({ "word_count": count }))
            },
        )?;
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// MCP-style tool server stub — shows how external tools are bridged in
// ---------------------------------------------------------------------------

/// Represents one tool discovered from an MCP server.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct McpToolDef {
    name: String,
    description: String,
    #[serde(rename = "inputSchema")]
    input_schema: Value,
}

/// Simulates an MCP server connection. In production this would communicate
/// over JSON-RPC / stdio with a real child process.
struct McpServerStub {
    server_name: String,
    tools: Vec<McpToolDef>,
}

impl McpServerStub {
    /// Simulate connecting to an MCP server and discovering its tools.
    fn connect(server_name: &str) -> Self {
        // In a real implementation this would:
        //   1. Spawn the server process
        //   2. Send `initialize` JSON-RPC handshake
        //   3. Send `tools/list` to discover available tools
        let tools = vec![McpToolDef {
            name: "read_file".into(),
            description: "Read the contents of a file (simulated MCP tool)".into(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "Path to the file to read"
                    }
                },
                "required": ["path"]
            }),
        }];
        println!(
            "  [mcp] Connected to '{}', discovered {} tools",
            server_name,
            tools.len()
        );
        Self {
            server_name: server_name.into(),
            tools,
        }
    }

    /// Register all tools from this MCP server into the agent's tool registry,
    /// using the namespace convention mcp__{server}__{tool}.
    fn bridge_tools(&self, registry: &mut ToolRegistry) -> Result<(), String> {
        for tool in &self.tools {
            let namespaced = format!("mcp__{}_{}", self.server_name, tool.name);
            let original_name = tool.name.clone();
            let server = self.server_name.clone();

            registry.register_tool(
                &format!("mcp:{}", self.server_name),
                ToolDefinition {
                    name: namespaced.clone(),
                    description: tool.description.clone(),
                    parameters: tool.input_schema.clone(),
                },
                move |params| {
                    let name = original_name.clone();
                    let srv = server.clone();
                    async move {
                        // In production, this would send a JSON-RPC `tools/call`
                        // request to the MCP server process and return the result.
                        Ok(serde_json::json!({
                            "simulated": true,
                            "server": srv,
                            "tool": name,
                            "params": params,
                            "content": [{ "type": "text", "text": "(simulated result)" }]
                        }))
                    }
                },
            )?;
        }
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Agent — ties hooks, tools, and the agentic loop together
// ---------------------------------------------------------------------------

struct Agent {
    hook_manager: HookManager,
    tool_registry: ToolRegistry,
}

impl Agent {
    /// Execute a tool call, running pre/post hooks around it.
    async fn execute_tool(&self, tool_name: &str, args: Value) -> Result<Value, String> {
        // --- Pre-tool hooks ---
        let pre_ctx = HookContext {
            tool_name: Some(tool_name.into()),
            tool_input: Some(args.clone()),
            tool_output: None,
            message: None,
            metadata: serde_json::json!({}),
        };
        let pre_result = self.hook_manager.run(&HookPoint::PreToolUse, pre_ctx).await;
        let final_args = match pre_result {
            HookAction::Continue(ctx) => ctx.tool_input.unwrap_or(args),
            HookAction::Skip(reason) => {
                return Ok(serde_json::json!({ "skipped": true, "reason": reason }));
            }
            HookAction::Abort(err) => {
                return Err(format!("Hook aborted tool call: {}", err));
            }
        };

        // --- Actual invocation ---
        let result = self.tool_registry.invoke(tool_name, final_args).await?;

        // --- Post-tool hooks ---
        let post_ctx = HookContext {
            tool_name: Some(tool_name.into()),
            tool_input: None,
            tool_output: Some(result.clone()),
            message: None,
            metadata: serde_json::json!({}),
        };
        let post_result = self
            .hook_manager
            .run(&HookPoint::PostToolUse, post_ctx)
            .await;
        match post_result {
            HookAction::Continue(ctx) => Ok(ctx.tool_output.unwrap_or(result)),
            HookAction::Skip(reason) => {
                Ok(serde_json::json!({ "modified": true, "reason": reason }))
            }
            HookAction::Abort(err) => Err(format!("Post-hook aborted: {}", err)),
        }
    }

    /// Handle a user message, running message hooks.
    async fn handle_message(&self, user_msg: &str) -> Result<String, String> {
        let pre_ctx = HookContext {
            tool_name: None,
            tool_input: None,
            tool_output: None,
            message: Some(user_msg.into()),
            metadata: serde_json::json!({}),
        };
        let action = self
            .hook_manager
            .run(&HookPoint::PreMessage, pre_ctx)
            .await;
        let msg = match action {
            HookAction::Continue(ctx) => ctx.message.unwrap_or_else(|| user_msg.into()),
            HookAction::Skip(reason) => return Ok(reason),
            HookAction::Abort(err) => return Err(err),
        };

        // (In a full agent this would call the LLM provider from ch13.)
        let response = format!("(simulated response to: {})", msg);

        // Post-message hook (on_response)
        let post_ctx = HookContext {
            tool_name: None,
            tool_input: None,
            tool_output: None,
            message: Some(response.clone()),
            metadata: serde_json::json!({}),
        };
        let action = self
            .hook_manager
            .run(&HookPoint::PostMessage, post_ctx)
            .await;
        match action {
            HookAction::Continue(ctx) => Ok(ctx.message.unwrap_or(response)),
            HookAction::Skip(r) => Ok(r),
            HookAction::Abort(e) => Err(e),
        }
    }
}

// ---------------------------------------------------------------------------
// main — boots the plugin system, registers hooks/tools, runs a demo loop
// ---------------------------------------------------------------------------

#[tokio::main]
async fn main() {
    println!("Chapter 14: Extensibility and Plugins\n");

    // 1. Create core registries
    let mut tool_registry = ToolRegistry::new();
    let mut hook_manager = HookManager::new();

    // Register a built-in "shell" tool so the security hook has something to guard.
    tool_registry
        .register_tool(
            "builtin",
            ToolDefinition {
                name: "shell".into(),
                description: "Execute a shell command".into(),
                parameters: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "command": {
                            "type": "string",
                            "description": "Shell command to run"
                        }
                    },
                    "required": ["command"]
                }),
            },
            |params| async move {
                let cmd = params
                    .get("command")
                    .and_then(|v| v.as_str())
                    .unwrap_or("echo noop");
                // (Simulated — a real agent would run the command.)
                Ok(serde_json::json!({ "stdout": format!("(simulated) {}", cmd) }))
            },
        )
        .expect("register shell tool");

    // 2. Load plugins
    println!("--- Loading plugins ---");
    let mut plugin_manager = PluginManager::new();
    plugin_manager.register(Box::new(SecurityPlugin::new()));
    plugin_manager.register(Box::new(LoggingPlugin::new()));
    plugin_manager.register(Box::new(WordCountPlugin::new()));

    // 3. Activate all plugins (registers their hooks and tools)
    println!("\n--- Activating plugins ---");
    plugin_manager
        .activate_all(&mut tool_registry, &mut hook_manager)
        .await
        .expect("activate plugins");

    // 4. Bridge tools from an MCP server stub
    println!("\n--- Connecting MCP server ---");
    let mcp = McpServerStub::connect("filesystem");
    mcp.bridge_tools(&mut tool_registry)
        .expect("bridge MCP tools");

    // 5. Show all registered tools (what the LLM would see)
    println!("\n--- Available tools ---");
    for def in tool_registry.list_definitions() {
        println!("  - {} : {}", def.name, def.description);
    }

    // 6. Build the agent and run a demo
    let agent = Agent {
        hook_manager,
        tool_registry,
    };

    println!("\n--- Demo: message handling ---");
    match agent.handle_message("Hello, agent!").await {
        Ok(resp) => println!("  Response: {}", resp),
        Err(e) => eprintln!("  Error: {}", e),
    }

    println!("\n--- Demo: safe tool call (word_count) ---");
    let params = serde_json::json!({ "text": "The quick brown fox jumps over the lazy dog" });
    match agent.execute_tool("word_count", params).await {
        Ok(val) => println!("  Result: {}", val),
        Err(e) => eprintln!("  Error: {}", e),
    }

    println!("\n--- Demo: safe shell command ---");
    let params = serde_json::json!({ "command": "ls -la" });
    match agent.execute_tool("shell", params).await {
        Ok(val) => println!("  Result: {}", val),
        Err(e) => eprintln!("  Error: {}", e),
    }

    println!("\n--- Demo: dangerous shell command (should be blocked) ---");
    let params = serde_json::json!({ "command": "rm -rf /" });
    match agent.execute_tool("shell", params).await {
        Ok(val) => println!("  Result: {}", val),
        Err(e) => println!("  Blocked: {}", e),
    }

    println!("\n--- Demo: MCP tool call ---");
    let params = serde_json::json!({ "path": "/tmp/example.rs" });
    match agent
        .execute_tool("mcp__filesystem_read_file", params)
        .await
    {
        Ok(val) => println!("  Result: {}", serde_json::to_string_pretty(&val).unwrap()),
        Err(e) => eprintln!("  Error: {}", e),
    }

    println!("\n--- Demo: inspect log entries (show_log tool) ---");
    match agent.execute_tool("show_log", serde_json::json!({})).await {
        Ok(val) => println!(
            "  Log: {}",
            serde_json::to_string_pretty(&val).unwrap()
        ),
        Err(e) => eprintln!("  Error: {}", e),
    }

    // 7. Graceful shutdown
    println!("\n--- Shutting down plugins ---");
    plugin_manager.deactivate_all().await;

    println!("\nDone.");
}
