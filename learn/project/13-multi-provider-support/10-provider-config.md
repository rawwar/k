---
title: Provider Config
description: Configuring providers through files and environment variables, managing API keys securely, and supporting per-project provider preferences.
---

# Provider Config

> **What you'll learn:**
> - How to design a configuration schema that supports multiple providers with per-provider settings
> - Techniques for secure API key management using environment variables and system keychains
> - How to support per-project provider overrides through local configuration files

Your provider system now supports three backends with fallback, cost tracking, and runtime switching. But how does the user configure which providers are available, which API keys to use, and what their default model should be? This subchapter builds the configuration layer that ties everything together.

## Configuration Sources

A good configuration system reads from multiple sources with a clear precedence order. From highest to lowest priority:

1. **Command-line arguments** (`--model claude-sonnet-4-20250514`)
2. **Environment variables** (`ANTHROPIC_API_KEY`, `OPENAI_API_KEY`)
3. **Project-local config file** (`.agent/config.toml` in the current directory)
4. **User-global config file** (`~/.config/agent/config.toml`)
5. **Built-in defaults**

Higher-priority sources override lower ones. This lets users set global defaults in their home directory and override specific settings per project.

## The Configuration Schema

Define the configuration structure using serde:

```rust
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ProviderConfig {
    /// The default provider to use when none is specified.
    #[serde(default = "default_provider")]
    pub default_provider: String,

    /// The default model for each provider.
    #[serde(default)]
    pub default_models: HashMap<String, String>,

    /// Anthropic-specific configuration.
    #[serde(default)]
    pub anthropic: AnthropicConfig,

    /// OpenAI-specific configuration.
    #[serde(default)]
    pub openai: OpenAIConfig,

    /// Ollama-specific configuration.
    #[serde(default)]
    pub ollama: OllamaConfig,

    /// Fallback chain configuration.
    #[serde(default)]
    pub fallback: FallbackConfig,

    /// Budget and cost settings.
    #[serde(default)]
    pub cost: CostConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AnthropicConfig {
    /// API key. Prefer ANTHROPIC_API_KEY env var over this field.
    #[serde(default)]
    pub api_key: Option<String>,

    /// Custom base URL (e.g., for proxies or enterprise endpoints).
    #[serde(default)]
    pub base_url: Option<String>,

    /// Default model if not specified elsewhere.
    #[serde(default)]
    pub default_model: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct OpenAIConfig {
    #[serde(default)]
    pub api_key: Option<String>,

    #[serde(default)]
    pub base_url: Option<String>,

    #[serde(default)]
    pub default_model: Option<String>,

    /// Organization ID for OpenAI's multi-org API.
    #[serde(default)]
    pub organization: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OllamaConfig {
    /// Base URL for the Ollama server.
    #[serde(default = "default_ollama_url")]
    pub base_url: String,

    /// Default model for Ollama.
    #[serde(default)]
    pub default_model: Option<String>,
}

impl Default for OllamaConfig {
    fn default() -> Self {
        Self {
            base_url: default_ollama_url(),
            default_model: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct FallbackConfig {
    /// Enable automatic fallback when the primary provider fails.
    #[serde(default = "default_true")]
    pub enabled: bool,

    /// Ordered list of provider names for the fallback chain.
    #[serde(default)]
    pub chain: Vec<String>,

    /// Maximum retries per provider before falling back.
    #[serde(default = "default_retries")]
    pub max_retries: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CostConfig {
    /// Maximum cost per session in USD. None means no limit.
    #[serde(default)]
    pub session_budget_usd: Option<f64>,

    /// Warn when remaining budget drops below this threshold.
    #[serde(default = "default_warning_threshold")]
    pub warning_threshold_usd: f64,
}

fn default_provider() -> String { "anthropic".to_string() }
fn default_ollama_url() -> String { "http://localhost:11434".to_string() }
fn default_true() -> bool { true }
fn default_retries() -> u32 { 2 }
fn default_warning_threshold() -> f64 { 0.50 }
```

The schema uses `#[serde(default)]` extensively so that a minimal config file works. A user who only sets their API key gets sensible defaults for everything else.

## Loading Configuration

Configuration loading merges multiple sources:

```rust
use std::fs;
use std::path::Path;

impl ProviderConfig {
    /// Load configuration from all sources, with proper precedence.
    pub fn load() -> Result<Self, ConfigError> {
        // Start with defaults
        let mut config = ProviderConfig::default();

        // Layer 1: Global config file
        if let Some(global_path) = global_config_path() {
            if global_path.exists() {
                let global = Self::load_from_file(&global_path)?;
                config.merge(global);
            }
        }

        // Layer 2: Project-local config file
        let local_path = PathBuf::from(".agent/config.toml");
        if local_path.exists() {
            let local = Self::load_from_file(&local_path)?;
            config.merge(local);
        }

        // Layer 3: Environment variables (highest priority for secrets)
        config.apply_env_overrides();

        Ok(config)
    }

    fn load_from_file(path: &Path) -> Result<Self, ConfigError> {
        let content = fs::read_to_string(path)
            .map_err(|e| ConfigError::ReadError {
                path: path.to_path_buf(),
                source: e,
            })?;

        toml::from_str(&content)
            .map_err(|e| ConfigError::ParseError {
                path: path.to_path_buf(),
                source: e.to_string(),
            })
    }

    fn merge(&mut self, other: ProviderConfig) {
        // Only override fields that are explicitly set in the other config
        if other.default_provider != default_provider() {
            self.default_provider = other.default_provider;
        }

        for (k, v) in other.default_models {
            self.default_models.insert(k, v);
        }

        // Merge provider-specific configs
        if other.anthropic.api_key.is_some() {
            self.anthropic.api_key = other.anthropic.api_key;
        }
        if other.anthropic.base_url.is_some() {
            self.anthropic.base_url = other.anthropic.base_url;
        }
        if other.anthropic.default_model.is_some() {
            self.anthropic.default_model = other.anthropic.default_model;
        }

        if other.openai.api_key.is_some() {
            self.openai.api_key = other.openai.api_key;
        }
        if other.openai.base_url.is_some() {
            self.openai.base_url = other.openai.base_url;
        }
        if other.openai.default_model.is_some() {
            self.openai.default_model = other.openai.default_model;
        }
        if other.openai.organization.is_some() {
            self.openai.organization = other.openai.organization;
        }

        if other.ollama.default_model.is_some() {
            self.ollama.default_model = other.ollama.default_model;
        }
        if other.ollama.base_url != default_ollama_url() {
            self.ollama.base_url = other.ollama.base_url;
        }

        // Cost config
        if other.cost.session_budget_usd.is_some() {
            self.cost.session_budget_usd = other.cost.session_budget_usd;
        }
    }

    fn apply_env_overrides(&mut self) {
        if let Ok(key) = std::env::var("ANTHROPIC_API_KEY") {
            self.anthropic.api_key = Some(key);
        }
        if let Ok(key) = std::env::var("OPENAI_API_KEY") {
            self.openai.api_key = Some(key);
        }
        if let Ok(url) = std::env::var("ANTHROPIC_BASE_URL") {
            self.anthropic.base_url = Some(url);
        }
        if let Ok(url) = std::env::var("OPENAI_BASE_URL") {
            self.openai.base_url = Some(url);
        }
        if let Ok(url) = std::env::var("OLLAMA_BASE_URL") {
            self.ollama.base_url = url;
        }
        if let Ok(model) = std::env::var("AGENT_DEFAULT_MODEL") {
            // Parse "provider:model" format
            if let Some((provider, model)) = model.split_once(':') {
                self.default_provider = provider.to_string();
                self.default_models.insert(
                    provider.to_string(),
                    model.to_string(),
                );
            }
        }
        if let Ok(budget) = std::env::var("AGENT_SESSION_BUDGET") {
            if let Ok(usd) = budget.parse::<f64>() {
                self.cost.session_budget_usd = Some(usd);
            }
        }
    }
}

fn global_config_path() -> Option<PathBuf> {
    dirs::config_dir().map(|p| p.join("agent").join("config.toml"))
}

#[derive(Debug)]
pub enum ConfigError {
    ReadError { path: PathBuf, source: std::io::Error },
    ParseError { path: PathBuf, source: String },
}

impl std::fmt::Display for ConfigError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ConfigError::ReadError { path, source } => {
                write!(f, "Cannot read config file {:?}: {}", path, source)
            }
            ConfigError::ParseError { path, source } => {
                write!(f, "Cannot parse config file {:?}: {}", path, source)
            }
        }
    }
}
```

::: python Coming from Python
Python configuration often uses a combination of `os.environ`, `configparser` or `pydantic-settings`:
```python
from pydantic_settings import BaseSettings

class Config(BaseSettings):
    anthropic_api_key: str = ""
    openai_api_key: str = ""
    default_model: str = "claude-sonnet-4-20250514"

    class Config:
        env_prefix = "AGENT_"
```
Rust's `serde` provides similar automatic deserialization from TOML files, but you handle environment variable overlays manually. The upside is full control over precedence -- you decide exactly which source wins for each field.
:::

## Example Configuration Files

A minimal global config (`~/.config/agent/config.toml`):

```toml
default_provider = "anthropic"

[anthropic]
# API key should be set via ANTHROPIC_API_KEY env var, not here
default_model = "claude-sonnet-4-20250514"

[openai]
default_model = "gpt-4o"

[cost]
session_budget_usd = 5.00
warning_threshold_usd = 1.00
```

A project-local override (`.agent/config.toml`):

```toml
# This project uses GPT-4o for cost reasons
default_provider = "openai"

[openai]
default_model = "gpt-4o-mini"

[cost]
# Tighter budget for this project
session_budget_usd = 1.00

[fallback]
enabled = true
chain = ["openai", "ollama"]
```

## Secure API Key Management

API keys should never be stored in config files that might be committed to version control. The configuration system supports several approaches:

```rust
impl ProviderConfig {
    /// Resolve the API key for a provider, checking multiple sources.
    pub fn resolve_api_key(&self, provider: &str) -> Option<String> {
        match provider {
            "anthropic" => {
                // 1. Environment variable (highest priority)
                std::env::var("ANTHROPIC_API_KEY").ok()
                    // 2. Config file (fallback)
                    .or_else(|| self.anthropic.api_key.clone())
            }
            "openai" => {
                std::env::var("OPENAI_API_KEY").ok()
                    .or_else(|| self.openai.api_key.clone())
            }
            "ollama" => {
                // Ollama doesn't need an API key
                Some(String::new())
            }
            _ => None,
        }
    }

    /// Check which providers have valid API keys configured.
    pub fn available_providers(&self) -> Vec<String> {
        let mut providers = Vec::new();

        if self.resolve_api_key("anthropic").is_some() {
            providers.push("anthropic".to_string());
        }
        if self.resolve_api_key("openai").is_some() {
            providers.push("openai".to_string());
        }
        // Ollama is always "available" -- actual availability checked at runtime
        providers.push("ollama".to_string());

        providers
    }
}
```

For enhanced security, you could integrate with the system keychain. The `keyring` crate provides cross-platform keychain access:

```rust
/// Store an API key in the system keychain.
pub fn store_key_in_keychain(provider: &str, api_key: &str) -> Result<(), String> {
    let entry = keyring::Entry::new("coding-agent", provider)
        .map_err(|e| format!("Keychain error: {}", e))?;
    entry.set_password(api_key)
        .map_err(|e| format!("Failed to store key: {}", e))?;
    Ok(())
}

/// Retrieve an API key from the system keychain.
pub fn get_key_from_keychain(provider: &str) -> Option<String> {
    let entry = keyring::Entry::new("coding-agent", provider).ok()?;
    entry.get_password().ok()
}
```

## Building Providers from Config

The final piece connects configuration to provider construction:

```rust
use std::sync::Arc;
use crate::provider::{Provider, ProviderError};
use crate::provider::anthropic::AnthropicProvider;
use crate::provider::openai::OpenAIProvider;
use crate::provider::ollama::OllamaProvider;
use crate::provider::fallback::{FallbackChain, FallbackChainBuilder};

impl ProviderConfig {
    /// Build the primary provider based on configuration.
    pub fn build_provider(&self) -> Result<Arc<dyn Provider>, String> {
        let provider_name = &self.default_provider;
        self.build_named_provider(provider_name)
    }

    /// Build a specific provider by name.
    pub fn build_named_provider(&self, name: &str) -> Result<Arc<dyn Provider>, String> {
        match name {
            "anthropic" => {
                let api_key = self.resolve_api_key("anthropic")
                    .ok_or("Anthropic API key not configured. Set ANTHROPIC_API_KEY.")?;
                let model = self.anthropic.default_model.clone()
                    .or_else(|| self.default_models.get("anthropic").cloned())
                    .unwrap_or_else(|| "claude-sonnet-4-20250514".to_string());

                let mut provider = AnthropicProvider::new(api_key, model);
                if let Some(url) = &self.anthropic.base_url {
                    provider = AnthropicProvider::with_base_url(
                        self.resolve_api_key("anthropic").unwrap(),
                        self.anthropic.default_model.clone()
                            .unwrap_or_else(|| "claude-sonnet-4-20250514".to_string()),
                        url.clone(),
                    );
                }
                Ok(Arc::new(provider))
            }
            "openai" => {
                let api_key = self.resolve_api_key("openai")
                    .ok_or("OpenAI API key not configured. Set OPENAI_API_KEY.")?;
                let model = self.openai.default_model.clone()
                    .or_else(|| self.default_models.get("openai").cloned())
                    .unwrap_or_else(|| "gpt-4o".to_string());

                let provider = match &self.openai.base_url {
                    Some(url) => OpenAIProvider::with_base_url(
                        api_key, model, url.clone(),
                    ),
                    None => OpenAIProvider::new(api_key, model),
                };
                Ok(Arc::new(provider))
            }
            "ollama" => {
                let model = self.ollama.default_model.clone()
                    .or_else(|| self.default_models.get("ollama").cloned())
                    .unwrap_or_else(|| "llama3:latest".to_string());

                let provider = OllamaProvider::with_base_url(
                    model,
                    self.ollama.base_url.clone(),
                );
                Ok(Arc::new(provider))
            }
            other => Err(format!("Unknown provider: {}", other)),
        }
    }

    /// Build a fallback chain from configuration.
    pub fn build_fallback_chain(&self) -> Result<Arc<dyn Provider>, String> {
        if !self.fallback.enabled {
            return self.build_provider();
        }

        let chain_names = if self.fallback.chain.is_empty() {
            // Default chain: configured default, then others
            self.available_providers()
        } else {
            self.fallback.chain.clone()
        };

        let mut builder = FallbackChain::builder();
        for name in &chain_names {
            match self.build_named_provider(name) {
                Ok(provider) => {
                    builder = builder.add_with_retries(
                        provider,
                        self.fallback.max_retries,
                        500,
                    );
                }
                Err(_) => continue, // Skip unconfigured providers
            }
        }

        Ok(Arc::new(builder.build()))
    }
}
```

This is the entry point for the entire provider system. At agent startup, you call `ProviderConfig::load()` and then `build_fallback_chain()` to get a fully configured, resilient provider.

## Key Takeaways

- Configuration loads from multiple sources with clear precedence: CLI arguments override environment variables, which override project-local files, which override global files, which override built-in defaults
- API keys should live in environment variables or the system keychain, never in config files that might be committed to version control
- The TOML schema uses `#[serde(default)]` extensively so that a minimal config file with just an API key produces a fully functional configuration
- `build_fallback_chain()` is the top-level entry point that constructs the entire provider system from configuration, creating a resilient chain the agent can use like a single provider
- Per-project config files (`.agent/config.toml`) let teams enforce model choices and budget limits without affecting the user's global settings
