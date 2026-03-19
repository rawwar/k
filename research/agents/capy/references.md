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
| "Captain vs Build: Why We Split the AI Agent in Two" | Feb 2026 | Core architecture explanation; source for Captain/Build system prompts and design rationale |

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
