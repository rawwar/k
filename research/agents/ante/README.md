---
title: Ante (Antigma Labs) — Self-Organizing Terminal Coding Agent
status: complete
---

# Ante

> Rust-built, self-contained terminal coding agent by Antigma Labs — designed around self-organizing multi-agent intelligence with lock-free scheduling and full offline capability.

## Overview

**Ante** is an in-terminal coding agent built from scratch in Rust by **Antigma Labs**. Unlike most AI coding tools that layer on top of existing agent frameworks or scripting runtimes, Ante is a ground-up, first-principles implementation — its own runtime, its own scheduler, its own orchestration layer. The result is a fast, self-contained binary with no external framework dependencies.

Antigma Labs' mission is *"building substrate for self-organizing intelligence."* The company name is a play on "anti-Enigma," inspired by Alan Turing's codebreaking work at Bletchley Park. Their core philosophy treats agents as teammates and the user as just another agent in the system — not a privileged operator issuing commands, but a peer collaborator. This worldview extends to broader beliefs around individual AI sovereignty: Antigma frames personal AI tooling as a fundamental right, analogous to "the right to bear arms" in the AI era.

Ante sits in a distinctive corner of the AI coding agent landscape. Where tools like Claude Code and Codex prioritize cloud-connected, single-model agentic loops, Ante emphasizes **offline-first operation**, **multi-agent orchestration**, and **Rust-level performance guarantees**. It can run entirely self-contained with local models — no cloud dependency required — or connect to cloud LLM providers when available. A meta-agent layer orchestrates multiple sub-agents, allowing complex tasks to be decomposed and parallelized across a lock-free scheduling runtime.

### What Makes Ante Special

1. **Rust From Scratch**: Not a wrapper around Python or Node.js agent frameworks. Ante is written entirely in Rust, giving it memory safety, zero-cost abstractions, and a lock-free concurrency model purpose-built for multi-agent orchestration.

2. **Self-Organizing Multi-Agent Architecture**: A meta-agent orchestrates dynamically spawned sub-agents. Tasks are decomposed and distributed, with the user treated as a peer agent rather than a privileged controller.

3. **Offline-First Design**: Ante is fully self-contained and can operate without any cloud dependency. Paired with Antigma's own inference stack (see `nanochat-rs`), it can run local models entirely on-device.

4. **Lock-Free Scheduling**: The Rust runtime uses lock-free data structures for agent scheduling and orchestration, avoiding the mutex contention that plagues thread-based agent systems.

5. **Own Inference Stack**: Antigma Labs builds their own components — their `nanochat-rs` project is a tiny GPT-style cognitive core in pure Rust built on HuggingFace Candle, and `mcp-sdk` is a minimalistic Rust MCP implementation. They don't just consume the ecosystem; they build it.

6. **Benchmark Integrity Advocacy**: Ante was notably used as the investigative tool to write a forensic analysis exposing benchmark manipulation on Terminal-Bench 2.0 — a 13-minute deep-dive blog post titled *"How to Achieve #1 on Terminal Bench (and Why We Can't Have Nice Things)."*

## Benchmark Scores

| Benchmark | Configuration | Rank | Score |
|-----------|--------------|------|-------|
| Terminal-Bench 2.0 | Ante + Gemini 3 Pro | #18 | 69.4% |
| Terminal-Bench 1.0 | Ante + Claude Sonnet 4.5 | #4 | 60.3% |

## Key Stats

- **Language**: Rust (built from scratch, no agent framework dependency)
- **License**: Proprietary (core); open-source components on GitHub
- **Offline mode**: Full local operation with no cloud dependency
- **Open-source projects**: [`mcp-sdk`](https://github.com/AntigmaLabs/mcp-sdk) (64 ★), [`nanochat-rs`](https://github.com/AntigmaLabs/nanochat-rs) (62 ★)
- **Three pillars**: Privacy (private networks), Trust (trust boundaries), Tribute (compute united)
- **Docs**: [docs.antigma.ai](https://docs.antigma.ai) (access-restricted)

## Architecture at a Glance

```
┌─────────────────────────────────────────┐
│            User (Terminal)              │
└────────────────┬────────────────────────┘
                 │
       ┌─────────▼──────────┐
       │    Meta Agent       │
       │  (Orchestrator)     │
       └──┬──────┬───────┬──┘
          │      │       │
    ┌─────▼┐  ┌──▼───┐ ┌─▼────────┐
    │Sub   │  │Sub   │ │Sub       │
    │Agent │  │Agent │ │Agent     │
    │(task)│  │(task)│ │(task)    │
    └──┬───┘  └──┬───┘ └──┬──────┘
       │         │        │
    ┌──▼─────────▼────────▼──────┐
    │  Rust Runtime               │
    │  Lock-free Scheduler        │
    │  Offline / Cloud LLM Layer  │
    └─────────────────────────────┘
```

## Files in This Research

| File | Contents |
|------|----------|
| [architecture.md](architecture.md) | Rust runtime design, lock-free scheduler, meta-agent orchestration layer |
| [agentic-loop.md](agentic-loop.md) | Meta-agent → sub-agent task decomposition and execution loop |
| [tool-system.md](tool-system.md) | MCP integration via `mcp-sdk`, tool invocation from sub-agents |
| [context-management.md](context-management.md) | Context sharing between agents, offline vs. cloud model context strategies |
| [unique-patterns.md](unique-patterns.md) | Lock-free scheduling, self-organizing intelligence, own inference stack |
| [benchmarks.md](benchmarks.md) | Terminal-Bench 1.0 and 2.0 results, benchmark integrity analysis |
| [references.md](references.md) | Links to Antigma Labs site, GitHub repos, blog posts, docs |
