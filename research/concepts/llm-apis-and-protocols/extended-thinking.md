# Extended Thinking / Reasoning Tokens Across LLM Providers

## Introduction: What Is "Thinking" in LLMs and Why It Matters

Extended thinking (also called "reasoning tokens," "chain-of-thought reasoning," or simply
"thinking") refers to the ability of an LLM to perform explicit, step-by-step reasoning
before producing its final answer. Instead of jumping directly to a response, the model
generates an internal reasoning trace—a stream of tokens where it works through the problem,
considers alternatives, checks its logic, and plans its approach.

This capability represents a fundamental shift in how LLMs solve problems. Standard models
generate responses token-by-token in a single forward pass, with all "reasoning" implicit
in the neural network's weights. Thinking models add an explicit reasoning phase that can
be hundreds or thousands of tokens long, giving the model a "scratchpad" to work through
complex problems.

For coding tasks, thinking is particularly valuable because software engineering requires:
- **Multi-step reasoning**: Understanding how a change in one file affects others
- **Constraint satisfaction**: Balancing requirements, performance, and maintainability
- **Debugging**: Tracing cause and effect through layers of abstraction
- **Planning**: Breaking a complex task into ordered subtasks
- **Error analysis**: Systematically examining why something failed

The cost of thinking is real—reasoning tokens consume context window space and are billed—but
for complex tasks, the improvement in accuracy often justifies the expense.

---

## Claude Extended Thinking

Anthropic's Claude models (Claude 3.7 Sonnet, Claude Sonnet 4, Claude Opus 4) support
extended thinking as a first-class feature. When enabled, Claude explicitly reasons through
problems before responding, and this reasoning is visible (in some form) to the API consumer.

### How to Enable Extended Thinking

Extended thinking is enabled via the `thinking` parameter in the API request:

```json
POST /v1/messages
{
  "model": "claude-sonnet-4-20250514",
  "max_tokens": 16000,
  "thinking": {
    "type": "enabled",
    "budget_tokens": 10000
  },
  "messages": [
    {
      "role": "user",
      "content": "Find the bug in this code and explain your reasoning step by step:\n\nfunction fibonacci(n) {\n  if (n <= 1) return n;\n  return fibonacci(n - 1) + fibonacci(n - 3);\n}"
    }
  ]
}
```

### Adaptive Thinking

Claude 4 models (Sonnet 4, Opus 4) also support adaptive thinking, where the model decides
how much thinking is needed based on the task complexity:

```json
{
  "thinking": {
    "type": "enabled",
    "budget_tokens": 10000
  }
}
```

With adaptive thinking, simple questions may use few or no thinking tokens, while complex
problems will use more. The `budget_tokens` sets an upper limit. Claude 4 models naturally
adapt their thinking depth.

You can also configure a fixed effort level for more predictable behavior:

```json
{
  "thinking": {
    "type": "enabled",
    "budget_tokens": 8000
  }
}
```

### `budget_tokens` Parameter

The `budget_tokens` parameter controls the maximum number of tokens Claude can use for thinking:

| Budget | Typical Use Case |
|--------|-----------------|
| 1,024–4,000 | Simple clarifications, straightforward code edits |
| 4,000–10,000 | Moderate complexity: debugging, small refactors |
| 10,000–32,000 | Complex problems: architecture decisions, multi-file analysis |
| 32,000+ | Very complex reasoning: deep debugging, algorithm design |

Key constraints:
- `budget_tokens` must be ≥ 1,024
- `budget_tokens` must be less than `max_tokens`
- The actual thinking tokens used may be much less than the budget
- You are billed for actual thinking tokens consumed, not the budget

### ThinkingBlock in Response

When thinking is enabled, the response includes `thinking` content blocks before the
text response:

```json
{
  "id": "msg_abc123",
  "type": "message",
  "role": "assistant",
  "content": [
    {
      "type": "thinking",
      "thinking": "Let me analyze this fibonacci function step by step.\n\nThe function is:\nfunction fibonacci(n) {\n  if (n <= 1) return n;\n  return fibonacci(n - 1) + fibonacci(n - 3);\n}\n\nThe standard Fibonacci recurrence is:\nF(n) = F(n-1) + F(n-2)\n\nBut this code uses fibonacci(n - 3) instead of fibonacci(n - 2).\nThat's the bug - it should be n - 2, not n - 3.\n\nLet me verify:\n- fibonacci(0) = 0 ✓\n- fibonacci(1) = 1 ✓\n- fibonacci(2) = fibonacci(1) + fibonacci(-1)\n  With the bug: fibonacci(-1) would recurse with n = -1,\n  which is <= 1, so returns -1.\n  So fibonacci(2) = 1 + (-1) = 0 ✗ (should be 1)\n\nYes, the bug is confirmed. The fix is changing n - 3 to n - 2.",
      "signature": "zbbJhbGciOiJFZERTQSIsImtpZCI..."
    },
    {
      "type": "text",
      "text": "I found the bug! On line 3, you have:\n\n```javascript\nreturn fibonacci(n - 1) + fibonacci(n - 3);\n```\n\nIt should be `fibonacci(n - 2)`, not `fibonacci(n - 3)`. The Fibonacci recurrence relation is F(n) = F(n-1) + F(n-2)."
    }
  ],
  "usage": {
    "input_tokens": 85,
    "output_tokens": 142,
    "cache_creation_input_tokens": 0,
    "cache_read_input_tokens": 0,
    "thinking_tokens": 187
  }
}
```

Key fields:
- `type: "thinking"`: Identifies this as a thinking block
- `thinking`: The actual reasoning text (visible in Claude 3.7; summarized in Claude 4)
- `signature`: A cryptographic signature used to verify thinking block integrity when
  passed back in multi-turn conversations

### Summarized Thinking (Claude 4) vs Full Thinking (Claude 3.7)

This is an important distinction:

- **Claude 3.7 Sonnet**: Shows the **full** internal reasoning in the `thinking` field.
  You see every step of the model's thought process, including false starts, corrections,
  and exploratory reasoning.

- **Claude 4 models (Sonnet 4, Opus 4)**: By default, show **summarized** thinking. The
  full internal reasoning happens but is condensed into a summary. The `thinking` field
  contains a distilled version of the reasoning, not the raw token-by-token chain of thought.

You can control this with the `display` parameter:

```json
{
  "thinking": {
    "type": "enabled",
    "budget_tokens": 10000,
    "display": "summarized"
  }
}
```

| `display` value | Behavior |
|-----------------|----------|
| `"summarized"` | Thinking is summarized (default for Claude 4 models) |
| `"omitted"` | Thinking blocks still generated but content is empty string |

Even with `"omitted"`, you still pay for the full thinking tokens — the model still reasons
internally; you just don't see it.

### Interleaved Thinking with Tool Use

When extended thinking is combined with tool use, Claude can think between tool calls. The
thinking blocks appear interleaved with tool use blocks:

```json
{
  "content": [
    {
      "type": "thinking",
      "thinking": "The user wants me to fix a failing test. Let me first read the test file to understand what's being tested...",
      "signature": "abc123..."
    },
    {
      "type": "tool_use",
      "id": "toolu_01",
      "name": "read_file",
      "input": { "path": "tests/auth.test.ts" }
    }
  ]
}
```

After receiving the tool result:

```json
{
  "content": [
    {
      "type": "thinking",
      "thinking": "OK, I can see the test expects the authenticate function to return a JWT token, but the test is failing on line 42. Let me read the source file to see the authenticate implementation...",
      "signature": "def456..."
    },
    {
      "type": "tool_use",
      "id": "toolu_02",
      "name": "read_file",
      "input": { "path": "src/auth.ts" }
    }
  ]
}
```

This interleaved pattern is crucial for coding agents because it lets the model reason about
each tool result before deciding the next action. Without thinking, the model must make tool
call decisions based purely on implicit reasoning.

### Preserving Thinking Blocks in Multi-Turn Conversations

When building multi-turn conversations with thinking enabled, you should preserve thinking
blocks from previous turns. The `signature` field is used to verify integrity:

```json
{
  "messages": [
    { "role": "user", "content": "Analyze this code..." },
    {
      "role": "assistant",
      "content": [
        {
          "type": "thinking",
          "thinking": "Let me analyze...",
          "signature": "abc123..."
        },
        {
          "type": "text",
          "text": "Here's my analysis..."
        }
      ]
    },
    { "role": "user", "content": "Now fix the bug you found." }
  ]
}
```

If you omit thinking blocks from previous turns, Claude loses context about its earlier
reasoning, which can lead to inconsistent responses. However, you can also strip them to
save tokens if the reasoning context isn't needed.

### Streaming Thinking

When streaming is enabled, thinking tokens arrive as `thinking_delta` events:

```
event: content_block_start
data: {"type":"content_block_start","index":0,"content_block":{"type":"thinking","thinking":""}}

event: content_block_delta
data: {"type":"content_block_delta","index":0,"delta":{"type":"thinking_delta","thinking":"Let me analyze"}}

event: content_block_delta
data: {"type":"content_block_delta","index":0,"delta":{"type":"thinking_delta","thinking":" this code step"}}

event: content_block_delta
data: {"type":"content_block_delta","index":0,"delta":{"type":"thinking_delta","thinking":" by step.\n\n"}}

event: content_block_stop
data: {"type":"content_block_stop","index":0}

event: content_block_start
data: {"type":"content_block_start","index":1,"content_block":{"type":"text","text":""}}

event: content_block_delta
data: {"type":"content_block_delta","index":1,"delta":{"type":"text_delta","text":"Here's what I found..."}}
```

This allows applications to show a "thinking" indicator or display the reasoning in real-time.

### Constraints When Thinking Is Enabled

Several important constraints apply when extended thinking is active:

1. **`tool_choice` must be `"auto"`**: You cannot force a specific tool or use `"any"` when
   thinking is enabled. This is because the model needs the freedom to think before deciding
   whether to use a tool.

2. **`temperature` is fixed**: When thinking is enabled, the temperature is fixed and cannot
   be adjusted. The model uses its own internal temperature for the thinking phase.

3. **`top_p` and `top_k`**: Similarly not adjustable with thinking enabled.

4. **System prompt**: Fully supported with thinking.

5. **`max_tokens`**: Must be greater than `budget_tokens`. The `max_tokens` covers both
   thinking tokens and response tokens.

### Cost: Charged for Full Thinking Even When Summarized

This is a critical cost consideration: **you are billed for all thinking tokens the model
generates, even when thinking is summarized or omitted**. If the model uses 5,000 thinking
tokens internally but the summarized output is only 200 tokens, you pay for 5,000.

The `usage` object in the response tells you exactly how many thinking tokens were used:

```json
{
  "usage": {
    "input_tokens": 500,
    "output_tokens": 200,
    "thinking_tokens": 5000
  }
}
```

Total billed output = `output_tokens` + `thinking_tokens` = 5,200 tokens.

### Supported Models

| Model | Thinking Support | Notes |
|-------|-----------------|-------|
| Claude Opus 4 | ✅ Full | Summarized by default |
| Claude Sonnet 4 | ✅ Full | Summarized by default |
| Claude 3.7 Sonnet | ✅ Full | Full thinking visible |
| Claude 3.5 Sonnet | ❌ | Not supported |
| Claude 3.5 Haiku | ❌ | Not supported |
| Claude 3 Opus | ❌ | Not supported |

---

## OpenAI Reasoning Models

OpenAI takes a fundamentally different approach to reasoning. Rather than exposing thinking
as a visible feature, OpenAI's reasoning models (o1, o3, o4-mini, etc.) perform hidden
chain-of-thought reasoning that is **not visible** to the API consumer.

### Model Family

| Model | Released | Notes |
|-------|----------|-------|
| o1-preview | Sep 2023 | First reasoning model, limited availability |
| o1 | Dec 2024 | Full reasoning model |
| o1-mini | Sep 2024 | Faster, cheaper reasoning |
| o3 | Apr 2025 | Next-gen reasoning |
| o3-mini | Jan 2025 | Fast, cost-effective reasoning |
| o4-mini | Apr 2025 | Latest compact reasoning model |

### `reasoning_effort` Parameter

OpenAI's equivalent of Anthropic's `budget_tokens` is the `reasoning_effort` parameter,
which takes discrete levels rather than a token count:

```json
{
  "model": "o4-mini",
  "reasoning_effort": "medium",
  "messages": [
    { "role": "user", "content": "Find the bug in this sorting algorithm..." }
  ]
}
```

| Level | Behavior |
|-------|----------|
| `"low"` | Minimal reasoning, fastest and cheapest |
| `"medium"` | Balanced reasoning depth |
| `"high"` | Maximum reasoning effort, most accurate but slowest and most expensive |

The default varies by model, but `"medium"` is common.

### `reasoning.summary` Parameter (Responses API)

In the newer Responses API, you can request a summary of the model's reasoning:

```json
{
  "model": "o4-mini",
  "reasoning": {
    "effort": "high",
    "summary": "auto"
  },
  "input": [
    { "role": "user", "content": "Solve this complex problem..." }
  ]
}
```

The `summary` field can be:
- `"auto"`: Include a reasoning summary when possible
- `"concise"`: Include a brief summary
- `"detailed"`: Include a more thorough summary

This is similar to Anthropic's summarized thinking — you get insight into the model's
reasoning process without seeing every token.

### Hidden Reasoning Tokens

Unlike Anthropic's approach where thinking blocks are part of the response, OpenAI's
reasoning tokens are **completely hidden**. You don't see them in the response at all.
The model reasons internally, and you only see the final answer.

```json
{
  "choices": [{
    "message": {
      "role": "assistant",
      "content": "The bug is on line 7..."
    }
  }],
  "usage": {
    "prompt_tokens": 150,
    "completion_tokens": 85,
    "total_tokens": 235,
    "completion_tokens_details": {
      "reasoning_tokens": 2048,
      "accepted_prediction_tokens": 0,
      "rejected_prediction_tokens": 0
    }
  }
}
```

The `reasoning_tokens` field in `completion_tokens_details` tells you how many tokens the
model spent on reasoning. Note that `completion_tokens` includes reasoning tokens, so the
actual visible output tokens = `completion_tokens` - `reasoning_tokens`.

### Cost Implications

Reasoning tokens are billed at the same rate as output tokens. Since reasoning models can
generate thousands of reasoning tokens for complex problems, costs can be significantly
higher than standard models:

| Model | Input (per 1M tokens) | Output (per 1M tokens) | Notes |
|-------|----------------------|------------------------|-------|
| gpt-4o | $2.50 | $10.00 | No reasoning tokens |
| o4-mini | $1.10 | $4.40 | + hidden reasoning tokens |
| o3 | $2.00 | $8.00 | + hidden reasoning tokens |

A complex problem with `reasoning_effort: "high"` might use 10,000+ reasoning tokens,
making the effective cost 10-100× a simple GPT-4o completion.

### Limitations of Early Reasoning Models

The o1 family had several notable limitations (many relaxed in later versions):

- **No system message** (o1-preview, o1-mini): Early versions did not support system
  messages at all. This was relaxed in later o1 versions and o3/o4-mini.
- **No temperature control**: Temperature is fixed for reasoning models.
- **No streaming** (early versions): o1-preview didn't support streaming. Later models do.
- **No function calling** (early versions): o1-preview couldn't use tools. o3 and o4-mini
  fully support function calling.
- **No image inputs** (early versions): Some reasoning models couldn't process images.

### How Reasoning Tokens Count in Context Window

Reasoning tokens consume part of the model's context window. For example, if o4-mini has a
200K token context window and generates 10,000 reasoning tokens, only 190K tokens remain
for the conversation history and output.

This is important for agent loops: as the conversation grows, there's less room for reasoning,
which can degrade quality on complex steps. Implementations should monitor total token usage
including reasoning tokens.

---

## DeepSeek R1 Reasoning

DeepSeek R1 is an open-source reasoning model that takes a transparent approach: the
reasoning chain is fully visible in the model's output.

### Visible Reasoning with `<think>` Tags

DeepSeek R1 uses `<think>` tags to delineate the reasoning portion:

```
<think>
Let me analyze this fibonacci function step by step.

The standard recurrence is F(n) = F(n-1) + F(n-2).
Looking at the code: return fibonacci(n - 1) + fibonacci(n - 3)

The second term uses n-3 instead of n-2. That's the bug.

Let me verify with n=4:
- Standard: F(4) = F(3) + F(2) = 2 + 1 = 3
- Buggy: F(4) = F(3) + F(1) = ? 

For F(3) with bug: F(2) + F(0) = ?
For F(2) with bug: F(1) + F(-1) = 1 + (-1) = 0
So F(3) with bug: 0 + 0 = 0
F(4) with bug: 0 + 1 = 1 (should be 3)

Confirmed: the bug is n-3 should be n-2.
</think>

The bug is on line 3. You have `fibonacci(n - 3)` but it should be `fibonacci(n - 2)`.
The Fibonacci recurrence relation is F(n) = F(n-1) + F(n-2), not F(n-1) + F(n-3).
```

### Parsing Thinking from Final Answer

Applications need to parse the `<think>` tags to separate reasoning from the final answer:

```python
import re

def parse_deepseek_response(text):
    think_match = re.search(r'<think>(.*?)</think>', text, re.DOTALL)
    thinking = think_match.group(1).strip() if think_match else ""
    answer = re.sub(r'<think>.*?</think>', '', text, flags=re.DOTALL).strip()
    return thinking, answer
```

### Advantages of the DeepSeek Approach

1. **Fully transparent**: You see exactly what the model is thinking
2. **Open-source**: Can be run locally, fine-tuned, and inspected
3. **No additional API complexity**: Standard text generation, just with tags
4. **Cost control**: You can see (and limit) thinking length directly

### Disadvantages

1. **Token waste**: Thinking tokens are part of the regular output, consuming the same
   context window and output limit as the answer
2. **No structured separation**: Relying on text tags is less robust than API-level support
3. **Quality**: While impressive for an open model, reasoning quality generally trails
   the proprietary alternatives on the hardest problems

---

## Google Gemini Thinking

Google's Gemini 2.5 models include thinking capabilities, offered as "thinking mode" or
"reasoning mode."

### Gemini 2.5 Flash and Pro Thinking

Gemini 2.5 Flash and 2.5 Pro support a thinking mode that can be configured:

```json
{
  "model": "gemini-2.5-flash",
  "contents": [
    { "role": "user", "parts": [{ "text": "Explain the bug in this code..." }] }
  ],
  "generationConfig": {
    "thinkingConfig": {
      "thinkingBudget": 8192
    }
  }
}
```

### `thinkingConfig` Parameters

| Parameter | Type | Description |
|-----------|------|-------------|
| `thinkingBudget` | integer | Maximum tokens for thinking (0 to disable, -1 for dynamic) |

Setting `thinkingBudget: 0` disables thinking entirely. Setting it to `-1` lets the model
decide dynamically how much to think.

### Thinking in Response

When thinking is enabled, the response includes a `thought` field in the model's parts:

```json
{
  "candidates": [{
    "content": {
      "parts": [
        {
          "thought": true,
          "text": "Let me work through this step by step..."
        },
        {
          "text": "The bug is in the comparison on line 5..."
        }
      ],
      "role": "model"
    }
  }],
  "usageMetadata": {
    "promptTokenCount": 100,
    "candidatesTokenCount": 250,
    "totalTokenCount": 350,
    "thoughtsTokenCount": 180
  }
}
```

Parts with `"thought": true` contain the reasoning. The `thoughtsTokenCount` in usage
metadata shows how many tokens were used for thinking.

---

## When Thinking Helps for Coding

### Complex Debugging and Root Cause Analysis

Thinking excels when the model needs to trace through code execution, consider multiple
hypotheses, and systematically eliminate possibilities:

```
User: "This API endpoint returns 500 errors intermittently in production but works
fine locally. Here's the code..."

Without thinking: Model might suggest generic fixes (add error handling, check null)
With thinking: Model systematically considers race conditions, connection pooling,
environment differences, timeouts, and identifies the specific race condition
in the database transaction handling.
```

### Architectural Decisions

When the model needs to weigh tradeoffs:

```
User: "Should I use a microservices or monolithic architecture for this project?"

Without thinking: Generic pros/cons list
With thinking: Model considers the specific project requirements, team size,
deployment constraints, and gives a nuanced, justified recommendation.
```

### Multi-File Refactoring Planning

Before making changes across many files, thinking helps the model plan the order of
operations and identify dependencies:

```
User: "Rename the User class to Account across the entire codebase."

Without thinking: Might miss some usages or break circular dependencies
With thinking: Model maps out all files, identifies the dependency graph,
plans the order of changes, and considers edge cases like string references
and configuration files.
```

### Algorithm Design

Thinking is valuable when the model needs to reason about correctness:

```
User: "Implement a concurrent-safe LRU cache with TTL support."

Without thinking: Might have subtle concurrency bugs
With thinking: Model reasons through lock ordering, race conditions,
cache invalidation timing, and produces a correct implementation.
```

### When NOT to Use Thinking

Thinking adds latency and cost. Don't use it for:

- **Simple code completions**: Autocomplete-style suggestions don't need reasoning
- **Formatting or style changes**: Mechanical transformations need no thinking
- **Simple Q&A**: "What does `Array.prototype.map` do?"
- **Boilerplate generation**: Creating standard CRUD endpoints, config files, etc.
- **Cost-sensitive applications**: High-volume, low-complexity tasks where thinking
  tokens would dominate the cost

---

## Cost Implications

### Token Costs for Thinking

Thinking tokens are typically billed at output token rates, which are the most expensive:

| Provider | Model | Thinking Token Cost |
|----------|-------|-------------------|
| Anthropic | Claude Sonnet 4 | Same as output ($15/1M tokens) |
| Anthropic | Claude Opus 4 | Same as output ($75/1M tokens) |
| OpenAI | o4-mini | Same as output ($4.40/1M tokens) |
| OpenAI | o3 | Same as output ($8/1M tokens) |
| Google | Gemini 2.5 Flash | Reduced rate for thinking tokens |
| DeepSeek | R1 | Same as output (very cheap) |

### Budget Optimization Strategies

1. **Start with low budgets**: Begin with `budget_tokens: 2000` and increase only if
   the quality is insufficient.

2. **Use adaptive thinking**: Let the model decide how much to think rather than setting
   a fixed high budget.

3. **Task-based budgets**: Use low reasoning effort for simple tasks, high for complex ones:
   ```python
   def get_thinking_budget(task_complexity):
       if task_complexity == "simple":
           return 1024      # Quick clarifications
       elif task_complexity == "moderate":
           return 5000      # Standard coding tasks
       elif task_complexity == "complex":
           return 15000     # Multi-file refactoring, debugging
       else:
           return 30000     # Architectural decisions, deep analysis
   ```

4. **Monitor and adjust**: Track thinking token usage over time and optimize budgets
   based on actual needs.

5. **Cache thinking results**: For repeated similar queries, cache the final answer
   rather than re-running thinking.

### When to Use Low vs High Reasoning Effort

| Scenario | Recommended Effort | Rationale |
|----------|-------------------|-----------|
| Code completion / autocomplete | None (standard model) | Speed is critical, low complexity |
| Simple bug fix | Low | Quick reasoning sufficient |
| Code review | Medium | Needs some analysis but not deep |
| Complex debugging | High | Needs systematic investigation |
| Architecture design | High | Needs thorough tradeoff analysis |
| Multi-file refactoring | High | Needs to understand dependencies |
| Writing documentation | Low/None | Mostly generation, not reasoning |
| Test case generation | Medium | Needs to identify edge cases |

---

## How Coding Agents Configure Thinking

### Dynamic Thinking Budgets Based on Task Complexity

Sophisticated coding agents adjust thinking parameters based on what they're doing:

```python
class ThinkingConfig:
    def for_task(self, task_type: str, error_count: int = 0) -> dict:
        base_budget = {
            "file_read": 0,           # No thinking needed
            "simple_edit": 1024,      # Minimal thinking
            "code_generation": 4000,   # Moderate thinking
            "debugging": 10000,        # Significant thinking
            "refactoring": 15000,      # Deep thinking
            "architecture": 25000,     # Maximum thinking
        }.get(task_type, 5000)

        # Increase budget after errors (model needs to think harder)
        error_multiplier = min(1 + (error_count * 0.5), 3.0)
        adjusted_budget = int(base_budget * error_multiplier)

        return {
            "type": "enabled",
            "budget_tokens": adjusted_budget
        }
```

### How Copilot Configures Thinking

GitHub Copilot's agent mode uses thinking strategically:

- **Initial task analysis**: High thinking budget to understand the task and plan approach
- **File reading/searching**: No thinking (mechanical operations)
- **Code generation**: Moderate thinking to produce correct code
- **Error recovery**: Increased thinking budget when previous attempts failed
- **Final verification**: Moderate thinking to review changes

### How Cursor Configures Thinking

Cursor's approach to thinking typically involves:

- Using Claude's extended thinking for complex "agent" mode operations
- Dynamically adjusting based on task complexity
- Streaming thinking tokens for user visibility
- Using thinking primarily for planning steps in multi-file edits

### Adaptive Thinking Based on Error Patterns

A common pattern in coding agents: increase thinking effort after failures:

```python
class AdaptiveThinking:
    def __init__(self):
        self.consecutive_errors = 0
        self.base_budget = 4000

    def get_budget(self) -> int:
        return min(
            self.base_budget * (2 ** self.consecutive_errors),
            50000  # Cap at 50K tokens
        )

    def on_success(self):
        self.consecutive_errors = 0

    def on_error(self):
        self.consecutive_errors += 1
```

This pattern reflects the intuition that if the model keeps failing, it needs to "think
harder" about the problem—which often works, especially for debugging tasks where the model
needs to consider more hypotheses.

---

## Comparison Table of Thinking Across Providers

| Feature | Anthropic Claude | OpenAI o-series | DeepSeek R1 | Google Gemini |
|---------|-----------------|-----------------|-------------|---------------|
| **Thinking visibility** | Summarized (Claude 4) or full (3.7) | Hidden (summary optional) | Fully visible (`<think>` tags) | Visible (`thought: true` parts) |
| **Configuration** | `thinking.budget_tokens` | `reasoning_effort` | N/A (always on) | `thinkingConfig.thinkingBudget` |
| **Granularity** | Token count (1024+) | Three levels (low/med/high) | No control | Token count |
| **Tool use + thinking** | ✅ Interleaved | ✅ (o3, o4-mini) | ❌ Limited | ✅ Supported |
| **Streaming** | ✅ `thinking_delta` events | ✅ (newer models) | ✅ (standard streaming) | ✅ Supported |
| **Cost model** | Billed at output rate | Billed at output rate | Billed at output rate | Reduced rate |
| **Multi-turn preservation** | Via `signature` field | Automatic (hidden) | Manual (text in conversation) | Automatic |
| **Disable thinking** | Omit `thinking` param | Use standard model (gpt-4o) | Use different model | `thinkingBudget: 0` |
| **Max thinking tokens** | Model-dependent | Model-dependent | Output limit | Model-dependent |
| **Temperature control** | Fixed with thinking | Fixed | Standard | Configurable |
| **Open source** | ❌ | ❌ | ✅ | ❌ |

---

## Code Examples: Thinking Configuration for Each Provider

### Anthropic Claude (Python SDK)

```python
import anthropic

client = anthropic.Anthropic()

# Basic extended thinking
response = client.messages.create(
    model="claude-sonnet-4-20250514",
    max_tokens=16000,
    thinking={
        "type": "enabled",
        "budget_tokens": 10000
    },
    messages=[{
        "role": "user",
        "content": "Find and fix all bugs in this code: ..."
    }]
)

# Process thinking and text blocks
for block in response.content:
    if block.type == "thinking":
        print(f"[Thinking] {block.thinking[:200]}...")
    elif block.type == "text":
        print(f"[Answer] {block.text}")

print(f"Thinking tokens used: {response.usage.thinking_tokens}")
```

### Anthropic Claude with Tool Use and Thinking

```python
response = client.messages.create(
    model="claude-sonnet-4-20250514",
    max_tokens=16000,
    thinking={
        "type": "enabled",
        "budget_tokens": 8000
    },
    tools=[{
        "name": "read_file",
        "description": "Read file contents",
        "input_schema": {
            "type": "object",
            "properties": {
                "path": {"type": "string"}
            },
            "required": ["path"]
        }
    }],
    # tool_choice MUST be "auto" with thinking enabled
    tool_choice={"type": "auto"},
    messages=[{
        "role": "user",
        "content": "Read and analyze the auth module"
    }]
)
```

### OpenAI Reasoning (Python SDK)

```python
from openai import OpenAI

client = OpenAI()

# Using reasoning effort
response = client.chat.completions.create(
    model="o4-mini",
    reasoning_effort="high",
    messages=[{
        "role": "user",
        "content": "Find the concurrency bug in this code: ..."
    }]
)

print(response.choices[0].message.content)
print(f"Reasoning tokens: {response.usage.completion_tokens_details.reasoning_tokens}")
```

### OpenAI Responses API with Reasoning Summary

```python
response = client.responses.create(
    model="o4-mini",
    reasoning={
        "effort": "high",
        "summary": "detailed"
    },
    input=[{
        "role": "user",
        "content": "Design an efficient caching strategy for this API..."
    }]
)

for item in response.output:
    if item.type == "reasoning":
        print(f"[Reasoning Summary] {item.summary}")
    elif item.type == "message":
        print(f"[Answer] {item.content}")
```

### Google Gemini (Python SDK)

```python
import google.generativeai as genai

model = genai.GenerativeModel(
    model_name="gemini-2.5-flash",
    generation_config={
        "thinking_config": {
            "thinking_budget": 8192
        }
    }
)

response = model.generate_content(
    "Analyze this distributed system design for potential failure modes: ..."
)

for part in response.candidates[0].content.parts:
    if hasattr(part, 'thought') and part.thought:
        print(f"[Thought] {part.text[:200]}...")
    else:
        print(f"[Answer] {part.text}")
```

### DeepSeek R1 (OpenAI-Compatible API)

```python
from openai import OpenAI

# DeepSeek uses OpenAI-compatible API
client = OpenAI(
    api_key="your-deepseek-key",
    base_url="https://api.deepseek.com"
)

response = client.chat.completions.create(
    model="deepseek-reasoner",
    messages=[{
        "role": "user",
        "content": "Find the memory leak in this C++ code: ..."
    }]
)

full_text = response.choices[0].message.content

# Parse thinking from response
import re
think_match = re.search(r'<think>(.*?)</think>', full_text, re.DOTALL)
if think_match:
    thinking = think_match.group(1)
    answer = full_text[think_match.end():].strip()
    print(f"[Thinking] {thinking[:200]}...")
    print(f"[Answer] {answer}")
else:
    print(full_text)
```

---

## Conclusion

Extended thinking represents a significant evolution in LLM capabilities. By giving models
an explicit reasoning phase, we get substantially better performance on complex tasks—at the
cost of increased latency and token usage.

For coding agents, the key insight is that thinking should be **adaptive**: use it heavily
for planning, debugging, and complex reasoning, but skip it for mechanical operations like
file reading and simple edits. The best implementations dynamically adjust thinking budgets
based on task complexity, error patterns, and cost constraints.

The provider landscape is converging on the concept but diverging on implementation:
Anthropic offers the most transparent and controllable thinking, OpenAI provides the most
powerful (if opaque) reasoning, DeepSeek offers full transparency as open source, and Google
provides a middle ground. Understanding these differences is essential for building effective
coding tools that leverage the right thinking strategy for each situation.