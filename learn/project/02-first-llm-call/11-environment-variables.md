---
title: Environment Variables
description: Load and validate environment variables at startup to configure API keys, model selection, and runtime behavior.
---

# Environment Variables

> **What you'll learn:**
> - How to read environment variables with `std::env::var` and provide sensible defaults for optional ones
> - How to use the dotenvy crate to load variables from a `.env` file during local development
> - How to validate required variables at startup and fail fast with clear error messages if they are missing

You touched on environment variables in the API Keys subchapter. Now let's go deeper and build a proper configuration system for your agent. By the end of this subchapter, your agent will load all its settings from the environment, validate them at startup, and provide clear error messages when something is missing or invalid.

## Why Environment Variables?

Environment variables are the standard way to configure server-side and CLI applications. They have several advantages:

- **Security.** Secrets stay out of source code and version control.
- **Flexibility.** The same binary can run with different configurations by changing environment variables.
- **Convention.** Tools like Docker, CI systems, and cloud platforms all support setting environment variables natively.
- **Simplicity.** No config file format to parse, no file path to resolve, no file permissions to manage.

The twelve-factor app methodology (a widely adopted set of best practices for building software-as-a-service apps) explicitly recommends storing configuration in environment variables. Your CLI agent is not a SaaS app, but the principle applies: separate configuration from code.

## Reading Environment Variables in Rust

The `std::env` module provides the core functions:

```rust
use std::env;

fn main() {
    // Read a variable (returns Result<String, VarError>)
    match env::var("ANTHROPIC_API_KEY") {
        Ok(key) => println!("Key found: {}...", &key[..10]),
        Err(env::VarError::NotPresent) => println!("Variable not set"),
        Err(env::VarError::NotUnicode(_)) => println!("Variable is not valid UTF-8"),
    }

    // Read with a default value
    let model = env::var("ANTHROPIC_MODEL")
        .unwrap_or_else(|_| "claude-sonnet-4-20250514".to_string());

    println!("Using model: {model}");
}
```

`env::var` returns a `Result<String, VarError>`. The error type has two variants: `NotPresent` (the variable does not exist) and `NotUnicode` (the variable exists but contains non-UTF-8 bytes, which is rare on modern systems).

::: python Coming from Python
Python's `os.environ` is a dict-like object:
```python
import os

# Raises KeyError if not set
key = os.environ["ANTHROPIC_API_KEY"]

# Returns None or a default if not set
model = os.getenv("ANTHROPIC_MODEL", "claude-sonnet-4-20250514")
```
The Rust equivalent of `os.getenv` with a default is `env::var(...).unwrap_or_else(|_| default)`. The verbose syntax is intentional -- Rust makes you explicitly handle the "not set" case rather than silently returning a default.
:::

## The dotenvy Crate

For local development, typing environment variables every time you open a terminal is tedious. The `dotenvy` crate loads variables from a `.env` file in the current directory:

```toml
[dependencies]
dotenvy = "0.15"
```

Create a `.env` file in your project root:

```bash
# .env — local development only (add to .gitignore!)
ANTHROPIC_API_KEY=sk-ant-api03-xxxxx
ANTHROPIC_MODEL=claude-sonnet-4-20250514
MAX_TOKENS=4096
LOG_LEVEL=debug
```

Load it at the very start of your program:

```rust
fn main() {
    // Load .env file if present. In production, env vars are set directly.
    dotenvy::dotenv().ok();

    // Now env::var will find variables from both .env and the actual environment
    let api_key = std::env::var("ANTHROPIC_API_KEY")
        .expect("ANTHROPIC_API_KEY must be set");
}
```

The `.ok()` call discards the error if the file does not exist. This is correct behavior -- the `.env` file is only for local development. In production or CI, the variables are set through the deployment platform.

**Important precedence rule:** If a variable is set in both the actual environment and the `.env` file, `dotenvy` preserves the existing environment variable. The `.env` file only fills in variables that are not already set. This means you can override `.env` values from the shell:

```bash
ANTHROPIC_MODEL=claude-opus-4-20250514 cargo run
# Uses claude-opus-4-20250514 even if .env says claude-sonnet-4-20250514
```

## Building a Complete Config Struct

Let's build the definitive `Config` struct for your agent. This centralizes all configuration, validates it at startup, and provides defaults for optional values:

```rust
use std::env;
use std::fmt;

/// All runtime configuration for the CLI agent.
pub struct Config {
    /// Anthropic API key (required)
    pub api_key: String,
    /// Model identifier (default: claude-sonnet-4-20250514)
    pub model: String,
    /// Maximum tokens in each response (default: 4096)
    pub max_tokens: u32,
    /// API base URL (default: https://api.anthropic.com)
    pub api_base_url: String,
    /// API version string (default: 2023-06-01)
    pub api_version: String,
}

impl Config {
    /// Load configuration from environment variables.
    /// Returns a descriptive error if required variables are missing.
    pub fn from_env() -> Result<Self, ConfigError> {
        let api_key = env::var("ANTHROPIC_API_KEY").map_err(|_| {
            ConfigError::MissingRequired {
                name: "ANTHROPIC_API_KEY".to_string(),
                help: "Get your API key at https://console.anthropic.com \
                       and set it with: export ANTHROPIC_API_KEY=sk-ant-..."
                    .to_string(),
            }
        })?;

        let model = env::var("ANTHROPIC_MODEL")
            .unwrap_or_else(|_| "claude-sonnet-4-20250514".to_string());

        let max_tokens = match env::var("MAX_TOKENS") {
            Ok(val) => val.parse::<u32>().map_err(|e| ConfigError::InvalidValue {
                name: "MAX_TOKENS".to_string(),
                value: val,
                reason: format!("must be a positive integer: {e}"),
            })?,
            Err(_) => 4096,
        };

        let api_base_url = env::var("ANTHROPIC_API_BASE_URL")
            .unwrap_or_else(|_| "https://api.anthropic.com".to_string());

        let api_version = env::var("ANTHROPIC_API_VERSION")
            .unwrap_or_else(|_| "2023-06-01".to_string());

        Ok(Config {
            api_key,
            model,
            max_tokens,
            api_base_url,
            api_version,
        })
    }

    /// Build the full Messages API URL from the base URL.
    pub fn messages_url(&self) -> String {
        format!("{}/v1/messages", self.api_base_url)
    }
}

/// Errors that can occur during configuration loading.
#[derive(Debug)]
pub enum ConfigError {
    MissingRequired { name: String, help: String },
    InvalidValue { name: String, value: String, reason: String },
}

impl fmt::Display for ConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ConfigError::MissingRequired { name, help } => {
                write!(f, "Missing required environment variable: {name}\n  {help}")
            }
            ConfigError::InvalidValue { name, value, reason } => {
                write!(f, "Invalid value for {name}={value}: {reason}")
            }
        }
    }
}

impl std::error::Error for ConfigError {}

// Redact the API key in Debug output
impl fmt::Debug for Config {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Config")
            .field("api_key", &"[REDACTED]")
            .field("model", &self.model)
            .field("max_tokens", &self.max_tokens)
            .field("api_base_url", &self.api_base_url)
            .field("api_version", &self.api_version)
            .finish()
    }
}
```

The `ConfigError` enum provides two distinct error cases with specific guidance. When `ANTHROPIC_API_KEY` is missing, the error tells the user exactly where to get a key and how to set it. When `MAX_TOKENS` has an invalid value, the error shows what value was provided and why it is wrong.

## Using Config in main

Here is how to use the `Config` struct to initialize your agent:

```rust
use reqwest::header::{HeaderMap, HeaderValue};

fn build_client(config: &Config) -> Result<reqwest::Client, reqwest::Error> {
    let mut headers = HeaderMap::new();
    headers.insert(
        "x-api-key",
        HeaderValue::from_str(&config.api_key).unwrap(),
    );
    headers.insert(
        "anthropic-version",
        HeaderValue::from_str(&config.api_version).unwrap(),
    );

    reqwest::Client::builder()
        .default_headers(headers)
        .timeout(std::time::Duration::from_secs(60))
        .build()
}

#[tokio::main]
async fn main() {
    dotenvy::dotenv().ok();

    let config = match Config::from_env() {
        Ok(config) => config,
        Err(e) => {
            eprintln!("Configuration error:\n  {e}");
            std::process::exit(1);
        }
    };

    println!("Configuration loaded: {:?}", config);
    // Output: Config { api_key: "[REDACTED]", model: "claude-sonnet-4-20250514", ... }

    let client = build_client(&config).expect("Failed to build HTTP client");

    println!("Ready to connect to {}", config.messages_url());
    // Output: Ready to connect to https://api.anthropic.com/v1/messages
}
```

The `match` on `Config::from_env()` gives you a clean error message and a non-zero exit code on failure. This is better than `expect()` or `unwrap()`, which produce a panic backtrace that is confusing for end users.

## Listing All Expected Variables

It is good practice to document all the environment variables your program reads. Add a help function that lists them:

```rust
fn print_env_help() {
    println!("Environment Variables:");
    println!();
    println!("  ANTHROPIC_API_KEY      (required) Your Anthropic API key");
    println!("  ANTHROPIC_MODEL        (optional) Model ID [default: claude-sonnet-4-20250514]");
    println!("  MAX_TOKENS             (optional) Max response tokens [default: 4096]");
    println!("  ANTHROPIC_API_BASE_URL (optional) API base URL [default: https://api.anthropic.com]");
    println!("  ANTHROPIC_API_VERSION  (optional) API version [default: 2023-06-01]");
}
```

You can wire this into a `--help` flag or a `/config` REPL command so users can discover what settings are available.

::: wild In the Wild
Claude Code reads a substantial number of environment variables: `ANTHROPIC_API_KEY` for authentication, `CLAUDE_MODEL` for model selection, and various others for proxy configuration, debug logging, and feature flags. It also reads from a JSON configuration file at `~/.claude/config.json` for persistent settings. The environment variables always take precedence over the config file, following the same override pattern you see with `dotenvy`.
:::

## Key Takeaways

- Use `std::env::var` to read environment variables, providing defaults with `unwrap_or_else` for optional ones and returning errors for required ones.
- The `dotenvy` crate loads a `.env` file for local development without affecting production, where variables are set directly by the deployment platform.
- Centralize all configuration in a `Config` struct with a `from_env()` constructor that validates values and provides actionable error messages when configuration is wrong.
- Always redact secrets in `Debug` implementations and never log full API keys, even during development.
- Fail fast at startup: validate all required configuration before entering the REPL loop, so the user finds out immediately if something is missing.
