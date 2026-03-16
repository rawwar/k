---
title: Understanding LLM APIs
description: Learn how large language model APIs work at a conceptual level, from prompt submission to token-based response generation.
---

# Understanding LLM APIs

> **What you'll learn:**
> - How LLM APIs accept a sequence of messages and return generated completions
> - The difference between chat-based and completion-based API paradigms
> - How tokens, context windows, and pricing relate to the requests you send

In Chapter 1 you built a REPL that reads user input and echoes it back. Now you are going to replace that echo with something far more interesting: a call to a large language model that generates an intelligent response. Before you write any Rust networking code, though, you need a solid mental model of how LLM APIs work. This subchapter gives you that foundation.

## The Request-Response Cycle

At the most fundamental level, an LLM API is an HTTP endpoint. You send it an HTTP POST request containing a structured prompt, and it returns an HTTP response containing the model's generated text. This is no different from calling any other REST API -- the same HTTP verbs, headers, status codes, and JSON payloads you have used in Python apply here.

What makes LLM APIs distinctive is *what* goes inside that request and *how* the response is generated. Instead of querying a database or running a deterministic function, the server feeds your prompt into a neural network that predicts the next token (roughly, the next word or word-piece) one at a time until it decides to stop.

Here is the conceptual flow:

1. **You construct a request** with a list of messages (the conversation so far), a model identifier, and parameters like `max_tokens`.
2. **The API server** loads your messages into the model's context window.
3. **The model generates tokens** one at a time, each conditioned on everything that came before it.
4. **The server packages the generated tokens** into a response and sends it back to you as JSON.

From your code's perspective, this looks like a regular HTTP call that takes a few seconds to return. Behind the scenes, the model is doing billions of floating-point operations per token, but you do not need to care about that. Your job is to construct the right request and parse the response.

## Chat-Based vs. Completion-Based APIs

Early LLM APIs used a **completion** paradigm: you sent a single string of text (a prompt), and the model continued it. This is how GPT-3's original API worked. You had to manually format multi-turn conversations by embedding markers like `Human:` and `Assistant:` inside your prompt string, then hope the model understood the structure.

Modern APIs, including the Anthropic Messages API you will use in this chapter, use a **chat** (or messages) paradigm instead. You send a structured array of message objects, each with a `role` (like `"user"` or `"assistant"`) and `content`. The API enforces the conversational structure for you.

```json
{
  "model": "claude-sonnet-4-20250514",
  "max_tokens": 1024,
  "messages": [
    { "role": "user", "content": "What is the capital of France?" }
  ]
}
```

This is a complete, valid request to the Anthropic Messages API. The `messages` array contains one user message. The response comes back with an `assistant` role and the model's generated text:

```json
{
  "id": "msg_01XFDUDYJgAACzvnptvVoYEL",
  "type": "message",
  "role": "assistant",
  "content": [
    { "type": "text", "text": "The capital of France is Paris." }
  ],
  "model": "claude-sonnet-4-20250514",
  "stop_reason": "end_turn",
  "usage": { "input_tokens": 15, "output_tokens": 10 }
}
```

Notice that the response `content` is an array of content blocks, not a plain string. This is because a single response can contain multiple blocks of different types -- text, tool use calls, and more. You will explore this structure in depth in the [Message Format Deep Dive](/project/02-first-llm-call/07-message-format-deep-dive).

::: python Coming from Python
If you have used the `anthropic` Python SDK, you have seen this same structure:
```python
import anthropic

client = anthropic.Anthropic()
message = client.messages.create(
    model="claude-sonnet-4-20250514",
    max_tokens=1024,
    messages=[{"role": "user", "content": "What is the capital of France?"}],
)
print(message.content[0].text)
```
The Python SDK wraps the HTTP call and JSON parsing for you. In Rust, you will build this plumbing yourself using `reqwest` and `serde`, which gives you a much deeper understanding of what is actually happening on the wire.
:::

## Tokens and Context Windows

LLMs do not process raw text. They first break your input into **tokens** using a tokenizer. A token is typically 3-4 characters of English text, though this varies. The word "understanding" might be two tokens (`understand` + `ing`), while "cat" is one.

Tokens matter for two practical reasons:

**Context window size.** Every model has a maximum number of tokens it can process in a single request. For Claude, this ranges from 200,000 tokens for the latest models. Your messages, the system prompt, and the model's response all share this window. If you exceed it, the API returns an error.

**Pricing.** API calls are billed per token, separately for input tokens (what you send) and output tokens (what the model generates). The response's `usage` field tells you exactly how many tokens were consumed:

```json
"usage": {
  "input_tokens": 15,
  "output_tokens": 10
}
```

For your coding agent, context window management becomes critical as conversations grow longer. Each time you call the API, you send the *entire* conversation history -- every user message and assistant response so far. A long debugging session can easily consume tens of thousands of tokens. You will tackle context management strategies in a later chapter; for now, just be aware that tokens are the currency of LLM interactions.

## Statelessness

One detail that surprises many developers coming from a chat interface like claude.ai: the API is **stateless**. The server does not remember previous requests. Every API call must include the complete conversation history in the `messages` array.

This means that to implement a multi-turn conversation in your CLI agent, you maintain a `Vec<Message>` on your side, append each new user message and each assistant response to it, and send the full vector with every request. The model sees the entire conversation each time and generates its next response accordingly.

```
Request 1: [user: "Hi"]
Response 1: [assistant: "Hello! How can I help?"]

Request 2: [user: "Hi", assistant: "Hello! How can I help?", user: "What is Rust?"]
Response 2: [assistant: "Rust is a systems programming language..."]
```

This stateless design is actually an advantage for your agent. You have full control over the conversation state. You can insert, modify, or remove messages before sending them. You can branch conversations, retry with different prompts, or summarize earlier messages to save tokens. The API does not impose any history management on you -- it simply processes whatever messages you provide.

## What You Need to Make a Call

To make your first LLM API call, you need four things:

1. **An API key** -- a secret string that authenticates your requests (covered in [API Keys and Config](/project/02-first-llm-call/03-api-keys-and-config)).
2. **An HTTP client** -- something that can send POST requests with JSON bodies and custom headers (covered in [HTTP in Rust with Reqwest](/project/02-first-llm-call/04-http-in-rust-with-reqwest)).
3. **A properly formatted request body** -- JSON matching the Messages API schema (covered in [Making Your First Request](/project/02-first-llm-call/05-making-your-first-request)).
4. **A response parser** -- something that turns the JSON response back into Rust types (covered in [Parsing JSON with Serde](/project/02-first-llm-call/06-parsing-json-with-serde)).

The rest of this chapter walks you through each of these pieces one by one, building up to a working end-to-end integration.

::: wild In the Wild
Every production coding agent -- Claude Code, OpenCode, Codex -- starts with this same basic loop: construct a messages array, POST it to an LLM API, parse the response. The sophistication comes from what happens *around* that loop: tool dispatch, context management, streaming, and error recovery. You are building the foundation that everything else rests on.
:::

## Key Takeaways

- LLM APIs are standard HTTP endpoints: you POST a JSON request and receive a JSON response containing the model's generated text.
- Modern APIs use a chat/messages paradigm with structured `role` and `content` fields, replacing the older raw-prompt completion style.
- The API is stateless -- your code must maintain the full conversation history and send it with every request.
- Tokens are the fundamental unit of LLM interaction, determining both context window limits and pricing.
- Making an API call requires four components: an API key, an HTTP client, a formatted request body, and a response parser.
