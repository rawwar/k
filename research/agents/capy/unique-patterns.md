# Capy — Unique Patterns

> Planning/execution agent split, parallel-native design, task-based workflow, multiplayer collaboration, and a free-for-OSS commercial model.

## 1. Captain/Build Split: Hard-Enforced Planning vs Execution Separation

This is Capy's most architecturally distinctive pattern and the one most worth studying.

**The pattern**: Two separate agents with non-overlapping capabilities. Captain can plan but cannot code. Build can code but cannot ask questions.

**Why it matters**: Most coding agents use a single agent for both planning and execution, relying on prompt instructions like "think step by step before coding." Capy enforces the separation at the platform level — Captain literally lacks the tools to write code, and Build literally lacks the ability to ask questions.

**The forcing function**: Because Build cannot request clarification, Captain must produce thorough specs. Because Captain cannot write code, it doesn't skip ahead to implementation. This creates a natural quality gate that reduces wasted iterations.

**Trade-off**: The rigidity can be a disadvantage for simple tasks where a single-agent loop would be faster. Creating a spec for a one-line bug fix adds overhead. The architecture optimizes for medium-to-large tasks where upfront planning pays off.

## 2. Parallel-Native Design

**The pattern**: The entire platform is built around concurrent task execution, not sequential.

- Up to 25 concurrent jams on the Pro plan
- Each task gets its own sandboxed Ubuntu VM
- Git worktrees prevent merge conflicts between parallel branches
- The dashboard shows all active tasks simultaneously

**Why it matters**: Most coding agents are inherently sequential — one task at a time, one context window. Capy's architecture treats parallelism as the default mode, not an add-on. This maps better to how engineering teams actually work (multiple tickets in a sprint, not one at a time).

**Comparison**: Claude Code's sub-agents provide limited parallelism within a single task. Codex supports concurrent sessions. But Capy's VM-per-task model provides stronger isolation and true independence between parallel workstreams.

## 3. Task-Based Workflow (Not Tab-Based)

**The pattern**: "Think in tasks, not tabs." Each task is a first-class unit that bundles:

- The chat conversation (with Captain)
- The git branch (via worktree)
- The execution environment (VM)
- The resulting pull request

**Why it matters**: Traditional IDEs organize around files and tabs. Even AI-enhanced IDEs (Cursor, Windsurf) maintain the tab-centric metaphor. Capy organizes around **tasks** — the unit of work, not the unit of code. This aligns the IDE with project management rather than file management.

## 4. Multiplayer Collaboration

**The pattern**: Capy is collaborative by default:

- Tag teammates on tasks
- Resume issues started by others
- Shared visibility into all active tasks
- 3 seats included in the $20/month Pro plan

**Why it matters**: Most coding agents are single-player tools. Even cloud-based agents like Devin are primarily designed for individual use. Capy's multiplayer design reflects its positioning as a team platform, not a personal assistant.

## 5. Git Worktree Isolation

**The pattern**: Every task automatically uses a separate git worktree.

**Why it matters**: This is the key enabler for parallel execution. Without worktree isolation, concurrent tasks modifying the same repository would create constant merge conflicts. By giving each task its own working directory (while sharing the underlying git object store), Capy enables true parallel development on a single repository.

**Technical note**: Git worktrees are a native git feature (not Capy-specific), but Capy's automatic use of them as the default isolation mechanism is distinctive. Most other agents either don't address parallel execution or use branches without worktree isolation.

## 6. Commercial/Enterprise Focus with Free OSS Tier

**The pattern**: Capy is a commercial product with enterprise features:

- SOC 2 Type II certification (March 2026)
- Custom Enterprise pricing
- 50,000+ engineer user base
- Team-oriented pricing ($20/month for 3 seats)

But also:

- **Free for open source projects**

**Why it matters**: The free-for-OSS policy differentiates Capy from other commercial agents (Devin, Factory/Droid) that don't offer OSS-specific pricing. It also serves as a growth lever — OSS developers try Capy for free, then bring it to their commercial work.

## 7. Asynchronous Execution Model

**The pattern**: Once Build starts working, it runs fully asynchronously. The user can close the browser, work on other tasks, or go to lunch. Build will complete the task and open a PR.

**Why it matters**: This is the natural outcome of the Captain/Build split — since Build can't ask questions, there's no reason for the user to watch. This contrasts with interactive agents (Claude Code, Aider) where the user is expected to monitor and approve each step.

**Trade-off**: Less control during execution. If Build goes in the wrong direction, the user doesn't find out until the PR is ready. The mitigation is that Captain's spec should prevent this, but in practice some iteration is inevitable.

## Summary of Patterns

| Pattern | Key Insight |
|---------|------------|
| Captain/Build split | Hard capability boundaries > soft prompt instructions |
| Parallel-native | VM + worktree isolation enables true concurrency |
| Task-based workflow | Organize around work units, not files |
| Multiplayer | Team platform, not personal tool |
| Worktree isolation | Native git feature, elegantly applied |
| Commercial + free OSS | Growth lever and community goodwill |
| Async execution | Planning/execution split enables fire-and-forget |
