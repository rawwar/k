# grok-cli — Benchmarks

## Terminal-Bench 2.0

| Model | Date | Rank | Score |
|---|---|---|---|
| Grok 4.20 Reasoning | 2026-04-02 | **#43** | 57.3% |

Single published row as of 2026-05.

## Context

Rank #43 with 57.3% places grok-cli mid-pack on the TB2 leaderboard,
between Letta Code (Claude Opus 4.5, 59.1%, #36) and Goose (Claude Opus
4.5, 54.3%, #46).

For context on the model itself: every other Grok-family entry on the
leaderboard is via Terminus 2 (the reference scaffold) and scores
considerably lower:

| Agent | Model | Rank | Score |
|---|---|---:|---:|
| **grok-cli** | **Grok 4.20 Reasoning** | **#43** | **57.3%** |
| Terminus 2 | Grok 4 | #106 | 23.1% |
| Terminus 2 | Grok Code Fast 1 | #116 | 14.2% |

So grok-cli's harness adds substantial value over the bare-tmux scaffold
when running Grok models — about a 30-point lift versus the closest
Terminus 2 / Grok comparison (`Grok 4` at 23.1%, though that's an older
model than `Grok 4.20 Reasoning`).

## No Margin Published

The TB2 entry shows no `± margin` for grok-cli — likely fewer benchmark
runs than the deeper-pocketed first-party submissions.

## Other Benchmarks

No other public benchmark scores are listed for grok-cli as of 2026-05.
The README emphasises future "deeper autonomous agent testing" with
"persistent sandbox sessions" and "stronger 'prove it works' evidence",
suggesting more benchmark coverage is on the roadmap.

---

*Numbers from <https://www.tbench.ai/leaderboard/terminal-bench/2.0>
(snapshot 2026-05-01).*
