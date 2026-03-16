---
title: "Chapter 14: Provider Abstraction"
description: Build a flexible provider abstraction layer that lets your coding agent work with any LLM backend through a unified interface.
---

# Provider Abstraction

A production coding agent should not be welded to a single LLM provider. Users want to choose between Anthropic, OpenAI, local models via Ollama, and whatever new provider appears next month. Your agent's core logic — the agentic loop, tool system, and conversation management — should be completely decoupled from the specifics of any one API. This chapter teaches you how to build that decoupling properly.

We start with the abstraction principles that guide good provider interfaces, then dive into Rust's trait system as the mechanism for defining provider contracts. You will implement the adapter pattern to wrap each provider's unique API behind a common interface, building concrete adapters for Anthropic, OpenAI, and Ollama. Along the way, we handle the messy realities: different providers support different model capabilities, streaming formats vary, and error shapes differ.

The chapter also covers runtime provider switching (letting users change models mid-session), fallback chains for resilience, and cost/usage tracking across providers. By the end, you will have a provider layer that makes adding a new LLM backend a matter of implementing a single trait rather than touching code throughout the agent.

## Learning Objectives
- Apply abstraction principles to design a provider interface that captures LLM capabilities without leaking implementation details
- Use Rust traits and associated types to define a provider contract with compile-time safety guarantees
- Implement the adapter pattern to wrap Anthropic, OpenAI, and Ollama APIs behind a unified interface
- Build a model capabilities registry that tracks what each provider/model combination supports
- Implement runtime provider switching with fallback chains and automatic retry logic
- Add cost and usage tracking that works transparently across all providers

## Subchapters
1. [Abstraction Principles](/linear/14-provider-abstraction/01-abstraction-principles)
2. [Trait Based Design](/linear/14-provider-abstraction/02-trait-based-design)
3. [Adapter Pattern](/linear/14-provider-abstraction/03-adapter-pattern)
4. [Anthropic Provider](/linear/14-provider-abstraction/04-anthropic-provider)
5. [OpenAI Provider](/linear/14-provider-abstraction/05-openai-provider)
6. [Ollama Local Provider](/linear/14-provider-abstraction/06-ollama-local-provider)
7. [Model Capabilities Registry](/linear/14-provider-abstraction/07-model-capabilities-registry)
8. [Runtime Switching](/linear/14-provider-abstraction/08-runtime-switching)
9. [Fallback and Retry](/linear/14-provider-abstraction/09-fallback-and-retry)
10. [Cost and Usage Tracking](/linear/14-provider-abstraction/10-cost-and-usage-tracking)
11. [Provider Testing](/linear/14-provider-abstraction/11-provider-testing)
12. [Summary](/linear/14-provider-abstraction/12-summary)

## Prerequisites
- Chapter 3 (understanding of LLM API request/response structures)
- Chapter 8 (streaming patterns and server-sent events)
