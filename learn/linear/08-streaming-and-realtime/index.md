---
title: "Chapter 8: Streaming and Realtime"
description: Mastering HTTP streaming protocols, server-sent events, and incremental rendering to build responsive real-time agent interfaces.
---

# Streaming and Realtime

This chapter tackles one of the most user-visible aspects of a coding agent: streaming responses in real time. When an LLM generates a response token by token, the agent must receive those tokens over HTTP, parse the streaming protocol, and render them incrementally to the user. The difference between a snappy, responsive agent and one that feels sluggish comes down to how well this streaming pipeline is engineered.

We begin with the HTTP-level protocols that make streaming possible: chunked transfer encoding and server-sent events (SSE). You will learn how these protocols work at the wire level, how to parse them correctly in Rust, and how to handle the many edge cases that arise with partial JSON payloads, network interruptions, and backpressure. These are not abstract concerns — every production LLM API uses one of these protocols.

The chapter then moves to the application-level challenges of streaming: how to render partial content without flickering, how to handle user interrupts mid-stream, how to reconnect after network failures, and how to manage buffering for optimal throughput and latency. You will build the streaming infrastructure that later chapters depend on for terminal rendering and conversation state management.

## Learning Objectives
- Understand HTTP streaming protocols including chunked encoding and server-sent events at the wire level
- Parse SSE streams in Rust with correct handling of event types, retry fields, and multi-line data
- Handle partial JSON payloads that arrive mid-stream without valid parse boundaries
- Implement backpressure and flow control between the network layer and the rendering layer
- Build reconnection logic with exponential backoff and stream resumption
- Design buffering strategies that balance latency and throughput for real-time display

## Subchapters
1. [Why Streaming Matters](/linear/08-streaming-and-realtime/01-why-streaming-matters)
2. [HTTP Streaming Protocols](/linear/08-streaming-and-realtime/02-http-streaming-protocols)
3. [Server Sent Events](/linear/08-streaming-and-realtime/03-server-sent-events)
4. [Chunked Encoding](/linear/08-streaming-and-realtime/04-chunked-encoding)
5. [Parsing SSE in Rust](/linear/08-streaming-and-realtime/05-parsing-sse-in-rust)
6. [Partial JSON Handling](/linear/08-streaming-and-realtime/06-partial-json-handling)
7. [Incremental Rendering](/linear/08-streaming-and-realtime/07-incremental-rendering)
8. [Backpressure and Flow Control](/linear/08-streaming-and-realtime/08-backpressure-and-flow-control)
9. [Interrupt and Cancel](/linear/08-streaming-and-realtime/09-interrupt-and-cancel)
10. [Reconnection Strategies](/linear/08-streaming-and-realtime/10-reconnection-strategies)
11. [Buffering Patterns](/linear/08-streaming-and-realtime/11-buffering-patterns)
12. [Event Driven Architecture](/linear/08-streaming-and-realtime/12-event-driven-architecture)
13. [Performance Considerations](/linear/08-streaming-and-realtime/13-performance-considerations)
14. [Summary](/linear/08-streaming-and-realtime/14-summary)

## Prerequisites
- Chapter 3 (LLM API streaming protocol basics and familiarity with API response formats)
