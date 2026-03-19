# LLM APIs and Protocols for Coding Agents

## Introduction

Large Language Model APIs are the foundational layer upon which all modern coding agents are built. Whether it's GitHub Copilot generating inline suggestions, Cursor composing multi-file edits, or an autonomous SWE-agent resolving GitHub issues end-to-end, every one of these systems ultimately communicates with an LLM through an HTTP API.

Understanding these APIs deeply matters for several reasons:

1. **Architecture decisions** — The capabilities and constraints of each API directly shape what a coding agent can and cannot do. Tool-calling support, context window size, streaming behavior, and structured output features all influence agent design.
2. **Cost optimization** — LLM API calls are the primary cost center for coding agents. Understanding token economics, prompt caching, and batch APIs enables building agents that are economically viable.
3. **Reliability engineering** — Rate limits, timeout behaviors, error codes, and retry strategies differ across providers. Production coding agents must handle these gracefully.
4. **Performance** — Streaming vs. non-streaming, parallel tool calls, and speculative decoding all affect the perceived latency of an agent. Understanding the wire protocol enables optimization.
5. **Provider flexibility** — The best coding agents are not locked to a single provider. Understanding the common patterns and key differences across APIs enables multi-provider architectures.

This folder contains deep-dive research into every major LLM API relevant to building coding agents.

---

## Table of Contents

| File | Topic | Description |
|------|-------|-------------|
| [chat-completions.md](./chat-completions.md) | OpenAI Chat Completions API | The foundational API that established the chat paradigm. Full request/response schema, streaming, tool calling, structured outputs. |
| [responses-api.md](./responses-api.md) | OpenAI Responses API | OpenAI's next-generation stateful API with built-in tools (web search, file search, code interpreter), multi-turn state management. |
| [messages-api.md](./messages-api.md) | Anthropic Messages API | Anthropic's Claude API — content blocks, extended thinking, tool use, prompt caching, and the differences from OpenAI's approach. |
| [function-calling.md](./function-calling.md) | Function/Tool Calling Deep-Dive | Cross-provider comparison of tool calling: JSON schema definitions, parallel calls, forced tool use, and agent loop patterns. |
| [extended-thinking.md](./extended-thinking.md) | Extended Thinking & Reasoning | How reasoning models (o3, o4-mini, Claude with extended thinking) expose chain-of-thought via APIs, budget tokens, and streaming thinking. |
| [embeddings.md](./embeddings.md) | Embeddings APIs | Text embedding endpoints for RAG-based coding agents — OpenAI, Voyage, Cohere embeddings for code search and retrieval. |
| [batch-api.md](./batch-api.md) | Batch & Async APIs | Batch processing endpoints for offline workloads — OpenAI Batch API, Anthropic Message Batches, cost savings and use cases. |
| [rate-limits-and-retries.md](./rate-limits-and-retries.md) | Rate Limits & Retry Strategies | Rate limit headers, exponential backoff, retry-after, circuit breakers, and production resilience patterns. |
| [agent-comparison.md](./agent-comparison.md) | Coding Agent API Usage Comparison | How major coding agents (Copilot, Cursor, Cline, Aider, SWE-agent) use these APIs in practice. |

---

## The Landscape of LLM API Providers

### Tier 1: Frontier Model Providers

These companies train and serve their own frontier models behind proprietary APIs:

**OpenAI**
- APIs: Chat Completions (`/v1/chat/completions`), Responses (`/v1/responses`), Embeddings, Audio, Images
- Models: GPT-4o, GPT-4o-mini, GPT-4-turbo, o3, o4-mini, o3-mini
- Key differentiator: Largest ecosystem, most third-party integrations, industry-standard API format
- SDKs: `openai` (Python), `openai` (Node.js/TypeScript)

**Anthropic**
- APIs: Messages (`/v1/messages`), Message Batches
- Models: Claude Sonnet 4, Claude Opus 4, Claude Haiku 3.5, Claude 3 Opus
- Key differentiator: Extended thinking for complex reasoning, prompt caching, superior long-context performance
- SDKs: `anthropic` (Python), `@anthropic-ai/sdk` (TypeScript)

**Google DeepMind**
- APIs: Gemini API (`/v1/models/{model}:generateContent`), Vertex AI
- Models: Gemini 2.5 Pro, Gemini 2.5 Flash, Gemini 2.0 Flash
- Key differentiator: Massive context windows (1M+ tokens), native multimodal, competitive pricing
- SDKs: `google-genai` (Python), `@google/genai` (TypeScript)

### Tier 2: Inference Providers (Open-Source Model Hosts)

These providers serve open-weight models, often with OpenAI-compatible APIs:

- **Together AI** — Llama, Mixtral, Qwen, DeepSeek with OpenAI-compatible API
- **Fireworks AI** — Optimized inference for open models, function calling support
- **Groq** — Ultra-low-latency inference using custom LPU hardware
- **AWS Bedrock** — Managed access to Claude, Llama, Mistral, and other models
- **Azure OpenAI** — Microsoft-hosted OpenAI models with enterprise compliance
- **DeepSeek** — DeepSeek-V3, DeepSeek-R1, competitive pricing, OpenAI-compatible

### Tier 3: Local/Self-Hosted

- **Ollama** — Local model serving with OpenAI-compatible API
- **vLLM** — High-throughput serving engine, OpenAI-compatible server
- **llama.cpp** — CPU/GPU inference with HTTP server mode
- **LM Studio** — Desktop app with local API server

---

## Key Concepts

### The Chat Completions Paradigm

The modern LLM API interaction model was established by OpenAI's Chat Completions API in March 2023 and has since become the de facto standard. The core abstraction is a **conversation as a sequence of messages**, where each message has a **role** and **content**:

```
system:    "You are a helpful coding assistant..."
user:      "Write a function that sorts a list"
assistant: "Here's a Python function that sorts a list..."
user:      "Make it handle None values"
assistant: "Here's the updated function..."
```

This paradigm replaced the earlier "completions" API where you sent a single text prompt and got a continuation. The chat format provides structure that enables:
- **System prompts** for persistent instructions and persona
- **Multi-turn conversations** with preserved context
- **Tool calling** via structured assistant messages with tool invocations
- **Clear delineation** between user input and model output

### Message Roles

| Role | Purpose | Provider Support |
|------|---------|------------------|
| `system` | Sets behavior, persona, and instructions | OpenAI, Google (as `system_instruction`), Anthropic (as top-level `system` param) |
| `user` | Human input — text, images, files | All providers |
| `assistant` | Model output — text, tool calls, refusals | All providers |
| `tool` | Results from tool/function execution | OpenAI, Google, Anthropic (as `tool_result` role) |
| `developer` | OpenAI-specific: trusted instructions from the application developer | OpenAI only (Responses API) |

### Token Economics

Tokens are the atomic units of LLM processing and billing. Understanding tokenization is critical for cost management:

- **1 token ≈ 4 characters** in English (approximately 0.75 words)
- **Code tokenizes differently** — variable names, operators, and whitespace each consume tokens
- **Input tokens** (prompt) are cheaper than **output tokens** (completion) — typically 2-4x cheaper
- **Cached input tokens** are even cheaper — typically 50% of standard input cost
- Models have a **context window** (max total tokens) and **max output tokens** (max generation length)

### Streaming

All major APIs support server-sent events (SSE) for streaming responses token-by-token:

```
data: {"choices":[{"delta":{"content":"Hello"}}]}
data: {"choices":[{"delta":{"content":" world"}}]}
data: [DONE]
```

Streaming is essential for coding agents because:
- Users see output as it's generated, reducing perceived latency
- Agents can begin processing tool calls before the full response completes
- Long generations (multi-file edits) would otherwise have unacceptable time-to-first-token

---

## Summary Comparison of Major APIs

| Feature | OpenAI Chat Completions | OpenAI Responses API | Anthropic Messages | Google Gemini |
|---------|------------------------|---------------------|--------------------|---------------|
| **Endpoint** | `POST /v1/chat/completions` | `POST /v1/responses` | `POST /v1/messages` | `POST /v1/models/{m}:generateContent` |
| **Message format** | `messages[]` with role/content | `input` (string or items[]) | `messages[]` with role/content blocks | `contents[]` with role/parts |
| **System prompt** | `system` role in messages | `instructions` param | Top-level `system` param | `system_instruction` param |
| **Tool calling** | `tools[]` + `tool_choice` | Built-in tools + custom `tools[]` | `tools[]` + `tool_choice` | `tools[]` + `tool_config` |
| **Structured output** | `response_format` with JSON schema | `text.format` with JSON schema | Partial (via tool_use trick) | `response_mime_type` + schema |
| **Streaming** | SSE with `delta` objects | SSE with typed events | SSE with typed events | SSE with candidates |
| **Thinking/reasoning** | `reasoning_effort` param (o-series) | `reasoning` param | `thinking` with `budget_tokens` | `thinking_config` with budget |
| **Prompt caching** | Automatic (zero config) | Automatic | `cache_control` breakpoints | Context caching API |
| **Stateful** | No (client manages state) | Yes (server stores conversation) | No (client manages state) | No (client manages state) |
| **Max context** | 128K-200K tokens | 200K tokens | 200K tokens | 1M-2M tokens |
| **Parallel tool calls** | Yes (default) | Yes | Yes (tool_choice `any`) | Yes |

---

## Evolution: From Completions to Agentic APIs

The LLM API landscape has evolved through several distinct phases:

### Phase 1: Text Completions (2020-2023)
- Simple prompt-in, completion-out
- `POST /v1/completions` with a text `prompt`
- No conversation structure, no roles
- Agent developers had to format conversations manually

### Phase 2: Chat Completions (2023-2024)
- Structured conversations with roles
- `POST /v1/chat/completions` with `messages[]`
- Function calling added (June 2023), later renamed to tool calling
- JSON mode for structured output
- This became the universal standard — even non-OpenAI providers adopted this format

### Phase 3: Agentic APIs (2024-present)
- Server-side conversation state (OpenAI Responses API)
- Built-in tools (web search, code execution, file search)
- Extended thinking and reasoning tokens
- Multi-step agent loops with automatic tool execution
- Prompt caching across turns

### Phase 4: Emerging Patterns
- **Model Context Protocol (MCP)** — Anthropic's open standard for tool integration
- **Computer Use / Browser Use** — Models that can interact with desktop and web UIs
- **Multi-agent orchestration** — APIs designed for agent-to-agent communication
- **Real-time APIs** — WebSocket-based voice and video interaction

---

## How Coding Agents Choose Which API to Use

Coding agents typically select their LLM API based on:

1. **Task complexity** → Simple autocomplete uses fast/cheap models (GPT-4o-mini, Haiku); complex multi-file refactoring uses frontier models (Claude Sonnet, GPT-4o, o3)
2. **Latency requirements** → Inline completion needs <500ms time-to-first-token; background tasks can tolerate seconds
3. **Context length** → Large codebases may need 100K+ token contexts; Gemini's 1M+ window is advantageous here
4. **Tool calling needs** → Agents with many tools benefit from strong tool-calling models
5. **Cost constraints** → High-volume usage favors cheaper models or batch APIs
6. **Compliance** → Enterprise deployments may require Azure OpenAI or AWS Bedrock

---

## Common Patterns Across All APIs

### Request/Response

Every LLM API follows this basic pattern:
1. Send an HTTP POST request with a JSON body containing the model name, messages, and parameters
2. Receive a JSON response with the model's output, token usage, and metadata
3. For tool-calling flows, process tool calls and send results back in a follow-up request

### Streaming with Server-Sent Events (SSE)

All providers use SSE for streaming:
- Set `stream: true` in the request
- Response uses `Content-Type: text/event-stream`
- Each event is prefixed with `data: ` followed by a JSON object
- A final sentinel (`data: [DONE]` for OpenAI, `event: message_stop` for Anthropic) signals completion

### Tool/Function Calling

The universal tool-calling pattern:
1. Define tools as JSON objects with name, description, and parameter schema
2. Send the tools array alongside messages
3. Model responds with tool call(s) — function name and arguments
4. Agent executes the function and returns results
5. Model generates a final response incorporating tool results

### Structured Output

Force the model to produce valid JSON matching a schema:
- OpenAI: `response_format: { type: "json_schema", json_schema: { ... } }`
- Anthropic: Use a tool with the desired schema and force it via `tool_choice`
- Google: `response_mime_type: "application/json"` + `response_schema`

---

## Authentication Patterns

### API Keys (Most Common)
```
Authorization: Bearer sk-...
```
Simple, stateless, and used by OpenAI, Anthropic, Google, and most inference providers. Keys are typically scoped to an organization/project.

### OAuth 2.0 (Enterprise)
Azure OpenAI and Google Vertex AI support OAuth with service accounts and managed identities for enterprise deployments where API keys are insufficient.

### Environment Variable Conventions
```bash
OPENAI_API_KEY=sk-...
ANTHROPIC_API_KEY=sk-ant-...
GOOGLE_API_KEY=AI...
```

---

## SDK Ecosystem

| Provider | Python SDK | TypeScript SDK | API Compatibility |
|----------|-----------|----------------|-------------------|
| OpenAI | `openai` (PyPI) | `openai` (npm) | Reference implementation |
| Anthropic | `anthropic` (PyPI) | `@anthropic-ai/sdk` (npm) | Unique API format |
| Google | `google-genai` (PyPI) | `@google/genai` (npm) | Unique API format |
| Azure OpenAI | `openai` (with azure config) | `openai` (with azure config) | OpenAI-compatible |
| Together AI | `together` or `openai` | `openai` (with base URL) | OpenAI-compatible |
| Fireworks | `openai` (with base URL) | `openai` (with base URL) | OpenAI-compatible |
| Ollama | `openai` (with base URL) | `openai` (with base URL) | OpenAI-compatible |

The OpenAI SDK has become the lingua franca — most inference providers offer OpenAI-compatible APIs, meaning a single SDK can target multiple backends by changing the `base_url`.

---

## Cost Comparison (as of mid-2025, per 1M tokens)

| Model | Input Cost | Output Cost | Cached Input | Context Window |
|-------|-----------|-------------|--------------|----------------|
| GPT-4o | $2.50 | $10.00 | $1.25 | 128K |
| GPT-4o-mini | $0.15 | $0.60 | $0.075 | 128K |
| o3 | $10.00 | $40.00 | $2.50 | 200K |
| o4-mini | $1.10 | $4.40 | $0.275 | 200K |
| Claude Sonnet 4 | $3.00 | $15.00 | $0.30 | 200K |
| Claude Opus 4 | $15.00 | $75.00 | $1.50 | 200K |
| Claude Haiku 3.5 | $0.80 | $4.00 | $0.08 | 200K |
| Gemini 2.5 Pro | $1.25 | $10.00 | — | 1M |
| Gemini 2.5 Flash | $0.15 | $0.60 | — | 1M |
| DeepSeek-V3 | $0.27 | $1.10 | $0.07 | 128K |
| DeepSeek-R1 | $0.55 | $2.19 | $0.14 | 128K |

*Note: Prices are approximate and change frequently. Check provider pricing pages for current rates.*

---

## Context Window Comparison

| Model | Total Context | Max Output Tokens | Effective Input Budget |
|-------|--------------|-------------------|----------------------|
| GPT-4o | 128,000 | 16,384 | ~111,600 |
| GPT-4o-mini | 128,000 | 16,384 | ~111,600 |
| o3 | 200,000 | 100,000 | ~100,000 |
| o4-mini | 200,000 | 100,000 | ~100,000 |
| Claude Sonnet 4 | 200,000 | 16,000 (default) / 64,000 (extended) | ~136,000-184,000 |
| Claude Opus 4 | 200,000 | 16,000 (default) / 32,000 (extended) | ~168,000-184,000 |
| Gemini 2.5 Pro | 1,048,576 | 65,536 | ~983,000 |
| Gemini 2.5 Flash | 1,048,576 | 65,536 | ~983,000 |
| DeepSeek-V3 | 128,000 | 8,192 | ~119,800 |
| Llama 3.1 405B | 128,000 | 4,096 | ~123,900 |

---

## References and Links

### Official API Documentation
- [OpenAI API Reference](https://platform.openai.com/docs/api-reference)
- [Anthropic API Reference](https://docs.anthropic.com/en/api)
- [Google Gemini API Reference](https://ai.google.dev/api)
- [DeepSeek API Docs](https://platform.deepseek.com/api-docs)

### SDKs
- [openai-python](https://github.com/openai/openai-python) — Official OpenAI Python SDK
- [openai-node](https://github.com/openai/openai-node) — Official OpenAI TypeScript/Node SDK
- [anthropic-sdk-python](https://github.com/anthropics/anthropic-sdk-python) — Official Anthropic Python SDK
- [anthropic-sdk-typescript](https://github.com/anthropics/anthropic-sdk-typescript) — Official Anthropic TypeScript SDK
- [google-genai (Python)](https://github.com/googleapis/python-genai) — Official Google GenAI Python SDK

### Specifications and Standards
- [OpenAI API Spec (OpenAPI)](https://github.com/openai/openai-openapi) — Machine-readable API specification
- [Model Context Protocol](https://modelcontextprotocol.io/) — Anthropic's open standard for tool integration
- [Server-Sent Events Spec](https://html.spec.whatwg.org/multipage/server-sent-events.html) — W3C SSE specification

### Community Resources
- [LiteLLM](https://github.com/BerriAI/litellm) — Unified interface for 100+ LLM providers
- [Instructor](https://github.com/jxnl/instructor) — Structured output extraction across providers
- [tiktoken](https://github.com/openai/tiktoken) — OpenAI's tokenizer library for token counting
