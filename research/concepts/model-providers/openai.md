# OpenAI as a Model Provider for Coding Agents

## Overview

OpenAI is the most widely integrated model provider in the CLI coding agent ecosystem.
Its GPT-4 family, o-series reasoning models, and mature API infrastructure make it the
default choice for many agents. OpenAI pioneered function calling in LLMs, established
the Chat Completions API format that became the de facto industry standard, and
continues to push the frontier with models like GPT-4.1 and the o3/o4-mini reasoning
series.

Among the 17 agents studied in this research library, **12 (71%)** support OpenAI
models, making it the second most popular provider after Anthropic.

---

## Model Lineup for Coding

### GPT-4.1

GPT-4.1 is OpenAI's flagship model optimized for coding tasks (released April 2025):

| Property | Value |
|----------|-------|
| **Model ID** | `gpt-4.1` |
| **Context Window** | 1,048,576 tokens (1M) |
| **Max Output** | 32,768 tokens |
| **Input Price** | $2.00 / MTok |
| **Output Price** | $8.00 / MTok |
| **Cached Input** | $0.50 / MTok |
| **Knowledge Cutoff** | June 2025 |
| **Strengths** | Instruction following, long context, coding |

GPT-4.1 was specifically designed for agentic coding workflows. Key improvements over
GPT-4o include:

- **Better instruction following** — More reliable adherence to system prompt constraints
- **Improved long-context performance** — Less degradation with large codebases
- **Better coding benchmarks** — Higher SWE-bench scores than GPT-4o
- **Cost reduction** — Significantly cheaper than GPT-4 Turbo

```python
# Using GPT-4.1 via OpenAI SDK
from openai import OpenAI

client = OpenAI()
response = client.chat.completions.create(
    model="gpt-4.1",
    messages=[
        {"role": "system", "content": "You are a coding assistant."},
        {"role": "user", "content": "Refactor this function to use async/await..."}
    ],
    tools=[{
        "type": "function",
        "function": {
            "name": "edit_file",
            "description": "Edit a file at the given path",
            "parameters": {
                "type": "object",
                "properties": {
                    "path": {"type": "string"},
                    "new_content": {"type": "string"}
                },
                "required": ["path", "new_content"]
            }
        }
    }]
)
```

### GPT-4.1 mini and GPT-4.1 nano

Lower-cost variants for high-volume or latency-sensitive tasks:

| Model | Input Price | Output Price | Context | Best For |
|-------|-----------|-------------|---------|----------|
| GPT-4.1 mini | $0.40 / MTok | $1.60 / MTok | 1M | Sub-agent tasks, classification |
| GPT-4.1 nano | $0.10 / MTok | $0.40 / MTok | 1M | High-volume, simple tasks |

### GPT-4o

The previous-generation flagship, still widely used:

| Property | Value |
|----------|-------|
| **Model ID** | `gpt-4o` |
| **Context Window** | 128,000 tokens |
| **Max Output** | 16,384 tokens |
| **Input Price** | $2.50 / MTok |
| **Output Price** | $10.00 / MTok |
| **Cached Input** | $1.25 / MTok |

GPT-4o remains the default model in many agents that haven't yet migrated to GPT-4.1.
It offers strong multimodal capabilities (vision, audio) and good coding performance.

### o-Series Reasoning Models

OpenAI's reasoning models use chain-of-thought "thinking tokens" before producing
an answer. These are particularly powerful for complex coding tasks that require
planning and multi-step reasoning.

#### o3

| Property | Value |
|----------|-------|
| **Model ID** | `o3` |
| **Context Window** | 200,000 tokens |
| **Max Output** | 100,000 tokens |
| **Input Price** | $2.00 / MTok |
| **Output Price** | $8.00 / MTok |
| **Reasoning Output** | $8.00 / MTok |

o3 is the high-end reasoning model. It excels at:
- Complex algorithmic problems
- Multi-file refactoring planning
- Architecture design decisions
- Debugging subtle concurrency issues

#### o4-mini

| Property | Value |
|----------|-------|
| **Model ID** | `o4-mini` |
| **Context Window** | 200,000 tokens |
| **Max Output** | 100,000 tokens |
| **Input Price** | $1.10 / MTok |
| **Output Price** | $4.40 / MTok |
| **Reasoning Output** | $4.40 / MTok |

o4-mini offers reasoning capabilities at a lower cost, making it practical for
agentic coding loops where multiple reasoning steps are needed.

#### Using Reasoning Models in Agents

Reasoning models work differently from standard chat models:

```python
# o-series models use a different parameter structure
response = client.chat.completions.create(
    model="o3",
    messages=[
        {"role": "user", "content": "Plan the refactoring of this module..."}
    ],
    # Reasoning models use 'reasoning_effort' instead of 'temperature'
    reasoning_effort="high",  # low, medium, high
    max_completion_tokens=25000  # Includes both thinking + output tokens
)

# Access reasoning summary (when available)
message = response.choices[0].message
if hasattr(message, 'reasoning'):
    print("Reasoning:", message.reasoning)
print("Answer:", message.content)
```

**Key differences for reasoning models:**
- No `temperature` or `top_p` parameters (reasoning is deterministic)
- `max_completion_tokens` replaces `max_tokens` (includes thinking tokens)
- `reasoning_effort` controls depth of thinking (low/medium/high)
- System messages work but with different behavioral patterns
- Function calling is supported but the model reasons about tool use

---

## API Architecture

### Chat Completions API

The Chat Completions API (`/v1/chat/completions`) is the workhorse of the OpenAI
platform and the de facto standard that other providers emulate:

```python
# Standard Chat Completions request
response = client.chat.completions.create(
    model="gpt-4.1",
    messages=[
        {"role": "system", "content": "You are a coding agent..."},
        {"role": "user", "content": "Fix the bug in auth.py"},
        {"role": "assistant", "content": None, "tool_calls": [
            {"id": "call_1", "type": "function", "function": {
                "name": "read_file", "arguments": '{"path": "auth.py"}'
            }}
        ]},
        {"role": "tool", "tool_call_id": "call_1", "content": "def login():..."}
    ],
    tools=[...],
    stream=True
)
```

**Key features:**
- Structured message history with roles (system, user, assistant, tool)
- Native function/tool calling with JSON schemas
- Streaming via Server-Sent Events (SSE)
- Vision support (image URLs or base64 in user messages)
- Logprobs for confidence estimation
- Seed parameter for reproducibility

### Responses API

The Responses API (`/v1/responses`) is OpenAI's newer, stateful API designed for
agentic workflows. Codex CLI uses this API:

```python
# Responses API — designed for agentic use cases
response = client.responses.create(
    model="gpt-4.1",
    input="Fix the authentication bug in the login module",
    tools=[
        {"type": "file_search"},  # Built-in tools
        {"type": "code_interpreter"},
        {"type": "function", "function": {...}}  # Custom tools
    ],
    previous_response_id="resp_abc123",  # Automatic conversation threading
    instructions="You are a coding agent that edits files directly."
)
```

**Key differences from Chat Completions:**
- **Stateful conversations** — `previous_response_id` chains responses without
  resending full history
- **Built-in tools** — File search, code interpreter, web search as first-class tools
- **Simpler message format** — Single `input` string instead of message array
- **Response objects** — Richer output structure with tool call results embedded

**When Codex CLI uses the Responses API:**

```javascript
// Codex CLI's approach (simplified from source)
const response = await openai.responses.create({
    model: userConfig.model,  // e.g., "gpt-4.1"
    input: userMessage,
    instructions: SYSTEM_PROMPT,
    tools: [
        { type: "function", function: shellFunction },
        { type: "function", function: fileEditFunction }
    ],
    previous_response_id: lastResponseId,
    stream: true
});
```

### Function Calling

OpenAI's function calling is the most mature in the industry. It allows agents to
define tools as JSON schemas and receive structured function calls in response:

```python
tools = [
    {
        "type": "function",
        "function": {
            "name": "execute_command",
            "description": "Run a shell command in the user's terminal",
            "strict": True,  # Enforce JSON schema compliance
            "parameters": {
                "type": "object",
                "properties": {
                    "command": {
                        "type": "string",
                        "description": "The shell command to execute"
                    },
                    "working_directory": {
                        "type": "string",
                        "description": "Directory to run the command in"
                    }
                },
                "required": ["command"],
                "additionalProperties": False
            }
        }
    }
]

# Parallel function calling — model can call multiple tools at once
response = client.chat.completions.create(
    model="gpt-4.1",
    messages=messages,
    tools=tools,
    parallel_tool_calls=True  # Allow multiple simultaneous tool calls
)

# Process tool calls
for tool_call in response.choices[0].message.tool_calls:
    function_name = tool_call.function.name
    arguments = json.loads(tool_call.function.arguments)
    result = execute_tool(function_name, arguments)
    messages.append({
        "role": "tool",
        "tool_call_id": tool_call.id,
        "content": str(result)
    })
```

**Strict mode** (`"strict": True`) guarantees the model's output matches the JSON
schema exactly. This is critical for coding agents where malformed tool calls would
break the agentic loop.

---

## Streaming

OpenAI supports Server-Sent Events (SSE) streaming for real-time output:

```python
# Streaming with function calls
stream = client.chat.completions.create(
    model="gpt-4.1",
    messages=messages,
    tools=tools,
    stream=True
)

collected_tool_calls = {}
for chunk in stream:
    delta = chunk.choices[0].delta
    
    # Handle text content
    if delta.content:
        print(delta.content, end="", flush=True)
    
    # Handle streaming tool calls
    if delta.tool_calls:
        for tc in delta.tool_calls:
            idx = tc.index
            if idx not in collected_tool_calls:
                collected_tool_calls[idx] = {
                    "id": tc.id,
                    "function": {"name": tc.function.name, "arguments": ""}
                }
            if tc.function.arguments:
                collected_tool_calls[idx]["function"]["arguments"] += tc.function.arguments
```

---

## Vision Capabilities

GPT-4o and GPT-4.1 support image input, useful for agents that need to understand
screenshots, diagrams, or UI mockups:

```python
response = client.chat.completions.create(
    model="gpt-4o",
    messages=[{
        "role": "user",
        "content": [
            {"type": "text", "text": "Implement this UI design in React"},
            {"type": "image_url", "image_url": {
                "url": "data:image/png;base64,{base64_image}",
                "detail": "high"  # low, high, auto
            }}
        ]
    }]
)
```

**Image pricing (GPT-4o):**
- Low detail: ~85 tokens per image
- High detail: ~170 tokens per 512x512 tile + 85 base tokens

---

## Prompt Caching

OpenAI offers automatic prompt caching for repeated prefixes:

| Feature | Details |
|---------|---------|
| **Activation** | Automatic for prompts > 1,024 tokens |
| **Discount** | 50% off cached input tokens |
| **TTL** | Typically 5-10 minutes (varies) |
| **Minimum prefix** | 1,024 tokens |

Unlike Anthropic's explicit cache breakpoints, OpenAI's caching is automatic—the
system detects when the beginning of a prompt matches a previously processed prompt
and reuses the cached computation.

**Impact for coding agents:** System prompts + tool definitions often exceed 1,024
tokens, so subsequent turns in a conversation automatically benefit from caching.

---

## Batch API

The Batch API processes large numbers of requests asynchronously at a 50% discount:

```python
# Create a batch file
import json

batch_requests = []
for i, task in enumerate(tasks):
    batch_requests.append({
        "custom_id": f"task-{i}",
        "method": "POST",
        "url": "/v1/chat/completions",
        "body": {
            "model": "gpt-4.1",
            "messages": [{"role": "user", "content": task}]
        }
    })

# Write JSONL file
with open("batch_input.jsonl", "w") as f:
    for req in batch_requests:
        f.write(json.dumps(req) + "\n")

# Upload and create batch
batch_file = client.files.create(file=open("batch_input.jsonl", "rb"), purpose="batch")
batch = client.batches.create(
    input_file_id=batch_file.id,
    endpoint="/v1/chat/completions",
    completion_window="24h"
)
```

**Batch API pricing (50% discount):**

| Model | Batch Input | Batch Output |
|-------|-----------|-------------|
| GPT-4.1 | $1.00 / MTok | $4.00 / MTok |
| GPT-4o | $1.25 / MTok | $5.00 / MTok |
| o4-mini | $0.55 / MTok | $2.20 / MTok |

**Use cases for coding agents:**
- Running evaluations across many test cases (SWE-bench)
- Batch code review of multiple files
- Generating documentation for an entire codebase

---

## Rate Limits

OpenAI uses a tier-based rate limiting system:

| Tier | RPM (GPT-4.1) | TPM (GPT-4.1) | Requirement |
|------|---------------|---------------|-------------|
| Free | 500 | 30,000 | Default |
| Tier 1 | 500 | 30,000 | $5 paid |
| Tier 2 | 5,000 | 450,000 | $50 paid |
| Tier 3 | 5,000 | 800,000 | $100 paid |
| Tier 4 | 10,000 | 2,000,000 | $250 paid |
| Tier 5 | 10,000 | 10,000,000 | $1,000 paid |

RPM = Requests Per Minute, TPM = Tokens Per Minute

**Rate limit headers in responses:**
```
x-ratelimit-limit-requests: 5000
x-ratelimit-limit-tokens: 800000
x-ratelimit-remaining-requests: 4999
x-ratelimit-remaining-tokens: 799000
x-ratelimit-reset-requests: 12ms
x-ratelimit-reset-tokens: 150ms
```

**Handling rate limits in agents:**

```python
import time
from openai import RateLimitError

def call_with_retry(client, **kwargs):
    max_retries = 5
    base_delay = 1.0
    
    for attempt in range(max_retries):
        try:
            return client.chat.completions.create(**kwargs)
        except RateLimitError as e:
            if attempt == max_retries - 1:
                raise
            delay = base_delay * (2 ** attempt)  # Exponential backoff
            retry_after = e.response.headers.get("retry-after")
            if retry_after:
                delay = max(delay, float(retry_after))
            time.sleep(delay)
```

---

## How Agents Use OpenAI

### Codex CLI

Codex CLI is OpenAI's official coding agent and the deepest OpenAI integration:

- **API:** Responses API (not Chat Completions)
- **Default model:** GPT-4.1
- **Supported models:** o3, o4-mini, GPT-4o, any model via `--model` flag
- **Unique features:**
  - Uses `previous_response_id` for stateful conversation threading
  - Automatic prompt caching via the Responses API
  - Sandboxed command execution with user approval
  - Custom OpenAI-compatible endpoints via TOML config

```toml
# Codex CLI config (~/.codex/config.toml)
model = "gpt-4.1"

# Custom provider (e.g., Ollama)
[providers.ollama]
base_url = "http://localhost:11434/v1"
api_key = "ollama"
```

### Aider (via LiteLLM)

Aider uses OpenAI through LiteLLM's unified interface:

```bash
# Using OpenAI models with Aider
aider --model gpt-4.1
aider --model o3  # Reasoning model
aider --model gpt-4.1 --editor-model gpt-4.1-mini  # Architect mode
```

Aider's "Architect mode" pairs an expensive planning model with a cheaper editing
model—a common pattern when using OpenAI's model range.

### OpenHands (via LiteLLM)

OpenHands configures OpenAI through LiteLLM:

```bash
export LLM_MODEL="openai/gpt-4.1"
export LLM_API_KEY="sk-..."
```

### ForgeCode

ForgeCode integrates OpenAI as one of its native providers:

```yaml
# ForgeCode configuration
model:
  provider: openai
  name: gpt-4.1
  api_key: ${OPENAI_API_KEY}
  
routing:
  planning: o3          # Reasoning model for planning
  editing: gpt-4.1      # Standard model for code edits
  review: gpt-4.1-mini  # Cheap model for reviews
```

---

## Fine-Tuning for Coding

OpenAI supports fine-tuning GPT-4o and GPT-4o-mini for specialized coding tasks:

```python
# Upload training data (JSONL format)
training_file = client.files.create(
    file=open("coding_finetune_data.jsonl", "rb"),
    purpose="fine-tune"
)

# Create fine-tuning job
job = client.fine_tuning.jobs.create(
    training_file=training_file.id,
    model="gpt-4o-mini-2024-07-18",
    hyperparameters={
        "n_epochs": 3,
        "batch_size": "auto",
        "learning_rate_multiplier": "auto"
    }
)
```

**Fine-tuning use cases for coding agents:**
- Training on organization-specific coding patterns
- Improving tool call reliability for custom tool schemas
- Teaching the model project-specific conventions
- Reducing hallucinations in domain-specific code

**Fine-tuning pricing:**
| Model | Training | Input (fine-tuned) | Output (fine-tuned) |
|-------|---------|-------------------|-------------------|
| GPT-4o mini | $3.00 / MTok | $0.60 / MTok | $2.40 / MTok |
| GPT-4o | $25.00 / MTok | $3.75 / MTok | $15.00 / MTok |

---

## Pricing Summary

### Per-Token Pricing (USD per million tokens)

| Model | Input | Cached Input | Output | Batch Input | Batch Output |
|-------|-------|-------------|--------|------------|-------------|
| **GPT-4.1** | $2.00 | $0.50 | $8.00 | $1.00 | $4.00 |
| **GPT-4.1 mini** | $0.40 | $0.10 | $1.60 | $0.20 | $0.80 |
| **GPT-4.1 nano** | $0.10 | $0.025 | $0.40 | $0.05 | $0.20 |
| **GPT-4o** | $2.50 | $1.25 | $10.00 | $1.25 | $5.00 |
| **GPT-4o mini** | $0.15 | $0.075 | $0.60 | $0.075 | $0.30 |
| **o3** | $2.00 | $0.50 | $8.00 | $1.00 | $4.00 |
| **o4-mini** | $1.10 | $0.275 | $4.40 | $0.55 | $2.20 |

### Cost Estimation for Coding Tasks

Typical token usage for a coding agent session:

| Task Type | Input Tokens | Output Tokens | Cost (GPT-4.1) | Cost (GPT-4.1 mini) |
|-----------|-------------|--------------|-----------------|---------------------|
| Simple bug fix | ~5,000 | ~2,000 | $0.03 | $0.005 |
| Feature implementation | ~30,000 | ~10,000 | $0.14 | $0.03 |
| Large refactoring | ~100,000 | ~40,000 | $0.52 | $0.10 |
| SWE-bench task (avg) | ~50,000 | ~15,000 | $0.22 | $0.04 |

---

## Best Practices for Coding Agents

### 1. Model Selection

```python
# Use reasoning models for planning, standard for execution
PLANNING_MODEL = "o3"        # Complex reasoning
EDITING_MODEL = "gpt-4.1"    # Code generation
REVIEW_MODEL = "gpt-4.1-mini"  # Quick checks

# Aider's architect mode embodies this pattern
# aider --model o3 --editor-model gpt-4.1
```

### 2. Optimize for Caching

```python
# Structure messages so the system prompt is always the same prefix
# This maximizes automatic cache hits
messages = [
    {"role": "system", "content": LARGE_SYSTEM_PROMPT},  # Cached after first call
    {"role": "user", "content": "..."},  # Changes per request
]
```

### 3. Use Strict Function Calling

```python
# Always use strict mode for tool definitions
tools = [{
    "type": "function",
    "function": {
        "name": "edit_file",
        "strict": True,  # Guarantees valid JSON output
        "parameters": {
            "type": "object",
            "properties": {...},
            "required": [...],
            "additionalProperties": False  # Required for strict mode
        }
    }
}]
```

### 4. Handle Streaming Tool Calls Correctly

When streaming, tool call arguments arrive incrementally. Agents must buffer them
before parsing:

```python
def collect_stream_tool_calls(stream):
    tool_calls = {}
    for chunk in stream:
        for tc_delta in (chunk.choices[0].delta.tool_calls or []):
            idx = tc_delta.index
            if tc_delta.id:
                tool_calls[idx] = {"id": tc_delta.id, "name": "", "arguments": ""}
            if tc_delta.function:
                if tc_delta.function.name:
                    tool_calls[idx]["name"] = tc_delta.function.name
                if tc_delta.function.arguments:
                    tool_calls[idx]["arguments"] += tc_delta.function.arguments
    return [
        {"id": tc["id"], "function": {"name": tc["name"], 
         "arguments": json.loads(tc["arguments"])}}
        for tc in tool_calls.values()
    ]
```

---

## Limitations and Considerations

### Context Window vs. Effective Context

While GPT-4.1 advertises a 1M token context window, performance degrades with very
long contexts. For coding agents, practical limits are often lower:

- **Reliable performance:** Up to ~200K tokens
- **Acceptable performance:** Up to ~500K tokens
- **Maximum capacity:** 1M tokens (with quality degradation)

### Reasoning Token Costs

For o-series models, reasoning tokens (internal chain-of-thought) are billed at the
output rate. A complex reasoning task might generate 10x more reasoning tokens than
visible output tokens, making actual costs much higher than simple estimation.

### Rate Limits for Agentic Loops

Coding agents make many rapid API calls in succession. Even Tier 5 limits can be
hit during intensive sessions. Best practice: implement exponential backoff and
consider using the Batch API for non-interactive tasks.

---

## See Also

- [API Patterns](api-patterns.md) — Retry logic, rate limit handling, streaming
- [Pricing and Cost](pricing-and-cost.md) — Cross-provider pricing comparison
- [Model Routing](model-routing.md) — When to use which OpenAI model
- [Agent Comparison](agent-comparison.md) — Which agents support OpenAI