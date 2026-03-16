---
title: Config Driven Extensions
description: Enabling extensions through configuration files rather than code, supporting declarative tool definitions, prompt templates, and workflow rules in TOML or JSON.
---

# Config Driven Extensions

> **What you'll learn:**
> - How to define declarative tool specifications in configuration files that the agent loads at startup
> - Techniques for config-driven prompt templates and system prompt composition
> - Patterns for validating extension configurations and providing clear error messages for misconfigurations

Not everyone who wants to extend an agent can write Rust. In fact, the most common extensions are simple: add a tool that runs a shell command, inject a project-specific system prompt, or configure an MCP server. Configuration-driven extensions make these cases possible without any code at all. The user describes what they want in a TOML or JSON file, and the agent brings it to life.

## The Extension Configuration Format

Design a configuration format that covers the most common extension needs:

```rust
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::path::PathBuf;

/// The top-level configuration file structure.
#[derive(Debug, Deserialize, Serialize)]
pub struct AgentConfig {
    /// General agent settings.
    #[serde(default)]
    pub agent: AgentSettings,

    /// Custom tools defined as shell commands.
    #[serde(default)]
    pub tools: HashMap<String, ToolConfig>,

    /// MCP server configurations.
    #[serde(default)]
    pub mcp_servers: HashMap<String, McpServerConfig>,

    /// System prompt additions.
    #[serde(default)]
    pub prompts: Vec<PromptConfig>,

    /// Hook configurations.
    #[serde(default)]
    pub hooks: HookConfigs,

    /// Skills to auto-activate.
    #[serde(default)]
    pub skills: Vec<String>,
}

#[derive(Debug, Default, Deserialize, Serialize)]
pub struct AgentSettings {
    pub default_provider: Option<String>,
    pub default_model: Option<String>,
    pub max_turns: Option<usize>,
    pub working_directory: Option<PathBuf>,
}

/// A tool defined entirely through configuration.
#[derive(Debug, Deserialize, Serialize)]
pub struct ToolConfig {
    pub description: String,
    /// The shell command to execute. Use {{param_name}} for argument interpolation.
    pub command: String,
    /// Parameter definitions.
    #[serde(default)]
    pub parameters: HashMap<String, ParamConfig>,
    /// Working directory for the command.
    pub working_dir: Option<String>,
    /// Timeout in seconds.
    #[serde(default = "default_timeout")]
    pub timeout_secs: u64,
}

fn default_timeout() -> u64 {
    30
}

#[derive(Debug, Deserialize, Serialize)]
pub struct ParamConfig {
    #[serde(rename = "type")]
    pub param_type: String, // "string", "number", "boolean"
    pub description: String,
    #[serde(default)]
    pub required: bool,
    pub default: Option<Value>,
}

/// System prompt text to inject.
#[derive(Debug, Deserialize, Serialize)]
pub struct PromptConfig {
    /// When to inject this prompt (always, or when specific files exist).
    #[serde(default = "default_when")]
    pub when: String,
    /// The prompt text to inject.
    pub text: String,
    /// File patterns that must exist for "when: project" prompts.
    #[serde(default)]
    pub file_triggers: Vec<String>,
}

fn default_when() -> String {
    "always".to_string()
}

/// Hook configurations -- shell commands that run at hook points.
#[derive(Debug, Default, Deserialize, Serialize)]
pub struct HookConfigs {
    #[serde(default)]
    pub pre_tool_use: Vec<ShellHookConfig>,
    #[serde(default)]
    pub post_tool_use: Vec<ShellHookConfig>,
    #[serde(default)]
    pub pre_message: Vec<ShellHookConfig>,
    #[serde(default)]
    pub notification: Vec<ShellHookConfig>,
}

/// A hook that runs a shell command.
#[derive(Debug, Deserialize, Serialize)]
pub struct ShellHookConfig {
    /// Which tool names this hook applies to (glob patterns).
    #[serde(default)]
    pub matcher: Option<String>,
    /// Shell command to execute. Context is passed via stdin.
    pub command: String,
    /// Timeout in seconds.
    #[serde(default = "default_hook_timeout")]
    pub timeout_secs: u64,
}

fn default_hook_timeout() -> u64 {
    10
}
```

Here is what a real configuration file looks like:

```toml
[agent]
default_provider = "anthropic"
default_model = "claude-sonnet-4-20250514"
max_turns = 50

# Custom tools as shell commands
[tools.run_tests]
description = "Run the project's test suite"
command = "cargo test {{args}}"
timeout_secs = 120
[tools.run_tests.parameters.args]
type = "string"
description = "Additional arguments for cargo test"
required = false

[tools.lint]
description = "Run clippy lints on the project"
command = "cargo clippy -- -D warnings {{extra_args}}"
[tools.lint.parameters.extra_args]
type = "string"
description = "Additional clippy arguments"
required = false

[tools.search_code]
description = "Search for a pattern in the codebase"
command = "rg --json {{pattern}} {{path}}"
[tools.search_code.parameters.pattern]
type = "string"
description = "The regex pattern to search for"
required = true
[tools.search_code.parameters.path]
type = "string"
description = "Directory to search in"
required = false
default = "."

# MCP servers
[mcp_servers.memory]
command = "npx"
args = ["-y", "@modelcontextprotocol/server-memory"]

[mcp_servers.filesystem]
command = "npx"
args = ["-y", "@modelcontextprotocol/server-filesystem", "."]

# Prompt additions
[[prompts]]
when = "always"
text = "This is a Rust project. Always run cargo check after making changes."

[[prompts]]
when = "file_exists"
file_triggers = ["docker-compose.yml"]
text = "This project uses Docker. Use docker-compose commands for service management."

# Hooks
[[hooks.pre_tool_use]]
matcher = "shell"
command = "python3 scripts/security_check.py"
timeout_secs = 5

# Skills to auto-activate
skills = ["rust-dev"]
```

::: tip Coming from Python
Python developers are accustomed to configuration files like `pyproject.toml`, `setup.cfg`, or `.flake8`. The concept here is the same -- declare behavior in configuration rather than code:
```toml
# pyproject.toml
[tool.pytest.ini_options]
testpaths = ["tests"]
addopts = "-v --tb=short"

[tool.ruff]
line-length = 88
select = ["E", "F", "I"]
```
The agent's configuration file follows the same philosophy: common customizations should not require writing Rust. Just as `pyproject.toml` configures your Python toolchain, the agent config file configures tools, prompts, and hooks declaratively.
:::

## Loading and Validating Configuration

Configuration loading needs robust validation. A typo in a config file should not crash the agent -- it should produce a clear error:

```rust
use std::path::Path;

#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("Config file not found: {0}")]
    NotFound(String),
    #[error("Failed to read config: {0}")]
    ReadError(String),
    #[error("Invalid TOML: {0}")]
    ParseError(String),
    #[error("Validation error: {0}")]
    ValidationError(String),
}

pub struct ConfigLoader;

impl ConfigLoader {
    /// Load configuration from a file path, merging with defaults.
    pub fn load(path: &Path) -> Result<AgentConfig, ConfigError> {
        if !path.exists() {
            // Return defaults if no config file exists
            return Ok(AgentConfig::default());
        }

        let content = std::fs::read_to_string(path)
            .map_err(|e| ConfigError::ReadError(e.to_string()))?;

        let config: AgentConfig = toml::from_str(&content)
            .map_err(|e| ConfigError::ParseError(format!(
                "Error in {}: {}", path.display(), e
            )))?;

        Self::validate(&config)?;
        Ok(config)
    }

    /// Load from a hierarchy: project config overrides user config overrides defaults.
    pub fn load_merged(
        project_path: Option<&Path>,
        user_path: Option<&Path>,
    ) -> Result<AgentConfig, ConfigError> {
        let mut config = AgentConfig::default();

        // Load user-level config first
        if let Some(user) = user_path {
            if user.exists() {
                let user_config = Self::load(user)?;
                config = Self::merge(config, user_config);
            }
        }

        // Project-level config overrides user config
        if let Some(project) = project_path {
            if project.exists() {
                let project_config = Self::load(project)?;
                config = Self::merge(config, project_config);
            }
        }

        Ok(config)
    }

    /// Validate the configuration for common mistakes.
    fn validate(config: &AgentConfig) -> Result<(), ConfigError> {
        // Check that tool commands are not empty
        for (name, tool) in &config.tools {
            if tool.command.trim().is_empty() {
                return Err(ConfigError::ValidationError(
                    format!("Tool '{}' has an empty command", name),
                ));
            }

            // Check that parameter placeholders in the command have matching definitions
            for placeholder in extract_placeholders(&tool.command) {
                if !tool.parameters.contains_key(&placeholder) {
                    return Err(ConfigError::ValidationError(format!(
                        "Tool '{}' command uses placeholder '{{{{{}}}}}' \
                         but no matching parameter is defined",
                        name, placeholder
                    )));
                }
            }
        }

        // Check that MCP server commands exist (basic check)
        for (name, server) in &config.mcp_servers {
            if server.command.trim().is_empty() {
                return Err(ConfigError::ValidationError(
                    format!("MCP server '{}' has an empty command", name),
                ));
            }
        }

        // Check prompt configurations
        for (i, prompt) in config.prompts.iter().enumerate() {
            if prompt.text.trim().is_empty() {
                return Err(ConfigError::ValidationError(
                    format!("Prompt entry {} has empty text", i),
                ));
            }
            if prompt.when == "file_exists" && prompt.file_triggers.is_empty() {
                return Err(ConfigError::ValidationError(format!(
                    "Prompt entry {} has when='file_exists' but no file_triggers",
                    i
                )));
            }
        }

        Ok(())
    }

    /// Merge two configs, with `override_config` taking precedence.
    fn merge(base: AgentConfig, override_config: AgentConfig) -> AgentConfig {
        AgentConfig {
            agent: AgentSettings {
                default_provider: override_config
                    .agent
                    .default_provider
                    .or(base.agent.default_provider),
                default_model: override_config
                    .agent
                    .default_model
                    .or(base.agent.default_model),
                max_turns: override_config
                    .agent
                    .max_turns
                    .or(base.agent.max_turns),
                working_directory: override_config
                    .agent
                    .working_directory
                    .or(base.agent.working_directory),
            },
            tools: {
                let mut merged = base.tools;
                merged.extend(override_config.tools);
                merged
            },
            mcp_servers: {
                let mut merged = base.mcp_servers;
                merged.extend(override_config.mcp_servers);
                merged
            },
            prompts: {
                let mut merged = base.prompts;
                merged.extend(override_config.prompts);
                merged
            },
            hooks: override_config.hooks, // Hooks don't merge -- project overrides user
            skills: if override_config.skills.is_empty() {
                base.skills
            } else {
                override_config.skills
            },
        }
    }
}

/// Extract {{placeholder}} names from a command template.
fn extract_placeholders(command: &str) -> Vec<String> {
    let mut placeholders = Vec::new();
    let mut chars = command.chars().peekable();

    while let Some(ch) = chars.next() {
        if ch == '{' && chars.peek() == Some(&'{') {
            chars.next(); // consume second {
            let mut name = String::new();
            for inner in chars.by_ref() {
                if inner == '}' {
                    // Consume the second }
                    chars.next();
                    break;
                }
                name.push(inner);
            }
            if !name.is_empty() {
                placeholders.push(name);
            }
        }
    }

    placeholders
}

impl Default for AgentConfig {
    fn default() -> Self {
        Self {
            agent: AgentSettings::default(),
            tools: HashMap::new(),
            mcp_servers: HashMap::new(),
            prompts: Vec::new(),
            hooks: HookConfigs::default(),
            skills: Vec::new(),
        }
    }
}
```

## Converting Config Tools to Runtime Tools

Config-defined tools need to become executable tool handlers. The bridge between configuration and runtime is a template-based shell executor:

```rust
use std::process::Stdio;
use tokio::process::Command;

/// Create a tool handler from a config-defined tool.
pub fn create_config_tool_handler(
    tool_name: String,
    config: ToolConfig,
) -> (ToolDefinition, ToolHandler) {
    // Build the JSON Schema from parameter definitions
    let mut properties = serde_json::Map::new();
    let mut required = Vec::new();

    for (param_name, param) in &config.parameters {
        let mut prop = serde_json::Map::new();
        prop.insert("type".to_string(), Value::String(param.param_type.clone()));
        prop.insert(
            "description".to_string(),
            Value::String(param.description.clone()),
        );
        if let Some(default) = &param.default {
            prop.insert("default".to_string(), default.clone());
        }
        properties.insert(param_name.clone(), Value::Object(prop));
        if param.required {
            required.push(Value::String(param_name.clone()));
        }
    }

    let schema = serde_json::json!({
        "type": "object",
        "properties": properties,
        "required": required,
    });

    let definition = ToolDefinition {
        name: tool_name.clone(),
        description: config.description.clone(),
        parameters: schema,
    };

    let command_template = config.command.clone();
    let timeout = config.timeout_secs;
    let working_dir = config.working_dir.clone();

    let handler: ToolHandler = Arc::new(move |params: Value| {
        let template = command_template.clone();
        let wd = working_dir.clone();
        let tool = tool_name.clone();

        Box::pin(async move {
            // Interpolate parameters into the command template
            let mut command = template.clone();
            if let Some(obj) = params.as_object() {
                for (key, value) in obj {
                    let placeholder = format!("{{{{{}}}}}", key);
                    let replacement = match value {
                        Value::String(s) => s.clone(),
                        other => other.to_string(),
                    };
                    command = command.replace(&placeholder, &replacement);
                }
            }

            // Remove any unresolved placeholders (optional params not provided)
            let command = remove_unresolved_placeholders(&command);

            // Execute the shell command
            let mut cmd = Command::new("sh");
            cmd.arg("-c").arg(&command);

            if let Some(ref dir) = wd {
                cmd.current_dir(dir);
            }

            cmd.stdout(Stdio::piped()).stderr(Stdio::piped());

            let result = tokio::time::timeout(
                std::time::Duration::from_secs(timeout),
                cmd.output(),
            )
            .await;

            match result {
                Ok(Ok(output)) => {
                    let stdout = String::from_utf8_lossy(&output.stdout);
                    let stderr = String::from_utf8_lossy(&output.stderr);
                    Ok(serde_json::json!({
                        "exit_code": output.status.code().unwrap_or(-1),
                        "stdout": stdout.to_string(),
                        "stderr": stderr.to_string(),
                    }))
                }
                Ok(Err(e)) => Err(ToolError::ExecutionError(
                    tool.clone(),
                    format!("Failed to execute: {}", e),
                )),
                Err(_) => Err(ToolError::ExecutionError(
                    tool.clone(),
                    format!("Command timed out after {}s", timeout),
                )),
            }
        })
    });

    (definition, handler)
}

fn remove_unresolved_placeholders(command: &str) -> String {
    let mut result = command.to_string();
    while let Some(start) = result.find("{{") {
        if let Some(end) = result[start..].find("}}") {
            result = format!(
                "{}{}",
                &result[..start],
                &result[start + end + 2..]
            );
        } else {
            break;
        }
    }
    result.trim().to_string()
}
```

::: info In the Wild
Claude Code's configuration hierarchy mirrors what we have built here. Settings can be defined at the user level (`~/.claude/settings.json`), project level (`.claude/settings.json`), and even per-directory. Project settings override user settings. The hooks system in Claude Code is entirely config-driven -- you define shell commands in the settings file and they run at the appropriate lifecycle points. This layered configuration approach means teams can share project-level settings via version control while individual developers keep their personal preferences in their home directory.
:::

## Applying Configuration at Startup

Wire everything together when the agent starts:

```rust
pub async fn apply_config(
    config: &AgentConfig,
    tool_registry: &mut ToolRegistry,
    hook_registry: &mut HookRegistry,
    mcp_bridge: &mut McpToolBridge,
    skill_loader: &mut SkillLoader,
) -> Result<(), anyhow::Error> {
    // Register config-defined tools
    for (name, tool_config) in &config.tools {
        let (definition, handler) = create_config_tool_handler(
            name.clone(),
            tool_config.clone(),
        );
        tool_registry.register("config", definition, handler)?;
        println!("[config] Registered tool: {}", name);
    }

    // Connect MCP servers
    for (name, server_config) in &config.mcp_servers {
        match mcp_bridge
            .add_server(name, &server_config.command, &server_config.args, &server_config.env, tool_registry)
            .await
        {
            Ok(()) => println!("[config] Connected MCP server: {}", name),
            Err(e) => eprintln!("[config] Failed to connect MCP server '{}': {}", name, e),
        }
    }

    // Activate requested skills
    for skill_name in &config.skills {
        if let Err(e) = skill_loader.activate(skill_name).await {
            eprintln!("[config] Failed to activate skill '{}': {}", skill_name, e);
        }
    }

    println!("[config] Configuration applied successfully");
    Ok(())
}
```

## Key Takeaways

- Configuration-driven extensions let users add tools, prompts, MCP servers, and hooks without writing Rust -- just TOML or JSON declarations
- A layered configuration hierarchy (user-level, project-level) lets teams share project settings via version control while preserving individual preferences
- Config-defined tools use command templates with `{{placeholder}}` interpolation, converting declarative definitions into executable shell commands at runtime
- Robust validation at load time catches common mistakes (empty commands, unresolved placeholders, missing parameters) with clear error messages instead of runtime failures
- The configuration format covers the 90% case -- most extensions are simple shell-command tools, prompt additions, or MCP server references that do not need compiled code
