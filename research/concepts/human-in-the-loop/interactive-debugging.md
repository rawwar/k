# Interactive Debugging with Human Assistance

> How CLI coding agents collaborate with humans to diagnose, isolate, and fix bugs—
> escalating when autonomous recovery fails and leveraging human domain knowledge
> to resolve errors that no amount of retry logic can solve alone.

---

## Overview

Debugging is inherently collaborative. When an agent encounters errors it cannot
resolve through autonomous retry—malformed stack traces, ambiguous failures,
infrastructure-level issues, or domain-specific logic errors—the human becomes a
debugging partner rather than a passive observer.

This document covers patterns for **agent-human collaborative debugging** in CLI
coding agents: how agents ask for help, when they escalate, how they surface
intermediate results, and the back-and-forth that makes interactive debugging work.

Interactive debugging intersects several concerns documented elsewhere:

- **[Permission Prompts](./permission-prompts.md)** — pausing for approval before
  dangerous debugging commands (e.g., restarting services, dropping databases)
- **[Feedback Loops](./feedback-loops.md)** — the broader loop of agent action →
  human feedback → agent adjustment
- **[UX Patterns](./ux-patterns.md)** — how debugging interactions render in the terminal

Unlike fully autonomous error recovery (covered in `../../concepts/agentic-loop/error-recovery.md`),
interactive debugging assumes the human is present and willing to engage.

---

## The Debugging Interaction Model

The collaborative debugging loop follows a predictable escalation pattern:

```
┌────────────────────────────────────────────────────────────────┐
│                 COLLABORATIVE DEBUGGING LOOP                   │
├────────────────────────────────────────────────────────────────┤
│                                                                │
│  ┌──────────┐    ┌─────────────┐    ┌──────────────┐          │
│  │  Agent   │───▶│ Agent reads │───▶│ Agent forms  │          │
│  │hits error│    │ stacktrace, │    │ hypothesis   │          │
│  └──────────┘    │ checks logs │    └──────┬───────┘          │
│                  └─────────────┘           │                  │
│                                            ▼                  │
│                                  ┌──────────────────┐         │
│                                  │ Agent attempts   │◀──┐     │
│                                  │ autonomous fix   │   │     │
│                                  └────────┬─────────┘   │     │
│                                           │             │     │
│                                 ┌─────────▼─────────┐   │     │
│                                 │  Did fix work?    │   │     │
│                                 └────┬─────────┬────┘   │     │
│                                 yes  │         │ no     │     │
│                                      ▼         ▼        │     │
│                                 ┌────────┐ ┌────────┐   │     │
│                                 │ Done ✅│ │Retry < │───┘     │
│                                 └────────┘ │limit?  │         │
│                                            └───┬────┘         │
│                                                │ no           │
│                                                ▼              │
│  ┌─────────────────────────────────────────────────────┐      │
│  │            ESCALATION TO HUMAN                      │      │
│  │  ┌────────────┐  ┌────────────┐  ┌──────────────┐  │      │
│  │  │Agent shows │─▶│Human gives │─▶│Agent retries │──┘      │
│  │  │work & asks │  │guidance    │  │with guidance │         │
│  │  └────────────┘  └────────────┘  └──────────────┘         │
│  └─────────────────────────────────────────────────────┘      │
└────────────────────────────────────────────────────────────────┘
```

The key architectural question: **when does the agent transition from autonomous
recovery to human escalation?** Different agents answer this differently.

---

## Agent Asks for Help Patterns

When agents explicitly ask the human for assistance, the request takes one of
several recognizable forms.

### "I'm Stuck" Pattern

The agent admits it cannot solve the problem and hands control back:

```typescript
// Claude Code style — conversational admission
// See ../../agents/claude-code/ for full architecture

interface StuckEscalation {
  type: "stuck";
  errorsSeen: Error[];
  fixesAttempted: FixAttempt[];
  filesExamined: string[];
  message: string;
}

// Example output:
// "I've tried three approaches to fix this TypeError but none worked:
//  1. Added null check before accessing .name property
//  2. Changed the function signature to accept optional params
//  3. Added a default value in the destructuring
//  The error persists. Could you take a look at the test setup?"
```

### Presenting Options Pattern

The agent identifies multiple possible causes and asks the human to choose:

```python
# Aider style — presenting alternatives in chat
# See ../../agents/aider/ for implementation details

def present_options(self, error, context):
    hypotheses = self.generate_hypotheses(error, context)
    print("I see two possible causes for this failure:\n")
    print("1. The database migration hasn't been run — the 'users' table")
    print("   is missing the 'email_verified' column from migration 047.")
    print("   Fix: run `python manage.py migrate`\n")
    print("2. The test fixture is outdated — it creates users without the")
    print("   'email_verified' field, which is now NOT NULL.")
    print("   Fix: update tests/fixtures/users.json\n")
    print("Which is more likely, or should I investigate further?")
```

### Requesting Context and Suggesting Diagnostics

| Pattern                      | Example Agent Query                                       | When Used                      |
|------------------------------|-----------------------------------------------------------|--------------------------------|
| Architecture question        | "How does auth work in this project?"                     | Unfamiliar codebase structure  |
| Environment question         | "What version of Node are you running?"                   | Version-specific bugs          |
| Convention question          | "Do you use barrel exports in this project?"              | Style/convention ambiguity     |
| History question             | "Was this test passing before the last refactor?"         | Regression debugging           |
| Infrastructure question      | "Is the Redis instance running locally or in Docker?"     | Connection/service errors      |
| Diagnostic request           | "Could you run `psql -c '\\d users'` and share output?"  | Agent can't access resource    |

Agents like Codex (see `../../agents/codex/`) often request diagnostics because
sandbox restrictions prevent direct access to databases or external services.

---

## Error Escalation

Agents need a principled strategy for deciding when to stop retrying and involve
the human.

### Escalation Triggers

| Trigger                 | Description                                     | Example                              |
|-------------------------|-------------------------------------------------|--------------------------------------|
| Retry limit reached     | Agent tried N fixes, all failed                 | 3 attempts to fix a type error       |
| Error outside domain    | Infrastructure, auth, network issues            | `ECONNREFUSED` to a database         |
| Ambiguous root cause    | Multiple equally likely causes                  | Test fails only on CI, not locally   |
| Circular fix pattern    | Fix for A breaks B, fix for B breaks A          | Dependency version conflicts         |
| Missing context         | Agent lacks information to diagnose             | Custom build tool, no docs           |
| Dangerous recovery      | Fix requires destructive action                 | Dropping and recreating a database   |

### Escalation Logic

```rust
// Conceptual escalation logic
// See ../../agents/claude-code/ and ../../agents/codex/ for real implementations

enum DebugState {
    Analyzing,
    AttemptingFix { attempt: u32, max_attempts: u32 },
    Escalating { reason: EscalationReason },
    WaitingForHuman,
    Resolved,
}

enum EscalationReason {
    RetryLimitReached { attempts: u32, errors: Vec<String> },
    OutsideDomain { category: String },
    CircularFix { cycle: Vec<String> },
    NeedsContext { questions: Vec<String> },
}

fn should_escalate(state: &DebugState, error: &Error, history: &[FixAttempt]) -> bool {
    if let DebugState::AttemptingFix { attempt, max_attempts } = state {
        if attempt >= max_attempts { return true; }
    }
    // Detect circular fixes — same error signature seen before
    let sig = error.signature();
    if history.iter().filter(|h| h.error_sig == sig).count() >= 2 {
        return true;
    }
    error.is_infrastructure() || error.is_auth() || error.is_network()
}
```

A good escalation provides: (1) what failed, (2) what was tried, (3) current
hypothesis, (4) a specific question. A bad escalation dumps a raw stack trace.

---

## Showing Intermediate Results

Transparency is the foundation of effective collaborative debugging. When the
human can see what the agent is doing, they can intervene early.

```
┌──────────────────────────────────────────────────────────┐
│             INTERMEDIATE RESULTS DISPLAY                  │
├──────────────────────────────────────────────────────────┤
│                                                          │
│  ┌─────────────────┐                                     │
│  │ Error Detection  │──▶ "Found TypeError in api.ts:42"  │
│  └────────┬────────┘                                     │
│           ▼                                              │
│  ┌─────────────────┐    "Reading api.ts, types.ts,       │
│  │ File Examination │──▶  handlers/user.ts..."           │
│  └────────┬────────┘                                     │
│           ▼                                              │
│  ┌─────────────────┐    "I think the error is because    │
│  │ Hypothesis       │──▶  UserInput type doesn't include │
│  └────────┬────────┘     the 'role' field from v2.3"     │
│           ▼                                              │
│  ┌─────────────────┐    "Applying fix: adding 'role'     │
│  │ Fix Attempt      │──▶  to UserInput interface..."     │
│  └────────┬────────┘                                     │
│           ▼                                              │
│  ┌─────────────────┐    "Running tests... 47/48 pass.    │
│  │ Verification     │──▶  1 failure in auth.test.ts:89"  │
│  └─────────────────┘                                     │
└──────────────────────────────────────────────────────────┘
```

Agents that stream their reasoning (Claude Code with extended thinking, Gemini CLI)
produce better collaborative debugging outcomes than agents that silently attempt
fixes. See `../../agents/claude-code/` and `../../agents/gemini-cli/`.

When an agent lists which files it examined, the human can identify gaps: "You
checked the handler but not the middleware—the auth middleware modifies the request
before it reaches the handler." This is a lightweight [Feedback Loop](./feedback-loops.md).

---

## Breakpoint-Like Pauses

Traditional debuggers let developers set breakpoints. Coding agents have analogous
patterns: configurable points where the agent pauses for human review.

### Plan Mode as a Breakpoint

Claude Code's plan mode (see `../../agents/claude-code/`) functions as a "breakpoint
before execution." The agent analyzes the error, formulates a fix, and **presents it
for approval** before applying. Valuable when the fix might have side effects.

### Permission Prompts as Debugging Breakpoints

[Permission prompts](./permission-prompts.md) naturally create breakpoints. When an
agent needs to run a diagnostic command, the permission prompt pauses execution:

```go
// Conceptual breakpoint-like pause during debugging
// See ../../agents/goose/ for Goose's permission model

func (a *Agent) debugWithBreakpoints(err error) {
    a.ReadFile("src/api.ts")              // ✅ no breakpoint
    a.GrepLogs("error", "*.log")          // ✅ no breakpoint
    a.RunCommand("npm test")              // ⚠️ BREAKPOINT — human reviews
    a.RunCommand("docker restart api")    // 🛑 BREAKPOINT — must approve
}
```

### Configurable Pause Points

| Configuration                     | Effect                                       | Agent Examples           |
|-----------------------------------|----------------------------------------------|--------------------------|
| Pause before file modifications   | Agent shows diff before applying fix         | Claude Code, OpenCode    |
| Pause before running tests        | Human reviews changes before test execution  | Aider                    |
| Pause before service restarts     | Prevent disruption to running services       | Goose, Junie CLI         |
| Pause after N failed attempts     | Force escalation after retry budget          | Codex, Droid             |
| Pause on new error type           | Stop when a different error appears          | Claude Code              |

Users can create debugging-specific breakpoints through permission rules:

```json
{
  "permissions": {
    "allow": ["Read(*)", "Grep(*)", "Bash(cat *)", "Bash(git log*)"],
    "deny": ["Bash(rm *)", "Bash(git push*)"]
  }
}
```

Everything not in the allow/deny lists prompts—creating breakpoints at file edits,
test runs, and unrecognized shell commands.

---

## Collaborative Debugging Between Human and Agent

The most effective debugging interactions are **multi-turn conversations** where
human and agent iteratively narrow down the problem.

### Sequence Diagram: API 500 Error Debugging

```
Human                              Agent                         System
  │                                  │                              │
  │  "The API returns 500            │                              │
  │   errors on /users"              │                              │
  │─────────────────────────────────▶│                              │
  │                                  │  grep logs for 500 errors    │
  │                                  │─────────────────────────────▶│
  │                                  │◀──── error log entries ──────│
  │                                  │  read src/handlers/users.ts  │
  │                                  │─────────────────────────────▶│
  │                                  │◀──── file contents ──────────│
  │  "Handler isn't validating       │                              │
  │   null inputs. Adding fix."      │                              │
  │◀─────────────────────────────────│                              │
  │  [human approves edit]           │                              │
  │─────────────────────────────────▶│                              │
  │                                  │  edit users.ts, run tests    │
  │                                  │─────────────────────────────▶│
  │                                  │◀── 47/48 pass, 1 fail ──────│
  │  "Integration test fails —       │                              │
  │   Redis connection refused."     │                              │
  │◀─────────────────────────────────│                              │
  │  "That test needs Redis.         │                              │
  │   Skip it for now."              │                              │
  │─────────────────────────────────▶│                              │
  │                                  │  npm test -- --grep unit     │
  │                                  │─────────────────────────────▶│
  │                                  │◀──── 47/47 pass ────────────│
  │  "All unit tests pass. Want      │                              │
  │   me to add integration config?" │                              │
  │◀─────────────────────────────────│                              │
```

**Key observations:** The human provided domain knowledge the agent couldn't infer
(Redis requirement). The agent provided speed—reading logs and code fast. The
conversation was iterative, and the agent adapted strategy based on human input.

This pattern appears in Claude Code (`../../agents/claude-code/`), Aider
(`../../agents/aider/`), and Goose (`../../agents/goose/`).

---

## Test Failure Investigation

Test failures are the most common debugging scenario for coding agents.

### The Agent Test-Debug Loop

```
  ┌─────────────┐
  │  Run Tests  │
  └──────┬──────┘
         ▼
  ┌──────────────┐  pass   ┌────────┐
  │ All passing? │────────▶│ Done ✅│
  └──────┬───────┘         └────────┘
         │ fail
         ▼
  ┌───────────────────┐
  │ Read failure output│──▶ assertion errors, stack traces
  └────────┬──────────┘
           ▼
  ┌───────────────────┐
  │ Trace to source   │──▶ find failing line, read context
  └────────┬──────────┘
           ▼
  ┌───────────────────┐
  │ Classify cause    │
  │  bug in code? ────│──▶ Fix the code
  │  bug in test? ────│──▶ Fix the test
  │  env issue?  ─────│──▶ Escalate to human
  └───────────────────┘
```

### Test Failure Classification

| Failure Type         | Agent Action                             | Human Needed? |
|----------------------|------------------------------------------|---------------|
| Assertion mismatch   | Read expected vs actual, trace to source | Sometimes     |
| Timeout              | Check for infinite loops, slow ops       | Often         |
| Import/module error  | Check dependencies, file paths           | Rarely        |
| Environment error    | DB connection, missing service           | Almost always |
| Flaky test           | Re-run to confirm, check race conditions | Often         |
| Compilation error    | Fix syntax/type errors                   | Rarely        |

Agents like Mini-SWE-Agent (`../../agents/mini-swe-agent/`) and Droid
(`../../agents/droid/`) are optimized for test-driven workflows.

---

## Debugging in Sandboxed Environments

Sandbox constraints create a tension: sandboxes prevent damage, but debugging
often requires the system access that sandboxes restrict.

**Codex** (`../../agents/codex/`) runs in a network-isolated sandbox:
- ✅ Can run tests, read/write files, install cached packages
- ❌ Cannot connect to databases/APIs, run debug servers, use `strace`/`gdb`

**OpenHands** (`../../agents/openhands/`) uses a full Docker container:
- ✅ Full debugging tools (gdb, strace, tcpdump), internal networking
- ❌ Higher resource footprint, risk of state mutation

```
Security ◀──────────────────────────────────────▶ Debuggability

  Codex         Claude Code       Goose         OpenHands
  (strict       (permission-      (extension-   (full
   sandbox)      gated)            based)        container)
    │               │                │               │
    ▼               ▼                ▼               ▼
  Very safe      Safe with        Flexible       Maximum debug
  but limited    oversight        but complex    power, higher risk
```

Capy (`../../agents/capy/`), ForgeCode (`../../agents/forgecode/`), and Warp
(`../../agents/warp/`) each find different positions on this spectrum.

---

## Error Context Enrichment

Before escalating, effective agents gather context around the error:

```python
# Conceptual context enrichment pipeline
# See ../../agents/claude-code/ and ../../agents/openhands/

class ErrorContextEnricher:
    def enrich(self, error):
        context = EnrichedError(original=error)
        context.source_code = self.read_file_around(error.file, error.line, 20)
        context.recent_changes = self.git_log(error.file, max_commits=5)
        context.test_files = self.find_tests_for(error.file)
        if error.type == "ImportError":
            context.deps = self.check_package_versions(error.module_name)
        context.similar_issues = self.grep_history(error.message, max_results=3)
        return context
```

| Strategy                  | Claude Code | Aider | Codex | OpenHands | Goose |
|---------------------------|:-----------:|:-----:|:-----:|:---------:|:-----:|
| Read surrounding code     | ✅          | ✅    | ✅    | ✅        | ✅    |
| Check recent git changes  | ✅          | ✅    | ❌    | ✅        | ✅    |
| Find related tests        | ✅          | ⚡    | ✅    | ✅        | ⚡    |
| Check dependency versions | ✅          | ⚡    | ✅    | ✅        | ⚡    |
| Search project history    | ✅          | ❌    | ❌    | ✅        | ⚡    |

✅ = Actively does this · ⚡ = Does when prompted · ❌ = Not observed

Well-enriched errors let the human give targeted guidance instead of asking
"can you show me the code?"

---

## The "Show Your Work" Pattern

Debugging transparency means showing the **reasoning process**, not just results.

### Extended Thinking in Debugging

Agents with chain-of-thought capabilities produce visible reasoning traces:

```
[thinking] TypeError: Cannot read property 'id' of undefined at order.ts:87
[thinking] Line 87: `const orderId = order.id` — so `order` is undefined
[thinking] Tracing call chain... src/routes/orders.ts:23 → getOrder(req.params.orderId)
           → src/services/orders.ts:45 → DB query using findOne()
[thinking] findOne() returns null when no record found. Handler doesn't check for null.
[thinking] Fix: add null check in handler, return 404. This matches the pattern
           I see in src/handlers/user.ts:52
```

This lets the human verify reasoning, learn the codebase, and redirect early.

### Streaming vs Batched Output

| Approach  | Description                                       | Agent Examples             |
|-----------|---------------------------------------------------|----------------------------|
| Streaming | Reasoning appears in real-time                    | Claude Code, Gemini CLI    |
| Batched   | Agent works silently, presents summary            | Codex, Ante                |
| Hybrid    | Key milestones streamed, details on request       | Goose, Junie CLI           |

Streaming is preferred for debugging—it enables real-time intervention. Pi Coding
Agent (`../../agents/pi-coding-agent/`) and Sage Agent (`../../agents/sage-agent/`)
vary in streaming granularity.

---

## Comparison Table: Debugging Capabilities Across Agents

| Capability                       | Claude Code | Aider | Codex | OpenHands | Goose | Gemini CLI | Junie CLI |
|----------------------------------|:-----------:|:-----:|:-----:|:---------:|:-----:|:----------:|:---------:|
| Autonomous test-fix loop         | ✅          | ✅    | ✅    | ✅        | ✅    | ✅         | ✅        |
| Explicit "I'm stuck" escalation  | ✅          | ✅    | ⚡    | ✅        | ✅    | ⚡         | ⚡        |
| Presents diagnostic options      | ✅          | ✅    | ❌    | ✅        | ⚡    | ⚡         | ⚡        |
| Streams reasoning/thinking       | ✅          | ❌    | ❌    | ✅        | ⚡    | ✅         | ❌        |
| Plan mode (pre-exec review)      | ✅          | ❌    | ❌    | ❌        | ❌    | ⚡         | ❌        |
| Error context enrichment         | ✅          | ⚡    | ✅    | ✅        | ⚡    | ✅         | ⚡        |
| Multi-turn debug conversations   | ✅          | ✅    | ⚡    | ✅        | ✅    | ✅         | ✅        |

✅ = Native support · ⚡ = Partial/prompted · ❌ = Not observed

### All 17 Agents

| Agent           | Primary Debugging Style        | Escalation Pattern          | Notable Feature                |
|-----------------|--------------------------------|-----------------------------|--------------------------------|
| Aider           | Chat-based, human-directed     | Asks in chat                | `/run` command for test exec   |
| Ante            | Autonomous with summaries      | Batched failure report      | Parallel file analysis         |
| Capy            | Conversational                 | Context-requesting          | Lightweight runtime            |
| Claude Code     | Deep reasoning, multi-turn     | Structured escalation       | Extended thinking traces       |
| Codex           | Sandboxed test loop            | Sandbox-limit escalation    | Full environment snapshots     |
| Droid           | Test-driven                    | Retry-limit based           | CI integration focus           |
| ForgeCode       | Task-oriented                  | Task-failure based          | Structured task decomposition  |
| Gemini CLI      | Streaming reasoning            | Conversational              | Google ecosystem integration   |
| Goose           | Extension-driven               | Plugin-aware escalation     | Extensible tool system         |
| Junie CLI       | IDE-integrated                 | IDE notification            | JetBrains integration          |
| Mini-SWE-Agent  | Minimal, script-driven         | Stdout-based signaling      | Minimal footprint              |
| OpenCode        | Terminal-native                | Prompt-based                | TUI interface                  |
| OpenHands       | Full container, deep access    | Multi-agent delegation      | Container debugging tools      |
| Pi Coding Agent | Cloud-hosted                   | API-based escalation        | Remote execution model         |
| Sage Agent      | Research-oriented              | Structured reporting        | Multi-model reasoning          |
| TongAgents      | Multi-agent collaboration      | Inter-agent escalation      | Agent team coordination        |
| Warp            | Terminal-integrated            | Inline terminal escalation  | Warp terminal integration      |

---

## Design Recommendations

### 1. Default to Transparency

Stream the debugging process. At minimum, surface which files are being read,
what hypothesis is forming, what fix is being attempted, and whether it worked.

### 2. Implement Graduated Escalation

```yaml
escalation_policy:
  max_autonomous_retries: 3
  escalation_triggers:
    - retry_limit_reached
    - circular_fix_detected
    - infrastructure_error
    - confidence_below_threshold
  escalation_content:
    - original_error
    - fixes_attempted
    - current_hypothesis
    - specific_question_for_human
```

### 3. Classify Errors Before Acting

Infrastructure errors should escalate immediately. Syntax errors should be fixed
autonomously. Type errors deserve one or two attempts before escalation.

### 4. Make Breakpoints Configurable

```json
{
  "debugging": {
    "pause_before": ["file_edit", "test_run", "service_restart"],
    "auto_approve": ["file_read", "grep", "git_log"],
    "always_escalate": ["database_migration", "deployment"]
  }
}
```

### 5. Enrich Errors Before Escalating

Before asking the human, the agent should have already read failing code and
context, checked recent git history, identified related tests, and searched
for similar past errors.

### 6. Support Multi-Turn Conversations

Debugging conversations must maintain context across turns. This requires either
full conversation history in context (Claude Code, Aider) or session state
management (Goose, OpenHands).

### 7. Separate "Bug in Code" from "Bug in Test"

When a test fails, explicitly ask: "Should I fix the code to match the test, or
update the test to match the current behavior?"

### 8. Respect Sandbox Boundaries Gracefully

When sandbox constraints prevent debugging, explain what cannot be done and why,
suggest alternatives the human can take, and ask for the needed information.

---

*This analysis covers interactive debugging patterns as observed in publicly available
open-source CLI coding agents as of mid-2025. Agent capabilities, debugging strategies,
and escalation behaviors may change between versions. For related topics, see
[Permission Prompts](./permission-prompts.md), [Feedback Loops](./feedback-loops.md),
and [UX Patterns](./ux-patterns.md).*
