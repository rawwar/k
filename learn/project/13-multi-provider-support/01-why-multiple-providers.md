---
title: Why Multiple Providers
description: The business and technical case for supporting multiple LLM providers, including cost optimization, availability, capability matching, and avoiding vendor lock-in.
---

# Why Multiple Providers

> **What you'll learn:**
> - Why vendor lock-in is a practical risk for LLM-powered tools and how abstraction mitigates it
> - How different providers excel at different tasks, making multi-provider support a quality advantage
> - The cost, latency, and availability trade-offs that drive provider selection at runtime

Up until now, your agent has been hardwired to a single LLM API. Every HTTP request, every streaming parser, every message format assumption lives in code that directly references one provider. That works fine for learning and prototyping, but it creates real problems as soon as your agent faces production workloads. In this subchapter, you will explore why supporting multiple providers is not just a nice-to-have -- it is a strategic requirement for any serious coding agent.

## The Vendor Lock-in Problem

When your entire agent depends on a single provider's API format, authentication scheme, and pricing model, you are locked in. If that provider raises prices by 50%, you absorb the cost or scramble to rewrite. If they experience an outage during a critical deployment, your agent stops working entirely. If they deprecate a model you depend on, you are on their timeline.

This is not a hypothetical concern. LLM providers regularly adjust pricing, retire models, and change API behavior. A coding agent that cannot switch providers is at the mercy of a single company's business decisions.

The solution is an abstraction layer -- a provider trait in Rust terms -- that isolates your core agent logic from any specific API. Your agentic loop, tool system, conversation management, and permission system all interact with the trait, never with a concrete provider directly.

::: python Coming from Python
In Python, you might handle multiple providers with duck typing -- just pass any object that has a `send_message()` method. Or you might use `typing.Protocol` for structural subtyping:
```python
from typing import Protocol, AsyncIterator

class LLMProvider(Protocol):
    async def send_message(self, messages: list[dict]) -> dict: ...
    async def stream_message(self, messages: list[dict]) -> AsyncIterator[dict]: ...
```
Rust's approach uses traits, which serve a similar purpose but are checked at compile time. You will never accidentally pass an object that is missing a required method -- the compiler catches it before the code runs.
:::

## Different Models for Different Jobs

Not every task demands the most capable (and expensive) model. Consider the range of operations a coding agent performs:

- **Simple file reads and searches**: A small, fast model handles these well. The agent just needs to understand "read this file" or "search for this pattern."
- **Complex refactoring**: Restructuring code across multiple files benefits from a larger context window and stronger reasoning. You want the best model available.
- **Code review and explanation**: Mid-tier models often produce excellent explanations at a fraction of the cost.
- **Test generation**: Models with strong code generation but moderate reasoning can write good tests quickly.

A multi-provider agent can route each request to the most appropriate model. You might use Claude for complex architectural decisions, GPT-4o for quick code completions, and a local Ollama model for file operations that do not need cloud-level intelligence.

This is not just about cost -- it is about quality. Matching the model to the task often produces better results than always using the largest model, because smaller models tend to be faster and less prone to over-thinking simple requests.

## Cost Optimization

LLM API costs add up quickly in a coding agent. A typical agent session might involve 20-50 LLM calls, each consuming thousands of tokens. At the rates charged by frontier models, a single coding session can cost several dollars. Multiply that by a team of developers using the agent throughout the day, and monthly bills reach into the thousands.

Multi-provider support enables a cost optimization strategy. You define a tiered approach:

| Tier | Use Case | Example Models | Cost Level |
|------|----------|---------------|------------|
| Local | Simple tool dispatch, formatting | Ollama (Llama 3, CodeLlama) | Free |
| Economy | File reads, simple edits, search | GPT-4o-mini, Claude Haiku | Low |
| Standard | Code generation, review, debugging | GPT-4o, Claude Sonnet | Medium |
| Premium | Complex refactoring, architecture | Claude Opus, o1 | High |

Your agent can start with an economy model and escalate to premium only when the task requires it -- or when the economy model fails to produce a satisfactory result. This automatic tiering can reduce costs by 60-80% compared to always using a frontier model.

## Availability and Reliability

Cloud services go down. Rate limits get hit. Network connectivity fluctuates. A single-provider agent treats all of these as fatal errors. A multi-provider agent treats them as routing decisions.

When your primary provider returns a 429 (rate limited) or 503 (service unavailable), the agent can automatically fall back to an alternative. When you are working offline or on a restricted network, a local Ollama model keeps the agent functional, even if with reduced capabilities.

This fallback behavior is especially valuable for professional use cases where developers cannot afford to stop working because an API endpoint is temporarily unavailable.

::: wild In the Wild
Claude Code primarily uses Anthropic's own models but is designed with an internal provider abstraction. OpenCode takes multi-provider support further, allowing users to configure any OpenAI-compatible endpoint, which covers a broad range of providers since many LLM APIs follow the OpenAI chat completions format. Codex similarly defaults to OpenAI models but supports custom base URLs for compatible providers.
:::

## Privacy and Compliance

Some organizations cannot send code to external APIs due to security policies, regulatory requirements, or intellectual property concerns. Local model support through Ollama means your agent can operate entirely on-premises, processing sensitive code without any data leaving the developer's machine.

A flexible provider system lets the same agent binary serve different deployment contexts:
- **Personal development**: Cloud providers for maximum capability
- **Enterprise with restrictions**: Approved cloud providers only, with specific models
- **Air-gapped environments**: Local models exclusively

## The Architecture Ahead

Over the next eleven subchapters, you will build this provider abstraction layer piece by piece:

1. A **provider trait** that defines the contract all providers must satisfy
2. **Concrete adapters** for Anthropic, OpenAI, and Ollama that implement the trait
3. **Capability detection** so the agent knows what each model can do
4. **Runtime switching** to change models mid-session
5. **Fallback chains** for automatic retry on failure
6. **Cost tracking** for budget awareness
7. **Configuration** for managing API keys and provider preferences
8. **Testing infrastructure** for verifying adapter correctness

The goal is clean separation of concerns: your agentic loop never knows or cares which provider is handling a request. It sends messages through the trait and receives responses in a uniform format. All the provider-specific translation happens behind the adapter boundary.

## Key Takeaways

- Vendor lock-in is a practical risk that affects cost, reliability, and autonomy -- a provider abstraction layer mitigates all three
- Different models excel at different tasks; routing requests to the right model improves both quality and cost efficiency
- Fallback chains turn provider outages from fatal errors into transparent routing decisions
- Local model support enables offline operation and satisfies privacy and compliance requirements
- The provider trait you will build isolates the entire agent core from provider-specific API details
