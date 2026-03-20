---
title: Specialist Agent Patterns
status: complete
---

# Specialist Agent Patterns

Specialist agents are purpose-built agents designed for a single, well-defined role within
a multi-agent system. Rather than giving one agent every capability and hoping it
knows when to use each, specialist agents enforce role boundaries — a planner that
literally cannot write code, a researcher that is never user-facing, a reviewer that
cannot modify files. Across every production multi-agent coding system we studied, the
most effective implementations use hard-enforced specialization over soft prompt-based
role assignments.

---

## Why Specialization Works

The argument for specialization in coding agents mirrors software engineering itself:
**separation of concerns**. A single agent asked to simultaneously plan, implement,
test, and review faces conflicting objectives. Planning benefits from breadth and
caution; implementation benefits from focus and speed; review benefits from skepticism.

Three concrete benefits emerge:

1. **Reduced context pollution** — A researcher agent can explore 50 files without those
   file contents polluting the implementer's context window
2. **Optimized model selection** — Research tasks can use cheaper/faster models (Haiku, Flash);
   implementation tasks benefit from more capable models (Opus, Sonnet)
3. **Enforceable quality gates** — When the reviewer is a separate agent, it cannot
   simply skip its own review. The system architecture enforces the quality check

---

## The Specialist Role Taxonomy

Across all coding agents we studied, specialist roles cluster into six categories:

```
┌─────────────────────────────────────────────────────────────────┐
│                     SPECIALIST ROLES                            │
├──────────────┬──────────────┬──────────────┬───────────────────┤
│   RESEARCH   │   PLANNING   │ IMPLEMENTATION│   VERIFICATION   │
│              │              │              │                   │
│  Explorer    │  Architect   │  Coder       │  Reviewer         │
│  Researcher  │  Planner     │  Implementer │  Tester           │
│  Analyst     │  Strategist  │  Builder     │  Evaluator        │
│              │              │              │  Observer          │
├──────────────┴──────────────┴──────────────┴───────────────────┤
│              CROSS-CUTTING SPECIALISTS                         │
├──────────────────────────┬────────────────────────────────────┤
│   ORCHESTRATION          │   DOMAIN-SPECIFIC                  │
│                          │                                    │
│  Coordinator             │  Security Reviewer                 │
│  Meta-Agent              │  Performance Analyst               │
│  Task Router             │  Documentation Writer              │
│                          │  Migration Specialist              │
└──────────────────────────┴────────────────────────────────────┘
```

---

## Production Implementations

### ForgeCode: The Gold Standard of Specialization

ForgeCode implements the clearest specialist pattern with three agents that have
**hard-enforced, non-overlapping capabilities**:

| Agent | Role | Access | User-Facing? | Cannot Do |
|-------|------|--------|-------------|-----------|
| **Forge** | Implementer | Read + Write | Yes (`:forge`) | Deep research |
| **Muse** | Planner | Read-only | Yes (`:muse`) | Write code, run commands |
| **Sage** | Researcher | Read-only | No (internal only) | Face users, write code |

**Why this works:** The constraints are **architectural, not advisory**. Muse literally
does not have write tools in its tool set. Sage cannot generate user-facing messages.
These are not prompt instructions that the LLM might ignore — they are hard boundaries
enforced by the runtime.

```
User types ":muse refactor the auth module"
  │
  ▼
┌─────────┐     ┌─────────┐     ┌─────────┐
│  MUSE   │────►│  SAGE   │────►│  MUSE   │
│ (plan)  │     │(research)│     │ (plan)  │
│         │     │         │     │         │
│ Read    │     │ Read    │     │ Creates │
│ codebas │     │ 50 files│     │ step-by │
│ structl │     │ analyze │     │ step    │
│         │     │ deps    │     │ plan    │
└─────────┘     └────┬────┘     └────┬────┘
                     │               │
              Returns summary   Plan passed to
              (not raw data)    Forge for execution
                                     │
                                     ▼
                               ┌─────────┐
                               │  FORGE  │
                               │ (impl)  │
                               │         │
                               │ Read +  │
                               │ Write   │
                               │ Execute │
                               └─────────┘
```

**ForgeCode's progressive reasoning budget** adds another dimension of specialization:
agents in early turns get high thinking budgets, agents in later turns (routine
implementation) get lower budgets, and verification agents get high budgets again.
This ensures expensive reasoning resources go where they matter most.

### Capy: Hard Boundaries at the Platform Level

Capy splits into exactly two specialists with **forcing functions** — architectural
constraints that make each agent better by limiting it:

| Agent | Role | Capabilities | Constraint |
|-------|------|-------------|------------|
| **Captain** | Planning | Read code, research docs, ask questions, write specs | Cannot write code |
| **Build** | Implementation | Edit files, run commands, install deps, open PRs | Cannot ask questions |

**The forcing function insight:** Because Build cannot ask clarifying questions, Captain
is forced to write thorough specs. Because Captain cannot write code, it focuses purely
on understanding the problem. These constraints aren't limitations — they're what make
each agent excellent at its role.

```python
# Capy's Captain/Build interaction (conceptual)

class Captain:
    tools = [ReadFile, SearchCode, AskUser, WriteSpec]
    # No: EditFile, RunCommand, GitCommit

    def plan(self, user_request):
        # Captain MUST be thorough — Build can't ask follow-ups
        research = self.search_code(user_request)
        clarifications = self.ask_user(ambiguities)
        return self.write_spec(research, clarifications)

class Build:
    tools = [ReadFile, EditFile, RunCommand, GitCommit, OpenPR]
    # No: AskUser, AskCaptain

    def execute(self, spec):
        # Build works autonomously from spec — no interaction
        for step in spec.steps:
            self.implement(step)
        self.run_tests()
        self.open_pr()
```

### Claude Code: Model-Tiered Specialists

Claude Code's sub-agent types represent **model-tiered specialization** — the same
underlying architecture but different models, tools, and permissions per role:

```
┌────────────────────────────────────────┐
│         MAIN AGENT (Sonnet/Opus)       │
│  Full tools, full context, full model  │
├────────────┬──────────┬────────────────┤
│ Sub-Agent  │ Sub-Agent│ Sub-Agent      │
│  EXPLORE   │  PLAN    │ GENERAL-PURPOSE│
│            │          │                │
│ Model:     │ Model:   │ Model:         │
│  Haiku     │  Parent  │  Parent        │
│            │          │                │
│ Tools:     │ Tools:   │ Tools:         │
│  Read-only │  Read    │  All           │
│  Grep,Glob │  only    │                │
│            │          │                │
│ Cost:      │ Purpose: │ Purpose:       │
│  Low       │  Analyze │  Complex tasks │
└────────────┴──────────┴────────────────┘
```

**The Haiku optimization:** Explore sub-agents use Claude Haiku — a smaller, faster,
cheaper model. For codebase research (grep files, read contents, summarize findings),
Haiku is sufficient. This means Claude Code can spawn many explore agents cheaply,
gathering information without burning expensive Sonnet/Opus tokens on routine I/O.

**Custom specialists via markdown:**

```yaml
---
name: test-writer
description: Writes comprehensive unit tests for code changes
tools: Read, Grep, Glob, Write, Bash
model: sonnet
---

You are a senior test engineer. Given a code change, write
comprehensive unit tests that cover:
- Happy path scenarios
- Edge cases and boundary conditions
- Error handling paths
- Integration points

Follow the existing test patterns in the codebase.
Use the same testing framework already in use.
```

### SageAgent: Pipeline Specialists

SageAgent's five-agent pipeline assigns each stage to a dedicated specialist class
(not just different prompts on a generic agent):

```python
# SageAgent's specialist hierarchy
class AgentBase:
    """Base class for all specialist agents"""
    def __init__(self, model, tools, config):
        self.model = model
        self.tools = tools

class TaskAnalysisAgent(AgentBase):
    """Understands requirements, identifies scope"""
    # Specialist: requirement parsing, ambiguity detection

class PlanningAgent(AgentBase):
    """Decomposes into subtasks, orders execution"""
    # Specialist: dependency analysis, task ordering

class ExecutorAgent(AgentBase):
    """Executes subtasks using tools"""
    # Specialist: tool selection, file manipulation

class ObservationAgent(AgentBase):
    """Monitors progress, provides feedback"""
    # Specialist: error detection, progress assessment
    # KEY: Can loop back to PlanningAgent

class TaskSummaryAgent(AgentBase):
    """Generates final output"""
    # Specialist: result synthesis, formatting
```

**The single feedback edge:** SageAgent's architecture has exactly one feedback loop —
from ObservationAgent back to PlanningAgent. This is a deliberate constraint:
arbitrary feedback between any two agents would create unmanageable complexity.

```
Analysis → Planning → Execution → Observation ──(loop)──► Planning
                                      │
                                      └──(done)──► Summary
```

### TongAgents: Specialist Teams (Research-Backed)

TongAgents (from Beijing Institute for General Artificial Intelligence) uses
specialist teams inspired by cognitive architecture research. While no source code
is publicly available, their Terminal-Bench 2.0 performance (80.2% with Gemini Pro)
suggests a sophisticated multi-agent system.

**Speculative architecture based on BIGAI research patterns:**

```
┌──────────────────────────────┐
│     ORCHESTRATOR             │
│  (cognitive control model)   │
├──────────────────────────────┤
│                              │
│  ┌────────┐  ┌────────┐     │
│  │Planning│  │ Domain │     │
│  │ Agent  │  │ Expert │     │
│  └────────┘  └────────┘     │
│                              │
│  ┌────────┐  ┌────────┐     │
│  │Execution│  │Verify  │     │
│  │ Agent  │  │ Agent  │     │
│  └────────┘  └────────┘     │
│                              │
└──────────────────────────────┘
```

**Model-agnostic design:** TongAgents achieves very different scores across models
(80.2% Gemini Pro vs 71.9% Claude Opus), suggesting the architecture may leverage
specific model capabilities. Specialist agents may use different models depending
on the task type.

---

## Specialist Design Patterns

### Pattern 1: Tool-Based Specialization

The simplest form — give each specialist different tools.

```python
# Researcher: read-only tools
researcher = Agent(
    name="Researcher",
    tools=[ReadFile, Grep, Glob, ListDir],
    # Cannot: EditFile, RunCommand, GitCommit
)

# Implementer: read-write tools
implementer = Agent(
    name="Implementer",
    tools=[ReadFile, EditFile, CreateFile, RunCommand],
    # Cannot: SearchWeb, QueryDatabase
)

# Tester: execution-focused tools
tester = Agent(
    name="Tester",
    tools=[ReadFile, RunCommand, RunTests],
    # Cannot: EditFile, GitCommit
)
```

**Used by:** Claude Code (explore agents get read-only tools), ForgeCode (Muse/Sage
lack write tools), Capy (Captain lacks file edit tools)

### Pattern 2: Model-Based Specialization

Different specialists use different LLM models optimized for their role.

| Task | Optimal Model Characteristics | Example |
|------|------------------------------|---------|
| Research | Fast, cheap, good at extraction | Haiku, Gemini Flash |
| Planning | Strong reasoning, broad knowledge | Opus, o1 |
| Implementation | Excellent code generation | Sonnet, GPT-4o |
| Review | Critical thinking, detail-oriented | Opus, o1 |

**Junie CLI's multi-model router** implements this dynamically:

```python
def route_to_model(task_type, complexity):
    if task_type == "complex_reasoning":
        return "claude-sonnet"    # Reasoning Model
    elif task_type == "fast_edit":
        return "gemini-flash"     # Speed Model
    elif task_type == "code_generation":
        return "best-for-language" # Coding Model
    elif task_type == "planning":
        return "reasoning-model"   # Planning Model
```

This dynamic routing produces a **6.7 percentage point improvement** on Terminal-Bench
(71.0% multi-model vs 64.3% single-model Gemini Flash).

### Pattern 3: Prompt-Based Specialization

Same tools and model, but different system prompts create behavioral specialization.

```python
# Same tools, same model — different prompts create specialists
planner = Agent(
    model="sonnet",
    tools=all_tools,
    system_prompt="""You are a senior software architect.
    NEVER write code directly. Instead:
    1. Analyze the request thoroughly
    2. Identify all affected files and components
    3. Create a step-by-step implementation plan
    4. Identify risks and edge cases"""
)

implementer = Agent(
    model="sonnet",
    tools=all_tools,
    system_prompt="""You are an expert programmer.
    Follow the provided plan exactly.
    For each step: read the file, make the change, verify.
    NEVER modify the plan or skip steps."""
)
```

**Weakness:** The LLM can ignore prompt-based constraints. ForgeCode's research
shows that programmatic enforcement (removing tools, adding runtime checks) is
strictly more reliable than prompt-based role assignment.

### Pattern 4: Permission-Based Specialization

Specialists operate in different permission sandboxes.

```
┌────────────────────────────────────────────────┐
│                PERMISSION MATRIX               │
├──────────────┬────────┬────────┬───────────────┤
│ Specialist   │ Read   │ Write  │ Execute       │
├──────────────┼────────┼────────┼───────────────┤
│ Researcher   │ ✓ All  │ ✗      │ ✗             │
│ Planner      │ ✓ All  │ ✗      │ ✗             │
│ Implementer  │ ✓ All  │ ✓ Scoped│ ✓ Sandboxed  │
│ Reviewer     │ ✓ All  │ ✗      │ ✓ Tests only  │
│ Deployer     │ ✓ All  │ ✓ All  │ ✓ All         │
└──────────────┴────────┴────────┴───────────────┘
```

**Codex CLI's file ownership semantics:** Worker agents can be assigned ownership of
specific files, preventing multiple workers from editing the same file simultaneously.

---

## Domain-Specific Specialists

Beyond the core roles (research, plan, implement, verify), some systems define
domain-specific specialists:

### Security Reviewer

```yaml
# Claude Code custom agent definition
---
name: security-auditor
description: Audits code changes for security vulnerabilities
tools: Read, Grep, Glob
model: opus
---

You are a senior security engineer. Review all code changes for:
- SQL injection and command injection vulnerabilities
- Authentication and authorization bypasses
- Data exposure (PII in logs, error messages, API responses)
- Insecure cryptographic practices
- SSRF, XSS, CSRF vulnerabilities
- Hardcoded secrets or credentials
- Insecure deserialization
- Path traversal vulnerabilities

Rate each finding: CRITICAL / HIGH / MEDIUM / LOW
Provide specific remediation for each finding.
```

### Performance Analyst

```yaml
---
name: performance-analyst
description: Analyzes code for performance implications
tools: Read, Grep, Glob, Bash
model: sonnet
---

Analyze code changes for performance implications:
- Algorithmic complexity (identify O(n²) or worse)
- Database query patterns (N+1 queries, missing indexes)
- Memory allocation patterns (unnecessary copies, leaks)
- I/O patterns (blocking calls, missing batching)
- Concurrency issues (lock contention, deadlocks)
```

### Migration Specialist

A specialist for large-scale codebase migrations (framework upgrades, language
version bumps, dependency replacements):

```python
migration_agent = Agent(
    name="MigrationSpecialist",
    instructions="""You specialize in codebase migrations.
    For each file:
    1. Identify deprecated patterns
    2. Apply the migration transformation
    3. Verify the transformation preserves behavior
    4. Document any manual steps needed""",
    tools=[ReadFile, EditFile, RunTests, Grep],
)
```

---

## Aider's Architect Mode: Two-Model Specialization

Aider implements a notable variant — **two-model specialization** where a reasoning
model describes the solution and a code-editing model translates it into file edits:

```
┌──────────────────────┐     ┌──────────────────────┐
│   ARCHITECT MODEL    │     │   EDITOR MODEL       │
│   (o1, o3, R1)       │────►│   (Sonnet, GPT-4o)   │
│                      │     │                      │
│   Describes solution │     │   Translates to      │
│   in natural language│     │   file edits         │
│                      │     │                      │
│   "Move the auth     │     │   --- a/routes/auth  │
│    middleware to a    │     │   +++ b/routes/auth  │
│    separate module"  │     │   @@ -10,5 +10,8 @@  │
└──────────────────────┘     └──────────────────────┘
```

**Benchmark impact:** o1-preview alone scored 79.7% on SWE-bench. With architect
mode (o1-preview as architect + o1-mini as editor), this jumped to **85.0%** — a
state-of-the-art result at the time.

This is not multi-agent in the traditional sense (no spawned sub-processes), but it
demonstrates the power of separating "what to do" from "how to do it" across
different model capabilities.

---

## Specialist Communication Patterns

How specialists communicate determines system effectiveness:

### Summary-Based Handoff (Most Common)

Agent A explores, produces a summary, hands it to Agent B.

```
Sage researches → "The auth module uses passport.js with local
                   and OAuth strategies. 15 routes depend on it.
                   Key files: src/auth/*.ts, src/middleware/auth.ts"
                                    │
                                    ▼
Muse plans → "Step 1: Create JWT utility in src/auth/jwt.ts
              Step 2: Replace passport.authenticate calls in 15 routes
              Step 3: Update middleware to verify JWT tokens
              Step 4: Remove passport.js dependency"
```

### Spec-Document Handoff (Capy)

Captain writes a complete specification document; Build executes from it.

### Event-Stream Communication (OpenHands)

All specialists post to a shared EventStream; each subscribes to relevant events.

### Direct Tool Invocation (Claude Code, Agents SDK)

The orchestrator calls specialists as tools — they execute and return results inline.

---

## Anti-Patterns in Specialist Design

### 1. Role Overlap

Two specialists that can both do the same thing create confusion.

**Bad:** "Code Writer" + "Code Implementer" (what's the difference?)
**Good:** "Planner" (read-only, produces spec) + "Implementer" (read-write, follows spec)

### 2. Soft Boundaries

Relying on prompts alone to enforce specialization. The LLM will eventually ignore them.

**Bad:** Prompt says "you are a reviewer, do not modify code" but the agent has EditFile tool
**Good:** The agent literally does not have EditFile in its tool set

### 3. Too Many Specialists

Diminishing returns set in quickly. The coordination overhead of 10 specialists
exceeds the benefit over 3-4 well-designed ones.

**Practical guideline:** Most effective systems use 2-5 specialist roles. Beyond that,
consider whether sub-roles can be merged or handled by a single specialist with
mode-switching.

### 4. Specialists Without Clear Interfaces

If it's not clear what a specialist receives as input and produces as output, the
orchestrator can't effectively use it.

**Every specialist should have:**
- Clear input specification (what context it needs)
- Clear output specification (what it returns)
- Clear boundary (what it cannot do)

---

## Cross-References

- [orchestrator-worker.md](./orchestrator-worker.md) — How specialists are coordinated
- [evaluation-agent.md](./evaluation-agent.md) — The evaluator/reviewer specialist in depth
- [context-sharing.md](./context-sharing.md) — How specialists share knowledge
- [real-world-examples.md](./real-world-examples.md) — Full implementations
- [agent-comparison.md](./agent-comparison.md) — Which agents use which specialist patterns

---

## References

- Anthropic. "Building Effective Agents." 2024. https://www.anthropic.com/research/building-effective-agents
- Research files: `/research/agents/forgecode/`, `/research/agents/capy/`, `/research/agents/sage-agent/`, `/research/agents/claude-code/`, `/research/agents/aider/`, `/research/agents/tongagents/`, `/research/agents/junie-cli/`
