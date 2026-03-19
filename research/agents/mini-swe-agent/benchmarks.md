# Benchmarks

> mini-SWE-agent scores >74% on SWE-bench Verified — competitive with agents that have orders of magnitude more code.

## SWE-bench Verified (500 instances)

SWE-bench Verified is the primary benchmark for coding agents. It consists of 500 human-verified instances from real Python repositories, requiring agents to understand codebases, reproduce issues, implement fixes, and verify solutions.

mini-SWE-agent serves as the **official bash-only baseline** on the SWE-bench leaderboard. The "bash-only" track isolates LM capability from agent scaffold sophistication — all entries use the same minimal agent, differing only in the LM.

### Key Results

| Model | % Resolved | Notes |
|-------|-----------|-------|
| **Gemini 3 Pro** | **>74%** | Highest mini-SWE-agent score (as of late 2025) |
| Claude 4.5 Opus (high reasoning) | ~72% | With extended thinking |
| GPT-5 + Sonnet 4 (roulette) | ~66.6% | Random model switching beats either alone |
| GPT-5 | ~63% | |
| Claude Sonnet 4 | ~63% | |
| Gemini 2.5 Pro | ~60% | |
| GPT-5 mini | ~58% | |
| Claude Sonnet 3.5 | ~55% | |
| GPT-5 nano | ~40% | |

*Note: Exact numbers vary by configuration (step limit, cost limit, mini-SWE-agent version). Values are approximate based on published results and blog posts.*

### Context: Comparison with Full Agent Systems

For reference, the top entries on SWE-bench Verified (using complex agent scaffolds) score ~75-80%. mini-SWE-agent's >74% with Gemini 3 Pro means the gap between "100 lines of Python + bash" and "sophisticated multi-tool agents" is **less than 6 percentage points**.

## The Roulette Experiment

In an innovative experiment, the mini-SWE-agent team randomly switched between GPT-5 and Sonnet 4 at each step. The result was surprising:

| Configuration | % Resolved (500 instances) |
|--------------|---------------------------|
| GPT-5 + Sonnet 4 (roulette) | **66.6%** |
| GPT-5 alone | ~63% |
| Sonnet 4 alone | ~63% |

**Random model switching outperformed either model alone.** The hypothesis: different models have complementary failure modes, so randomly switching between them provides implicit ensemble-like diversity.

### Small-Scale Roulette Results (50 instances)

| Models | Score |
|--------|-------|
| GPT-5 + Sonnet 4 | 39 / 50 |
| GPT-5 + Sonnet 4 + Gemini 2.5 Pro | 33 / 50 |
| GPT-5 + Gemini 2.5 Pro | 31 / 50 |
| GPT-5 + GPT-5 mini | 31 / 50 |
| GPT-5 mini + GPT-5 nano | 20 / 50 |

Baselines for comparison:

| Model | Score |
|-------|-------|
| Sonnet 4 | 33 / 50 |
| GPT-5 | 32 / 50 |
| GPT-5 mini | 32 / 50 |
| Gemini 2.5 Pro | 29 / 50 |
| GPT-5 nano | 16 / 50 |

The effect is most pronounced when combining models of similar capability (GPT-5 + Sonnet 4), and diminishes when combining models of very different strengths.

### Cost Analysis

The roulette approach costs approximately **$0.30 per instance** at maximum performance — roughly in the middle of the two component models' costs. Performance gains become marginal around a 50-step limit.

## SWE-bench Full (2294 instances)

mini-SWE-agent scores are reported primarily on the Verified subset. The full SWE-bench (2294 instances) includes harder, unverified instances where all agents perform lower.

## SWE-bench Multilingual (300 instances)

SWE-bench Multilingual includes tasks across 9 programming languages. mini-SWE-agent's bash-only approach is inherently language-agnostic — the same agent works for Python, JavaScript, Java, C++, Go, Rust, etc., since it just executes shell commands.

## Performance vs Cost Curves

The mini-SWE-agent team has published performance-vs-step-limit curves showing:

1. **Most tasks resolve within 20-30 steps** — diminishing returns beyond that
2. **Performance plateaus around step 50** — additional steps rarely help
3. **Cost scales linearly** — no hidden costs from complex scaffold operations
4. **GPT-5 reaches plateau faster than Sonnet 4** — suggesting different exploration strategies

## What mini-SWE-agent's Scores Mean

### For Model Evaluation

Because mini-SWE-agent is so minimal, its SWE-bench scores are a **near-direct measure of LM capability for software engineering**. There's minimal scaffold interference — the score primarily reflects:
- The model's ability to understand code
- Its skill at writing bash commands
- Its debugging and problem-solving capability
- Its ability to follow instructions

This is why mini-SWE-agent was chosen as the bash-only baseline on the official leaderboard.

### For Agent Design

The fact that ~100 lines of Python + bash can score >74% suggests:
- **Scaffold complexity has sharply diminishing returns** for frontier models
- **Investment in models pays off more** than investment in frameworks
- **Simple baselines should always be established** before building complexity

### For Research

mini-SWE-agent enables clean research comparisons:
- **Model A vs Model B** with identical scaffolding
- **Prompt A vs Prompt B** with identical execution
- **RL training** on simple, clean trajectories
- **Fine-tuning** with trajectory == training data identity

## Historical Performance Trajectory

The bash-only approach has improved dramatically with model improvements:

| Time Period | Best Bash-Only Score | Model |
|-------------|---------------------|-------|
| Early 2024 | ~15-20% | GPT-4 |
| Mid 2024 | ~30-35% | Claude 3.5 Sonnet |
| Late 2024 | ~45-50% | GPT-4o, Claude 3.5 |
| Early 2025 | ~55-60% | Claude Sonnet 4, GPT-5 |
| Mid 2025 | ~65% | GPT-5, Sonnet 4 |
| Late 2025 | **>74%** | Gemini 3 Pro |

This trajectory itself is evidence for the "invest in models, not scaffolds" thesis — the same ~100 lines of agent code went from ~20% to >74% purely through model improvements.
