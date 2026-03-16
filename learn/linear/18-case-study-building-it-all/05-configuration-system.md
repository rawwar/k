---
title: Configuration System
description: Design the unified configuration system that controls all agent behavior through layered config files, environment variables, and CLI flags.
---

# Configuration System

> **What you'll learn:**
> - How to implement a configuration system that merges CLI arguments, environment variables, project config, and global config with clear precedence rules
> - Techniques for making configuration self-documenting with typed schemas, validation, and generated reference documentation
> - How to handle configuration for all subsystems (providers, tools, safety, plugins) through a single unified system without creating a monolithic config struct

Every component in the agent reads from configuration. The provider needs an API key and model name. The safety layer needs permission rules and allowed paths. The context manager needs compaction thresholds. The tool registry needs to know which tools are enabled. The renderer needs to know whether to use TUI mode or plain text. A well-designed configuration system is the connective tissue that lets all of these components be controlled without recompilation.

## The Configuration Hierarchy

A production coding agent loads configuration from multiple sources with a clear precedence order. Higher-precedence sources override lower ones:

```
Priority (highest to lowest):
1. CLI flags          --model claude-sonnet-4-20250514
2. Environment vars   AGENT_MODEL=claude-sonnet-4-20250514
3. Project config     ./agent.toml (in the project directory)
4. Global config      ~/.config/agent/config.toml
5. Built-in defaults  Hardcoded in the binary
```

This layering is what makes a CLI tool feel natural. The global config sets your preferred defaults. The project config customizes behavior for a specific codebase. Environment variables let CI pipelines override settings. CLI flags give you one-off control for a single invocation.

Let's implement this layered loading:

```rust
use std::path::{Path, PathBuf};
use serde::Deserialize;

#[derive(Debug, Deserialize, Default)]
#[serde(default)]
pub struct Config {
    pub provider: ProviderConfig,
    pub tools: ToolsConfig,
    pub safety: SafetyConfig,
    pub context: ContextConfig,
    pub ui: UiConfig,
    pub mcp_servers: Vec<McpServerConfig>,

    /// Tracks where this config was loaded from (not serialized)
    #[serde(skip)]
    pub source_path: Option<PathBuf>,
}

#[derive(Debug, Deserialize)]
#[serde(default)]
pub struct ProviderConfig {
    pub default_provider: String,
    pub model: String,
    pub api_key: Option<String>,
    pub max_tokens: usize,
    pub temperature: f32,
}

impl Default for ProviderConfig {
    fn default() -> Self {
        Self {
            default_provider: "anthropic".into(),
            model: "claude-sonnet-4-20250514".into(),
            api_key: None,
            max_tokens: 8192,
            temperature: 0.0,
        }
    }
}

#[derive(Debug, Deserialize, Default)]
#[serde(default)]
pub struct SafetyConfig {
    pub allowed_directories: Vec<PathBuf>,
    pub blocked_commands: Vec<String>,
    pub auto_approve_reads: bool,
    pub require_approval_for_writes: bool,
    pub max_file_size_bytes: usize,
}

#[derive(Debug, Deserialize, Default)]
#[serde(default)]
pub struct ContextConfig {
    pub max_conversation_turns: usize,
    pub compaction_threshold: f32, // 0.0 to 1.0, fraction of context window
    pub session_persistence: bool,
    pub sessions_directory: Option<PathBuf>,
}
```

The `#[serde(default)]` annotation is critical. It means that any field missing from the config file gets its `Default` value. Users only need to specify the settings they want to change.

## Loading and Merging

The loading function applies the precedence hierarchy:

```rust
pub fn load_config(cli_path: Option<&Path>) -> anyhow::Result<Config> {
    // Start with built-in defaults
    let mut config = Config::default();

    // Layer 1: Global config (~/.config/agent/config.toml)
    let global_path = dirs::config_dir()
        .map(|d| d.join("agent").join("config.toml"));

    if let Some(path) = &global_path {
        if path.exists() {
            let global = load_toml_file(path)
                .context("Failed to load global config")?;
            merge_config(&mut config, global);
            config.source_path = Some(path.clone());
        }
    }

    // Layer 2: Project config (./agent.toml)
    let project_path = find_project_config()?;
    if let Some(path) = &project_path {
        let project = load_toml_file(path)
            .context("Failed to load project config")?;
        merge_config(&mut config, project);
        config.source_path = Some(path.clone());
    }

    // Layer 3: CLI-specified config file
    if let Some(path) = cli_path {
        let cli_config = load_toml_file(path)
            .context("Failed to load specified config file")?;
        merge_config(&mut config, cli_config);
        config.source_path = Some(path.to_owned());
    }

    // Layer 4: Environment variables
    apply_env_overrides(&mut config);

    Ok(config)
}

fn apply_env_overrides(config: &mut Config) {
    if let Ok(key) = std::env::var("ANTHROPIC_API_KEY") {
        config.provider.api_key = Some(key);
    }
    if let Ok(model) = std::env::var("AGENT_MODEL") {
        config.provider.model = model;
    }
    if let Ok(provider) = std::env::var("AGENT_PROVIDER") {
        config.provider.default_provider = provider;
    }
    // Additional env vars for other subsystems...
}

fn load_toml_file(path: &Path) -> anyhow::Result<Config> {
    let content = std::fs::read_to_string(path)
        .with_context(|| format!("Could not read {}", path.display()))?;
    toml::from_str(&content)
        .with_context(|| format!("Invalid TOML in {}", path.display()))
}
```

::: python Coming from Python
This layered merging is similar to what `python-dotenv` and libraries like `pydantic-settings` do — loading from `.env`, then environment variables, then constructor arguments. The key difference is that Rust's serde deserialization gives you a fully typed config struct at load time. In Python, you might have a dict that you access with string keys, discovering typos at runtime. Here, if you write `config.provider.modle` (a typo), the compiler catches it immediately.
:::

## The Merge Function

Merging two config structs is the trickiest part. You want to override only the fields that the higher-priority source explicitly set, while keeping values from lower-priority sources for fields that were not specified. Serde's `#[serde(default)]` handles this for simple cases — missing fields get defaults. For a proper merge, you need a slightly more sophisticated approach:

```rust
fn merge_config(base: &mut Config, overlay: Config) {
    // For simple fields, only override if the overlay has a non-default value
    if overlay.provider.model != ProviderConfig::default().model {
        base.provider.model = overlay.provider.model;
    }
    if overlay.provider.api_key.is_some() {
        base.provider.api_key = overlay.provider.api_key;
    }
    if overlay.provider.max_tokens != ProviderConfig::default().max_tokens {
        base.provider.max_tokens = overlay.provider.max_tokens;
    }

    // For Vec fields, extend rather than replace
    base.safety.allowed_directories.extend(overlay.safety.allowed_directories);
    base.safety.blocked_commands.extend(overlay.safety.blocked_commands);

    // MCP servers are additive — both global and project servers are available
    base.mcp_servers.extend(overlay.mcp_servers);
}
```

An alternative is to use `Option<T>` for every field in a raw overlay struct, and only apply `Some` values:

```rust
#[derive(Debug, Deserialize)]
pub struct ConfigOverlay {
    pub provider: Option<ProviderOverlay>,
    pub safety: Option<SafetyOverlay>,
    // ...
}

#[derive(Debug, Deserialize)]
pub struct ProviderOverlay {
    pub model: Option<String>,
    pub api_key: Option<String>,
    pub max_tokens: Option<usize>,
}

fn apply_overlay(config: &mut Config, overlay: ConfigOverlay) {
    if let Some(provider) = overlay.provider {
        if let Some(model) = provider.model {
            config.provider.model = model;
        }
        if let Some(key) = provider.api_key {
            config.provider.api_key = Some(key);
        }
        if let Some(tokens) = provider.max_tokens {
            config.provider.max_tokens = tokens;
        }
    }
}
```

This Option-based approach is more verbose but unambiguous. It clearly distinguishes "field was not in the file" (None) from "field was explicitly set to the default value" (Some).

## Validation

After loading and merging, validate the final configuration before passing it to the rest of the system:

```rust
impl Config {
    pub fn validate(&self) -> anyhow::Result<()> {
        // Provider validation
        if self.provider.api_key.is_none() {
            // Check if we can infer it from common env vars
            let has_env_key = std::env::var("ANTHROPIC_API_KEY").is_ok()
                || std::env::var("OPENAI_API_KEY").is_ok();
            if !has_env_key {
                anyhow::bail!(
                    "No API key configured. Set ANTHROPIC_API_KEY or \
                     add provider.api_key to your config file."
                );
            }
        }

        // Safety validation
        for dir in &self.safety.allowed_directories {
            if !dir.exists() {
                tracing::warn!(
                    "Allowed directory does not exist: {}",
                    dir.display()
                );
            }
        }

        // Context validation
        if self.context.compaction_threshold < 0.0
            || self.context.compaction_threshold > 1.0
        {
            anyhow::bail!(
                "context.compaction_threshold must be between 0.0 and 1.0, \
                 got {}",
                self.context.compaction_threshold
            );
        }

        Ok(())
    }
}
```

Validation catches errors early — at startup, with a clear message — rather than letting them surface as mysterious runtime failures deep in the agentic loop.

## What a Config File Looks Like

Here is a complete example `agent.toml` showing all sections:

```toml
[provider]
default_provider = "anthropic"
model = "claude-sonnet-4-20250514"
max_tokens = 8192
temperature = 0.0

[tools]
enabled = ["read_file", "write_file", "shell", "search"]

[safety]
allowed_directories = ["."]
blocked_commands = ["rm -rf /", "sudo", "curl | sh"]
auto_approve_reads = true
require_approval_for_writes = true

[context]
max_conversation_turns = 100
compaction_threshold = 0.8
session_persistence = true

[ui]
mode = "plain"  # "plain" or "tui"
color = true
show_token_usage = true

[[mcp_servers]]
name = "filesystem"
command = "npx"
args = ["-y", "@modelcontextprotocol/server-filesystem", "/home/user/projects"]
```

Users can include as little or as much as they want. A minimal config might be just two lines setting the model and API key. Everything else falls back to sensible defaults.

::: wild In the Wild
Claude Code uses a layered configuration system with a global `~/.claude/settings.json` and per-project `.claude/settings.json` files. Project-level settings override global ones, and CLI flags override both. OpenCode follows a similar pattern with `~/.config/opencode/config.json` as the global config and project-level configuration that can customize model selection, tool behavior, and UI preferences. Both use JSON rather than TOML, but the layering principle is identical.
:::

## Subsystem-Specific Config Without a Monolith

One risk with a unified config system is creating a massive config struct that every component depends on. You can avoid this by having each subsystem accept only its relevant config section:

```rust
impl SafetyLayer {
    pub fn new(config: &SafetyConfig) -> Self {
        // Only receives SafetyConfig, not the full Config
        Self {
            allowed_dirs: config.allowed_directories.clone(),
            blocked_commands: config.blocked_commands.clone(),
            auto_approve_reads: config.auto_approve_reads,
            // ...
        }
    }
}

impl ContextManager {
    pub fn new(max_tokens: usize, config: &ContextConfig) -> Self {
        Self {
            max_tokens,
            compaction_threshold: config.compaction_threshold,
            persistence: config.session_persistence,
            // ...
        }
    }
}
```

Each component takes a reference to its own config section. This keeps dependencies narrow — the safety layer does not know about provider settings, the context manager does not know about UI preferences.

## Key Takeaways

- A layered configuration system (defaults, global config, project config, environment variables, CLI flags) gives users control at every granularity — from permanent preferences down to one-off invocations.
- Use `serde` with `#[serde(default)]` to make every field optional in config files, falling back to built-in defaults for anything not specified.
- Validate the merged configuration at startup and fail with clear, actionable error messages rather than letting invalid values cause mysterious failures later.
- Keep configuration modular by passing subsystem-specific config sections (like `SafetyConfig` or `ContextConfig`) to each component rather than a single monolithic config struct.
- Support both TOML config files for human editing and environment variables for CI/CD pipelines, with a clear and documented precedence order.
