---
title: Testing and Verification
status: complete
---

# Testing and Verification

Every coding agent generates code that may be wrong. Unlike human developers who build mental models of correctness through years of experience, LLMs produce plausible-looking code without any inherent verification mechanism. Testing and verification is the **primary feedback signal** that transforms unreliable code generation into reliable code editing. This document synthesizes the verification patterns, strategies, and architectural decisions observed across 17 real-world agent implementations.

The impact is stark: without verification loops, agents succeed roughly 30–40% of the time on benchmarks like SWE-bench. With proper **Edit-Apply-Verify** cycles, the same agents reach 60–80%+ pass rates. Verification is not a nice-to-have — it is the single largest lever for agent accuracy.

---

## Why Testing Matters for Coding Agents

Humans verify code through a combination of mental simulation, type intuition, and experience-based pattern matching. An experienced developer "knows" that off-by-one errors lurk in loop boundaries and checks carefully. LLMs have none of this — they produce the most statistically likely next token, which correlates with correctness but does not guarantee it.

### The Failure Modes Without Verification

1. **Silent incorrectness** — Code that parses, compiles, and looks reasonable but produces wrong results. LLMs are particularly prone to this because they optimize for plausibility, not correctness.
2. **Import/dependency drift** — The model references a function, class, or module that doesn't exist in the current codebase. Without a build or import check, this goes undetected until runtime.
3. **Syntax errors in unfamiliar languages** — Models trained predominantly on Python/JS make subtle syntax mistakes in Rust, Go, or Haskell. A syntax check catches these instantly.
4. **Stale context hallucination** — The model "remembers" a function signature from training data that has since changed. Type checking or build verification catches the mismatch.
5. **Partial edits** — The model edits one file but forgets to update a dependent file. Integration tests or build verification reveal the breakage.

### The Quantitative Case

Evidence from multiple sources converges on the same conclusion:

| Condition | SWE-bench Pass Rate (approx.) |
|-----------|-------------------------------|
| Single-shot generation, no verification | 25–35% |
| Generation + lint feedback loop | 40–50% |
| Generation + lint + test feedback loop | 55–70% |
| Multi-agent + verification enforcement | 65–80%+ |

ForgeCode's benchmark results are the most explicit: programmatic verification enforcement was described as the **"single biggest score improvement"** on TermBench. The difference between prompting the model to verify and *forcing* verification through the runtime was the inflection point.

---

## The Edit-Apply-Verify Pattern

The **Edit-Apply-Verify** (E-A-V) pattern is the fundamental loop that underlies every successful coding agent. It is the verification-aware extension of the basic **ReAct** agentic loop.

### The Universal Pseudocode

```
messages = [system_prompt, user_task]

while true:
    response = llm.generate(messages)
    messages.append(response)

    edits = parse_edits(response)
    if edits is empty:
        break                              # model is done

    # --- APPLY ---
    for edit in edits:
        apply_to_file(edit)

    # --- VERIFY ---
    errors = []
    if config.auto_lint:
        lint_result = run_lint()
        if lint_result.failed:
            errors.append(lint_result.output)

    if config.auto_test:
        test_result = run_tests()
        if test_result.failed:
            errors.append(test_result.output)

    if errors:
        error_msg = truncate(join(errors), max_lines=50)
        messages.append(error_message(error_msg))
        continue                           # retry with error context

    commit_changes()
```

This is not a simplification — it directly mirrors implementations across agents. The critical insight is that the **verify** step closes the feedback loop. Without it, the agent is flying blind after every edit.

### Aider: The Gold Standard

Aider's implementation is the most explicit and well-documented E-A-V loop. Its architecture cleanly separates the edit, apply, lint, and test phases:

```python
# Simplified from Aider's core loop
def run_one_round(self, user_message):
    # 1. EDIT: Get LLM response with code changes
    content = self.send_to_llm(messages)

    # 2. APPLY: Parse and apply edits to files
    edits = self.get_edits(content)
    self.apply_edits(edits)

    # 3. VERIFY (lint): Run linter if configured
    if self.auto_lint:
        lint_errors = self.linter.run(self.get_dirty_files())
        if lint_errors:
            self.auto_commit()  # commit what we have
            lint_output = self.linter.format_errors(lint_errors)
            # Feed errors back — bounded retry
            return self.send_to_llm([
                f"Fix these lint errors:\n{lint_output}"
            ])

    # 4. VERIFY (test): Run tests if configured
    if self.auto_test:
        test_result = self.commands.cmd_test(self.test_cmd)
        if test_result.exit_code != 0:
            self.auto_commit()
            test_output = truncate(test_result.output, 50)
            return self.send_to_llm([
                f"Fix these test failures:\n{test_output}"
            ])

    # 5. COMMIT: All checks passed
    self.auto_commit()
```

Key design decisions in Aider's implementation:

1. **Lint runs before tests** — Lint errors are fast to detect and usually fast to fix. Running lint first avoids wasting time on test execution when there are obvious syntax/style issues.
2. **Auto-commit between retry rounds** — Each attempt is committed separately, creating a clean git history and enabling rollback.
3. **Output truncation to 50 lines** — Aider deliberately truncates error output. More context actually *hurts* — the model gets confused by long tracebacks and tries to fix symptoms rather than root causes.
4. **Bounded retries** — On benchmarks, Aider uses 2 attempts maximum. Diminishing returns set in quickly.

### Junie CLI: First-Class Verification Phase

Junie CLI elevates verification from an optional step to a **mandatory phase** in its execution pipeline:

```
Understand → Plan → Implement → Verify → Diagnose
```

Verification is not something the user must invoke — it is an integral part of every task. If verification fails, the agent enters a **Diagnose** phase that analyzes failures with a potentially different (more capable) model before retrying implementation.

---

## The Verification Hierarchy

Not all verification is equal. Faster checks should run first — they catch cheap errors before expensive verification runs. This forms a natural hierarchy:

```
                          ┌─────────────────────┐
                          │  Level 7: Self-     │
                          │  Review (re-read    │  Seconds
                          │  code, diff review) │
                        ┌─┴─────────────────────┴─┐
                        │  Level 6: Integration    │
                        │  Tests                   │  Minutes
                      ┌─┴─────────────────────────┴─┐
                      │  Level 5: Unit Tests         │
                      │                              │  Seconds–Minutes
                    ┌─┴─────────────────────────────┴─┐
                    │  Level 4: Build (compile,        │
                    │  bundle, link)                    │  Seconds–Minutes
                  ┌─┴─────────────────────────────────┴─┐
                  │  Level 3: Type Check (tsc, mypy,     │
                  │  pyright)                             │  Seconds
                ┌─┴─────────────────────────────────────┴─┐
                │  Level 2: Lint (ESLint, Ruff, pylint)    │
                │                                          │  Seconds
              ┌─┴─────────────────────────────────────────┴─┐
              │  Level 1: Syntax Check (parse, tree-sitter)  │
              │                                              │  Milliseconds
              └──────────────────────────────────────────────┘

              FAST / CHEAP ──────────────────────► SLOW / EXPENSIVE
```

### Level 1: Syntax Checking

The cheapest verification. Tree-sitter can parse a file in single-digit milliseconds and immediately detect malformed code. Several agents use tree-sitter for pre-verification before even attempting to run external tools:

- **Aider** uses tree-sitter to validate that edits produce syntactically valid code before committing
- **Claude Code** validates edit results through syntax parsing
- **ForgeCode** catches malformed tool calls at the schema level

### Level 2: Linting

Lint catches style violations, unused imports, undefined variables, and common anti-patterns. Agents configure linting through different mechanisms:

| Agent | Lint Configuration | Auto-Lint |
|-------|-------------------|-----------|
| **Aider** | `--lint-cmd` flag per language | Yes (`--auto-lint`) |
| **Claude Code** | Model-driven (agent decides when to lint) | Model's discretion |
| **OpenCode** | User-configured via settings | Optional |
| **Goose** | Extension-provided lint commands | Extension-dependent |
| **Warp** | Active AI error detection in terminal | Yes (automatic) |

### Level 3: Type Checking

Type checkers (tsc, mypy, pyright) catch a class of errors that linters miss — wrong argument types, missing return values, incompatible interfaces. They are particularly valuable for agents because LLMs frequently hallucinate function signatures.

### Level 4: Build Verification

Compilation and bundling catch linking errors, missing dependencies, and module resolution failures. For compiled languages (Rust, Go, Java), the build step is the primary verification mechanism.

### Level 5–6: Unit and Integration Tests

Tests are the highest-signal verification. A passing test suite means the code *works*, not just that it *looks right*. This is the level where most agents focus their verification effort.

### Level 7: Self-Review

Some agents re-read their own output, review diffs, or run a separate "reviewer" pass. This catches logical errors that automated tools miss. Claude Code's three-phase loop includes a verify phase where the model reviews its own changes. Capy's Captain/Build separation creates a natural review boundary.

---

## How Agents Approach Verification

Agents fall into three distinct philosophies for when and how verification runs. The choice has profound implications for reliability.

### Philosophy 1: Model-Driven Verification

The agent's LLM decides what to verify, when to verify, and how to interpret results. No external enforcement exists — the model must choose to run tests.

**Representatives**: **Claude Code**, **Codex**, **Goose**, **Gemini CLI**

```
┌────────────────────────────────────────┐
│              LLM decides:              │
│  "I should run the tests now"          │
│  "Let me check if this compiles"       │
│  "I'll review my changes"              │
└────────────┬───────────────────────────┘
             │
             ▼
    Tool call: run_command("npm test")
             │
             ▼
    Results feed back into conversation
```

**Strengths**: Maximum flexibility. The model can skip verification when it's confident (e.g., trivial typo fix) and run extensive verification when uncertain (e.g., complex refactor). Adapts to context naturally.

**Weaknesses**: Models skip verification ~40% of the time when it's optional. Prompt instructions like "always run tests after editing" are followed inconsistently. Under context pressure, verification is the first thing the model drops.

### Philosophy 2: Framework-Enforced Verification

The runtime programmatically requires verification. The agent cannot mark a task as complete without passing through a verification gate.

**Representatives**: **ForgeCode**, **Junie CLI**, **Droid**

```
┌────────────────────────────────────────┐
│           Runtime enforces:            │
│  edit_file() → auto_lint()             │
│  task_complete() → BLOCKED until       │
│    verification_passed == true         │
└────────────┬───────────────────────────┘
             │
             ▼
    Verification runs automatically
    regardless of model's preference
```

ForgeCode's approach is the most explicit. The runtime intercepts task completion and programmatically inserts a verification step. The model cannot skip it — verification is a **runtime invariant**, not a prompt suggestion. This was the architectural decision that produced the single biggest score improvement on TermBench.

Junie CLI makes verification a first-class phase (Understand → Plan → Implement → **Verify** → Diagnose). The phase boundary is structural — the agent transitions through it as part of its normal execution flow.

**Strengths**: Consistent verification regardless of model quality or context pressure. Catches the ~40% of cases where models would skip verification.

**Weaknesses**: Less flexible — may run unnecessary verification on trivial changes. Adds latency to every edit cycle.

### Philosophy 3: User-Configured Verification

The user specifies exactly which commands to run. The agent executes them mechanically.

**Representatives**: **Aider**, **OpenCode**, **mini-SWE-agent**

```
# User configures at startup:
aider --auto-lint --lint-cmd "ruff check" \
      --auto-test --test-cmd "pytest tests/"
```

```
┌────────────────────────────────────────┐
│         User configures:               │
│  lint_cmd = "ruff check"               │
│  test_cmd = "pytest tests/"            │
│  auto_lint = true                      │
│  auto_test = true                      │
└────────────┬───────────────────────────┘
             │
             ▼
    After every edit:
      1. Run lint_cmd → feed errors back
      2. Run test_cmd → feed errors back
```

**Strengths**: Deterministic — the exact same verification runs every time. User controls cost/speed tradeoff. Works with any language/framework.

**Weaknesses**: Requires user configuration. Wrong test command = wrong feedback. No adaptive verification based on change scope.

### Philosophy Comparison

| Aspect | Model-Driven | Framework-Enforced | User-Configured |
|--------|-------------|-------------------|-----------------|
| **Consistency** | Low (~60% of the time) | High (always runs) | High (always runs) |
| **Flexibility** | High | Low | Medium |
| **Configuration** | None | None | Required |
| **Overhead** | Variable | Fixed per edit | Fixed per edit |
| **Best for** | Capable models, varied tasks | Benchmarks, CI | Known codebases |
| **Agents** | Claude Code, Codex, Goose | ForgeCode, Junie, Droid | Aider, OpenCode |

---

## Enforce, Don't Prompt

This is the single most important insight in agent verification design:

> **Models skip optional verification ~40% of the time. Building verification into the runtime — not the prompt — is the key architectural decision.**

ForgeCode's experience is the clearest evidence. Their agents were prompted to verify. They did — sometimes. Programmatic enforcement made verification happen 100% of the time. The result: the **single biggest score improvement** on TermBench.

Why do models skip verification?

1. **Token economics** — Under context pressure, the model optimizes for brevity. Verification adds tokens. The model "decides" it's done.
2. **Confidence miscalibration** — LLMs are overconfident. A model that generates code rates its own output highly, reducing the perceived need for verification.
3. **Instruction decay** — In long conversations, system prompt instructions lose influence. "Always run tests" in the system prompt is followed less as the conversation grows.
4. **Prompt saturation** — Too many instructions compete for attention. "Verify your work" competes with "be concise", "follow the user's style", "use idiomatic patterns", etc.

The enforcement pattern in pseudocode:

```python
class VerificationEnforcer:
    """Runtime enforcement — model cannot skip verification."""

    def complete_task(self, agent_state):
        # Agent says it's done — but we check first
        if not agent_state.verification_passed:
            # Force a verification round
            lint_result = self.run_lint(agent_state.changed_files)
            test_result = self.run_tests(agent_state.test_command)

            if lint_result.failed or test_result.failed:
                # Feed errors back — agent MUST address them
                agent_state.add_message(
                    f"Verification failed. Fix before completing:\n"
                    f"{lint_result.errors}\n{test_result.errors}"
                )
                return TaskStatus.NEEDS_RETRY

            agent_state.verification_passed = True

        return TaskStatus.COMPLETE
```

The pattern generalizes: any behavior you want the agent to reliably exhibit should be enforced by the runtime, not requested by the prompt. This applies beyond verification — to file backups, git commits, permission checks, and context management.

---

## The Verification Feedback Loop

How error output feeds back into the agent's context is a critical design decision. Too little output and the model can't diagnose the problem. Too much and the model gets confused.

### Truncation Strategies

**Aider's 50-line rule** is the most well-studied approach. Aider truncates all error output to approximately 50 lines before feeding it back to the model. The reasoning:

1. **Long tracebacks contain noise** — A 200-line Python traceback has maybe 5 relevant lines. The model tries to "fix" lines that aren't errors.
2. **Token budget competition** — Error output competes with code context for the limited context window. 200 lines of traceback may push out the actual code being edited.
3. **Focused errors produce focused fixes** — A concise error message like `TypeError: expected str, got int at line 42` is more actionable than the full traceback.

```python
# Aider's truncation approach
def truncate_output(output, max_lines=50):
    lines = output.strip().splitlines()
    if len(lines) <= max_lines:
        return output

    # Keep first and last lines (most informative)
    half = max_lines // 2
    truncated = lines[:half] + [
        f"\n... ({len(lines) - max_lines} lines truncated) ...\n"
    ] + lines[-half:]
    return "\n".join(truncated)
```

### Structured vs. Raw Errors

Some agents structure error output before feeding it back:

| Approach | Agent | Example |
|----------|-------|---------|
| **Raw output** | mini-SWE-agent, Aider | Pass command stdout/stderr directly |
| **Structured extraction** | ForgeCode, Warp | Parse errors into file:line:message format |
| **Classified errors** | OpenHands, Droid | Categorize as syntax/type/runtime/test errors |
| **AI-interpreted** | Warp | Use AI to analyze terminal output for errors |

Warp's approach is unique: its **Active AI Error Detection** monitors terminal output in real-time and uses a model to identify errors, even in non-standard output formats. This catches errors that regex-based parsers miss.

### Error Classification

Some agents classify errors to determine the appropriate response:

```
Syntax error     → Re-edit the file (fast fix)
Type error       → Check function signatures, fix types
Import error     → Check available modules, fix imports
Test failure     → Analyze assertion, understand expected vs actual
Build error      → Check dependencies, configuration
Runtime error    → Add error handling, fix logic
```

This classification helps the agent focus its retry attempt rather than blindly re-generating code.

---

## Bounded Retries

Retrying verification failures improves success rates — but with rapidly diminishing returns. The empirical sweet spot across agents is **2–3 retries**.

### Why Retries Have Diminishing Returns

```
Success Rate by Retry Attempt (approximate):

Attempt 1 (initial):  ████████████████████░░░░░░░░░░  60%
Attempt 2 (+retry 1): █████████████████████████░░░░░  75%
Attempt 3 (+retry 2): ██████████████████████████░░░░  80%
Attempt 4 (+retry 3): ██████████████████████████░░░░  81%
Attempt 5 (+retry 4): ███████████████████████████░░░  82%
                       ──────────────────────────────
                       Diminishing returns after retry 2
```

The first retry catches the majority of fixable errors — typos, missing imports, off-by-one errors. The second retry catches subtler issues. By the third retry, the model is typically stuck in a loop, regenerating the same incorrect approach.

### Agent-Specific Retry Bounds

| Agent | Max Retries | Mechanism | Notes |
|-------|------------|-----------|-------|
| **Aider** | 2 attempts on benchmark | `--map-auto-retries` | Deliberately conservative |
| **ForgeCode** | `max_tool_failure_per_turn` | Configurable per-turn limit | Prevents cascading failures |
| **OpenHands** | 8 retries | StuckDetector | Detects repetitive actions |
| **Codex** | Context-dependent | Auto-compaction at 90% | Retries until context full |
| **Junie CLI** | Phase-based | Diagnose phase on failure | Escalates to stronger model |
| **mini-SWE-agent** | Fixed iteration limit | `max_steps` parameter | Hard stop after N steps |
| **Goose** | Configurable | Extension-dependent | Varies by verification type |

### Stuck Detection

OpenHands implements a **StuckDetector** that identifies when the agent is looping without progress:

```python
# Simplified from OpenHands' stuck detection
class StuckDetector:
    def is_stuck(self, history):
        recent_actions = history[-4:]

        # Same action repeated 3+ times
        if all_identical(recent_actions):
            return True

        # Alternating between two actions (edit/undo cycle)
        if is_oscillating(recent_actions):
            return True

        # Same error message appearing repeatedly
        if same_error_repeated(recent_actions, threshold=3):
            return True

        return False
```

When stuck, agents take different recovery actions:

1. **Stop and report** — mini-SWE-agent, Aider (on benchmark)
2. **Escalate to stronger model** — Junie CLI (Diagnose phase uses Opus)
3. **Compact context and retry** — Codex (auto-compaction)
4. **Delegate to different agent** — ForgeCode (Forge → Sage for research)
5. **Rollback and retry differently** — Codex (undo support)

---

## Rollback and Recovery

When verification fails repeatedly, agents need a way to undo changes and try a different approach. Rollback strategies vary significantly:

### Git-Based Rollback

**Aider** commits after every successful edit, creating natural rollback points:

```
commit 1: Initial edit attempt
commit 2: Lint fix
commit 3: Test fix (PASSES) ← keep this
--- or ---
commit 1: Initial edit attempt
commit 2: Lint fix
commit 3: Test fix (STILL FAILS) ← rollback to before commit 1
```

### Checkpoint-Based Rollback

**Codex** maintains `GhostSnapshot` items that survive context compaction, enabling undo:

```rust
// Codex's undo mechanism
Op::Undo { n_turns } => {
    // Revert conversation state to N turns ago
    // GhostSnapshots preserve enough state for redo
    context_manager.undo(n_turns);
}
```

**Claude Code** implements checkpoints that capture both conversation state and file state, allowing rewind to any previous point.

### Runtime State Tracking

**ForgeCode** tracks changed files and can revert to the state before the current turn:

```python
# Revert all changes from current turn
for file_path, original_content in turn_state.file_snapshots.items():
    write_file(file_path, original_content)
```

---

## Self-Review Patterns

Beyond automated verification, some agents implement self-review — using the LLM itself to evaluate its own output.

### Diff Review

The agent re-reads the diff of its changes and evaluates them:

```
Agent generates edit → Apply → Generate diff →
Feed diff back to agent: "Review this diff for correctness" →
Agent identifies issues → Fix → Re-verify
```

**Claude Code**'s three-phase loop (gather → act → verify) includes this as the verify phase. The model reviews its own changes before considering the task complete.

### Separate Reviewer Model

Some architectures use a different model for review:

- **Junie CLI** routes verification to potentially different models — Flash for quick checks, Opus for deep analysis
- **Aider's architect mode** separates the reasoning model (designs solution) from the editing model (implements it), creating a natural review boundary
- **ForgeCode's Sage agent** provides read-only deep analysis that acts as a review step

### Self-Review Limitations

Self-review has a fundamental limitation: the same model that made the mistake is asked to find the mistake. Research shows that LLMs are better at catching *other* models' errors than their own. This is why multi-agent approaches (where a different agent reviews) tend to outperform single-agent self-review.

---

## CI/CD Integration

Several agents integrate with continuous integration systems, either consuming CI results or triggering CI pipelines:

| Agent | CI/CD Integration | Mechanism |
|-------|------------------|-----------|
| **Droid** | GitHub Actions repair | Monitors CI failures, auto-fixes |
| **Codex** | Sandbox-isolated builds | Runs build in bubblewrap sandbox |
| **Claude Code** | User-configured commands | Model runs test/build commands |
| **Goose** | Extension-provided | CI integration via extensions |
| **Warp** | Terminal-native | Monitors build output in PTY |
| **OpenHands** | Docker-isolated | Full test suite in container |

**Droid** has the most sophisticated CI integration — it monitors GitHub Actions workflows, detects failures, analyzes logs, and automatically generates fixes. This closes the loop between CI and the coding agent.

---

## Cross-Agent Verification Summary

| Agent | Auto-Test | Auto-Lint | Type Check | Build Verify | Self-Review | Rollback | CI/CD |
|-------|-----------|-----------|------------|-------------|-------------|----------|-------|
| **ForgeCode** | Enforced | Enforced | Via build | Enforced | Sage agent | Turn-level | — |
| **Claude Code** | Model-driven | Model-driven | Model-driven | Model-driven | 3-phase loop | Checkpoints | User commands |
| **Codex** | Model-driven | Model-driven | Model-driven | Sandbox | — | Undo/redo | Sandbox builds |
| **Aider** | `--auto-test` | `--auto-lint` | Via lint cmd | Via test cmd | Architect mode | Git commits | — |
| **Junie CLI** | Verify phase | Verify phase | Via build | Via build | Diagnose phase | — | — |
| **OpenHands** | Agent-driven | Agent-driven | Agent-driven | Docker | StuckDetector | Event stream | Docker |
| **Droid** | Enforced | Enforced | Via build | Via build | Spec review | — | GitHub Actions |
| **Goose** | Extension | Extension | Extension | Extension | — | — | Extension |
| **Gemini CLI** | Model-driven | Model-driven | Model-driven | Model-driven | — | — | User commands |
| **OpenCode** | User config | User config | User config | User config | — | — | — |
| **Warp** | AI detection | AI detection | AI detection | AI detection | — | — | Terminal |
| **mini-SWE-agent** | Agent-driven | — | — | — | — | — | — |
| **Pi** | Extension | Extension | Extension | Extension | — | — | — |
| **Ante** | Sub-agent | Sub-agent | Sub-agent | Sub-agent | Meta-agent | — | — |
| **Sage Agent** | Pipeline | Pipeline | — | — | Observation | — | — |
| **TongAgents** | Multi-agent | — | — | — | Cross-agent | — | — |
| **Capy** | Build phase | Build phase | Build phase | Build phase | Captain review | — | — |

---

## Key Takeaways

1. **Enforce, don't prompt** — The single most impactful design decision. Runtime enforcement beats prompt instructions every time. ForgeCode proved this empirically.

2. **Fast checks first** — The verification hierarchy is a cost optimization. Syntax checks in milliseconds catch errors that would waste minutes of test execution time.

3. **Truncate aggressively** — Aider's 50-line rule is counterintuitive but well-supported. More error output hurts more than it helps. Models need focused, actionable errors.

4. **2–3 retries maximum** — Diminishing returns are sharp. If the agent hasn't fixed it in 3 attempts, it needs a different approach, not another retry.

5. **Git is your safety net** — Agents that commit after every successful step (Aider) have trivial rollback. Agents without rollback mechanisms are fragile.

6. **Multi-model verification beats self-review** — Using a different model (or a different agent role) for review catches errors that self-review misses.

7. **User configuration scales poorly** — Requiring users to specify lint/test commands works for power users but fails for onboarding. The trend is toward auto-detection and enforcement.

---

## Topic Index

| File | Description |
|------|-------------|
| [`README.md`](README.md) | This file — overview of testing and verification patterns |
| [`agent-comparison.md`](agent-comparison.md) | Detailed cross-agent comparison of verification strategies |
| [`build-verification.md`](build-verification.md) | Build and compilation verification patterns |
| [`ci-cd-integration.md`](ci-cd-integration.md) | CI/CD pipeline integration and automation |
| [`lint-integration.md`](lint-integration.md) | Linter integration across languages and agents |
| [`rollback-strategies.md`](rollback-strategies.md) | Rollback, undo, and recovery mechanisms |
| [`self-review.md`](self-review.md) | Self-review and AI-assisted code review patterns |
| [`test-driven-development.md`](test-driven-development.md) | Test-driven workflows and test generation |
| [`type-checking.md`](type-checking.md) | Type checking integration (tsc, mypy, pyright) |

---

## Real-World Implementations

| Agent | Verification Approach | Reference |
|-------|----------------------|-----------|
| **ForgeCode** | Programmatic enforcement, Sage review agent, biggest benchmark impact | [`../agents/forgecode/testing-and-verification.md`](../agents/forgecode/testing-and-verification.md) |
| **Claude Code** | Model-driven 3-phase loop (gather → act → verify), checkpoints | [`../agents/claude-code/testing-and-verification.md`](../agents/claude-code/testing-and-verification.md) |
| **Codex** | Sandbox-isolated builds, undo/redo, model-driven verification | [`../agents/codex/testing-and-verification.md`](../agents/codex/testing-and-verification.md) |
| **Aider** | Gold-standard E-A-V loop, --auto-lint, --auto-test, 50-line truncation | [`../agents/aider/testing-and-verification.md`](../agents/aider/testing-and-verification.md) |
| **OpenHands** | Docker-isolated tests, StuckDetector, event-stream rollback | [`../agents/openhands/testing-and-verification.md`](../agents/openhands/testing-and-verification.md) |
| **Junie CLI** | First-class Verify phase, multi-model Diagnose escalation | [`../agents/junie-cli/testing-and-verification.md`](../agents/junie-cli/testing-and-verification.md) |
| **Droid** | CI/CD repair, GitHub Actions monitoring, enforced verification | [`../agents/droid/testing-and-verification.md`](../agents/droid/testing-and-verification.md) |
| **Goose** | Extension-based verification, MOIM context injection | [`../agents/goose/testing-and-verification.md`](../agents/goose/testing-and-verification.md) |
| **Gemini CLI** | Model-driven with tool scheduler, parallel read-only checks | [`../agents/gemini-cli/testing-and-verification.md`](../agents/gemini-cli/testing-and-verification.md) |
| **OpenCode** | User-configured lint/test commands, settings-driven | [`../agents/opencode/testing-and-verification.md`](../agents/opencode/testing-and-verification.md) |
| **Warp** | Active AI error detection, PTY-native terminal monitoring | [`../agents/warp/testing-and-verification.md`](../agents/warp/testing-and-verification.md) |
| **mini-SWE-agent** | Minimal — agent-driven bash commands, fixed iteration limit | [`../agents/mini-swe-agent/testing-and-verification.md`](../agents/mini-swe-agent/testing-and-verification.md) |
| **Pi** | Extension-based lint/test hooks on tool events | [`../agents/pi-coding-agent/testing-and-verification.md`](../agents/pi-coding-agent/testing-and-verification.md) |
| **Ante** | Multi-agent fan-out/fan-in, sub-agent verification | [`../agents/ante/testing-and-verification.md`](../agents/ante/testing-and-verification.md) |
| **Sage Agent** | Linear pipeline with Observation feedback loop | [`../agents/sage-agent/testing-and-verification.md`](../agents/sage-agent/testing-and-verification.md) |
| **TongAgents** | Multi-agent cross-verification, benchmark-driven | [`../agents/tongagents/testing-and-verification.md`](../agents/tongagents/testing-and-verification.md) |
| **Capy** | Captain/Build phase separation, Ubuntu VM isolation | [`../agents/capy/testing-and-verification.md`](../agents/capy/testing-and-verification.md) |