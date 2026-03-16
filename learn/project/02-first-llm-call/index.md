---
title: "Chapter 2: First LLM Call"
description: Connect your Rust CLI to the Anthropic API and send your first message to Claude.
---

# First LLM Call

This chapter bridges the gap between a local CLI application and a networked AI-powered tool. You will learn how HTTP APIs work at the protocol level, then use the reqwest crate to make real requests to the Anthropic Messages API. Every response comes back as JSON, so you will also learn how serde deserializes structured data into strongly-typed Rust structs.

Beyond the mechanics of a single request-response cycle, this chapter covers the practical concerns that every production API integration must handle: managing API keys securely through environment variables, dealing with rate limits and transient errors through retry logic, and understanding the message format that Claude expects. You will also get your first taste of async Rust, which is essential for non-blocking I/O in a responsive CLI.

By the end of this chapter your REPL from Chapter 1 will be able to send a user prompt to Claude and print the response. This is the moment the project stops being a toy and starts becoming an agent.

## Learning Objectives
- Understand the Anthropic Messages API request and response format
- Make HTTP POST requests from Rust using reqwest with proper headers and authentication
- Deserialize JSON responses into typed Rust structs with serde
- Handle API errors, rate limits, and transient failures gracefully
- Manage secrets with environment variables instead of hardcoding them
- Write basic async Rust code with tokio to perform non-blocking network I/O

## Subchapters
1. [Understanding LLM APIs](/project/02-first-llm-call/01-understanding-llm-apis)
2. [Anthropic API Overview](/project/02-first-llm-call/02-anthropic-api-overview)
3. [API Keys and Config](/project/02-first-llm-call/03-api-keys-and-config)
4. [HTTP in Rust with Reqwest](/project/02-first-llm-call/04-http-in-rust-with-reqwest)
5. [Making Your First Request](/project/02-first-llm-call/05-making-your-first-request)
6. [Parsing JSON with Serde](/project/02-first-llm-call/06-parsing-json-with-serde)
7. [Message Format Deep Dive](/project/02-first-llm-call/07-message-format-deep-dive)
8. [System Prompts](/project/02-first-llm-call/08-system-prompts)
9. [Handling API Errors](/project/02-first-llm-call/09-handling-api-errors)
10. [Rate Limiting and Retries](/project/02-first-llm-call/10-rate-limiting-and-retries)
11. [Environment Variables](/project/02-first-llm-call/11-environment-variables)
12. [Async Rust Basics](/project/02-first-llm-call/12-async-rust-basics)
13. [Streaming Intro](/project/02-first-llm-call/13-streaming-intro)
14. [Summary](/project/02-first-llm-call/14-summary)

## Prerequisites
- Chapter 1 completed (working Rust environment, basic Rust syntax)
- An Anthropic API key (free tier is sufficient for all exercises)
- Basic understanding of HTTP request/response cycles
