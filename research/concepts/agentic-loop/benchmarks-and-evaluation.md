# Benchmarks and Evaluation

## Overview

Benchmarks drive agent development — what gets measured gets optimized. The coding agent
benchmark landscape has evolved rapidly since 2023, growing from simple function-completion
tasks (HumanEval) to full end-to-end issue-resolution harnesses (SWE-bench) that exercise
every layer of an agentic loop: issue comprehension, codebase navigation, multi-file editing,
tool orchestration, and verification.

Different benchmarks test fundamentally different aspects of the loop. A benchmark that
evaluates single-function generation (HumanEval) tells you about model quality; one that
evaluates full GitHub-issue resolution (SWE-bench) tells you about agent architecture. How
you benchmark shapes which loop components you invest in: if your target metric is SWE-bench
Verified, you will build verification loops and file-search tools; if your target is
Terminal-Bench, you will invest in shell error recovery and state tracking.

The most important meta-insight from the benchmark landscape is that simple loops can
compete with complex ones when the underlying model is strong enough. mini-SWE-agent — 100
lines of Python — scores 65 % on SWE-bench Verified. This does not mean loop design is
irrelevant; it means that the marginal value of loop complexity must be evaluated against the
marginal value of a better model.

---

## SWE-bench — The Gold Standard

SWE-bench (Software Engineering Benchmark) was introduced by Jimenez et al. in late 2023 and
quickly became the most-cited evaluation for coding agents. It converts real GitHub issues
and their corresponding pull-request fixes into a reproducible test harness.

### Variants

| Variant | Tasks | Description |
|---------|-------|-------------|
| **SWE-bench Full** | 2,294 | Real GitHub issues from 12 Python repositories |
| **SWE-bench Verified** | 500 | Human-vetted subset; the most-cited leaderboard |
| **SWE-bench Lite** | 300 | Curated subset for cheaper evaluation runs |
| **SWE-bench Multilingual** | 300 | Tasks across 9 programming languages (Java, JS, Go, Rust, …) |
| **SWE-bench Multimodal** | 517 | Issues containing visual elements — screenshots, UI diagrams, plots |

**SWE-bench Full** draws from repositories like Django, Flask, Scikit-learn, Sympy,
Matplotlib, Astropy, Requests, Sphinx, Pylint, Pytest, xarray, and Seaborn. Each task
consists of:

1. A natural-language issue description (often including stack traces, reproduction steps).
2. A repository snapshot checked out to a specific commit.
3. A test suite that the agent's patch must pass (including both the new regression test
   from the PR and all pre-existing tests).

**SWE-bench Verified** was created because the original full set contains ambiguous or
under-specified issues. Human annotators reviewed each task to confirm it is solvable given
only the information in the issue description and the repository state. This makes Verified
the fairest comparison point across agents.

**SWE-bench Multilingual** extends evaluation beyond Python. This matters because many loop
components — file search heuristics, test-runner invocations, language-specific linting —
must generalize across ecosystems. An agent that hard-codes `pytest` as its test runner
will fail on Java tasks.

**SWE-bench Multimodal** adds an important dimension: many real issues include screenshots
("the button renders wrong"), plots ("the axis labels overlap"), or diagrams. Agents that
can only process text will miss these. This variant pushes loop design toward vision-capable
models or image-description pre-processing steps.

### What It Tests

SWE-bench exercises the **full agentic loop**:

1. **Issue comprehension** — parse a natural-language description, sometimes with stack
   traces, code snippets, or references to specific files.
2. **Codebase navigation** — find the relevant files in a repository with hundreds or
   thousands of files. Agents must decide what to search for, read, and ignore.
3. **Patch generation** — produce a diff that fixes the issue without breaking existing
   functionality.
4. **Verification** — the generated patch is applied and the full test suite is run. Only
   patches that pass *all* tests (existing + the PR's new test) count as resolved.

This end-to-end nature is what makes SWE-bench the gold standard. A model that can generate
perfect isolated functions (HumanEval 100 %) may still fail SWE-bench because it cannot
*find* the right file to edit, or because its edit breaks an import in a downstream module.

### Scoring

- **% Resolved**: the percentage of instances where the agent's patch passes all tests.
- Current state-of-the-art on **SWE-bench Verified**: approximately **74 %** (mid-2025).
- On the **Full** set, scores are lower (typically 30–50 %) because it includes ambiguous
  or extremely difficult tasks.
- Cost matters: some agents spend $2–5 per task; at 500 tasks, a single evaluation run on
  Verified can cost $1,000–$2,500 in API calls.

### How SWE-bench Drives Loop Design

SWE-bench has directly shaped the tool sets and loop architectures of every major coding
agent:

**Minimum tool set.** To solve SWE-bench tasks, an agent needs at minimum:
- `search` (grep, ripgrep, or semantic search across the repo)
- `read` (view file contents, possibly with line ranges)
- `edit` (apply a patch, replace lines, or insert code)
- `bash` (run tests, install dependencies, inspect runtime behavior)

Every major agent (SWE-agent, OpenHands, Aider, Devon, Cursor Agent) implements some variant
of this quartet. SWE-bench made this the de facto standard.

**Long context.** Some issues require understanding 10+ files simultaneously. Agents with
larger context windows or effective summarization score higher on these tasks. This drove
investment in context compaction (dropping older messages, summarizing tool outputs).

**Verification loops.** Because scoring is binary (pass all tests or fail), agents that run
tests after each edit attempt can catch regressions before submitting. The build-test-fix
cycle that many agents implement was directly motivated by SWE-bench scoring.

**Edit format precision.** Malformed patches (wrong line numbers, incorrect indentation,
partial edits) cause test failures even when the logic is correct. This drove SWE-agent's
`edit` command design and Aider's multiple edit-format experiments.

### Key Insight: mini-SWE-agent

In early 2025, researchers demonstrated that a minimal agent — **100 lines of Python** —
could score **65 % on SWE-bench Verified**. This agent, dubbed mini-SWE-agent, uses the
simplest possible loop:

```
while not done:
    response = model.query(context)
    result = execute(response.tool_call)
    context.append(result)
```

No planning phase. No reflection step. No multi-agent delegation. No sophisticated context
management. Just query → execute → append → repeat.

This result is profoundly important for loop design. It suggests that **model quality is the
dominant variable**, and that loop complexity has diminishing returns once you have a strong
model and a functional tool set. It does *not* mean complex loops are useless — they help on
harder tasks, reduce cost on easy tasks, and improve reliability — but it reframes the
question: *how much additional % does each loop component add, and at what cost?*

The "roulette" experiment reinforces this: randomly switching between models mid-task can
actually *improve* aggregate benchmark scores, because different models have complementary
strengths. This suggests that model diversity may matter more than loop sophistication.

---

## Terminal-Bench

Terminal-Bench evaluates agents on CLI and system-administration tasks — a complementary
angle to SWE-bench's focus on code editing.

### Versions

| Version | Tasks | Focus |
|---------|-------|-------|
| **Terminal-Bench 1.0** | ~50 | Initial release; basic CLI tasks |
| **Terminal-Bench 2.0** | 89 | Realistic CLI environments; expanded scope |
| **Terminal-Bench 3.0** | 100+ | Further expanded; multi-step system tasks |

### What It Tests

- **File system navigation**: finding files, parsing directory structures, manipulating paths.
- **System administration**: managing processes, configuring services, editing config files.
- **Multi-step command sequences**: tasks that require 5–20 sequential shell commands, each
  depending on the output of the previous one.
- **Error recovery**: commands fail frequently in terminal environments (wrong flags, missing
  packages, permission issues). Agents must detect failures and retry with corrections.
- **Non-interactive execution**: many CLI tools prompt for input (`y/n`, interactive
  editors). Agents must handle these or invoke tools with non-interactive flags.

### How It Drives Loop Design

Terminal-Bench exposes weaknesses that SWE-bench does not. An agent might be excellent at
editing Python files but terrible at running shell commands reliably. Key design pressures:

**Reliable shell execution.** Agents must correctly invoke commands, capture stdout/stderr,
detect exit codes, and handle timeouts. This pushes loop implementations toward robust
`bash` tool implementations with proper error reporting.

**Error recovery is critical.** In Terminal-Bench, commands fail at a much higher rate than
in typical SWE-bench tasks (wrong flags, missing dependencies, unexpected output formats).
Agents with explicit retry logic or error-classification steps score significantly higher.

**State tracking across commands.** Unlike SWE-bench (where the codebase is mostly static
between edits), Terminal-Bench tasks modify system state. The agent must track what it has
already done — what files it created, what services it started, what environment variables
it set — to avoid undoing its own progress.

**ForgeCode's progression on Terminal-Bench** illustrates how benchmark results drive loop
improvements:
- Initial score: ~25 % (basic agent loop)
- After adding verification enforcement: ~30 %
- After tool-call correction (auto-fixing malformed commands): ~35 %
- After non-interactive mode optimization: ~38 %

Each improvement was directly motivated by failure analysis on Terminal-Bench tasks.

### Notable Results

| Agent | Terminal-Bench 2.0 Score | Key Feature |
|-------|--------------------------|-------------|
| TongAgents | 80.2 % | Multi-agent collaboration |
| ForgeCode (latest) | ~38 % | Verification + tool correction |
| Baseline GPT-4 | ~20 % | No agentic loop |

The gap between TongAgents (80.2 %) and a bare model (20 %) demonstrates that **loop
design matters enormously for terminal tasks**, even more so than for code editing tasks.
Multi-agent architectures — where one agent plans and another executes — show particular
strength here.

---

## HumanEval and MBPP

These older benchmarks predate the agentic era but remain important as baselines for model
capability.

### HumanEval

- **164** Python function-completion problems.
- Each problem includes a function signature, docstring, and a set of unit tests.
- The agent (or model) must generate the function body.
- Metric: **pass@k** — the probability that at least one of k generated samples passes all
  tests. Typically reported as pass@1 (single attempt).
- Current frontier models score 95–99 % on pass@1.

HumanEval does **not** test the agentic loop. There is no codebase to navigate, no issue to
parse, no multi-file context. It tests raw code generation quality.

### MBPP (Mostly Basic Programming Problems)

- **974** Python programming problems drawn from crowd-sourced descriptions.
- Broader coverage than HumanEval but individually simpler.
- Same pass@k metric.
- Current frontier models score 90–95 %.

### Why These Matter for Agents

HumanEval and MBPP measure the **model quality component** of agent performance. An agent's
ability to solve SWE-bench tasks decomposes (roughly) into:

```
agent_performance ≈ f(model_quality, loop_design, tool_quality)
```

HumanEval/MBPP measure `model_quality` in isolation. A model that scores 60 % on HumanEval
will produce worse patches in an agentic loop than a model scoring 95 %, all else equal.
But the relationship is not linear — loop design and tool quality create multiplicative
effects. A mediocre model in an excellent loop can outperform an excellent model with no
loop (this is exactly what SWE-bench demonstrates).

**The orthogonality insight**: model quality and loop quality are largely independent axes.
You can improve either one. The benchmark ecosystem helps you isolate which axis to invest
in:
- Low HumanEval score + low SWE-bench score → improve the model first.
- High HumanEval score + low SWE-bench score → improve the loop / tools.
- High HumanEval score + high SWE-bench score → optimize cost, latency, UX.

---

## AgentBench

### Overview

AgentBench (Liu et al., 2023, Tsinghua University) was the first systematic benchmark to
evaluate LLMs as agents across multiple interactive environments. It tests whether a model
can *act* in the world, not just generate text.

### Environments

| # | Environment | Description |
|---|-------------|-------------|
| 1 | **Operating System** | Execute shell commands, navigate file systems, manage processes |
| 2 | **Database** | Write SQL queries, manage schemas, interpret query results |
| 3 | **Knowledge Graph** | Navigate entity relationships, answer multi-hop questions |
| 4 | **Web Browsing** | Interact with web pages, fill forms, click links |
| 5 | **Lateral Thinking** | Solve creative reasoning puzzles via interaction |
| 6 | **House Holding** | Navigate simulated household environments (ALFWorld) |
| 7 | **Digital Card Game** | Play card games requiring strategy |
| 8 | **Web Shopping** | Find and purchase items on simulated e-commerce sites |

### Relevance for Coding Agents

The OS and Database environments directly test capabilities that coding agents need:
- Executing shell commands reliably (same as Terminal-Bench).
- Writing and debugging SQL (relevant for data-engineering agents).
- Managing file systems and processes.

The Web Browsing environment is relevant for agents that need to read documentation, search
Stack Overflow, or interact with web-based tools (CI dashboards, issue trackers).

AgentBench's key contribution is demonstrating that agent capability varies enormously
across environments. A model might excel at OS tasks but fail at web browsing. This implies
that **loop design should be environment-aware** — the same loop structure may not work
equally well for terminal tasks and web tasks.

### Results Summary

In the original paper, GPT-4 significantly outperformed all other models across
environments. Open-source models (LLaMA, Vicuna) struggled, particularly on tasks requiring
multi-step reasoning. This was an early signal that **model scale matters for agentic
tasks**, and that fine-tuning on agent trajectories could help close the gap.

---

## CodeClash (Newer)

CodeClash was announced in late 2025 by members of the SWE-bench team and represents a
philosophical shift in agent evaluation.

- **Goal-oriented, not task-oriented**: instead of "fix this issue," CodeClash defines
  higher-level goals ("improve the test coverage of this module," "refactor this subsystem
  to use the new API") and evaluates whether the agent makes meaningful progress.
- **Open-ended evaluation**: there is no single correct patch. Multiple approaches may be
  valid. Evaluation uses a combination of automated checks and human review.
- **More realistic**: real development work is rarely as well-specified as a GitHub issue
  with a failing test. CodeClash aims to test agent behavior under ambiguity.
- **Website**: https://codeclash.ai

CodeClash is still maturing, but it signals where the benchmark community is heading:
toward evaluations that test judgment, planning, and iterative refinement — the higher-order
capabilities that separate good agents from great ones.

---

## Aider's Code Editing Benchmark

### Structure

Aider (an open-source AI pair-programming tool) maintains its own benchmark specifically
designed to test the **edit-apply-verify** loop:

1. **133 Exercism Python exercises** serve as the task set.
2. **Two-attempt evaluation**:
   - **Attempt 1**: Send the exercise description + function stub to the LLM. Apply the
     response as an edit. Run tests.
   - **If tests fail**: Send the first 50 lines of test errors back to the LLM with a
     "fix it" instruction. Apply the fix. Run tests again.
   - **Attempt 2 result**: Record pass/fail.
3. This directly tests the loop's ability to **generate an edit**, **apply it correctly**
   (parse the edit format, produce valid code), and **recover from errors**.

### How It Drives Design

Aider's benchmark has been instrumental in developing and comparing **edit formats** — the
syntax an LLM uses to express code changes:

| Edit Format | Description | Strengths |
|-------------|-------------|-----------|
| **whole** | Return entire file content | Simple; no parsing errors |
| **diff** | Unified diff format | Compact; familiar |
| **udiff** | Modified unified diff | Better line-number handling |
| **search/replace** | Search for old code, replace with new | Precise; works well with large files |
| **editor-diff** | Structured diff with editor commands | Reduced ambiguity |
| **editor-whole** | Structured whole-file with editor commands | Combines precision and simplicity |

Different models perform best with different edit formats. For example, Claude models tend
to perform well with whole-file and search/replace formats, while GPT models may prefer
diff-based formats. This is not a minor detail — **the wrong edit format can cost 10–20 %
on the benchmark**.

### Leaderboard Insights

- **Architect mode** (two-model approach: one model reasons about the change, another
  generates the edit) often outperforms single-model approaches. This validates the
  "planner + coder" pattern used by several agent architectures.
- The benchmark has directly led to Aider's innovations in edit format design, including
  the search/replace format that has been adopted by other tools.
- Cost per task varies dramatically: from $0.01 (small fast models) to $0.50+ (large
  frontier models with whole-file output).

---

## SWE-smith

SWE-smith (released May 2025) is not a benchmark per se but a **training data generation
pipeline** for software engineering agents. It is included here because it represents a
crucial link between benchmarks and agent improvement:

- **Synthetic trajectory generation**: Run existing agents on SWE-bench tasks, collect
  successful trajectories, and use them as training data.
- **Fine-tuning pipeline**: Take an open-source model (e.g., LLaMA, CodeLlama) and
  fine-tune it on these trajectories to produce a specialized coding-agent model.
- **Bootstrapping**: Fine-tuned models can generate new trajectories, which can be used to
  fine-tune further (iterative self-improvement).

SWE-smith demonstrates that benchmark performance can be directly translated into training
signal. This creates a feedback loop:

```
benchmark → evaluate agent → collect trajectories → fine-tune model → better agent → benchmark
```

This is the mechanism by which benchmarks *literally* shape model capabilities, not just
measure them.

---

## How Benchmarks Drive Loop Design Decisions

### 1. Tool Set Design

Benchmarks define the **minimum viable tool set** for coding agents:

- **SWE-bench requires**: file search, file read, file edit, bash execution.
- **Terminal-Bench requires**: reliable shell execution with error detection.
- **AgentBench requires**: environment-specific interaction protocols.

This convergence is why virtually every coding agent implements the same four core tools
(search, read, edit, bash) — because that is what you need to score well on SWE-bench.
Agents that add more tools (web browsing, image analysis, LSP integration) do so to capture
additional benchmark points or to target benchmarks beyond SWE-bench.

### 2. Verification Integration

SWE-bench's binary scoring (all tests pass or fail) creates a strong incentive to integrate
verification into the loop:

```
edit → run tests → if fail: analyze errors → edit again → run tests → …
```

Agents without verification loops leave points on the table. Many initial edits have minor
errors (off-by-one in line numbers, missing imports, wrong indentation) that a single test
run would catch. The verification loop converts these near-misses into successes.

ForgeCode's verification enforcement was directly motivated by Terminal-Bench analysis: they
found that 30 % of failures could be recovered by running a verification step and retrying.

### 3. Context Management

Long SWE-bench tasks (complex Django issues, multi-file Scikit-learn refactors) require
the agent to hold information from 10–20 files simultaneously. This drives investment in:

- **Context compaction**: summarizing older messages to free token budget.
- **Sliding windows**: keeping only the most recent N messages.
- **Hierarchical context**: maintaining a "scratchpad" of key facts separate from the full
  conversation history.

But short tasks (60 % of SWE-bench) do not need any of this. mini-SWE-agent wins without
compaction because most tasks fit comfortably within a large model's context window. The
lesson: context management is only valuable for the long tail, but the long tail is where
the hardest (and most differentiating) tasks live.

### 4. Error Recovery

Both SWE-bench and Terminal-Bench involve frequent tool failures:
- Bash commands return non-zero exit codes.
- File edits fail to apply (wrong line numbers, file not found).
- Tests fail with unexpected errors.
- Models generate malformed tool calls.

Agents with better error recovery consistently score higher. Key patterns:
- **Retry with correction**: detect the error, send it back to the model, let it try again.
- **Stuck detection**: if the agent has made N attempts without progress, reset or escalate
  (OpenHands implements this explicitly, motivated by benchmark evaluation).
- **Tool-call validation**: catch malformed tool calls before execution and ask the model
  to reformat (ForgeCode's tool-call correction).

### 5. Simple vs Complex Loops

The most provocative benchmark result is mini-SWE-agent's 65 % on SWE-bench Verified with
100 lines of code. Combined with:
- The "roulette" experiment (random model switching improves aggregate scores).
- The observation that model quality correlates more strongly with SWE-bench scores than
  loop complexity does.
- The diminishing returns of adding loop features (each feature adds 1–3 % but increases
  cost and latency).

These results suggest a design principle: **start with the simplest loop that works, add
complexity only when benchmarks prove it helps, and always compare against the simple
baseline.**

---

## Benchmark Comparison Table

| Benchmark | Focus | Tasks | Languages | Tests Agentic Loop? | Key Metric | Cost per Run |
|-----------|-------|-------|-----------|---------------------|------------|-------------|
| SWE-bench Verified | GitHub issue resolution | 500 | Python | Full loop | % Resolved | $1,000–2,500 |
| SWE-bench Multilingual | Cross-language issues | 300 | 9 languages | Full loop | % Resolved | $600–1,500 |
| SWE-bench Multimodal | Visual issues | 517 | Python | Full loop | % Resolved | $1,000–2,500 |
| Terminal-Bench 2.0 | CLI tasks | 89 | Shell | Terminal agents | % Success | $50–200 |
| HumanEval | Function completion | 164 | Python | Model only | pass@k | $1–10 |
| MBPP | Basic programming | 974 | Python | Model only | pass@k | $5–50 |
| AgentBench | Multi-environment | 8 envs | Various | Partial | Composite | $200–500 |
| CodeClash | Goal-oriented dev | TBD | Various | Full loop | TBD | TBD |
| Aider Bench | Code editing | 133 | Python | Edit loop | % Correct | $5–50 |
| SWE-smith | Training data gen | N/A | Python | N/A (training) | Model quality | GPU hours |

---

## The Benchmark Gap

### What Benchmarks Miss

Current benchmarks evaluate a narrow slice of real-world agent usage:

**Long-running tasks.** Real development tasks take hours or days, not the 2–10 minutes
that benchmark tasks typically require. No benchmark tests whether an agent can maintain
coherence over a multi-hour session, manage growing context, or resume after interruption.

**Multi-session work.** Developers pick up where they left off. They context-switch between
tasks. They come back the next day and continue. No benchmark tests session persistence,
task handoff, or progressive understanding of a codebase over time.

**Real-world ambiguity.** SWE-bench tasks are derived from actual GitHub issues, but they
have been filtered and curated. The messiest real-world scenarios — contradictory
requirements, underspecified behavior, missing documentation — are underrepresented.

**User interaction quality.** How well does the agent communicate its progress? Does it ask
clarifying questions at the right time? Does it explain its changes? These soft skills
matter enormously in practice but are not measured by any current benchmark.

**Cost efficiency.** Most benchmarks report only accuracy. In production, cost per task
matters. An agent that scores 70 % at $0.50 per task may be more valuable than one that
scores 75 % at $5.00 per task. Only Aider's benchmark systematically tracks cost.

**Safety.** Does the agent execute dangerous commands? Does it overwrite files it should
not? Does it leak secrets? These concerns are paramount in production but absent from
benchmarks.

### Goodhart's Law in Practice

"When a measure becomes a target, it ceases to be a good measure."

Several Goodhart's Law dynamics are visible in the coding-agent benchmark landscape:

1. **Training data contamination**: SWE-bench tasks are drawn from public GitHub repos.
   Models trained on internet data may have seen the exact solutions. SWE-bench Verified
   attempts to mitigate this with human vetting, but the concern persists.

2. **Benchmark-specific optimization**: agents can be tuned for SWE-bench patterns (Django
   issues have certain characteristics, Scikit-learn issues have others) in ways that do not
   generalize to other repos or languages.

3. **Metric gaming**: maximizing % Resolved on a fixed set of 500 tasks can lead to
   overfitting — an agent that handles those 500 patterns well but fails on the 501st.

4. **Leaderboard culture**: the race to top the SWE-bench leaderboard can incentivize
   benchmark-specific tricks over genuine capability improvements.

The antidote is **multi-benchmark evaluation**: test on SWE-bench *and* Terminal-Bench *and*
Aider's benchmark *and* internal real-world tasks. An agent that scores well across all four
is genuinely capable; one that tops SWE-bench but fails elsewhere has likely overfit.

---

## Best Practices for Evaluation

1. **Use multiple benchmarks.** SWE-bench alone is insufficient. Combine it with
   Terminal-Bench, Aider's benchmark, and ideally your own internal task set drawn from your
   actual codebase and workflows.

2. **Track cost per task, not just accuracy.** Report both % Resolved and $/task. An agent
   that costs 10x more for 5 % higher accuracy may not be the right choice for production.

3. **Measure latency and user experience.** Time-to-first-token, total task duration, and
   the number of user interactions required all matter. A benchmark that ignores these
   produces agents optimized for accuracy at the expense of usability.

4. **Test on real tasks, not just benchmarks.** Maintain an internal eval set of tasks from
   your own repos. These tasks will have the ambiguity, scale, and context that public
   benchmarks lack.

5. **Compare against simple baselines.** Always benchmark against the simplest possible
   agent (mini-SWE-agent style). If your complex loop does not meaningfully beat the simple
   one, the complexity is not paying for itself.

6. **Version your benchmark results.** Models improve. Benchmarks evolve. Record the exact
   model version, benchmark version, date, and cost for every evaluation run. Without this
   metadata, historical comparisons are meaningless.

7. **Separate model quality from loop quality.** Run HumanEval/MBPP alongside SWE-bench.
   If a model scores high on HumanEval but low on SWE-bench, the bottleneck is the loop.
   If both are low, improve the model first.

8. **Evaluate failure modes, not just success rates.** A 70 % score means 30 % failure.
   Categorize those failures: wrong file? correct file but bad edit? good edit but test
   timeout? Each failure category points to a different loop improvement.

---

## References

- Jimenez, C. E., et al. "SWE-bench: Can Language Models Resolve Real-World GitHub Issues?" (2023)
- Liu, X., et al. "AgentBench: Evaluating LLMs as Agents" (2023)
- Chen, M., et al. "Evaluating Large Language Models Trained on Code" (HumanEval, 2021)
- Austin, J., et al. "Program Synthesis with Large Language Models" (MBPP, 2021)
- Aider code editing benchmark: https://aider.chat/docs/leaderboards/
- SWE-bench leaderboard: https://www.swebench.com
- SWE-smith: https://swesmith.com
- CodeClash: https://codeclash.ai
- Terminal-Bench: https://terminalbench.com
- mini-SWE-agent analysis: Princeton NLP blog