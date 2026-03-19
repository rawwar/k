---
title: "Ante — References & Links"
status: complete
---

# Ante — References & Links

> Curated collection of references, links, and resources related to **Ante** by Antigma Labs — a terminal-native AI coding agent built in Rust. This page covers official channels, source code, publications, benchmark results, and the broader ecosystem that informs Ante's design.

---

## Official Resources

- **Website** — [antigma.ai](https://antigma.ai)
  Product homepage for Antigma Labs; includes blog, documentation links, and company information.

- **Documentation** — [docs.antigma.ai](https://docs.antigma.ai)
  Technical documentation for Ante and related tooling. Access-restricted; may require an account or invitation.

- **Discord** — [discord.gg/pqhj3DNGz2](https://discord.gg/pqhj3DNGz2) · [discord.gg/yvgypZ6B](https://discord.gg/yvgypZ6B)
  Community server for discussion, support, and announcements. Two invite links circulate; both lead to the same server.

- **Twitter / X** — [@antigma_labs](https://x.com/antigma_labs)
  Official account for product updates, benchmark results, and blog post announcements.

- **Hugging Face** — [huggingface.co/Antigma](https://huggingface.co/Antigma)
  Organization page hosting models, datasets, and interactive spaces (e.g., Abliteration demo).

- **Contact** — [contact@antigma.ai](mailto:contact@antigma.ai)

---

## GitHub Repositories

- **Organization** — [github.com/AntigmaLabs](https://github.com/AntigmaLabs)

- **`mcp-sdk`** (64 ⭐) — [github.com/AntigmaLabs/mcp-sdk](https://github.com/AntigmaLabs/mcp-sdk)
  Minimalistic Rust implementation of the Model Context Protocol (MCP). Provides the protocol layer Ante uses to communicate with LLM providers and tool servers.
  - Crates.io package: [crates.io/crates/mcp-sdk](https://crates.io/crates/mcp-sdk)

- **`nanochat-rs`** (62 ⭐) — [github.com/AntigmaLabs/nanochat-rs](https://github.com/AntigmaLabs/nanochat-rs)
  Tiny cognitive core built with native Rust, inspired by [nanochat](https://github.com/karpathy/nanochat) from Andrej Karpathy. Demonstrates local inference without Python dependencies, using the HuggingFace Candle tensor library.
  - Model weights: [huggingface.co/Antigma/nanochat-d32](https://huggingface.co/Antigma/nanochat-d32)

---

## Blog Posts

- **"How to Achieve #1 on Terminal Bench"** — Mar 13, 2026 · 13 min read
  [antigma.ai/blog/2026/03/13/terminal-bench](https://antigma.ai/blog/2026/03/13/terminal-bench)
  Forensic analysis of benchmark manipulation on Terminal Bench 2.0. Exposes how rankings can be gamed, and discusses the implications for evaluating coding agents honestly.

- **"Abliteration: Declaration of Independence from Excessive Model Restriction"** — Jul 1, 2025 · 5 min read
  [antigma.ai/blog/2025/07/01/Abliteration](https://antigma.ai/blog/2025/07/01/Abliteration)
  Describes a technique for relaxing overly aggressive moderation guardrails in open-source LLMs without full fine-tuning. Builds on research from the `remove-refusals-with-transformers` project.
  - Interactive demo: [huggingface.co/spaces/Antigma/Abliteration](https://huggingface.co/spaces/Antigma/Abliteration)

- **"Neural Cellular Automata (NCA) — Interactive Demo"** — Jun 29, 2025 · 13 min read
  [antigma.ai/blog/2025/06/29/NCA](https://antigma.ai/blog/2025/06/29/NCA)
  Exploration of self-organizing neural systems with a browser-based interactive demo.

- **"Sovereign Compute and Network State"** — Jan 22, 2025 · 4 min read
  [antigma.ai/blog/2025/01/22/network-state/vision](https://antigma.ai/blog/2025/01/22/network-state/vision)
  Articulates a vision for privacy, trust, and the "right to bear arms" in AI — arguing that access to powerful models is a fundamental right, not a privilege to be gatekept.

- **"The Crypto Way"** — Aug 3, 2024 · 2 min read
  [antigma.ai/blog/2024/08/03/crypto/the_crypto_way](https://antigma.ai/blog/2024/08/03/crypto/the_crypto_way)
  Short essay on cryptographic principles as a philosophical foundation for decentralized AI infrastructure.

- **"Antigma Manifesto"** — Jul 17, 2024 · 2 min read
  [antigma.ai/blog/2024/07/17/manifesto](https://antigma.ai/blog/2024/07/17/manifesto)
  Founding mission statement for Antigma Labs. Outlines the three pillars of the company and the origin of the name (anti-Enigma → Antigma, a nod to Turing and Bletchley Park).

---

## Benchmarks

- **Terminal-Bench 2.0** — Rank **#17** (Ante + Gemini 3 Pro, score 69.4%)
  Second-generation terminal agent benchmark measuring real-world coding task completion.

- **Terminal-Bench 1.0** — Rank **#4** (Ante + claude-sonnet-4-5, score 60.3%)
  First-generation benchmark; Ante's strong placement here preceded the team's investigation into benchmark integrity, which led to the "How to Achieve #1 on Terminal Bench" blog post.

- **Terminal-Bench website** — [terminal-bench.com](https://terminal-bench.com)
  Leaderboard and methodology details for the Terminal Bench benchmark series.

---

## Related Technologies

- **Model Context Protocol (MCP)** — [github.com/modelcontextprotocol](https://github.com/modelcontextprotocol)
  Open protocol for connecting LLMs with external tools and data sources. Ante's `mcp-sdk` crate is a Rust implementation of this specification.

- **HuggingFace Candle** — [github.com/huggingface/candle](https://github.com/huggingface/candle)
  Minimalist ML tensor library written in Rust. Used by `nanochat-rs` for local inference without a Python runtime.

- **Andrej Karpathy's nanochat** — [github.com/karpathy/nanochat](https://github.com/karpathy/nanochat)
  The original Python-based tiny chat model that inspired Antigma's Rust port, `nanochat-rs`.

- **remove-refusals-with-transformers** — [github.com/Sumandora/remove-refusals-with-transformers](https://github.com/Sumandora/remove-refusals-with-transformers)
  Research tool for selectively removing refusal behaviors from transformer models. Served as the basis for Antigma's abliteration technique and blog post.

---

## Key Influences and Context

- **Alan Turing / Bletchley Park** — The Antigma name derives from "anti-Enigma," referencing the codebreaking effort at Bletchley Park during WWII. Symbolizes the mission to break open black-box AI systems.

- **"The Sovereign Individual"** by James Dale Davidson & Lord William Rees-Mogg (frequently cited alongside Peter Thiel) — Referenced in the Antigma Manifesto as a philosophical anchor for the belief that individuals should control their own computational resources and AI capabilities.

- **Balaji Srinivasan's Network State** — Concept of cloud-first, digitally-native communities that influence Antigma's "Sovereign Compute and Network State" vision for decentralized AI infrastructure.

- **Chris Dixon's "Read, Write, Own" framework** — Articulates the progression from read-only web (Web1) to read-write (Web2) to read-write-own (Web3). Antigma draws on this framing to argue that users should own their AI tooling, not merely rent access to it.