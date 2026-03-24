# Model Providers in CLI Coding Agents

## Overview

Model providers are the backbone of every CLI coding agent. They supply the large language
models (LLMs) that power code generation, reasoning, tool orchestration, and autonomous
software engineering. Understanding how agents integrate with—and abstract over—multiple
providers is essential for evaluating agent architectures, predicting costs, and choosing
the right tool for a given workflow.

This section of the research library covers every major model provider used by CLI coding
agents, the abstraction layers that unify them, and the routing strategies that decide
which model handles which task.

---

## Why Model Providers Matter

A coding agent is only as capable as the model behind it. The choice of provider determines:

| Dimension | Impact |
|-----------|--------|
| **Intelligence ceiling** | Whether the agent can solve hard SWE-bench problems |
| **Context window** | How much code the agent can "see" at once |
| **Latency** | How fast the agent responds to each turn |
| **Cost** | Whether a task costs $0.02 or $2.00 |
| **Tool calling** | Native function calling vs. prompt-based parsing |
| **Availability** | Rate limits, regional restrictions, uptime |
| **Privacy** | Whether code leaves your network |

The best agents decouple themselves from any single provider, giving users the power to
choose based on their own constraints.

---

## The Provider Landscape (2025)

### Tier 1: Frontier Cloud Providers

These providers offer the most capable models and are the default choice for most agents:

| Provider | Key Models | Strengths |
|----------|-----------|-----------|
| **[OpenAI](openai.md)** | GPT-4.1, GPT-4o, o3/o4-mini | Best-in-class function calling, Responses API, massive ecosystem |
| **[Anthropic](anthropic.md)** | Claude Opus 4.6, Sonnet 4.6, Haiku 4.5 | Extended thinking, 1M context, prompt caching, agentic tool use |
| **[Google](google.md)** | Gemini 2.5 Pro/Flash, Gemini 3 | 1M+ context, multimodal, grounding with Search |

### Tier 2: Cost-Optimized Providers

| Provider | Key Models | Strengths |
|----------|-----------|-----------|
| **[DeepSeek](deepseek.md)** | DeepSeek-V3.2, R1 reasoning | 10-50x cheaper than GPT-4, open weights, MoE efficiency |
| **xAI** | Grok 4 | Strong coding, competitive pricing |

### Tier 3: Open-Source / Local

| Provider | Key Models | Strengths |
|----------|-----------|-----------|
| **[Open-source](open-source-models.md)** | Llama 3, Qwen2.5-Coder, CodeLlama | Free, private, self-hosted, no rate limits |

### Meta-Providers (Abstraction Layers)

| Provider | Purpose |
|----------|---------|
| **[LiteLLM](litellm.md)** | Unified Python SDK + proxy for 100+ providers |
| **[OpenRouter](model-routing.md)** | Unified API gateway with cost-based routing |

---

## How Agents Integrate with Providers

Agents take one of four architectural approaches to model provider integration:

### 1. Single-Provider Lock-In

The simplest approach: the agent is built for one provider and one API.

```
┌─────────────┐     ┌──────────────┐
│  Claude Code │────▶│  Anthropic   │
│              │     │  Messages API│
└─────────────┘     └──────────────┘
```

**Examples:**
- **Claude Code** → Anthropic only (Claude Sonnet/Opus/Haiku)
- **Gemini CLI** → Google only (Gemini 2.5/3 Flash)
- **Codex CLI** → OpenAI only (GPT-4.1, o3/o4-mini) + custom compatible endpoints

**Pros:** Deep integration, optimized for provider features (extended thinking, prompt
caching), simpler codebase.

**Cons:** Vendor lock-in, no fallback if provider goes down, can't optimize cost.

### 2. Unified SDK Abstraction (LiteLLM)

The agent delegates all provider communication to LiteLLM, which translates a single
`completion()` call into the correct provider-specific API.

```
┌─────────────┐     ┌──────────┐     ┌──────────────┐
│   Aider     │────▶│ LiteLLM  │────▶│ OpenAI       │
│             │     │          │────▶│ Anthropic    │
│             │     │          │────▶│ Google       │
│             │     │          │────▶│ Ollama       │
└─────────────┘     └──────────┘     └──────────────┘
```

**Examples:**
- **Aider** → LiteLLM (supports virtually any model)
- **OpenHands** → LiteLLM (100+ providers)
- **mini-SWE-agent** → LiteLLM (any model via `litellm.completion()`)

**Pros:** Instant multi-provider support, unified error handling, cost tracking built-in.

**Cons:** Abstraction lag (new provider features take time to appear), potential overhead.

### 3. Native Multi-Provider SDKs

The agent implements direct integrations with multiple provider SDKs, maintaining
per-provider clients.

```
┌─────────────┐     ┌──────────────┐
│  OpenCode   │────▶│ Anthropic SDK│
│             │────▶│ OpenAI SDK   │
│             │────▶│ Gemini SDK   │
│             │────▶│ AWS Bedrock  │
│             │────▶│ OpenRouter   │
└─────────────┘     └──────────────┘
```

**Examples:**
- **OpenCode** → 10+ native provider implementations in Go
- **ForgeCode** → 7+ providers with per-task routing
- **Pi Coding Agent** → 15+ providers via custom `pi-ai` abstraction

**Pros:** Full feature access per provider, no abstraction overhead.

**Cons:** More code to maintain, each new provider needs a new integration.

### 4. Backend-Managed Routing

A backend service (not the CLI itself) decides which model to use.

```
┌─────────────┐     ┌──────────────┐     ┌──────────────┐
│  Junie CLI  │────▶│  JetBrains   │────▶│ Claude       │
│             │     │  Backend     │────▶│ GPT          │
│             │     │              │────▶│ Gemini       │
└─────────────┘     └──────────────┘     └──────────────┘
```

**Examples:**
- **Junie CLI** → JetBrains backend routes between Claude, GPT, Gemini
- **Warp** → Proprietary routing with Auto Modes (Cost-efficient, Responsive, Genius)

**Pros:** Centralized optimization, users don't need API keys, seamless updates.

**Cons:** Dependency on backend service, less user control, potential privacy concerns.

---

## Cross-Reference: Which Agents Use Which Providers

The table below maps all 17 studied agents to their supported providers. See
[agent-comparison.md](agent-comparison.md) for a full analysis.

| Agent | OpenAI | Anthropic | Google | DeepSeek | Local/OSS | LiteLLM | OpenRouter | Multi-Model |
|-------|--------|-----------|--------|----------|-----------|---------|------------|-------------|
| **Aider** | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ Core | ✅ | ✅ Architect |
| **Ante** | ❌ | ✅ | ✅ | ❌ | ✅ | ❌ | ❌ | ✅ Multi-agent |
| **Capy** | ✅ | ✅ | ✅ | ❌ | ❌ | ❌ | ❌ | ✅ |
| **Claude Code** | ❌ | ✅ | ❌ | ❌ | ❌ | ❌ | ❌ | ⚠️ Tier switch |
| **Codex CLI** | ✅ | ❌ | ❌ | ❌ | ✅ | ❌ | ❌ | ⚠️ Limited |
| **Droid** | ✅ | ✅ | ✅ | ❌ | ✅ | ❌ | ❌ | ✅ Routing |
| **ForgeCode** | ✅ | ✅ | ✅ | ✅ | ❌ | ❌ | ✅ | ✅ Per-phase |
| **Gemini CLI** | ❌ | ❌ | ✅ | ❌ | ❌ | ❌ | ❌ | ❌ |
| **Goose** | ✅ | ✅ | ✅ | ❌ | ✅ | ✅ | ✅ | ✅ Fast model |
| **Junie CLI** | ✅ | ✅ | ✅ | ❌ | ❌ | ❌ | ❌ | ✅ Per-task |
| **mini-SWE-agent** | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ Core | ✅ | ❌ |
| **OpenCode** | ✅ | ✅ | ✅ | ❌ | ❌ | ❌ | ✅ | ✅ Sub-agent |
| **OpenHands** | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ Core | ✅ | ⚠️ Limited |
| **Pi Coding Agent** | ✅ | ✅ | ✅ | ❌ | ❌ | ❌ | ✅ | ✅ Handoff |
| **SageAgent** | ✅ | ❌ | ❌ | ❌ | ❌ | ❌ | ❌ | ⚠️ Potential |
| **TongAgents** | ❌ | ✅ | ✅ | ❌ | ❌ | ❓ | ❓ | ❓ |
| **Warp** | ✅ | ✅ | ✅ | ❌ | ❌ | ❌ | ❌ | ✅ Auto modes |

**Legend:** ✅ = Supported | ❌ = Not supported | ⚠️ = Limited | ❓ = Unknown

### Provider Popularity Among Agents

```
Anthropic:  ████████████████  14/17 agents (82%)
OpenAI:     █████████████░░░  12/17 agents (71%)
Google:     ████████████░░░░  12/17 agents (71%)
OpenRouter: ███████░░░░░░░░░   7/17 agents (41%)
LiteLLM:    █████░░░░░░░░░░░   5/17 agents (29%)
Local/OSS:  █████░░░░░░░░░░░   5/17 agents (29%)
DeepSeek:   ███░░░░░░░░░░░░░   3/17 agents (18%)
```

---

## Key Concepts

### Context Windows

Context window size determines how much code an agent can process in a single turn:

| Model | Context Window | Effective for Coding |
|-------|---------------|---------------------|
| Gemini 2.5 Pro | 1,048,576 tokens | Entire large codebases |
| Claude Opus 4.6 | 1,000,000 tokens | Entire large codebases |
| Claude Sonnet 4.6 | 1,000,000 tokens | Entire large codebases |
| GPT-4.1 | 1,048,576 tokens | Entire large codebases |
| GPT-4o | 128,000 tokens | Large files + context |
| DeepSeek-V3.2 | 128,000 tokens | Large files + context |
| Llama 3 (70B) | 128,000 tokens | Large files + context |

### Function Calling / Tool Use

Native function calling is critical for coding agents. It determines how reliably the
model can invoke tools like file editors, terminal commands, and search:

| Provider | Mechanism | Reliability |
|----------|-----------|-------------|
| OpenAI | `tools` parameter in Chat/Responses API | Excellent — most mature |
| Anthropic | `tools` parameter in Messages API | Excellent — structured output |
| Google | `function_declarations` in Gemini API | Very good |
| DeepSeek | OpenAI-compatible `tools` parameter | Good |
| Local models | Varies; often prompt-based parsing | Variable |

### Prompt Caching

Prompt caching dramatically reduces costs for agents that send the same system prompt
and codebase context on every turn:

| Provider | Caching Mechanism | Savings |
|----------|------------------|---------|
| Anthropic | Explicit cache breakpoints, 5min/1hr TTL | 90% on cached input |
| OpenAI | Automatic (50%+ of prompt) | 50% on cached input |
| Google | Context caching API | Varies by model |
| DeepSeek | Automatic disk caching | 90% on cache hits |

---

## Document Index

| Document | Description |
|----------|-------------|
| [openai.md](openai.md) | OpenAI models, APIs, pricing, and agent integrations |
| [anthropic.md](anthropic.md) | Anthropic Claude models, Messages API, extended thinking |
| [google.md](google.md) | Google Gemini models, AI Studio, Vertex AI |
| [deepseek.md](deepseek.md) | DeepSeek models, MoE architecture, cost efficiency |
| [open-source-models.md](open-source-models.md) | Open-source models, Ollama, local deployment |
| [litellm.md](litellm.md) | LiteLLM unified provider interface |
| [model-routing.md](model-routing.md) | Model routing and selection strategies |
| [pricing-and-cost.md](pricing-and-cost.md) | Pricing comparison and cost optimization |
| [api-patterns.md](api-patterns.md) | Common API integration patterns |
| [agent-comparison.md](agent-comparison.md) | Cross-agent provider support comparison |

---

## Architecture Decision Framework

When building or choosing a coding agent, use this framework to evaluate provider
integration strategy:

### Decision Tree

```
Do you need to support multiple providers?
├── No → Single-provider lock-in (Claude Code, Gemini CLI pattern)
│   └── Optimize for that provider's unique features
│       (extended thinking, prompt caching, etc.)
└── Yes
    ├── Do you need deep per-provider feature access?
    │   ├── Yes → Native multi-provider SDKs (OpenCode pattern)
    │   │   └── More code, but full feature access
    │   └── No → Unified SDK abstraction (Aider/LiteLLM pattern)
    │       └── Less code, consistent interface, slight feature lag
    └── Do users need to manage their own API keys?
        ├── Yes → Client-side SDK integration
        └── No → Backend-managed routing (Junie/Warp pattern)
```

### Trade-off Matrix

| Strategy | Dev Effort | Feature Access | Flexibility | Maintenance |
|----------|-----------|---------------|-------------|-------------|
| Single provider | Low | Full | None | Low |
| LiteLLM abstraction | Low | Partial | High | Low |
| Native multi-SDK | High | Full | High | High |
| Backend routing | Medium | Varies | Medium | Medium |

---

## Emerging Trends

### 1. Multi-Model Orchestration

The most sophisticated agents (Aider, ForgeCode, Junie, Warp) use different models for
different phases of a task:

- **Planning/Reasoning:** High-intelligence model (Claude Opus, o3, Gemini 2.5 Pro)
- **Code Editing:** Fast, cost-effective model (Claude Sonnet, GPT-4o, Gemini Flash)
- **Sub-tasks:** Cheap model for routine operations (Haiku, GPT-4o-mini)

### 2. Provider Commoditization

As models converge in capability, the provider becomes less important than the
integration layer. LiteLLM and OpenRouter are driving this commoditization by making
it trivial to swap providers.

### 3. Local-First Privacy

Enterprise users increasingly demand that code never leaves their network. Agents
that support Ollama, vLLM, and other local inference engines are gaining adoption.

### 4. Cost-Performance Optimization

With DeepSeek offering 10-50x cost savings over frontier models, agents are
beginning to implement sophisticated cost-based routing that uses cheap models for
simple tasks and expensive models only when needed.

### 5. Reasoning Model Integration

OpenAI's o-series and DeepSeek-R1 introduced explicit reasoning tokens. Agents are
learning to leverage these for planning while using standard models for execution.

---

## Provider Selection Guide

Choosing a provider for your coding agent involves balancing multiple factors. Use
this decision matrix to narrow down your options:

### Quick Decision Matrix

| Priority | Best Provider | Runner-Up |
|----------|-------------|-----------|
| **Maximum quality** | Anthropic (Opus 4.6) | OpenAI (o3) |
| **Best value** | Anthropic (Sonnet 4.6) | OpenAI (GPT-4.1) |
| **Lowest cost** | DeepSeek-V3.2 | Google (Gemini Flash) |
| **Free usage** | Google (AI Studio free tier) | Ollama (local) |
| **Privacy** | Local (Ollama/vLLM) | Self-hosted DeepSeek |
| **Longest context** | Google (Gemini 2.5 Pro, 1M) | Anthropic (1M) |
| **Best function calling** | OpenAI (strict mode) | Anthropic |
| **Best ecosystem** | OpenAI (widest SDK support) | Anthropic |
| **Best caching** | Anthropic (90% savings) | DeepSeek (90% auto) |
| **Multimodal** | Google (video/audio/images) | OpenAI (images) |
| **Enterprise** | Any (via Bedrock/Vertex/Azure) | Anthropic (data residency) |

### Decision by Team Size

| Team Size | Recommended Approach |
|-----------|---------------------|
| **Solo developer** | Single provider (Anthropic or OpenAI) + prompt caching |
| **Small team (2-5)** | LiteLLM proxy with 2-3 providers + virtual keys |
| **Medium team (5-20)** | LiteLLM proxy + budget management + model tiering |
| **Large org (20+)** | Enterprise platform (Bedrock/Vertex) + governance |

### Decision by Budget

| Monthly Budget | Strategy |
|---------------|----------|
| **$0** | Gemini CLI (free tier) or Ollama (local) |
| **$0-50** | DeepSeek for most tasks, Gemini Flash for backup |
| **$50-200** | Claude Sonnet with caching, DeepSeek for simple tasks |
| **$200-1000** | Claude Sonnet/Opus with smart routing |
| **$1000+** | Full multi-provider with per-phase routing |

---

## Glossary

| Term | Definition |
|------|-----------|
| **Provider** | Company/service that hosts and serves LLM models (OpenAI, Anthropic, etc.) |
| **Model** | Specific LLM version (e.g., claude-sonnet-4-6, gpt-4.1) |
| **Meta-provider** | Service that aggregates multiple providers (LiteLLM, OpenRouter) |
| **Endpoint** | URL where the API accepts requests |
| **Context window** | Maximum tokens the model can process in a single request |
| **Prompt caching** | Reusing previously computed prompt prefixes to save cost |
| **Extended thinking** | Chain-of-thought reasoning tokens generated before the answer |
| **Function calling** | Model's ability to emit structured tool invocations |
| **MoE** | Mixture of Experts — architecture where only a subset of parameters activate per token |
| **Token** | Smallest unit of text the model processes (~4 chars for English) |
| **MTok** | Million tokens — standard unit for LLM pricing |
| **RPM** | Requests per minute — rate limit dimension |
| **TPM** | Tokens per minute — rate limit dimension |
| **SSE** | Server-Sent Events — protocol used for streaming responses |
| **FIM** | Fill-in-the-middle — code completion where the model fills a gap |

---

## Related Sections

- [LLM APIs and Protocols](../llm-apis-and-protocols/) — Low-level API details
- [Agentic Loop](../agentic-loop/) — How agents orchestrate model calls
- [Context Management](../context-management/) — How agents manage token budgets
- [Prompt Engineering](../prompt-engineering/) — System prompt strategies per provider
- [Streaming](../streaming/) — Streaming implementation patterns
- [Tool Systems](../tool-systems/) — How tools integrate with function calling
- [Prompt Engineering](../prompt-engineering/) — System prompt strategies per provider