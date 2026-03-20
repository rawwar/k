# When to Use Agents

> A decision framework for choosing the right agent design pattern

## Overview

Anthropic's "Building Effective Agents" blog identifies seven design patterns ranging from the
simple augmented LLM to the fully autonomous agent. The critical question practitioners face
is not *how* to implement these patterns but *when* each pattern is appropriate. Choosing the
wrong pattern leads to predictable failures: over-engineering simple tasks wastes tokens and
latency, while under-engineering complex tasks produces unreliable results.

This document provides a structured decision framework built from analyzing how 17 CLI coding
agents make these tradeoffs in practice. The framework evaluates six dimensions—complexity,
autonomy, error tolerance, latency, cost, and reliability—and maps them to concrete pattern
recommendations.

The key insight from studying real-world agents: **most production agents combine multiple
patterns rather than implementing a single one**. The decision framework helps you choose the
*primary* pattern and then layer additional patterns as needed.

---

## Task Complexity Assessment

Before selecting a pattern, you need to assess the task along three axes that together define
its complexity profile.

### Single-Step vs Multi-Step Tasks

The most fundamental distinction. A single-step task can be completed with one LLM call plus
tool use—fix a typo, answer a question about code, generate a single function. A multi-step
task requires planning, sequencing, and state management across multiple operations.

**Single-step indicators:**
- The entire task can be described in one sentence
- Output is a single artifact (one file edit, one response)
- No intermediate validation is needed
- The LLM doesn't need to observe its own output to proceed

**Multi-step indicators:**
- Task requires "first X, then Y, then Z" reasoning
- Multiple files or systems need coordinated changes
- Intermediate results need validation before proceeding
- The task description includes words like "refactor," "implement," or "migrate"

Real-world example: mini-SWE-agent handles multi-step tasks with a single augmented LLM
pattern by giving the model bash access and letting it iterate. This works because the model
can observe bash output and self-correct. The pattern is technically autonomous agent, but the
scaffold is so minimal (~100 lines) that it blurs the line with augmented LLM.

### Predictable vs Unpredictable Subtasks

For multi-step tasks, the next question is whether you know the subtasks in advance.

**Predictable subtasks:**
- Code review: always parse → analyze → comment → summarize
- Test generation: always read code → identify cases → write tests → verify
- Junie CLI's pipeline: understand → plan → implement → verify → iterate → present
- Sage Agent's pipeline: TaskAnalysis → Planning → Executor → Observation → TaskSummary

**Unpredictable subtasks:**
- "Debug this failing test" — could be a typo, a logic error, a dependency issue
- "Implement this feature from spec" — scope varies wildly
- "Fix this CI pipeline" — root cause is unknown until investigation

When subtasks are predictable, prompt chaining or explicit pipelines work well. When they're
unpredictable, you need orchestrator-workers or full autonomous agents that can dynamically
decide what to do next.

### Fixed vs Dynamic Scope

Does the task have clear boundaries, or does it expand as the agent works?

**Fixed scope:** "Add input validation to the login form" — clear start and end points, well-
defined success criteria.

**Dynamic scope:** "Make this codebase production-ready" — the agent discovers new subtasks
as it works. ForgeCode's multi-agent architecture (Forge/Muse/Sage) handles dynamic scope by
having specialized sub-agents that can be invoked as new concerns emerge.

### Measuring Task Complexity Dimensions

A practical scoring approach for task complexity:

```
Complexity Score = Steps x Unpredictability x Scope_Dynamism

Steps:          1 (single) | 2-5 (moderate) | 5+ (high)
Unpredictability: 1 (fixed pipeline) | 2 (some variance) | 3 (unknown path)
Scope_Dynamism:   1 (fixed) | 2 (bounded growth) | 3 (unbounded)
```

- Score 1-3: Augmented LLM or Prompt Chaining
- Score 4-12: Routing, Parallelization, or Orchestrator-Workers
- Score 13+: Full Autonomous Agent with safety rails

---

## The Decision Dimensions

Six dimensions determine which pattern is appropriate. Each creates pressure toward or away
from specific patterns.

### Dimension 1: Complexity

**How many steps? How unpredictable?**

Low complexity pushes toward simpler patterns (augmented LLM, prompt chaining). High
complexity pushes toward autonomous agents with dynamic planning capabilities.

Key observation: Claude Code operates as a single agentic loop for most tasks but spawns
sub-agents (Explore, Plan, custom) when complexity exceeds what a single loop handles
efficiently. This is adaptive complexity matching—the agent increases its own architectural
complexity in response to task complexity.

### Dimension 2: Autonomy

**Can it run without human input?**

Full autonomy requires either high confidence in correctness or the ability to recover from
errors. The permission models across agents reveal different autonomy philosophies:

- **Claude Code's graduated permissions**: Start restricted, earn trust through safe actions,
  escalate for dangerous operations. This enables progressive autonomy—the agent runs
  autonomously for safe operations but pauses for risky ones.
- **Codex CLI's sandbox-first**: All operations run in a 3-layer OS sandbox (macOS Seatbelt,
  Linux bubblewrap+seccomp, Windows ACLs). Maximum autonomy within strict boundaries.
- **Goose's multi-layered inspection**: Security → Adversary → Permission → Repetition
  pipeline gates every tool call. Autonomy is filtered, not blanket-granted.

Higher autonomy requirements push toward patterns with built-in safety: sandboxed autonomous
agents, evaluator-optimizer loops with verification, or orchestrator-workers with approval
gates between phases.

### Dimension 3: Error Tolerance

**What's the cost of a mistake?**

Low error tolerance (production deployments, irreversible operations) demands patterns with
verification stages: evaluator-optimizer, prompt chaining with gates, or human-in-the-loop
checkpoints. High error tolerance (exploration, prototyping) permits full autonomous agents.

The cost of errors varies by action type:
- **Read-only operations**: Near-zero cost. File reads, searches, analysis.
- **Reversible writes**: Low cost if tracked. Git-tracked file edits (Aider's git-native
  approach), database transactions.
- **Irreversible writes**: High cost. API calls with side effects, production deployments,
  email sends.
- **Destructive operations**: Very high cost. File deletions, data mutations without backup.

### Dimension 4: Latency

**How fast does it need to respond?**

Interactive use (chat, IDE integration) requires sub-second to seconds response times.
Batch processing (CI/CD, automated reviews) tolerates minutes.

Latency profiles by pattern:
- Augmented LLM: 1-5 seconds (single LLM call + tool use)
- Prompt Chaining: 5-30 seconds (sequential LLM calls)
- Routing: 2-10 seconds (classification + single handler)
- Parallelization: 3-15 seconds (concurrent LLM calls, bottleneck is slowest)
- Orchestrator-Workers: 10-60 seconds (planning + parallel execution + synthesis)
- Evaluator-Optimizer: 15-120 seconds (multiple generate-evaluate cycles)
- Autonomous Agent: 30 seconds to minutes (open-ended loop)

Warp's approach is instructive: by owning the full PTY and rendering with Metal GPU
acceleration, it minimizes the non-LLM latency so more of the time budget can go to model
inference. Latency optimization is not just about choosing simpler patterns—it's about
reducing overhead in whatever pattern you use.

### Dimension 5: Cost

**What's the token/compute budget?**

Cost scales with the number and size of LLM calls. Patterns that make fewer calls (augmented
LLM, prompt chaining) are cheaper than patterns that iterate (evaluator-optimizer) or fan
out (orchestrator-workers with parallelization).

Cost considerations:
- **Token input costs**: Long contexts (Gemini CLI's 1M tokens) enable richer augmented LLM
  but at higher per-call cost.
- **Token output costs**: Typically 3-4x input costs. Patterns generating lots of output
  (code generation, documentation) are more expensive.
- **Iteration costs**: Evaluator-optimizer loops multiply costs by the number of iterations.
  Budget caps are essential.
- **Parallelization costs**: Multiple simultaneous calls are cheaper than sequential if they
  reduce total iterations (get it right first time vs retry).

### Dimension 6: Reliability

**How consistent does it need to be?**

Some tasks need deterministic outputs (code formatting, migration scripts). Others tolerate
variation (creative writing, exploration). Reliability requirements push toward:

- **More constrained patterns** for high reliability: Prompt chaining with validation gates,
  structured outputs, few-shot examples.
- **More autonomous patterns** for lower reliability needs: Let the agent explore and iterate.
- **Voting/consensus patterns** for critical reliability: Run multiple agents, compare outputs.
  This is Anthropic's parallelization pattern applied for reliability.

---

## Decision Tree

Use this tree as a starting point, then refine with the dimension analysis above.

```
Is it a single-step task?
├── YES → Is there a clear correct answer?
│   ├── YES → Augmented LLM (single call + tools)
│   └── NO → Need multiple perspectives?
│       ├── YES → Parallelization (voting)
│       └── NO → Augmented LLM with rich context
│
└── NO (multi-step) → Are subtasks predictable in advance?
    ├── YES → Are subtasks independent of each other?
    │   ├── YES → Parallelization (sectioning)
    │   │         Can be combined with prompt chaining for
    │   │         the sequential parts
    │   └── NO → Prompt Chaining
    │             Gate each step's output before proceeding
    │             Example: Junie's understand→plan→implement→verify
    │
    └── NO (unpredictable subtasks) → Does input vary significantly?
        ├── YES → Routing to specialized handlers
        │         Then each handler uses its own pattern
        │         Example: ForgeCode's model routing per phase
        │         Example: Droid's multi-interface routing
        │
        └── NO → Is iterative refinement the core loop?
            ├── YES → Evaluator-Optimizer
            │         Generate → test → refine cycle
            │         Example: Aider's edit→test→fix loop
            │         Example: OpenHands' StuckDetector
            │
            └── NO → Is the scope dynamic/unknown?
                ├── YES → Autonomous Agent
                │         LLM-driven loop with tool use
                │         Requires: sandbox, permissions, recovery
                │         Example: Claude Code's main loop
                │
                └── NO → Orchestrator-Workers
                          Plan → delegate → synthesize
                          Example: Capy's Captain/Build split
                          Example: Ante's self-organizing agents
```

### Reading the Decision Tree

The tree captures the most common decision path but real tasks often straddle multiple
branches. When that happens, combine patterns: use routing at the top level to classify
the task, then apply the appropriate sub-pattern for each class.

---

## Pattern Selection Guide

| Pattern | Best For | Complexity | Latency | Cost | Reliability | Example Agents |
|---------|----------|:----------:|:-------:|:----:|:-----------:|----------------|
| Augmented LLM | Single-step tasks, Q&A, simple edits | Low | Low (1-5s) | Low | High | mini-SWE-agent, Pi Agent, OpenCode |
| Prompt Chaining | Known multi-step workflows | Medium | Medium (5-30s) | Medium | High | Junie CLI, Sage Agent |
| Routing | Variable input types/complexity | Medium | Low-Med (2-10s) | Medium | High | ForgeCode, Droid, Goose |
| Parallelization | Independent subtasks, consensus | Medium | Low-Med (3-15s) | Medium-High | Very High | ForgeCode, Capy |
| Orchestrator-Workers | Complex multi-part tasks | High | High (10-60s) | High | Medium-High | Claude Code, Ante, Capy |
| Evaluator-Optimizer | Quality-critical iterative tasks | High | High (15-120s) | High | Very High | Aider, OpenHands |
| Autonomous Agent | Open-ended, dynamic scope | Very High | Very High (30s+) | Very High | Medium | Claude Code, Codex CLI, OpenHands |

### Interpreting the Table

- **Reliability** refers to consistency of output quality, not uptime. Simpler patterns are
  more reliable because there's less to go wrong.
- **Cost** assumes equivalent model quality. Using cheaper models for routing classification
  (as ForgeCode does with model routing per phase) can dramatically reduce costs.
- **Latency** is wall-clock time for the pattern itself. Network latency and model inference
  speed vary by provider.

---

## Autonomy Requirements

### Human-in-the-Loop Needs

Not all tasks should run autonomously. The decision framework for human involvement:

**Full autonomy appropriate when:**
- All operations are reversible (git-tracked changes)
- Operations run in a sandbox (Codex CLI, OpenHands Docker)
- The task has clear, verifiable success criteria
- Error cost is low relative to the value of speed

**Human checkpoints appropriate when:**
- Operations have real-world side effects (API calls, deployments)
- The task involves ambiguous requirements
- The cost of errors exceeds the cost of interruption
- Trust in the agent is still being established

**Full human control appropriate when:**
- Operations are irreversible and high-stakes
- Regulatory or compliance requirements mandate human review
- The agent is operating in a new, untested domain

### Permission Models in Practice

Claude Code's graduated permission system illustrates adaptive autonomy:

```
Level 1: Read-only operations          → Always allowed
Level 2: Reversible file operations    → Allowed after first approval
Level 3: Shell commands (safe subset)  → Prompt once, remember
Level 4: Destructive operations        → Always prompt
Level 5: External network operations   → Always prompt with details
```

Codex CLI takes a different approach: everything runs in a sandbox, so the agent has full
autonomy within boundaries that are enforced at the OS level rather than the application
level. This is arguably more robust—the agent can't accidentally bypass permissions because
the sandbox is enforced by macOS Seatbelt or Linux seccomp.

### Sandbox Requirements

Sandbox sophistication should match task risk:

| Risk Level | Sandbox Approach | Example |
|------------|-----------------|---------|
| Low (read-only) | No sandbox needed | Code analysis, Q&A |
| Medium (file writes) | Git-tracked workspace | Aider, Gemini CLI shadow repos |
| High (shell commands) | Process isolation | Codex CLI bubblewrap, OpenHands Docker |
| Very High (network/system) | Full VM isolation | Capy's sandboxed Ubuntu VMs |

### Trust Levels and Pattern Implications

Lower trust → simpler patterns with more gates and verification.
Higher trust → more autonomous patterns with fewer interruptions.

Trust is not static—it should be built incrementally. Claude Code's approach of remembering
approved permissions and Gemini CLI's progressive skill disclosure both model trust as
something that grows over a session.

---

## Error Tolerance Analysis

### Reversible vs Irreversible Actions

The reversibility of actions is the single strongest factor in choosing an error tolerance
strategy.

**Reversible actions** allow aggressive autonomy:
- Git-tracked file edits (revert with `git checkout`)
- Database transactions (rollback)
- Container/VM state (snapshot and restore)
- Gemini CLI's git checkpoint shadow repos enable rollback to any prior state

**Irreversible actions** demand verification patterns:
- Sending emails or messages
- Calling external APIs with side effects
- Modifying production databases
- Publishing packages or deploying services

### Safety-Critical vs Exploratory Tasks

For safety-critical tasks, layer multiple verification strategies:
1. **Static verification**: Type checking, linting (before execution)
2. **Dynamic verification**: Test execution (after code generation)
3. **Semantic verification**: LLM review of changes (evaluator pattern)
4. **Human verification**: Final approval gate

For exploratory tasks, minimize verification overhead:
1. Run in sandbox
2. Let the agent iterate freely
3. Review the final result rather than intermediate steps

### Verification Strategies Per Pattern

| Pattern | Verification Strategy |
|---------|-----------------------|
| Augmented LLM | Output validation (format, length, basic checks) |
| Prompt Chaining | Gate between each step; reject and retry on failure |
| Routing | Validate classification; fallback to general handler |
| Parallelization | Compare outputs; flag disagreements for review |
| Orchestrator-Workers | Verify each worker output; synthesizer validates coherence |
| Evaluator-Optimizer | Built-in: evaluator IS the verification |
| Autonomous Agent | Periodic checkpoints; sandbox containment; StuckDetector |

---

## Latency Considerations

### Latency Budgets by Use Case

| Use Case | Acceptable Latency | Recommended Patterns |
|----------|-------------------|---------------------|
| Autocomplete | < 500ms | Pre-computed, no agent pattern |
| Chat response | 1-5 seconds | Augmented LLM |
| Code edit | 5-30 seconds | Prompt Chaining, Routing |
| Feature implementation | 1-5 minutes | Orchestrator-Workers, Autonomous Agent |
| Codebase migration | 5-30 minutes | Full Autonomous Agent with checkpointing |
| CI/CD pipeline | Minutes to hours | Any pattern, optimize for quality over speed |

### When to Trade Latency for Quality

The tradeoff is not linear. Key thresholds from real agent behavior:

- **Below 5 seconds**: Users expect responsiveness. Extra time feels like lag.
- **5-30 seconds**: Users tolerate delay if there's progress indication. Claude Code's
  streaming output and context compaction keep users informed.
- **30+ seconds**: Users context-switch. The agent should work asynchronously. Capy's 25+
  concurrent task support acknowledges this—users don't wait for one task.

### Latency Optimization Techniques

1. **Speculative execution**: Start likely subtasks before confirming they're needed.
   ForgeCode's multi-agent architecture can speculatively prepare multiple approaches.
2. **Context compaction**: Reduce input tokens to speed up inference. Claude Code and Droid
   both implement proprietary compaction strategies.
3. **Model routing for speed**: Use faster models for classification and simple subtasks,
   reserve powerful models for complex reasoning. ForgeCode routes different models per phase.
4. **Caching**: Cache common tool outputs (file reads, search results). OpenCode's SQLite
   persistence enables cross-session caching.
5. **Streaming**: Return partial results as they're generated. Nearly all 17 agents stream
   output to provide perceived responsiveness.

---

## Cost Modeling

### Token Costs Per Pattern

Approximate token costs for a "medium" task (e.g., adding a REST endpoint):

| Pattern | Input Tokens | Output Tokens | Total Calls | Estimated Cost |
|---------|:-----------:|:------------:|:-----------:|:--------------:|
| Augmented LLM | 5K-15K | 1K-3K | 1-2 | ~1-5 cents |
| Prompt Chaining | 15K-40K | 3K-10K | 3-5 | ~5-20 cents |
| Routing + Handler | 10K-30K | 2K-8K | 2-4 | ~3-15 cents |
| Parallelization | 20K-60K | 5K-15K | 3-8 | ~8-30 cents |
| Orchestrator-Workers | 30K-80K | 8K-25K | 5-12 | ~15-50 cents |
| Evaluator-Optimizer | 40K-120K | 10K-30K | 4-10 | ~20-60 cents |
| Autonomous Agent | 50K-200K+ | 15K-50K+ | 10-30+ | ~30 cents to 2+ dollars |

Costs based on typical frontier model pricing. Actual costs vary by provider and model.

### Model Routing as Cost Optimization

ForgeCode's model routing per phase is a cost optimization pattern: use cheap, fast models
for classification and simple subtasks, expensive models for complex reasoning. Junie CLI's
dynamic per-task model routing similarly matches model capability to subtask difficulty.

Example cost reduction through routing:

```
Without routing:  All steps use frontier-class model
  10 steps x ~5 cents/step = ~50 cents

With routing:     Classification uses small model (~0.5 cents)
                  3 complex steps use frontier model (~5 cents each)
                  6 simple steps use small model (~0.5 cents each)
  = 0.5 + (3 x 5) + (6 x 0.5) = ~18.5 cents

Savings: ~63%
```

### When Expensive Models Are Worth It

Expensive frontier models justify their cost when:
- The task is ambiguous and requires strong reasoning (planning, architecture)
- Errors are costly (production code, security-sensitive logic)
- The task is one-shot (no opportunity to iterate)
- The total token count is low (cost difference is small in absolute terms)

Cheaper models are sufficient when:
- The task is well-defined with clear patterns (boilerplate, formatting)
- There are verification steps that catch errors (evaluator-optimizer)
- The task is high-volume (cost savings multiply)
- Latency matters more than peak quality

---

## Real-World Decision Examples

### Example 1: "Fix this typo" → Augmented LLM

**Analysis:**
- Complexity: Single-step, predictable
- Autonomy: Can run fully autonomously (reversible git change)
- Error tolerance: High (it's a typo fix, easy to verify)
- Latency: Should be fast (< 5 seconds)
- Cost: Should be minimal

**Pattern**: Augmented LLM. One LLM call to identify and fix the typo. No orchestration,
no iteration, no routing needed. This is where mini-SWE-agent and Pi Coding Agent excel—
minimal scaffold, maximum efficiency.

### Example 2: "Refactor this function" → Prompt Chaining

**Analysis:**
- Complexity: Multi-step but predictable (analyze → plan → implement → verify)
- Autonomy: Semi-autonomous (may need clarification on scope)
- Error tolerance: Medium (should preserve behavior)
- Latency: 10-30 seconds acceptable
- Cost: Moderate (3-4 LLM calls)

**Pattern**: Prompt Chaining. Step 1: Analyze the function and identify refactoring
opportunities. Gate: Confirm the refactoring plan. Step 2: Implement the refactoring.
Gate: Verify tests still pass. This is Junie CLI's natural mode.

### Example 3: "Add a REST API endpoint" → Orchestrator-Workers

**Analysis:**
- Complexity: High (route handler, validation, business logic, tests, docs)
- Autonomy: Can be autonomous in a sandboxed environment
- Error tolerance: Medium (tests should catch issues)
- Latency: 1-3 minutes acceptable
- Cost: Higher but justified by multi-file scope

**Pattern**: Orchestrator-Workers. The orchestrator plans: "I need a route handler in X,
a service function in Y, tests in Z, and OpenAPI docs in W." Workers handle each
independently. Synthesis step verifies everything integrates. This is how Claude Code
naturally handles such tasks—its main loop acts as orchestrator, sub-agents as workers.

### Example 4: "Debug this failing test" → Evaluator-Optimizer

**Analysis:**
- Complexity: High and unpredictable (root cause unknown)
- Autonomy: Fully autonomous (sandbox, reversible)
- Error tolerance: Self-correcting (the test IS the evaluator)
- Latency: May take minutes
- Cost: Variable (depends on iterations)

**Pattern**: Evaluator-Optimizer. Generate a hypothesis → apply a fix → run the test →
if it fails, analyze the new failure → refine the fix. This is exactly OpenHands'
StuckDetector pattern and Aider's edit-test-fix loop. The test suite serves as an
automated evaluator.

### Example 5: "Implement this feature from spec" → Full Autonomous Agent

**Analysis:**
- Complexity: Very high, dynamic scope
- Autonomy: Needs to run independently for extended periods
- Error tolerance: Medium (iterative, tests verify)
- Latency: Minutes to tens of minutes
- Cost: High but necessary

**Pattern**: Full Autonomous Agent. The task is too open-ended for a fixed pipeline and too
complex for a single orchestration round. The agent needs to: read the spec, explore the
codebase, plan an approach, implement across multiple files, write tests, iterate on
failures, and potentially revise the plan. Claude Code and ForgeCode handle this with
sophisticated agentic loops that combine multiple sub-patterns.

---

## Combining Patterns

### Why Single Patterns Are Insufficient

No production-quality coding agent uses just one pattern. Real tasks have sub-components
at different complexity levels, and the agent needs to match its approach to each.

### How Real Agents Combine Patterns

**Claude Code** layers three primary patterns:
1. **Augmented LLM** as the base: Rich tool access, CLAUDE.md memory, context compaction
2. **Orchestrator-Workers** for complex tasks: Spawns Explore/Plan/custom sub-agents
3. **Evaluator-Optimizer** implicitly: Tool feedback loops where the agent observes
   command output and adjusts its approach

**ForgeCode** combines four patterns:
1. **Routing**: Model routing per phase (different models for planning vs execution)
2. **Orchestrator-Workers**: Forge/Muse/Sage sub-agents with specialized roles
3. **Parallelization**: Multiple sub-agents can work concurrently
4. **Evaluator-Optimizer**: Results from sub-agents feed back to the orchestrator

**Goose** demonstrates MCP-native pattern composition:
1. **Augmented LLM**: Base LLM with extensive MCP tool access
2. **Routing**: Multi-layered tool inspection pipeline (Security → Adversary → Permission
   → Repetition) routes each tool call through appropriate filters
3. **Orchestrator-Workers**: Summon sub-agents for specialized tasks

### Pattern Composition Rules

1. **Augmented LLM is always the base**: Every agent starts with an LLM + tools.
2. **Routing typically sits at the top**: Classify first, then apply sub-patterns.
3. **Evaluator-Optimizer wraps other patterns**: Any pattern's output can be evaluated
   and refined.
4. **Parallelization is orthogonal**: Can be applied within any other pattern.
5. **Orchestrator-Workers nests naturally**: Workers can themselves use any pattern.

---

## Anti-Patterns in Pattern Selection

### Using Full Agents for Simple Tasks

**Symptom**: 30-second response times for typo fixes. High token costs for trivial changes.
User frustration with unnecessary complexity.

**Cause**: Default to the most capable pattern regardless of task complexity.

**Fix**: Implement complexity detection. Claude Code's approach of starting with the main
loop and only spawning sub-agents when needed is the right model. mini-SWE-agent's entire
philosophy—"scaffold complexity has diminishing returns"—is a response to this anti-pattern.

### Static Pipelines for Dynamic Problems

**Symptom**: The pipeline breaks when the task doesn't fit the expected shape. Error
recovery is poor because the pipeline has no mechanism to re-plan.

**Cause**: Using prompt chaining (fixed sequence) when orchestrator-workers or autonomous
agents (dynamic sequencing) are needed.

**Fix**: Add re-planning capability. If a pipeline step fails, don't just retry—reconsider
whether the pipeline itself is correct. Sage Agent's 5-stage pipeline works because each
stage can feed back to earlier stages.

### Ignoring Cost/Latency Tradeoffs

**Symptom**: Using frontier models for every sub-call. Running evaluator-optimizer loops
with no iteration cap. Token costs that grow linearly with task complexity.

**Cause**: Optimizing solely for quality without considering resource constraints.

**Fix**: Implement model routing (ForgeCode, Junie CLI), iteration caps on evaluator loops,
and cost monitoring. Budget-aware pattern selection: if the task budget is low, use simpler
patterns even if quality is slightly lower.

### Insufficient Error Recovery

**Symptom**: Agents get stuck in loops, repeat the same failing approach, or produce
incorrect results without detection.

**Cause**: Missing evaluator components, no stuck detection, no fallback strategies.

**Fix**: Implement OpenHands-style StuckDetector, add fallback routing (if primary pattern
fails, try a different approach), and set hard limits on retries.

---

## Code Examples

### Decision Logic: Pattern Selection

```python
def select_pattern(task):
    """Select the primary pattern based on task characteristics."""

    complexity = assess_complexity(task)

    if complexity.steps == 1:
        if complexity.needs_consensus:
            return Pattern.PARALLELIZATION_VOTING
        return Pattern.AUGMENTED_LLM

    if complexity.subtasks_predictable:
        if complexity.subtasks_independent:
            return Pattern.PARALLELIZATION_SECTIONING
        return Pattern.PROMPT_CHAINING

    if complexity.input_varies_significantly:
        return Pattern.ROUTING

    if complexity.iterative_refinement_core:
        return Pattern.EVALUATOR_OPTIMIZER

    if complexity.scope_dynamic:
        return Pattern.AUTONOMOUS_AGENT

    return Pattern.ORCHESTRATOR_WORKERS


def assess_complexity(task):
    """Assess task complexity along multiple dimensions."""
    return TaskComplexity(
        steps=estimate_steps(task),
        subtasks_predictable=are_subtasks_known(task),
        subtasks_independent=are_subtasks_independent(task),
        input_varies_significantly=has_variable_input(task),
        iterative_refinement_core=needs_iteration(task),
        scope_dynamic=is_scope_dynamic(task),
        needs_consensus=needs_multiple_perspectives(task),
    )
```

### Adaptive Pattern Escalation

```python
def execute_with_escalation(task, pattern=None):
    """Start with the simplest viable pattern and escalate if needed."""

    if pattern is None:
        pattern = select_pattern(task)

    result = execute_pattern(task, pattern)

    if result.success:
        return result

    if result.failure_type == "too_complex":
        next_pattern = escalate(pattern)
        if next_pattern:
            return execute_with_escalation(task, next_pattern)

    if result.failure_type == "stuck":
        alternative = find_alternative_pattern(task, pattern)
        if alternative:
            return execute_with_escalation(task, alternative)

    return result  # Failed, return best effort


ESCALATION_PATH = {
    Pattern.AUGMENTED_LLM: Pattern.PROMPT_CHAINING,
    Pattern.PROMPT_CHAINING: Pattern.ORCHESTRATOR_WORKERS,
    Pattern.ORCHESTRATOR_WORKERS: Pattern.AUTONOMOUS_AGENT,
    Pattern.EVALUATOR_OPTIMIZER: Pattern.AUTONOMOUS_AGENT,
}
```

### Cost-Aware Model Routing

```python
def route_model(subtask, budget_remaining):
    """Route to appropriate model based on subtask complexity and budget."""

    if budget_remaining < MINIMUM_BUDGET:
        return Model.FAST_CHEAP  # Always have a fallback

    if subtask.type == "classification":
        return Model.FAST_CHEAP  # Routing doesn't need frontier models

    if subtask.type == "planning":
        return Model.FRONTIER  # Planning benefits from strong reasoning

    if subtask.type == "code_generation":
        if subtask.complexity == "boilerplate":
            return Model.FAST_CHEAP
        return Model.FRONTIER

    if subtask.type == "verification":
        return Model.FAST_CHEAP  # Often just parsing test output

    return Model.BALANCED  # Default to mid-tier
```

---

## Key Takeaways

1. **Start simple, escalate as needed.** Begin with the simplest pattern that could work.
   Most tasks don't need full autonomous agents. mini-SWE-agent proves that a minimal
   scaffold can handle surprisingly complex tasks.

2. **Match the pattern to the task, not the agent.** The best agents (Claude Code, ForgeCode)
   dynamically select their approach based on task characteristics rather than applying the
   same pattern to everything.

3. **The six dimensions are not equally weighted.** Error tolerance and cost typically
   dominate in production. Latency dominates in interactive use. Complexity dominates in
   research settings.

4. **Combine patterns deliberately.** Most real tasks need 2-3 patterns working together.
   Use routing at the top level, the appropriate pattern for each subtask type, and
   evaluator-optimizer as a wrapper for quality-critical outputs.

5. **Budget for failure.** Any pattern can fail. Build in escalation paths, fallback
   strategies, and hard limits on iteration. OpenHands' StuckDetector is not just a nice
   feature—it's essential for production autonomous agents.

6. **Cost optimization is a first-class concern.** Model routing per phase (ForgeCode,
   Junie CLI) can reduce costs by 50-70% without meaningful quality loss. Don't use
   frontier models for classification or verification.

7. **Trust is earned, not assumed.** Progressive permission models (Claude Code) and
   sandbox-first approaches (Codex CLI) let agents be autonomous *safely*. The pattern
   should match the trust level, and trust should grow over time.

8. **The decision tree is a starting point, not a prescription.** Real tasks are messy.
   Use the framework to make a principled initial choice, then adapt based on what you
   learn during execution. The best agents do this automatically.
