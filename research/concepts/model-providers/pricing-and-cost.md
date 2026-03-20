# LLM Pricing and Cost Optimization for Coding Agents

## Overview

LLM costs are the primary operational expense for CLI coding agents. A single complex
coding session can cost anywhere from $0.01 (DeepSeek) to $10.00 (Claude Opus with
extended thinking), making cost optimization critical for sustainable agent usage. This
document provides comprehensive pricing data for all major providers and actionable
strategies for minimizing costs without sacrificing quality.

---

## Master Pricing Table (2025)

### Per Million Tokens (USD)

| Model | Input | Cached Input | Output | Batch Input | Batch Output |
|-------|-------|-------------|--------|------------|-------------|
| **OpenAI** | | | | | |
| GPT-4.1 | $2.00 | $0.50 | $8.00 | $1.00 | $4.00 |
| GPT-4.1 mini | $0.40 | $0.10 | $1.60 | $0.20 | $0.80 |
| GPT-4.1 nano | $0.10 | $0.025 | $0.40 | $0.05 | $0.20 |
| GPT-4o | $2.50 | $1.25 | $10.00 | $1.25 | $5.00 |
| GPT-4o mini | $0.15 | $0.075 | $0.60 | $0.075 | $0.30 |
| o3 | $2.00 | $0.50 | $8.00 | $1.00 | $4.00 |
| o4-mini | $1.10 | $0.275 | $4.40 | $0.55 | $2.20 |
| **Anthropic** | | | | | |
| Claude Opus 4.6 | $5.00 | $0.50 | $25.00 | $2.50 | $12.50 |
| Claude Sonnet 4.6 | $3.00 | $0.30 | $15.00 | $1.50 | $7.50 |
| Claude Haiku 4.5 | $1.00 | $0.10 | $5.00 | $0.50 | $2.50 |
| Claude Haiku 3.5 | $0.80 | $0.08 | $4.00 | $0.40 | $2.00 |
| **Google** | | | | | |
| Gemini 2.5 Pro (≤200K) | $1.25 | $0.3125 | $10.00 | $0.625 | $5.00 |
| Gemini 2.5 Pro (>200K) | $2.50 | $0.625 | $15.00 | $1.25 | $7.50 |
| Gemini 2.5 Flash (≤200K) | $0.15 | $0.0375 | $0.60 | $0.075 | $0.30 |
| Gemini 2.5 Flash (>200K) | $0.30 | $0.075 | $1.20 | $0.15 | $0.60 |
| Gemini 2.5 Flash-Lite | $0.075 | — | $0.30 | — | — |
| **DeepSeek** | | | | | |
| DeepSeek-V3.2 (chat) | $0.28 | $0.028 | $0.42 | — | — |
| DeepSeek-V3.2 (reasoner) | $0.28 | $0.028 | $0.42 | — | — |
| **Local/Self-Hosted** | | | | | |
| Ollama (any model) | $0.00 | $0.00 | $0.00 | $0.00 | $0.00 |
| vLLM (self-hosted) | $0.00* | $0.00* | $0.00* | $0.00* | $0.00* |

*\* Hardware/electricity costs not included. See self-hosting section.*

### Special Pricing

| Feature | Provider | Price |
|---------|----------|-------|
| **Anthropic Fast Mode** (Opus 4.6) | Anthropic | $30 / MTok input, $150 / MTok output (6x) |
| **Anthropic Data Residency** (US) | Anthropic | 1.1x multiplier on all prices |
| **Anthropic Long Context** (Sonnet 4.5) | Anthropic | 2x input, 1.5x output for >200K tokens |
| **Gemini Thinking Tokens** (Pro) | Google | $3.50-7.00 / MTok |
| **Gemini Thinking Tokens** (Flash) | Google | $1.50-3.00 / MTok |
| **OpenAI Reasoning Tokens** (o3) | OpenAI | Billed at output rate ($8.00 / MTok) |

---

## Cost Per Coding Task

### Typical Token Usage

| Task Type | System Prompt | Context | User Message | Output | Total Tokens |
|-----------|--------------|---------|-------------|--------|-------------|
| Simple bug fix | 3K | 5K | 500 | 2K | ~10.5K |
| Add feature | 3K | 20K | 1K | 8K | ~32K |
| Multi-file refactor | 3K | 50K | 2K | 20K | ~75K |
| Architecture redesign | 3K | 100K | 5K | 30K | ~138K |
| Full SWE-bench task | 3K | 30K | 2K | 10K | ~45K |

### Cost Per Task by Provider

Based on the token profiles above:

| Task | Claude Sonnet | GPT-4.1 | Gemini Pro | Gemini Flash | DeepSeek | Local |
|------|-------------|---------|-----------|-------------|----------|-------|
| Simple bug fix | $0.055 | $0.033 | $0.031 | $0.003 | $0.003 | $0.00 |
| Add feature | $0.192 | $0.128 | $0.110 | $0.009 | $0.010 | $0.00 |
| Multi-file refactor | $0.465 | $0.310 | $0.263 | $0.021 | $0.022 | $0.00 |
| Architecture redesign | $0.864 | $0.516 | $0.475 | $0.039 | $0.051 | $0.00 |
| SWE-bench task | $0.255 | $0.150 | $0.138 | $0.011 | $0.013 | $0.00 |

### Multi-Turn Session Cost

A 20-turn coding session (system prompt + growing conversation):

| Provider/Model | Without Caching | With Caching | Savings |
|---------------|----------------|-------------|---------|
| Claude Opus 4.6 | $12.50 | $2.50 | 80% |
| Claude Sonnet 4.6 | $7.50 | $1.50 | 80% |
| GPT-4.1 | $5.00 | $2.50 | 50% |
| Gemini 2.5 Pro | $4.00 | $1.50 | 63% |
| Gemini 2.5 Flash | $0.40 | $0.15 | 63% |
| DeepSeek-V3.2 | $0.30 | $0.06 | 80% |

---

## SWE-bench Cost Analysis

SWE-bench Verified is the gold standard benchmark for coding agents. Here's the
cost-quality trade-off:

### Cost per SWE-bench Task

| Agent/Model | SWE-bench Score | Avg Cost/Task | Cost per Solved Task |
|-------------|----------------|--------------|---------------------|
| Claude Code (Opus 4.6) | ~58% | $2.00 | $3.45 |
| Claude Code (Sonnet 4.6) | ~55% | $0.50 | $0.91 |
| OpenHands (Claude Opus) | ~53% | $1.80 | $3.40 |
| Aider (GPT-4.1) | ~50% | $0.30 | $0.60 |
| Aider (Gemini 2.5 Pro) | ~48% | $0.25 | $0.52 |
| mini-SWE-agent (Gemini 3 Pro) | ~74% | $0.20 | $0.27 |
| Aider (DeepSeek-V3.2) | ~40% | $0.02 | $0.05 |

### Cost Efficiency Ranking (Cost per Solved Task)

```
Most Efficient                              Least Efficient
───────────────────────────────────────────────────────
DeepSeek   Gemini    GPT-4.1   Sonnet    Opus
$0.05      $0.27     $0.60     $0.91     $3.45
per solve  per solve per solve per solve per solve
```

---

## Cost Optimization Strategies

### 1. Prompt Caching

The highest-impact optimization for multi-turn agent sessions:

| Provider | How | Savings on Cached Tokens |
|----------|-----|------------------------|
| Anthropic | Explicit cache breakpoints | 90% |
| OpenAI | Automatic (>1024 token prefix match) | 50% |
| Google | Context caching API | 75% |
| DeepSeek | Automatic disk caching | 90% |

```python
# Anthropic prompt caching example
# Place stable content early, variable content late
system = [{
    "type": "text",
    "text": SYSTEM_PROMPT + CODEBASE_CONTEXT,  # ~50K tokens
    "cache_control": {"type": "ephemeral"}     # Cached for 5 min
}]

# Turn 1: 50K tokens × $3.75/MTok (cache write) = $0.19
# Turn 2-20: 50K tokens × $0.30/MTok (cache hit) × 19 = $0.29
# Total: $0.48 vs $3.00 without caching = 84% savings
```

### 2. Model Tiering

Use expensive models only when needed:

```python
# Cost-tiered model selection
def select_model(task_complexity: str) -> dict:
    tiers = {
        "trivial": {"model": "deepseek-chat", "cost": "$0.001/task"},
        "simple": {"model": "claude-haiku-4-5", "cost": "$0.01/task"},
        "moderate": {"model": "claude-sonnet-4-6", "cost": "$0.15/task"},
        "complex": {"model": "claude-opus-4-6", "cost": "$0.50/task"},
    }
    return tiers[task_complexity]
```

**Impact:** If 60% of tasks are simple and 40% are complex:
- Always Sonnet: 100 tasks × $0.15 = $15.00
- Tiered: 60 × $0.01 + 40 × $0.25 = $10.60 (29% savings)

### 3. Batch API Discounts

For non-interactive tasks, use the Batch API for 50% off:

| Provider | Batch Discount | Use Cases |
|----------|---------------|-----------|
| OpenAI | 50% off all tokens | Batch code review, test generation |
| Anthropic | 50% off all tokens | Bulk refactoring, documentation |
| Google | 50% off all tokens | Evaluation runs, benchmarks |

```python
# Use batch for non-interactive tasks
if not task.requires_interaction:
    response = await batch_api.create(tasks)
    # 50% cheaper than real-time API
```

### 4. Context Management

Reduce token usage by managing what goes into context:

| Strategy | Token Savings | Quality Impact |
|----------|-------------|---------------|
| Summarize old conversation turns | 40-60% | Minimal |
| Only include relevant files | 50-80% | Depends on selection quality |
| Truncate large file contents | 20-40% | Some information loss |
| Use embeddings for retrieval | 60-80% | Depends on retrieval quality |

```python
# Context compression example
def compress_context(messages, max_tokens=50000):
    """Keep recent messages, summarize older ones."""
    recent = messages[-4:]  # Keep last 4 messages verbatim
    older = messages[:-4]
    
    if estimate_tokens(older) > max_tokens / 2:
        summary = summarize_conversation(older)  # Use cheap model
        return [{"role": "system", "content": f"Previous conversation summary: {summary}"}] + recent
    
    return messages
```

### 5. Token Counting

Track token usage to avoid surprises:

```python
import tiktoken

def count_tokens(text: str, model: str = "gpt-4") -> int:
    """Count tokens for OpenAI models."""
    encoding = tiktoken.encoding_for_model(model)
    return len(encoding.encode(text))

def estimate_cost(input_tokens: int, output_tokens: int, model: str) -> float:
    """Estimate cost based on token counts."""
    pricing = {
        "gpt-4.1": {"input": 2.0, "output": 8.0},
        "claude-sonnet-4-6": {"input": 3.0, "output": 15.0},
        "deepseek-chat": {"input": 0.28, "output": 0.42},
    }
    
    p = pricing.get(model, pricing["gpt-4.1"])
    return (input_tokens * p["input"] + output_tokens * p["output"]) / 1_000_000
```

### 6. Self-Hosting Economics

When does self-hosting become cheaper than API usage?

```
Monthly API Spend:  $100  $500  $1000  $5000  $10000
────────────────────────────────────────────────
Self-hosting cost:  $1500-3000/month (cloud GPU)
Break-even:               ✓      ✓       ✓

API is cheaper below ~$2000/month
Self-hosting is cheaper above ~$2000/month
```

**Cloud GPU costs:**

| GPU Setup | Monthly Cost | Models It Can Run |
|-----------|-------------|-------------------|
| 1x A100 80GB | ~$1,500 | Up to 70B (Q4) |
| 2x A100 80GB | ~$3,000 | Up to 70B (FP16) |
| 4x A100 80GB | ~$6,000 | Up to 405B (Q4) |
| 8x H100 80GB | ~$20,000 | DeepSeek-V3 full |
| RTX 4090 (owned) | ~$30/mo (electricity) | Up to 32B (Q4) |
| M4 Max 128GB (owned) | ~$10/mo (electricity) | Up to 70B (Q4) |

---

## Budget Management in Agents

### Per-Session Budget Limits

```python
class BudgetManager:
    def __init__(self, session_budget: float = 5.0, daily_budget: float = 50.0):
        self.session_budget = session_budget
        self.daily_budget = daily_budget
        self.session_spent = 0.0
        self.daily_spent = 0.0
    
    def can_afford(self, estimated_cost: float) -> bool:
        return (self.session_spent + estimated_cost <= self.session_budget and
                self.daily_spent + estimated_cost <= self.daily_budget)
    
    def record(self, actual_cost: float):
        self.session_spent += actual_cost
        self.daily_spent += actual_cost
    
    def get_warning(self) -> str | None:
        session_pct = self.session_spent / self.session_budget
        if session_pct > 0.9:
            return f"⚠️ Session budget 90% consumed (${self.session_spent:.2f}/${self.session_budget:.2f})"
        elif session_pct > 0.7:
            return f"Session budget 70% consumed (${self.session_spent:.2f}/${self.session_budget:.2f})"
        return None
```

### Cost-Aware Model Downgrade

```python
async def cost_aware_completion(messages, budget_manager):
    """Automatically downgrade models as budget is consumed."""
    remaining = budget_manager.session_budget - budget_manager.session_spent
    
    if remaining > 2.0:
        model = "anthropic/claude-sonnet-4-6"
    elif remaining > 0.5:
        model = "openai/gpt-4.1-mini"
    elif remaining > 0.1:
        model = "deepseek/deepseek-chat"
    else:
        raise BudgetExhaustedError("Session budget exhausted")
    
    response = await litellm.acompletion(model=model, messages=messages)
    cost = litellm.completion_cost(completion_response=response)
    budget_manager.record(cost)
    
    return response
```

---

## Hidden Costs

### Reasoning Token Costs

Reasoning models (o3, o4-mini, DeepSeek-R1, Claude with extended thinking) generate
hidden "thinking" tokens billed at the output rate:

| Model | Visible Output | Hidden Thinking | Actual Cost Multiplier |
|-------|---------------|----------------|----------------------|
| o3 (high effort) | 2K tokens | 20-50K tokens | 10-25x |
| o4-mini | 2K tokens | 10-30K tokens | 5-15x |
| Claude Opus (extended) | 2K tokens | 5-20K tokens | 3-10x |
| DeepSeek-R1 | 2K tokens | 5-15K tokens | 3-8x |

**Example:** An o3 request that produces 2K visible output tokens might generate
30K reasoning tokens → total output billed: 32K × $8/MTok = $0.26, not the
expected $0.016.

### Tool Use Token Overhead

Each tool definition adds tokens to every request:

| Tools Defined | Extra Tokens (per request) | Cost at $3/MTok |
|---------------|---------------------------|-----------------|
| 5 tools | ~500 tokens | $0.0015 |
| 10 tools | ~1,200 tokens | $0.0036 |
| 20 tools | ~2,500 tokens | $0.0075 |
| 50 tools | ~6,000 tokens | $0.0180 |

Over 100 turns, 20 tools adds ~$0.75 in overhead.

### System Prompt Costs

Large system prompts compound across turns:

| System Prompt Size | Cost per Turn (Sonnet) | Cost per 20 turns (no cache) | With Cache |
|-------------------|----------------------|---------------------------|------------|
| 1K tokens | $0.003 | $0.06 | $0.006 |
| 5K tokens | $0.015 | $0.30 | $0.030 |
| 20K tokens | $0.060 | $1.20 | $0.120 |
| 50K tokens | $0.150 | $3.00 | $0.300 |

---

## Provider Comparison: Best Value

### Best Value by Use Case

| Use Case | Best Value | Cost | Quality |
|----------|-----------|------|---------|
| **Maximum quality** | Claude Opus 4.6 | $$$$$ | ★★★★★ |
| **Best overall** | Claude Sonnet 4.6 | $$$ | ★★★★☆ |
| **Best for budget** | DeepSeek-V3.2 | $ | ★★★☆☆ |
| **Best free** | Gemini 2.5 Flash (free tier) | Free | ★★★☆☆ |
| **Best for privacy** | Ollama/local | Free* | ★★★☆☆ |
| **Best for long context** | Gemini 2.5 Pro | $$ | ★★★★☆ |
| **Best for reasoning** | o3 | $$$$ | ★★★★★ |
| **Best batch** | GPT-4.1 (batch) | $$ | ★★★★☆ |
| **Best high-volume** | Gemini 2.5 Flash | $ | ★★★☆☆ |

### Monthly Cost Estimates by Usage Level

| Usage Level | Tasks/Day | Claude Sonnet | GPT-4.1 | DeepSeek | Local |
|-------------|-----------|-------------|---------|----------|-------|
| **Light** | 5 | $25 | $15 | $0.50 | $0 |
| **Moderate** | 25 | $125 | $75 | $2.50 | $0 |
| **Heavy** | 100 | $500 | $300 | $10 | $0 |
| **Team (5 devs)** | 500 | $2,500 | $1,500 | $50 | $0 |

---

## Cost Tracking Implementation

### Real-Time Cost Display

```python
class CostTracker:
    """Track and display costs in real-time."""
    
    def __init__(self):
        self.session_costs = []
        self.total = 0.0
    
    def add(self, model: str, input_tokens: int, output_tokens: int, cost: float):
        self.session_costs.append({
            "model": model,
            "input_tokens": input_tokens,
            "output_tokens": output_tokens,
            "cost": cost,
            "timestamp": time.time()
        })
        self.total += cost
    
    def display(self):
        """Show cost summary in terminal."""
        print(f"\n💰 Session cost: ${self.total:.4f}")
        print(f"   Turns: {len(self.session_costs)}")
        if self.session_costs:
            avg = self.total / len(self.session_costs)
            print(f"   Avg per turn: ${avg:.4f}")
            
            # Per-model breakdown
            by_model = {}
            for entry in self.session_costs:
                model = entry["model"]
                by_model[model] = by_model.get(model, 0) + entry["cost"]
            
            for model, cost in sorted(by_model.items(), key=lambda x: -x[1]):
                print(f"   {model}: ${cost:.4f}")
```

### Dashboard Metrics

```sql
-- Track costs in a database for long-term analysis
CREATE TABLE usage_log (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    timestamp DATETIME DEFAULT CURRENT_TIMESTAMP,
    model TEXT NOT NULL,
    input_tokens INTEGER,
    output_tokens INTEGER,
    cached_tokens INTEGER DEFAULT 0,
    cost_usd REAL,
    task_type TEXT,
    success BOOLEAN
);

-- Daily cost summary
SELECT 
    DATE(timestamp) as day,
    model,
    SUM(cost_usd) as total_cost,
    COUNT(*) as requests,
    AVG(cost_usd) as avg_cost_per_request
FROM usage_log
GROUP BY DATE(timestamp), model
ORDER BY day DESC;

-- Cost per successful task by model
SELECT 
    model,
    SUM(cost_usd) / NULLIF(SUM(CASE WHEN success THEN 1 ELSE 0 END), 0) 
        as cost_per_success
FROM usage_log
GROUP BY model;
```

---

## Free Tier Comparison

| Provider | Free Models | Limits | Best For |
|----------|-----------|--------|---------|
| Google AI Studio | Gemini 2.5 Flash, Pro | 10-500 req/day | Prototyping |
| OpenAI | GPT-4o mini | Very limited | Testing |
| Anthropic | None | — | — |
| DeepSeek | None (but very cheap) | — | Budget users |
| Ollama | All open models | Hardware only | Privacy, offline |
| OpenRouter | Free model tier | Limited | Experimentation |

---

## See Also

- [OpenAI](openai.md) — OpenAI-specific pricing details
- [Anthropic](anthropic.md) — Anthropic pricing and caching
- [Google](google.md) — Google pricing and free tier
- [DeepSeek](deepseek.md) — DeepSeek's extreme cost efficiency
- [Model Routing](model-routing.md) — Cost-based routing strategies
- [API Patterns](api-patterns.md) — Implementing cost tracking in retry logic