---
title: API Keys and Config
description: Securely manage API keys and application configuration without hardcoding secrets in source code.
---

# API Keys and Config

> **What you'll learn:**
> - Why API keys must never be committed to version control and how to prevent accidental exposure
> - How to load configuration from environment variables, dotenv files, and config files in priority order
> - How to structure a Config struct that centralizes all runtime configuration for your agent

Before you write a single line of HTTP code, you need to handle the most sensitive piece of the puzzle: your API key. Getting this wrong can be expensive -- literally. A leaked API key can be scraped by bots and used to rack up charges on your account. This subchapter shows you the right way to manage secrets in a Rust project.

## Getting Your API Key

First, you need an Anthropic API key. Head to [console.anthropic.com](https://console.anthropic.com), create an account if you have not already, and navigate to the API Keys section. Click "Create Key", give it a name like "cli-agent-dev", and copy the key. It looks something like this:

```
sk-ant-api03-xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx
```

This key is shown **exactly once**. If you lose it, you will need to create a new one. Copy it somewhere safe right now -- you will use it in a moment.

## The Cardinal Rule: Never Hardcode Secrets

This might seem obvious, but it is worth stating explicitly: **never put your API key directly in your source code.** Not even temporarily. Not even in a "test file." Here is what NOT to do:

```rust
// DO NOT DO THIS -- this key will end up in version control
const API_KEY: &str = "sk-ant-api03-real-key-here";
```

The moment you commit that file and push it to a public (or even private) repository, your key is compromised. GitHub actively scans for API key patterns and will notify Anthropic, who may revoke the key, but the window of exposure is still dangerous.

Instead, you will load the key from the environment at runtime.

## Environment Variables: The Standard Approach

The most common way to pass secrets to a program is through environment variables. You set them in your shell before running your program:

```bash
export ANTHROPIC_API_KEY="sk-ant-api03-xxxxx"
cargo run
```

Inside your Rust code, you read them with `std::env::var`:

```rust
use std::env;

fn main() {
    let api_key = env::var("ANTHROPIC_API_KEY")
        .expect("ANTHROPIC_API_KEY must be set");

    println!("Key loaded: {}...{}", &api_key[..10], &api_key[api_key.len()-4..]);
}
```

This program reads the `ANTHROPIC_API_KEY` environment variable. If it is not set, `env::var` returns `Err`, and `expect` panics with a descriptive message. If it is set, you get the key as a `String`.

The `println!` only shows the first 10 and last 4 characters of the key -- never log a full API key, even during development.

::: python Coming from Python
In Python, you would use `os.environ` or `os.getenv`:
```python
import os
api_key = os.environ["ANTHROPIC_API_KEY"]  # Raises KeyError if missing
# or
api_key = os.getenv("ANTHROPIC_API_KEY")   # Returns None if missing
```
Rust's `env::var` is equivalent to `os.environ.get()` -- it returns a `Result` that you must handle explicitly. There is no silent `None` return. This forced error handling is exactly what you want for a required secret: you find out immediately at startup if the key is missing, not halfway through a conversation.
:::

## Dotenv Files for Local Development

Typing `export ANTHROPIC_API_KEY=...` every time you open a new terminal gets old fast. The `dotenvy` crate (a maintained fork of the older `dotenv` crate) loads variables from a `.env` file into the process environment at startup.

First, add it to your `Cargo.toml`:

```toml
[dependencies]
dotenvy = "0.15"
```

Create a `.env` file in your project root:

```bash
# .env -- local development secrets (DO NOT COMMIT)
ANTHROPIC_API_KEY=sk-ant-api03-xxxxx
ANTHROPIC_MODEL=claude-sonnet-4-20250514
MAX_TOKENS=4096
```

Then load it early in your program:

```rust
use std::env;

fn main() {
    // Load .env file if it exists (silently ignore if missing)
    dotenvy::dotenv().ok();

    let api_key = env::var("ANTHROPIC_API_KEY")
        .expect("ANTHROPIC_API_KEY must be set");

    println!("API key loaded successfully");
}
```

The `dotenvy::dotenv().ok()` call reads the `.env` file and injects each line as an environment variable. The `.ok()` discards any error (like the file not existing), which is the right behavior -- in production, you set environment variables directly and do not use a `.env` file.

**Critical:** Add `.env` to your `.gitignore` immediately:

```gitignore
# .gitignore
.env
target/
```

This prevents accidental commits of your secrets.

## Building a Config Struct

As your agent grows, you will need more than just an API key. Model name, max tokens, temperature, base URL -- these all belong together. A `Config` struct centralizes them:

```rust
use std::env;

pub struct Config {
    pub api_key: String,
    pub model: String,
    pub max_tokens: u32,
    pub api_base_url: String,
}

impl Config {
    pub fn from_env() -> Result<Self, String> {
        let api_key = env::var("ANTHROPIC_API_KEY")
            .map_err(|_| "ANTHROPIC_API_KEY environment variable is not set. \
                          Get your key at https://console.anthropic.com".to_string())?;

        let model = env::var("ANTHROPIC_MODEL")
            .unwrap_or_else(|_| "claude-sonnet-4-20250514".to_string());

        let max_tokens = env::var("MAX_TOKENS")
            .unwrap_or_else(|_| "4096".to_string())
            .parse::<u32>()
            .map_err(|e| format!("MAX_TOKENS must be a valid integer: {e}"))?;

        let api_base_url = env::var("ANTHROPIC_API_BASE_URL")
            .unwrap_or_else(|_| "https://api.anthropic.com".to_string());

        Ok(Config {
            api_key,
            model,
            max_tokens,
            api_base_url,
        })
    }
}
```

Notice the pattern here: required variables (like `api_key`) return an error if missing, while optional variables (like `model`) fall back to sensible defaults with `unwrap_or_else`. The `MAX_TOKENS` parsing demonstrates how to validate and convert string environment variables to the types you actually need.

Using this in `main`:

```rust
fn main() {
    dotenvy::dotenv().ok();

    let config = Config::from_env().unwrap_or_else(|e| {
        eprintln!("Configuration error: {e}");
        std::process::exit(1);
    });

    println!("Connecting to {} with model {}", config.api_base_url, config.model);
}
```

The `unwrap_or_else` with `process::exit(1)` gives you a clean error message and a non-zero exit code, which is better than a panic backtrace for a configuration error.

## Preventing Accidental Key Exposure

Beyond `.gitignore`, here are additional safeguards:

**Git hooks.** A pre-commit hook can scan staged files for patterns that look like API keys:

```bash
#!/bin/bash
# .git/hooks/pre-commit
if git diff --cached --diff-filter=ACM | grep -q 'sk-ant-api'; then
    echo "ERROR: Possible API key detected in staged changes!"
    exit 1
fi
```

**Implement `Debug` carefully.** If you ever derive `Debug` on a struct containing secrets, the key will appear in debug output and logs. Override the `Debug` implementation to redact it:

```rust
use std::fmt;

pub struct Config {
    pub api_key: String,
    pub model: String,
    pub max_tokens: u32,
    pub api_base_url: String,
}

impl fmt::Debug for Config {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Config")
            .field("api_key", &"[REDACTED]")
            .field("model", &self.model)
            .field("max_tokens", &self.max_tokens)
            .field("api_base_url", &self.api_base_url)
            .finish()
    }
}
```

Now `println!("{:?}", config)` shows `[REDACTED]` instead of the actual key.

::: wild In the Wild
Claude Code reads the API key from the `ANTHROPIC_API_KEY` environment variable, exactly as you are doing here. It also supports configuration through a JSON config file at `~/.claude/config.json` for settings that persist across sessions. OpenCode similarly uses environment variables for secrets but stores non-sensitive configuration in a TOML file. The pattern of "secrets in env vars, settings in config files" is universal in production tools.
:::

## Key Takeaways

- Never hardcode API keys in source code. Load them from environment variables at runtime and add `.env` to `.gitignore`.
- Use `dotenvy` to load a `.env` file during development, falling back to actual environment variables in production.
- Centralize configuration in a `Config` struct with a `from_env()` constructor that validates required values and provides defaults for optional ones.
- Redact secrets in `Debug` implementations to prevent accidental logging of API keys.
- Fail fast at startup if required configuration is missing -- a clear error message at launch is far better than a cryptic failure mid-conversation.
