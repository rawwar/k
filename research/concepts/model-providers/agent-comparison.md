# Cross-Agent Comparison of Model Provider Support

## Overview

This document provides a comprehensive comparison of how all 17 CLI coding agents
studied in this research library integrate with model providers. The analysis covers
which providers each agent supports, default model configurations, multi-model
capabilities, and the architectural approaches used for provider integration.

---

## Master Comparison Table

### Provider Support Matrix

| Agent | OpenAI | Anthropic | Google | DeepSeek | xAI | Local/OSS | LiteLLM | OpenRouter | Other |
|-------|--------|-----------|--------|----------|-----|-----------|---------|------------|-------|
| **Aider** | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ Core | ✅ | Groq, Together, Fireworks |
| **Ante** | ❌ | ✅ | ✅ | ❌ | ❌ | ✅ | ❌ | ❌ | nanochat-rs (custom) |
| **Capy** | ✅ | ✅ | ✅ | ❌ | ✅ | ❌ | ❌ | ❌ | Kimi, GLM, Qwen API |
| **Claude Code** | ❌ | ✅ | ❌ | ❌ | ❌ | ❌ | ❌ | ❌ | AWS Bedrock, Vertex AI |
| **Codex CLI** | ✅ | ❌ | ❌ | ❌ | ❌ | ✅ | ❌ | ❌ | Custom endpoints (TOML) |
| **Droid** | ✅ | ✅ | ✅ | ❌ | ✅ | ✅ | ❌ | ❌ | Proprietary router |
| **ForgeCode** | ✅ | ✅ | ✅ | ✅ | ❌ | ❌ | ❌ | ✅ | Mistral |
| **Gemini CLI** | ❌ | ❌ | ✅ | ❌ | ❌ | ❌ | ❌ | ❌ | — |
| **Goose** | ✅ | ✅ | ✅ | ❌ | ❌ | ✅ | ✅ | ✅ | Bedrock, Databricks, Docker |
| **Junie CLI** | ✅ | ✅ | ✅ | ❌ | ❌ | ❌ | ❌ | ❌ | JetBrains backend |
| **mini-SWE-agent** | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ Core | ✅ | Any LiteLLM provider |
| **OpenCode** | ✅ | ✅ | ✅ | ❌ | ✅ | ❌ | ❌ | ✅ | Bedrock, Groq, Azure, Copilot |
| **OpenHands** | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ Core | ✅ | Any LiteLLM provider |
| **Pi Coding Agent** | ✅ | ✅ | ✅ | ❌ | ✅ | ❌ | ❌ | ✅ | Groq, Cerebras, HF, Kimi |
| **SageAgent** | ✅ | ❌ | ❌ | ❌ | ❌ | ❌ | ❌ | ❌ | — |
| **TongAgents** | ❌ | ✅ | ✅ | ❌ | ❌ | ❌ | ❓ | ❓ | Unknown internals |
| **Warp** | ✅ | ✅ | ✅ | ❌ | ❌ | ❌ | ❌ | ❌ | Proprietary routing |

**Legend:** ✅ = Supported | ❌ = Not supported | ❓ = Unknown

### Provider Adoption Statistics

| Provider | Agents Supporting | Adoption Rate |
|----------|------------------|---------------|
| Anthropic | 14 | 82% |
| OpenAI | 12 | 71% |
| Google | 12 | 71% |
| OpenRouter | 7 | 41% |
| xAI | 5 | 29% |
| Local/OSS | 5 | 29% |
| LiteLLM | 5 | 29% |
| DeepSeek | 3 | 18% |

---

## Default Model Configuration

| Agent | Default Model | Can Change? | Configuration Method |
|-------|--------------|------------|---------------------|
| **Aider** | User-selected (no default) | ✅ | `--model` flag, `.aider.conf.yml` |
| **Ante** | Not documented | ✅ | Configuration file |
| **Capy** | User-selected | ✅ | UI selector |
| **Claude Code** | Claude Sonnet 4.6 | ✅ | `/model` command, `--model` flag |
| **Codex CLI** | GPT-4.1 | ✅ | `--model` flag, `config.toml` |
| **Droid** | Team-configurable | ✅ | Team/project configuration |
| **ForgeCode** | `claude-sonnet-4` | ✅ | `forge.yaml` configuration |
| **Gemini CLI** | Gemini 3 Flash | ✅ | `--model` flag |
| **Goose** | User-configured via env | ✅ | `GOOSE_MODEL` env var |
| **Junie CLI** | Dynamic (backend) | ⚠️ Limited | JetBrains backend decides |
| **mini-SWE-agent** | User-specified | ✅ | Command-line argument |
| **OpenCode** | User-configured | ✅ | TUI dialog, config file |
| **OpenHands** | User-configured | ✅ | `LLM_MODEL` env var |
| **Pi Coding Agent** | User-configured | ✅ | Configuration file |
| **SageAgent** | GPT-5.3-Codex (tested) | ❓ | Not documented |
| **TongAgents** | Unknown | ❓ | Not documented |
| **Warp** | Auto-selected (modes) | ✅ | Auto modes or manual selection |

---

## Multi-Model Capabilities

### Multi-Model Architecture Comparison

| Agent | Multi-Model Type | Description |
|-------|-----------------|-------------|
| **Aider** | Architect Mode | Reasoning model plans → editing model codes |
| **Ante** | Multi-Agent | Different sub-agents can use different models |
| **Capy** | Model-Agnostic | Supports 7+ models, user-selectable |
| **Claude Code** | Tier Switching | Main agent (Sonnet/Opus) + Explore sub-agent (Haiku) |
| **Codex CLI** | Single Model | One model per session, custom endpoints |
| **Droid** | Model Router | Cost-efficient models for execution, frontier for planning |
| **ForgeCode** | Per-Phase Routing | Different models for thinking/coding/review/large-context |
| **Gemini CLI** | Single Model | One Gemini model at a time |
| **Goose** | Fast Model Config | Main model + separate fast model for quick ops |
| **Junie CLI** | Per-Task Routing | Backend routes between Claude/GPT/Gemini per task |
| **mini-SWE-agent** | Single Model | Deliberately minimal, one model per run |
| **OpenCode** | Sub-Agent | Delegates to child agents, supports model switching |
| **OpenHands** | Single + Condenser | Primary model + optional different condenser model |
| **Pi Coding Agent** | Cross-Provider Handoff | Switch providers mid-conversation with context preservation |
| **SageAgent** | Pipeline Agents | Each pipeline agent can potentially have different model |
| **TongAgents** | Multi-Agent | Multiple agents, likely different models |
| **Warp** | Auto Modes | Intelligent routing across 3 cost/quality tiers |

### Multi-Model Sophistication Ranking

```
Most Sophisticated                          Single Model
──────────────────────────────────────────────────────
ForgeCode  Junie  Warp  Aider  Goose  Claude  Codex  Gemini
  ████████  █████  █████  ████   ███    ██      █      █
Per-phase  Per-   Auto   Arch-  Fast   Tier    One    One
routing    task   modes  itect  model  switch  model  model
```

---

## Integration Architecture Comparison

### Architecture Categories

| Category | Agents | Approach |
|----------|--------|----------|
| **LiteLLM Abstraction** | Aider, OpenHands, mini-SWE-agent | All LLM calls through `litellm.completion()` |
| **Single Provider SDK** | Claude Code, Gemini CLI, Codex CLI, SageAgent | Direct SDK for one provider |
| **Native Multi-Provider** | OpenCode, ForgeCode, Pi Coding Agent, Goose | Custom integrations per provider |
| **Backend-Managed** | Junie CLI, Warp, Droid | Server-side model routing |
| **Custom Abstraction** | Ante, Capy, TongAgents | Custom provider abstraction layer |

### Architecture Characteristics

| Architecture | Dev Effort | Provider Coverage | Feature Depth | Maintenance |
|-------------|-----------|------------------|--------------|-------------|
| LiteLLM Abstraction | Low | Very High (100+) | Medium | Low |
| Single Provider SDK | Low | One provider | Very High | Very Low |
| Native Multi-Provider | High | Medium (5-15) | High | High |
| Backend-Managed | Medium | Variable | Variable | Medium |
| Custom Abstraction | Medium | Low (2-5) | Variable | Medium |

---

## Detailed Agent Profiles

### Aider

```
Provider Integration: LiteLLM (core dependency)
─────────────────────────────────────────────
Supported Providers: Virtually all (via LiteLLM)
Default Model: None (user must specify)
Multi-Model: ✅ Architect mode (planner + editor)
Key Feature: Top 20 OpenRouter app
API Format: OpenAI (via LiteLLM translation)
Streaming: ✅
Function Calling: ✅
Special: --model and --editor-model flags
```

### Claude Code

```
Provider Integration: Direct Anthropic SDK
─────────────────────────────────────────────
Supported Providers: Anthropic only (+ Bedrock, Vertex)
Default Model: Claude Sonnet 4.6
Multi-Model: ⚠️ Tier switching (Sonnet/Opus/Haiku)
Key Feature: Deepest Anthropic integration
API Format: Anthropic Messages API
Streaming: ✅
Function Calling: ✅ (native tool use)
Special: Extended thinking, prompt caching, Explore sub-agent (Haiku)
```

### Codex CLI

```
Provider Integration: Direct OpenAI SDK
─────────────────────────────────────────────
Supported Providers: OpenAI + custom OpenAI-compatible endpoints
Default Model: GPT-4.1
Multi-Model: ⚠️ Single model per session
Key Feature: Uses Responses API (not Chat Completions)
API Format: OpenAI Responses API
Streaming: ✅
Function Calling: ✅
Special: Custom providers via TOML config, Ollama support
```

### Gemini CLI

```
Provider Integration: Direct Google Gemini SDK
─────────────────────────────────────────────
Supported Providers: Google only
Default Model: Gemini 3 Flash
Multi-Model: ❌ Single model
Key Feature: Google Search grounding
API Format: Gemini API (AI Studio)
Streaming: ✅
Function Calling: ✅
Special: Free tier, multimodal input, code execution
```

### OpenHands

```
Provider Integration: LiteLLM (core dependency)
─────────────────────────────────────────────
Supported Providers: All LiteLLM providers (100+)
Default Model: User-configured
Multi-Model: ⚠️ Single + condenser model
Key Feature: Docker-sandboxed execution
API Format: OpenAI (via LiteLLM)
Streaming: ✅
Function Calling: ✅
Special: Condenser strategies can use different models
```

### Goose

```
Provider Integration: Native + LiteLLM + OpenRouter
─────────────────────────────────────────────
Supported Providers: 30+ (most of any agent)
Default Model: User-configured (GOOSE_MODEL)
Multi-Model: ✅ Fast model config
Key Feature: Broadest provider support
API Format: Mixed (native per provider)
Streaming: ✅
Function Calling: ✅ (+ toolshim for non-tool models)
Special: Docker Model Runner, Ramalama, ACP providers
```

### ForgeCode

```
Provider Integration: Custom multi-provider layer
─────────────────────────────────────────────
Supported Providers: 7+ including OpenRouter
Default Model: claude-sonnet-4
Multi-Model: ✅ Per-phase routing
Key Feature: Context-preserving mid-session model switching
API Format: Mixed (per provider)
Streaming: ✅
Function Calling: ✅
Special: Thinking/coding/review models, large-context routing
```

### OpenCode

```
Provider Integration: Native Go SDK per provider
─────────────────────────────────────────────
Supported Providers: 10+ (native implementations)
Default Model: User-configured
Multi-Model: ✅ Sub-agent delegation
Key Feature: Go-native, no Python dependency
API Format: Native per provider
Streaming: ✅
Function Calling: ✅
Special: TUI model switcher, GitHub Copilot provider
```

---

## Feature Support Matrix

### Advanced Provider Features

| Agent | Prompt Caching | Extended Thinking | Vision | Batch API | Context >200K |
|-------|---------------|-------------------|--------|-----------|---------------|
| **Aider** | ✅ (via LiteLLM) | ❌ | ✅ | ❌ | ✅ |
| **Claude Code** | ✅ (native) | ✅ (native) | ✅ | ❌ | ✅ (1M) |
| **Codex CLI** | ✅ (auto) | ❌ | ✅ | ❌ | ✅ (1M) |
| **Gemini CLI** | ✅ (context cache) | ✅ (thinking) | ✅ | ❌ | ✅ (1M) |
| **OpenHands** | ✅ (via LiteLLM) | ❌ | ❌ | ❌ | ✅ |
| **Goose** | ⚠️ (varies) | ❌ | ❌ | ❌ | ✅ |
| **ForgeCode** | ⚠️ (varies) | ❌ | ❌ | ❌ | ✅ |
| **Junie CLI** | Unknown | Unknown | ❌ | ❌ | ✅ |
| **Warp** | Unknown | Unknown | ✅ | ❌ | ✅ |

### Tool Calling Quality by Provider

| Provider | Reliability | Parallel Calls | Streaming Tools | Strict Mode |
|----------|-------------|---------------|----------------|-------------|
| OpenAI | ★★★★★ | ✅ | ✅ | ✅ |
| Anthropic | ★★★★★ | ✅ | ✅ | ❌ |
| Google | ★★★★☆ | ✅ | ✅ | ❌ |
| DeepSeek | ★★★☆☆ | ✅ | ⚠️ | ❌ |
| Ollama (local) | ★★☆☆☆ | ⚠️ | ⚠️ | ❌ |

---

## Cost Profiles

### Estimated Cost per Session (20 turns, moderate complexity)

| Agent | Typical Provider | Estimated Cost | Notes |
|-------|-----------------|---------------|-------|
| **Claude Code** (Sonnet) | Anthropic | $1.50-3.00 | With prompt caching |
| **Claude Code** (Opus) | Anthropic | $5.00-10.00 | With prompt caching |
| **Codex CLI** | OpenAI | $0.50-2.00 | GPT-4.1 with auto-cache |
| **Aider** (Sonnet) | Anthropic | $1.50-3.00 | Via LiteLLM |
| **Aider** (DeepSeek) | DeepSeek | $0.02-0.10 | Cheapest cloud option |
| **Gemini CLI** | Google | $0.10-0.50 | Flash model (cheap) |
| **Goose** (Sonnet) | Anthropic | $1.50-3.00 | Similar to Claude Code |
| **OpenHands** | Varies | $0.50-5.00 | Depends on model choice |
| **mini-SWE-agent** | Varies | $0.10-2.00 | Minimal token usage |
| **Warp** (Auto) | Mixed | $0.50-5.00 | Depends on auto mode |

---

## Recommendations

### By Use Case

| Use Case | Recommended Agent | Why |
|----------|------------------|-----|
| **Best overall experience** | Claude Code | Deepest provider integration, best defaults |
| **Maximum provider flexibility** | Goose or Aider | 30+ or 100+ providers respectively |
| **Cheapest operation** | Aider + DeepSeek | LiteLLM flexibility + cheapest provider |
| **Enterprise (multi-team)** | Droid or Junie CLI | Backend-managed routing and governance |
| **Privacy-first** | Codex CLI + Ollama | Easy local model configuration |
| **Cost-optimized quality** | ForgeCode or Warp | Per-phase routing / auto modes |
| **Maximum SWE-bench score** | Claude Code (Opus) or OpenHands | Best on benchmarks |
| **Free usage** | Gemini CLI | Gemini free tier |

### By Provider Preference

| If You Prefer... | Best Agents |
|------------------|-------------|
| **Anthropic Claude** | Claude Code, Aider, ForgeCode |
| **OpenAI GPT** | Codex CLI, Aider, OpenCode |
| **Google Gemini** | Gemini CLI, Aider, ForgeCode |
| **DeepSeek** | Aider, OpenHands, mini-SWE-agent |
| **Local models** | Aider (via LiteLLM/Ollama), Goose, Codex CLI |
| **Multiple providers** | Aider, Goose, OpenCode |

---

## Trends Observed

### 1. Anthropic Dominance

With 82% agent support, Anthropic/Claude is the most popular provider. This reflects:
- Claude Code's influence as the reference implementation
- Claude's strong coding benchmark performance
- Prompt caching enabling cost-effective agentic loops
- Extended thinking for complex reasoning tasks

### 2. LiteLLM as Standard Abstraction

The 5 agents using LiteLLM collectively support 100+ providers through a single
integration. This pattern is likely to grow as it dramatically reduces development
effort for multi-provider support.

### 3. Multi-Model is the Future

13 out of 17 agents (76%) support some form of multi-model configuration. The trend
is moving from simple model switching to sophisticated per-phase routing.

### 4. Local Model Support Growing

5 agents explicitly support local models, and many more can through custom endpoints.
As open-source models improve, local-first agents will become more viable.

### 5. Convergence on OpenAI API Format

Even non-OpenAI providers (Google, DeepSeek) offer OpenAI-compatible endpoints,
and LiteLLM translates everything to OpenAI format. The OpenAI Chat Completions
API has become the de facto standard interface.

---

## See Also

- [README](README.md) — Overview of model providers in coding agents
- [OpenAI](openai.md) — OpenAI as a model provider
- [Anthropic](anthropic.md) — Anthropic as a model provider
- [Google](google.md) — Google as a model provider
- [Model Routing](model-routing.md) — Routing strategies across agents
- [Pricing and Cost](pricing-and-cost.md) — Cost comparison across providers

---

## Methodology

### Data Collection

The data in this comparison was collected from:

1. **Agent source code** — Examining provider integrations, configuration files,
   and model selection logic in each agent's repository
2. **Agent documentation** — README files, architecture documents, and user guides
3. **Public benchmarks** — SWE-bench Verified scores, HumanEval results
4. **Community reports** — GitHub issues, Discord discussions, blog posts

### Limitations

- **Rapidly evolving** — Agent capabilities change frequently; this comparison
  reflects the state as of mid-2025
- **Closed-source agents** — Some agents (Droid, Warp, Capy, TongAgents) have
  limited public documentation, so data may be incomplete
- **Cost estimates** — Based on typical usage patterns; actual costs vary widely
  depending on task complexity and conversation length
- **Benchmark scores** — SWE-bench scores depend on the specific evaluation setup
  and may not reflect real-world performance

### Update Frequency

This comparison should be updated when:
- A new agent is added to the research library
- An existing agent adds support for a new provider
- Provider pricing changes significantly
- New provider features (e.g., new caching mechanisms) are released

---

## Appendix: API Endpoint Reference

Quick reference for configuring providers across agents:

| Provider | Base URL | Auth Method | Model Format |
|----------|---------|-------------|-------------|
| OpenAI | `https://api.openai.com/v1` | Bearer token (`OPENAI_API_KEY`) | `gpt-4.1` |
| Anthropic | `https://api.anthropic.com/v1` | `x-api-key` header (`ANTHROPIC_API_KEY`) | `claude-sonnet-4-6` |
| Google AI Studio | `https://generativelanguage.googleapis.com/v1beta` | API key parameter | `gemini-2.5-pro` |
| DeepSeek | `https://api.deepseek.com` | Bearer token (`DEEPSEEK_API_KEY`) | `deepseek-chat` |
| OpenRouter | `https://openrouter.ai/api/v1` | Bearer token (`OPENROUTER_API_KEY`) | `anthropic/claude-sonnet-4-6` |
| Ollama | `http://localhost:11434/v1` | None needed | `qwen2.5-coder:32b` |
| LiteLLM Proxy | `http://localhost:4000` | Bearer token (master key) | Config-defined names |
| AWS Bedrock | Region-specific | AWS IAM | `anthropic.claude-sonnet-4-6` |
| Google Vertex AI | Region-specific | Google Cloud IAM | `claude-sonnet-4-6` |
| Azure OpenAI | Deployment-specific | Azure AD / API key | Deployment name |

### Environment Variable Reference

```bash
# OpenAI
export OPENAI_API_KEY="sk-..."
export OPENAI_BASE_URL="https://api.openai.com/v1"  # Optional override

# Anthropic
export ANTHROPIC_API_KEY="sk-ant-..."

# Google
export GOOGLE_API_KEY="..."        # AI Studio
export GOOGLE_APPLICATION_CREDENTIALS="path/to/sa.json"  # Vertex AI

# DeepSeek
export DEEPSEEK_API_KEY="..."

# OpenRouter
export OPENROUTER_API_KEY="sk-or-..."

# LiteLLM
export LITELLM_MASTER_KEY="sk-litellm-..."

# Agent-specific
export GOOSE_MODEL="claude-sonnet-4-6"
export GOOSE_PROVIDER="anthropic"
export LLM_MODEL="anthropic/claude-sonnet-4-6"  # OpenHands
```
- [Pricing and Cost](pricing-and-cost.md) — Cost comparison across providers