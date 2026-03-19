# Gemini CLI — Benchmarks

> Terminal-Bench 2.0 results, model comparison, and performance analysis.

## Terminal-Bench 2.0

Terminal-Bench 2.0 is a benchmark designed to evaluate terminal-based coding agents
on real-world software engineering tasks. It measures agents' ability to navigate
codebases, understand context, make correct changes, and verify their work.

### Gemini CLI Results

| Configuration | Rank | Score | Notes |
|---|---|---|---|
| Gemini 3 Flash | #55 | 47.4% | Best Gemini CLI result |
| Gemini 2.5 Pro | #105 | 19.6% | Significantly lower despite "Pro" label |

### Context: Leaderboard Position

```
Terminal-Bench 2.0 Approximate Leaderboard (selected entries)

Rank  Agent + Model               Score
────  ─────────────────────────   ─────
#1    [Top agents]                ~75%+
...
#20   [Strong performers]         ~60%
...
#55   Gemini CLI (3 Flash)        47.4%  <-- Best Gemini result
...
#80   [Mid-tier agents]           ~30%
...
#105  Gemini CLI (2.5 Pro)        19.6%  <-- Surprising underperformance
...
```

## Model Comparison: Gemini 3 Flash vs Gemini 2.5 Pro

### The Surprising Gap

The most notable finding is that Gemini 3 Flash **dramatically outperforms** Gemini 2.5 Pro
on Terminal-Bench 2.0 (47.4% vs 19.6% — a 2.4x difference).

This is counterintuitive because:
- Gemini 2.5 Pro is positioned as the more capable, reasoning-heavy model
- On other benchmarks (MMLU, HumanEval, etc.), 2.5 Pro generally outperforms Flash models
- The "Pro" label implies higher quality across tasks

### Possible Explanations

#### 1. Agent-Model Optimization Fit

Gemini CLI may be specifically optimized for newer Gemini 3 series models:
- System prompts tuned for Gemini 3 instruction following
- Tool calling format optimized for Gemini 3's function calling behavior
- Token caching implementation aligned with Gemini 3 API features
- The agent's agentic patterns may better match Gemini 3's training

#### 2. Speed vs Depth Tradeoff

Terminal-Bench 2.0 may reward speed and breadth over deep reasoning:
- Flash models respond faster, enabling more iterations within time limits
- More turns = more tool calls = more exploration
- 2.5 Pro may "overthink" simple tasks, spending tokens on reasoning
  rather than tool execution
- Flash's faster responses may lead to better real-time adaptation

#### 3. Function Calling Quality

Flash models in the Gemini 3 generation may have improved function calling:
- Better adherence to tool schemas
- More consistent parameter formatting
- Fewer hallucinated tool calls
- Better tool result interpretation

#### 4. Context Window Utilization

The 1M token window may benefit differently per model:
- Flash may be more efficient at utilizing large contexts
- Pro's deeper reasoning may not translate to better tool-based task execution
- The agentic loop's iteration patterns may favor Flash's response characteristics

#### 5. Benchmark-Specific Factors

Terminal-Bench 2.0's task distribution may favor certain capabilities:
- Tasks may be more about correct tool usage than deep reasoning
- File navigation and search tasks favor speed
- Simple code modifications favor quick, correct tool calls
- Pro's advanced reasoning may not be needed for the benchmark's task complexity

## Benchmark Analysis

### What Terminal-Bench 2.0 Measures

Terminal-Bench 2.0 evaluates:
1. **Codebase navigation**: Finding relevant files and understanding project structure
2. **Code comprehension**: Understanding existing code to make correct changes
3. **Code modification**: Making precise, correct edits
4. **Verification**: Running tests and validating changes
5. **Multi-step reasoning**: Chaining multiple operations correctly

### Score Breakdown (Estimated)

Without detailed per-category breakdowns from Terminal-Bench, we can hypothesize
based on the overall scores:

```
Hypothesized Category Performance (Gemini 3 Flash @ 47.4%):

Category              Est. Score    Analysis
─────────────────     ──────────    ────────────────────────────────────
File navigation       ~60%          Strong — glob + grep tools are solid
Code comprehension    ~50%          Moderate — 1M context helps
Code modification     ~40%          Lower — replace tool precision matters
Test verification     ~45%          Moderate — shell command execution
Multi-step reasoning  ~35%          Weakest — requires sustained coherence
```

### Comparison with Competing Agents

While exact rankings vary, approximate positioning:

```
Agent Category          Typical TB 2.0 Range   Gemini CLI Position
──────────────────      ────────────────────    ───────────────────
Top tier (Claude Code)  65-80%                  Well above Gemini
Strong tier             50-65%                  Gemini 3 Flash approaches
Mid tier                35-50%                  Gemini 3 Flash sits here
Lower tier              15-35%                  Gemini 2.5 Pro sits here
Baseline                <15%                    Below Gemini
```

## Performance Implications

### For Users

1. **Use Gemini 3 Flash** over Gemini 2.5 Pro for agentic coding tasks
2. Expect ~50% task completion rate on complex, multi-step coding tasks
3. Best suited for tasks that leverage Gemini CLI's unique strengths:
   - Large codebase navigation (1M context)
   - Tasks requiring web search (Google Search grounding)
   - Multimodal tasks (screenshots, PDFs)
   - Tasks in well-documented codebases (skills + GEMINI.md)

### For Architecture Evaluation

The benchmark results suggest:
- **Agent architecture matters** — the same underlying models perform very differently
  based on how the agent layer orchestrates them
- **Newer models can be better for agents** — Flash models can outperform Pro models
  when the agent is optimized for speed of iteration
- **Context window != performance** — having 1M tokens doesn't automatically mean
  better performance; how the context is utilized matters more

### Benchmark Limitations

Terminal-Bench 2.0 results should be interpreted with caveats:
1. **Single benchmark**: One benchmark cannot capture all real-world scenarios
2. **Task distribution**: May not represent typical user workloads
3. **Version sensitivity**: Results depend on exact agent and model versions
4. **Configuration**: Sandbox, model, and setting choices affect performance
5. **Time constraints**: Benchmark time limits may not reflect real usage patterns

## Performance Optimization Opportunities

Based on the benchmark analysis, potential improvements for Gemini CLI:

### Short-term
- Optimize tool calling patterns for common task types
- Improve replace tool precision (reduce failed replacements)
- Better error recovery in multi-step operations
- Tune system prompts for benchmark-style tasks

### Medium-term
- Implement speculative tool execution (pre-read likely files)
- Improve plan mode to increase first-attempt success rate
- Better conversation management for long multi-step tasks
- Optimize token caching for more scenarios

### Long-term
- Model fine-tuning for agentic coding patterns
- Learned tool selection strategies
- Adaptive prompting based on task complexity
- Cross-model routing (Flash for simple, Pro for complex)

## Historical Context

Gemini CLI is relatively new compared to Claude Code, which has had more time to
optimize its agentic patterns. The benchmark gap may narrow as:
- Google iterates on Gemini CLI's agent architecture
- Newer Gemini models improve function calling capabilities
- Community feedback drives optimization of common patterns
- The skills system matures with better built-in skills

## Raw Data Reference

```
Source: Terminal-Bench 2.0 Leaderboard
Date: 2025 (check leaderboard for exact date)
Agent: Gemini CLI (google-gemini/gemini-cli)

Entry 1:
  Model: Gemini 3 Flash
  Rank: #55
  Score: 47.4%
  
Entry 2:
  Model: Gemini 2.5 Pro
  Rank: #105
  Score: 19.6%
```
