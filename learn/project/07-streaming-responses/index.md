---
title: "Chapter 7: Streaming Responses"
description: Implementing server-sent events, token-by-token rendering, and partial tool call assembly for real-time AI responses.
---

# Streaming Responses

Waiting for a complete response before displaying anything to the user is unacceptable in an interactive coding agent. This chapter teaches you how to implement streaming from the ground up, turning your agent from a batch processor into a responsive, real-time assistant.

You will start with the Server-Sent Events (SSE) protocol and chunked transfer encoding that underpin the Anthropic streaming API. From there, you will build a token-by-token rendering pipeline that displays text as it arrives, and tackle the tricky problem of assembling partial tool call JSON fragments into complete, parseable tool invocations.

The chapter culminates in a streaming state machine that manages the full lifecycle of a streamed response, from connection setup through content delivery to graceful completion or error recovery. You will also implement interrupt handling so users can cancel mid-stream, backpressure mechanisms to prevent memory blowouts on slow terminals, and reconnection logic for unreliable networks.

## Learning Objectives
- Parse and consume Server-Sent Events (SSE) streams from the Anthropic API
- Render tokens incrementally as they arrive for a responsive user experience
- Assemble partial JSON fragments into complete tool call invocations
- Build a state machine that tracks streaming lifecycle phases
- Handle interrupts, backpressure, and error recovery during streaming
- Implement reconnection with exponential backoff for dropped connections

## Subchapters
1. [Why Streaming](/project/07-streaming-responses/01-why-streaming)
2. [SSE Protocol](/project/07-streaming-responses/02-sse-protocol)
3. [Chunked Transfer](/project/07-streaming-responses/03-chunked-transfer)
4. [Token By Token Rendering](/project/07-streaming-responses/04-token-by-token-rendering)
5. [Partial Tool Call Assembly](/project/07-streaming-responses/05-partial-tool-call-assembly)
6. [Buffering Strategies](/project/07-streaming-responses/06-buffering-strategies)
7. [Interrupt Handling](/project/07-streaming-responses/07-interrupt-handling)
8. [Backpressure](/project/07-streaming-responses/08-backpressure)
9. [Streaming State Machine](/project/07-streaming-responses/09-streaming-state-machine)
10. [Error Recovery](/project/07-streaming-responses/10-error-recovery)
11. [Reconnection](/project/07-streaming-responses/11-reconnection)
12. [Progress Display](/project/07-streaming-responses/12-progress-display)
13. [Real Time UI Updates](/project/07-streaming-responses/13-real-time-ui-updates)
14. [Summary](/project/07-streaming-responses/14-summary)

## Prerequisites
- Chapter 2: HTTP client setup and Anthropic API interaction patterns
- Chapter 3: The agentic loop and message processing pipeline
