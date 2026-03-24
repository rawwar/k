---
title: DeerFlow Benchmarks and Reception
status: complete
---

# DeerFlow Benchmarks and Reception

## Traditional Benchmarks

DeerFlow is **not evaluated on traditional coding benchmarks** (SWE-bench, Terminal-Bench, HumanEval, MBPP) because:

1. It is not primarily a coding agent — it is a general-purpose super agent harness
2. Its performance is model-dependent: it acts as an orchestration layer over any configured LLM
3. Its value is in multi-step, long-horizon tasks (research, synthesis, content generation) not captured by coding benchmarks

**For coding tasks specifically**, DeerFlow's performance is bounded by the underlying model and the quality of the relevant skill specifications. ByteDance recommends Doubao-Seed-2.0-Code for coding tasks.

---

## Community Reception (v2 Launch, February 2026)

| Metric | Value |
|--------|-------|
| GitHub Trending rank | **#1 globally** (Feb 28, 2026) |
| Trendshift ranking | **#1** repositories chart |
| Launch catalyst | v2 release (ground-up rewrite from v1) |

DeerFlow's reception is notable because GitHub Trending #1 is typically achieved by viral developer tools, not AI agent harnesses. The launch day ranking reflects significant pent-up demand from the v1 user community, which had been extending the framework beyond its intended scope.

---

## v1 → v2 Trajectory

| Version | Architecture | Scope | Status |
|---------|-------------|-------|--------|
| v1 (1.x branch) | Monolithic deep-research framework | Deep web research + report generation | Maintained (legacy) |
| v2 (main branch) | Super agent harness on LangGraph | General-purpose: research, coding, slides, dashboards, pipelines | Active development |

The jump from v1 to v2 was a **complete rewrite with no shared code** — an unusual decision that signals how fundamentally the use case expanded. ByteDance documented the rationale explicitly: community developers had pushed v1 into territory it was never designed for, and v2 was designed to formalize what users were already doing.

---

## Recommended Models (ByteDance-Promoted)

DeerFlow is model-agnostic but ByteDance highlights specific models for optimal performance:

| Model | Provider | Best For |
|-------|----------|----------|
| Doubao-Seed-2.0-Code | ByteDance Volcengine | Coding tasks (via Coding Plan) |
| DeepSeek v3.2 | DeepSeek | General-purpose reasoning |
| Kimi 2.5 | Moonshot | Long-context tasks |

Note: these are promotional recommendations from ByteDance's ecosystem partners. Any OpenAI-compatible endpoint works.

---

## Comparison Position in This Research

Within the agent taxonomy used in this research:

```
CLI Coding Agents (Terminal-Bench focus)
    ForgeCode (81.8%) ← #1 Terminal-Bench 2.0
    Claude Code (58%)
    Codex CLI
    ...

General Coding + Research Agents
    OpenHands (event-driven, broader task scope)
    DeerFlow ← here (research-first, coding capable)

Deep Research + Synthesis Agents
    DeerFlow v1 (retired) ← narrow research scope
```

DeerFlow v2 sits between general-purpose coding agents and dedicated research tools — the "super agent harness" framing is accurate in that it intentionally spans both categories.

---

## Qualitative Signals

- Community extended v1 beyond its design scope (data pipelines, dashboards, slide decks) — indicating strong fit-to-need
- ByteDance committed to a ground-up rewrite rather than patching v1 — indicating organizational investment
- #1 GitHub Trending on launch day — indicating broad developer awareness
- Active contributions from external contributors (not just ByteDance employees) — indicating healthy open-source ecosystem
