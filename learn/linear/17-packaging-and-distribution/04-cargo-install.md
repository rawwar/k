---
title: Cargo Install
description: Publish your agent to crates.io and optimize the cargo install experience for users who build from source.
---

# Cargo Install

> **What you'll learn:**
> - How to prepare your crate for publication on crates.io, including metadata, documentation, feature flags, and license configuration
> - Techniques for optimizing the cargo install build time by minimizing compile-time dependencies and using feature gates for optional components
> - How to handle the challenges of cargo install for end users, including build failures from missing system dependencies and long compile times

`cargo install` is Rust's equivalent of `pip install` for command-line tools. It downloads the source from crates.io, compiles it on the user's machine, and places the binary in `~/.cargo/bin/`. For Rust developers, this is the most natural installation method. Even for non-Rust developers, it provides a fallback when prebuilt binaries are not available for their platform. This subchapter covers how to prepare your crate for publication and make the `cargo install` experience as smooth as possible.

## Preparing Your Crate for Publication

Before publishing, your `Cargo.toml` needs complete metadata. Crates.io requires certain fields and uses others for the package listing page:

```toml
[package]
name = "my-agent"
version = "0.5.2"
edition = "2024"
authors = ["Your Name <you@example.com>"]
description = "A CLI coding agent powered by large language models"
readme = "README.md"
license = "MIT"
repository = "https://github.com/yourname/my-agent"
homepage = "https://github.com/yourname/my-agent"
keywords = ["cli", "agent", "llm", "coding", "ai"]
categories = ["command-line-utilities", "development-tools"]
exclude = [
    "tests/fixtures/*",
    "docs/*",
    ".github/*",
    "target/*",
]
```

Let's walk through the important fields:

- **`name`** -- Must be unique on crates.io. Check availability at [crates.io](https://crates.io/) before settling on a name.
- **`version`** -- Follow [Semantic Versioning](https://semver.org/). Crates.io rejects re-uploads of the same version, so every publish needs a version bump.
- **`license`** -- Use an [SPDX identifier](https://spdx.org/licenses/). `MIT`, `Apache-2.0`, or `MIT OR Apache-2.0` are the most common for Rust projects.
- **`description`** -- One sentence. This appears in search results on crates.io.
- **`keywords`** -- Up to 5. Help users find your crate through search.
- **`categories`** -- Choose from the [official list](https://crates.io/categories). `command-line-utilities` is the right fit for CLI agents.
- **`exclude`** -- Keeps test fixtures, docs, and CI configuration out of the published crate. This reduces download size for `cargo install` users.

::: python Coming from Python
This is similar to `pyproject.toml` metadata for PyPI publication, where you set `name`, `version`, `description`, `license`, and classifiers. The key difference: `cargo install` downloads source and compiles it, while `pip install` downloads precompiled wheels (or source distributions that may need compilation). There is no equivalent of Python's `wheel` format in Rust -- `cargo install` always builds from source.
:::

## Feature Flags for Optional Components

Feature flags let users opt into optional functionality. This is critical for `cargo install` because every dependency adds to the compile time. Users who do not need certain features should not pay the build cost:

```toml
[features]
default = ["git", "syntax-highlighting"]
git = ["dep:git2"]
syntax-highlighting = ["dep:syntect"]
telemetry = ["dep:opentelemetry", "dep:opentelemetry-otlp"]
self-update = ["dep:self_update"]

[dependencies]
git2 = { version = "0.19", features = ["vendored"], optional = true }
syntect = { version = "5", optional = true }
opentelemetry = { version = "0.27", optional = true }
opentelemetry-otlp = { version = "0.27", optional = true }
self_update = { version = "0.42", optional = true }
```

Users can then install with exactly the features they want:

```bash
# Install with default features
cargo install my-agent

# Install with all features
cargo install my-agent --all-features

# Install with minimal features (fastest build)
cargo install my-agent --no-default-features

# Install with specific features
cargo install my-agent --no-default-features --features "git,telemetry"
```

Design your features so that the `default` set covers what most users need while keeping build times reasonable. Heavy optional dependencies like `syntect` (which bundles grammar files and is slow to compile) should be behind a feature flag.

## Optimizing Build Time

`cargo install` compiles your crate from source on the user's machine. A clean build of a typical coding agent with its dependency tree can take 3-10 minutes depending on the machine. Here are techniques to reduce that:

### Minimize the Dependency Tree

Every dependency you add extends the `cargo install` time. Audit your dependencies regularly:

```bash
# Count your dependencies (including transitive)
cargo tree | wc -l

# Visualize the dependency tree
cargo tree --depth 2

# Find duplicate versions of the same crate
cargo tree --duplicates
```

Look for opportunities to replace heavy dependencies with lighter alternatives:

| Heavy Dependency | Lighter Alternative | Savings |
|-----------------|-------------------|---------|
| `chrono` | `time` or `jiff` | ~20 crates removed |
| `clap` (derive) | `clap` (builder) or `lexopt` | Faster proc-macro compile |
| `regex` | `memchr` (for simple patterns) | ~5 crates removed |
| `serde_json` (for simple cases) | `simd-json` or manual parsing | Marginal |

### Use Workspace Hack Crates (for Development)

If your agent is in a workspace with multiple crates, a "workspace hack" crate unifies feature flags across workspace members to avoid recompilation. The `cargo-hakari` tool automates this:

```bash
cargo install cargo-hakari
cargo hakari generate
cargo hakari manage-deps
```

This is primarily a development-time optimization rather than a `cargo install` optimization, but it matters if your published crate is part of a workspace.

### Conditional Compilation for Expensive Features

Gate expensive compile-time operations behind feature flags:

```rust
#[cfg(feature = "syntax-highlighting")]
mod syntax {
    use syntect::easy::HighlightLines;
    use syntect::parsing::SyntaxSet;
    use syntect::highlighting::ThemeSet;

    pub fn highlight_code(code: &str, language: &str) -> String {
        let ss = SyntaxSet::load_defaults_newlines();
        let ts = ThemeSet::load_defaults();
        let syntax = ss.find_syntax_by_token(language)
            .unwrap_or_else(|| ss.find_syntax_plain_text());
        let mut h = HighlightLines::new(syntax, &ts.themes["base16-ocean.dark"]);
        // ... highlighting logic
        code.to_string()
    }
}

#[cfg(not(feature = "syntax-highlighting"))]
mod syntax {
    pub fn highlight_code(code: &str, _language: &str) -> String {
        code.to_string()
    }
}
```

## The Publishing Workflow

Once your metadata is ready, publish with:

```bash
# Dry run -- checks everything without actually publishing
cargo publish --dry-run

# Review what will be included in the published crate
cargo package --list

# Publish to crates.io (requires authentication)
cargo login  # paste your API token from crates.io/settings/tokens
cargo publish
```

The `--dry-run` step is essential. It catches missing files, invalid metadata, and dependency issues before you make an irreversible publish. Remember: you cannot unpublish or overwrite a version on crates.io. You can only `yank` it (mark it as broken), which prevents new installations but does not remove it from existing lock files.

### Versioning Discipline

Follow a strict versioning workflow:

```bash
# Bump version in Cargo.toml (use cargo-edit for convenience)
cargo install cargo-edit
cargo set-version 0.5.3

# Or for a specific bump type:
cargo set-version --bump patch  # 0.5.2 -> 0.5.3
cargo set-version --bump minor  # 0.5.2 -> 0.6.0
cargo set-version --bump major  # 0.5.2 -> 1.0.0

# Commit, tag, push, publish
git add Cargo.toml Cargo.lock
git commit -m "release: v0.5.3"
git tag v0.5.3
git push && git push --tags
cargo publish
```

## Handling Build Failures

The biggest challenge with `cargo install` is that it requires the user to have a working Rust toolchain and any C compilation dependencies. Common failure modes:

| Failure | Cause | Solution |
|---------|-------|----------|
| "linker 'cc' not found" | No C compiler installed | User needs `build-essential` (Linux) or Xcode (macOS) |
| "failed to run custom build command for openssl-sys" | Missing OpenSSL headers | Switch to `rustls` or instruct user to install `libssl-dev` |
| "error: could not compile..." | Old Rust version | Specify `rust-version` in Cargo.toml |
| Timeout / killed | Insufficient RAM | Reduce `codegen-units`, disable LTO for install builds |

Mitigate these proactively:

```toml
[package]
# Specify minimum Rust version
rust-version = "1.80"
```

And in your README, provide clear installation instructions:

```markdown
## Installation

### Prebuilt binaries (recommended)
Download from [Releases](https://github.com/yourname/my-agent/releases)

### From source
Requires Rust 1.80+ and a C compiler:
```bash
cargo install my-agent
```
```

Always list prebuilt binaries as the primary installation method and `cargo install` as the fallback for Rust developers or unsupported platforms.

::: wild In the Wild
Most production coding agents do not rely on `cargo install` as their primary distribution method. It requires a Rust toolchain (which most users do not have), takes several minutes to compile, and can fail if system dependencies are missing. Production agents like Claude Code and similar tools distribute prebuilt binaries through Homebrew, GitHub Releases, or platform-specific package managers. However, `cargo install` remains valuable as a fallback and is the standard method for the Rust developer community.
:::

## Key Takeaways

- Complete your `Cargo.toml` metadata (description, license, repository, keywords, categories) before publishing to crates.io.
- Use feature flags to gate optional, heavy dependencies so users can minimize compile time with `--no-default-features`.
- Always run `cargo publish --dry-run` before a real publish -- crates.io versions are immutable once published.
- Specify `rust-version` in your package metadata so users with older toolchains get a clear error instead of a confusing compilation failure.
- Position `cargo install` as a secondary installation method behind prebuilt binaries -- it requires a Rust toolchain and can be slow.
