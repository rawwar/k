---
title: Configuration Driven Extensions
description: Enable users to customize agent behavior through configuration files that activate extensions, set parameters, and define workflows without writing code.
---

# Configuration Driven Extensions

> **What you'll learn:**
> - How to design a layered configuration system (global, project, session) that controls which extensions are active and how they behave
> - Techniques for implementing configuration schemas with validation, defaults, and documentation so users get clear error messages for misconfigurations
> - How to support declarative workflow definitions in configuration that compose built-in and plugin-provided capabilities

Not every customization requires writing Rust code. Many users want to configure their agent's behavior -- which MCP servers to connect to, which hooks to run, what tools to enable -- without touching source code. Configuration-driven extensions let users declare what they want in TOML or JSON files, and the agent sets everything up at startup.

This approach lowers the barrier to extensibility dramatically. A user does not need to understand Rust, traits, or async programming to add an MCP server or define a pre-commit hook. They just edit a configuration file.

## The Configuration Hierarchy

Production agents use a layered configuration system where more specific settings override more general ones:

```
1. Built-in defaults (compiled into the binary)
2. Global config   (~/.config/agent/config.toml)
3. Project config  (./.agent/config.toml)
4. Session config  (command-line flags, environment variables)
```

Each layer can override settings from the layer below it. This lets users set global preferences (like their preferred LLM provider) while projects override specific settings (like which MCP servers to use).

```rust
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentConfig {
    /// LLM provider settings
    #[serde(default)]
    pub provider: ProviderConfig,

    /// MCP server configurations
    #[serde(default)]
    pub mcp_servers: HashMap<String, McpServerEntry>,

    /// Hook definitions
    #[serde(default)]
    pub hooks: HooksConfig,

    /// Enabled skills
    #[serde(default)]
    pub skills: SkillsConfig,

    /// Tool-specific overrides
    #[serde(default)]
    pub tools: ToolsConfig,

    /// Extension search paths
    #[serde(default)]
    pub extensions: ExtensionsConfig,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ProviderConfig {
    pub default_provider: Option<String>,
    pub model: Option<String>,
    pub max_tokens: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpServerEntry {
    pub command: String,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default)]
    pub env: HashMap<String, String>,
    #[serde(default = "default_true")]
    pub enabled: bool,
}

fn default_true() -> bool { true }

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct HooksConfig {
    #[serde(default)]
    pub pre_tool_execution: Vec<HookEntry>,
    #[serde(default)]
    pub post_tool_execution: Vec<HookEntry>,
    #[serde(default)]
    pub pre_message: Vec<HookEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HookEntry {
    /// Human-readable name for logging
    pub name: String,
    /// Shell command to execute
    pub command: String,
    /// Only run for these tool names (empty means all tools)
    #[serde(default)]
    pub tool_filter: Vec<String>,
    /// Timeout in seconds
    #[serde(default = "default_hook_timeout")]
    pub timeout_secs: u64,
}

fn default_hook_timeout() -> u64 { 10 }

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SkillsConfig {
    #[serde(default)]
    pub enabled: Vec<String>,
    #[serde(default)]
    pub disabled: Vec<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ToolsConfig {
    /// Tools to disable entirely
    #[serde(default)]
    pub disabled: Vec<String>,
    /// Per-tool configuration overrides
    #[serde(default)]
    pub overrides: HashMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ExtensionsConfig {
    #[serde(default)]
    pub search_paths: Vec<PathBuf>,
}
```

Here is what a project-level configuration file looks like:

```toml
# .agent/config.toml

[provider]
default_provider = "anthropic"
model = "claude-sonnet-4-20250514"
max_tokens = 8192

[mcp_servers.postgres]
command = "npx"
args = ["-y", "@modelcontextprotocol/server-postgres"]
env = { DATABASE_URL = "postgresql://localhost/myapp" }

[mcp_servers.github]
command = "npx"
args = ["-y", "@modelcontextprotocol/server-github"]
env = { GITHUB_TOKEN = "${GITHUB_TOKEN}" }

[skills]
enabled = ["code-review", "db-migration"]

[tools]
disabled = ["web_search"]

[[hooks.pre_tool_execution]]
name = "Confirm shell commands"
command = "echo 'About to run: $TOOL_ARGS' && read -p 'Continue? ' confirm"
tool_filter = ["shell"]
timeout_secs = 30

[[hooks.post_tool_execution]]
name = "Lint after file write"
command = "cargo fmt -- --check"
tool_filter = ["write_file", "edit_file"]
timeout_secs = 15
```

## Loading and Merging Configuration

The configuration loader reads from each layer and merges them:

```rust
impl AgentConfig {
    /// Load configuration from all layers, with later layers overriding earlier ones.
    pub fn load() -> Result<Self> {
        let mut config = Self::defaults();

        // Layer 2: Global config
        let global_path = dirs::config_dir()
            .map(|d| d.join("agent").join("config.toml"));
        if let Some(path) = global_path {
            if path.exists() {
                let global = Self::load_from_file(&path)?;
                config.merge(global);
            }
        }

        // Layer 3: Project config
        let project_path = PathBuf::from(".agent/config.toml");
        if project_path.exists() {
            let project = Self::load_from_file(&project_path)?;
            config.merge(project);
        }

        // Layer 4: Environment variable overrides
        config.apply_env_overrides();

        Ok(config)
    }

    fn defaults() -> Self {
        Self {
            provider: ProviderConfig {
                default_provider: Some("anthropic".to_string()),
                model: Some("claude-sonnet-4-20250514".to_string()),
                max_tokens: Some(4096),
            },
            mcp_servers: HashMap::new(),
            hooks: HooksConfig::default(),
            skills: SkillsConfig::default(),
            tools: ToolsConfig::default(),
            extensions: ExtensionsConfig::default(),
        }
    }

    fn load_from_file(path: &PathBuf) -> Result<Self> {
        let content = std::fs::read_to_string(path)?;
        let config: Self = toml::from_str(&content)
            .map_err(|e| anyhow::anyhow!(
                "Invalid config at {}: {e}", path.display()
            ))?;
        Ok(config)
    }

    fn merge(&mut self, other: Self) {
        // Provider: other overrides self field by field
        if other.provider.default_provider.is_some() {
            self.provider.default_provider = other.provider.default_provider;
        }
        if other.provider.model.is_some() {
            self.provider.model = other.provider.model;
        }
        if other.provider.max_tokens.is_some() {
            self.provider.max_tokens = other.provider.max_tokens;
        }

        // MCP servers: merge maps (project servers add to/override global)
        for (name, entry) in other.mcp_servers {
            self.mcp_servers.insert(name, entry);
        }

        // Hooks: append (both global and project hooks run)
        self.hooks.pre_tool_execution.extend(other.hooks.pre_tool_execution);
        self.hooks.post_tool_execution.extend(other.hooks.post_tool_execution);
        self.hooks.pre_message.extend(other.hooks.pre_message);

        // Skills: project config controls which skills are active
        if !other.skills.enabled.is_empty() {
            self.skills.enabled = other.skills.enabled;
        }
        self.skills.disabled.extend(other.skills.disabled);

        // Tools: disabled list is additive
        self.tools.disabled.extend(other.tools.disabled);
        for (name, value) in other.tools.overrides {
            self.tools.overrides.insert(name, value);
        }
    }

    fn apply_env_overrides(&mut self) {
        if let Ok(model) = std::env::var("AGENT_MODEL") {
            self.provider.model = Some(model);
        }
        if let Ok(provider) = std::env::var("AGENT_PROVIDER") {
            self.provider.default_provider = Some(provider);
        }
    }
}
```

::: python Coming from Python
Python projects typically use layered config with `pydantic-settings` or similar:
```python
class Settings(BaseSettings):
    model: str = "claude-sonnet-4-20250514"
    provider: str = "anthropic"

    class Config:
        env_prefix = "AGENT_"
        toml_file = [".agent/config.toml", "~/.config/agent/config.toml"]
```
Rust's `serde` gives you similar power. The key difference is that Rust deserialization fails at load time with a descriptive error if the config does not match the schema, rather than silently accepting invalid types or missing fields. This means configuration errors are caught before the agent starts, not during execution when a misconfigured plugin is first used.
:::

## Configuration Validation

Loading a config file is only half the battle. You also need to validate that the configuration makes sense -- that referenced MCP server commands exist, that tool names in filters match real tools, and that conflicting options are flagged:

```rust
#[derive(Debug)]
pub struct ConfigWarning {
    pub path: String,
    pub message: String,
    pub severity: WarningSeverity,
}

#[derive(Debug)]
pub enum WarningSeverity {
    Info,
    Warning,
    Error,
}

pub fn validate_config(
    config: &AgentConfig,
    available_tools: &[String],
    available_skills: &[String],
) -> Vec<ConfigWarning> {
    let mut warnings = Vec::new();

    // Check that disabled tools actually exist
    for tool_name in &config.tools.disabled {
        if !available_tools.contains(tool_name) {
            warnings.push(ConfigWarning {
                path: format!("tools.disabled[{}]", tool_name),
                message: format!("Tool '{}' is not recognized", tool_name),
                severity: WarningSeverity::Warning,
            });
        }
    }

    // Check that enabled skills exist
    for skill_name in &config.skills.enabled {
        if !available_skills.contains(skill_name) {
            warnings.push(ConfigWarning {
                path: format!("skills.enabled[{}]", skill_name),
                message: format!("Skill '{}' is not available", skill_name),
                severity: WarningSeverity::Warning,
            });
        }
    }

    // Check hook tool filters reference valid tools
    for hook in &config.hooks.pre_tool_execution {
        for tool_name in &hook.tool_filter {
            if !available_tools.contains(tool_name) {
                warnings.push(ConfigWarning {
                    path: format!("hooks.pre_tool_execution.{}.tool_filter", hook.name),
                    message: format!(
                        "Hook '{}' filters on unknown tool '{}'",
                        hook.name, tool_name
                    ),
                    severity: WarningSeverity::Warning,
                });
            }
        }
    }

    // Check for environment variable references that are not set
    for (name, entry) in &config.mcp_servers {
        for (key, value) in &entry.env {
            if value.starts_with("${") && value.ends_with('}') {
                let var_name = &value[2..value.len() - 1];
                if std::env::var(var_name).is_err() {
                    warnings.push(ConfigWarning {
                        path: format!("mcp_servers.{}.env.{}", name, key),
                        message: format!(
                            "Environment variable '{}' is not set",
                            var_name
                        ),
                        severity: WarningSeverity::Error,
                    });
                }
            }
        }
    }

    warnings
}
```

::: tip In the Wild
Claude Code uses a layered configuration system with global settings (`~/.claude/settings.json`), project settings (`.claude/settings.json`), and MCP configuration (`~/.claude/mcp.json` and `.claude/mcp.json`). The project-level config can add MCP servers, define hooks, and set permissions that are specific to the codebase. This pattern lets teams share agent configuration through version control -- everyone working on the project gets the same MCP servers and hooks without individual setup.
:::

## Applying Configuration at Startup

The agent's startup sequence reads configuration and translates it into live objects:

```rust
pub async fn apply_config(
    config: &AgentConfig,
    tool_registry: &mut ToolRegistry,
    hook_registry: &HookRegistry,
    skill_registry: &SkillRegistry,
    mcp_manager: &McpServerManager,
) -> Result<Vec<ConfigWarning>> {
    // Disable configured tools
    for tool_name in &config.tools.disabled {
        tool_registry.disable(tool_name);
    }

    // Connect MCP servers
    for (name, entry) in &config.mcp_servers {
        if !entry.enabled {
            continue;
        }

        // Expand environment variables in the env map
        let mut env = HashMap::new();
        for (key, value) in &entry.env {
            let expanded = expand_env_vars(value);
            env.insert(key.clone(), expanded);
        }

        let mcp_config = McpServerConfig {
            name: name.clone(),
            command: entry.command.clone(),
            args: entry.args.clone(),
            env,
            init_timeout_secs: 30,
        };

        match mcp_manager.connect(mcp_config).await {
            Ok(tools) => {
                println!("MCP server '{}' connected with {} tools", name, tools.len());
            }
            Err(e) => {
                eprintln!("Failed to connect MCP server '{}': {e}", name);
            }
        }
    }

    // Register config-driven hooks
    for hook_entry in &config.hooks.pre_tool_execution {
        let hook = ConfigDrivenHook::new(hook_entry.clone());
        hook_registry.register_tool_execution_hook(Arc::new(hook)).await;
    }

    // Validate and return warnings
    let available_tools = tool_registry.list_tool_names();
    let available_skills = skill_registry.list_skills()
        .iter().map(|s| s.name.clone()).collect::<Vec<_>>();
    let warnings = validate_config(config, &available_tools, &available_skills);

    Ok(warnings)
}

fn expand_env_vars(value: &str) -> String {
    let mut result = value.to_string();
    while let Some(start) = result.find("${") {
        if let Some(end) = result[start..].find('}') {
            let var_name = &result[start + 2..start + end];
            let var_value = std::env::var(var_name).unwrap_or_default();
            result = format!(
                "{}{}{}",
                &result[..start],
                var_value,
                &result[start + end + 1..]
            );
        } else {
            break;
        }
    }
    result
}
```

## Key Takeaways

- A **layered configuration system** (defaults, global, project, session) lets users set preferences at the right scope without creating conflicts between projects.
- **TOML configuration files** with strongly-typed Rust structs (via serde) catch misconfigurations at load time rather than at runtime, preventing the class of "silent misconfiguration" bugs.
- **Configuration validation** should check that referenced tools, skills, and environment variables exist, reporting warnings and errors before the agent starts operating.
- **Environment variable expansion** in configuration values lets sensitive data (API keys, tokens) stay out of config files while still being referenced by MCP server entries and hook commands.
- The configuration-driven approach **lowers the barrier** to extensibility: users customize their agent by editing a TOML file rather than writing Rust code or understanding plugin APIs.
