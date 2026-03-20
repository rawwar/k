---
title: Rollback Strategies
status: complete
---

# Rollback Strategies

## Why Rollback Matters

Coding agents iterate. They write code, run tests, see failures, and try again. This
**edit-verify-retry** loop is the heartbeat of every agentic system — but it has a dark
side: failed attempts leave debris. A function rewritten incorrectly, an import deleted
by accident, a config file corrupted by a bad regex substitution — without the ability
to cleanly revert, each failure compounds the next.

**The core insight:** rollback is not just an undo button — it is what enables the
entire verify-and-retry paradigm to work at scale. Without it, the agent must succeed
on the first attempt or accumulate damage. With it, the agent can explore freely.

```
Without rollback:                    With rollback:

  Edit → Test → FAIL                   Edit → Test → FAIL
    │                                     │
    ▼                                     ▼
  Edit (on broken state) → FAIL        Revert → Edit → Test → FAIL
    │                                     │
    ▼                                     ▼
  Edit (worse state) → FAIL            Revert → Edit → Test → PASS ✓
    │
    ▼
  Give up (codebase damaged)
```

The practical impact is measurable. Agents with proper rollback attempt 3–5 fix
cycles without degradation; those without see success rates drop after 1–2 failed
attempts as codebase state becomes increasingly corrupted.

---

## The Rollback Spectrum

Not all rollback is created equal. The 17 agents in this research library span a wide
spectrum of rollback sophistication, from agents with no rollback mechanism at all to
those with full VM-level snapshot/restore capabilities.

```
Sophistication ──────────────────────────────────────────────────────────►

No rollback    File backup    Git stash/     Checkpoints    Event        VM/Container
               & restore      reset                        sourcing     snapshots
─────────────────────────────────────────────────────────────────────────────────────
mini-SWE       ForgeCode      Aider          Claude Code    OpenHands    Capy
               Droid          Gemini CLI     Codex
                              Goose          Junie CLI
                              Warp
```

Each step up the spectrum adds both capability and complexity:

| Level | Mechanism | Granularity | Cost | Recovery Scope |
|-------|-----------|-------------|------|----------------|
| None | No rollback | N/A | Zero | None |
| File backup | Copy before edit | Per-file | Low | Single file |
| Git-based | Commits, stash, reset | Repository-wide | Low | All tracked files |
| Checkpoints | Snapshot code + conversation | Full agent state | Medium | Code + context |
| Event sourcing | Replayable event stream | Arbitrary point | High | Entire system |
| VM snapshots | Full environment image | Complete environment | Very high | Everything |

The sweet spot for most agents is **git-based rollback with checkpoint augmentation**.
Git is already the natural version control system for code — leveraging it for rollback
is nearly free. Checkpoints add the ability to restore conversation state alongside
code state, which is critical for maintaining coherent multi-step reasoning.

---

## Git-Based Rollback

Git is the most natural rollback mechanism for coding agents because the codebase is
almost always a git repository. Several agents exploit this directly.

### Aider: The `/undo` Command

Aider's rollback strategy is elegant in its simplicity. Every AI edit is committed
to git automatically. The `/undo` command simply reverts the last AI commit:

```python
# Aider's git-based rollback flow (simplified from aider/commands.py)
def cmd_undo(self, args):
    """Undo the last AI edit commit."""
    if not self.coder.repo:
        self.io.tool_error("No git repository found.")
        return

    last_commit = self.coder.repo.repo.head.commit

    # Only undo commits made by aider
    if not last_commit.message.startswith("aider:"):
        self.io.tool_error("Last commit was not made by aider.")
        return

    # Soft reset to undo the commit but keep changes staged
    self.coder.repo.repo.git.reset("--soft", "HEAD~1")

    # Hard reset to discard the changes entirely
    self.coder.repo.repo.git.reset("--hard", "HEAD")

    self.io.tool_output(f"Undid commit: {last_commit.hexsha[:7]}")
```

**The "commit before edit" pattern** is key: Aider pre-commits any dirty files before
making AI edits. This ensures that user work is never lost — the undo only affects
the AI's changes, not the developer's uncommitted work.

```
Timeline:
  ┌──────────────┐     ┌──────────────┐     ┌──────────────┐
  │  User's       │     │  Auto-commit │     │  AI edit      │
  │  dirty files  │────▶│  user work   │────▶│  committed    │
  │  (uncommitted)│     │  (protected) │     │  (undoable)   │
  └──────────────┘     └──────────────┘     └──────────────┘
                                                   │
                                            /undo  │
                                                   ▼
                                            ┌──────────────┐
                                            │  Back to      │
                                            │  user's state │
                                            └──────────────┘
```

### Gemini CLI: Shadow Git for Checkpoints

Gemini CLI uses a more sophisticated git-based approach: a **shadow git repository**
that tracks changes independently of the user's main git history. This allows
checkpoint-based rollback without polluting the user's commit log.

In plan mode, Gemini CLI creates a checkpoint before potentially destructive operations.
If the operation fails or the user rejects the result, the shadow git state is used
to restore files cleanly without touching the user's actual git history.

### Common Git Rollback Commands

All git-based agents rely on a small set of git operations:

```bash
git checkout -- path/to/file.py    # Revert single file to last commit
git stash push -m "before-ai-edit" # Stash current changes
git stash pop                       # Restore stashed changes
git reset --hard HEAD~1            # Hard reset to previous commit
git reset --soft HEAD~1            # Undo commit, keep changes staged
git revert HEAD --no-edit          # Create revert commit (preserves history)
```

Most agents use `reset` (rewrites history) rather than `revert` (preserves history)
because they operate in scratch contexts where clean state matters more than history.

---

## File-Level Checkpoints

Git-based rollback operates at the repository level — it restores the entire working
tree to a previous state. But agents often need finer-grained control: restore a
single file, or restore code without affecting conversation state. This is where
**file-level checkpoints** come in.

### Claude Code: Snapshots Before Every Edit

Claude Code takes the most aggressive checkpoint approach: a file snapshot is captured
**before every single edit**. This creates a fine-grained timeline that the user can
navigate with the `/rewind` command or by pressing `Esc+Esc`.

```
Checkpoint timeline:

  CP-0       CP-1       CP-2       CP-3       CP-4
  ──●──────────●──────────●──────────●──────────●── time
  Initial    Edit       Edit       Edit       Current
  state      auth.py    db.py      auth.py    state

  /rewind to CP-2: restores auth.py and db.py to pre-CP-2 state
  Options: restore code only, conversation only, or both
```

**Persistence across sessions:** Claude Code checkpoints survive session restarts —
you can rewind to checkpoints from previous sessions.

### Codex: GhostSnapshot Survival

Codex implements checkpoints through its **GhostSnapshot** system. When undo removes
turns from conversation, GhostSnapshot items survive — enabling redo.

```typescript
// Codex's Op::Undo — simplified from codex-rs/core/src/protocol.rs
enum Op {
    /// Remove the last N conversation turns.
    /// GhostSnapshot items survive this operation.
    Undo { n_turns: usize },

    /// Deep rollback: reload from persisted rollout file,
    /// replaying events up to a specific point.
    ThreadRollback { target_turn: usize },
}

// GhostSnapshot: survives Undo, enables Redo
struct GhostSnapshot {
    file_path: PathBuf,
    content: String,
    turn_index: usize,
    // Marked as "ghost" — not part of active conversation
    // but preserved for potential redo
}

impl ConversationState {
    fn apply_undo(&mut self, n_turns: usize) {
        // Remove last N turns from active conversation
        let removed = self.turns.split_off(self.turns.len() - n_turns);

        // Ghost the file snapshots — don't delete them
        for turn in &removed {
            for snapshot in &turn.file_snapshots {
                self.ghost_snapshots.push(GhostSnapshot {
                    file_path: snapshot.path.clone(),
                    content: snapshot.content.clone(),
                    turn_index: turn.index,
                });
            }
        }

        // Restore files to pre-edit state
        self.restore_files_to_turn(self.turns.len());
    }
}
```

While `Op::Undo` handles "go back one step," Codex also supports `Op::ThreadRollback`
for deeper recovery — reloading from a **persisted rollout file** and replaying
events up to a specific turn index:

```typescript
impl ConversationState {
    fn apply_thread_rollback(&mut self, target_turn: usize) {
        // Load the full session history from the rollout file
        let rollout = RolloutFile::load(&self.rollout_path)
            .expect("Rollout file must exist for ThreadRollback");

        // Reset in-memory state
        self.turns.clear();
        self.ghost_snapshots.clear();

        // Replay events up to the target turn
        for event in rollout.events() {
            if event.turn_index > target_turn {
                break;
            }
            self.replay_event(event);
        }

        // Restore filesystem to match the target turn's state
        self.restore_files_to_turn(target_turn);
    }
}
```

ThreadRollback is more expensive than Undo (disk I/O + event replay) but provides
recovery to **any** point in the session.

### Gemini CLI: Plan Mode Checkpoints

Gemini CLI creates checkpoints in **plan mode** before potentially destructive
operations. If the operation fails or the user rejects changes, the checkpoint
enables clean restoration without manual file-by-file recovery.

---

## Conversation-Level Rollback

File rollback restores the codebase. But what about the agent's **mental state**?
A failed attempt doesn't just corrupt files — it corrupts the conversation context.
Error messages, wrong assumptions, and dead-end reasoning all accumulate in the
context window, poisoning future generations.

**Why conversation rollback matters:** The LLM generates its next action based on
the entire conversation history. If that history contains three failed attempts with
misleading error messages, the model keeps trying variations of the same broken
approach instead of trying something fundamentally different.

```
Without conversation rollback:

  Turn 1: "Edit auth.py to add JWT validation"
  Turn 2: [AI edits auth.py — introduces bug]
  Turn 3: [Tests fail: "TypeError: expected str, got bytes"]
  Turn 4: [AI tries to fix — adds .decode() in wrong place]
  Turn 5: [Tests fail: "AttributeError: 'NoneType' has no 'decode'"]
  ...
  Turn 12: [AI hopelessly lost, context polluted with 10 failed attempts]

With conversation rollback:

  Turn 1: "Edit auth.py to add JWT validation"
  Turn 2: [AI edits — introduces bug]
  Turn 3: [Tests fail]
  ──── ROLLBACK to Turn 1 + keep error insight ────
  Turn 2': [AI tries fresh approach, informed by error but not polluted]
  Turn 3': [Tests pass ✓]
```

### Codex: Structured Undo and Rollback

Codex provides two levels of conversation rollback:

- **`Op::Undo`** — removes the last N turns, keeping GhostSnapshots for potential redo
- **`Op::ThreadRollback`** — replays from the persisted rollout file to any target turn

The two-tier approach balances speed and power: Undo is instant (in-memory),
while ThreadRollback is slower but reaches any point in history.

Claude Code's `/rewind` can restore conversation state independently of code state —
useful when the agent's reasoning went wrong but its code changes were correct.

Goose captures an **initial_messages snapshot** at loop start. On repeated failures,
the RetryManager resets conversation to this initial state and retries from scratch:

```rust
// Goose's RetryManager pattern (simplified from goose/src/agent.rs)
struct RetryManager {
    initial_messages: Vec<Message>,   // Snapshot of conversation at loop start
    max_attempts: usize,              // Typically 3-5
    current_attempt: usize,
    on_failure_command: Option<String>, // e.g., "git checkout -- ."
}

impl RetryManager {
    fn new(messages: &[Message], max_attempts: usize) -> Self {
        RetryManager {
            initial_messages: messages.to_vec(),  // Clone the snapshot
            max_attempts,
            current_attempt: 0,
            on_failure_command: None,
        }
    }

    fn on_failure(&mut self, agent: &mut Agent) -> RetryDecision {
        self.current_attempt += 1;

        if self.current_attempt >= self.max_attempts {
            return RetryDecision::Escalate;
        }

        // Reset conversation to initial state
        agent.messages = self.initial_messages.clone();

        // Run cleanup command if configured (e.g., git reset)
        if let Some(cmd) = &self.on_failure_command {
            execute_shell(cmd);
        }

        // Add context about the failure for the next attempt
        agent.messages.push(Message::system(format!(
            "Previous attempt {} of {} failed. Try a different approach.",
            self.current_attempt, self.max_attempts
        )));

        RetryDecision::Retry
    }
}
```

This is a **clean-slate** recovery pattern: rather than surgically removing bad
parts of the conversation, Goose discards everything and starts over. Blunt but
effective — it guarantees no context poisoning at the cost of losing partial progress.

---

## Event Sourcing for Rollback

Event sourcing is the most powerful rollback mechanism. Rather than storing current
state and trying to reverse-engineer undo, event sourcing stores the **complete
sequence of events** that produced the state. Rollback is just replaying fewer events.

### OpenHands: The Event Stream as Rollback Mechanism

OpenHands is the most thorough implementation of event sourcing among the 17 agents.
The **entire system state** derives from a single `EventStream` — no mutable shared
state exists outside of it.

```python
# OpenHands event sourcing — simplified from openhands/events/
class EventStream:
    """Append-only log of all events in a session."""

    def __init__(self):
        self._events: list[Event] = []
        self._subscribers: list[Callable] = []

    def add_event(self, event: Event):
        event.id = len(self._events)
        event.timestamp = datetime.now()
        self._events.append(event)
        for subscriber in self._subscribers:
            subscriber(event)

    def get_events_up_to(self, event_id: int) -> list[Event]:
        """Get all events up to (inclusive) the specified ID."""
        return self._events[:event_id + 1]

    def replay_to(self, target_id: int) -> "SystemState":
        """Reconstruct system state at any point in history."""
        state = SystemState()
        for event in self.get_events_up_to(target_id):
            state.apply(event)
        return state


class SystemState:
    """State derived entirely from replaying events."""

    def apply(self, event: Event):
        match event:
            case FileWriteAction(path, content):
                self.files[path] = content
            case FileReadAction(path):
                pass  # Read actions don't change state
            case CommandAction(command, output, exit_code):
                self.last_command_result = (output, exit_code)
            case AgentThinkAction(thought):
                self.reasoning_trace.append(thought)
            case ErrorObservation(error):
                self.error_count += 1
            case _:
                pass  # Other events handled similarly
```

**Key properties:**

1. **Any point is reachable** — replay events 0..N to get state at time N
2. **Debugging is trivial** — the event log is a complete audit trail
3. **Crash recovery is free** — persist the log, replay on restart
4. **No undo logic needed** — rollback = "replay fewer events"

The tradeoff is complexity. Replaying hundreds of events to reconstruct state is
slower than maintaining a mutable checkpoint. OpenHands mitigates this with periodic
state snapshots that serve as replay acceleration points:

```
Event stream with snapshot optimization:

  Event 0  Event 1  ... Event 99  Event 100  Event 101  ... Event 150
  ─────────────────────────●──────────────────────────────────●
                           │                                   │
                      Snapshot S1                         Snapshot S2
                      (cached state                      (cached state
                       at event 99)                       at event 150)

  To reconstruct state at Event 120:
    1. Load Snapshot S1 (state at Event 99)
    2. Replay Events 100-120 only
    (Instead of replaying all 121 events from scratch)
```

---

## Automatic Revert on Test Failure

The most impactful rollback pattern is **automatic revert on test failure** — the
agent detects failing tests and automatically reverts before trying again.

### Junie CLI: Regression Detection

Junie CLI implements the most explicit version: it tracks which tests were passing
before the AI edit and detects **regressions** (previously passing tests now failing).

```
Junie's implement-verify cycle (1 of 3-5):

  1. Record baseline: tests A, B, C passing
  2. AI edits code
  3. Run tests
  4. A, B, C still passing + new tests pass → SUCCESS ✓
     A, B, C still passing + new tests fail → continue cycle
     Regression detected (A/B/C failing)    → ROLLBACK, next cycle
```

Junie typically runs 3–5 implement-verify cycles before escalating to the user.

### Goose: RetryManager with Cleanup

Goose's RetryManager combines conversation reset with an optional **on_failure command**
(typically `git checkout -- .`) that restores the filesystem to its pre-attempt state.

The pattern is:
1. Tests fail → 2. Run on_failure command → 3. Reset conversation → 4. Retry or escalate

### The Test-Fail-Revert Pattern (Generic)

```python
def implement_with_rollback(task, max_attempts=3):
    baseline = run_tests()

    for attempt in range(max_attempts):
        checkpoint = create_checkpoint()
        ai_edit(task, attempt_context={
            "attempt": attempt + 1,
            "previous_failures": get_failure_history(),
        })
        result = run_tests()

        if result.all_passing:
            return Success(result)
        if result.has_regressions(baseline):
            restore_checkpoint(checkpoint)
            continue  # Regression — revert and retry

    return Escalate("Max attempts reached")
```

---

## Graceful Degradation

When rollback and retry aren't enough, graceful degradation determines what
happens next. The goal is to fail safely rather than destructively.

### ForgeCode: Bounded Failures

ForgeCode implements a **max_tool_failure_per_turn** counter that prevents infinite
retry loops. If a tool fails more than N times in a single turn, the agent stops
attempting that tool and either tries an alternative or escalates.

```python
# ForgeCode's failure bounding (simplified)
class ToolFailureTracker:
    def __init__(self, max_failures_per_turn=3):
        self.max_failures = max_failures_per_turn
        self.failure_counts: dict[str, int] = {}

    def record_failure(self, tool_name: str) -> Action:
        self.failure_counts[tool_name] = (
            self.failure_counts.get(tool_name, 0) + 1
        )

        if self.failure_counts[tool_name] >= self.max_failures:
            return Action.BLOCK_TOOL  # Don't allow this tool again this turn
        return Action.ALLOW_RETRY
```

### OpenHands: StuckDetector with Escalation

OpenHands' StuckDetector uses three-stage escalation:

1. **Inject recovery message** — "Try a different approach"
2. **Force context condensation** — compress history to break the loop
3. **Hard termination** — stop agent, preserve state for manual recovery

### Fallback Chains

Some agents implement **fallback chains** — a sequence of increasingly aggressive
strategies when the primary approach fails:

```
Edit strategy fallback chain:

  1. Line-level edit (surgical, precise)
     │ fails
     ▼
  2. Block-level replace (replace function body)
     │ fails
     ▼
  3. Whole-file rewrite (regenerate entire file)
     │ fails
     ▼
  4. Escalate to human with diff of attempted changes
```

This pattern appears implicitly in several agents — ForgeCode may fall back from
`str_replace` to whole-file write when repeated string replacements fail.

---

## Recovery Patterns

Across the 17 agents, four distinct recovery patterns emerge:

### 1. Clean-Slate Recovery

**Discard everything. Start from scratch.** Used by Goose's RetryManager.

```
Pros: No context poisoning. Guaranteed clean state.
Cons: Loses all partial progress.
Best for: When the failure is fundamental (wrong approach entirely).
```

### 2. Partial Recovery

**Keep what works. Revert what doesn't.** Requires test-level granularity to
attribute failures to specific changes.

```
Pros: Preserves partial progress. More efficient than clean slate.
Cons: Requires attribution (which change broke which test?).
Best for: Multi-file changes where some files are correct.
```

### 3. Checkpoint Recovery

**Restore to the last known-good state.** Used by Claude Code and Codex.

```
Pros: Precise control over restoration point. Fast.
Cons: Requires checkpoint infrastructure.
Best for: Interactive sessions with exploration.
```

### 4. Replay Recovery

**Reconstruct state from the event log.** Used by OpenHands.

```
Pros: Any point in history is reachable.
Cons: Replay can be slow. Requires deterministic event application.
Best for: Complex tasks where the recovery point isn't known in advance.
```

### Comparison

| Pattern | Info Preserved | Speed | Complexity | Used By |
|---------|---------------|-------|------------|---------|
| Clean slate | None | Fast | Low | Goose |
| Partial | Some files | Medium | Medium | Junie CLI |
| Checkpoint | Full state at point | Fast | Medium | Claude Code, Codex |
| Replay | Full history | Slow | High | OpenHands |

---

## VM and Container-Level Rollback

The most extreme rollback operates at the infrastructure level: roll back the
**entire environment** rather than individual files or conversations.

### Capy: Complete VM Per Task

Capy provisions a complete Ubuntu VM for each task, destroyed after completion.
This provides the ultimate rollback — if anything goes wrong, the entire environment
can be wiped and reprovisioned from scratch.

```
Capy's VM lifecycle:

  Task received → Provision fresh VM → Agent works → Success? Extract & destroy
                                                   → Failure? Snapshot & destroy
```

The VM provides **total isolation**: the agent cannot corrupt the host, and any
damage is contained and disposable.

### OpenHands: Docker Container Isolation

OpenHands runs tool execution inside a Docker container. If the agent corrupts
the environment, the container can be restarted from its base image.

```
OpenHands container architecture:

  ┌──────────────────────────────┐
  │  Host System                  │
  │  ┌────────────────────────┐  │
  │  │  Agent Controller      │  │
  │  │  (event stream, LLM)   │──┼──── LLM API
  │  └───────────┬────────────┘  │
  │              │ tool calls     │
  │  ┌───────────▼────────────┐  │
  │  │  Docker Container      │  │
  │  │  (filesystem, shell)   │  │
  │  │  Restartable from base │  │
  │  └────────────────────────┘  │
  └──────────────────────────────┘
```

### Cloud MicroVM Providers

Several agents integrate with cloud microVM providers (E2B, Daytona) that offer
snapshot/restore as a first-class feature — snapshot entire VM state in milliseconds,
restore to any snapshot, or fork a snapshot to explore multiple approaches in parallel.
This is the gold standard for rollback but requires cloud infrastructure dependency.

---

## Design Principles for Rollback

Five principles emerge from studying how the 17 agents approach rollback:

**1. Snapshot Before Mutate** — Every effective agent captures state before changing it.
Whether it's Aider committing before edits, Claude Code snapshotting files, or Goose
capturing initial_messages — the pattern is universal.

**2. Separate Code and Conversation Rollback** — Code state and conversation state are
independent dimensions. The most flexible agents (Claude Code, Codex) allow independent
rollback of each.

**3. Bound Your Retries** — Every retry system needs a maximum attempt count. Junie CLI's
3–5 cycles, Goose's configurable max_attempts, and ForgeCode's max_tool_failure_per_turn
all enforce this bound.

**4. Carry Forward Failure Context** — When retrying after rollback, the agent should know
**why** the previous attempt failed. Without this, it may repeat the same mistake.

**5. More Checkpoints = More Options** — Claude Code's per-edit checkpoints give maximum
flexibility. Aider's per-commit checkpoints are coarser but simpler. The right granularity
depends on the agent's use case.

---

## Cross-Agent Rollback Comparison

| Agent | File Rollback | Conversation Rollback | Auto-Revert on Failure | Mechanism | Granularity |
|-------|--------------|----------------------|----------------------|-----------|-------------|
| [Aider](../agents/aider/) | ✅ `/undo` | ❌ | ❌ | Git commits | Per-commit |
| [Claude Code](../agents/claude-code/) | ✅ `/rewind` | ✅ `/rewind` | ❌ | File checkpoints | Per-edit |
| [Codex](../agents/codex/) | ✅ `Op::Undo` | ✅ `Op::ThreadRollback` | ❌ | GhostSnapshot + rollout file | Per-turn |
| [OpenHands](../agents/openhands/) | ✅ Event replay | ✅ Event replay | Partial (StuckDetector) | Event sourcing | Any event |
| [Goose](../agents/goose/) | ✅ Git reset | ✅ initial_messages reset | ✅ RetryManager | Conversation snapshot | Per-attempt |
| [Gemini CLI](../agents/gemini-cli/) | ✅ Shadow git | ❌ | ❌ | Shadow git repo | Per-checkpoint |
| [Junie CLI](../agents/junie-cli/) | ✅ Cycle rollback | Partial | ✅ Regression detection | Test-driven cycles | Per-cycle |
| [ForgeCode](../agents/forgecode/) | Partial | ❌ | ❌ | Failure bounding | Per-tool-call |
| [Capy](../agents/capy/) | ✅ VM destroy | ✅ VM destroy | ❌ | Full VM provisioning | Entire environment |
| [Warp](../agents/warp/) | ✅ Git-based | ❌ | ❌ | Git operations | Per-commit |
| [OpenCode](../agents/opencode/) | Minimal | ❌ | ❌ | Manual git | User-initiated |
| [Droid](../agents/droid/) | ✅ File backup | ❌ | ❌ | File-level backup | Per-file |
| [mini-SWE](../agents/mini-swe-agent/) | ❌ | ❌ | ❌ | None | N/A |
| [Ante](../agents/ante/) | Minimal | ❌ | ❌ | Git-based | User-initiated |
| [TONGAGENTS](../agents/tongagents/) | ❌ | ❌ | ❌ | None | N/A |
| [Sage](../agents/sage-agent/) | Minimal | ❌ | ❌ | Git-based | User-initiated |
| [Pi Coding Agent](../agents/pi-coding-agent/) | Minimal | ❌ | ❌ | Git-based | User-initiated |

---

## Summary

Rollback is the safety net that makes agentic iteration viable. The most effective
agents combine multiple levels:

1. **Git-based rollback** for file-level recovery (nearly universal)
2. **File checkpoints** for fine-grained undo within a session (Claude Code, Codex)
3. **Conversation rollback** to prevent context poisoning (Codex, Goose)
4. **Automatic revert on test failure** to close the verify-retry loop (Junie, Goose)
5. **Container/VM isolation** for catastrophic recovery (Capy, OpenHands)

**Rollback quality determines iteration quality.** Agents that can cheaply undo
failed attempts are free to try bold strategies. Agents without rollback must be
conservative or risk cascading damage. The best rollback is invisible infrastructure
that lets the agent fail fast, recover instantly, and try again.