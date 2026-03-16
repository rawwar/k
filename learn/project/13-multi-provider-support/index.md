---
title: "Chapter 13: Multi-Provider Support"
description: Abstracting LLM providers behind a common trait to support Anthropic, OpenAI, and local models with seamless switching and fallback.
---

# Multi-Provider Support

A production coding agent should not be locked to a single LLM provider. Users have different preferences, budgets, and compliance requirements. Some tasks benefit from a more capable model while others need a faster, cheaper one. This chapter builds the provider abstraction layer that lets your agent work with Anthropic's Claude, OpenAI's GPT models, and local models through Ollama, all through a unified interface.

You will start by designing a provider trait that captures the common capabilities across LLM APIs: sending messages, streaming responses, and reporting token usage. Then you will implement concrete adapters for each provider, handling the differences in API formats, authentication, and streaming protocols. The chapter covers model capability detection so the agent knows which features each model supports, runtime model switching for cost optimization, and fallback chains that automatically retry with an alternative provider when one fails.

Cost tracking rounds out the chapter, giving you visibility into per-request and per-session spending across providers. By the end, your agent will be provider-agnostic at the core, with provider-specific details cleanly encapsulated behind adapter implementations.

## Learning Objectives
- Design a provider trait that abstracts away differences between LLM APIs
- Implement adapters for Anthropic, OpenAI, and Ollama with proper streaming support
- Build model capability detection for feature-aware tool and prompt construction
- Create runtime model switching and automatic fallback chains
- Implement cost tracking and token usage reporting across providers
- Test provider adapters with mock servers and recorded responses

## Subchapters
1. [Why Multiple Providers](/project/13-multi-provider-support/01-why-multiple-providers)
2. [Provider Trait](/project/13-multi-provider-support/02-provider-trait)
3. [Anthropic Adapter](/project/13-multi-provider-support/03-anthropic-adapter)
4. [OpenAI Adapter](/project/13-multi-provider-support/04-openai-adapter)
5. [Local Models Ollama](/project/13-multi-provider-support/05-local-models-ollama)
6. [Model Capabilities](/project/13-multi-provider-support/06-model-capabilities)
7. [Model Switching](/project/13-multi-provider-support/07-model-switching)
8. [Fallback Chains](/project/13-multi-provider-support/08-fallback-chains)
9. [Cost Tracking](/project/13-multi-provider-support/09-cost-tracking)
10. [Provider Config](/project/13-multi-provider-support/10-provider-config)
11. [Testing Providers](/project/13-multi-provider-support/11-testing-providers)
12. [Summary](/project/13-multi-provider-support/12-summary)

## Prerequisites
- Chapter 2: API integration patterns and HTTP client usage
- Chapter 7: Streaming response handling and SSE parsing
- Chapter 12: Permission and safety system (the agent you are extending)
