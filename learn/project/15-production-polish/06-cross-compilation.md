---
title: Cross Compilation
description: Cross-compiling the agent for Linux x86_64, Linux aarch64, and macOS targets using cross, handling platform-specific dependencies and linking challenges.
---

# Cross Compilation

> **What you'll learn:**
> - How to set up cross-compilation toolchains for Linux and macOS targets from any host platform
> - How to handle platform-specific dependencies like OpenSSL and native TLS libraries
> - Techniques for CI-based cross-compilation that produces verified binaries for each release

Your users run Linux, macOS, and Windows on both x86_64 and ARM architectures. Building only for your development machine means leaving the majority of your potential users without a simple installation path. Cross-compilation lets you produce binaries for all target platforms from a single machine -- or, more practically, from a CI pipeline. Rust has excellent cross-compilation support, and with the right setup, you can target six or more platforms from one GitHub Actions workflow.

## Understanding Rust Targets

Rust identifies platforms with target triples: `<arch>-<vendor>-<os>-<env>`. Here are the targets that matter most for a CLI tool:

| Target Triple | Platform | Notes |
|---|---|---|
| `x86_64-unknown-linux-gnu` | Linux x86_64 (glibc) | Most common server/desktop Linux |
| `x86_64-unknown-linux-musl` | Linux x86_64 (static) | Fully static binary, works everywhere |
| `aarch64-unknown-linux-gnu` | Linux ARM64 | AWS Graviton, Raspberry Pi 4 |
| `aarch64-unknown-linux-musl` | Linux ARM64 (static) | Static binary for ARM Linux |
| `x86_64-apple-darwin` | macOS Intel | Older Macs |
| `aarch64-apple-darwin` | macOS Apple Silicon | M1/M2/M3/M4 Macs |
| `x86_64-pc-windows-msvc` | Windows x86_64 | Windows desktop |

Install additional targets with rustup:

```bash
# Add a target
rustup target add x86_64-unknown-linux-musl
rustup target add aarch64-unknown-linux-gnu

# List installed targets
rustup target list --installed
```

## The TLS Challenge

The biggest cross-compilation hurdle for HTTP-heavy applications like your agent is TLS. The `reqwest` crate (which you use for API calls) needs a TLS implementation. You have two options:

**Option 1: `rustls` (recommended for cross-compilation)**

`rustls` is a pure-Rust TLS implementation. It requires no system libraries, making cross-compilation trivial.

```toml
[dependencies]
reqwest = { version = "0.12", default-features = false, features = [
    "json",
    "stream",
    "rustls-tls",
] }
```

**Option 2: `native-tls`**

Uses the platform's native TLS library (OpenSSL on Linux, Security.framework on macOS, SChannel on Windows). Better compatibility with corporate proxies but harder to cross-compile.

```toml
[dependencies]
reqwest = { version = "0.12", features = ["json", "stream", "native-tls"] }
```

For a CLI tool distributed as a binary, `rustls` is the pragmatic choice. It eliminates a whole class of cross-compilation problems.

::: python Coming from Python
Python sidesteps cross-compilation entirely for most use cases -- you distribute source code (or wheels), and the target machine's Python interpreter handles platform differences. When you do need platform-specific Python packages (like those with C extensions), you encounter similar pain points: linking against system libraries, compatibility across glibc versions, and the manylinux standards. Rust's advantage is that once you get cross-compilation working, you produce a single static binary with zero runtime dependencies.
:::

## Using cross for Easy Cross-Compilation

The `cross` tool uses Docker containers pre-configured with the right toolchains for each target. It is a drop-in replacement for `cargo` that handles the cross-compilation environment automatically.

```bash
# Install cross
cargo install cross --git https://github.com/cross-rs/cross

# Build for Linux x86_64 (musl for static linking)
cross build --release --target x86_64-unknown-linux-musl

# Build for Linux ARM64
cross build --release --target aarch64-unknown-linux-gnu

# Run tests on the target platform (inside the Docker container)
cross test --release --target x86_64-unknown-linux-musl
```

For custom build requirements, create a `Cross.toml` in your project root:

```toml
[build.env]
passthrough = [
    "ANTHROPIC_API_KEY",
    "AGENT_VERSION",
]

[target.x86_64-unknown-linux-musl]
image = "ghcr.io/cross-rs/x86_64-unknown-linux-musl:main"

[target.aarch64-unknown-linux-gnu]
image = "ghcr.io/cross-rs/aarch64-unknown-linux-gnu:main"
```

The `passthrough` setting forwards environment variables into the Docker container, which is necessary if your build script needs API keys or version information.

## Building macOS Universal Binaries

If you are on macOS, you can build a universal binary that works on both Intel and Apple Silicon Macs:

```bash
# Install both targets
rustup target add x86_64-apple-darwin
rustup target add aarch64-apple-darwin

# Build for both architectures
cargo build --release --target x86_64-apple-darwin
cargo build --release --target aarch64-apple-darwin

# Combine into a universal binary with lipo
lipo -create \
    target/x86_64-apple-darwin/release/agent \
    target/aarch64-apple-darwin/release/agent \
    -output target/release/agent-universal
```

The universal binary is roughly twice the size of a single-architecture binary, but it provides a seamless experience for all macOS users.

## Platform-Specific Code

Sometimes you need platform-specific behavior. Use Rust's conditional compilation:

```rust
/// Get the default config directory for the current platform.
pub fn default_config_dir() -> std::path::PathBuf {
    #[cfg(target_os = "linux")]
    {
        if let Ok(xdg) = std::env::var("XDG_CONFIG_HOME") {
            return std::path::PathBuf::from(xdg).join("agent");
        }
        dirs::home_dir()
            .map(|h| h.join(".config").join("agent"))
            .unwrap_or_else(|| std::path::PathBuf::from("."))
    }

    #[cfg(target_os = "macos")]
    {
        dirs::home_dir()
            .map(|h| h.join("Library").join("Application Support").join("agent"))
            .unwrap_or_else(|| std::path::PathBuf::from("."))
    }

    #[cfg(target_os = "windows")]
    {
        dirs::config_dir()
            .map(|c| c.join("agent"))
            .unwrap_or_else(|| std::path::PathBuf::from("."))
    }
}

/// Get the appropriate shell for the current platform.
pub fn default_shell() -> &'static str {
    #[cfg(target_os = "windows")]
    {
        "cmd.exe"
    }

    #[cfg(not(target_os = "windows"))]
    {
        "/bin/sh"
    }
}
```

## Static Linking with musl

For maximum portability on Linux, use musl instead of glibc. The resulting binary has zero dynamic library dependencies and runs on any Linux distribution, regardless of glibc version.

```bash
# Build a fully static binary
cross build --release --target x86_64-unknown-linux-musl

# Verify it is statically linked
file target/x86_64-unknown-linux-musl/release/agent
# Output: ELF 64-bit LSB executable, x86-64, statically linked...

# Check dynamic dependencies (should show none)
ldd target/x86_64-unknown-linux-musl/release/agent
# Output: not a dynamic executable
```

Static musl binaries are ideal for Docker containers based on `scratch` or `alpine`:

```dockerfile
FROM scratch
COPY target/x86_64-unknown-linux-musl/release/agent /agent
ENTRYPOINT ["/agent"]
```

This produces a Docker image that contains only your binary -- no operating system, no shell, no libraries. The image size equals your binary size.

::: wild In the Wild
Claude Code distributes as an npm package that bundles platform-specific binaries, avoiding the cross-compilation problem by leveraging npm's platform detection. OpenCode, being written in Go, benefits from Go's built-in cross-compilation support (`GOOS=linux GOARCH=arm64 go build`). Rust's cross-compilation story requires more setup, but the resulting binaries are typically smaller and faster than Go equivalents, and the static musl approach eliminates the glibc version compatibility issues that plague many Linux tools.
:::

## Key Takeaways

- Use `rustls` instead of `native-tls` in reqwest to eliminate the most common cross-compilation blocker -- linking against platform-specific TLS libraries.
- Target the six major platforms (Linux x86_64, Linux ARM64, macOS Intel, macOS Apple Silicon, each in glibc and musl variants) to cover the vast majority of your user base.
- Use the `cross` tool for Docker-based cross-compilation that handles toolchains automatically -- it is a drop-in replacement for `cargo` that just works.
- Build musl-linked static binaries for Linux to produce executables with zero dynamic dependencies that run on any Linux distribution regardless of glibc version.
- Use `#[cfg(target_os = "...")]` for platform-specific code paths like config directory locations and default shell selection, keeping the rest of your codebase platform-agnostic.
