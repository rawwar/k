---
title: Release Builds
description: Configure Cargo release profiles for optimized, production-ready binaries with appropriate LTO, codegen units, and symbol stripping.
---

# Release Builds

> **What you'll learn:**
> - How to configure Cargo.toml release profiles with link-time optimization (LTO), codegen-units, and opt-level settings for maximum performance
> - The tradeoffs between build time, binary size, and runtime performance for different release profile configurations
> - How to strip debug symbols, embed version information, and set up reproducible builds for consistent release artifacts

Throughout development, you have been running `cargo run` and `cargo build`, which produce debug builds. Debug builds compile fast but produce large, slow binaries stuffed with debug symbols. Before you ship your coding agent to users, you need to produce a release build -- a binary that is optimized for speed and size, stripped of debug information, and tuned for the target machine. This subchapter walks through every knob Cargo gives you and explains when to turn each one.

## Debug vs. Release: The Baseline

By default, `cargo build` uses the `dev` profile. Adding `--release` switches to the `release` profile. The difference is dramatic:

```bash
# Debug build
cargo build
ls -lh target/debug/my-agent
# -rwxr-xr-x  1 user  staff   142M  my-agent

# Release build
cargo build --release
ls -lh target/release/my-agent
# -rwxr-xr-x  1 user  staff    12M  my-agent
```

That ten-fold size difference comes from two things: optimization level (the compiler inlines, eliminates dead code, and unrolls loops) and debug info (the debug build embeds symbol tables, line number mappings, and type metadata). The release build also runs significantly faster -- anywhere from 2x to 20x depending on the workload.

::: python Coming from Python
Python has no equivalent of a "release build." You ship the same `.py` files you develop with. Tools like PyInstaller or Nuitka can bundle Python into a standalone binary, but they embed an entire Python interpreter (often 30-50 MB) and rarely produce the same performance gains. Rust's release builds are a compiler-level optimization pass -- the output is native machine code with no interpreter overhead.
:::

## The Release Profile in Cargo.toml

Cargo's release profile is configured in your `Cargo.toml` under `[profile.release]`. Here is the production-grade configuration we will use for our coding agent:

```toml
[profile.release]
opt-level = 3          # Maximum optimization
lto = "fat"            # Full link-time optimization across all crates
codegen-units = 1      # Single codegen unit for best optimization
panic = "abort"        # Abort on panic instead of unwinding
strip = "symbols"      # Strip debug symbols from the binary
```

Let's break down each setting.

### Optimization Level (`opt-level`)

The `opt-level` controls how aggressively the compiler optimizes:

| Value | Meaning | Build Time | Runtime Performance |
|-------|---------|------------|---------------------|
| `0` | No optimization (debug default) | Fastest | Slowest |
| `1` | Basic optimizations | Fast | Moderate |
| `2` | Most optimizations | Moderate | Good |
| `3` | All optimizations including vectorization (release default) | Slow | Best |
| `"s"` | Optimize for binary size | Moderate | Good |
| `"z"` | Aggressively optimize for size | Moderate | Moderate |

For a coding agent, `opt-level = 3` is the right choice. The agent's performance-sensitive paths -- streaming token parsing, file I/O, diffing -- benefit from aggressive optimization. If you were building for an embedded system with tight flash storage, `"s"` or `"z"` would make sense, but for a CLI tool installed on a developer's machine, runtime speed matters more than a few megabytes.

### Link-Time Optimization (`lto`)

LTO lets the compiler optimize across crate boundaries. Without LTO, each crate is compiled independently and the linker just stitches them together. With LTO, the compiler can inline functions from your dependencies, eliminate dead code that spans crate boundaries, and perform whole-program optimization.

```toml
# Options for lto:
lto = false       # No LTO (default for dev)
lto = "thin"      # Parallel LTO - faster builds, most of the benefit
lto = "fat"       # Full LTO - slower builds, maximum optimization
lto = true        # Alias for "fat"
```

For release builds, `lto = "fat"` gives you the best results. The build takes longer -- sometimes 2-3x longer than `lto = "thin"` -- but the output binary is smaller and faster. For CI where build time matters, `lto = "thin"` is a reasonable compromise: it captures roughly 80% of the optimization benefit at half the build cost.

### Codegen Units (`codegen-units`)

The compiler splits each crate into multiple "codegen units" that can be compiled in parallel. The default for release builds is 16. More units means faster compilation but worse optimization, because the compiler cannot optimize across unit boundaries.

Setting `codegen-units = 1` forces the compiler to process each crate as a single unit. Combined with `lto = "fat"`, this gives the optimizer maximum visibility into your code. The tradeoff is build time -- expect release builds to take 2-4x longer than with the default setting.

### Panic Strategy (`panic`)

When Rust code panics, the default behavior is "unwinding" -- the runtime walks the stack, calling destructors along the way. This is safe and correct, but it adds code to the binary for the unwinding machinery.

```toml
panic = "abort"    # Terminate immediately on panic
panic = "unwind"   # Walk the stack, run destructors (default)
```

Setting `panic = "abort"` removes the unwinding code entirely. This reduces binary size by 5-10% and makes panics slightly faster (not that you want panics in production). The tradeoff: you cannot catch panics with `std::panic::catch_unwind`. For a CLI tool, this is almost always the right choice. You are not running untrusted plugins in-process that need panic isolation.

::: details What about catch_unwind?
If your agent uses `catch_unwind` to isolate third-party code (for example, running user-provided transform functions), you need `panic = "unwind"`. But for most CLI agents, panics represent bugs that should crash loudly. `panic = "abort"` produces smaller, simpler binaries.
:::

### Symbol Stripping (`strip`)

Debug symbols tell debuggers like `lldb` and `gdb` how to map machine code back to source lines. They are invaluable during development but serve no purpose in production binaries.

```toml
strip = "none"        # Keep all symbols
strip = "debuginfo"   # Remove debug info, keep symbol names
strip = "symbols"     # Remove everything (smallest binary)
```

With `strip = "symbols"`, our 12 MB release binary might shrink to 4-5 MB. The only downside: crash backtraces will show hex addresses instead of function names. For production crash reporting, you can keep a separate symbols file (a `.dSYM` bundle on macOS or a `.debug` file on Linux) without shipping it to users.

## Embedding Version Information

Users need to check which version they are running. Cargo makes this easy with built-in environment variables:

```rust
fn main() {
    let version = env!("CARGO_PKG_VERSION");
    let name = env!("CARGO_PKG_NAME");

    println!("{name} v{version}");
}
```

For richer version information that includes the git commit hash and build timestamp, use a build script. Add `vergen` to your build dependencies:

```toml
[build-dependencies]
vergen-gitcl = { version = "1", features = ["build", "cargo", "rustc"] }
```

Then create `build.rs` at your project root:

```rust
use vergen_gitcl::{BuildBuilder, CargoBuilder, Emitter, GitclBuilder, RustcBuilder};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let build = BuildBuilder::default().build_timestamp(true).build()?;
    let cargo = CargoBuilder::default().build()?;
    let gitcl = GitclBuilder::default()
        .sha(true)
        .dirty(true)
        .build()?;
    let rustc = RustcBuilder::default().semver(true).build()?;

    Emitter::default()
        .add_instructions(&build)?
        .add_instructions(&cargo)?
        .add_instructions(&gitcl)?
        .add_instructions(&rustc)?
        .emit()?;

    Ok(())
}
```

Now you can access detailed build metadata in your code:

```rust
fn version_info() -> String {
    format!(
        "{} v{} (commit: {}, built: {}, rustc: {})",
        env!("CARGO_PKG_NAME"),
        env!("CARGO_PKG_VERSION"),
        env!("VERGEN_GIT_SHA"),
        env!("VERGEN_BUILD_TIMESTAMP"),
        env!("VERGEN_RUSTC_SEMVER"),
    )
}
```

This gives users a precise version string like `my-agent v0.5.2 (commit: a1b2c3d, built: 2026-03-16T10:30:00Z, rustc: 1.85.0)` that is invaluable for bug reports.

## Custom Release Profiles

Sometimes you need multiple release configurations. Cargo supports custom profiles that inherit from the standard ones:

```toml
# Fast release build for CI testing (not for distribution)
[profile.ci]
inherits = "release"
lto = "thin"
codegen-units = 16

# Maximum optimization for distribution
[profile.dist]
inherits = "release"
lto = "fat"
codegen-units = 1
strip = "symbols"
```

Build with a custom profile using `--profile`:

```bash
# CI build: faster, still optimized
cargo build --profile ci

# Distribution build: maximum optimization
cargo build --profile dist
```

The output lands in `target/ci/` or `target/dist/` respectively, keeping it separate from the standard `target/release/` directory.

## Measuring the Impact

Before settling on a configuration, measure the differences. Here is a quick comparison script:

```bash
#!/bin/bash
set -euo pipefail

echo "=== Debug build ==="
time cargo build 2>&1 | tail -1
ls -lh target/debug/my-agent | awk '{print "Size:", $5}'

echo "=== Release (default) ==="
time cargo build --release 2>&1 | tail -1
ls -lh target/release/my-agent | awk '{print "Size:", $5}'

echo "=== Release (optimized) ==="
# Uses our custom [profile.release] with LTO + strip
cargo clean --release
time cargo build --release 2>&1 | tail -1
ls -lh target/release/my-agent | awk '{print "Size:", $5}'
```

On a typical coding agent project, you might see numbers like:

| Profile | Build Time | Binary Size | Startup Time |
|---------|------------|-------------|--------------|
| Debug | 15s | 142 MB | 45ms |
| Release (default) | 45s | 12 MB | 8ms |
| Release (optimized) | 3m 20s | 4.8 MB | 6ms |

The optimized release build takes significantly longer to compile, but the output binary is dramatically smaller and slightly faster. For CI, this build-time cost is a one-time price you pay on tagged releases, not on every commit.

::: wild In the Wild
Claude Code and similar production agents ship heavily optimized release builds. The binary needs to start quickly because developers expect sub-second startup when they invoke the tool from their terminal. Every millisecond of startup time matters for perceived quality, which is why production agents invest in aggressive LTO and stripping.
:::

## Key Takeaways

- Always build with `--release` (or a custom profile inheriting from it) before distributing your binary -- debug builds are 10-30x larger and significantly slower.
- The combination of `lto = "fat"`, `codegen-units = 1`, `panic = "abort"`, and `strip = "symbols"` produces the smallest, fastest release binary at the cost of longer build times.
- Use `lto = "thin"` for CI builds where you need release-level optimization without the full LTO build time penalty.
- Embed version information including the git commit hash so users can report exactly which build they are running.
- Create custom profiles (`[profile.ci]`, `[profile.dist]`) to separate quick CI builds from fully-optimized distribution builds.
