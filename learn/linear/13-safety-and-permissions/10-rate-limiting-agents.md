---
title: Rate Limiting Agents
description: Implement rate limiting and circuit breaker patterns that prevent runaway agents from consuming excessive resources or causing repeated damage.
---

# Rate Limiting Agents

> **What you'll learn:**
> - How to design token bucket and sliding window rate limiters scoped to tool invocations, API calls, and file modifications
> - When and how to implement circuit breakers that halt agent execution after detecting repeated failures or suspicious patterns
> - Strategies for setting rate limits that balance agent productivity with safety, including adaptive limits based on trust level

An agent running in a loop can generate an enormous number of actions in a short time. Without rate limiting, a confused agent might rewrite the same file hundreds of times, spawn thousands of processes, or make so many API calls that it exhausts your token budget in minutes. Rate limiting acts as a governor -- it does not care whether individual actions are permitted; it ensures the overall pace and volume of actions stays within safe bounds.

## Token Bucket Rate Limiter

The token bucket algorithm is the most widely used rate limiting approach. The bucket starts with a fixed number of tokens. Each action consumes one token. Tokens are refilled at a steady rate. When the bucket is empty, actions are blocked until tokens regenerate:

```rust
use std::time::Instant;

/// A token bucket rate limiter.
struct TokenBucket {
    /// Maximum tokens the bucket can hold
    capacity: u32,
    /// Current number of available tokens
    tokens: f64,
    /// Rate at which tokens are added (tokens per second)
    refill_rate: f64,
    /// When tokens were last calculated
    last_refill: Instant,
    /// Name of what this bucket rate-limits (for logging)
    name: String,
}

impl TokenBucket {
    fn new(name: &str, capacity: u32, refill_rate: f64) -> Self {
        Self {
            capacity,
            tokens: capacity as f64,
            refill_rate,
            last_refill: Instant::now(),
            name: name.to_string(),
        }
    }

    /// Try to consume one token. Returns Ok if allowed, Err with wait time if rate limited.
    fn try_acquire(&mut self) -> Result<(), std::time::Duration> {
        self.refill();

        if self.tokens >= 1.0 {
            self.tokens -= 1.0;
            Ok(())
        } else {
            // Calculate how long until one token is available
            let deficit = 1.0 - self.tokens;
            let wait_secs = deficit / self.refill_rate;
            Err(std::time::Duration::from_secs_f64(wait_secs))
        }
    }

    /// Add tokens based on elapsed time since last refill.
    fn refill(&mut self) {
        let now = Instant::now();
        let elapsed = now.duration_since(self.last_refill).as_secs_f64();
        self.tokens = (self.tokens + elapsed * self.refill_rate).min(self.capacity as f64);
        self.last_refill = now;
    }

    /// Check how many tokens are available without consuming any.
    fn available(&mut self) -> u32 {
        self.refill();
        self.tokens as u32
    }
}

fn main() {
    // Allow 5 file writes, refilling at 1 per second
    let mut file_write_limiter = TokenBucket::new("file_writes", 5, 1.0);

    // Allow 20 shell commands, refilling at 2 per second
    let mut shell_limiter = TokenBucket::new("shell_commands", 20, 2.0);

    // Simulate rapid file writes
    println!("=== Simulating rapid file writes ===");
    for i in 0..8 {
        match file_write_limiter.try_acquire() {
            Ok(()) => println!("  Write {}: ALLOWED ({} tokens left)",
                i + 1, file_write_limiter.available()),
            Err(wait) => println!("  Write {}: RATE LIMITED (wait {:?})",
                i + 1, wait),
        }
    }

    println!("\n=== Shell command limiter ===");
    println!("  Available tokens: {}", shell_limiter.available());
}
```

## Multi-Dimensional Rate Limiting

A coding agent needs rate limits on multiple dimensions simultaneously. You might allow 10 file writes per minute, but only 50 total tool invocations per minute, and only 5 shell commands per minute. Let's build a multi-dimensional rate limiter:

```rust
use std::collections::HashMap;
use std::time::Instant;

/// Rate limiter that enforces limits across multiple dimensions.
struct MultiRateLimiter {
    limiters: HashMap<String, TokenBucket>,
}

struct TokenBucket {
    capacity: u32,
    tokens: f64,
    refill_rate: f64,
    last_refill: Instant,
}

impl TokenBucket {
    fn new(capacity: u32, refill_rate: f64) -> Self {
        Self {
            capacity,
            tokens: capacity as f64,
            refill_rate,
            last_refill: Instant::now(),
        }
    }

    fn try_acquire(&mut self) -> Result<(), f64> {
        self.refill();
        if self.tokens >= 1.0 {
            self.tokens -= 1.0;
            Ok(())
        } else {
            Err((1.0 - self.tokens) / self.refill_rate)
        }
    }

    fn refill(&mut self) {
        let elapsed = self.last_refill.elapsed().as_secs_f64();
        self.tokens = (self.tokens + elapsed * self.refill_rate).min(self.capacity as f64);
        self.last_refill = Instant::now();
    }
}

impl MultiRateLimiter {
    fn new() -> Self {
        Self {
            limiters: HashMap::new(),
        }
    }

    fn add_limit(&mut self, name: &str, capacity: u32, per_second: f64) {
        self.limiters
            .insert(name.to_string(), TokenBucket::new(capacity, per_second));
    }

    /// Check a tool invocation against all applicable rate limits.
    /// The tool must pass ALL applicable limits to proceed.
    fn check_tool(&mut self, tool_name: &str) -> Result<(), String> {
        // Every tool counts against the global limit
        let global_result = self.limiters
            .get_mut("global")
            .map(|l| l.try_acquire());

        if let Some(Err(wait)) = global_result {
            return Err(format!(
                "Global rate limit exceeded (wait {:.1}s)", wait
            ));
        }

        // Check tool-specific limit
        let specific_result = self.limiters
            .get_mut(tool_name)
            .map(|l| l.try_acquire());

        if let Some(Err(wait)) = specific_result {
            return Err(format!(
                "{} rate limit exceeded (wait {:.1}s)", tool_name, wait
            ));
        }

        Ok(())
    }

    /// Report current status of all limiters.
    fn status(&mut self) -> Vec<(String, u32)> {
        self.limiters
            .iter_mut()
            .map(|(name, bucket)| {
                bucket.refill();
                (name.clone(), bucket.tokens as u32)
            })
            .collect()
    }
}

fn main() {
    let mut limiter = MultiRateLimiter::new();

    // Global: 50 tool calls per minute
    limiter.add_limit("global", 50, 50.0 / 60.0);
    // File writes: 10 per minute
    limiter.add_limit("file_write", 10, 10.0 / 60.0);
    // Shell commands: 5 per minute
    limiter.add_limit("shell", 5, 5.0 / 60.0);
    // API calls: 20 per minute
    limiter.add_limit("api_call", 20, 20.0 / 60.0);

    // Simulate tool invocations
    let tools = ["file_write", "shell", "file_write", "api_call",
                  "shell", "shell", "shell", "shell", "shell"];

    println!("=== Multi-dimensional rate limiting ===\n");
    for tool in &tools {
        match limiter.check_tool(tool) {
            Ok(()) => println!("  {} -> ALLOWED", tool),
            Err(reason) => println!("  {} -> BLOCKED: {}", tool, reason),
        }
    }

    println!("\nCurrent limiter status:");
    for (name, tokens) in limiter.status() {
        println!("  {}: {} tokens available", name, tokens);
    }
}
```

## Circuit Breakers

Rate limiters control the pace of actions. Circuit breakers detect patterns of failure and halt the agent entirely. If the agent is making the same mistake over and over (editing a file, running tests, seeing the same error, editing the file again), a circuit breaker stops the loop:

```rust
use std::time::{Duration, Instant};

/// A circuit breaker that trips after detecting repeated failures.
struct CircuitBreaker {
    /// Name of the circuit (for logging)
    name: String,
    /// Current state
    state: CircuitState,
    /// Number of consecutive failures
    failure_count: u32,
    /// Threshold at which the circuit trips
    failure_threshold: u32,
    /// How long to stay in Open state before trying again
    recovery_timeout: Duration,
    /// When the circuit was last opened
    opened_at: Option<Instant>,
    /// Pattern detection: recent error messages
    recent_errors: Vec<String>,
}

#[derive(Debug, Clone, PartialEq)]
enum CircuitState {
    /// Normal operation -- requests flow through
    Closed,
    /// Circuit is tripped -- all requests are blocked
    Open,
    /// Testing recovery -- one request allowed through
    HalfOpen,
}

impl CircuitBreaker {
    fn new(name: &str, failure_threshold: u32, recovery_timeout: Duration) -> Self {
        Self {
            name: name.to_string(),
            state: CircuitState::Closed,
            failure_count: 0,
            failure_threshold,
            recovery_timeout,
            opened_at: None,
            recent_errors: Vec::new(),
        }
    }

    /// Check if the circuit allows a request through.
    fn allow_request(&mut self) -> Result<(), String> {
        match &self.state {
            CircuitState::Closed => Ok(()),
            CircuitState::Open => {
                // Check if recovery timeout has elapsed
                if let Some(opened) = self.opened_at {
                    if opened.elapsed() >= self.recovery_timeout {
                        self.state = CircuitState::HalfOpen;
                        println!(
                            "Circuit '{}': transitioning to HalfOpen (testing recovery)",
                            self.name
                        );
                        return Ok(());
                    }
                }
                Err(format!(
                    "Circuit '{}' is OPEN ({} consecutive failures). \
                     Agent execution halted. Wait {:?} for recovery attempt.",
                    self.name, self.failure_count, self.recovery_timeout
                ))
            }
            CircuitState::HalfOpen => {
                // Allow one request to test recovery
                Ok(())
            }
        }
    }

    /// Record a successful result, potentially closing the circuit.
    fn record_success(&mut self) {
        match self.state {
            CircuitState::HalfOpen => {
                println!("Circuit '{}': recovery successful, closing circuit", self.name);
                self.state = CircuitState::Closed;
                self.failure_count = 0;
                self.opened_at = None;
                self.recent_errors.clear();
            }
            CircuitState::Closed => {
                self.failure_count = 0;
            }
            _ => {}
        }
    }

    /// Record a failure, potentially tripping the circuit.
    fn record_failure(&mut self, error: &str) {
        self.failure_count += 1;
        self.recent_errors.push(error.to_string());

        // Keep only recent errors
        if self.recent_errors.len() > 10 {
            self.recent_errors.remove(0);
        }

        match self.state {
            CircuitState::Closed => {
                if self.failure_count >= self.failure_threshold {
                    println!(
                        "Circuit '{}': TRIPPED after {} failures",
                        self.name, self.failure_count
                    );
                    self.state = CircuitState::Open;
                    self.opened_at = Some(Instant::now());
                }
            }
            CircuitState::HalfOpen => {
                // Recovery failed -- reopen the circuit
                println!("Circuit '{}': recovery failed, reopening", self.name);
                self.state = CircuitState::Open;
                self.opened_at = Some(Instant::now());
            }
            CircuitState::Open => {
                // Already open, just count
            }
        }
    }

    /// Detect if recent errors show a repetitive pattern.
    fn detect_loop(&self) -> bool {
        if self.recent_errors.len() < 3 {
            return false;
        }

        // Check if the last 3 errors are the same
        let last = &self.recent_errors[self.recent_errors.len() - 1];
        self.recent_errors
            .iter()
            .rev()
            .take(3)
            .all(|e| e == last)
    }
}

fn main() {
    let mut breaker = CircuitBreaker::new(
        "cargo-test",
        3,
        Duration::from_secs(60),
    );

    // Simulate a series of failures
    let results = [
        Err("compilation error: expected `;`"),
        Err("compilation error: expected `;`"),
        Err("compilation error: expected `;`"), // This trips the circuit
        Err("compilation error: expected `;`"), // This is blocked
    ];

    for (i, result) in results.iter().enumerate() {
        println!("\n--- Attempt {} ---", i + 1);

        match breaker.allow_request() {
            Ok(()) => {
                match result {
                    Ok(()) => {
                        println!("  Success!");
                        breaker.record_success();
                    }
                    Err(error) => {
                        println!("  Failed: {}", error);
                        breaker.record_failure(error);
                    }
                }
            }
            Err(reason) => {
                println!("  BLOCKED by circuit breaker: {}", reason);
            }
        }
    }

    println!("\nLoop detected: {}", breaker.detect_loop());
    println!("Circuit state: {:?}", breaker.state);
}
```

::: wild In the Wild
Claude Code implements a form of rate limiting through its tool use patterns -- it tracks how many tool calls occur per turn and can detect when the agent enters a repetitive loop (making the same edit, running tests, seeing the same error). When this happens, it surfaces the pattern to the user rather than continuing indefinitely. Codex addresses runaway agents differently: since it runs in a sandboxed environment with resource limits (memory, CPU), the container itself acts as a circuit breaker -- if the agent consumes too many resources, the container is killed.
:::

::: python Coming from Python
Python developers might use libraries like `ratelimit` or `tenacity` for rate limiting and retries. The Rust implementations shown here use the same algorithms (token bucket, circuit breaker) but are built from primitives, giving you complete control over the behavior. Rust's ownership model ensures that the rate limiter state cannot be accidentally shared between threads without explicit synchronization -- the `&mut self` on `try_acquire` guarantees exclusive access, whereas Python rate limiters need explicit locking for thread safety.
:::

## Adaptive Rate Limits

Static rate limits are a good start, but sophisticated agents benefit from limits that adjust based on context. An agent that is succeeding should be allowed to work faster; an agent that is struggling should be slowed down:

```rust
/// An adaptive rate limiter that adjusts based on success/failure ratio.
struct AdaptiveRateLimiter {
    base_rate: f64,       // Starting tokens per second
    current_rate: f64,    // Current adjusted rate
    min_rate: f64,        // Floor -- never go below this
    max_rate: f64,        // Ceiling -- never exceed this
    success_count: u32,
    failure_count: u32,
    window_size: u32,     // Reset counts after this many events
}

impl AdaptiveRateLimiter {
    fn new(base_rate: f64) -> Self {
        Self {
            base_rate,
            current_rate: base_rate,
            min_rate: base_rate * 0.1,  // Can slow down to 10% of base
            max_rate: base_rate * 3.0,  // Can speed up to 300% of base
            success_count: 0,
            failure_count: 0,
            window_size: 20,
        }
    }

    fn record_outcome(&mut self, success: bool) {
        if success {
            self.success_count += 1;
        } else {
            self.failure_count += 1;
        }

        let total = self.success_count + self.failure_count;
        if total >= self.window_size {
            self.adjust_rate();
            self.success_count = 0;
            self.failure_count = 0;
        }
    }

    fn adjust_rate(&mut self) {
        let total = (self.success_count + self.failure_count) as f64;
        let success_ratio = self.success_count as f64 / total;

        // High success rate -> increase rate (agent is doing well)
        // Low success rate -> decrease rate (agent is struggling)
        let adjustment = if success_ratio > 0.9 {
            1.5 // Speed up 50%
        } else if success_ratio > 0.7 {
            1.0 // Maintain current rate
        } else if success_ratio > 0.5 {
            0.7 // Slow down 30%
        } else {
            0.3 // Slow down 70%
        };

        self.current_rate = (self.current_rate * adjustment)
            .max(self.min_rate)
            .min(self.max_rate);

        println!(
            "Rate adjusted: {:.2} -> {:.2} tokens/sec (success ratio: {:.0}%)",
            self.current_rate / adjustment,
            self.current_rate,
            success_ratio * 100.0
        );
    }

    fn current_rate(&self) -> f64 {
        self.current_rate
    }
}

fn main() {
    let mut limiter = AdaptiveRateLimiter::new(1.0); // 1 token/sec base

    println!("Initial rate: {:.2} tokens/sec\n", limiter.current_rate());

    // Simulate a successful run (18 successes, 2 failures)
    println!("--- Mostly successful run ---");
    for i in 0..20 {
        limiter.record_outcome(i % 10 != 5); // Fail on every 6th
    }

    println!("Rate after success: {:.2}\n", limiter.current_rate());

    // Simulate a struggling run (5 successes, 15 failures)
    println!("--- Struggling run ---");
    for i in 0..20 {
        limiter.record_outcome(i < 5); // Only first 5 succeed
    }

    println!("Rate after failures: {:.2}", limiter.current_rate());
}
```

## Key Takeaways

- Token bucket rate limiters provide smooth rate control for tool invocations, allowing short bursts while enforcing a long-term average rate
- Multi-dimensional rate limiting applies separate limits to different tool categories (file writes, shell commands, API calls) plus a global limit across all operations
- Circuit breakers detect repeated failures and halt the agent entirely, preventing infinite loops where the agent keeps making the same mistake
- Loop detection (checking if recent errors are identical) catches the common failure mode where an agent repeatedly applies the same broken fix
- Adaptive rate limits that increase during successful operation and decrease during failures automatically balance productivity with safety
