# Human-in-the-Loop Patterns for Coding Agents

> Research into how AI coding agents balance autonomy with human oversight—the mechanisms,
> trade-offs, and design principles that keep humans in control without destroying productivity.

---

## Why Human-in-the-Loop Matters

AI coding agents can read files, write code, execute shell commands, manage git repos, and
interact with external services. This power creates real risk:

1. **Agents are powerful but fallible.** LLMs hallucinate. They misinterpret intent. They
   confidently execute the wrong plan. A single bad `rm -rf` or `git push --force` can cause
   irreversible damage that no amount of clever prompting can undo.

2. **Irreversible actions demand oversight.** File deletion, git operations, network requests,
   package installations, and database mutations are all actions where the cost of a mistake
   vastly exceeds the cost of a confirmation prompt.

3. **Users must maintain agency and understanding.** If an agent makes 50 changes across 12
   files without explanation, the user loses the ability to review, learn, or debug. The human
   becomes a passenger, not a pilot.

4. **Trust must be earned progressively.** A new user should not be asked to grant blanket
   permissions. Trust should ratchet up as the agent demonstrates competence in the current
   context—and ratchet back down when the context changes.

5. **Compliance and safety requirements.** Enterprise environments require audit trails,
   policy enforcement, and the ability to prove that a human approved critical actions.
   Headless automation needs different guardrails than interactive sessions.

```
Risk without HITL:

  User: "Clean up this project"
  Agent: rm -rf node_modules/ && rm -rf .git/ && rm -rf src/legacy/
  Result: Irreversible data loss, no undo possible
```

---

## The Spectrum of Human Involvement

Human-in-the-loop is not binary. It exists on a spectrum:

```
Full Manual     Supervised       Guided          Monitored        Full
Control         Autonomy         Autonomy        Autonomy         Autonomy
    │               │               │               │               │
    ▼               ▼               ▼               ▼               ▼
┌─────────┐   ┌─────────┐   ┌─────────┐   ┌─────────┐   ┌─────────┐
│ Human   │   │ Agent   │   │ Agent   │   │ Agent   │   │ Agent   │
│ types   │   │ suggests│   │ plans,  │   │ acts,   │   │ acts    │
│ every   │   │ human   │   │ human   │   │ human   │   │ freely, │
│ command │   │ executes│   │ approves│   │ reviews │   │ reports │
│         │   │         │   │ then    │   │ after   │   │ results │
│         │   │         │   │ agent   │   │         │   │         │
│         │   │         │   │ executes│   │         │   │         │
└─────────┘   └─────────┘   └─────────┘   └─────────┘   └─────────┘
  Aider         Copilot       Claude Code     Codex           CI/CD
  (suggest)     Chat          (plan mode)     (full-auto)     Headless
```

**Different tasks require different levels of oversight:**

| Task Type              | Appropriate Oversight    | Why                                    |
|------------------------|--------------------------|----------------------------------------|
| Reading files          | None                     | No side effects                        |
| Editing source code    | Session-scoped approval  | Reversible via git                     |
| Running tests          | Auto-approve             | Safe, read-mostly                      |
| Installing packages    | Per-action prompt        | Modifies environment                   |
| Executing shell cmds   | Risk-based prompt        | Unbounded side effects                 |
| Git push / deploy      | Always prompt            | Affects shared state                   |
| File deletion          | Always prompt + preview  | Potentially irreversible               |
| Network requests       | Always prompt            | Data exfiltration risk                 |

**The key challenge:** finding the balance where the agent is productive enough to be useful
but constrained enough to be safe. Too many prompts → prompt fatigue → users click "allow all"
→ safety theater. Too few prompts → unreviewed destructive actions → real damage.

---

## Core Pattern Categories

This research folder explores eight interconnected pattern categories. Each has a dedicated
deep-dive document.

### 1. Permission Prompts

**The primary mechanism for human oversight.** How agents ask for permission before performing
potentially dangerous actions—and how they avoid asking too often.

📄 **[permission-prompts.md](./permission-prompts.md)**

Key findings across agents:
- Claude Code uses a **5-mode permission system** with glob-pattern rules and 3-layer defense
- Codex implements a **3-valued execution policy engine** (Allow/Prompt/Forbidden) with MDM
- Goose runs a **4-inspector pipeline** including AI-powered smart approval
- OpenCode uses **Go channel synchronization** for async permission flow
- Gemini CLI uses an **event-driven confirmation bus** with multi-tier sandboxing

### 2. Plan and Confirm

**Showing plans before executing.** Agents that generate a plan, present it for review, and
only execute after human approval. This inverts the typical "act first, ask forgiveness later"
pattern.

📄 **[plan-and-confirm.md](./plan-and-confirm.md)**

Key approaches: Claude Code's `plan` mode, Codex's structured plan output, Aider's architect
mode with separate planning and coding models, and Goose's plan-based tool use.

### 3. Interactive Debugging

**Collaborative debugging sessions.** When something goes wrong, how do agents and humans
work together to diagnose and fix the problem? This covers breakpoint-aware agents,
error-driven loops, and conversational debugging.

📄 **[interactive-debugging.md](./interactive-debugging.md)**

### 4. Feedback Loops

**How agents learn from corrections.** When a human rejects an action, edits agent output, or
provides explicit feedback—how does that information propagate? Covers session-level learning,
memory files (CLAUDE.md, .goosehints), and preference persistence.

📄 **[feedback-loops.md](./feedback-loops.md)**

### 5. Undo and Rollback

**Recovering from mistakes.** Git as the universal undo mechanism, checkpoint systems,
snapshot-based rollback, and the design principle that "undo > prevent."

📄 **[undo-and-rollback.md](./undo-and-rollback.md)**

Key insight: every agent studied uses git as its primary undo mechanism. Some (Claude Code,
Codex) create automatic checkpoints. Others rely on the user's existing git workflow.

### 6. Trust Levels

**Graduated autonomy.** How agents progress from restricted to autonomous operation. Covers
per-session trust escalation, persistent trust rules, organizational trust policies, and the
challenge of trust revocation.

📄 **[trust-levels.md](./trust-levels.md)**

### 7. UX Patterns

**Terminal UX for human-agent interaction.** The TUI frameworks, rendering strategies,
keyboard interactions, and visual design patterns that make HITL interactions smooth in a
terminal environment.

📄 **[ux-patterns.md](./ux-patterns.md)**

Covers: Ink (React for terminals), Bubble Tea, charm/lipgloss, streaming markdown renderers,
diff viewers, permission dialogs, and progress indicators.

### 8. Agent Comparison

**Cross-agent comparison matrix.** A systematic comparison of HITL features across all 11
agents studied, with scoring rubrics and architectural trade-off analysis.

📄 **[agent-comparison.md](./agent-comparison.md)**

---

## Key Insights from Research

After analyzing 11 coding agents, these patterns emerge:

### 1. Permission Fatigue Is Real
Too many prompts degrades UX. Users who are prompted for every file edit start clicking
"allow" reflexively—defeating the purpose of the prompt. The best agents use **risk-based
prompting**: only interrupt for genuinely dangerous actions.

### 2. Git Is the Universal Undo Mechanism
Every agent studied uses git as its primary safety net. Some create automatic checkpoints
before changes. The implication: agents should ensure they're operating in a git repo and
should refuse destructive operations outside one.

### 3. Plan-First Approaches Improve Outcomes
Agents that show a plan before executing (Claude Code's plan mode, Aider's architect mode)
produce better results. The plan serves as both a review checkpoint and a way to align the
agent's understanding with the user's intent.

### 4. Trust Should Be Graduated, Not Binary
Binary "allow all" / "deny all" is insufficient. The best systems offer per-tool, per-path,
and per-session trust scoping. Claude Code's glob-pattern permission rules and Codex's
policy amendment system exemplify this.

### 5. The Best UX Is Invisible Until Needed
Permission prompts should be unobtrusive for safe actions and prominent for dangerous ones.
The UI should not interrupt flow for read-only operations. Streaming output should feel
natural, not jarring.

### 6. Non-Interactive Mode Is Essential for CI/CD
Every serious agent supports a headless mode for automation. The permission model must
degrade gracefully: from interactive prompts → pre-configured policies → full bypass with
audit logging.

### 7. Proactive Assistance Is an Emerging Pattern
Rather than waiting for commands, advanced agents suggest fixes, propose refactors, and
anticipate needs. This inverts the HITL model: instead of "agent proposes, human approves,"
it becomes "agent observes, agent suggests, human accepts."

### 8. Defense-in-Depth Beats Any Single Layer
The most robust agents layer multiple protection mechanisms: permission prompts + OS
sandboxing + hook systems + git checkpoints. No single layer is sufficient.

---

## Agents Analyzed

| Agent        | Language   | Permission Model          | Plan Mode | Undo Mechanism     | TUI Framework     |
|--------------|------------|---------------------------|-----------|--------------------|--------------------|
| Claude Code  | TypeScript | 5-mode + glob rules       | ✅ plan    | Git checkpoint     | Ink (React)        |
| Codex        | TypeScript | 3-valued policy engine    | ✅ plan    | Git checkpoint     | Ink (React)        |
| Goose        | Rust       | 4-inspector pipeline      | ✅ plan    | Git undo           | Ratatui            |
| Aider        | Python     | Minimal (auto-approve)    | ✅ architect | Git auto-commit  | Prompt Toolkit     |
| OpenCode     | Go         | Channel-based sync        | ❌         | Git diff           | Bubble Tea         |
| Gemini CLI   | TypeScript | Event-driven bus          | ❌         | Git checkpoint     | Ink (React)        |
| ForgeCode    | TypeScript | Session-scoped            | ❌         | Git stash          | Ink (React)        |
| Warp         | Rust       | Inline confirmation       | ✅ plan    | Terminal session    | Custom Rust TUI    |
| OpenHands    | Python     | Full sandbox              | ✅ plan    | Container snapshot | Web UI             |
| Droid        | TypeScript | Configurable              | ✅ plan    | Git                | Custom TUI         |
| Junie        | Kotlin     | IDE-integrated            | ✅ plan    | IDE undo           | IntelliJ UI        |

---

## Design Principles

These principles emerge from studying what works across agents:

### 1. Minimize Interruption, Maximize Safety
The goal is not to prompt for everything—it's to prompt for the right things. Read-only
operations should never require approval. Reversible operations should use session-scoped
approval. Only truly dangerous, irreversible actions should prompt every time.

### 2. Progressive Disclosure of Complexity
New users see simple "allow/deny" prompts. Power users can configure glob patterns,
per-tool policies, and organizational rules. The permission system grows with the user.

### 3. Sensible Defaults with Escape Hatches
Out of the box, the agent should be safe. Users who understand the risks can opt into more
autonomy. The default should never be "allow all."

### 4. Undo > Prevent
It's better to let an action happen and provide easy undo than to block the action entirely.
Git checkpoints, file snapshots, and container rollbacks all embody this principle.

### 5. Context Preservation Across Interactions
When a user approves a pattern (e.g., "allow npm test"), that approval should persist at
the right scope—session, project, or global. Re-prompting for the same action is friction
without safety benefit.

### 6. Earn Trust Through Demonstrated Reliability
An agent that consistently makes good suggestions earns the right to more autonomy. This
can be explicit (user grants permissions) or implicit (agent tracks its success rate).

### 7. Fail Open for Reads, Fail Closed for Writes
When in doubt about whether to prompt: if the action only reads state, don't prompt. If the
action modifies state, prompt. This simple heuristic handles most edge cases correctly.

### 8. Audit Everything
Even when an action is auto-approved, log it. Enterprise environments need audit trails.
Users debugging unexpected behavior need to know what the agent did and why.

---

## Research Methodology

### Sources
- **Open-source codebases**: Direct analysis of agent source code on GitHub
- **Documentation**: Official docs, READMEs, architecture guides
- **Configuration schemas**: Permission configs, policy files, settings schemas
- **User experience**: Terminal recordings, UX flows, interaction patterns

### Analysis Approach
For each agent, we examined:
1. **Permission architecture**: How permissions are defined, stored, and enforced
2. **UX flow**: What the user sees when a permission check fires
3. **Configurability**: How users customize the permission model
4. **Automation support**: How the permission model works in headless/CI mode
5. **Safety layers**: Defense-in-depth beyond just permission prompts

### Limitations
- Analysis is based on publicly available source code as of mid-2025
- Proprietary agents (Cursor, Windsurf) were not analyzed at source level
- Agent behavior may differ between versions

---

## File Index

| File                                                        | Description                                                  |
|-------------------------------------------------------------|--------------------------------------------------------------|
| [README.md](./index.md)                                    | This file—overview of the research folder                    |
| [permission-prompts.md](./permission-prompts.md)            | How agents ask for permission before actions                 |
| [plan-and-confirm.md](./plan-and-confirm.md)                | Plan presentation and approval workflows                     |
| [interactive-debugging.md](./interactive-debugging.md)      | Collaborative human-agent debugging sessions                 |
| [feedback-loops.md](./feedback-loops.md)                    | How agents learn from human corrections                      |
| [undo-and-rollback.md](./undo-and-rollback.md)             | Recovering from agent mistakes via git and snapshots         |
| [trust-levels.md](./trust-levels.md)                        | Graduated autonomy and trust escalation patterns             |
| [ux-patterns.md](./ux-patterns.md)                         | Terminal UX design for human-agent interaction                |
| [agent-comparison.md](./agent-comparison.md)                | Cross-agent comparison matrix and scoring                    |

---

## How to Use This Research

**If you're building a coding agent:**
Start with [permission-prompts.md](./permission-prompts.md) for the core safety mechanism,
then [trust-levels.md](./trust-levels.md) for graduated autonomy, and
[ux-patterns.md](./ux-patterns.md) for terminal interaction design.

**If you're evaluating agents:**
Start with [agent-comparison.md](./agent-comparison.md) for the comparison matrix, then
dive into specific pattern files for areas that matter most to your use case.

**If you're interested in the design space:**
Read this README for the overview, then explore pattern files in any order. Each is
self-contained with cross-references to related patterns.

---

*Research conducted as part of the broader coding agents architecture study. All analysis
is based on publicly available open-source code and documentation.*
