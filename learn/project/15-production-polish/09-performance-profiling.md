---
title: Performance Profiling
description: Profiling the agent to identify bottlenecks in startup time, tool execution, and response handling using flamegraphs, tracing spans, and benchmark suites.
---

# Performance Profiling

> **What you'll learn:**
> - How to use flamegraphs and profiling tools to identify CPU and memory bottlenecks in the agent
> - How to leverage tracing spans to measure latency of individual operations and tool calls
> - Techniques for writing benchmark suites with criterion that track performance over time

Your agent compiles and passes all tests. Users can install it with `brew install` or `cargo install`. But is it *fast*? A coding agent that takes 500ms to start or adds noticeable latency to every tool call will frustrate users -- especially those coming from snappy tools like `ripgrep` or `fd`. Performance profiling shows you where time is being spent, so you can optimize the right things instead of guessing.

## Measuring Startup Time

Startup time is the first thing users notice. Every time they run `agent`, they experience the delay between pressing Enter and seeing the first output. Measure it:

```bash
# Simple wall-clock measurement
time ./target/release/agent --version

# More precise measurement with hyperfine
cargo install hyperfine
hyperfine --warmup 3 './target/release/agent --version'
```

`hyperfine` runs the command multiple times and reports mean, min, max, and standard deviation. Aim for under 50ms for `--version` and under 200ms for interactive startup.

Common startup bottlenecks:
- Loading and parsing large config files
- Initializing the TLS stack (rustls or native-tls)
- Compiling regular expressions at startup
- Scanning the file system for project detection

## Flamegraphs with cargo-flamegraph

Flamegraphs give you a visual map of where CPU time is spent. Each bar represents a function, and its width shows relative time consumed.

```bash
# Install the flamegraph tool
cargo install flamegraph

# Generate a flamegraph (requires root on Linux, dtrace on macOS)
# Run your agent with a quick task and exit
cargo flamegraph --bin agent -- --non-interactive "say hello"
```

This produces `flamegraph.svg` that you can open in a browser. Look for wide bars at the bottom of the graph -- these are the functions consuming the most CPU time.

::: python Coming from Python
Python profiling tools like `cProfile` and `py-spy` serve a similar purpose. The key difference is what you are measuring. In Python, profiling often reveals that the interpreter overhead dominates, and the fix is often "rewrite in C" or "use a different algorithm." In Rust, the compiled code is already fast, so profiling typically reveals algorithmic issues, unnecessary allocations, or I/O bottlenecks. Flamegraphs are especially useful in Rust because they show the full call stack including library code, helping you identify if a dependency is the bottleneck.
:::

## Using tracing Spans for Latency Analysis

You already have tracing spans throughout your agent (from the structured logging subchapter). You can repurpose these spans for performance analysis by adding a timing layer:

```rust
use std::time::Instant;
use tracing::{info, span, Level, Subscriber};
use tracing_subscriber::layer::Context;
use tracing_subscriber::Layer;

/// A tracing layer that records span durations and logs slow operations.
pub struct PerformanceLayer {
    slow_threshold_ms: u64,
}

impl PerformanceLayer {
    pub fn new(slow_threshold_ms: u64) -> Self {
        Self { slow_threshold_ms }
    }
}

impl<S: Subscriber> Layer<S> for PerformanceLayer {
    fn on_enter(&self, id: &span::Id, ctx: Context<'_, S>) {
        if let Some(span) = ctx.span(id) {
            span.extensions_mut().insert(Instant::now());
        }
    }

    fn on_exit(&self, id: &span::Id, ctx: Context<'_, S>) {
        if let Some(span) = ctx.span(id) {
            if let Some(start) = span.extensions().get::<Instant>() {
                let elapsed = start.elapsed();
                let elapsed_ms = elapsed.as_millis() as u64;

                if elapsed_ms > self.slow_threshold_ms {
                    tracing::warn!(
                        span_name = span.name(),
                        elapsed_ms = elapsed_ms,
                        "Slow operation detected"
                    );
                }
            }
        }
    }
}
```

Register it in your logging setup:

```rust
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

pub fn init_logging_with_perf(verbose: bool) {
    let env_filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("agent=info"));

    let fmt_layer = tracing_subscriber::fmt::layer().pretty();

    // Flag operations taking longer than 100ms
    let perf_layer = PerformanceLayer::new(100);

    tracing_subscriber::registry()
        .with(env_filter)
        .with(fmt_layer)
        .with(perf_layer)
        .init();
}
```

Now every span that takes longer than 100ms automatically logs a warning. Run the agent and look for `Slow operation detected` messages to find your hotspots.

## Benchmarking with criterion

For targeted performance testing, the `criterion` crate provides statistical benchmarks that detect performance regressions across commits.

Add it to your `Cargo.toml`:

```toml
[dev-dependencies]
criterion = { version = "0.5", features = ["html_reports"] }

[[bench]]
name = "agent_benchmarks"
harness = false
```

Create `benches/agent_benchmarks.rs`:

```rust
use criterion::{black_box, criterion_group, criterion_main, Criterion};
use std::time::Duration;

fn bench_json_parsing(c: &mut Criterion) {
    let sample_response = r#"{
        "content": "Here is the fix for your code.",
        "tool_calls": [
            {
                "name": "write_file",
                "arguments": {
                    "path": "src/main.rs",
                    "content": "fn main() { println!(\"hello\"); }"
                }
            }
        ]
    }"#;

    c.bench_function("parse_llm_response", |b| {
        b.iter(|| {
            let value: serde_json::Value =
                serde_json::from_str(black_box(sample_response)).unwrap();
            black_box(value);
        })
    });
}

fn bench_config_loading(c: &mut Criterion) {
    let sample_config = r#"
        [provider]
        name = "anthropic"
        model = "claude-sonnet-4-20250514"
        max_tokens = 8192

        [tools]
        allowed_commands = ["cargo", "git", "npm"]
        command_timeout_secs = 30

        [logging]
        level = "info"
    "#;

    c.bench_function("parse_config_toml", |b| {
        b.iter(|| {
            let config: toml::Value = toml::from_str(black_box(sample_config)).unwrap();
            black_box(config);
        })
    });
}

fn bench_json_repair(c: &mut Criterion) {
    // Simulate a truncated LLM response
    let truncated = r#"{"content": "Here is some text", "tool_calls": [{"name": "read_file", "arguments": {"path": "src/main.rs"#;

    c.bench_function("repair_truncated_json", |b| {
        b.iter(|| {
            let repaired = repair_truncated_json(black_box(truncated));
            black_box(repaired);
        })
    });
}

fn repair_truncated_json(text: &str) -> String {
    let mut result = text.to_string();
    let mut open_braces = 0i32;
    let mut open_brackets = 0i32;
    let mut in_string = false;
    let mut prev_char = '\0';

    for ch in text.chars() {
        if ch == '"' && prev_char != '\\' {
            in_string = !in_string;
        }
        if !in_string {
            match ch {
                '{' => open_braces += 1,
                '}' => open_braces -= 1,
                '[' => open_brackets += 1,
                ']' => open_brackets -= 1,
                _ => {}
            }
        }
        prev_char = ch;
    }

    if in_string {
        result.push('"');
    }
    for _ in 0..open_brackets {
        result.push(']');
    }
    for _ in 0..open_braces {
        result.push('}');
    }

    result
}

criterion_group!(
    benches,
    bench_json_parsing,
    bench_config_loading,
    bench_json_repair,
);
criterion_main!(benches);
```

Run benchmarks:

```bash
# Run all benchmarks
cargo bench

# Run a specific benchmark
cargo bench -- parse_llm_response

# Compare against a baseline
cargo bench -- --save-baseline before-optimization
# ... make changes ...
cargo bench -- --baseline before-optimization
```

Criterion generates HTML reports in `target/criterion/` with graphs showing performance distributions and comparisons.

## Memory Profiling

Memory usage matters for a long-running interactive agent. Use the `dhat` crate for heap profiling:

```toml
[dev-dependencies]
dhat = "0.3"

[profile.release]
debug = 1  # Keep some debug info for profiling
```

```rust
#[cfg(feature = "dhat-heap")]
#[global_allocator]
static ALLOC: dhat::Alloc = dhat::Alloc;

fn main() {
    #[cfg(feature = "dhat-heap")]
    let _profiler = dhat::Profiler::new_heap();

    // ... normal agent code ...
}
```

Run with the profiler enabled:

```bash
cargo run --release --features dhat-heap -- --non-interactive "hello"
```

This generates `dhat-heap.json` which you can view in the [DHAT viewer](https://nnethercote.github.io/dh_view/dh_view.html) to see allocation patterns and identify memory-heavy operations.

::: wild In the Wild
Claude Code tracks latency metrics for every LLM call and tool execution, using the data to identify slow operations and optimize the user experience. OpenCode profiles startup time carefully because Go binaries are expected to start instantly. For Rust CLI tools, the `fd` project (a `find` alternative) maintains criterion benchmarks that run in CI, catching any commit that degrades search performance -- a pattern worth adopting for your agent's critical paths.
:::

## Practical Optimization Targets

After profiling, here are common optimizations for a coding agent:

```rust
// BEFORE: Compiling regex on every call
fn matches_pattern(text: &str) -> bool {
    let re = regex::Regex::new(r"```(\w+)\n").unwrap();
    re.is_match(text)
}

// AFTER: Compile regex once with lazy_static or std::sync::LazyLock
use std::sync::LazyLock;

static CODE_BLOCK_RE: LazyLock<regex::Regex> = LazyLock::new(|| {
    regex::Regex::new(r"```(\w+)\n").unwrap()
});

fn matches_pattern(text: &str) -> bool {
    CODE_BLOCK_RE.is_match(text)
}
```

```rust
// BEFORE: Cloning strings unnecessarily
fn process_response(response: String) -> String {
    let trimmed = response.clone().trim().to_string();
    trimmed
}

// AFTER: Work with references where possible
fn process_response(response: &str) -> &str {
    response.trim()
}
```

## Key Takeaways

- Measure startup time with `hyperfine` and target under 50ms for `--version` and under 200ms for interactive mode -- startup latency is the first thing users notice.
- Use `cargo flamegraph` to generate visual CPU profiles that reveal which functions consume the most time, including code deep in dependency crates.
- Build a custom tracing layer that warns about slow spans, turning your existing structured logging into an automatic performance monitoring system.
- Write criterion benchmarks for critical code paths (JSON parsing, config loading, response recovery) and run them in CI to catch performance regressions early.
- Profile memory with `dhat` to find allocation-heavy code paths, and optimize by compiling regexes once with `LazyLock` and reducing unnecessary string clones.
