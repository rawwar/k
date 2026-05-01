# Capy — References

> Links to primary sources, blog posts, and relevant resources.

## Primary Sources

| Source | URL | Notes |
|--------|-----|-------|
| Capy website | https://capy.ai | Main product page, pricing, feature overview |
| Capy blog | https://capy.ai/blog | Engineering blog with architecture posts |

## Key Blog Posts

| Title | Date | Relevance |
|-------|------|-----------|
| "Captain vs Build: Why We Split the AI Agent in Two" | 9 Feb 2026 | Core architecture explanation; source for Captain/Build system prompts and design rationale |
| "We Stress-Tested GPT-5.4 Before Launch. Here's What Happened." | 5 Mar 2026 | Pre-launch eval of GPT-5.4 in the Capy harness |
| "Capy Is SOC 2 Type II Certified" | 10 Mar 2026 | Security/compliance announcement |
| "$1,000 in Capy Credits for Every YC Company" | 9 Apr 2026 | YC partnership / distribution |
| "The April Update: Captain Takes the Helm" | 16 Apr 2026 | Build mode deprecated; Captain-only workflow; thread-local task IDs; thread-scoped model selection; Opus 4.7 default Captain; CI-aware Captain |
| "GPT-5.5 Is the First OpenAI Model We'd Actually Run as Captain." | 23 Apr 2026 | Production telemetry from 495 sessions / ~56k model calls comparing GPT-5.5 vs Opus 4.6 vs GPT-5.4 as Captain (tail latency, ambition, review-loop convergence) |

## Benchmark Sources

| Source | URL | Notes |
|--------|-----|-------|
| Terminal-Bench 2.0 leaderboard | — | Capy rank #7, Claude Opus 4.6, 75.3% ±2.4 (2026-03-12) |

## Company Information

- **Company**: Lowercase (Lowercase Labs)
- **Product**: Capy — cloud-based AI coding IDE
- **Pricing**: $20/month Pro (3 seats), custom Enterprise, free for open source
- **Security**: SOC 2 Type II certified (March 2026)
- **User base**: 50,000+ engineers

## Related Research in This Repository

| Agent | Comparison Point |
|-------|-----------------|
| [Droid (Factory)](../droid/) | Also model-agnostic, commercial, cloud-based; uses interface-agnostic approach vs Capy's task-based approach |
| [Claude Code](../claude-code/) | Single-agent architecture with sub-agents; contrast with Captain/Build split |
| [Codex](../codex/) | Also supports parallel sessions; different isolation model |
| [ForgeCode](../forgecode/) | Terminal-native multi-agent (Forge/Muse/Sage); different split from Captain/Build |

## Limitations

Capy is a closed-source commercial product. There is no public GitHub repository or open-source codebase to inspect. Internal architecture details beyond what appears in blog posts and marketing materials are not available. All analysis in this research is based on publicly available information.
