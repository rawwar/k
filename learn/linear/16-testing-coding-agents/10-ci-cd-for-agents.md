---
title: CI CD for Agents
description: Set up continuous integration and deployment pipelines that run the right tests at the right time, balancing coverage with API costs.
---

# CI CD for Agents

> **What you'll learn:**
> - How to structure CI pipelines with tiered test stages — fast deterministic tests on every push, recorded replay tests on PRs, and live API benchmarks on release
> - Techniques for managing API credentials and cost budgets in CI environments without leaking secrets or overspending
> - How to implement test result dashboards that track agent quality metrics, benchmark scores, and safety test coverage over time

A coding agent's test suite spans everything from millisecond unit tests to minute-long benchmark evaluations that call real APIs. Running all of them on every commit is wasteful and expensive. Running none of them is dangerous. The key is a tiered pipeline that runs the right tests at the right time.

This subchapter shows you how to set up GitHub Actions workflows that implement a multi-tier testing strategy, manage API credentials safely, control costs, and track quality metrics over time.

## The Three-Tier Pipeline

Your CI pipeline should have three tiers, each triggered by a different event:

**Tier 1: Every push.** Fast, deterministic tests that catch code-level bugs. These run in seconds and never call external APIs.
- Unit tests for all tools
- Safety tests (path traversal, command injection)
- Input validation tests
- Parsing and formatting tests
- Property-based tests
- Linting and formatting checks

**Tier 2: Every pull request.** Replay tests and integration tests that verify the agentic loop works end-to-end. These use recorded fixtures, not live APIs.
- Integration tests with mock providers
- Replay tests using recorded fixtures
- Snapshot tests
- Compilation on all target platforms

**Tier 3: Scheduled or release.** Benchmark evaluations against the live API. These run on a schedule (nightly or weekly) or when tagging a release.
- Full benchmark suite against real API
- Performance regression detection
- Token usage tracking

Here is the GitHub Actions workflow that implements this:

```yaml
# .github/workflows/ci.yml
name: CI

on:
  push:
    branches: [main, "feature/**"]
  pull_request:
    branches: [main]

jobs:
  # Tier 1: Runs on every push
  unit-tests:
    name: Unit Tests
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
      - uses: Swatinem/rust-cache@v2

      - name: Run unit tests
        run: cargo test --lib

      - name: Run safety tests
        run: cargo test --lib safety

      - name: Check formatting
        run: cargo fmt -- --check

      - name: Run clippy
        run: cargo clippy -- -D warnings

  # Tier 2: Runs on PRs
  integration-tests:
    name: Integration Tests
    runs-on: ubuntu-latest
    needs: unit-tests
    if: github.event_name == 'pull_request'
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
      - uses: Swatinem/rust-cache@v2

      - name: Run integration tests
        run: cargo test --test '*'

      - name: Run snapshot tests
        run: cargo test snapshot

  # Tier 3: Scheduled benchmarks
  benchmarks:
    name: Benchmark Evaluation
    runs-on: ubuntu-latest
    if: github.event_name == 'schedule' || startsWith(github.ref, 'refs/tags/')
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
      - uses: Swatinem/rust-cache@v2

      - name: Run benchmarks
        env:
          ANTHROPIC_API_KEY: ${{ secrets.ANTHROPIC_API_KEY }}
        run: cargo test --test benchmarks -- --ignored

      - name: Upload benchmark results
        uses: actions/upload-artifact@v4
        with:
          name: benchmark-results
          path: benchmark-results/
```

::: python Coming from Python
Python CI pipelines use tox or nox for multi-environment testing:
```yaml
# Python equivalent
- name: Run unit tests
  run: pytest -m "not integration and not benchmark" --timeout=30
- name: Run integration tests
  run: pytest -m integration --timeout=120
- name: Run benchmarks
  run: pytest -m benchmark --timeout=600
```
Rust achieves the same tiering through different mechanisms: `--lib` runs tests inside source files, `--test` runs files in `tests/`, and `--ignored` runs tests marked with `#[ignore]`. The key difference is that Rust compiles test binaries separately, so you can also control tiers at the compilation level with feature flags.
:::

## Managing API Credentials

Your benchmark tests need API keys, but those keys must never appear in code, logs, or error messages. GitHub Actions secrets are the standard approach:

```yaml
# In your repository settings, add:
# ANTHROPIC_API_KEY = sk-ant-...

# In the workflow, only expose to jobs that need it:
benchmarks:
  env:
    ANTHROPIC_API_KEY: ${{ secrets.ANTHROPIC_API_KEY }}
```

In your Rust code, load the key from the environment and fail clearly if it is missing:

```rust
pub fn load_api_key() -> Result<String, ConfigError> {
    std::env::var("ANTHROPIC_API_KEY").map_err(|_| {
        ConfigError::MissingKey(
            "ANTHROPIC_API_KEY not set. \
             Benchmark tests require a valid API key."
                .to_string(),
        )
    })
}

#[derive(Debug)]
pub enum ConfigError {
    MissingKey(String),
}

// In your benchmark test:
#[test]
#[ignore] // Only run when explicitly requested
fn benchmark_requires_api_key() {
    let key = load_api_key().expect(
        "Set ANTHROPIC_API_KEY to run benchmarks. \
         This test is skipped in normal CI."
    );
    assert!(!key.is_empty());
}
```

## Controlling API Costs

Benchmark tests cost real money. Set up guardrails to prevent runaway spending:

```rust
pub struct CostTracker {
    pub max_input_tokens: u64,
    pub max_output_tokens: u64,
    pub current_input_tokens: u64,
    pub current_output_tokens: u64,
}

impl CostTracker {
    pub fn new(max_input: u64, max_output: u64) -> Self {
        Self {
            max_input_tokens: max_input,
            max_output_tokens: max_output,
            current_input_tokens: 0,
            current_output_tokens: 0,
        }
    }

    pub fn record_usage(&mut self, input: u64, output: u64) -> Result<(), CostError> {
        self.current_input_tokens += input;
        self.current_output_tokens += output;

        if self.current_input_tokens > self.max_input_tokens {
            return Err(CostError::BudgetExceeded {
                kind: "input",
                used: self.current_input_tokens,
                limit: self.max_input_tokens,
            });
        }
        if self.current_output_tokens > self.max_output_tokens {
            return Err(CostError::BudgetExceeded {
                kind: "output",
                used: self.current_output_tokens,
                limit: self.max_output_tokens,
            });
        }
        Ok(())
    }

    pub fn estimated_cost_usd(&self) -> f64 {
        // Rough estimate for Claude Sonnet pricing
        let input_cost = self.current_input_tokens as f64 * 3.0 / 1_000_000.0;
        let output_cost = self.current_output_tokens as f64 * 15.0 / 1_000_000.0;
        input_cost + output_cost
    }
}

#[derive(Debug)]
pub enum CostError {
    BudgetExceeded {
        kind: &'static str,
        used: u64,
        limit: u64,
    },
}

#[cfg(test)]
mod cost_tests {
    use super::*;

    #[test]
    fn enforces_token_budget() {
        let mut tracker = CostTracker::new(1000, 500);
        assert!(tracker.record_usage(800, 200).is_ok());
        assert!(tracker.record_usage(300, 100).is_err()); // Exceeds input limit
    }

    #[test]
    fn tracks_estimated_cost() {
        let mut tracker = CostTracker::new(1_000_000, 1_000_000);
        tracker.record_usage(100_000, 10_000).unwrap();
        let cost = tracker.estimated_cost_usd();
        assert!(cost > 0.0);
        assert!(cost < 1.0); // Sanity check
    }
}
```

In your CI workflow, set a maximum spend per run:

```yaml
- name: Run benchmarks with cost limit
  env:
    ANTHROPIC_API_KEY: ${{ secrets.ANTHROPIC_API_KEY }}
    BENCHMARK_MAX_INPUT_TOKENS: "500000"
    BENCHMARK_MAX_OUTPUT_TOKENS: "100000"
  run: cargo test --test benchmarks -- --ignored
```

## Using Feature Flags for Test Tiers

Rust's feature flags let you conditionally compile test code, which is useful for separating test tiers:

```toml
# Cargo.toml
[features]
default = []
benchmark-tests = ["dep:reqwest", "dep:chrono"]

[dev-dependencies]
reqwest = { version = "0.12", optional = true }
chrono = { version = "0.4", optional = true }
```

```rust
// tests/benchmarks.rs
#![cfg(feature = "benchmark-tests")]

#[tokio::test]
#[ignore]
async fn benchmark_fix_bug() {
    // This entire file is only compiled when benchmark-tests is enabled
    let api_key = std::env::var("ANTHROPIC_API_KEY").unwrap();
    // ... run benchmark
}
```

```yaml
# Only compile and run benchmarks when the feature is enabled
- name: Run benchmarks
  run: cargo test --test benchmarks --features benchmark-tests -- --ignored
```

## Tracking Quality Metrics

Store benchmark results as CI artifacts and build a simple dashboard:

```rust
use serde::Serialize;

#[derive(Serialize)]
pub struct CiReport {
    pub commit: String,
    pub branch: String,
    pub timestamp: String,
    pub unit_tests: TestTierResult,
    pub integration_tests: TestTierResult,
    pub safety_tests: TestTierResult,
    pub benchmarks: Option<BenchmarkSummary>,
}

#[derive(Serialize)]
pub struct TestTierResult {
    pub total: usize,
    pub passed: usize,
    pub failed: usize,
    pub duration_secs: f64,
}

pub fn save_ci_report(report: &CiReport) -> std::io::Result<()> {
    let dir = std::path::Path::new("benchmark-results");
    std::fs::create_dir_all(dir)?;
    let filename = format!("{}.json", report.commit);
    let path = dir.join(filename);
    let json = serde_json::to_string_pretty(report)?;
    std::fs::write(path, json)?;
    Ok(())
}
```

::: wild In the Wild
Claude Code's CI pipeline runs unit and safety tests on every push, integration tests on every PR, and a full evaluation suite on a nightly schedule. The evaluation results are tracked in a dashboard that shows pass rates, token usage trends, and performance metrics over time. When a benchmark regression is detected, it triggers an alert for the team to investigate before the next release.
:::

## Caching for Faster CI

Rust's compilation is slow on first build but fast on incremental builds. Use caching to keep CI times reasonable:

```yaml
# The rust-cache action caches target/ and the cargo registry
- uses: Swatinem/rust-cache@v2
  with:
    # Cache key includes Cargo.lock hash
    key: ${{ runner.os }}-cargo-${{ hashFiles('**/Cargo.lock') }}
```

For replay tests, cache the fixture files as well:

```yaml
- name: Cache replay fixtures
  uses: actions/cache@v4
  with:
    path: tests/fixtures/
    key: replay-fixtures-${{ hashFiles('tests/fixtures/**') }}
```

## Key Takeaways

- Structure CI into three tiers: fast unit and safety tests on every push, integration and replay tests on PRs, and live benchmark evaluations on schedule or release
- Manage API keys through CI secrets and never expose them in logs, code, or error messages — only benchmark jobs need API access
- Implement token budgets and cost tracking to prevent runaway API spending during benchmark runs
- Use Rust feature flags and `#[ignore]` to control which tests compile and run at each tier, keeping the fast path fast
- Track benchmark results as CI artifacts over time to detect quality regressions before they reach users
