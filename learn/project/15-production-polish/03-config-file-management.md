---
title: Config File Management
description: Designing a layered configuration system that merges global defaults, user preferences, and project-specific overrides from TOML files and environment variables.
---

# Config File Management

> **What you'll learn:**
> - How to implement a layered config system that merges global, user, and project-level settings
> - Techniques for config file discovery following XDG conventions and project root detection
> - How to validate configuration at load time and provide clear error messages for invalid values

A coding agent needs configuration: which LLM provider to use, API keys, model preferences, tool permissions, default behaviors. Hardcoding these values is a non-starter. Environment variables work for secrets but are clumsy for complex settings. You need a configuration file system that is layered, discoverable, and validated. In this subchapter, you will build one.

## The Configuration Hierarchy

Production CLI tools use a layered configuration model where more specific settings override more general ones. Here is the hierarchy for the agent, from lowest to highest priority:

1. **Built-in defaults** -- compiled into the binary
2. **Global config** -- `~/.config/agent/config.toml` (user-wide settings)
3. **Project config** -- `.agent.toml` in the project root (per-project overrides)
4. **Environment variables** -- `AGENT_*` prefixed variables
5. **CLI flags** -- highest priority, override everything

Let's define the configuration structure:

```rust
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct AgentConfig {
    pub provider: ProviderConfig,
    pub tools: ToolsConfig,
    pub logging: LoggingConfig,
    pub ui: UiConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ProviderConfig {
    /// The LLM provider to use: "anthropic", "openai", or "ollama"
    pub name: String,
    /// The model identifier, e.g. "claude-sonnet-4-20250514"
    pub model: String,
    /// API base URL (useful for proxies or self-hosted models)
    pub api_url: Option<String>,
    /// Maximum tokens for the response
    pub max_tokens: u32,
    /// Temperature for response generation (0.0 to 1.0)
    pub temperature: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ToolsConfig {
    /// Shell commands that are allowed without confirmation
    pub allowed_commands: Vec<String>,
    /// Directories the agent can read from
    pub allowed_read_paths: Vec<PathBuf>,
    /// Directories the agent can write to
    pub allowed_write_paths: Vec<PathBuf>,
    /// Maximum shell command execution time in seconds
    pub command_timeout_secs: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct LoggingConfig {
    /// Log level: "error", "warn", "info", "debug", "trace"
    pub level: String,
    /// Write logs to a file instead of stderr
    pub file: Option<PathBuf>,
    /// Use JSON format for log output
    pub json: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct UiConfig {
    /// Enable colored output
    pub color: bool,
    /// Enable markdown rendering in responses
    pub markdown: bool,
    /// Maximum width for response output (0 = terminal width)
    pub max_width: usize,
}

// Sensible defaults compiled into the binary
impl Default for AgentConfig {
    fn default() -> Self {
        Self {
            provider: ProviderConfig::default(),
            tools: ToolsConfig::default(),
            logging: LoggingConfig::default(),
            ui: UiConfig::default(),
        }
    }
}

impl Default for ProviderConfig {
    fn default() -> Self {
        Self {
            name: "anthropic".to_string(),
            model: "claude-sonnet-4-20250514".to_string(),
            api_url: None,
            max_tokens: 4096,
            temperature: 0.0,
        }
    }
}

impl Default for ToolsConfig {
    fn default() -> Self {
        Self {
            allowed_commands: vec![
                "cargo".to_string(),
                "git".to_string(),
                "ls".to_string(),
                "cat".to_string(),
            ],
            allowed_read_paths: vec![PathBuf::from(".")],
            allowed_write_paths: vec![PathBuf::from(".")],
            command_timeout_secs: 30,
        }
    }
}

impl Default for LoggingConfig {
    fn default() -> Self {
        Self {
            level: "info".to_string(),
            file: None,
            json: false,
        }
    }
}

impl Default for UiConfig {
    fn default() -> Self {
        Self {
            color: true,
            markdown: true,
            max_width: 0,
        }
    }
}
```

The `#[serde(default)]` attribute on every struct is critical. It means that if a config file only specifies `provider.model`, all other fields get their default values. Users only need to set the things they want to change.

::: python Coming from Python
In Python, you might use `pydantic` for configuration with validation, or `configparser` for INI files, or just a `dict` with defaults. Rust's `serde` with `#[serde(default)]` gives you the same ergonomics as pydantic's `BaseModel` with default values -- the struct defines the schema, defaults, and deserialization all in one place. The difference is that type mismatches are caught at deserialize time with clear error messages, not at runtime when you access a field.
:::

## Config File Discovery

The agent needs to find config files in standard locations. You follow XDG Base Directory conventions on Linux and the standard config locations on macOS.

```rust
use std::path::{Path, PathBuf};

/// Locate the global config file path following platform conventions.
pub fn global_config_path() -> PathBuf {
    if let Ok(xdg) = std::env::var("XDG_CONFIG_HOME") {
        return PathBuf::from(xdg).join("agent").join("config.toml");
    }

    if let Some(home) = dirs::home_dir() {
        #[cfg(target_os = "macos")]
        {
            return home
                .join("Library")
                .join("Application Support")
                .join("agent")
                .join("config.toml");
        }

        #[cfg(not(target_os = "macos"))]
        {
            return home.join(".config").join("agent").join("config.toml");
        }
    }

    // Fallback if we cannot determine the home directory
    PathBuf::from(".agent.toml")
}

/// Search upward from the current directory for a project-level config file.
pub fn find_project_config(start: &Path) -> Option<PathBuf> {
    let mut current = start.to_path_buf();
    loop {
        let candidate = current.join(".agent.toml");
        if candidate.exists() {
            return Some(candidate);
        }

        // Also check for the config inside an .agent directory
        let dir_candidate = current.join(".agent").join("config.toml");
        if dir_candidate.exists() {
            return Some(dir_candidate);
        }

        if !current.pop() {
            return None;
        }
    }
}
```

## Loading and Merging Configuration

Here is where the layers come together. You load each config source in order and merge them, with later sources overriding earlier ones.

```rust
use std::path::Path;

/// Load configuration from all sources, merged in priority order.
pub fn load_config(project_dir: &Path) -> Result<AgentConfig, ConfigError> {
    // Layer 1: Built-in defaults
    let mut config = AgentConfig::default();

    // Layer 2: Global config file
    let global_path = global_config_path();
    if global_path.exists() {
        let global = load_toml_config(&global_path)?;
        merge_config(&mut config, global);
        tracing::debug!(path = %global_path.display(), "Loaded global config");
    }

    // Layer 3: Project config file
    if let Some(project_path) = find_project_config(project_dir) {
        let project = load_toml_config(&project_path)?;
        merge_config(&mut config, project);
        tracing::debug!(path = %project_path.display(), "Loaded project config");
    }

    // Layer 4: Environment variables
    apply_env_overrides(&mut config);

    // Validate the final merged config
    validate_config(&config)?;

    Ok(config)
}

fn load_toml_config(path: &Path) -> Result<toml::Value, ConfigError> {
    let content = std::fs::read_to_string(path).map_err(|e| ConfigError::ReadError {
        path: path.to_path_buf(),
        source: e,
    })?;

    toml::from_str(&content).map_err(|e| ConfigError::ParseError {
        path: path.to_path_buf(),
        message: e.to_string(),
    })
}

fn merge_config(base: &mut AgentConfig, overrides: toml::Value) {
    // Re-serialize the base config to a TOML Value, merge, then deserialize back.
    // This approach handles partial overrides correctly.
    let mut base_value = toml::Value::try_from(&*base).unwrap();

    if let (toml::Value::Table(base_table), toml::Value::Table(override_table)) =
        (&mut base_value, overrides)
    {
        merge_tables(base_table, override_table);
    }

    if let Ok(merged) = base_value.try_into::<AgentConfig>() {
        *base = merged;
    }
}

fn merge_tables(base: &mut toml::map::Map<String, toml::Value>, overrides: toml::map::Map<String, toml::Value>) {
    for (key, value) in overrides {
        match (base.get_mut(&key), value.clone()) {
            (Some(toml::Value::Table(existing)), toml::Value::Table(incoming)) => {
                merge_tables(existing, incoming);
            }
            _ => {
                base.insert(key, value);
            }
        }
    }
}

fn apply_env_overrides(config: &mut AgentConfig) {
    if let Ok(val) = std::env::var("AGENT_PROVIDER") {
        config.provider.name = val;
    }
    if let Ok(val) = std::env::var("AGENT_MODEL") {
        config.provider.model = val;
    }
    if let Ok(val) = std::env::var("AGENT_API_URL") {
        config.provider.api_url = Some(val);
    }
    if let Ok(val) = std::env::var("AGENT_MAX_TOKENS") {
        if let Ok(n) = val.parse() {
            config.provider.max_tokens = n;
        }
    }
    if let Ok(val) = std::env::var("AGENT_LOG_LEVEL") {
        config.logging.level = val;
    }
}

#[derive(Debug)]
pub enum ConfigError {
    ReadError { path: PathBuf, source: std::io::Error },
    ParseError { path: PathBuf, message: String },
    ValidationError { field: String, message: String },
}

impl std::fmt::Display for ConfigError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ConfigError::ReadError { path, source } => {
                write!(f, "Cannot read config {}: {}", path.display(), source)
            }
            ConfigError::ParseError { path, message } => {
                write!(f, "Invalid TOML in {}: {}", path.display(), message)
            }
            ConfigError::ValidationError { field, message } => {
                write!(f, "Config validation failed for '{field}': {message}")
            }
        }
    }
}

impl std::error::Error for ConfigError {}
```

## Validating Configuration

Validation catches errors early, before they cause confusing failures deep in the agent.

```rust
fn validate_config(config: &AgentConfig) -> Result<(), ConfigError> {
    // Validate provider
    let valid_providers = ["anthropic", "openai", "ollama"];
    if !valid_providers.contains(&config.provider.name.as_str()) {
        return Err(ConfigError::ValidationError {
            field: "provider.name".to_string(),
            message: format!(
                "Unknown provider '{}'. Valid options: {}",
                config.provider.name,
                valid_providers.join(", ")
            ),
        });
    }

    // Validate temperature range
    if !(0.0..=2.0).contains(&config.provider.temperature) {
        return Err(ConfigError::ValidationError {
            field: "provider.temperature".to_string(),
            message: format!(
                "Temperature {} is out of range. Must be between 0.0 and 2.0",
                config.provider.temperature
            ),
        });
    }

    // Validate log level
    let valid_levels = ["error", "warn", "info", "debug", "trace"];
    if !valid_levels.contains(&config.logging.level.as_str()) {
        return Err(ConfigError::ValidationError {
            field: "logging.level".to_string(),
            message: format!(
                "Invalid log level '{}'. Valid options: {}",
                config.logging.level,
                valid_levels.join(", ")
            ),
        });
    }

    // Validate timeout
    if config.tools.command_timeout_secs == 0 {
        return Err(ConfigError::ValidationError {
            field: "tools.command_timeout_secs".to_string(),
            message: "Command timeout must be greater than 0".to_string(),
        });
    }

    Ok(())
}
```

## Example Config Files

Here is what a user's global config might look like:

```toml
# ~/.config/agent/config.toml

[provider]
name = "anthropic"
model = "claude-sonnet-4-20250514"
max_tokens = 8192

[logging]
level = "info"

[ui]
color = true
markdown = true
```

And a project-level override that uses a different model for a specific codebase:

```toml
# /path/to/project/.agent.toml

[provider]
model = "claude-sonnet-4-20250514"
temperature = 0.1

[tools]
allowed_commands = ["cargo", "git", "npm", "node"]
command_timeout_secs = 60

[tools]
allowed_write_paths = ["src/", "tests/"]
```

::: wild In the Wild
Claude Code reads configuration from `~/.claude/settings.json` for global settings and `.claude/settings.json` in the project root for project-specific overrides. OpenCode uses a similar layered TOML approach. Both systems allow project-level config to restrict permissions (for example, limiting which directories the agent can write to), which is essential when the agent operates on shared codebases with sensitive areas.
:::

## Key Takeaways

- Use a layered configuration hierarchy (defaults, global file, project file, environment variables, CLI flags) where each layer overrides the previous one, giving users precise control at every scope.
- Follow platform conventions for config file locations: XDG on Linux, `~/Library/Application Support` on macOS, and search upward from the working directory for project-level config.
- Apply `#[serde(default)]` on all config structs so users only need to specify the fields they want to change -- everything else gets sensible defaults.
- Validate configuration at load time with clear error messages that name the offending field and explain valid values, rather than letting invalid config cause mysterious failures later.
- Use environment variables with an `AGENT_*` prefix for settings that should not be checked into version control, especially API keys and URLs for different environments.
