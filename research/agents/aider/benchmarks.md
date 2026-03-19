# Aider — The Benchmark Leaderboard

## Overview

Aider maintains the most comprehensive public benchmark for **LLM code-editing performance**. Unlike general coding benchmarks (HumanEval, SWE-bench), Aider's benchmark tests the full end-to-end pipeline: the LLM must understand the task, write correct code, **and** format its output so the tool can apply edits to files.

The leaderboard at [aider.chat/docs/leaderboards/](https://aider.chat/docs/leaderboards/) is updated regularly and has become a de facto reference for model providers and developers alike.

## Methodology

### Exercise Set

The benchmark uses **225 coding exercises** from [Exercism](https://exercism.org/):
- Originally 133 Python exercises from `exercism/python`
- Later expanded to include polyglot exercises

Each exercise includes:
1. **Natural language instructions** (markdown)
2. **Stub code** with function/class skeletons to implement
3. **Unit tests** that must pass

### The Protocol

For each exercise:

1. **First attempt**: Aider sends the LLM:
   - The exercise instructions
   - The stub implementation file
   - System prompt with edit format instructions
   - The instruction: "Use the above instructions to modify the supplied files"

2. **Apply edits**: Parse the LLM's response and apply edits to the file

3. **Run tests**: Execute the unit test suite

4. **Second attempt** (if tests fail): Aider sends:
   - The first 50 lines of test error output
   - The instruction: "The tests are correct. Fix the code to resolve the errors."

5. **Apply and re-test**: Parse, apply, and test again

6. **Score**: Exercise passes if all unit tests pass after either attempt

### What's Being Measured

The benchmark measures a **compound skill**:
- **Code reasoning** — understanding the problem and writing correct logic
- **Edit formatting** — producing output that conforms to the edit format specification
- **Error recovery** — diagnosing test failures and producing correct fixes

A model can fail at any of these steps. Some write correct code but mangle the edit format. Others format perfectly but write buggy code. The benchmark captures both failure modes.

### Key Design Decisions

- **Two attempts**: Models get one try, plus one fix attempt with test output. This mirrors real usage (write code, debug based on errors).
- **No test source access**: The LLM never sees the unit test source code — only error output from failures. (Though the tests were likely in training data.)
- **Temperature 0**: All runs use temperature=0 for reproducibility.
- **Cost tracking**: Each run's total API cost is recorded and reported.
- **Edit format variation**: Each model is tested with its optimal edit format (and sometimes multiple formats).

## The Leaderboard (July 2025)

### Top Tier (>75%)

| Model | Score | Cost | Edit Format |
|-------|-------|------|-------------|
| GPT-5 (high) | 88.0% | $29.08 | diff |
| GPT-5 (medium) | 86.7% | $17.69 | diff |
| o3-pro (high) | 84.9% | $146.32 | diff |
| Gemini 2.5 Pro Preview 06-05 (32k think) | 83.1% | $49.88 | diff-fenced |
| GPT-5 (low) | 81.3% | $10.37 | diff |
| o3 (high) | 81.3% | $21.23 | diff |
| Grok-4 (high) | 79.6% | $59.62 | diff |
| Gemini 2.5 Pro Preview 06-05 (default think) | 79.1% | $45.60 | diff-fenced |
| o3 (high) + GPT-4.1 (architect) | 78.2% | $17.55 | architect |
| o3 | 76.9% | $13.75 | diff |
| Gemini 2.5 Pro Preview 05-06 | 76.9% | $37.41 | diff-fenced |

### Strong Tier (60-75%)

| Model | Score | Cost | Edit Format |
|-------|-------|------|-------------|
| DeepSeek-V3.2-Exp (Reasoner) | 74.2% | $1.30 | diff |
| Gemini 2.5 Pro Preview 03-25 | 72.9% | — | diff-fenced |
| Claude Opus 4 (32k thinking) | 72.0% | $65.75 | diff |
| o4-mini (high) | 72.0% | $19.64 | diff |
| DeepSeek R1 (0528) | 71.4% | $4.80 | diff |
| Claude Opus 4 (no think) | 70.7% | $68.63 | diff |
| DeepSeek-V3.2-Exp (Chat) | 70.2% | $0.88 | diff |
| Claude 3.7 Sonnet (32k thinking) | 64.9% | $36.83 | diff |
| DeepSeek R1 + Sonnet (architect) | 64.0% | $13.29 | architect |
| o1 (high) | 61.7% | $186.50 | diff |
| Claude Sonnet 4 (32k thinking) | 61.3% | $26.58 | diff |
| Claude 3.7 Sonnet (no thinking) | 60.4% | $17.72 | diff |
| o3-mini (high) | 60.4% | $18.16 | diff |

### Mid Tier (45-60%)

| Model | Score | Cost | Edit Format |
|-------|-------|------|-------------|
| Qwen3 235B (no think) | 59.6% | — | diff |
| Kimi K2 | 59.1% | $1.24 | diff |
| DeepSeek R1 | 56.9% | $5.42 | diff |
| Claude Sonnet 4 (no thinking) | 56.4% | $15.82 | diff |
| Gemini 2.5 Flash (24k think) | 55.1% | $8.56 | diff |
| DeepSeek V3 (0324) | 55.1% | $1.12 | diff |
| Quasar Alpha | 54.7% | — | diff |
| o3-mini (medium) | 53.8% | $8.86 | diff |
| Grok 3 Beta | 53.3% | $11.03 | diff |
| Optimus Alpha | 52.9% | — | diff |
| GPT-4.1 | 52.4% | $9.86 | diff |
| Claude 3.5 Sonnet (Oct 2024) | 51.6% | $14.41 | diff |
| Grok 3 Mini Beta (high) | 49.3% | $0.73 | whole |
| DeepSeek Chat V3 (prev) | 48.4% | $0.34 | diff |
| Gemini 2.5 Flash (default) | 47.1% | $1.85 | diff |
| ChatGPT-4o-latest | 45.3% | $19.74 | diff |

### Lower Tier (<45%)

| Model | Score | Cost | Edit Format |
|-------|-------|------|-------------|
| GPT-4.5 Preview | 44.9% | $183.18 | diff |
| Gemini 2.5 Flash (no think) | 44.0% | $1.14 | diff |
| GPT-oss-120b (high) | 41.8% | $0.74 | diff |
| Qwen3 32B | 40.0% | $0.76 | diff |
| Gemini exp-1206 | 38.2% | — | whole |
| Gemini 2.0 Pro | 35.6% | — | whole |
| Grok 3 Mini Beta (low) | 34.7% | $0.79 | whole |
| o1-mini | 32.9% | $18.58 | whole |
| GPT-4.1 Mini | 32.4% | $1.99 | diff |
| Claude 3.5 Haiku | 28.0% | $6.06 | diff |
| GPT-4o (2024-08-06) | 23.1% | $7.03 | diff |
| GPT-4o-mini | 3.6% | $0.32 | whole |

## Key Observations

### 1. Cost vs. Performance Frontier

The most interesting dimension is **cost-effectiveness**:

| Model | Score | Cost | $/% point |
|-------|-------|------|-----------|
| DeepSeek-V3.2-Exp (Chat) | 70.2% | $0.88 | $0.013 |
| DeepSeek V3 (0324) | 55.1% | $1.12 | $0.020 |
| GPT-5 (low) | 81.3% | $10.37 | $0.128 |
| o3-pro (high) | 84.9% | $146.32 | $1.723 |

DeepSeek V3.2 at 70.2% for $0.88 is extraordinarily cost-effective — roughly **170× cheaper per benchmark point** than o3-pro.

### 2. Edit Format Matters Enormously

The same model can score very differently depending on the edit format:
- Models that default to `diff` generally score higher than with `whole` (for capable models)
- Gemini models specifically need `diff-fenced` — they underperform with standard `diff`
- `whole` format is safest for weaker models but wasteful for strong ones

### 3. Thinking/Reasoning Tokens Help

Models with extended thinking consistently outperform their non-thinking variants:
- Claude 3.7 Sonnet: 60.4% (no think) → 64.9% (32k think)
- Claude Opus 4: 70.7% (no think) → 72.0% (32k think)
- Gemini 2.5 Pro: 79.1% (default) → 83.1% (32k think)

### 4. Architect Mode Boosts Weaker Models

Models that struggle with code editing alone can benefit from architect mode:
- o3 solo: 76.9% → o3 + GPT-4.1 architect: 78.2%
- The benefit is most pronounced for reasoning-focused models

### 5. The "Correct Format" Rate

The leaderboard also reports what percentage of exercises had edits that could be **correctly parsed and applied** (regardless of whether the code was correct):

| Model | Score | Correct Format |
|-------|-------|---------------|
| GPT-5 (high) | 88.0% | 91.6% |
| o3-pro (high) | 84.9% | 97.8% |
| Gemini 2.5 Pro (32k) | 83.1% | 99.6% |
| DeepSeek V3 (0324) | 55.1% | 99.6% |

Some models (Gemini, DeepSeek) achieve near-perfect format compliance but score lower on actual coding. Others (GPT-5) have slightly lower format compliance but higher coding ability.

## Historical Context

The benchmark has evolved significantly:

1. **June 2023**: First benchmark — 133 Exercism Python exercises, testing GPT-3.5 vs GPT-4 with whole/diff/function-call formats
2. **Key finding**: Function-call formats performed worse than plain text (surprised everyone)
3. **Late 2023**: Added unified diff format to combat GPT-4 Turbo laziness
4. **Sep 2024**: Architect mode introduced, achieving SOTA with o1-preview + editor pairs
5. **2025**: Expanded to 225 exercises, became the primary industry leaderboard
6. **July 2025**: GPT-5 sets new record at 88.0%

## Comparison to Other Benchmarks

| Benchmark | What it tests | Aider's advantage |
|-----------|--------------|-------------------|
| HumanEval | Standalone function generation | Aider tests editing existing code |
| SWE-bench | Real GitHub issue resolution | Aider is lighter-weight, more reproducible |
| MBPP | Simple coding problems | Aider tests format compliance too |
| LiveCodeBench | Competitive programming | Aider tests practical code editing |

Aider's benchmark is uniquely positioned because it tests the **practical workflow**: can the model edit files correctly through a tool interface? This is exactly what matters for AI coding assistants.

## Running the Benchmark

The benchmark code is open source in the Aider repository:

```bash
# Clone and setup
git clone https://github.com/Aider-AI/aider
cd aider/benchmark

# Run against a specific model
python benchmark.py --model gpt-4o --edit-format diff
```

Results are deterministic at temperature=0, though OpenAI's API has some non-determinism (~5-10 response variants for identical requests, likely due to load balancing across model instances).