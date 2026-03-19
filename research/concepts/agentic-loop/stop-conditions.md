# Stop Conditions

## Overview

The loop MUST terminate — runaway agents are expensive, dangerous, and annoying.
A single GPT-4 call costs roughly $0.03–0.10; an uncontrolled loop can burn through
hundreds of dollars in minutes. Beyond cost, a runaway agent can corrupt files, spam
APIs, or fill disks. Every production agent therefore implements a **layered** approach:
multiple independent conditions, any one of which can halt the loop.

The evaluation order matters:

```
interrupt > resource limits > stuck detection > verification gate > natural completion
```

Higher-priority conditions override lower ones. A user pressing Ctrl+C wins over
everything else. A budget cap wins over a model that "wants" to keep going. Natural
completion — the model simply stops calling tools — is the *last* condition checked,
because it is the least trustworthy in adversarial or degenerate cases.

---

## 1. Model Signals Completion (Natural Termination)

The most common stop condition across all agents: the LLM responds with **text only**
and makes **no tool calls**. This signals the model believes it has nothing left to do.

Every agent implements this. It is the universal stop condition.

### How Each Agent Detects It

| Agent | Detection Mechanism | Code Path |
|-------|---------------------|-----------|
| mini-SWE-agent | `messages[-1].role == "exit"` (model outputs `submit` command) | `agent.run()` main loop |
| OpenCode | `FinishReason == EndTurn` (not `ToolUse`) | Session event loop |
| Goose | Text-only response → exit loop | `reply()` method |
| Codex | `!session.has_pending_tool_results()` → emit `TurnComplete` | Rollout handler |
| OpenHands | Agent returns `AgentFinishAction` → controller transitions to `FINISHED` | AgentController state machine |
| Claude Code | No tool_use blocks in response → respond to user | Main agent loop |
| Aider | No edit blocks in response → display and return | Coder.run() |
| Gemini CLI | No function calls in response → print and prompt | Turn loop |

### Pseudocode

```python
response = llm.chat(messages)

# Check: did the model invoke any tools?
tool_calls = [b for b in response.content if b.type == "tool_use"]

if not tool_calls:
    # Natural termination — model is done
    display(response.text)
    return
```

### Why It Works (Usually)

Modern LLMs are well-calibrated on when tasks are "done." Given a system prompt like
*"Fix the failing test in auth.py"*, the model will edit the file, see the test pass,
and respond with a summary — no further tool calls. The system prompt shapes completion
behavior: telling the model *"respond with DONE when finished"* or *"submit your answer"*
gives it a clear termination signal.

### Why It's Not Sufficient Alone

- The model can hallucinate tool calls in a loop, never stopping
- Ambiguous tasks ("improve the codebase") have no natural end
- Degenerate states (empty responses, repeated actions) aren't "completion"
- Cost accumulates silently while the model keeps calling tools
- You need safety nets

---

## 2. Max Iterations / Turn Limits

Hard caps to prevent runaway loops. The simplest and most widely-used safety net.

| Agent | Iteration Limit | Budget Limit | What Happens |
|-------|----------------|--------------|--------------|
| mini-SWE-agent | `step_limit` (configurable) | `cost_limit` ($3.00 default) | `LimitsExceeded` raised |
| OpenHands | `max_iterations` per run | Per-task budget | Controller stops |
| Goose | `max_turns` (1000 default, `GOOSE_MAX_TURNS`) | — | Loop exits |
| Gemini CLI | Max iterations per turn + timeout | Token budget exhaustion | Turn terminates |
| Junie | 3–5 implement-verify cycles | Token + time budget | Escalates to user |
| ForgeCode | `max_requests_per_turn` | `FORGE_TOOL_TIMEOUT` (300s) | Turn ends |
| Codex | — (no explicit turn limit) | Auto-compaction at 90% context | Compacts, continues |
| OpenCode | No explicit turn limit | — | Relies on model |
| Claude Code | Configurable per session | Token-based | Pauses, asks user |

### Implementation Pattern

```python
MAX_ITERATIONS = 25  # Safety cap

for i in range(MAX_ITERATIONS):
    response = llm.generate(messages)

    if no_tool_calls(response):
        return response.text      # Natural completion

    results = execute_tools(response.tool_calls)
    messages.extend(results)

# Fell through — hit the cap
logger.warning(f"Agent hit iteration limit ({MAX_ITERATIONS})")
return "I reached my iteration limit. Here's what I accomplished so far: ..."
```

### Design Decision: Fixed vs Dynamic Limits

**Fixed limits** (e.g., `max_iterations=25`):
- Simple, predictable, easy to reason about
- May terminate too early on complex tasks
- May be wastefully high for simple tasks
- Used by: mini-SWE-agent, OpenHands, Goose

**Dynamic limits** (adapt to task complexity):
- Can scale up for hard problems, down for easy ones
- Harder to reason about, harder to set cost guarantees
- Used by: Junie (cycle-based), Codex (compaction-based)

**No explicit limit** (trust the model):
- OpenCode and Codex deliberately omit hard turn limits
- Philosophy: the model should decide when it's done
- Relies heavily on other conditions (budget, stuck detection)
- Risky for unattended operation

---

## 3. Token Budget Exhaustion

Every API call costs tokens. Tokens cost money. Budget tracking prevents $100 agent runs.

### Cost Tracking

```python
# mini-SWE-agent's approach
class AgentLoop:
    def __init__(self, config):
        self.cost = 0.0
        self.config = config  # config.cost_limit defaults to $3.00

    def step(self):
        # Check budget BEFORE making the call
        if 0 < self.config.cost_limit <= self.cost:
            raise LimitsExceeded({
                "role": "exit",
                "content": "LimitsExceeded",
                "extra": {"exit_status": "LimitsExceeded"}
            })

        response = self.llm.call(self.messages)

        # Track cost from response metadata
        self.cost += response.usage.get("cost", 0.0)

        return response
```

### Context Window Exhaustion

Distinct from dollar-cost budget — the model's context window fills up:

```
┌──────────────────────────────────────────────┐
│               Context Window (128K)           │
├──────────────────────────────────────────────┤
│ System prompt          │    ~2K tokens        │
│ Conversation history   │    grows each turn   │
│ Tool results           │    can be huge        │
│ Available space        │    shrinks each turn  │
└──────────────────────────────────────────────┘
```

When the window fills:
- **Codex**: auto-compaction at 90% usage — summarize old turns, continue
- **Goose**: `ContextLengthExceeded` → compaction attempt (2 retries), then fail
- **OpenHands**: condense events (LLM-based summarization of old history)
- **Claude Code**: automatic context compaction with conversation summary

### Token Budget vs Context Budget

| Dimension | Token Budget | Context Budget |
|-----------|-------------|---------------|
| What fills it | Cumulative tokens across all calls | Tokens in current context window |
| Cost | Real dollars | Latency + quality degradation |
| Recovery | None — hard stop | Compaction / summarization |
| Typical limit | $3–$10 per task | 128K–200K tokens |

---

## 4. User Interruption

Every interactive agent supports interruption. This is the highest-priority stop
condition — a human override.

| Agent | Mechanism | Behavior |
|-------|-----------|----------|
| OpenCode | `Cancel(sessionID)` → Go context cancellation | Clean abort, context preserved |
| Claude Code | `Esc` key | Stop mid-action, preserve context, can redirect |
| Goose | `CancellationToken` | Checked at loop top and during tool collection |
| Codex | `Op::Interrupt` → cancel stream + abort tools | Double Ctrl+C exits entirely |
| Warp | Handback pattern | Agent yields at natural breakpoints |
| Aider | Ctrl+C | Cancel current LLM call, return to prompt |

### Interrupt Hierarchy

```
Level 0: Soft redirect
  └─ User types while agent works (Claude Code)
  └─ Agent sees message and adjusts approach

Level 1: Single interrupt (Ctrl+C / Esc)
  └─ Cancel current operation
  └─ Preserve all context
  └─ Return to user for new instructions

Level 2: Double interrupt (Ctrl+C × 2)
  └─ Force quit (Codex exits program entirely)
  └─ May lose unsaved context
```

### Claude Code's Notable Approach

Claude Code allows the user to **type while the agent works**. The agent sees the
incoming message and can adjust its approach mid-stream — no need to wait for completion,
no need to explicitly interrupt. `Esc` provides a hard interrupt, but the soft redirect
is often sufficient.

```
User: "Fix the auth bug"
Claude: [working... editing auth.py... running tests...]
User: "Actually, focus on the JWT expiration logic"    ← typed mid-stream
Claude: [sees message, pivots to JWT logic]
```

### Implementation: Cooperative Cancellation

Most agents use **cooperative** cancellation — they check a flag at safe points:

```python
# Goose-style cancellation
class AgentLoop:
    def __init__(self):
        self.cancelled = False

    def run(self):
        while not self.cancelled:          # Check at loop top
            response = self.llm.generate(self.messages)

            if self.cancelled:             # Check after LLM call
                break

            for tool_call in response.tool_calls:
                if self.cancelled:         # Check before each tool
                    break
                self.execute(tool_call)

    def cancel(self):
        self.cancelled = True
```

The alternative is **preemptive** cancellation (killing the thread/process), used as a
last resort (Level 2 interrupts).

---

## 5. Error Accumulation Thresholds

Rather than failing on a single error, production agents tolerate errors up to a
threshold, then bail.

### Error Budget Pattern

```python
consecutive_errors = 0
MAX_CONSECUTIVE = 5

for each_turn:
    result = execute_tool(action)
    if result.is_error:
        consecutive_errors += 1
        if consecutive_errors >= MAX_CONSECUTIVE:
            log.error(f"Hit {MAX_CONSECUTIVE} consecutive errors, stopping")
            break  # Too many errors, give up
    else:
        consecutive_errors = 0  # Reset on success
```

### Per-Agent Error Handling

| Agent | Strategy | Threshold | Recovery |
|-------|----------|-----------|----------|
| ForgeCode | `max_tool_failure_per_turn` | Configurable | End turn |
| OpenHands | Error loop detection in StuckDetector | K consecutive | Inject hint or stop |
| Goose | `ContextLengthExceeded` triggers compaction | 2 attempts | Compaction, then fail |
| Aider | Malformed edit retries | 3 retries | Fall back to whole-file |
| Claude Code | Tool error fed back to model | Implicit | Model self-corrects |

### Error Classification

Not all errors are equal. Sophisticated agents classify:

```
Retriable errors (don't count toward budget):
  - Rate limit (429) → wait and retry
  - Transient network error → retry with backoff
  - Server error (500) → retry once

Countable errors (increment error budget):
  - Tool returned error output → model should adapt
  - File not found → model picked wrong path
  - Syntax error in generated code → model made a mistake

Fatal errors (immediate stop):
  - Authentication failure → can't continue
  - Sandbox violation → security boundary
  - Out of disk space → infrastructure issue
```

---

## 6. Stuck Detection

The most sophisticated stop condition. Detects when an agent is looping without
making progress, even though it hasn't hit any hard limits.

### OpenHands' 4-Strategy StuckDetector

OpenHands runs stuck detection **after every step**, checking four independent
strategies. If any fires, the agent is considered stuck.

#### Strategy 1: Identical Repetition

```python
def check_identical_repetition(actions: list) -> bool:
    """Three identical actions in a row → stuck."""
    if len(actions) < 3:
        return False
    return (
        actions[-1] == actions[-2] == actions[-3]
    )

# Example: agent keeps running `cat /etc/passwd` three times
# → stuck
```

#### Strategy 2: Alternating Pattern (Ping-Pong)

```python
def check_alternating(actions: list) -> bool:
    """A-B-A-B pattern → stuck."""
    if len(actions) < 4:
        return False
    return (
        actions[-1] == actions[-3] and
        actions[-2] == actions[-4] and
        actions[-1] != actions[-2]
    )

# Example: agent alternates between `edit file` and `undo edit`
# → stuck in a ping-pong loop
```

#### Strategy 3: Error Loop

```python
def check_error_loop(observations: list, k: int = 4) -> bool:
    """Last K observations are all errors → stuck."""
    if len(observations) < k:
        return False
    return all(
        isinstance(obs, ErrorObservation)
        for obs in observations[-k:]
    )
```

#### Strategy 4: Empty Response

```python
def check_empty_response(actions: list, k: int = 3) -> bool:
    """Last K actions have empty/near-empty content → degenerate."""
    if len(actions) < k:
        return False
    return all(
        len(action.content.strip()) < 5
        for action in actions[-k:]
    )
```

#### Stuck Detection Flow

```
                    ┌─────────────┐
                    │  Agent Step  │
                    └──────┬──────┘
                           │
                    ┌──────▼──────┐
                    │ StuckDetector│
                    │  (4 checks) │
                    └──────┬──────┘
                           │
              ┌────────────┼────────────┐
              │            │            │
         Not Stuck     Stuck       Stuck
              │       (recoverable) (terminal)
              │            │            │
              ▼            ▼            ▼
          Continue    Inject hint   Raise error
                     "Try a         and stop
                      different
                      approach"
```

### Recovery Options When Stuck Is Detected

1. **Raise `AgentStuckInLoopError`** — hard terminate, report to user
2. **Inject `LoopRecoveryAction`** — add a message: *"You seem stuck. Try a completely different approach."*
3. **Force condensation** — compress conversation history to break the pattern (the model sees a fresh-looking context and often tries something new)

### Goose's RepetitionInspector

Goose takes a different approach: it tracks tool call patterns and can **block** further
calls to the same tool:

```rust
// Pseudocode for Goose's repetition detection
if tool_called_count[tool_name] > THRESHOLD
   && no_observable_progress():
    block_tool(tool_name)
    inject_message("Tool '{tool_name}' isn't making progress. Try something else.")
```

---

## 7. Verification-Based Stopping

A fundamentally different philosophy: the agent doesn't stop when it *thinks* it's
done — it stops when it can *prove* it's done.

### ForgeCode: Mandatory Verification

ForgeCode's runtime **programmatically enforces** a verification pass before marking
a task complete:

```python
# ForgeCode verification enforcement (conceptual)
class TaskRunner:
    def complete_task(self, result):
        if not self.verification_skill_was_called:
            # Don't accept completion — force verification
            self.inject_message(
                "Before finishing, you MUST run the verification skill "
                "to confirm your changes are correct."
            )
            return False  # Continue loop

        if not self.verification_passed:
            self.inject_message(
                "Verification failed. Fix the issues and re-verify."
            )
            return False  # Continue loop

        return True  # Actually done
```

**Key insight from TermBench**: models skip optional verification under time/token
pressure. Making verification mandatory — not just suggested — dramatically improves
completion quality.

### Aider: Lint/Test as Implicit Verification

```
Edit code → auto-lint → auto-test → check results
    │                                      │
    │         ┌────── pass ◄──────────────┘
    │         │
    │         └────── fail → retry (bounded)
    │                          │
    │                     max retries?
    │                     ├── no → loop back to edit
    │                     └── yes → present results with errors
    ▼
  Done (if --auto-test enabled and tests pass)
```

- If `--auto-test` enabled and tests fail → iterate (bounded retries)
- Tests pass → done
- Tests fail after retries → present results with errors, let user decide

### Junie: First-Class Verification Cycles

Junie treats verification as a **first-class phase**, not an afterthought:

1. **Implement** — make the code changes
2. **Verify** — run tests, check inspections, validate compilation
3. **If failures** → diagnostic loop (analyze → fix → re-verify)
4. **3–5 iterations** before escalating to user

```
┌──────────┐    ┌──────────┐    ┌──────────┐
│Implement │───▶│  Verify  │───▶│   Done   │
└──────────┘    └────┬─────┘    └──────────┘
                     │ fail
                     ▼
                ┌──────────┐
                │ Diagnose │
                │  & Fix   │──── (max 3-5 cycles)
                └──────────┘
```

---

## Stop Condition Layering

In practice, agents check multiple conditions in a strict priority order. Here's the
canonical pattern distilled from studying 10+ agent implementations:

```python
def agent_loop(task, config):
    messages = [system_prompt, task]
    iterations = 0
    total_cost = 0.0
    consecutive_errors = 0
    action_history = []
    verified = False

    while True:
        # ── Priority 1: User interrupt ──────────────────────
        if cancelled:
            log("Stopped by user")
            return partial_result()

        # ── Priority 2: Resource limits ─────────────────────
        iterations += 1
        if iterations > config.max_iterations:
            log(f"Hit iteration limit: {config.max_iterations}")
            return partial_result()

        if total_cost > config.budget:
            log(f"Budget exhausted: ${total_cost:.2f}")
            return partial_result()

        # ── Priority 3: Stuck detection ─────────────────────
        if is_stuck(action_history):
            if config.stuck_recovery:
                inject_recovery_hint(messages)
            else:
                log("Agent stuck in loop")
                return partial_result()

        # ── Core: LLM call ──────────────────────────────────
        response = llm.generate(messages)
        total_cost += response.cost

        # ── Priority 5: Natural completion ──────────────────
        if no_tool_calls(response):
            # ── Priority 4: Verification gate ───────────────
            if config.require_verification and not verified:
                inject_verification_prompt(messages)
                continue
            return response.text  # Done!

        # ── Execute tools ───────────────────────────────────
        for tool_call in response.tool_calls:
            result = execute_with_timeout(
                tool_call,
                timeout=config.tool_timeout  # e.g., 300s
            )
            messages.append(result)
            action_history.append(tool_call)

            if result.is_error:
                consecutive_errors += 1
                if consecutive_errors >= config.max_consecutive_errors:
                    log("Too many consecutive errors")
                    return partial_result()
            else:
                consecutive_errors = 0

            if result.triggers_verification:
                verified = True
```

### Priority Order (Why This Order)

| Priority | Condition | Rationale |
|----------|-----------|-----------|
| 1 | User interrupt | Human override is sacrosanct |
| 2 | Resource limits (budget, iterations) | Prevent financial/compute damage |
| 3 | Stuck detection | Avoid wasting remaining budget on loops |
| 4 | Verification gate | Ensure quality before accepting completion |
| 5 | Natural completion | Model's own judgment (least trustworthy) |

The ordering reflects a trust hierarchy: we trust the human most, the model least.

---

## Timeout Patterns

Timeouts are a special class of stop condition that apply to **sub-operations** within
the loop, not the loop itself.

### Tool Execution Timeouts

```python
import signal

class ToolTimeout(Exception):
    pass

def execute_with_timeout(tool_call, timeout=300):
    """Run a tool with a wall-clock timeout."""
    def handler(signum, frame):
        raise ToolTimeout(f"Tool {tool_call.name} timed out after {timeout}s")

    signal.signal(signal.SIGALRM, handler)
    signal.alarm(timeout)
    try:
        result = tool_call.execute()
    finally:
        signal.alarm(0)  # Cancel the alarm
    return result
```

### Timeout Hierarchy

```
┌─────────────────────────────────────────────────┐
│ Session timeout (e.g., 30 min for CI/CD)        │
│  ┌─────────────────────────────────────────────┐ │
│  │ Turn timeout (e.g., 5 min per LLM call)     │ │
│  │  ┌─────────────────────────────────────────┐ │ │
│  │  │ Tool timeout (e.g., 300s per tool exec) │ │ │
│  │  │  ┌─────────────────────────────────────┐ │ │ │
│  │  │  │ Subprocess timeout (e.g., 60s)      │ │ │ │
│  │  │  └─────────────────────────────────────┘ │ │ │
│  │  └─────────────────────────────────────────┘ │ │
│  └─────────────────────────────────────────────┘ │
│                                                   │
└─────────────────────────────────────────────────┘
```

| Level | Typical Value | Agent Example |
|-------|--------------|---------------|
| Session | 30 min | CI/CD integrations, Codex background tasks |
| Turn | 5 min | Gemini CLI per-turn timeout |
| Tool execution | 300s | ForgeCode `FORGE_TOOL_TIMEOUT` |
| Subprocess | 60s | Sandbox command execution limits |

### Hung Process Detection

```python
# ForgeCode-style tool timeout with cleanup
async def run_tool_with_timeout(tool, args, timeout=300):
    proc = await asyncio.create_subprocess_exec(...)

    try:
        stdout, stderr = await asyncio.wait_for(
            proc.communicate(),
            timeout=timeout
        )
    except asyncio.TimeoutError:
        proc.kill()
        await proc.wait()
        return ToolResult(
            error=True,
            output=f"Tool timed out after {timeout}s. "
                   f"The process was killed."
        )

    return ToolResult(output=stdout, error=proc.returncode != 0)
```

---

## Comparison: Agent Stop Condition Coverage

Which agents implement which conditions:

| Condition | mini-SWE | OpenHands | Goose | Codex | Claude Code | ForgeCode | OpenCode | Aider |
|-----------|----------|-----------|-------|-------|-------------|-----------|----------|-------|
| Natural completion | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ |
| Iteration limit | ✅ | ✅ | ✅ | — | ✅ | ✅ | — | — |
| Cost budget | ✅ | ✅ | — | — | ✅ | — | — | — |
| User interrupt | — | — | ✅ | ✅ | ✅ | — | ✅ | ✅ |
| Stuck detection | — | ✅ | ✅ | — | — | — | — | — |
| Error threshold | — | ✅ | ✅ | — | — | ✅ | — | — |
| Verification gate | — | — | — | — | — | ✅ | — | ✅ |
| Tool timeout | — | ✅ | — | — | — | ✅ | — | — |
| Context compaction | — | ✅ | ✅ | ✅ | ✅ | — | — | — |

Key observations:
- **Natural completion** is universal — every agent has it
- **Stuck detection** is rare — only OpenHands and Goose invest in it
- **Verification gates** are rarest — only ForgeCode enforces it programmatically
- **Headless agents** (mini-SWE, OpenHands) rely more on hard limits
- **Interactive agents** (Claude Code, Codex) rely more on user interrupt

---

## Anti-Patterns

### 1. No Safety Cap

```python
# DANGEROUS: relies entirely on model judgment
while True:
    response = llm.generate(messages)
    if not response.tool_calls:
        break
    execute(response.tool_calls)
# If model never stops calling tools → infinite loop → infinite cost
```

### 2. Too-Tight Limits

```python
# FRUSTRATING: agent can't finish real tasks
MAX_ITERATIONS = 3  # Way too low for any non-trivial task
```

### 3. Silent Termination

```python
# BAD: user has no idea why the agent stopped
if iterations > max:
    return ""  # Just... stops. No explanation.
```

### 4. No Cost Tracking

```python
# EXPENSIVE: no visibility into spend
for i in range(1000):
    response = llm.generate(messages)  # Each call costs $$$
    # No tracking, no limits, surprise $200 bill
```

---

## Best Practices

1. **Always have at least 2 stop conditions** — natural completion + a safety cap.
   Belt AND suspenders. The model is usually right about when it's done, but "usually"
   isn't good enough.

2. **Track costs from the start** — cheap to add, expensive to miss. Even if you don't
   enforce a budget, *log* the cost of every call. You'll need this data to set
   reasonable limits later.

3. **Implement stuck detection if running unattended** — in CI/CD or batch processing,
   there's no human to notice the agent is spinning. OpenHands' 4-strategy approach is
   a good reference implementation.

4. **Make interruption clean** — preserve context so the user can resume or redirect.
   Don't throw away the conversation history on Ctrl+C. Claude Code's approach (context
   preserved, user can redirect) is the gold standard.

5. **Log why the agent stopped** — essential for debugging and improvement. Every stop
   condition should produce a distinct, machine-readable exit reason:

   ```python
   class StopReason(Enum):
       NATURAL_COMPLETION = "natural_completion"
       ITERATION_LIMIT = "iteration_limit"
       BUDGET_EXHAUSTED = "budget_exhausted"
       USER_INTERRUPT = "user_interrupt"
       STUCK_DETECTED = "stuck_detected"
       ERROR_THRESHOLD = "error_threshold"
       VERIFICATION_FAILED = "verification_failed"
       TOOL_TIMEOUT = "tool_timeout"
   ```

6. **Consider verification enforcement for production systems** — TermBench showed that
   models routinely skip optional verification. If correctness matters, make verification
   a hard gate, not a suggestion.

7. **Separate tool timeouts from loop limits** — a single hung `bash` command shouldn't
   burn your entire iteration budget. Timeout the tool independently, feed the error back
   to the model, and let it adapt.

8. **Provide graceful degradation** — when a limit is hit, don't just stop. Summarize
   what was accomplished, what remains, and why the agent stopped. This turns a failure
   into actionable information.