---
title: Startup Sequence
description: Design the agent startup sequence that initializes configuration, providers, tools, safety systems, and plugins in the correct order.
---

# Startup Sequence

> **What you'll learn:**
> - How to design an ordered startup sequence that respects component dependencies — configuration before providers, providers before tools, safety before execution
> - Techniques for handling startup failures gracefully, including partial initialization, fallback modes, and clear error reporting to the user
> - How to optimize startup time by deferring expensive initialization (MCP server connections, model loading) until first use

The builder pattern from the previous subchapter defines *what* gets wired together. The startup sequence defines *when* and *in what order*. Get the order wrong and you'll face cryptic initialization errors — a tool that tries to read configuration before it's loaded, a context manager that asks for the model's token limit before the provider is connected. Get error handling wrong and a missing API key will crash the agent instead of producing a helpful message.

This subchapter walks through the startup sequence step by step, from the moment the user runs the command to the moment the REPL prompt appears.

## The Dependency Order

Startup must follow the dependency graph. You cannot initialize a component before its dependencies are ready. Here is the order, and why each step comes where it does:

```
1. Parse CLI arguments     ← No dependencies
2. Initialize logging      ← Depends on verbosity flag from CLI
3. Load configuration      ← Depends on config path from CLI
4. Validate configuration  ← Depends on loaded config
5. Initialize provider     ← Depends on config (API key, model name)
6. Register built-in tools ← Depends on config (enabled tools, paths)
7. Initialize safety layer ← Depends on config (permission rules)
8. Initialize context mgr  ← Depends on provider (token limits)
9. Connect MCP servers     ← Depends on config, tool registry
10. Initialize renderer    ← Depends on config (output mode)
11. Start REPL             ← Depends on everything above
```

Let's implement this as a concrete function:

```rust
pub async fn startup(cli: Cli) -> anyhow::Result<Agent> {
    // Step 1: CLI arguments are already parsed (clap handles this)

    // Step 2: Initialize logging
    let log_level = if cli.verbose { "debug" } else { "warn" };
    tracing_subscriber::fmt()
        .with_env_filter(log_level)
        .init();
    tracing::debug!("Logging initialized at level {}", log_level);

    // Step 3: Load configuration
    let config = load_config(cli.config.as_deref())
        .context("Failed to load configuration")?;
    tracing::debug!("Configuration loaded from {:?}", config.source_path);

    // Step 4: Validate configuration
    config.validate().context("Invalid configuration")?;

    // Step 5: Initialize provider
    let provider = init_provider(&config, cli.model.as_deref())
        .await
        .context("Failed to initialize LLM provider")?;
    tracing::info!("Provider initialized: {}", provider.name());

    // Step 6: Register built-in tools
    let tool_registry = init_tools(&config)
        .context("Failed to initialize tool registry")?;
    tracing::info!("Registered {} tools", tool_registry.count());

    // Step 7: Initialize safety layer
    let safety = SafetyLayer::new(Arc::clone(&config));
    tracing::debug!("Safety layer initialized with {} rules", safety.rule_count());

    // Step 8: Initialize context manager
    let context = ContextManager::new(
        provider.context_window_size(),
        &config,
    );
    tracing::debug!(
        "Context manager initialized (max {} tokens)",
        provider.context_window_size()
    );

    // Step 9: Connect MCP servers (deferred — see below)
    let mcp_handle = if !config.mcp_servers.is_empty() {
        Some(init_mcp_servers(&config, &tool_registry).await?)
    } else {
        None
    };

    // Step 10: Initialize renderer
    let renderer = init_renderer(&config, cli.verbose)?;

    // Step 11: Assemble and return the agent
    Ok(Agent {
        config: Arc::new(config),
        provider,
        tool_registry,
        safety,
        context: Arc::new(RwLock::new(context)),
        renderer,
        _mcp_handle: mcp_handle,
    })
}
```

Each step uses `.context()` from `anyhow` to wrap errors with a human-readable description. If the provider fails to initialize because the API key is missing, the user sees "Failed to initialize LLM provider: ANTHROPIC_API_KEY not set" rather than a raw error from the HTTP client.

## Handling Startup Failures

Not all startup failures are equal. A missing API key is fatal — the agent cannot function without a provider. A failed MCP server connection is degraded — the agent works but with fewer tools. A missing config file is defaultable — use built-in defaults and tell the user.

Categorize startup failures into three tiers:

```rust
pub enum StartupSeverity {
    /// Agent cannot function — abort with a clear error message
    Fatal,
    /// Agent can function with reduced capability — warn and continue
    Degraded,
    /// Non-issue — use defaults silently
    Defaultable,
}
```

Here is how each startup step maps to a severity:

| Step | Failure Example | Severity |
|------|----------------|----------|
| Load config | File not found | Defaultable (use defaults) |
| Validate config | Invalid model name | Fatal |
| Init provider | Missing API key | Fatal |
| Init provider | Network timeout on model list | Degraded (use cached model info) |
| Register tools | A plugin tool fails to load | Degraded (skip that tool) |
| Init safety | Invalid rule syntax | Fatal (safety must be correct) |
| Init context | Token limit unknown | Degraded (use conservative default) |
| Connect MCP | Server unreachable | Degraded (skip MCP tools) |
| Init renderer | Terminal doesn't support TUI | Degraded (fall back to plain text) |

Implementing tiered failure handling:

```rust
async fn init_mcp_servers(
    config: &Config,
    registry: &ToolRegistry,
) -> anyhow::Result<McpHandle> {
    let mut handle = McpHandle::new();

    for server_config in &config.mcp_servers {
        match connect_mcp_server(server_config).await {
            Ok(connection) => {
                let tools = connection.list_tools().await?;
                for tool in tools {
                    registry.register_mcp_tool(tool, connection.clone());
                }
                handle.add(connection);
                tracing::info!(
                    "Connected to MCP server: {}",
                    server_config.name
                );
            }
            Err(e) => {
                // Degraded — warn but continue
                tracing::warn!(
                    "Failed to connect to MCP server '{}': {}. \
                     Continuing without its tools.",
                    server_config.name,
                    e
                );
            }
        }
    }

    Ok(handle)
}
```

The MCP connection loop continues past failures. If one server is down, the agent loses that server's tools but keeps everything else. The user sees a warning, not a crash.

::: python Coming from Python
In Python, you might handle this with try/except blocks and print statements. The Rust approach is structurally similar but the `Result` type makes it impossible to accidentally ignore an error — you must explicitly handle or propagate it. The `tracing` crate's structured logging also gives you more context than Python's `print()` or even `logging.warning()`, because spans and fields are automatically captured for later analysis.
:::

## Deferred Initialization

Some components are expensive to initialize but may not be needed immediately. MCP server connections involve network round-trips. Loading a local model can take seconds. Initializing the TUI requires terminal capability detection. If the user is running a one-shot command (`agent "fix the typo in README.md"`), they should not wait for MCP servers they will never use.

The deferred initialization pattern wraps an expensive component in a `OnceCell` that initializes on first access:

```rust
use tokio::sync::OnceCell;

pub struct LazyProvider {
    config: Arc<Config>,
    inner: OnceCell<Box<dyn Provider>>,
}

impl LazyProvider {
    pub fn new(config: Arc<Config>) -> Self {
        Self {
            config,
            inner: OnceCell::new(),
        }
    }

    pub async fn get(&self) -> anyhow::Result<&dyn Provider> {
        let provider = self.inner.get_or_try_init(|| async {
            create_provider(&self.config, None).await
        }).await?;
        Ok(provider.as_ref())
    }
}
```

The `OnceCell` guarantees the provider is initialized exactly once, even if multiple tasks try to access it concurrently. The first call pays the initialization cost; subsequent calls get a cached reference instantly.

Use deferred initialization for components that meet two criteria: (1) expensive to create and (2) might not be needed in every session. For components that are cheap or always needed (like the config or safety layer), eager initialization is simpler and clearer.

::: tip In the Wild
Claude Code uses a form of deferred initialization for its MCP server connections. The initial startup is fast because it only loads the core tools and configuration. MCP servers connect in the background, and their tools become available as the connections complete. If you start typing before all MCP servers are ready, the agent works with the built-in tools and gains MCP tools as they come online. OpenCode takes a similar approach with its LSP integration — the language server starts in the background and code intelligence features activate once it's ready.
:::

## Startup Time Budget

For a CLI tool, startup time is critical. Users expect the prompt to appear in under 500 milliseconds. Here is a typical breakdown:

| Step | Typical Duration |
|------|-----------------|
| Parse CLI args | < 1ms |
| Load config (file I/O) | 1-5ms |
| Validate config | < 1ms |
| Init provider (API key check, no network) | 1-2ms |
| Register built-in tools | < 1ms |
| Init safety layer | < 1ms |
| Init context manager | < 1ms |
| Init renderer | 5-20ms (terminal detection) |
| **Total (eager, no MCP)** | **~30ms** |
| Connect MCP servers (deferred) | 100-2000ms (network) |

By deferring MCP connections, the prompt appears in under 50 milliseconds. The user can start typing immediately. MCP tools appear in the tool list once their servers connect.

## Session Restoration

If your agent supports persistent sessions (as discussed in Chapter 10), the startup sequence includes an optional session restoration step:

```rust
// After context manager initialization
if let Some(session_id) = cli.resume_session {
    let session_path = config.sessions_dir.join(format!("{}.json", session_id));
    match load_session(&session_path).await {
        Ok(session) => {
            let mut ctx = context.write().await;
            ctx.restore_from_session(session)?;
            tracing::info!("Restored session {}", session_id);
        }
        Err(e) => {
            tracing::warn!("Failed to restore session: {}. Starting fresh.", e);
        }
    }
}
```

Session restoration is a degraded-severity operation. If the session file is corrupted or missing, the agent starts fresh rather than crashing.

## The Startup Checklist

Before the REPL prompt appears, run a quick self-check:

```rust
fn startup_health_check(agent: &Agent) -> Vec<String> {
    let mut warnings = Vec::new();

    if agent.tool_registry.count() == 0 {
        warnings.push("No tools registered — the agent cannot take actions".into());
    }

    if agent.config.api_key_source == ApiKeySource::Environment {
        warnings.push(
            "API key loaded from environment variable (consider using a config file)".into()
        );
    }

    if agent.context.try_read().map_or(true, |ctx| ctx.max_tokens() < 8_000) {
        warnings.push(
            "Context window is very small — complex tasks may fail".into()
        );
    }

    warnings
}
```

Print warnings before the first prompt so the user knows the agent's state. Don't make these fatal — an agent with limited tools is still useful for simple conversations.

## Key Takeaways

- The startup sequence must follow the dependency graph: configuration first, then providers and registries that depend on configuration, then the agentic loop components that depend on registries.
- Classify startup failures as fatal (abort), degraded (warn and continue with reduced capability), or defaultable (silently use defaults) — never crash the agent for a non-fatal issue like a missing MCP server.
- Use deferred initialization (`OnceCell`) for expensive components that might not be needed immediately, such as MCP server connections and LSP integration, to keep startup time under 500 milliseconds.
- Session restoration and health checks happen at the end of startup, providing graceful recovery from corrupted state and clear visibility into the agent's operational status.
- Every startup step should wrap errors with human-readable context using `.context()` so that users see "Failed to initialize LLM provider: API key not set" rather than raw error details.
