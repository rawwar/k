---
title: Config File Management
description: Design a configuration file system with sensible defaults, layered overrides, and migration strategies for handling config changes across updates.
---

# Config File Management

> **What you'll learn:**
> - How to implement a layered configuration system that merges global (~/.config), project (.agent.toml), and environment variable settings
> - Techniques for schema evolution and config migration that update user configuration files when the format changes between versions
> - How to provide clear error messages, default values, and documentation for every configuration option so users can self-serve

Your coding agent is now installable. But users need to customize it: API keys, preferred model, output format, safety settings. A well-designed configuration system makes this seamless. A poorly designed one leads to confusion, lost settings, and angry issues on your GitHub repository. This subchapter covers how to build a layered configuration system that follows platform conventions, supports per-project overrides, and handles schema changes across versions gracefully.

## Platform-Specific Paths: The XDG Convention

Every operating system has conventions for where configuration files live. Following these conventions means your tool plays nicely with the user's existing setup:

| Platform | Config Directory | Data Directory |
|----------|-----------------|----------------|
| Linux | `~/.config/my-agent/` | `~/.local/share/my-agent/` |
| macOS | `~/Library/Application Support/my-agent/` or `~/.config/my-agent/` | Same |
| Windows | `%APPDATA%\my-agent\` | `%LOCALAPPDATA%\my-agent\` |

The `dirs` crate handles platform detection for you:

```rust
use std::path::PathBuf;

fn config_dir() -> PathBuf {
    dirs::config_dir()
        .expect("Could not determine config directory")
        .join("my-agent")
}

fn data_dir() -> PathBuf {
    dirs::data_dir()
        .expect("Could not determine data directory")
        .join("my-agent")
}

fn cache_dir() -> PathBuf {
    dirs::cache_dir()
        .expect("Could not determine cache directory")
        .join("my-agent")
}
```

On Linux, `dirs::config_dir()` respects the `XDG_CONFIG_HOME` environment variable. If it is set, configuration goes to `$XDG_CONFIG_HOME/my-agent/`. If not, it falls back to `~/.config/my-agent/`. This is important for users who have customized XDG paths.

::: python Coming from Python
Python tools typically use `~/.toolname` (a hidden directory in the home folder) or leverage `platformdirs` / `appdirs` for cross-platform paths. The concepts are identical -- the `dirs` crate in Rust serves the same purpose as `platformdirs` in Python. The main difference is that Rust tools almost always use TOML for configuration (following Cargo's lead), while Python tools vary between INI, YAML, JSON, and TOML.
:::

## Configuration File Format

TOML is the natural choice for Rust projects. Users who work with `Cargo.toml` are already familiar with the syntax. Define your configuration as a Rust struct using serde:

```rust
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    /// The LLM provider to use (e.g., "anthropic", "openai")
    pub provider: String,

    /// The specific model to use
    pub model: String,

    /// Maximum tokens per response
    pub max_tokens: u32,

    /// Whether to stream responses incrementally
    pub stream: bool,

    /// Safety and permission settings
    pub safety: SafetyConfig,

    /// Telemetry settings
    pub telemetry: TelemetryConfig,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct SafetyConfig {
    /// Require confirmation before running shell commands
    pub confirm_shell: bool,

    /// Require confirmation before modifying files
    pub confirm_file_write: bool,

    /// Directories the agent is allowed to modify
    pub allowed_dirs: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct TelemetryConfig {
    /// Whether telemetry is enabled
    pub enabled: bool,

    /// Anonymous installation ID
    pub install_id: Option<String>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            provider: "anthropic".to_string(),
            model: "claude-sonnet-4-20250514".to_string(),
            max_tokens: 4096,
            stream: true,
            safety: SafetyConfig::default(),
            telemetry: TelemetryConfig::default(),
        }
    }
}

impl Default for SafetyConfig {
    fn default() -> Self {
        Self {
            confirm_shell: true,
            confirm_file_write: true,
            allowed_dirs: vec![],
        }
    }
}

impl Default for TelemetryConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            install_id: None,
        }
    }
}
```

The `#[serde(default)]` attribute is crucial. When a user's config file is missing fields (because they were added in a newer version), serde fills them with the defaults instead of failing with a parse error.

The corresponding TOML file (`~/.config/my-agent/config.toml`) looks like:

```toml
provider = "anthropic"
model = "claude-sonnet-4-20250514"
max_tokens = 4096
stream = true

[safety]
confirm_shell = true
confirm_file_write = true
allowed_dirs = ["/home/user/projects"]

[telemetry]
enabled = false
```

## Layered Configuration

A professional tool merges configuration from multiple sources with a clear precedence order. Higher-precedence sources override lower ones:

1. **Defaults** -- Hardcoded in the binary (lowest precedence)
2. **Global config** -- `~/.config/my-agent/config.toml`
3. **Project config** -- `.agent.toml` in the current directory or repository root
4. **Environment variables** -- `MY_AGENT_MODEL=claude-sonnet-4-20250514`
5. **Command-line flags** -- `--model claude-sonnet-4-20250514` (highest precedence)

Here is how to implement the layered merge:

```rust
use std::path::Path;
use std::fs;

impl Config {
    pub fn load() -> Result<Self, Box<dyn std::error::Error>> {
        // Start with defaults
        let mut config = Config::default();

        // Layer 1: Global config
        let global_path = config_dir().join("config.toml");
        if global_path.exists() {
            let contents = fs::read_to_string(&global_path)?;
            let global: Config = toml::from_str(&contents)?;
            config.merge(global);
        }

        // Layer 2: Project config (walk up to find .agent.toml)
        if let Some(project_config_path) = find_project_config() {
            let contents = fs::read_to_string(&project_config_path)?;
            let project: Config = toml::from_str(&contents)?;
            config.merge(project);
        }

        // Layer 3: Environment variables
        config.apply_env_overrides();

        Ok(config)
    }

    fn merge(&mut self, other: Config) {
        // Only override fields that were explicitly set.
        // A real implementation might use Option<T> for every field
        // to distinguish "not set" from "set to default value."
        self.provider = other.provider;
        self.model = other.model;
        self.max_tokens = other.max_tokens;
        self.stream = other.stream;
        // Merge nested configs similarly...
    }

    fn apply_env_overrides(&mut self) {
        if let Ok(provider) = std::env::var("MY_AGENT_PROVIDER") {
            self.provider = provider;
        }
        if let Ok(model) = std::env::var("MY_AGENT_MODEL") {
            self.model = model;
        }
        if let Ok(val) = std::env::var("MY_AGENT_MAX_TOKENS") {
            if let Ok(tokens) = val.parse() {
                self.max_tokens = tokens;
            }
        }
    }
}

fn find_project_config() -> Option<std::path::PathBuf> {
    let mut dir = std::env::current_dir().ok()?;
    loop {
        let candidate = dir.join(".agent.toml");
        if candidate.exists() {
            return Some(candidate);
        }
        if !dir.pop() {
            return None;
        }
    }
}
```

::: details Using the `config` crate for layered configuration
The [`config`](https://crates.io/crates/config) crate provides a batteries-included solution for layered configuration with support for multiple file formats, environment variables, and typed access. It is a good choice if you want a more feature-rich configuration system without implementing the merge logic yourself.
:::

## Schema Evolution and Migration

Configuration schemas change over time. A field gets renamed, a section gets restructured, a deprecated option gets removed. Without migration support, users see confusing errors when they upgrade to a new version.

The simplest approach is a version field in the config file:

```toml
config_version = 2

provider = "anthropic"
model = "claude-sonnet-4-20250514"
```

Then handle migration in code:

```rust
use serde_json::Value;

fn migrate_config(raw: &str) -> Result<String, Box<dyn std::error::Error>> {
    let mut table: toml::Table = toml::from_str(raw)?;

    let version = table.get("config_version")
        .and_then(|v| v.as_integer())
        .unwrap_or(1);

    // Migrate from v1 to v2: renamed "api_key" to provider-specific key
    if version < 2 {
        if let Some(api_key) = table.remove("api_key") {
            let provider = table.entry("provider_config")
                .or_insert(toml::Value::Table(toml::Table::new()));
            if let toml::Value::Table(ref mut t) = provider {
                t.insert("api_key".to_string(), api_key);
            }
        }
        table.insert(
            "config_version".to_string(),
            toml::Value::Integer(2),
        );
    }

    // Migrate from v2 to v3: "safety.auto_approve" replaced by
    // separate confirm_shell and confirm_file_write
    if version < 3 {
        // ... migration logic
        table.insert(
            "config_version".to_string(),
            toml::Value::Integer(3),
        );
    }

    Ok(toml::to_string_pretty(&table)?)
}
```

When migrating, write the updated config back to disk so the user only sees the migration once:

```rust
fn load_and_migrate(path: &Path) -> Result<Config, Box<dyn std::error::Error>> {
    let raw = fs::read_to_string(path)?;
    let migrated = migrate_config(&raw)?;

    // Write back the migrated config
    if migrated != raw {
        eprintln!("Migrated config file to latest format: {}", path.display());
        fs::write(path, &migrated)?;
    }

    Ok(toml::from_str(&migrated)?)
}
```

## Error Messages and Validation

When a config file has errors, tell the user exactly what is wrong and how to fix it:

```rust
fn validate_config(config: &Config) -> Result<(), Vec<String>> {
    let mut errors = Vec::new();

    if config.max_tokens == 0 {
        errors.push(
            "max_tokens must be greater than 0 (current: 0)".to_string()
        );
    }

    if config.max_tokens > 100_000 {
        errors.push(format!(
            "max_tokens exceeds maximum of 100,000 (current: {})",
            config.max_tokens
        ));
    }

    let valid_providers = ["anthropic", "openai"];
    if !valid_providers.contains(&config.provider.as_str()) {
        errors.push(format!(
            "Unknown provider '{}'. Valid options: {}",
            config.provider,
            valid_providers.join(", ")
        ));
    }

    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}
```

When the config file fails to parse, show the file path and the parse error:

```rust
match toml::from_str::<Config>(&contents) {
    Ok(config) => Ok(config),
    Err(e) => {
        eprintln!("Error parsing config file: {}", path.display());
        eprintln!("{}", e);
        eprintln!();
        eprintln!("Fix the error above, or delete the file to reset to defaults:");
        eprintln!("  rm {}", path.display());
        std::process::exit(1);
    }
}
```

::: wild In the Wild
Claude Code uses a layered configuration approach where project-level settings (in files like `.claude/settings.json`) override global settings. This lets teams share a baseline configuration in their repository while individual developers can customize their personal setup. The pattern of walking up the directory tree to find project configuration is borrowed from tools like `.gitignore`, `.editorconfig`, and `tsconfig.json`.
:::

## Generating Default Config

Add a `--init` command that creates a documented default configuration file:

```rust
fn init_config() -> Result<(), Box<dyn std::error::Error>> {
    let dir = config_dir();
    std::fs::create_dir_all(&dir)?;

    let path = dir.join("config.toml");
    if path.exists() {
        eprintln!("Config file already exists: {}", path.display());
        eprintln!("Edit it directly or delete it to regenerate.");
        return Ok(());
    }

    let default_config = r#"# my-agent configuration
# See https://github.com/yourname/my-agent/blob/main/docs/config.md for details
config_version = 3

# LLM provider: "anthropic" or "openai"
provider = "anthropic"

# Model to use for code generation
model = "claude-sonnet-4-20250514"

# Maximum tokens per response
max_tokens = 4096

# Stream responses incrementally
stream = true

[safety]
# Require confirmation before running shell commands
confirm_shell = true

# Require confirmation before modifying files
confirm_file_write = true

# Directories the agent is allowed to modify (empty = current directory only)
# allowed_dirs = ["/home/user/projects"]

[telemetry]
# Enable anonymous usage telemetry
enabled = false
"#;

    std::fs::write(&path, default_config)?;
    println!("Created config file: {}", path.display());
    Ok(())
}
```

## Key Takeaways

- Use the `dirs` crate to follow platform conventions for config file locations: `~/.config/` on Linux, `~/Library/Application Support/` on macOS, `%APPDATA%` on Windows.
- Implement layered configuration with clear precedence: defaults, global config, project config, environment variables, CLI flags.
- Always use `#[serde(default)]` on your config struct so that missing fields in older config files are filled with defaults instead of causing parse errors.
- Include a `config_version` field and migration logic so that schema changes across versions are handled automatically.
- Provide an `--init` command that generates a well-commented default configuration file to help users get started.
