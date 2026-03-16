---
title: "Chapter 3: Understanding LLMs"
description: How language models work from an agent builder's perspective — tokens, APIs, tool use, streaming, and prompt engineering.
---

# Understanding LLMs

Building a coding agent requires a practical understanding of how large language models work — not as a researcher studying architectures, but as an engineer who needs to know what goes over the wire, how tokens are counted, and why the model sometimes does unexpected things. This chapter provides exactly that level of understanding.

We start with the fundamentals: what tokens are, how context windows constrain your design, and how temperature affects output. We then dive deep into the API layer, covering message formats, the tool use protocol, function calling, and streaming. You will learn the specific anatomy of both Anthropic and OpenAI APIs, because a good agent should be provider-agnostic.

The chapter culminates with prompt engineering techniques specifically tailored for agent systems. Writing a system prompt for an agent is fundamentally different from writing one for a chatbot — you need to guide the model toward tool use, structured output, and multi-step reasoning. By the end, you will understand the LLM not as a black box but as a well-characterized component with known behaviors, limitations, and configuration surfaces.

## Learning Objectives
- Explain how tokenization works and why token counts matter for agent design
- Understand context window limits and strategies for managing conversation length
- Implement the tool use protocol including function calling and result handling
- Work with both Anthropic and OpenAI API formats for messages, tools, and streaming
- Apply prompt engineering techniques that improve agent reliability and tool use accuracy
- Make informed decisions about model selection, temperature, and sampling parameters

## Subchapters
1. [What Are LLMs](/linear/03-understanding-llms/01-what-are-llms)
2. [Tokens and Tokenization](/linear/03-understanding-llms/02-tokens-and-tokenization)
3. [Context Windows](/linear/03-understanding-llms/03-context-windows)
4. [Temperature and Sampling](/linear/03-understanding-llms/04-temperature-and-sampling)
5. [System Prompts](/linear/03-understanding-llms/05-system-prompts)
6. [Message Formats](/linear/03-understanding-llms/06-message-formats)
7. [Tool Use Protocol](/linear/03-understanding-llms/07-tool-use-protocol)
8. [Function Calling Deep Dive](/linear/03-understanding-llms/08-function-calling-deep-dive)
9. [JSON Mode](/linear/03-understanding-llms/09-json-mode)
10. [Streaming Protocol](/linear/03-understanding-llms/10-streaming-protocol)
11. [API Anatomy Anthropic](/linear/03-understanding-llms/11-api-anatomy-anthropic)
12. [API Anatomy OpenAI](/linear/03-understanding-llms/12-api-anatomy-openai)
13. [Rate Limits and Pricing](/linear/03-understanding-llms/13-rate-limits-and-pricing)
14. [Prompt Engineering for Agents](/linear/03-understanding-llms/14-prompt-engineering-for-agents)
15. [Model Selection](/linear/03-understanding-llms/15-model-selection)
16. [Summary](/linear/03-understanding-llms/16-summary)

## Prerequisites
- Chapter 1 (understanding of what agents do)
