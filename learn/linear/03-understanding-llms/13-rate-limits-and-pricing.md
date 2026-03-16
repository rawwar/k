---
title: Rate Limits and Pricing
description: Understanding API rate limits, token-based pricing models, and strategies for cost-effective agent operation.
---

# Rate Limits and Pricing

> **What you'll learn:**
> - How rate limits work across requests per minute, tokens per minute, and concurrent requests
> - The pricing structure for input and output tokens and how agent loops multiply API costs
> - Practical strategies for retry logic, exponential backoff, and cost optimization in agent systems

Every API call costs money and is subject to rate limits. For a chatbot that makes one API call per user message, this is straightforward. For a coding agent that might make 20-50 API calls to complete a single task -- each with tool definitions, conversation history, and tool results -- costs and rate limits become a real engineering concern. This subchapter gives you the numbers and the strategies to handle them.

## How Rate Limits Work

API providers impose rate limits across multiple dimensions simultaneously:

**Requests per minute (RPM):** The number of API calls you can make per minute.

**Tokens per minute (TPM):** The total tokens (input + output) you can process per minute.

**Tokens per day (TPD):** Some tiers also have daily token limits.

You hit whichever limit comes first. A burst of small requests might hit RPM. A few large requests with extensive context might hit TPM. Here are typical limits for reference (check current docs for exact numbers, as these change):

| Provider/Tier | RPM | TPM (Input) | TPM (Output) |
|---|---|---|---|
| Anthropic (Build tier) | 50 | 40,000 | 8,000 |
| Anthropic (Scale tier) | 1,000 | 400,000 | 80,000 |
| OpenAI (Tier 1) | 500 | 30,000 | - |
| OpenAI (Tier 3) | 5,000 | 800,000 | - |

For a coding agent, the TPM limit is usually the binding constraint. A single request with a 50K-token conversation history, 2K tokens of tool definitions, and an 8K-token response consumes 60K tokens. At Anthropic's Build tier (40K TPM input), that single request would exceed the per-minute limit.

## Rate Limit Headers

Both providers return rate limit information in HTTP response headers:

**Anthropic:**
```
anthropic-ratelimit-requests-limit: 50
anthropic-ratelimit-requests-remaining: 49
anthropic-ratelimit-requests-reset: 2025-01-15T12:00:30Z
anthropic-ratelimit-tokens-limit: 40000
anthropic-ratelimit-tokens-remaining: 38477
anthropic-ratelimit-tokens-reset: 2025-01-15T12:00:30Z
```

**OpenAI:**
```
x-ratelimit-limit-requests: 500
x-ratelimit-remaining-requests: 499
x-ratelimit-reset-requests: 200ms
x-ratelimit-limit-tokens: 30000
x-ratelimit-remaining-tokens: 28500
x-ratelimit-reset-tokens: 6s
```

Your agent should read these headers and use them proactively. If `remaining-tokens` is below your estimated next request size, wait until the reset time rather than sending a request that will be rejected.

## Token Pricing

API pricing is based on tokens processed, with input and output priced differently:

| Model | Input Price (per 1M tokens) | Output Price (per 1M tokens) |
|---|---|---|
| Claude 4 Sonnet | $3.00 | $15.00 |
| Claude 3.5 Sonnet | $3.00 | $15.00 |
| Claude 3.5 Haiku | $0.80 | $4.00 |
| GPT-4o | $2.50 | $10.00 |
| GPT-4o mini | $0.15 | $0.60 |
| GPT-4.1 | $2.00 | $8.00 |
| GPT-4.1 mini | $0.40 | $1.60 |

*Prices are approximate and change over time. Check current docs for exact pricing.*

The 3-5x multiplier on output tokens is important for agents. A response where the model generates 4,000 tokens of code is much more expensive than the same 4,000 tokens of context you sent as input.

## Why Agents Are Expensive

A single coding task might involve this sequence:

```
Call 1: User request + system prompt + tools (3K input, 200 output)    = $0.012
Call 2: + tool result from file read (8K input, 2K output)              = $0.054
Call 3: + file edit + verify command (15K input, 3K output)             = $0.090
Call 4: + compilation error result (20K input, 1K output)               = $0.075
Call 5: + second edit attempt (25K input, 2K output)                    = $0.105
Call 6: + successful test result (30K input, 500 output)                = $0.097
```

Total: approximately $0.43 for a single task with Claude 3.5 Sonnet. That is one task for one user. Scale to thousands of users making dozens of requests per day, and API costs become a significant operational concern.

Notice the compounding effect: each call includes the full conversation history, so input tokens grow with every turn. By call 6, you are sending 30K tokens of input even though the actual new information (the test result) is only a few hundred tokens. This is why context management (covered in [Context Windows](/linear/03-understanding-llms/03-context-windows)) matters for cost as well as for fitting within limits.

::: python Coming from Python
In Python agent frameworks, cost tracking is often an afterthought -- you add a callback or middleware to log token counts. In Rust, you can build cost tracking into your core types. Define a `TokenUsage` struct that accumulates across the session, calculate costs per model, and present a running total to the user. The type system helps ensure you never forget to track usage.
:::

## Implementing Retry Logic

When you hit a rate limit, the API returns HTTP 429 with a `Retry-After` header. Your agent needs automatic retry logic:

```rust
use std::time::Duration;
use tokio::time::sleep;

async fn call_api_with_retry(
    client: &Client,
    request: &ApiRequest,
    max_retries: u32,
) -> Result<ApiResponse, ApiError> {
    let mut retries = 0;
    let mut delay = Duration::from_secs(1);

    loop {
        match client.send(request).await {
            Ok(response) => return Ok(response),
            Err(ApiError::RateLimit { retry_after }) => {
                if retries >= max_retries {
                    return Err(ApiError::RateLimit { retry_after });
                }
                let wait = retry_after.unwrap_or(delay);
                sleep(wait).await;
                retries += 1;
                delay *= 2; // Exponential backoff
            }
            Err(ApiError::ServerError(_)) if retries < max_retries => {
                sleep(delay).await;
                retries += 1;
                delay *= 2;
            }
            Err(e) => return Err(e),
        }
    }
}
```

**Exponential backoff** doubles the wait time after each retry: 1s, 2s, 4s, 8s, 16s. This prevents thundering herd problems where many clients retry simultaneously and overwhelm the API.

**Jitter** adds a random component to the delay to prevent synchronized retries:

```rust
use rand::Rng;

let jitter = rand::thread_rng().gen_range(0..1000);
let wait = delay + Duration::from_millis(jitter);
```

## Cost Optimization Strategies

### 1. Use Prompt Caching

Anthropic offers prompt caching that reduces the cost of repetitive content (system prompt, tool definitions) across consecutive requests. Since agents send the same system prompt and tools on every call, caching can reduce input token costs by up to 90% for the cached portion.

```json
{
  "system": [
    {
      "type": "text",
      "text": "You are an expert coding assistant...",
      "cache_control": {"type": "ephemeral"}
    }
  ]
}
```

### 2. Use Cheaper Models for Simple Tasks

Not every API call needs the most capable model. Model routing uses a cheaper model for simple operations and reserves the expensive model for complex reasoning:

```rust
fn select_model(task: &TaskType) -> &str {
    match task {
        TaskType::FileRead | TaskType::SimpleEdit => "claude-3-5-haiku-20241022",
        TaskType::ComplexReasoning | TaskType::MultiFileRefactor => "claude-sonnet-4-20250514",
    }
}
```

### 3. Compress Context Aggressively

Every token in your conversation history costs money on every subsequent call. Summarize old tool results, truncate large outputs, and remove redundant messages.

### 4. Truncate Large Tool Results

If a shell command outputs 50,000 characters, sending all of it consumes roughly 15,000 tokens on every subsequent API call. Truncate to a reasonable size:

```rust
fn truncate_tool_result(result: &str, max_chars: usize) -> String {
    if result.len() <= max_chars {
        return result.to_string();
    }
    let half = max_chars / 2;
    format!(
        "{}\n\n[... {} characters truncated ...]\n\n{}",
        &result[..half],
        result.len() - max_chars,
        &result[result.len() - half..]
    )
}
```

### 5. Track and Display Costs

Show the user how much their session is costing. This is not just a courtesy -- it helps users make informed decisions about whether to continue a complex conversation or start fresh.

::: wild In the Wild
Claude Code tracks token usage across a session and displays a running cost indicator. It also uses prompt caching to reduce costs for the system prompt and tool definitions that are sent identically on every call. Some open-source agents implement model routing, using a smaller model for simple file reads and grep operations while reserving the frontier model for complex edits and reasoning.
:::

## Handling Overloaded APIs

Beyond rate limits, APIs can become temporarily overloaded during high-demand periods. The Anthropic API returns HTTP 529 for overloaded conditions, and OpenAI returns 503. These require longer retry delays than rate limit errors:

```rust
Err(ApiError::Overloaded) => {
    if retries >= max_retries {
        return Err(ApiError::Overloaded);
    }
    // Longer initial delay for overloaded conditions
    let wait = Duration::from_secs(5) * (2_u32.pow(retries));
    sleep(wait + jitter()).await;
    retries += 1;
}
```

For a production agent, consider implementing a circuit breaker pattern: after N consecutive failures, stop sending requests for a cool-down period rather than continuing to retry. This protects both the API and the user experience.

## Key Takeaways

- Rate limits operate on multiple dimensions (RPM, TPM, TPD) simultaneously -- for agents, TPM is usually the binding constraint because conversation history accumulates tokens across turns
- Output tokens cost 3-5x more than input tokens, and agent loops compound costs because each call resends the full conversation history -- a single task can cost $0.30-1.00+ with frontier models
- Implement exponential backoff with jitter for rate limit retries, and use the `Retry-After` header and rate limit response headers for proactive request pacing
- Optimize costs through prompt caching, model routing (cheap models for simple tasks), aggressive context compression, and tool result truncation
- Track and display token usage and costs to the user -- transparency about spending builds trust and helps users manage their API budget
