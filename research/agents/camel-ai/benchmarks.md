# CAMEL-AI — Benchmarks

## Terminal-Bench 2.0

| Model | Date | Rank | Score |
|---|---|---|---|
| Claude Sonnet 4.5 | 2025-12-24 | **#58** | 46.5% ±2.4 |

Single published row.

## Reading the Number

Rank #58 with 46.5% places CAMEL-AI in the lower-middle of the TB2
leaderboard. The model used (**Sonnet 4.5**, not Opus) is notable — it's
a smaller / cheaper Anthropic model. Most agents at comparable
leaderboard positions are running larger models:

| Rank | Agent | Model | Score |
|---:|---|---|---:|
| 56 | Terminus 2 | GPT-5.1 | 47.6% |
| 57 | Gemini CLI | Gemini 3 Flash | 47.4% |
| **58** | **CAMEL-AI** | **Claude Sonnet 4.5** | **46.5%** |
| 60 | OpenHands | GPT-5 | 43.8% |

So the comparison gets more flattering when you account for model
choice: 46.5% on Sonnet 4.5 is roughly on par with what Goose hits with
the same model (43.1%, #64), and substantially above what Terminus 2 +
Sonnet 4.5 manages (42.8%, #65).

## Vs. Reference Scaffold

| Agent | Model | Rank | Score |
|---|---|---:|---:|
| **CAMEL-AI** | **Claude Sonnet 4.5** | **#58** | **46.5%** |
| Terminus 2 | Claude Sonnet 4.5 | #65 | 42.8% |

A ~4-point lift over the bare scaffold. Modest, but real.

## GAIA (via OWL)

The sister project [OWL](https://github.com/camel-ai/owl) — built on
CAMEL — claims **#1 among open-source frameworks on GAIA** with a 69.09
average score. GAIA is a different benchmark (general assistant tasks,
not terminal-specific) but is the most-cited evaluation for CAMEL
ecosystem capability.

CAMEL itself does not directly carry the OWL GAIA result as its own,
but the two co-evolve and OWL is the headline showcase for what the
framework's primitives can compose into.

## Other Benchmarks

The CAMEL paper (arXiv:2303.17760) reported results on its own
role-playing communication tasks; the framework has been used in many
academic publications since for various multi-agent evaluations
(specifics vary by paper). No single broadly-comparable
coding-agent benchmark beyond TB2 has a standalone CAMEL row.

---

*Numbers from <https://www.tbench.ai/leaderboard/terminal-bench/2.0>
(snapshot 2026-05-01) and the OWL README.*
