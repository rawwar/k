# Common API Integration Patterns for Coding Agents

## Overview

Every CLI coding agent must implement a robust API integration layer that handles
the realities of calling LLM APIs in production: network failures, rate limits,
timeouts, streaming interruptions, and provider-specific quirks. This document
catalogs the most important patterns used across the 17 agents studied, providing
reusable implementations for retry logic, error handling, streaming, connection
management, and more.

---

## Retry Logic and Exponential Backoff

### The Standard Pattern

Every agent needs retry logic. The universally adopted approach is exponential
backoff with jitter:

```python
import time
import random
from typing import TypeVar, Callable

T = TypeVar('T')

def retry_with_backoff(
    fn: Callable[..., T],
    max_retries: int = 5,
    base_delay: float = 1.0,
    max_delay: float = 60.0,
    jitter: bool = True,
    retryable_exceptions: tuple = (RateLimitError, ServiceUnavailableError, Timeout),
) -> T:
    """Retry a function with exponential backoff and optional jitter.
    
    Delay sequence (no jitter): 1s, 2s, 4s, 8s, 16s
    Delay sequence (with jitter): 0.5-1.5s, 1-3s, 2-6s, 4-12s, 8-24s
    """
    last_exception = None
    
    for attempt in range(max_retries + 1):
        try:
            return fn()
        except retryable_exceptions as e:
            last_exception = e
            
            if attempt == max_retries:
                raise
            
            # Calculate delay with exponential backoff
            delay = min(base_delay * (2 ** attempt), max_delay)
            
            # Add jitter to prevent thundering herd
            if jitter:
                delay = delay * (0.5 + random.random())
            
            # Respect Retry-After header if present
            retry_after = getattr(e, 'retry_after', None)
            if retry_after:
                delay = max(delay, float(retry_after))
            
            time.sleep(delay)
    
    raise last_exception
```

### Async Version

```python
import asyncio

async def async_retry_with_backoff(
    fn,
    max_retries: int = 5,
    base_delay: float = 1.0,
    max_delay: float = 60.0,
    retryable_exceptions: tuple = (RateLimitError, ServiceUnavailableError),
):
    """Async retry with exponential backoff."""
    for attempt in range(max_retries + 1):
        try:
            return await fn()
        except retryable_exceptions as e:
            if attempt == max_retries:
                raise
            
            delay = min(base_delay * (2 ** attempt), max_delay)
            delay *= (0.5 + random.random())  # Jitter
            
            await asyncio.sleep(delay)
```

### Provider-Specific Retry Behavior

| Provider | Rate Limit Response | Retry-After Header | Recommended Strategy |
|----------|-------------------|--------------------|---------------------|
| OpenAI | 429 + `retry-after` | ✅ (seconds) | Honor header, then backoff |
| Anthropic | 429 + `retry-after` | ✅ (seconds) | Honor header, then backoff |
| Google | 429 + quota info | ✅ | Honor header, longer waits |
| DeepSeek | 429 | Sometimes | Exponential backoff |
| Ollama (local) | N/A (no rate limits) | N/A | No retry needed |

---

## Rate Limit Handling

### Understanding Rate Limits

LLM providers enforce rate limits across multiple dimensions:

| Dimension | OpenAI | Anthropic | Google |
|-----------|--------|-----------|--------|
| **Requests Per Minute (RPM)** | 500-10,000 | 1,000-4,000 | 60-1,000 |
| **Tokens Per Minute (TPM)** | 30K-10M | 40K-400K | Varies |
| **Tokens Per Day (TPD)** | Unlimited | Varies by tier | Varies |
| **Concurrent Requests** | Tier-based | Varies | Varies |

### Proactive Rate Limiting

Don't wait for 429 errors—track your usage and self-throttle:

```python
import time
from collections import deque
from threading import Lock

class RateLimiter:
    """Token bucket rate limiter for LLM API calls."""
    
    def __init__(self, rpm: int = 500, tpm: int = 100000):
        self.rpm = rpm
        self.tpm = tpm
        self.request_times = deque()
        self.token_counts = deque()
        self.lock = Lock()
    
    def wait_if_needed(self, estimated_tokens: int = 1000):
        """Block until we're within rate limits."""
        with self.lock:
            now = time.time()
            window = 60.0  # 1 minute window
            
            # Clean old entries
            while self.request_times and self.request_times[0] < now - window:
                self.request_times.popleft()
            while self.token_counts and self.token_counts[0][0] < now - window:
                self.token_counts.popleft()
            
            # Check RPM
            if len(self.request_times) >= self.rpm:
                wait_until = self.request_times[0] + window
                time.sleep(max(0, wait_until - now))
            
            # Check TPM
            current_tokens = sum(tc[1] for tc in self.token_counts)
            if current_tokens + estimated_tokens > self.tpm:
                wait_until = self.token_counts[0][0] + window
                time.sleep(max(0, wait_until - now))
            
            # Record this request
            self.request_times.append(time.time())
            self.token_counts.append((time.time(), estimated_tokens))
    
    def record_usage(self, actual_tokens: int):
        """Update with actual token count after response."""
        with self.lock:
            if self.token_counts:
                # Replace estimated with actual
                self.token_counts[-1] = (time.time(), actual_tokens)
```

### Reading Rate Limit Headers

```python
def extract_rate_limit_info(response_headers: dict) -> dict:
    """Extract rate limit information from response headers."""
    return {
        "limit_requests": int(response_headers.get("x-ratelimit-limit-requests", 0)),
        "limit_tokens": int(response_headers.get("x-ratelimit-limit-tokens", 0)),
        "remaining_requests": int(response_headers.get("x-ratelimit-remaining-requests", 0)),
        "remaining_tokens": int(response_headers.get("x-ratelimit-remaining-tokens", 0)),
        "reset_requests": response_headers.get("x-ratelimit-reset-requests", ""),
        "reset_tokens": response_headers.get("x-ratelimit-reset-tokens", ""),
    }

# Use this info to adjust behavior
info = extract_rate_limit_info(response.headers)
if info["remaining_requests"] < 10:
    # Getting close to limit — slow down
    time.sleep(2.0)
```

---

## Streaming vs. Non-Streaming

### When to Stream

| Scenario | Stream? | Why |
|----------|---------|-----|
| Interactive terminal output | ✅ Yes | User sees progress immediately |
| Agentic loop (tool calls) | ✅ Yes | Start processing tool calls early |
| Batch processing | ❌ No | Simpler code, same latency |
| Cost estimation | ❌ No | Full usage info in one response |
| Tests/evaluation | ❌ No | Simpler assertion logic |

### Streaming Implementation (OpenAI Format)

```python
import json
from dataclasses import dataclass, field

@dataclass
class StreamCollector:
    """Collect streaming chunks into a complete response."""
    content: str = ""
    tool_calls: dict = field(default_factory=dict)
    finish_reason: str | None = None
    usage: dict = field(default_factory=dict)
    
    def process_chunk(self, chunk):
        """Process a single SSE chunk."""
        if not chunk.choices:
            # Final chunk with usage info
            if hasattr(chunk, 'usage') and chunk.usage:
                self.usage = {
                    "prompt_tokens": chunk.usage.prompt_tokens,
                    "completion_tokens": chunk.usage.completion_tokens,
                    "total_tokens": chunk.usage.total_tokens,
                }
            return
        
        delta = chunk.choices[0].delta
        
        # Collect text content
        if delta.content:
            self.content += delta.content
        
        # Collect tool calls (streamed incrementally)
        if delta.tool_calls:
            for tc in delta.tool_calls:
                idx = tc.index
                if idx not in self.tool_calls:
                    self.tool_calls[idx] = {
                        "id": tc.id or "",
                        "function": {"name": "", "arguments": ""}
                    }
                if tc.id:
                    self.tool_calls[idx]["id"] = tc.id
                if tc.function:
                    if tc.function.name:
                        self.tool_calls[idx]["function"]["name"] = tc.function.name
                    if tc.function.arguments:
                        self.tool_calls[idx]["function"]["arguments"] += tc.function.arguments
        
        if chunk.choices[0].finish_reason:
            self.finish_reason = chunk.choices[0].finish_reason
    
    def get_tool_calls(self):
        """Parse collected tool calls."""
        return [
            {
                "id": tc["id"],
                "type": "function",
                "function": {
                    "name": tc["function"]["name"],
                    "arguments": json.loads(tc["function"]["arguments"])
                }
            }
            for tc in sorted(self.tool_calls.values(), key=lambda x: x["id"])
        ]


# Usage in an agent
async def stream_response(client, messages, tools):
    collector = StreamCollector()
    
    stream = await client.chat.completions.create(
        model="gpt-4.1",
        messages=messages,
        tools=tools,
        stream=True,
        stream_options={"include_usage": True}  # Get token counts
    )
    
    async for chunk in stream:
        collector.process_chunk(chunk)
        
        # Print text as it arrives
        if chunk.choices and chunk.choices[0].delta.content:
            print(chunk.choices[0].delta.content, end="", flush=True)
    
    return collector
```

### Streaming with Anthropic (Different Event Structure)

```python
async def stream_anthropic(client, messages, tools):
    """Anthropic uses a different streaming format."""
    content_blocks = {}
    
    async with client.messages.stream(
        model="claude-sonnet-4-6",
        max_tokens=4096,
        messages=messages,
        tools=tools
    ) as stream:
        async for event in stream:
            if event.type == "content_block_start":
                idx = event.index
                if event.content_block.type == "text":
                    content_blocks[idx] = {"type": "text", "text": ""}
                elif event.content_block.type == "tool_use":
                    content_blocks[idx] = {
                        "type": "tool_use",
                        "id": event.content_block.id,
                        "name": event.content_block.name,
                        "input": ""
                    }
            
            elif event.type == "content_block_delta":
                idx = event.index
                if event.delta.type == "text_delta":
                    content_blocks[idx]["text"] += event.delta.text
                    print(event.delta.text, end="", flush=True)
                elif event.delta.type == "input_json_delta":
                    content_blocks[idx]["input"] += event.delta.partial_json
            
            elif event.type == "message_delta":
                if event.delta.stop_reason:
                    pass  # Message complete
    
    return content_blocks
```

---

## Timeout Strategies

### Multi-Level Timeouts

```python
from dataclasses import dataclass

@dataclass
class TimeoutConfig:
    """Timeout configuration for LLM API calls."""
    
    # Connection timeout — how long to wait for TCP connection
    connect_timeout: float = 5.0
    
    # Read timeout — how long to wait for first byte of response
    read_timeout: float = 30.0
    
    # Total timeout — maximum time for entire request
    total_timeout: float = 120.0
    
    # Stream timeout — max time between chunks during streaming
    stream_chunk_timeout: float = 30.0
    
    # Tool execution timeout — max time for a single tool call
    tool_timeout: float = 60.0
    
    # Turn timeout — max time for one full agent turn
    turn_timeout: float = 300.0


# Model-specific timeouts
TIMEOUT_CONFIGS = {
    "gpt-4.1": TimeoutConfig(
        read_timeout=15.0,
        total_timeout=60.0
    ),
    "claude-sonnet-4-6": TimeoutConfig(
        read_timeout=20.0,
        total_timeout=90.0
    ),
    "claude-opus-4-6": TimeoutConfig(
        read_timeout=30.0,
        total_timeout=180.0  # Opus is slower
    ),
    "o3": TimeoutConfig(
        read_timeout=60.0,
        total_timeout=300.0  # Reasoning takes longer
    ),
    "ollama/*": TimeoutConfig(
        connect_timeout=10.0,
        read_timeout=60.0,  # Local inference can be slow
        total_timeout=300.0
    ),
}
```

### Stream Watchdog

Detect and handle stalled streams:

```python
import asyncio

async def stream_with_watchdog(stream, chunk_timeout: float = 30.0):
    """Monitor a stream and raise timeout if chunks stop arriving."""
    async for chunk in asyncio.wait_for(
        stream.__anext__(), timeout=chunk_timeout
    ):
        yield chunk
    
# Usage
try:
    async for chunk in stream_with_watchdog(api_stream, chunk_timeout=30):
        process_chunk(chunk)
except asyncio.TimeoutError:
    # Stream stalled — retry or fallback
    pass
```

---

## Error Handling Across Providers

### Unified Error Taxonomy

```python
class LLMError(Exception):
    """Base class for LLM errors."""
    pass

class AuthenticationError(LLMError):
    """Invalid API key or credentials."""
    retryable = False

class RateLimitError(LLMError):
    """Too many requests."""
    retryable = True

class ContextWindowExceeded(LLMError):
    """Input exceeds model's context window."""
    retryable = False  # But can retry with truncated input

class ContentFilterError(LLMError):
    """Content blocked by safety filters."""
    retryable = False

class ServerError(LLMError):
    """Provider server error (5xx)."""
    retryable = True

class ModelNotFoundError(LLMError):
    """Requested model doesn't exist."""
    retryable = False

class InsufficientQuotaError(LLMError):
    """Account balance depleted."""
    retryable = False

class TimeoutError(LLMError):
    """Request timed out."""
    retryable = True
```

### Provider Error Mapping

```python
def map_openai_error(error) -> LLMError:
    """Map OpenAI SDK errors to our unified error types."""
    from openai import (
        AuthenticationError as OAIAuthError,
        RateLimitError as OAIRateLimitError,
        BadRequestError as OAIBadRequestError,
        APIError as OAIAPIError,
    )
    
    if isinstance(error, OAIAuthError):
        return AuthenticationError(str(error))
    elif isinstance(error, OAIRateLimitError):
        if "insufficient_quota" in str(error):
            return InsufficientQuotaError(str(error))
        return RateLimitError(str(error))
    elif isinstance(error, OAIBadRequestError):
        if "context_length_exceeded" in str(error):
            return ContextWindowExceeded(str(error))
        if "content_filter" in str(error):
            return ContentFilterError(str(error))
        return LLMError(str(error))
    elif isinstance(error, OAIAPIError):
        if error.status_code and error.status_code >= 500:
            return ServerError(str(error))
        return LLMError(str(error))
    
    return LLMError(str(error))


def map_anthropic_error(error) -> LLMError:
    """Map Anthropic SDK errors to our unified error types."""
    import anthropic
    
    if isinstance(error, anthropic.AuthenticationError):
        return AuthenticationError(str(error))
    elif isinstance(error, anthropic.RateLimitError):
        return RateLimitError(str(error))
    elif isinstance(error, anthropic.BadRequestError):
        if "prompt is too long" in str(error):
            return ContextWindowExceeded(str(error))
        return LLMError(str(error))
    elif isinstance(error, anthropic.InternalServerError):
        return ServerError(str(error))
    
    return LLMError(str(error))
```

### Context Window Recovery

When input exceeds the context window, agents can recover:

```python
async def call_with_context_recovery(messages, model, tools):
    """Handle context window overflow gracefully."""
    try:
        return await call_llm(messages, model, tools)
    except ContextWindowExceeded:
        # Strategy 1: Truncate old messages
        truncated = truncate_conversation(messages, target_tokens=100000)
        try:
            return await call_llm(truncated, model, tools)
        except ContextWindowExceeded:
            pass
        
        # Strategy 2: Summarize conversation
        summary = await summarize_messages(messages[:- 4])
        condensed = [
            {"role": "system", "content": f"Previous context summary: {summary}"},
            *messages[-4:]
        ]
        try:
            return await call_llm(condensed, model, tools)
        except ContextWindowExceeded:
            pass
        
        # Strategy 3: Switch to larger context model
        return await call_llm(messages, "gemini-2.5-pro", tools)
```

---

## Connection Management

### Connection Pooling

```python
import httpx

class LLMClient:
    """HTTP client with connection pooling for LLM APIs."""
    
    def __init__(self):
        self.clients = {}
    
    def get_client(self, base_url: str) -> httpx.AsyncClient:
        if base_url not in self.clients:
            self.clients[base_url] = httpx.AsyncClient(
                base_url=base_url,
                timeout=httpx.Timeout(
                    connect=5.0,
                    read=30.0,
                    write=10.0,
                    pool=10.0
                ),
                limits=httpx.Limits(
                    max_connections=20,
                    max_keepalive_connections=10,
                    keepalive_expiry=30.0
                ),
                http2=True  # HTTP/2 for multiplexing
            )
        return self.clients[base_url]
    
    async def close(self):
        for client in self.clients.values():
            await client.aclose()
```

### HTTP/2 Benefits

HTTP/2 provides significant advantages for LLM API calls:

| Feature | HTTP/1.1 | HTTP/2 |
|---------|----------|--------|
| Connection per request | Yes | Multiplexed |
| Header compression | No | HPACK |
| Server push | No | Yes |
| Stream priority | No | Yes |
| **Impact** | Higher latency | Lower latency, fewer connections |

---

## Request/Response Logging

### Structured Logging for Debugging

```python
import json
import time
import logging

logger = logging.getLogger("llm_api")

class APILogger:
    """Log API calls for debugging and cost tracking."""
    
    def log_request(self, model: str, messages: list, tools: list = None):
        logger.info(json.dumps({
            "event": "llm_request",
            "model": model,
            "message_count": len(messages),
            "tool_count": len(tools or []),
            "estimated_input_tokens": self._estimate_tokens(messages),
            "timestamp": time.time()
        }))
    
    def log_response(self, model: str, response, duration: float, cost: float):
        logger.info(json.dumps({
            "event": "llm_response",
            "model": model,
            "output_tokens": response.usage.completion_tokens,
            "input_tokens": response.usage.prompt_tokens,
            "duration_seconds": round(duration, 3),
            "cost_usd": round(cost, 6),
            "finish_reason": response.choices[0].finish_reason,
            "has_tool_calls": bool(response.choices[0].message.tool_calls),
            "timestamp": time.time()
        }))
    
    def log_error(self, model: str, error: Exception, attempt: int):
        logger.warning(json.dumps({
            "event": "llm_error",
            "model": model,
            "error_type": type(error).__name__,
            "error_message": str(error)[:500],
            "attempt": attempt,
            "retryable": getattr(error, 'retryable', False),
            "timestamp": time.time()
        }))
```

---

## Provider Abstraction Pattern

### The Interface

Most agents define a common interface that all providers must implement:

```python
from abc import ABC, abstractmethod
from typing import AsyncIterator

class LLMProvider(ABC):
    """Abstract base class for LLM providers."""
    
    @abstractmethod
    async def complete(
        self,
        messages: list[dict],
        tools: list[dict] | None = None,
        stream: bool = False,
        **kwargs
    ) -> dict | AsyncIterator[dict]:
        """Send a completion request to the provider."""
        pass
    
    @abstractmethod
    def count_tokens(self, text: str) -> int:
        """Count tokens for the provider's tokenizer."""
        pass
    
    @abstractmethod
    def max_context_tokens(self) -> int:
        """Return the maximum context window size."""
        pass
    
    @abstractmethod
    def supports_tools(self) -> bool:
        """Whether this provider supports native function calling."""
        pass


class OpenAIProvider(LLMProvider):
    async def complete(self, messages, tools=None, stream=False, **kwargs):
        return await self.client.chat.completions.create(
            model=self.model, messages=messages, tools=tools, stream=stream, **kwargs
        )
    
    def count_tokens(self, text):
        return len(self.encoding.encode(text))
    
    def max_context_tokens(self):
        return {"gpt-4.1": 1048576, "gpt-4o": 128000}.get(self.model, 128000)
    
    def supports_tools(self):
        return True


class AnthropicProvider(LLMProvider):
    async def complete(self, messages, tools=None, stream=False, **kwargs):
        # Translate OpenAI format to Anthropic format
        system = self._extract_system(messages)
        anthropic_messages = self._translate_messages(messages)
        anthropic_tools = self._translate_tools(tools)
        
        if stream:
            return self.client.messages.stream(
                model=self.model, system=system,
                messages=anthropic_messages, tools=anthropic_tools,
                max_tokens=kwargs.get("max_tokens", 4096)
            )
        else:
            response = await self.client.messages.create(
                model=self.model, system=system,
                messages=anthropic_messages, tools=anthropic_tools,
                max_tokens=kwargs.get("max_tokens", 4096)
            )
            return self._translate_response(response)
```

---

## Idempotency and Deduplication

### Preventing Duplicate Tool Executions

In agentic loops, network retries can cause the same tool call to be executed twice:

```python
class IdempotentToolExecutor:
    """Execute tools at most once per tool call ID."""
    
    def __init__(self):
        self.executed = {}  # tool_call_id -> result
    
    async def execute(self, tool_call_id: str, function_name: str, arguments: dict):
        """Execute a tool call, returning cached result if already executed."""
        if tool_call_id in self.executed:
            return self.executed[tool_call_id]
        
        result = await self._execute(function_name, arguments)
        self.executed[tool_call_id] = result
        return result
```

---

## Health Checking

### Provider Health Monitor

```python
class ProviderHealthChecker:
    """Monitor provider health and route away from unhealthy providers."""
    
    def __init__(self, providers: list[str]):
        self.providers = providers
        self.health = {p: {"healthy": True, "last_check": 0, "failures": 0} for p in providers}
    
    async def check_health(self, provider: str) -> bool:
        """Quick health check — send a minimal request."""
        try:
            response = await litellm.acompletion(
                model=provider,
                messages=[{"role": "user", "content": "hi"}],
                max_tokens=1,
                timeout=10
            )
            self.health[provider]["healthy"] = True
            self.health[provider]["failures"] = 0
            return True
        except Exception:
            self.health[provider]["failures"] += 1
            if self.health[provider]["failures"] >= 3:
                self.health[provider]["healthy"] = False
            return False
    
    def get_healthy_providers(self) -> list[str]:
        return [p for p in self.providers if self.health[p]["healthy"]]
```

---

## Best Practices Summary

| Practice | Impact | Difficulty |
|----------|--------|-----------|
| Exponential backoff with jitter | Prevents cascading failures | Easy |
| Respect Retry-After headers | Avoids ban escalation | Easy |
| Proactive rate limiting | Prevents 429 errors | Medium |
| Stream for interactive, batch for offline | Better UX + cost savings | Easy |
| Multi-level timeouts | Prevents hung requests | Medium |
| Unified error handling | Cleaner code, better debugging | Medium |
| Connection pooling with HTTP/2 | Lower latency | Easy |
| Structured request/response logging | Debugging + cost tracking | Easy |
| Idempotent tool execution | Prevents duplicate side effects | Medium |
| Provider health monitoring | Automatic failover | Advanced |
| Context window recovery | Graceful degradation | Advanced |

---

## See Also

- [LiteLLM](litellm.md) — Handles many of these patterns automatically
- [Model Routing](model-routing.md) — Fallback and routing strategies
- [Pricing and Cost](pricing-and-cost.md) — Cost implications of retries
- [OpenAI](openai.md) — OpenAI-specific API patterns
- [Anthropic](anthropic.md) — Anthropic-specific API patterns