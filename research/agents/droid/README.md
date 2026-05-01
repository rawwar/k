---
title: Droid — Factory.ai's Agent-Native Software Development Platform
status: complete
---

# Droid

> Factory.ai's enterprise coding agent that works across every developer surface — IDE, web, CLI, Slack, Linear, CI/CD — with model-agnostic, vendor-agnostic architecture and industry-leading context compaction.

## Overview

**Droid** is the core AI agent of **Factory**, an agent-native software development platform founded by **Matan Grinberg** (CEO). Factory positions itself not as an AI coding assistant but as a platform for delegating complete engineering tasks — refactors, incident response, migrations, testing, code review — to autonomous agents that work wherever developers already work.

What sets Droid apart from competitors is its **interface-agnostic architecture**. While most coding agents are tied to a single surface (Claude Code to the terminal, Cursor to the IDE, Devin to a web workspace), Droid operates identically across desktop IDE, web workspaces, terminal CLI, Slack, Linear, Microsoft Teams, and CI/CD pipelines. A developer can start a task in Slack, monitor it in the web UI, and review the results as a pull request — all within the same Droid session.

Factory has raised funding with participation from **Wipro Ventures** and serves enterprise customers including Chainguard, Clari, and others across financial services, healthcare, manufacturing, and technology sectors. The platform operates in 100+ programming languages, 100+ frameworks, and communicates in 40+ human languages. Factory has expanded globally with offices in London (opened March 2026) and partners with Wipro to roll out across tens of thousands of engineers.

### What Makes Droid Special

1. **Interface Agnosticism**: Same agent core, deployable across IDE, web, CLI, Slack, Linear, Teams, CI/CD. No other major agent achieves this breadth.

2. **Context Compaction**: Factory's proprietary compaction engine preserves context across sessions lasting days or weeks — a Chainguard engineer ran a single session for two weeks without context degradation.

3. **Model/Vendor Agnostic**: Works with OpenAI, Anthropic, Google, xAI, open-source, and local models. Teams can mix models — cost-efficient ones (GLM-5, Kimi-K2.5) for execution, frontier models (Opus 4.6) for planning.

4. **Enterprise-Native**: SSO/SAML, dedicated compute, audit logging, SIEM export, ISO 42001 compliance, SOC 2 Type I, single-tenant VPC sandboxing, and comprehensive analytics.

5. **Factory Analytics**: Full observability stack (built on OpenTelemetry) tracking tokens, tool usage, activity/adoption, productivity output, per-user breakdowns, and agent readiness — with API access for AI-generated ROI reports.

## Terminal-Bench Scores

| Benchmark | Model Config | Rank | Score |
|-----------|-------------|------|-------|
| Terminal-Bench 2.0 | Droid + GPT-5.3-Codex | #7 | 77.3% |
| Terminal-Bench 2.0 | Droid + Claude Opus 4.6 | #17 | 69.9% |
| Terminal-Bench 2.0 | Droid + GPT-5.2 | #24 | 64.9% |
| Terminal-Bench 2.0 | Droid + Claude Opus 4.5 | #27 | 63.1% |
| Terminal-Bench 2.0 | Droid + Gemini 3 Pro | #33 | 61.1% |
| Terminal-Bench 1.0 | Droid + Claude Opus 4.1 | #5 | 58.8% |

## Key Stats

- **Type**: Closed-source, commercial SaaS platform
- **Enterprise focus**: SSO/SAML, SCIM, dedicated compute, audit logging, compliance
- **Interfaces**: Desktop IDE, Web, CLI, Slack, Linear, Microsoft Teams, CI/CD (non-interactive mode)
- **Model support**: OpenAI, Anthropic, Google, xAI, open-source, local models
- **Languages**: 100+ programming languages, 100+ frameworks, 40+ human languages
- **Integrations**: GitHub, GitLab, Jira, Linear, Notion, Sentry, PagerDuty, Slack, Asana, Teams, Codecov, Google Cloud, Confluence, Google Drive, Azure, Docker, DX, CircleCI
- **Security**: ISO 42001, SOC 2 Type I, AES-256 at rest, TLS 1.2+ in transit, single-tenant VPC

## Architecture at a Glance

```
┌──────────────────────────────────────────────────────────────┐
│                     Developer Surfaces                        │
│  ┌─────┐ ┌─────┐ ┌─────┐ ┌───────┐ ┌────────┐ ┌──────────┐ │
│  │ IDE │ │ Web │ │ CLI │ │ Slack │ │ Linear │ │  CI/CD   │ │
│  └──┬──┘ └──┬──┘ └──┬──┘ └───┬───┘ └───┬────┘ └────┬─────┘ │
└─────┼───────┼───────┼────────┼─────────┼───────────┼────────┘
      │       │       │        │         │           │
      └───────┴───────┴────┬───┴─────────┴───────────┘
                           │
              ┌────────────▼────────────┐
              │     Droid Agent Core     │
              │  ┌────────────────────┐  │
              │  │ Compaction Engine  │  │
              │  │ (anchor points,   │  │
              │  │  incremental)     │  │
              │  └────────────────────┘  │
              │  ┌────────────────────┐  │
              │  │   Model Router     │  │
              │  │ (vendor-agnostic)  │  │
              │  └────────────────────┘  │
              │  ┌────────────────────┐  │
              │  │   Tool System      │  │
              │  │ Read/Edit/Execute/ │  │
              │  │ Grep/Glob/Skills  │  │
              │  └────────────────────┘  │
              └────────────┬────────────┘
                           │
         ┌─────────────────┼─────────────────┐
         │                 │                 │
  ┌──────▼──────┐  ┌──────▼──────┐  ┌──────▼───────┐
  │   Source     │  │  Project    │  │  Incident    │
  │   Code Mgmt │  │  Mgmt       │  │  Mgmt        │
  │  (GitHub,   │  │ (Jira,      │  │ (Sentry,     │
  │   GitLab)   │  │  Linear)    │  │  PagerDuty)  │
  └─────────────┘  └─────────────┘  └──────────────┘
```

## Quick Start

```bash
# Factory offers multiple interfaces:
# 1. CLI: Terminal-based interactive agent
# 2. Web: Cloud workspaces at app.factory.ai
# 3. IDE: Desktop plugin integration
# 4. CI/CD: Non-interactive pipeline mode
```

## Files in This Research

| File | Contents |
|------|----------|
| [architecture.md](architecture.md) | Multi-interface core, model routing, integration layer |
| [agentic-loop.md](agentic-loop.md) | Task delegation, async execution, review loop |
| [tool-system.md](tool-system.md) | Tool categories, skills, slash commands, MCP servers |
| [context-management.md](context-management.md) | Compaction engine, anchor points, multi-week sessions |
| [unique-patterns.md](unique-patterns.md) | Interface agnosticism, enterprise analytics, model routing |
| [benchmarks.md](benchmarks.md) | Terminal-Bench scores, enterprise productivity metrics |
| [references.md](references.md) | Links to docs, blog posts, case studies |

## References

- Website: https://factory.ai
- Docs: https://docs.factory.ai
- Enterprise: https://factory.ai/enterprise
- Security: https://factory.ai/security
- Terminal-Bench 2.0: rank #7 (Droid + GPT-5.3-Codex, 77.3%); rank #17 (Claude Opus 4.6, 69.9%); rank #24 (GPT-5.2, 64.9%); rank #27 (Claude Opus 4.5, 63.1%); rank #33 (Gemini 3 Pro, 61.1%)
- Terminal-Bench 1.0: rank #5 (Claude Opus 4.1, 58.8%)

## Recent Updates (April 2026)

Highlights from the Factory CLI changelog `v0.109.x` → `v0.114.0` (Apr 23–30, 2026):

- **Hybrid ripgrep Grep tool** — Grep now uses a hybrid ripgrep engine for substantially faster search.
- **Security-review skill on by default** — the `security-review` skill is now enabled out of the box.
- **`interval` loop command** — new CLI command for running tasks on a recurring interval, complementing scheduled missions.
- **Auto-fallback to Droid Core** — sessions hitting usage limits automatically fall back to Droid Core when the org's overage preference is set accordingly.
- **Per-tool MCP autonomy overrides** — new `mcpAutonomyOverrides` setting lets you set autonomy levels per individual MCP tool.
- **Fast-mode toggle** — single opt-in toggle to enable fast-mode model variants across the agent.
- **BYOM admin toggle** — org admins can enable/disable bring-your-own-machine computers from enterprise settings.
- **Droid Shield improvements** — push scanning targets only commits not yet on any remote (no duplicate scans); max buffer raised from 20 MB → 64 MB.
- **Per-run automation viewer** and **org-level model access table** added in the web app.
- **Mission entry overage gating** and snapshotted mission model settings keep long-running missions from drifting mid-run.
- **Slack** — multi-turn delegation fix; per-channel auto-run model selection; native loading status indicators.
- **Models** — DeepSeek V4 Pro added; sorted CLI model list by release date (newest first).
- **i18n** — Italian translations added for the CLI.
