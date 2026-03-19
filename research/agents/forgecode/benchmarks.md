# ForgeCode — Benchmark Results & Analysis

## Terminal-Bench 2.0 Results

Terminal-Bench (TermBench) 2.0 is a realistic evaluation suite where agents receive coding tasks in a sandboxed terminal environment and must complete them autonomously under strict time constraints. It tests codebase navigation, problem decomposition, tool calling, and task completion.

### Current Leaderboard Positions

| Rank | Agent + Model | Completion % |
|------|--------------|--------------|
| #1 | ForgeCode + Claude Opus 4.6 | **81.8%** |
| #1 | ForgeCode + GPT 5.4 | **81.8%** |
| — | ForgeCode + Gemini 3.1 Pro Preview | **78.4%** |
| — | Warp | 61.2% |
| — | Claude Code | 58% |
| — | Open Code | 51.7% |

Source: [tbench.ai leaderboard](https://www.tbench.ai/leaderboard/terminal-bench/2.0) and ForgeCode landing page.

### Model-Vendor Comparison

Google reports Gemini 3.1 Pro Preview scoring 68.5% on TermBench when run natively. ForgeCode ran the same model and scored 78.4% — a **10 percentage point improvement** from the same weights in a better runtime harness.

## The Journey: 25% → 78.4% → 81.8%

ForgeCode's benchmark improvement was documented in two detailed blog posts. The progression reveals how specific failure-mode fixes compound into large gains.

### Phase 1: Baseline — ~25%

The initial run used ForgeCode's interactive-first runtime. The agent asked clarifying questions (no user to answer in a benchmark), hesitated before committing, and used conversational patterns unsuited to autonomous execution.

**Root cause**: The agent was built for interactive use, not autonomous execution.

### Phase 2: Stabilization — ~38%

**Fixes applied**:
- **Non-Interactive Mode**: System prompt rewritten to prohibit clarification and hedging
- **Tool-call naming**: Renamed arguments to training-data-aligned names (e.g., `old_string`/`new_string`)
- **Per-tool micro-evaluations**: Built targeted evals that isolate tool-call failures by class (wrong tool, wrong args, wrong sequence) per model

**Key insight**: Tool misuse was one of the top two failure classes. Models pattern-match tool names against training data priors — alignment with those priors reduces errors.

### Phase 3: Planning Control — 66%

**Fixes applied**:
- **`todo_write` enforcement**: Made task decomposition tracking mandatory, not optional
- **Low-level evals**: Built assertions that check whether `todo_write` is called for multi-step tasks and items are updated as the agent progresses

**Key insight**: Optional tools get deprioritized under pressure. When an agent is deep in a complex problem, it takes the path of least resistance — the next tool call that seems relevant, not the one that maintains planning state. Enforcement, not suggestion, was required.

### Phase 4: Speed Architecture — 78.4% (SOTA with Gemini 3.1 Pro)

**Fixes applied**:
- **Sub-agent parallelization**: Low-complexity work (file reads, pattern searches, routine edits) delegated to sub-agents with minimal thinking budget
- **Progressive thinking policy**: High thinking for first 10 messages (planning), low for messages 11+ (execution), high again for verification
- **Skill routing**: Dynamic skill loading matched to task profiles
- **Semantic entry-point discovery**: Context engine identifies starting files before the agent explores

**Key insight**: Intelligence without speed fails benchmarks. TermBench has strict wall-clock time limits. A brilliant but slow agent times out just as definitively as a wrong one.

### Phase 5: Model-Specific Tuning — 81.8% (SOTA with Opus 4.6 + GPT 5.4)

**Fixes applied**:
- **Schema field ordering**: `required` before `properties` in JSON schemas (reduced GPT 5.4 malformed calls)
- **Schema flattening**: Flat schemas over nested to reduce structural confusion
- **Explicit truncation signals**: Plain-text reminders in tool result bodies for models that don't infer from metadata
- **Enforced verification skill**: Programmatic requirement to run a verification pass before task completion

**Key insight**: The two models fail in different places. Opus is more forgiving (tolerates messy schemas, infers from metadata). GPT 5.4 needs cleaner structure but reaches the same score with model-appropriate runtime tuning. The headline isn't "model X beat model Y" — it's "runtime version N learned how to stop triggering model X's failure modes."

## Improvement Summary

| Phase | Change | Pass Rate |
|-------|--------|-----------|
| Baseline | Interactive-first runtime | ~25% |
| Stabilization | Non-Interactive mode + tool naming + micro-evals | ~38% |
| Planning control | `todo_write` enforcement | 66% |
| Speed architecture | Sub-agent parallelization + progressive thinking + skill routing | 78.4% |
| Model-specific tuning | Schema engineering + verification enforcement | **81.8%** |

## Seven Failure Modes Identified

The blog posts document seven specific failure modes discovered through TermBench:

1. **Interactive behavior in autonomous context** — asking questions when no one is listening
2. **Tool-call misuse** — wrong tool, wrong args, wrong sequencing
3. **Training-data prior conflicts** — tool names/args that conflict with model expectations
4. **Entry-point discovery latency** — exploring the wrong part of the codebase
5. **Time budget exhaustion** — brilliant but meandering trajectories timing out
6. **Missing planning state** — forgetting sub-tasks without explicit tracking
7. **Premature completion** — declaring done before the task is actually complete

Each failure mode was addressed with a targeted fix. No fix was a "general quality improvement" — each was a specific intervention against a specific class.

## Ongoing Evaluation

ForgeCode now runs continuous evals in CI/CD that gate releases:

- **Per-tool reliability scores by model** — different models have different weak tools
- **`todo_write` compliance** for decomposed tasks
- **Entry-point discovery precision**
- **Skill routing accuracy**
- **Recovery rate** after first tool-call error in a trajectory
- **Time-efficiency curves** under tight budgets

These run in minutes and are derived from TermBench failure classes.

## Meta-Analysis: What the Benchmarks Show

The ForgeCode benchmark story demonstrates three things:

1. **Runtime engineering > model capability**: Same model weights, 10+ points higher in a better harness.
2. **Model comparison requires runtime-awareness**: Evaluating models outside an agent runtime doesn't predict agent performance.
3. **Specific fixes beat general improvements**: Each phase targeted a named failure class, not "make the agent better."

As their blog concludes: "Don't run a benchmark to get a number. Run it to find out which part of your system is lying to you in production."