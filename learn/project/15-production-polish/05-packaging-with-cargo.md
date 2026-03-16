---
title: Packaging with Cargo
description: Preparing the agent for distribution through crates.io and cargo install, including Cargo.toml metadata, feature flags, and binary size optimization.
---

# Packaging with Cargo

> **What you'll learn:**
> - How to configure Cargo.toml metadata for publishing to crates.io including categories and keywords
> - How to use Cargo feature flags to make optional dependencies and provider support toggleable
> - Techniques for optimizing binary size with LTO, strip, and codegen-units settings

Your agent works. It has error recovery, structured logging, configuration management, and a proper CLI interface. Now it is time to package it so other people can install it. Rust's Cargo makes this remarkably straightforward -- `cargo install` is the Rust equivalent of `pip install`, and crates.io is the equivalent of PyPI. But getting the packaging right takes attention to metadata, feature flags, and binary optimization.

## Preparing Cargo.toml for Publishing

A publishable `Cargo.toml` needs more metadata than a development project. Here is a complete example:

```toml
[package]
name = "agent"
version = "0.1.0"
edition = "2021"
authors = ["Your Name <you@example.com>"]
description = "A CLI coding agent powered by LLMs"
license = "MIT"
repository = "https://github.com/yourusername/agent"
homepage = "https://github.com/yourusername/agent"
documentation = "https://docs.rs/agent"
readme = "README.md"
keywords = ["cli", "agent", "llm", "coding", "ai"]
categories = ["command-line-utilities", "development-tools"]
exclude = [
    "tests/fixtures/**",
    ".github/**",
    "docs/**",
    "*.excalidraw",
]

# Ensure the binary name is what users expect
[[bin]]
name = "agent"
path = "src/main.rs"

[dependencies]
tokio = { version = "1", features = ["full"] }
reqwest = { version = "0.12", features = ["json", "stream"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
clap = { version = "4", features = ["derive", "env"] }
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter", "json"] }
anyhow = "1"
toml = "0.8"
dirs = "5"
```

The `exclude` field keeps test fixtures, CI configuration, and documentation sources out of the published crate. This reduces download size and avoids including files that are irrelevant to installation.

::: python Coming from Python
In Python, you configure packaging metadata in `pyproject.toml` (or the older `setup.py`). Rust's `Cargo.toml` serves the same purpose but with stricter conventions. Where Python packages need a `MANIFEST.in` to control what goes into a distribution, Cargo uses the `exclude` and `include` fields. The `keywords` and `categories` fields in Cargo map to classifiers in PyPI, helping users discover your package.
:::

## Feature Flags for Optional Dependencies

Not every user needs every LLM provider. Feature flags let users compile only what they need, reducing binary size and avoiding unnecessary dependencies.

```toml
[features]
default = ["anthropic", "markdown-rendering"]

# LLM Providers
anthropic = ["dep:reqwest"]
openai = ["dep:reqwest"]
ollama = ["dep:reqwest"]

# UI features
markdown-rendering = ["dep:termimad"]
syntax-highlighting = ["dep:syntect"]

# All features for development
full = ["anthropic", "openai", "ollama", "markdown-rendering", "syntax-highlighting"]

[dependencies]
reqwest = { version = "0.12", features = ["json", "stream"], optional = true }
termimad = { version = "0.30", optional = true }
syntect = { version = "5", optional = true }
```

In your code, you conditionally compile provider support:

```rust
pub fn create_provider(name: &str, config: &ProviderConfig) -> Result<Box<dyn Provider>, AgentError> {
    match name {
        #[cfg(feature = "anthropic")]
        "anthropic" => Ok(Box::new(AnthropicProvider::new(config)?)),

        #[cfg(feature = "openai")]
        "openai" => Ok(Box::new(OpenAiProvider::new(config)?)),

        #[cfg(feature = "ollama")]
        "ollama" => Ok(Box::new(OllamaProvider::new(config)?)),

        other => {
            let available = available_providers();
            Err(AgentError::ConfigInvalid {
                field: "provider.name".to_string(),
                message: format!(
                    "Provider '{other}' is not available. Compiled providers: {}. \
                     Rebuild with the appropriate feature flag to enable other providers.",
                    available.join(", ")
                ),
            })
        }
    }
}

fn available_providers() -> Vec<&'static str> {
    let mut providers = Vec::new();
    #[cfg(feature = "anthropic")]
    providers.push("anthropic");
    #[cfg(feature = "openai")]
    providers.push("openai");
    #[cfg(feature = "ollama")]
    providers.push("ollama");
    providers
}
```

Users install with specific features:

```bash
# Default features (anthropic + markdown)
cargo install agent

# Only OpenAI support, no markdown rendering
cargo install agent --no-default-features --features openai

# Everything
cargo install agent --features full
```

::: wild In the Wild
Claude Code bundles platform-specific binaries within an npm package, sidestepping the crates.io distribution model entirely. OpenCode publishes to Go's module system. For Rust CLI tools, the `ripgrep` project is an excellent model -- it uses feature flags extensively to make optional features like PCRE2 regex support toggleable, and its `Cargo.toml` metadata is meticulously maintained for clean crates.io publishing.
:::

## Optimizing Binary Size

Rust binaries can be large. A debug build of a moderately complex application might be 50MB or more. Release builds are much smaller, but you can optimize further.

Add these settings to your `Cargo.toml`:

```toml
[profile.release]
# Link-Time Optimization: enables cross-crate optimizations
# "fat" is slower to compile but produces smaller, faster binaries
lto = "fat"

# Use a single codegen unit for maximum optimization
# (default is 16; 1 = slower compile, better optimization)
codegen_units = 1

# Strip debug symbols from the binary
strip = "symbols"

# Optimize for size with some speed tradeoff (optional)
# opt-level = "s"    # optimize for size
# opt-level = "z"    # aggressively optimize for size
opt-level = 3        # optimize for speed (default for release)

# Abort on panic instead of unwinding (smaller binary, no backtraces)
panic = "abort"
```

Here is the impact of each setting on a typical agent binary:

| Setting | Size Impact | Compile Time Impact |
|---------|-----------|-------------------|
| Default release | Baseline (~15MB) | Baseline |
| `lto = "fat"` | -20-30% | +50-100% |
| `codegen_units = 1` | -5-10% | +20-30% |
| `strip = "symbols"` | -30-50% | None |
| `panic = "abort"` | -5-10% | None |
| All combined | -50-70% (~5MB) | +100-150% |

For development builds, you want fast compile times, so leave these settings in the `[profile.release]` section only.

::: details How LTO works
Link-Time Optimization lets the compiler optimize across crate boundaries. Normally, each crate is compiled independently and linked together at the end. With LTO, the linker sees the entire program and can inline functions across crate boundaries, eliminate dead code more aggressively, and perform whole-program optimizations that are impossible with separate compilation.
:::

## Building for Release

The build command is straightforward:

```bash
# Standard release build
cargo build --release

# Build with specific features
cargo build --release --features full

# Check the binary size
ls -lh target/release/agent
```

You can further reduce size with `upx`, a binary packer:

```bash
# Install upx
brew install upx  # macOS
# or: apt install upx-ucl  # Ubuntu

# Compress the binary (typically 50-70% size reduction)
upx --best target/release/agent

# Verify it still works
./target/release/agent --version
```

## Adding a Build Script for Version Information

A build script can embed Git information into the binary at compile time:

```rust
// build.rs (in the project root, next to Cargo.toml)
use std::process::Command;

fn main() {
    // Embed the git commit hash
    let output = Command::new("git")
        .args(["rev-parse", "--short", "HEAD"])
        .output();

    if let Ok(output) = output {
        let git_hash = String::from_utf8_lossy(&output.stdout).trim().to_string();
        println!("cargo:rustc-env=GIT_HASH={git_hash}");
    } else {
        println!("cargo:rustc-env=GIT_HASH=unknown");
    }

    // Embed the build timestamp
    let now = chrono::Utc::now().format("%Y-%m-%d").to_string();
    println!("cargo:rustc-env=BUILD_DATE={now}");

    // Re-run if git HEAD changes
    println!("cargo:rerun-if-changed=.git/HEAD");
}
```

Use the embedded values in your CLI version output:

```rust
use clap::Parser;

#[derive(Parser)]
#[command(
    version = format!(
        "{} ({}; built {})",
        env!("CARGO_PKG_VERSION"),
        env!("GIT_HASH"),
        env!("BUILD_DATE")
    ),
)]
pub struct Cli {
    // ... fields
}
```

Now `agent --version` shows:

```
agent 0.1.0 (a3f2b1c; built 2026-03-16)
```

## Publishing to crates.io

Before publishing, verify everything is in order:

```bash
# Dry run: check what would be published
cargo publish --dry-run

# Check the package contents
cargo package --list

# Run all tests
cargo test

# Publish (requires a crates.io account and API token)
cargo publish
```

After publishing, anyone can install your agent with:

```bash
cargo install agent
```

## Key Takeaways

- Fill in all metadata fields in `Cargo.toml` (description, license, repository, keywords, categories) before publishing -- these are not optional for a professional package.
- Use feature flags to make provider support and optional UI features toggleable, letting users install only what they need and reducing binary size for minimal installations.
- Apply `lto = "fat"`, `codegen_units = 1`, `strip = "symbols"`, and `panic = "abort"` in `[profile.release]` to reduce binary size by 50-70% compared to a default release build.
- Use a `build.rs` script to embed Git commit hash and build date into the binary, making `--version` output useful for debugging and support.
- Always run `cargo publish --dry-run` before actually publishing to catch metadata issues, missing files, or accidental inclusion of large test fixtures.
