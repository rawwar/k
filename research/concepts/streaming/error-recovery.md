# Error Recovery in Streaming

When streaming breaks: comprehensive guide to failure modes and recovery strategies.

Streaming is inherently fragile. Unlike a simple request-response where you either get the
full answer or an error, a streaming response can fail *mid-delivery*. The agent has partial
content, the user has seen partial output, and the system must decide: discard, retry, or
recover? Every serious coding agent must answer this question, and they answer it differently.

---

## 1. Network Interruption Handling

### Types of Network Failures

Not all network failures look the same. Each type has different detection characteristics
and recovery implications:

| Failure Type | Detection Signal | Typical Cause | Recovery Difficulty |
|---|---|---|---|
| TCP connection reset | RST packet / ECONNRESET | Server crash, load balancer timeout | Medium — retry usually works |
| DNS resolution failure | ENOTFOUND / NXDOMAIN | DNS outage, misconfigured resolver | Low — retry after brief wait |
| TLS handshake timeout | ETIMEDOUT during handshake | Certificate issues, firewall | High — may need config change |
| Proxy/firewall interruption | Connection closed unexpectedly | Corporate proxy, WAF rules | High — environmental |
| Wi-Fi disconnection | ENETUNREACH / ENETDOWN | Physical layer issue | Medium — wait for reconnect |
| Server-side termination | Clean close or 5xx before stream | Server overload, deployment | Low — standard retry |

### Detection Mechanisms

Detection varies by language and HTTP client:

```python
# Python with httpx streaming
import httpx

async def detect_stream_failure(url: str, payload: dict):
    try:
        async with httpx.AsyncClient(timeout=30.0) as client:
            async with client.stream("POST", url, json=payload) as response:
                async for chunk in response.aiter_bytes():
                    process(chunk)
    except httpx.ReadTimeout:
        # No data received within timeout — server may be stalled
        handle_read_timeout()
    except httpx.RemoteProtocolError:
        # Server violated HTTP protocol — likely mid-stream crash
        handle_protocol_error()
    except httpx.ConnectError:
        # Could not establish connection at all
        handle_connect_error()
```

```go
// Go with net/http streaming
resp, err := http.Post(url, "application/json", body)
if err != nil {
    // Connection-level failure
    return fmt.Errorf("connection failed: %w", err)
}
defer resp.Body.Close()

reader := bufio.NewReader(resp.Body)
for {
    line, err := reader.ReadBytes('\n')
    if err == io.EOF {
        break // Clean stream end
    }
    if err != nil {
        // Mid-stream failure: io.ErrUnexpectedEOF, net.OpError, etc.
        return fmt.Errorf("stream interrupted: %w", err)
    }
    processLine(line)
}
```

### Agent Strategies

- **OpenCode**: Context-based cancellation at 4 levels (session, loop, stream, tool).
  When cancelled mid-stream, message is finalized using `context.Background()` to ensure
  the DB write succeeds even though the parent context is cancelled.

- **Codex**: `Op::Interrupt` cancels active streams, aborts pending tools, emits `TurnAborted`.
  The single-threaded event queue (SQ) ensures interrupt is processed in order.

- **Goose**: Yields "please resend your last message" and breaks, preserving conversation
  state. Simple but effective — the user decides whether to retry.

- **OpenHands**: Entire event stream is persistent and append-only. `ReplayManager`
  reconstructs state from any point. Network failure just means replaying from last event.

- **Ante**: Lock-free scheduler ensures no sub-agent blocks another. If one stream fails,
  sibling agents continue unaffected. Failed agent can be independently retried.

---

## 2. Reconnection Strategies

### SSE Auto-Reconnect

Server-Sent Events have reconnection built into the spec:

```
# Server sends event with ID
id: 42
event: delta
data: {"text": "Hello"}

# Server sets retry interval (milliseconds)
retry: 3000
```

When the connection drops, the browser's `EventSource` automatically:
1. Waits the `retry` interval (default: varies by browser)
2. Reconnects with `Last-Event-ID: 42` header
3. Server can resume from that point

**BUT**: LLM APIs typically don't support resuming mid-generation. Each inference is
stateless — the model doesn't checkpoint its decoding state. `Last-Event-ID` is useless
if the server can't resume token generation from token #42.

### In Practice for LLM APIs

Most agents treat a dropped stream as a failed turn and retry from scratch:

```
[Turn N]
User: "Refactor the auth module"
                                    ← Stream starts
Assistant: "I'll refactor the..."   ← Stream drops here
                                    ← Agent detects failure
[Turn N, Attempt 2]
User: "Refactor the auth module"    ← Same request re-sent
                                    ← Full conversation history included
Assistant: "I'll refactor the..."   ← Fresh generation
```

### Anthropic's Error Recovery (Partial Response Continuation)

For Claude models, Anthropic documents a specific recovery pattern:

**Claude 4.5 and earlier:**
```json
{
  "messages": [
    {"role": "user", "content": "Write a long essay about..."},
    {"role": "assistant", "content": "Here is my essay. First, let me discuss"}
  ]
}
```
The partial assistant message is prefilled — the model continues from where it left off.

**Claude 4.6:**
```json
{
  "messages": [
    {"role": "user", "content": "Write a long essay about..."},
    {"role": "assistant", "content": "Here is my essay. First, let me discuss"},
    {"role": "user", "content": "Your response was interrupted. Continue exactly from where you stopped."}
  ]
}
```

**Limitation**: Tool use blocks and thinking blocks cannot be partially recovered.
If the stream failed during a tool_use or thinking block, you must discard back to
the most recent complete text block and resume from there.

---

## 3. Retry with Exponential Backoff

### Algorithm

```python
import time
import random

def retry_with_backoff(func, max_retries=5, base_delay=1.0, max_delay=60.0):
    """Retry with exponential backoff and full jitter."""
    for attempt in range(max_retries):
        try:
            return func()
        except RetryableError as e:
            if attempt == max_retries - 1:
                raise
            # Exponential backoff with cap
            exp_delay = min(base_delay * (2 ** attempt), max_delay)
            # Full jitter: uniform random between 0 and exp_delay
            delay = random.uniform(0, exp_delay)
            log.warning(f"Attempt {attempt + 1} failed: {e}. Retrying in {delay:.1f}s")
            time.sleep(delay)
```

```typescript
// TypeScript equivalent
async function retryWithBackoff<T>(
  fn: () => Promise<T>,
  maxRetries = 5,
  baseDelay = 1000,
  maxDelay = 60000
): Promise<T> {
  for (let attempt = 0; attempt < maxRetries; attempt++) {
    try {
      return await fn();
    } catch (error) {
      if (attempt === maxRetries - 1) throw error;
      if (!isRetryable(error)) throw error;
      const expDelay = Math.min(baseDelay * 2 ** attempt, maxDelay);
      const jitter = Math.random() * expDelay;
      await new Promise((r) => setTimeout(r, jitter));
    }
  }
  throw new Error("unreachable");
}
```

### Agent-Specific Configurations

| Agent | Max Retries | Backoff Strategy | Idle Timeout | Notes |
|---|---|---|---|---|
| **Codex** | 5 (stream) / 4 (request) | Provider-configured | 300s | `stream_max_retries` separate from request retries |
| **OpenCode** | 8 | Provider layer | Context-based | Uses Go SDK default retry with provider config |
| **Goose** | 2 (compaction) | Immediate retry | 1000 turns max | Retries compaction, not individual streams |
| **ForgeCode** | Configurable | Configurable | 300s | `FORGE_TOOL_TIMEOUT` env var |
| **Gemini CLI** | Configurable | Fallback module | Turn limits | Can fall back to alternative models |
| **Aider** | Via litellm | litellm handles | Provider-specific | Delegates entirely to litellm retry logic |

### Jitter Strategies

Jitter prevents thundering herd when many clients retry simultaneously:

```
Full Jitter:        sleep = random(0, base * 2^attempt)
Equal Jitter:       temp = base * 2^attempt; sleep = temp/2 + random(0, temp/2)
Decorrelated Jitter: sleep = random(base, previous_sleep * 3)
```

**Full jitter** is generally recommended (AWS architecture blog). It produces the
broadest distribution, minimizing collision probability. Equal jitter provides a
guaranteed minimum wait, which can be preferable when you want *some* backoff even
in the best case.

---

## 4. Partial Response Recovery

### Strategy 1: Discard and Retry (Most Common)

The simplest approach — if the stream fails, throw away whatever was received and
retry the entire request:

```
Stream: "I'll start by reading the fi" ← connection drops
Action: Discard partial content
Retry:  Full request re-sent
Result: "I'll start by reading the file..." ← complete response
```

**Used by**: OpenCode, Codex, Gemini CLI (default behavior)

**Pros**: Simple, correct, no risk of incoherent responses
**Cons**: Wastes tokens, user sees content disappear

### Strategy 2: Preserve Partial Content

Save partial response for debugging or inspection, even if not used for continuation:

- **OpenCode**: Every streaming delta is persisted to SQLite as it arrives.
  Even on failure, the partial message is in the database. The UI can show
  what was generated before the failure occurred.

- **Claude Code**: Partial content visible in conversation; user can reference
  what was generated before deciding to retry.

### Strategy 3: Emergency Compaction (Goose)

When `ContextLengthExceeded` occurs mid-stream:

```
1. Catch ContextLengthExceeded error
2. Call compact_messages() to summarize conversation
3. Replace message history with compacted version
4. Retry the request with reduced context
5. If compaction fails twice, give up gracefully
```

This is graceful degradation — the agent loses some conversation detail but can
continue operating. Up to 2 compaction attempts before the agent admits defeat.

### Strategy 4: Checkpoint-Based Recovery (Claude Code)

Claude Code takes file-level snapshots before every edit:

```
[Checkpoint: files at state S0]
  Tool: edit file A        → S1
[Checkpoint: files at state S1]
  Tool: edit file B        → S2
[Checkpoint: files at state S2]
  Stream fails during tool C
  
User presses Esc+Esc → rewind menu:
  - Revert to S2 (undo tool C attempt)
  - Revert to S1 (undo B and C)
  - Revert to S0 (undo everything)
```

This is turn-level recovery, not stream-level. The stream failure becomes a
non-issue because the file system state can be independently restored.

---

## 5. Rate Limit Handling Mid-Stream (429 Errors)

### Before Stream Starts

Standard HTTP 429 handling:

```python
response = requests.post(api_url, json=payload, stream=True)
if response.status_code == 429:
    retry_after = int(response.headers.get("Retry-After", 60))
    time.sleep(retry_after)
    # Retry the request
```

### During Streaming

More complex — the server terminates an active stream:

```python
async for chunk in stream:
    if chunk.get("error"):
        error = chunk["error"]
        if error.get("type") == "rate_limit_error":
            # Stream terminated due to rate limit
            wait_time = extract_retry_after(error)
            await asyncio.sleep(wait_time)
            # Must retry entire request — cannot resume
            break
    process(chunk)
```

### Agent-Specific Handling

- **Goose**: `CreditsExhausted` yields a notification with `top_up_url`, directing the
  user to add credits rather than silently retrying.

- **Codex**: Provider-level retry handles 429s automatically. `stream_max_retries`
  covers mid-stream disconnections including rate-limit-induced ones.

- **Aider**: litellm handles rate limiting transparently — the agent code doesn't
  even see 429s in most cases.

- **Gemini CLI**: Routing module with fallback to alternative models. If primary model
  is rate-limited, can switch to a fallback model transparently.

### Rate Limit Headers to Monitor

```
X-RateLimit-Limit: 100        # Total allowed requests per window
X-RateLimit-Remaining: 3      # Requests remaining in current window
X-RateLimit-Reset: 1697000000 # Unix timestamp when window resets
Retry-After: 30               # Seconds to wait before retrying
```

**Proactive checking**: Before starting a new stream, check `X-RateLimit-Remaining`.
If close to zero, wait for the reset window rather than starting a stream that will
be interrupted.

---

## 6. API Timeout Handling

### Types of Timeouts

```
┌─────────────────────────────────────────────────────────┐
│                    Total Request Timeout                 │
│  ┌──────────┐  ┌──────────────────────────────────────┐ │
│  │ Connect  │  │        Read / Idle Timeout            │ │
│  │ Timeout  │  │  (resets on each chunk received)      │ │
│  └──────────┘  └──────────────────────────────────────┘ │
└─────────────────────────────────────────────────────────┘
     2-10s              30s-300s per chunk
```

| Timeout Type | What It Means | Typical Value |
|---|---|---|
| Connection timeout | Can't establish TCP/TLS connection | 5-10 seconds |
| Read timeout | No data received within interval | 30-60 seconds |
| Idle timeout | Stream active but no new data | 60-300 seconds |
| Total request timeout | Overall wall-clock limit | 5-30 minutes |

### Agent Configurations

- **Codex**: `stream_idle_timeout_ms = 300,000` (5 minutes). Generous because large
  models like o1/o3 can take minutes for first token during extended thinking.

- **OpenCode**: `maxRetries = 8` at provider layer. No explicit idle timeout — relies
  on Go's `context.Context` cancellation propagated from the session level.

- **ForgeCode**: `FORGE_TOOL_TIMEOUT = 300` seconds. Applies to tool execution, which
  includes LLM calls made by sub-tools.

### Timeout vs Slow Model

This is a critical distinction. Extended thinking models (o1, o3, Claude with
thinking enabled) may produce no output for 30-120 seconds while "thinking."

```
Normal model:   [request] → 200ms → [first token] → tokens flow...
Thinking model:  [request] → 45s → [thinking...] → 30s → [first token] → tokens flow...
```

Agents must NOT interpret the thinking pause as a timeout. Solutions:
- Set idle timeout much higher than connection timeout
- Detect `thinking` content block type and extend timeout
- Use separate timeout profiles for thinking vs non-thinking models

---

## 7. User Interruption (Ctrl+C / Esc)

### The Most Common "Error" Case

Users interrupt far more often than networks fail. This is the normal case,
not an exception. It must be handled as a first-class operation.

### Agent Mechanisms

| Agent | Interrupt Key | Mechanism | State After Interrupt |
|---|---|---|---|
| **Claude Code** | Esc | Context preserved | Can continue conversation |
| **OpenCode** | Ctrl+C | Context cancellation cascade | Message finalized as canceled |
| **Codex** | Esc | `Op::Interrupt` via SQ | `TurnAborted`, clean state |
| **Goose** | Ctrl+C | `CancellationToken` per iteration | Loop exits cleanly |
| **Pi** | Enter | Steering message injection | Conversation continues naturally |

### Pi's Steering Approach

Rather than canceling, Pi lets users redirect mid-stream:

```
Agent: "I'll start by analyzing the entire codebase structure and then..."
User: [presses Enter]
User: "Skip the analysis, just fix the bug in auth.py"
Agent: [processes steering message before next LLM call]
Agent: "Looking at auth.py directly..."
```

- `Enter` → inject message after current tool call completes
- `Alt+Enter` → queue follow-up for after entire turn completes
- The agent processes steering messages at natural breakpoints

### Implementation Pattern (Go)

```go
ctx, cancel := context.WithCancel(parentCtx)

// Listen for interrupt signal in background
go func() {
    sig := make(chan os.Signal, 1)
    signal.Notify(sig, syscall.SIGINT)
    <-sig
    cancel() // Propagates to all child contexts
}()

// Stream processing loop
for {
    select {
    case <-ctx.Done():
        // Interrupt received — finalize gracefully
        // Use background context for DB write since parent is cancelled
        finalizeCtx := context.Background()
        finalizeMessage(finalizeCtx, msg, FinishReasonCanceled)
        return
    case chunk, ok := <-streamCh:
        if !ok {
            // Stream completed normally
            finalizeMessage(ctx, msg, FinishReasonStop)
            return
        }
        processChunk(chunk)
    }
}
```

### Implementation Pattern (TypeScript)

```typescript
const controller = new AbortController();

process.on("SIGINT", () => {
  controller.abort();
});

try {
  const stream = await client.messages.stream({
    model: "claude-sonnet-4-20250514",
    messages,
    signal: controller.signal,
  });

  for await (const event of stream) {
    if (controller.signal.aborted) break;
    processEvent(event);
  }
} catch (err) {
  if (err.name === "AbortError") {
    // User interrupted — not an error
    finalizeAsInterrupted(partialContent);
  } else {
    throw err;
  }
}
```

---

## 8. Graceful Degradation (Fall Back to Non-Streaming)

### When to Fall Back

Streaming isn't always available or desirable:

- Corporate proxy strips SSE headers or buffers the response
- CI/CD environment where streaming adds complexity for no UX benefit
- Client library doesn't support streaming
- Debugging — easier to inspect complete responses

### Implementation

```python
async def call_llm(messages: list, prefer_streaming: bool = True) -> str:
    if prefer_streaming:
        try:
            return await call_streaming(messages)
        except StreamingUnsupportedError:
            log.info("Streaming unavailable, falling back to non-streaming")
        except ConnectionError as e:
            if is_proxy_related(e):
                log.info("Proxy issue detected, falling back to non-streaming")
            else:
                raise

    # Non-streaming fallback
    response = await client.messages.create(
        model=model,
        messages=messages,
        stream=False,
    )
    return response.content[0].text
```

### Environment Detection

```python
def should_use_streaming() -> bool:
    # CI environments rarely benefit from streaming
    if os.environ.get("CI"):
        return False
    # Non-interactive terminals
    if not sys.stdout.isatty():
        return False
    # User preference
    if os.environ.get("NO_STREAM"):
        return False
    return True
```

---

## 9. Token Limit Exceeded Mid-Response

### Detection

The stream completes "normally" but the response is truncated:

```json
{
  "type": "message_delta",
  "delta": {
    "stop_reason": "max_tokens"
  },
  "usage": {
    "output_tokens": 4096
  }
}
```

The `stop_reason` (or `finish_reason` in OpenAI) tells you why generation stopped:
- `"end_turn"` / `"stop"` — model finished naturally
- `"max_tokens"` / `"length"` — hit token limit, response truncated
- `"tool_use"` — model wants to call a tool (not an error)

### Recovery Strategies

**Auto-continue**: Detect truncation and automatically ask for continuation.

```python
while True:
    response = await stream_response(messages)
    messages.append({"role": "assistant", "content": response.content})

    if response.stop_reason == "end_turn":
        break  # Model finished naturally
    elif response.stop_reason == "max_tokens":
        # Model was cut off — ask it to continue
        messages.append({
            "role": "user",
            "content": "Your response was truncated. Continue exactly where you left off."
        })
    else:
        break
```

**Context window exceeded** is different from max_tokens. It means the *input* is too
large, not the output. Recovery requires reducing the input:

- Goose: emergency compaction (summarize conversation)
- OpenCode: sliding window over message history
- Claude Code: prune old tool results, keep recent context

---

## 10. Connection Pooling and Keep-Alive

### HTTP Keep-Alive for Sequential LLM Calls

Agentic loops make many sequential LLM calls. Reusing TCP connections saves
~50-150ms per call (TLS handshake + TCP slow start):

```
Without keep-alive:  [TCP][TLS][Request][Response][Close] × N calls
With keep-alive:     [TCP][TLS][Request][Response][Request][Response]... × 1 connection
```

### Connection Pool Configuration (Python)

```python
import httpx

# Shared client with connection pooling
client = httpx.AsyncClient(
    limits=httpx.Limits(
        max_connections=10,        # Total connections in pool
        max_keepalive_connections=5, # Idle connections to keep
        keepalive_expiry=30.0,     # Seconds before closing idle connection
    ),
    timeout=httpx.Timeout(
        connect=10.0,
        read=60.0,
        write=10.0,
        pool=30.0,  # Wait for available connection from pool
    ),
)
```

### SSE and Keep-Alive

SSE connections are long-lived — one connection per active stream. Key differences
from regular HTTP keep-alive:

- SSE connection stays open for the duration of the stream (seconds to minutes)
- No multiplexing in HTTP/1.1 SSE (one stream per connection)
- HTTP/2 allows multiplexed streams on a single connection
- Connection drop requires full re-establishment (no resume)

---

## 11. Idempotency and Replay Safety

### The Problem

When a stream fails and you retry, the *request* may have already had side effects:

```
Request: "Create a new file called utils.py"
Stream:  [tool_call: create_file] → [partial response] → ← connection drops
Retry:   Same request re-sent
Result:  Agent tries to create utils.py again → FileExistsError!
```

### Idempotency Checks by Tool Type

| Tool Type | Idempotency Check | Strategy |
|---|---|---|
| File write | Does file exist with expected content? | Skip if content matches |
| File create | Does file already exist? | Error or skip |
| Git commit | Is HEAD already the expected commit? | Skip if already committed |
| Shell command | Check for side effects (process running, etc.) | Hardest to make idempotent |
| API call | Idempotency key header | `Idempotency-Key: <uuid>` |

### Implementation Pattern

```python
def idempotent_file_write(path: str, content: str) -> bool:
    """Write file only if content differs. Returns True if write occurred."""
    try:
        existing = Path(path).read_text()
        if existing == content:
            return False  # Already has correct content — skip
    except FileNotFoundError:
        pass  # File doesn't exist — proceed with write

    Path(path).write_text(content)
    return True
```

### Event Stream Approach (OpenHands)

OpenHands solves this architecturally — its event stream is append-only and
every event has a unique ID. On replay:

```
Event #42: FileWriteAction(path="utils.py", content="...")
Event #43: FileWriteObservation(success=True)
← crash here →
Replay from #42: Event already has observation #43 → skip execution
```

The event stream itself is the idempotency mechanism. No duplicate execution
is possible because the system replays *observations*, not *actions*.

---

## Summary: Error Recovery Decision Tree

```
Stream fails
├── Is it a rate limit (429)?
│   ├── Yes → Wait Retry-After, retry full request
│   └── No ↓
├── Is it a network error?
│   ├── Yes → Exponential backoff, retry full request
│   └── No ↓
├── Is it a context length error?
│   ├── Yes → Compact/summarize context, retry
│   └── No ↓
├── Is it a token limit (max_tokens)?
│   ├── Yes → Auto-continue with continuation prompt
│   └── No ↓
├── Is it a user interruption?
│   ├── Yes → Finalize partial content, preserve state, await next input
│   └── No ↓
├── Is it an API error (500, 503)?
│   ├── Yes → Exponential backoff, retry full request
│   └── No ↓
└── Unknown error
    └── Log error, notify user, preserve partial content for inspection
```

The key insight across all agents: **the conversation history is the recovery mechanism**.
Because LLM APIs are stateless, retrying a request with the same conversation history
produces equivalent (not identical) output. The stream is ephemeral; the messages are
persistent. Recovery means preserving messages and regenerating the stream.
