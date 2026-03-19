# Rate Limits and Retries

## Introduction

Rate limiting is one of the most critical concerns for production AI agents and applications that interact with LLM APIs. Every major provider — OpenAI, Anthropic, Google — enforces rate limits to protect infrastructure, ensure fair usage, and manage capacity across their user base.

For coding agents like Copilot, Cursor, and Aider, hitting a rate limit mid-task can break workflows, corrupt partially-generated code, or cause cascading failures. A well-designed retry and rate-limiting strategy is the difference between a fragile demo and a production-grade system.

**Why this matters for agents specifically:**

- Agents make many sequential API calls in tight loops (reasoning → tool use → observation → reasoning)
- A single 429 error can derail a multi-step chain-of-thought
- Users expect real-time responsiveness; long retry delays feel like failures
- Cost and rate limits are coupled — burning through tokens fast hits both walls
- Multi-user deployments multiply the problem by the number of concurrent users

---

## Understanding Rate Limits

LLM API providers enforce several types of rate limits simultaneously. You must respect **all** of them — the most restrictive limit wins.

### Types of Rate Limits

| Limit Type | Abbreviation | What It Measures |
|---|---|---|
| Requests per minute | RPM | Number of API calls per 60-second window |
| Tokens per minute | TPM | Total tokens (input + output) consumed per 60-second window |
| Tokens per day | TPD | Total tokens consumed per 24-hour window |
| Requests per day | RPD | Number of API calls per 24-hour window |
| Images per minute | IPM | Number of image generation requests per minute |
| Concurrent requests | — | Number of in-flight requests at any given time |

### How Limits Work

Most providers use a **sliding window** approach. The window is not aligned to clock minutes — it tracks the rolling 60-second period ending at the current time. Some providers use **fixed windows** aligned to clock boundaries, which can cause burst issues at window edges.

```
Timeline: ──────────────────────────────────────────────────►
                    |◄── 60 second sliding window ──►|
                    ▲                                  ▲
              oldest counted                      current time
              request in window                   (new request)
```

### OpenAI Rate Limit Tiers

OpenAI uses a tiered system where limits increase as you spend more. You automatically graduate to higher tiers based on cumulative spend.

| Tier | Qualification | RPM (gpt-4o) | TPM (gpt-4o) | RPM (gpt-4o-mini) | TPM (gpt-4o-mini) |
|---|---|---|---|---|---|
| Free | Default | 500 | 30,000 | 500 | 200,000 |
| Tier 1 | $5 paid | 500 | 30,000 | 500 | 200,000 |
| Tier 2 | $50 paid + 7 days | 5,000 | 450,000 | 5,000 | 2,000,000 |
| Tier 3 | $100 paid + 7 days | 5,000 | 800,000 | 5,000 | 4,000,000 |
| Tier 4 | $250 paid + 14 days | 10,000 | 2,000,000 | 10,000 | 10,000,000 |
| Tier 5 | $1,000 paid + 30 days | 10,000 | 30,000,000 | 10,000 | 15,000,000 |

> **Note:** These limits change frequently. Always check the [OpenAI rate limits page](https://platform.openai.com/docs/guides/rate-limits) for current values.

**Batch API limits** are separate and typically much higher — OpenAI's Batch API allows queuing requests with a 24-hour SLA at 50% cost reduction, which effectively bypasses per-minute limits for non-real-time workloads.

### Anthropic Rate Limits

Anthropic enforces limits per **model family** and **workspace**. Limits increase with usage tier:

| Tier | Qualification | RPM (Claude Sonnet) | TPM Input (Sonnet) | TPM Output (Sonnet) |
|---|---|---|---|---|
| Tier 1 (Build) | $1 deposit | 50 | 40,000 | 8,000 |
| Tier 2 (Build) | $40 spend | 1,000 | 80,000 | 16,000 |
| Tier 3 (Scale) | $200 spend | 2,000 | 160,000 | 32,000 |
| Tier 4 (Scale) | $400 spend | 4,000 | 400,000 | 80,000 |

Key Anthropic-specific details:
- **Input and output tokens are limited separately** — unlike OpenAI which combines them
- Rate limits apply per **workspace**, not per API key
- The `claude-3-5-sonnet` and `claude-3-5-haiku` model families have different limits
- Anthropic provides a `retry-after` header indicating seconds to wait

### Google (Gemini) Rate Limits

Google's Gemini API rate limits vary by model and pricing plan:

| Model | Free Tier RPM | Pay-as-you-go RPM | Free Tier TPM |
|---|---|---|---|
| Gemini 2.0 Flash | 15 | 2,000 | 1,000,000 |
| Gemini 2.5 Pro | 5 | 1,000 | 1,000,000 |
| Gemini 2.5 Flash | 10 | 2,000 | 1,000,000 |

Google also enforces **requests per day (RPD)** limits on the free tier (typically 1,500/day for Flash models), which are removed on the pay-as-you-go plan.

---

## HTTP 429 Too Many Requests

When you exceed a rate limit, the API returns an HTTP `429 Too Many Requests` status code. The response body and headers contain information about which limit was exceeded and when you can retry.

### Response Headers

Providers include rate limit information in response headers — both on successful (200) and rate-limited (429) responses:

#### OpenAI Headers

```http
HTTP/1.1 429 Too Many Requests
Content-Type: application/json

x-ratelimit-limit-requests: 5000
x-ratelimit-limit-tokens: 800000
x-ratelimit-remaining-requests: 0
x-ratelimit-remaining-tokens: 612344
x-ratelimit-reset-requests: 12ms
x-ratelimit-reset-tokens: 14.1s
```

| Header | Description |
|---|---|
| `x-ratelimit-limit-requests` | Maximum RPM for your tier |
| `x-ratelimit-limit-tokens` | Maximum TPM for your tier |
| `x-ratelimit-remaining-requests` | Requests remaining in current window |
| `x-ratelimit-remaining-tokens` | Tokens remaining in current window |
| `x-ratelimit-reset-requests` | Time until request limit resets |
| `x-ratelimit-reset-tokens` | Time until token limit resets |

#### Anthropic Headers

```http
HTTP/1.1 429 Too Many Requests

anthropic-ratelimit-requests-limit: 1000
anthropic-ratelimit-requests-remaining: 0
anthropic-ratelimit-requests-reset: 2024-01-15T12:00:30Z
anthropic-ratelimit-tokens-input-limit: 80000
anthropic-ratelimit-tokens-input-remaining: 45200
anthropic-ratelimit-tokens-input-reset: 2024-01-15T12:00:15Z
anthropic-ratelimit-tokens-output-limit: 16000
anthropic-ratelimit-tokens-output-remaining: 8500
anthropic-ratelimit-tokens-output-reset: 2024-01-15T12:00:20Z
retry-after: 30
```

Anthropic uses **ISO 8601 timestamps** for reset times (not durations like OpenAI) and separates input/output token limits in headers.

#### Google Headers

Google Gemini returns standard `Retry-After` headers and uses gRPC error codes (`RESOURCE_EXHAUSTED`) for gRPC endpoints. The REST API returns:

```http
HTTP/1.1 429 Too Many Requests
Retry-After: 60
```

### The `Retry-After` Header

The `Retry-After` header is the most reliable signal for when to retry. It can be specified as:

- **Seconds:** `Retry-After: 30` — wait 30 seconds
- **HTTP date:** `Retry-After: Wed, 15 Jan 2025 12:01:00 GMT` — wait until this time

**Always prefer `Retry-After` over your own backoff calculation when available.** The provider knows its own capacity better than your heuristic.

### Error Response Body

```json
{
  "error": {
    "message": "Rate limit reached for gpt-4o in organization org-xxx on tokens per min (TPM): Limit 800000, Used 799500, Requested 1200. Please try again in 1.2s.",
    "type": "tokens",
    "param": null,
    "code": "rate_limit_exceeded"
  }
}
```

The `message` field often contains specific details about which limit was hit and an estimated wait time — parse this as a supplementary signal.

---

## Retry Strategies

### Simple Retry with Fixed Delay

The simplest approach: wait a fixed time, then retry. Easy to implement but suboptimal — it doesn't adapt to varying congestion.

```python
import time
import httpx

def call_with_fixed_retry(url, payload, max_retries=3, delay=5.0):
    for attempt in range(max_retries + 1):
        response = httpx.post(url, json=payload)
        if response.status_code != 429:
            return response
        if attempt < max_retries:
            time.sleep(delay)
    raise Exception(f"Failed after {max_retries} retries")
```

**Problem:** If 100 clients all get rate-limited at the same time, they all retry at exactly `t + delay`, causing a **thundering herd** that rate-limits them all again.

### Exponential Backoff

Increase the wait time exponentially with each retry attempt:

```
delay = base_delay * (2 ^ attempt)
```

| Attempt | Delay (base=1s) | Delay (base=2s) |
|---|---|---|
| 0 | 1s | 2s |
| 1 | 2s | 4s |
| 2 | 4s | 8s |
| 3 | 8s | 16s |
| 4 | 16s | 32s |

```python
import time
import httpx

def call_with_exponential_backoff(url, payload, max_retries=5, base_delay=1.0):
    for attempt in range(max_retries + 1):
        response = httpx.post(url, json=payload)
        if response.status_code != 429:
            return response
        if attempt < max_retries:
            delay = base_delay * (2 ** attempt)
            time.sleep(delay)
    raise Exception(f"Failed after {max_retries} retries")
```

**Problem:** Still causes thundering herd — all clients with the same attempt count compute the same delay.

### Exponential Backoff with Jitter

Adding randomness (**jitter**) to the backoff delay desynchronizes retries across clients. There are several jitter strategies:

#### Full Jitter

```python
delay = random.uniform(0, base_delay * (2 ** attempt))
```

Randomizes the entire range from 0 to the exponential ceiling. Provides maximum spread but can produce very short delays.

#### Equal Jitter

```python
half = (base_delay * (2 ** attempt)) / 2
delay = half + random.uniform(0, half)
```

Guarantees at least half the exponential delay. Good balance between spread and minimum wait.

#### Decorrelated Jitter

```python
delay = min(max_delay, random.uniform(base_delay, previous_delay * 3))
```

Each delay is based on the previous delay rather than the attempt number. Adapts naturally to varying congestion.

#### Comparison

```
Attempt 3 delays (base=1s):
  No jitter:     8.00s (everyone retries at the same time)
  Full jitter:   0.00s – 8.00s (maximum spread, but some too eager)
  Equal jitter:  4.00s – 8.00s (good minimum wait + spread)
  Decorrelated:  varies based on history (adaptive)
```

**Recommendation:** Use **equal jitter** for most production systems. It prevents thundering herds while ensuring a reasonable minimum wait.

### Maximum Retry Attempts and Timeouts

Always cap retries to prevent infinite loops:

```python
MAX_RETRIES = 5          # Never retry more than 5 times
MAX_DELAY = 60           # Cap individual delay at 60 seconds
TOTAL_TIMEOUT = 120      # Give up after 2 minutes total
```

### Idempotency Considerations

Not all API calls are safe to retry:

- **Chat completions:** Generally safe to retry — the API is stateless. You may get different outputs but no side effects.
- **Function/tool calls with side effects:** If the model called a tool that modifies state (e.g., writing to a database), retrying the entire chain could duplicate the side effect. Track which tool calls have been executed.
- **Streaming responses:** If a stream is interrupted partway through, you'll get a partial response. Retry from the beginning, not from the partial output.
- **File uploads:** Use idempotency keys if the provider supports them, or check for existing uploads before retrying.

---

## Circuit Breaker Pattern

The circuit breaker pattern prevents your application from repeatedly calling a service that's likely down or overloaded. Instead of retrying indefinitely, the circuit "trips" after a threshold of failures, and subsequent requests fail immediately without making the API call.

### States

```
                    failure threshold reached
     ┌────────┐  ─────────────────────────────►  ┌────────┐
     │ CLOSED │                                   │  OPEN  │
     │(normal)│  ◄─────────────────────────────   │(reject)│
     └────────┘    probe succeeds (via half-open)  └────────┘
          ▲                                            │
          │         recovery timeout expires            │
          │              ┌───────────┐                  │
          └──────────────│ HALF-OPEN │◄─────────────────┘
            probe        │  (probe)  │
            succeeds     └───────────┘
```

**Closed (Normal Operation):**
- Requests pass through to the API
- Failures are counted
- If failures exceed the threshold within a window, transition to Open

**Open (Rejecting Requests):**
- All requests are immediately rejected without calling the API
- Returns a cached response, fallback, or error
- After a recovery timeout, transition to Half-Open

**Half-Open (Probing):**
- Allow a limited number of test requests through
- If they succeed, transition back to Closed
- If they fail, transition back to Open

### Configuration Parameters

```python
class CircuitBreakerConfig:
    failure_threshold: int = 5       # failures before opening
    recovery_timeout: float = 30.0   # seconds in open state
    half_open_max_calls: int = 3     # test calls in half-open
    failure_window: float = 60.0     # window for counting failures
    counted_exceptions: tuple = (RateLimitError, ServerError)
```

### When to Trip the Circuit

For LLM APIs, trip the circuit breaker on:

- **Consecutive 429s** exceeding a threshold (e.g., 5 in 60 seconds)
- **5xx server errors** (502, 503, 504) — the provider is having issues
- **Timeouts** — the provider is overloaded and not responding in time
- **Connection errors** — network-level failures

**Do NOT** trip the circuit on:
- 400 Bad Request (your fault — fix the request)
- 401 Unauthorized (your credentials are wrong)
- 422 Unprocessable Entity (invalid parameters)

### Implementation

```python
import time
import threading
from enum import Enum

class CircuitState(Enum):
    CLOSED = "closed"
    OPEN = "open"
    HALF_OPEN = "half-open"

class CircuitBreaker:
    def __init__(self, failure_threshold=5, recovery_timeout=30.0):
        self.failure_threshold = failure_threshold
        self.recovery_timeout = recovery_timeout
        self.state = CircuitState.CLOSED
        self.failure_count = 0
        self.last_failure_time = 0
        self.lock = threading.Lock()

    def can_execute(self) -> bool:
        with self.lock:
            if self.state == CircuitState.CLOSED:
                return True
            elif self.state == CircuitState.OPEN:
                if time.time() - self.last_failure_time >= self.recovery_timeout:
                    self.state = CircuitState.HALF_OPEN
                    return True
                return False
            elif self.state == CircuitState.HALF_OPEN:
                return True
        return False

    def record_success(self):
        with self.lock:
            self.failure_count = 0
            self.state = CircuitState.CLOSED

    def record_failure(self):
        with self.lock:
            self.failure_count += 1
            self.last_failure_time = time.time()
            if self.failure_count >= self.failure_threshold:
                self.state = CircuitState.OPEN
            elif self.state == CircuitState.HALF_OPEN:
                self.state = CircuitState.OPEN
```

### How Agents Use Circuit Breakers

Coding agents typically implement circuit breakers at the **provider level**:

1. Track failures per provider (OpenAI, Anthropic, etc.)
2. When a provider's circuit opens, fall back to an alternative provider
3. Periodically probe the original provider in half-open state
4. Resume using the original provider once it's healthy

---

## Token Bucket / Leaky Bucket

Rather than reacting to 429 errors, **pre-emptive rate limiting** avoids hitting limits in the first place.

### Token Bucket Algorithm

The token bucket is the most common approach for client-side rate limiting:

```
Bucket capacity: 800,000 tokens (matching your TPM limit)
Refill rate: 800,000 / 60 ≈ 13,333 tokens per second

Before each request:
  1. Estimate the token count (input + expected output)
  2. Check if the bucket has enough tokens
  3. If yes: deduct tokens and send request
  4. If no: wait until enough tokens have accumulated
```

```python
import time
import threading
import tiktoken

class TokenBucket:
    def __init__(self, capacity: int, refill_rate: float):
        self.capacity = capacity
        self.tokens = capacity
        self.refill_rate = refill_rate  # tokens per second
        self.last_refill = time.time()
        self.lock = threading.Lock()

    def _refill(self):
        now = time.time()
        elapsed = now - self.last_refill
        self.tokens = min(self.capacity, self.tokens + elapsed * self.refill_rate)
        self.last_refill = now

    def acquire(self, token_count: int, timeout: float = 60.0) -> bool:
        deadline = time.time() + timeout
        while time.time() < deadline:
            with self.lock:
                self._refill()
                if self.tokens >= token_count:
                    self.tokens -= token_count
                    return True
            time.sleep(0.1)
        return False

# Usage: rate limit to 800k TPM
bucket = TokenBucket(capacity=800_000, refill_rate=800_000 / 60)

def send_request(messages):
    enc = tiktoken.encoding_for_model("gpt-4o")
    token_estimate = sum(len(enc.encode(m["content"])) for m in messages)
    token_estimate += 500  # buffer for output tokens

    if bucket.acquire(token_estimate):
        return call_api(messages)
    else:
        raise TimeoutError("Rate limit bucket exhausted")
```

### Leaky Bucket Algorithm

The leaky bucket processes requests at a fixed rate, queuing excess requests:

```
Queue ──► [req4][req3][req2][req1] ──► Process at fixed rate ──► API
                                         (1 request per 12ms)
```

This smooths out bursts and ensures a steady request rate. It's ideal for batch processing where latency is less critical.

### Sliding Window Rate Limiting

Track actual usage over a rolling window rather than using token-based approximation:

```python
import time
from collections import deque

class SlidingWindowLimiter:
    def __init__(self, max_tokens: int, window_seconds: float = 60.0):
        self.max_tokens = max_tokens
        self.window = window_seconds
        self.usage = deque()  # (timestamp, token_count) pairs

    def can_send(self, token_count: int) -> bool:
        now = time.time()
        # Remove entries outside the window
        while self.usage and self.usage[0][0] < now - self.window:
            self.usage.popleft()
        current_usage = sum(t for _, t in self.usage)
        return current_usage + token_count <= self.max_tokens

    def record_usage(self, token_count: int):
        self.usage.append((time.time(), token_count))
```

### Token Counting Before Sending

Accurate pre-request token counting prevents surprises:

```python
import tiktoken

def count_tokens(messages: list, model: str = "gpt-4o") -> int:
    """Count tokens for a chat completion request."""
    encoding = tiktoken.encoding_for_model(model)
    num_tokens = 0
    for message in messages:
        num_tokens += 4  # message overhead
        for key, value in message.items():
            num_tokens += len(encoding.encode(str(value)))
            if key == "name":
                num_tokens += -1  # name token adjustment
    num_tokens += 2  # reply priming
    return num_tokens
```

For Anthropic, use their `anthropic.count_tokens()` method or the `anthropic-tokenizer` package.

---

## Provider Failover

When one provider is rate-limited or down, switching to an alternative provider keeps your agent running.

### Primary/Fallback Configuration

```python
from dataclasses import dataclass, field

@dataclass
class ProviderConfig:
    name: str
    api_key: str
    base_url: str
    model: str
    priority: int = 0
    is_healthy: bool = True
    max_retries: int = 3

@dataclass
class FailoverConfig:
    providers: list[ProviderConfig] = field(default_factory=list)

    def get_available_provider(self) -> ProviderConfig | None:
        """Return the highest-priority healthy provider."""
        healthy = [p for p in self.providers if p.is_healthy]
        if not healthy:
            return None
        return min(healthy, key=lambda p: p.priority)

config = FailoverConfig(providers=[
    ProviderConfig(
        name="openai-primary",
        api_key="sk-...",
        base_url="https://api.openai.com/v1",
        model="gpt-4o",
        priority=0,
    ),
    ProviderConfig(
        name="anthropic-fallback",
        api_key="sk-ant-...",
        base_url="https://api.anthropic.com/v1",
        model="claude-sonnet-4-20250514",
        priority=1,
    ),
    ProviderConfig(
        name="azure-failover",
        api_key="...",
        base_url="https://my-resource.openai.azure.com",
        model="gpt-4o",
        priority=2,
    ),
])
```

### Model-Level Fallbacks

When the primary model is rate-limited, fall back to a cheaper/faster model:

```python
MODEL_FALLBACK_CHAIN = {
    "gpt-4o": ["gpt-4o-mini", "gpt-3.5-turbo"],
    "claude-sonnet-4-20250514": ["claude-haiku-4-20250514"],
    "gemini-2.5-pro": ["gemini-2.0-flash"],
}

def get_fallback_model(model: str, attempt: int) -> str:
    chain = MODEL_FALLBACK_CHAIN.get(model, [])
    if attempt < len(chain):
        return chain[attempt]
    return model  # no more fallbacks, retry same model
```

**Trade-offs of model fallback:**
- Cheaper models may produce lower-quality code or miss nuances
- Different models have different tool-calling formats
- Context window sizes differ — your prompt may not fit the fallback model
- Token counting differs between model families

### Azure OpenAI as Failover

Azure OpenAI provides the same models via a separate infrastructure, making it an ideal failover:

```python
OPENAI_CONFIG = {
    "api_key": "sk-...",
    "base_url": "https://api.openai.com/v1",
}

AZURE_OPENAI_CONFIG = {
    "api_key": "...",
    "base_url": "https://my-resource.openai.azure.com/openai/deployments/gpt-4o",
    "api_version": "2024-10-21",
}
```

Azure has **separate rate limits** from the OpenAI API, so hitting limits on one doesn't affect the other. Azure also supports **Provisioned Throughput Units (PTUs)** for guaranteed capacity.

### Load Balancing Across Multiple API Keys

For high-throughput applications, distribute requests across multiple API keys:

```python
import itertools

class RoundRobinBalancer:
    def __init__(self, api_keys: list[str]):
        self.keys = api_keys
        self.cycle = itertools.cycle(api_keys)
        self.unhealthy = set()

    def get_next_key(self) -> str:
        for _ in range(len(self.keys)):
            key = next(self.cycle)
            if key not in self.unhealthy:
                return key
        raise Exception("All API keys are rate-limited")

    def mark_unhealthy(self, key: str):
        self.unhealthy.add(key)

    def mark_healthy(self, key: str):
        self.unhealthy.discard(key)
```

> **Warning:** Using multiple API keys to circumvent rate limits may violate provider terms of service. Check your agreement before implementing this pattern.

---

## How Coding Agents Handle 429s

### GitHub Copilot's Approach

GitHub Copilot operates behind a managed infrastructure layer, so individual users rarely see raw 429 errors. The system:

- Uses server-side rate limiting and queuing before requests reach the LLM provider
- Implements request prioritization (completions in active editor get priority over background suggestions)
- Manages capacity across a large fleet of API keys and Azure OpenAI deployments
- Degrades gracefully — slower suggestions rather than errors when at capacity
- Shows a status indicator in the IDE when suggestions are delayed or unavailable

### Cursor's Approach

Cursor handles rate limiting at multiple levels:

- **Server-side proxy:** Cursor routes all API calls through its own backend, which manages rate limits across providers
- **Request queuing:** When hitting limits, requests are queued rather than rejected
- **Model switching:** Cursor can automatically switch between models based on availability
- **Usage caps:** Cursor enforces per-user usage limits (e.g., "fast requests" quota on premium models) to prevent any single user from exhausting shared capacity
- **Graceful degradation:** Falls back from premium to standard models when fast-request quota is depleted

### Continue.dev's Approach

Continue.dev, being open-source, exposes retry configuration to the user:

```json
{
  "models": [
    {
      "title": "GPT-4o",
      "provider": "openai",
      "model": "gpt-4o",
      "requestOptions": {
        "timeout": 60,
        "retries": 3
      }
    }
  ]
}
```

- Users can configure multiple providers as fallbacks
- Retry logic uses exponential backoff with configurable parameters
- The open architecture allows users to implement custom retry logic via middleware
- Supports local models (Ollama, LM Studio) as unlimited-rate-limit fallbacks

### Aider's Retry Logic

Aider implements retry logic using the `litellm` library, which provides a unified interface across providers:

```python
# Aider uses litellm's built-in retry logic
# Simplified representation of Aider's approach:
from litellm import completion

response = completion(
    model="gpt-4o",
    messages=messages,
    num_retries=5,           # retry up to 5 times
    timeout=60,              # 60-second timeout per attempt
)
```

Key aspects of Aider's strategy:
- Uses `litellm` for automatic retry with exponential backoff on 429s and 5xx errors
- Displays retry status to the user in the terminal ("Retrying in X seconds...")
- Supports model fallback via command-line flags (`--model` and `--weak-model`)
- Allows users to configure multiple API keys for the same provider
- Tracks token usage and warns users when approaching limits

### Queue-Based Request Management

For agents making many concurrent requests, a queue-based approach provides the most control:

```python
import asyncio
from dataclasses import dataclass

@dataclass
class LLMRequest:
    messages: list
    model: str
    priority: int = 0  # lower = higher priority

class RequestQueue:
    def __init__(self, max_concurrent: int = 5, rpm_limit: int = 500):
        self.queue = asyncio.PriorityQueue()
        self.semaphore = asyncio.Semaphore(max_concurrent)
        self.rpm_limit = rpm_limit
        self.request_times = []

    async def submit(self, request: LLMRequest) -> dict:
        future = asyncio.Future()
        await self.queue.put((request.priority, request, future))
        return await future

    async def process_loop(self):
        while True:
            priority, request, future = await self.queue.get()
            async with self.semaphore:
                await self._wait_for_rate_limit()
                try:
                    result = await self._call_api(request)
                    future.set_result(result)
                except Exception as e:
                    future.set_exception(e)

    async def _wait_for_rate_limit(self):
        """Enforce RPM limit using sliding window."""
        import time
        now = time.time()
        self.request_times = [t for t in self.request_times if t > now - 60]
        if len(self.request_times) >= self.rpm_limit:
            wait_time = 60 - (now - self.request_times[0])
            await asyncio.sleep(wait_time)
        self.request_times.append(time.time())
```

---

## Code Examples

### Python: Retry Decorator with Exponential Backoff

A production-ready retry decorator that handles rate limits:

```python
import time
import random
import logging
import functools
from typing import Callable, Type

logger = logging.getLogger(__name__)

def retry_with_backoff(
    max_retries: int = 5,
    base_delay: float = 1.0,
    max_delay: float = 60.0,
    jitter: str = "equal",  # "none", "full", "equal", "decorrelated"
    retryable_status_codes: tuple = (429, 500, 502, 503, 504),
    retryable_exceptions: tuple = (ConnectionError, TimeoutError),
):
    """Retry decorator with configurable exponential backoff and jitter."""
    def decorator(func: Callable) -> Callable:
        @functools.wraps(func)
        def wrapper(*args, **kwargs):
            last_delay = base_delay
            for attempt in range(max_retries + 1):
                try:
                    response = func(*args, **kwargs)

                    if hasattr(response, "status_code"):
                        if response.status_code not in retryable_status_codes:
                            return response

                        if attempt == max_retries:
                            return response

                        # Use Retry-After header if available
                        retry_after = response.headers.get("retry-after")
                        if retry_after:
                            delay = float(retry_after)
                        else:
                            delay = _calculate_delay(
                                attempt, base_delay, max_delay, jitter, last_delay
                            )
                        last_delay = delay

                        logger.warning(
                            f"Rate limited (attempt {attempt + 1}/{max_retries}). "
                            f"Retrying in {delay:.1f}s..."
                        )
                        time.sleep(delay)
                    else:
                        return response

                except retryable_exceptions as e:
                    if attempt == max_retries:
                        raise
                    delay = _calculate_delay(
                        attempt, base_delay, max_delay, jitter, last_delay
                    )
                    last_delay = delay
                    logger.warning(
                        f"Request failed ({e}). Retrying in {delay:.1f}s..."
                    )
                    time.sleep(delay)

            raise Exception(f"Max retries ({max_retries}) exceeded")
        return wrapper
    return decorator

def _calculate_delay(
    attempt: int, base: float, max_delay: float, jitter: str, last_delay: float
) -> float:
    if jitter == "none":
        delay = base * (2 ** attempt)
    elif jitter == "full":
        delay = random.uniform(0, base * (2 ** attempt))
    elif jitter == "equal":
        exp_delay = base * (2 ** attempt)
        half = exp_delay / 2
        delay = half + random.uniform(0, half)
    elif jitter == "decorrelated":
        delay = random.uniform(base, last_delay * 3)
    else:
        delay = base * (2 ** attempt)
    return min(delay, max_delay)

# Usage:
@retry_with_backoff(max_retries=5, base_delay=1.0, jitter="equal")
def call_openai(messages):
    import httpx
    return httpx.post(
        "https://api.openai.com/v1/chat/completions",
        headers={"Authorization": "Bearer sk-..."},
        json={"model": "gpt-4o", "messages": messages},
        timeout=60.0,
    )
```

### TypeScript: Retry Wrapper

```typescript
interface RetryOptions {
  maxRetries: number;
  baseDelay: number;
  maxDelay: number;
  jitter: "none" | "full" | "equal";
  retryableStatusCodes: number[];
}

const DEFAULT_OPTIONS: RetryOptions = {
  maxRetries: 5,
  baseDelay: 1000,
  maxDelay: 60000,
  jitter: "equal",
  retryableStatusCodes: [429, 500, 502, 503, 504],
};

function calculateDelay(attempt: number, options: RetryOptions): number {
  const expDelay = options.baseDelay * Math.pow(2, attempt);

  let delay: number;
  switch (options.jitter) {
    case "full":
      delay = Math.random() * expDelay;
      break;
    case "equal":
      delay = expDelay / 2 + Math.random() * (expDelay / 2);
      break;
    default:
      delay = expDelay;
  }

  return Math.min(delay, options.maxDelay);
}

async function fetchWithRetry(
  url: string,
  init: RequestInit,
  options: Partial<RetryOptions> = {}
): Promise<Response> {
  const opts = { ...DEFAULT_OPTIONS, ...options };

  for (let attempt = 0; attempt <= opts.maxRetries; attempt++) {
    const response = await fetch(url, init);

    if (!opts.retryableStatusCodes.includes(response.status)) {
      return response;
    }

    if (attempt === opts.maxRetries) {
      return response;
    }

    // Prefer Retry-After header
    const retryAfter = response.headers.get("retry-after");
    const delay = retryAfter
      ? parseFloat(retryAfter) * 1000
      : calculateDelay(attempt, opts);

    console.warn(
      `Rate limited (attempt ${attempt + 1}/${opts.maxRetries}). ` +
        `Retrying in ${(delay / 1000).toFixed(1)}s...`
    );

    await new Promise((resolve) => setTimeout(resolve, delay));
  }

  throw new Error(`Max retries (${opts.maxRetries}) exceeded`);
}

// Usage:
const response = await fetchWithRetry(
  "https://api.openai.com/v1/chat/completions",
  {
    method: "POST",
    headers: {
      "Content-Type": "application/json",
      Authorization: `Bearer ${apiKey}`,
    },
    body: JSON.stringify({ model: "gpt-4o", messages }),
  },
  { maxRetries: 5, jitter: "equal" }
);
```

### Circuit Breaker with Provider Failover (Python)

```python
import time
import logging
from dataclasses import dataclass, field

logger = logging.getLogger(__name__)

@dataclass
class Provider:
    name: str
    call: callable
    circuit: "CircuitBreaker" = field(default_factory=lambda: CircuitBreaker())

class ResilientLLMClient:
    """LLM client with circuit breakers and provider failover."""

    def __init__(self, providers: list[Provider]):
        self.providers = providers

    def complete(self, messages: list) -> dict:
        errors = []
        for provider in self.providers:
            if not provider.circuit.can_execute():
                logger.info(f"Skipping {provider.name} (circuit open)")
                continue
            try:
                result = provider.call(messages)
                provider.circuit.record_success()
                return result
            except RateLimitError as e:
                provider.circuit.record_failure()
                errors.append((provider.name, e))
                logger.warning(f"{provider.name} rate limited: {e}")
            except Exception as e:
                provider.circuit.record_failure()
                errors.append((provider.name, e))
                logger.error(f"{provider.name} failed: {e}")

        raise AllProvidersFailedError(
            f"All providers failed: {errors}"
        )
```

---

## Context Window Limits

Rate limits are not the only constraint — **context window limits** bound how much data you can send in a single request.

### Model Context Windows

| Model | Context Window | Max Output Tokens |
|---|---|---|
| GPT-4o | 128,000 | 16,384 |
| GPT-4o mini | 128,000 | 16,384 |
| Claude 3.5 Sonnet | 200,000 | 8,192 |
| Claude 3.5 Haiku | 200,000 | 8,192 |
| Gemini 2.5 Pro | 1,000,000 | 65,536 |
| Gemini 2.0 Flash | 1,000,000 | 8,192 |

### Truncation Strategies

When conversation history exceeds the context window:

**1. Simple Truncation (Drop Oldest)**

```python
def truncate_messages(messages: list, max_tokens: int, model: str) -> list:
    """Keep system message + most recent messages that fit."""
    system = [m for m in messages if m["role"] == "system"]
    non_system = [m for m in messages if m["role"] != "system"]

    system_tokens = count_tokens(system, model)
    available = max_tokens - system_tokens - 500  # buffer for output

    result = []
    total = 0
    for msg in reversed(non_system):
        msg_tokens = count_tokens([msg], model)
        if total + msg_tokens > available:
            break
        result.insert(0, msg)
        total += msg_tokens

    return system + result
```

**2. Sliding Window with Summarization**

```python
async def manage_context(messages: list, max_tokens: int) -> list:
    """Summarize old messages when context is getting full."""
    current_tokens = count_tokens(messages)

    if current_tokens < max_tokens * 0.8:
        return messages  # still have room

    system = messages[0]  # keep system prompt
    old_messages = messages[1:-10]  # summarize older messages
    recent_messages = messages[-10:]  # keep recent 10 messages

    summary = await summarize_conversation(old_messages)
    summary_message = {
        "role": "system",
        "content": f"Summary of earlier conversation:\n{summary}"
    }

    return [system, summary_message] + recent_messages
```

**3. Priority-Based Truncation**

For coding agents, prioritize keeping:
1. System prompt (always keep)
2. Most recent user message and assistant response
3. Tool call results (code output, file contents)
4. Earlier tool calls (can be summarized)
5. Oldest conversation turns (drop first)

---

## Cost Control

Rate limits and cost control are deeply related — higher spend unlocks higher rate limits, but uncontrolled spend is a real risk.

### Setting Spending Limits

Most providers allow setting monthly spending caps:

- **OpenAI:** Set in Organization Settings → Billing → Usage Limits
  - Hard limit: API stops working when reached
  - Soft limit: Email notification when reached
- **Anthropic:** Set in Workspace Settings → Plans & Billing
- **Google:** Set budget alerts in Google Cloud Console

### Token Usage Monitoring

```python
import time
from dataclasses import dataclass, field

@dataclass
class UsageTracker:
    """Track token usage and costs across requests."""
    total_input_tokens: int = 0
    total_output_tokens: int = 0
    total_cost: float = 0.0
    request_count: int = 0
    history: list = field(default_factory=list)

    # Cost per 1M tokens (example rates)
    PRICING = {
        "gpt-4o": {"input": 2.50, "output": 10.00},
        "gpt-4o-mini": {"input": 0.15, "output": 0.60},
        "claude-3-5-sonnet": {"input": 3.00, "output": 15.00},
        "claude-3-5-haiku": {"input": 0.80, "output": 4.00},
    }

    def record(self, model: str, input_tokens: int, output_tokens: int):
        pricing = self.PRICING.get(model, {"input": 0, "output": 0})
        cost = (
            input_tokens * pricing["input"] / 1_000_000
            + output_tokens * pricing["output"] / 1_000_000
        )
        self.total_input_tokens += input_tokens
        self.total_output_tokens += output_tokens
        self.total_cost += cost
        self.request_count += 1
        self.history.append({
            "timestamp": time.time(),
            "model": model,
            "input_tokens": input_tokens,
            "output_tokens": output_tokens,
            "cost": cost,
        })

    def check_budget(self, budget: float) -> bool:
        """Return True if under budget."""
        return self.total_cost < budget

    def summary(self) -> str:
        return (
            f"Requests: {self.request_count} | "
            f"Input: {self.total_input_tokens:,} tokens | "
            f"Output: {self.total_output_tokens:,} tokens | "
            f"Cost: ${self.total_cost:.4f}"
        )
```

### Budget Alerts

Implement real-time budget alerting in your agent:

```python
class BudgetGuard:
    def __init__(self, daily_budget: float, alert_threshold: float = 0.8):
        self.daily_budget = daily_budget
        self.alert_threshold = alert_threshold
        self.tracker = UsageTracker()

    def pre_request_check(self, estimated_cost: float) -> bool:
        if self.tracker.total_cost + estimated_cost > self.daily_budget:
            logger.critical(
                f"BUDGET EXCEEDED: ${self.tracker.total_cost:.2f} / "
                f"${self.daily_budget:.2f}"
            )
            return False
        if self.tracker.total_cost > self.daily_budget * self.alert_threshold:
            logger.warning(
                f"Budget alert: {self.tracker.total_cost / self.daily_budget:.0%} "
                f"of daily budget used"
            )
        return True
```

---

## Best Practices for Production Deployments

### 1. Layer Your Defenses

Implement rate limit handling at multiple layers:

```
Application Layer:  Budget guards, usage tracking
    ↓
Client Layer:       Token bucket (pre-emptive), request queue
    ↓
Retry Layer:        Exponential backoff with jitter
    ↓
Circuit Breaker:    Provider-level health tracking
    ↓
Failover Layer:     Provider/model fallback chain
```

### 2. Respect Provider Signals

- **Always check `Retry-After` headers** before applying your own backoff
- **Monitor `x-ratelimit-remaining-*` headers** on successful responses to slow down proactively
- **Parse error messages** for specific limit information

### 3. Design for Graceful Degradation

| Scenario | Response |
|---|---|
| Primary model rate-limited | Fall back to cheaper model |
| All models rate-limited | Queue requests, increase delay |
| Provider fully down | Switch to alternative provider |
| All providers down | Return cached/partial results |
| Budget exceeded | Refuse new requests with clear message |

### 4. Avoid Common Mistakes

- **Don't retry on 400/401/403** — these are not transient errors
- **Don't use the same delay for all clients** — add jitter
- **Don't ignore response headers** — they tell you exactly when to retry
- **Don't retry indefinitely** — cap retries and total timeout
- **Don't assume limits are static** — providers change limits frequently
- **Don't count only input tokens** — output tokens count toward TPM too

### 5. Test Your Retry Logic

Mock rate-limit responses in your test suite:

```python
import pytest
from unittest.mock import patch, MagicMock

def test_retry_on_429():
    mock_responses = [
        MagicMock(status_code=429, headers={"retry-after": "1"}),
        MagicMock(status_code=429, headers={"retry-after": "1"}),
        MagicMock(status_code=200, json=lambda: {"choices": [...]}),
    ]
    with patch("httpx.post", side_effect=mock_responses):
        result = call_with_retry(messages=[...])
        assert result.status_code == 200
```

---

## Monitoring and Observability

### Key Metrics to Track

| Metric | Why It Matters |
|---|---|
| `llm.request.count` | Overall request volume |
| `llm.request.latency_ms` | Detect slowdowns before 429s |
| `llm.request.status_code` | Track 429 rate over time |
| `llm.retry.count` | How often retries happen |
| `llm.retry.exhausted` | How often all retries fail |
| `llm.tokens.input` | Input token consumption rate |
| `llm.tokens.output` | Output token consumption rate |
| `llm.cost.dollars` | Real-time cost tracking |
| `llm.circuit_breaker.state` | Circuit breaker transitions |
| `llm.failover.count` | How often failover is triggered |
| `llm.queue.depth` | Pending requests in queue |
| `llm.queue.wait_time_ms` | Time requests wait in queue |

### Structured Logging

```python
import structlog

logger = structlog.get_logger()

def log_llm_request(model, status_code, tokens_used, latency_ms, attempt):
    logger.info(
        "llm_request",
        model=model,
        status_code=status_code,
        input_tokens=tokens_used["input"],
        output_tokens=tokens_used["output"],
        latency_ms=latency_ms,
        retry_attempt=attempt,
    )
```

### Dashboards and Alerts

Set up alerts for:

- **429 rate > 10% of requests** — You're hitting limits regularly; consider tier upgrade or request reduction
- **P99 latency > 30s** — Retries and queuing are degrading user experience
- **Circuit breaker open for > 5 minutes** — A provider may be experiencing an extended outage
- **Daily cost > 80% of budget** — Approaching spend limit, may need to throttle
- **Retry exhaustion rate > 5%** — Requests are failing after all retries; systemic issue

### OpenTelemetry Integration

```python
from opentelemetry import trace, metrics

tracer = trace.get_tracer("llm-client")
meter = metrics.get_meter("llm-client")

request_counter = meter.create_counter("llm.requests")
retry_counter = meter.create_counter("llm.retries")
latency_histogram = meter.create_histogram("llm.latency")
token_counter = meter.create_counter("llm.tokens")

async def traced_llm_call(messages, model):
    with tracer.start_as_current_span("llm.chat_completion") as span:
        span.set_attribute("llm.model", model)
        span.set_attribute("llm.provider", "openai")

        start = time.time()
        response = await call_api(messages, model)
        latency = (time.time() - start) * 1000

        request_counter.add(1, {"model": model, "status": response.status_code})
        latency_histogram.record(latency, {"model": model})

        if response.status_code == 200:
            usage = response.json()["usage"]
            token_counter.add(usage["prompt_tokens"], {"type": "input", "model": model})
            token_counter.add(usage["completion_tokens"], {"type": "output", "model": model})

        return response
```

---

## Summary

Handling rate limits in production LLM applications requires a multi-layered approach:

1. **Understand your limits** — Know your tier, monitor headers, track usage
2. **Prevent hitting limits** — Token buckets, request queuing, pre-counting tokens
3. **Handle 429s gracefully** — Exponential backoff with jitter, respect `Retry-After`
4. **Protect your system** — Circuit breakers prevent cascading failures
5. **Have fallbacks ready** — Provider failover, model degradation, cached responses
6. **Monitor everything** — Track metrics, set alerts, log structured data
7. **Control costs** — Budget guards, usage tracking, spending caps

The best rate-limiting strategy is one you never notice — it works silently in the background, keeping your agent running smoothly while respecting provider constraints.