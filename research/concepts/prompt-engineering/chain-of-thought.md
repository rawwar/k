# Chain-of-Thought and Reasoning Strategies in Coding Agents

## Abstract

Chain-of-thought (CoT) reasoning — the practice of eliciting intermediate
reasoning steps from a language model before it produces a final answer — has
become a foundational technique in LLM applications. In coding agents, reasoning
serves a fundamentally different purpose than in question-answering or math
benchmarks. A coding agent must reason about *which files to read*, *what order
to make changes in*, *when to run tests*, and *how to recover from failures* —
all while managing a trajectory of tool calls that may span dozens of turns.
This document examines how the 17 studied agents incorporate chain-of-thought
and related reasoning strategies, from explicit prompt directives to extended
thinking APIs to full ReAct-style reasoning loops.

---

## 1. What Is Chain-of-Thought (CoT) Prompting?

Chain-of-thought prompting was formalized by Wei et al. (2022) in "Chain-of-
Thought Prompting Elicits Reasoning in Large Language Models." The core insight
is deceptively simple: when a language model is prompted to produce intermediate
reasoning steps before arriving at a final answer, accuracy improves
substantially on tasks that require multi-step logic.

### 1.1 The Original Formulation

The Wei et al. paper demonstrated two variants:

**Few-shot CoT** — Provide worked examples that include reasoning steps:

```
Q: Roger has 5 tennis balls. He buys 2 more cans of 3. How many does he have?
A: Roger started with 5 balls. 2 cans of 3 tennis balls each is 6 tennis balls.
   5 + 6 = 11. The answer is 11.
```

**Zero-shot CoT** — Append a simple trigger phrase without examples:

```
Q: [problem]
A: Let's think step by step.
```

Kojima et al. (2022) showed that zero-shot CoT (the "Let's think step by step"
prompt) was surprisingly effective — often rivaling few-shot CoT without
requiring hand-crafted examples. This discovery had immediate implications for
coding agents, where constructing few-shot examples of multi-step tool
orchestration is labor-intensive and brittle.

### 1.2 Why Reasoning Matters More for Coding Agents Than Chatbots

A chatbot answering "What is the capital of France?" does not need chain-of-
thought. The answer is a single retrieval step. A coding agent asked to "fix the
failing test in src/auth/" faces a cascade of decisions:

1. Which test file is failing? → Requires searching or running the test suite.
2. What does the test expect? → Requires reading the test code.
3. What does the implementation do? → Requires reading source code.
4. What is the root cause? → Requires reasoning about the gap between expected
   and actual behavior.
5. What change will fix it? → Requires generating a patch.
6. Does the fix break anything else? → Requires running the full test suite.

Each step depends on the output of previous steps, and each step involves a
tool call (search, read, edit, execute). Without explicit reasoning, the model
is likely to skip steps, make changes before understanding the problem, or fail
to verify its work. Chain-of-thought prompting — whether explicit or implicit —
is the mechanism that forces sequential reasoning through this dependency chain.

```
Chatbot:  Query ─────────────────────────────────────► Answer

Agent:    Task → Plan → Read → Analyze → Edit → Test → Verify
           │      │      │       │        │      │       │
           └Think─┘ Think─┘  Think─┘  Think─┘ Think─┘ Think─┘

Each "Think" is a reasoning trace. Without CoT, agents skip to Edit.
```

---

## 2. CoT in the Coding Agent Context

### 2.1 How Coding Agents Use Reasoning Differently

In academic CoT research, reasoning is typically a monolithic block of text
produced before a final answer. In coding agents, reasoning is *interleaved*
with actions across an extended trajectory. The agent does not reason once and
act once — it reasons, acts, observes, reasons again, and acts again, often
for dozens of cycles.

This creates a fundamentally different design space:

| Property | Academic CoT | Coding Agent CoT |
|----------|-------------|------------------|
| **Reasoning length** | 1–5 sentences | Varies per turn: 1 sentence to full paragraphs |
| **Number of reasoning steps** | 1 (before answer) | Many (before each tool call) |
| **Grounding** | Hypothetical reasoning | Grounded in tool output (file contents, test results) |
| **Verification** | None (answer is final) | Continuous (run tests, check output) |
| **Cost** | Fixed per query | Cumulative across trajectory — longer reasoning = more tokens per turn × many turns |

### 2.2 CoT Directives in System Prompts

🟡 **Observed in 4–9 agents** — Several agents embed explicit reasoning
directives in their system prompts, instructing the model to think before
acting.

**Claude Code** includes directives that shape reasoning behavior:

```
# claude-code/system-prompt (synthesized from observed behavior)
Think carefully about the user's request. Before making changes:
1. Understand the existing code structure
2. Plan your approach
3. Make changes incrementally
4. Verify your changes work
```

**Sage-Agent** emphasizes structured reasoning in its agent configuration:

```python
# sage-agent/prompts/system.py (simplified)
SYSTEM_PROMPT = """You are an expert software engineer.

When solving problems:
- Analyze the problem thoroughly before acting
- Break down complex tasks into smaller steps
- Think through edge cases before implementing
- Use chain-of-thought reasoning to work through problems step by step.
"""
```

**Codex** operates within a sandboxed execution environment where reasoning
is part of the model's natural output before tool invocations. The sandbox
creates a natural CoT loop: reason, emit tool call, observe result, reason
again — enforced by the execution environment rather than explicit directives.

**Droid** takes an implicit approach, structuring its agent loop so that the
model naturally produces reasoning before each action. The framework's turn
structure (observation → response → tool call) creates implicit CoT pressure.

### 2.3 Explicit vs. Implicit CoT Across the 17 Agents

| Agent | Explicit CoT Directive | Implicit CoT (via structure) | Notes |
|-------|----------------------|----------------------------|-------|
| **Claude Code** | ✅ | ✅ | "Think carefully" directives + extended thinking |
| **Codex** | Minimal | ✅ | Sandbox loop creates natural reasoning |
| **ForgeCode** | ✅ | ✅ | Structured reasoning in prompt engineering |
| **Droid** | Minimal | ✅ | Turn structure enforces reasoning |
| **Ante** | Minimal | ✅ | Relies on model's native reasoning |
| **OpenCode** | Minimal | ✅ | Lightweight prompt, structural CoT |
| **OpenHands** | ✅ | ✅ | Micro-agent planning with reasoning |
| **Warp** | Minimal | ✅ | Focused on action execution |
| **Gemini CLI** | Minimal | ✅ | Model-native reasoning capabilities |
| **Goose** | Minimal | ✅ | Tool-shim approach, implicit reasoning |
| **Junie CLI** | ✅ | ✅ | Planning phase with explicit reasoning |
| **Mini-SWE-Agent** | ✅ | ✅ | Single-command constraint forces reasoning |
| **Pi-Coding-Agent** | Minimal | ✅ | Lightweight prompt design |
| **Aider** | ✅ | ✅ | Architect mode separates reasoning |
| **Sage-Agent** | ✅ | ✅ | Explicit CoT emphasis in prompts |
| **TongAgents** | ✅ | ✅ | Multi-agent reasoning delegation |
| **Capy** | ✅ | ✅ | Captain/Build agent split enforces planning |

---

## 3. The ReAct Pattern (Reason + Act)

### 3.1 Theoretical Foundation

The ReAct framework, introduced by Yao et al. (2022) in "ReAct: Synergizing
Reasoning and Acting in Language Models," formalizes the interleaving of
reasoning traces with actions. The key insight is that neither pure reasoning
(CoT alone) nor pure acting (tool calls without reasoning) is optimal — the
combination outperforms both.

```
┌─────────────────────────────────────────────────────────────┐
│                     The ReAct Loop                           │
│                                                             │
│   ┌──────────┐    ┌──────────┐    ┌──────────┐             │
│   │  THINK   │───►│   ACT    │───►│ OBSERVE  │──┐          │
│   │          │    │          │    │          │  │          │
│   │ Reasoning│    │ Tool Call│    │ Tool     │  │          │
│   │ trace    │    │ execution│    │ output   │  │          │
│   └──────────┘    └──────────┘    └──────────┘  │          │
│        ▲                                         │          │
│        └─────────────────────────────────────────┘          │
│                    (loop until done)                         │
└─────────────────────────────────────────────────────────────┘
```

### 3.2 ReAct in Coding Agents

🟢 **Observed in 10+ agents** — Nearly every coding agent implements a
ReAct-like loop, even if they don't explicitly reference the paper. The
fundamental agent loop of "reason about what to do → call a tool → observe
the result → reason about what to do next" *is* ReAct.

A concrete example of a ReAct-style coding agent turn:

```
Thought: I should first run the test suite to see the failure.
Action:  bash("python -m pytest tests/ -x --tb=short 2>&1 | head -50")
Observation: FAILED tests/test_auth.py::test_login_redirect
             AssertionError: Expected 302, got 200
Thought: The test expects a redirect but gets 200. Let me read the handler.
Action:  read_file("src/auth/views.py")
Observation: [login handler returns JsonResponse instead of redirect]
Thought: The handler returns JSON, not a redirect. I need to add a
         condition for browser requests vs API requests.
Action:  edit_file("src/auth/views.py", ...)
```

### 3.3 Comparison: Pure CoT vs. Pure Action vs. ReAct

| Approach | Strength | Weakness |
|----------|----------|----------|
| **Pure CoT** (reason only) | Good analysis | Cannot verify hypotheses |
| **Pure Action** (act only) | Fast execution | Prone to wrong actions, no self-correction |
| **ReAct** (interleaved) | Grounded reasoning, self-correcting | Higher token cost per turn |

For coding agents, pure action is the most dangerous approach. Without reasoning
traces, the model may edit files before reading them, skip test verification,
or fail to diagnose errors (just retrying the same broken approach).

### 3.4 ReAct-Style Prompt Template

```python
# A ReAct-style prompt template for coding agents
REACT_SYSTEM_PROMPT = """You are an expert software engineer.

For each step:
1. THINK: Explain what you know and what you need to find out.
2. ACT: Use a tool to gather information or make a change.
3. OBSERVE: Analyze the tool's output.
4. Repeat until the task is complete.

Always read code before modifying it. Always run tests after changes.
If a test fails, reason about WHY before attempting a fix.
"""
```

This template is a simplified distillation; production agents embed these
principles across hundreds of lines of system prompt rather than in a concise
block (see [system-prompts.md](system-prompts.md) for full analysis).

---

## 4. Extended Thinking and Reasoning Tokens

### 4.1 Anthropic's Extended Thinking

Extended thinking is a model-level feature that allows Claude to perform
structured internal reasoning before generating a visible response. Unlike
CoT directives in prompts (which produce reasoning in the output text),
extended thinking creates a separate `thinking` content block that the model
uses for deeper analysis.

```python
# Enabling extended thinking via the Anthropic API
import anthropic

client = anthropic.Anthropic()
response = client.messages.create(
    model="claude-sonnet-4-20250514",
    max_tokens=16000,
    thinking={
        "type": "enabled",
        "budget_tokens": 10000  # tokens allocated for thinking
    },
    messages=[{
        "role": "user",
        "content": "Analyze this codebase and propose a refactoring plan..."
    }]
)

# Response contains both thinking and text blocks
for block in response.content:
    if block.type == "thinking":
        print(f"[Internal reasoning]: {block.thinking}")
    elif block.type == "text":
        print(f"[Visible response]: {block.text}")
```

### 4.2 How Thinking Blocks Work

The response contains sequential content blocks: first a `thinking` block with
the model's internal reasoning, then a `text` block with the visible response.
In Claude 4 models, thinking content may be provided as *summarized thinking*
— a distilled version of the model's internal reasoning rather than the raw
token-by-token trace. This reduces token consumption while preserving the
reasoning quality benefits.

### 4.3 Budget Tokens and Selection Strategy

The `budget_tokens` parameter controls how many tokens the model can allocate
to its thinking process. Choosing the right budget involves a tradeoff:

| Budget Range | Use Case | Tradeoff |
|-------------|----------|----------|
| 1,024–4,000 | Simple file edits, single-function changes | Low latency, minimal cost |
| 4,000–10,000 | Multi-file changes, debugging sessions | Moderate cost, good reasoning |
| 10,000–32,000 | Architecture decisions, complex refactoring | High cost, deep analysis |

🔴 **Observed in 1–3 agents** — **Claude Code** is the primary agent that
leverages extended thinking, dynamically adjusting the thinking budget based
on task complexity. When the user asks a simple question, thinking budget is
kept low; when the task involves complex multi-file reasoning, the budget
increases.

### 4.4 OpenAI's Reasoning Tokens

OpenAI introduced a parallel concept with reasoning models (o1, o3-mini). These
models produce internal reasoning tokens that are not directly visible in the
API response but are consumed and billed:

```python
# OpenAI reasoning with o3-mini
from openai import OpenAI

client = OpenAI()
response = client.chat.completions.create(
    model="o3-mini",
    reasoning_effort="high",  # low, medium, high
    messages=[{
        "role": "user",
        "content": "Debug this failing test and suggest a fix..."
    }]
)

# Reasoning tokens are consumed but not visible
# Usage shows both reasoning and completion tokens
print(response.usage.completion_tokens_details.reasoning_tokens)
```

The `reasoning_effort` parameter (low, medium, high) serves a similar purpose
to Anthropic's `budget_tokens` — controlling how much internal computation the
model performs before producing output.

### 4.5 Reasoning Tokens vs. Prompt-Level CoT

| Dimension | Prompt-Level CoT | Reasoning Tokens / Extended Thinking |
|-----------|-----------------|--------------------------------------|
| **Visibility** | Visible in output text | Separate block or hidden |
| **Control** | Prompt engineering | API parameter |
| **Cost** | Output tokens (billed) | Reasoning tokens (billed separately) |
| **Cacheability** | Generated text varies — breaks caching | Thinking blocks are not cached (see [prompt-caching.md](prompt-caching.md)) |
| **Quality** | Depends on prompt design | Model-optimized reasoning |
| **Model support** | Any model | Specific model families only |

For coding agents that use prompt caching aggressively, this matters: CoT text
in the model's output varies between runs and cannot be cached, whereas the
*prompt* triggering reasoning can be cached. Extended thinking sidesteps this
by moving reasoning to a dedicated block outside the cacheable prompt prefix.

---

## 5. Think-Then-Act Patterns in Agent Prompts

### 5.1 Explicit "Think Before You Act" Instructions

🟡 **Observed in 4–9 agents** — Several agents include explicit directives
telling the model to reason before taking action.

The simplest pattern:

```
Before making any changes, think through:
1. What is the current state of the code?
2. What needs to change and why?
3. What could go wrong with this change?
4. How will you verify the change works?
```

**Mini-SWE-Agent** enforces this implicitly through its single-command
constraint. By allowing only one bash command per turn, the framework forces
the model to reason carefully about which single action will be most valuable
— a structural form of think-then-act.

### 5.2 Internal Reasoning Blocks

Some agents use structured internal reasoning blocks to separate thinking from
action. The `<antThinking>` pattern (used in Anthropic models) creates a
dedicated space for reasoning that is distinct from the user-visible response:

```xml
<antThinking>
The user wants to refactor the authentication module. Current structure:
monolithic auth.py (500+ lines), 3 dependent modules, 12 tests.
Plan: 1) Read auth.py 2) Read imports 3) Split module 4) Update imports 5) Test
</antThinking>

I'll start by analyzing the current authentication module structure.
```

This pattern provides the benefits of CoT (better planning, fewer errors)
without cluttering the user-visible output with reasoning text.

### 5.3 Planning Phases in Agent Architecture

🟡 **Observed in 4–9 agents** — Several agents implement explicit planning
phases that separate reasoning from execution at the architectural level.

**OpenHands** implements micro-agent planning through its hierarchical
agent system. The delegator agent reasons about task decomposition and creates
plans that are executed by specialized sub-agents — the delegator evaluates
the task, determines whether it requires code changes or browsing, and routes
to the appropriate CodeActAgent or BrowsingAgent with a structured plan.

**ForgeCode** emphasizes structured reasoning in its tool-use guidelines,
showing that models produce better tool calls when they reason explicitly about
which tool to use: "State which file to modify and why, state what the current
code does, state what the new code should do, then use the edit tool."

### 5.4 Aider's Architect Mode

🔴 **Observed in 1–3 agents** — **Aider** implements a unique separation of
reasoning and execution through its "architect mode." In this configuration,
two different models are used:

```
┌──────────────────────────────────────────────┐
│  Architect Model (e.g., Claude Sonnet)       │
│  → Analyzes codebase, plans changes          │
│  → Does NOT write edit blocks                │
├──────────────────────────────────────────────┤
│  Editor Model (e.g., DeepSeek)               │
│  → Receives architect's plan                 │
│  → Generates precise edit blocks             │
└──────────────────────────────────────────────┘
```

This is chain-of-thought elevated to an architectural pattern: the reasoning
step and the action step are performed by *different models*, potentially
with different strengths. The architect model optimizes for analysis and
planning quality; the editor model optimizes for precise code generation.

See [model-specific-tuning.md](model-specific-tuning.md) for how different
models are selected for these roles.

### 5.5 Capy's Captain/Build Split

**Capy** implements a similar separation through its Captain and Build agents.
The Captain's system prompt explicitly constrains it to planning only — "You
are a technical architect who PLANS but never IMPLEMENTS." This hard constraint
is a form of enforced CoT: the Captain *must* reason and plan because it
literally cannot execute. Its reasoning output becomes the input for Build
agents that perform the actual code changes.

---

## 6. Tree-of-Thought and Self-Consistency

### 6.1 Tree-of-Thought (ToT)

Tree-of-Thought, introduced by Yao et al. (2023), extends chain-of-thought
by exploring multiple reasoning paths simultaneously and using evaluation
to select the most promising branch:

```
                        ┌─── Path A: Refactor into classes
                        │    Score: 7/10
          ┌── Step 1 ───┤
          │             └─── Path B: Use functional approach
          │                  Score: 5/10
Problem ──┤
          │             ┌─── Path C: Modify existing module
          └── Step 1' ──┤    Score: 8/10 ← Selected
                        │
                        └─── Path D: Create new module
                             Score: 6/10
```

### 6.2 Self-Consistency

Self-consistency (Wang et al., 2022) samples multiple CoT reasoning paths for
the same problem and takes the majority answer. For a bug fix, three out of
four sampled paths might identify the same root cause — the majority vote
increases confidence in the diagnosis.

### 6.3 Applicability to Coding Agents

🔴 **Observed in 1–3 agents** — Tree-of-thought and self-consistency are
largely theoretical for current coding agents due to practical constraints:

| Constraint | Impact |
|-----------|--------|
| **Cost** | Exploring 3–5 paths multiplies token cost by 3–5× |
| **Latency** | Each path requires full model inference |
| **Side effects** | Unlike math problems, coding actions have side effects (file writes, process starts) — exploring multiple paths requires sandboxing |
| **Evaluation** | Scoring code paths requires running tests for each, compounding latency |

Where ToT and self-consistency *could* help in coding agents:

- **Architecture decisions** — Exploring different approaches before committing
  could prevent costly backtracking.
- **Complex refactoring** — Evaluating multiple change orderings could identify
  the path with fewest intermediate breakages.
- **Ambiguous requirements** — Generating multiple interpretations and asking
  for clarification is a form of ToT.

**TongAgents** comes closest to ToT in practice through its multi-agent
discussion approach, where different agent personas may advocate for different
solutions before reaching consensus.

---

## 7. Reflection and Self-Correction Loops

### 7.1 Post-Action Reflection

🟢 **Observed in 10+ agents** — Virtually all coding agents implement some
form of reflection after tool execution. The most common pattern is checking
tool output for errors and adjusting behavior accordingly.

The basic reflection loop:

```
┌──────────────┐     ┌──────────────┐     ┌──────────────┐
│  Take Action │────►│ Check Result │────►│   Reflect    │
│  (tool call) │     │ (observe)    │     │  (reason)    │
└──────────────┘     └──────────────┘     └──────┬───────┘
                                                  │
                                    ┌─────────────┴─────────────┐
                                    │                           │
                              ┌─────▼─────┐             ┌──────▼──────┐
                              │  Success  │             │   Failure   │
                              │  Move on  │             │  Diagnose   │
                              └───────────┘             │  and retry  │
                                                        └─────────────┘
```

### 7.2 Retry-with-Reasoning Patterns

When a tool call fails, agents must decide between retrying the same action,
trying a different approach, or escalating to the user. The highest-quality
agents force explicit reasoning about *why* the failure occurred before
attempting a retry:

```python
# Reflection prompt pattern for error recovery
REFLECTION_PROMPT = """The previous action failed with this error:
{error_message}

Before retrying, answer:
1. What was I trying to do?
2. Why did it fail?
3. Should I retry the same approach or try something different?
4. What information am I missing?
"""
```

Without this reflection step, agents fall into the "retry loop" antipattern —
repeating the same failing action with minor variations, wasting tokens and
user patience. This is one of the most common failure modes in production
coding agents.

### 7.3 Verification Loops

🟢 **Observed in 10+ agents** — Testing after changes is a form of structured
reflection: the agent reasons about whether its changes achieved the desired
outcome by running tests and analyzing the results.

```python
# Verification loop pattern (common across agents)
async def edit_and_verify(agent, file_path, change):
    await agent.edit_file(file_path, change)
    result = await agent.run_tests()
    if result.passed:
        return await agent.reason(
            f"Tests passed. Is the original task fully complete?")
    else:
        return await agent.reason(
            f"Tests failed: {result.failures}\n"
            f"Was my change wrong, or is this pre-existing?")
```

### 7.4 Error Analysis Prompts

Several agents embed specific error analysis reasoning in their prompts.
**Claude Code** includes error output in the model's context for natural
interpretation. **Goose** provides structured guidance: "Read the error
carefully, consider whether it's about missing dependencies, syntax, runtime,
or permissions, fix the root cause not the symptom." **Junie CLI** implements
explicit reflection checkpoints after major operations, requiring the model to
summarize accomplishments and remaining work before proceeding.

---

## 8. Implementation Patterns and Code Examples

### 8.1 Constructing CoT Prompts for Coding Agents

A practical pattern for adding reasoning directives to a coding agent:

```python
# Building a coding agent prompt with CoT directives
def build_agent_prompt(task: str, context: dict) -> list[dict]:
    system = """You are an expert software engineer working in a codebase.

REASONING PROTOCOL:
Before each action, briefly state:
- What you know so far
- What you need to find out or change
- Why this specific action is the right next step

WORKFLOW:
1. Understand the task and existing code (read before write)
2. Plan your approach (think before act)
3. Make changes incrementally (small steps)
4. Verify each change works (test after edit)
"""
    messages = [{"role": "system", "content": system},
                {"role": "user", "content": task}]
    if context.get("repo_map"):
        messages[0]["content"] += f"\n\nRepository structure:\n{context['repo_map']}"
    return messages
```

### 8.2 Template Patterns for Think-Then-Act

A lightweight pattern that adds reasoning without excessive token overhead:

```python
# Minimal think-then-act wrapper
THINK_ACT_TEMPLATE = """## Current Task
{task_description}

## Your Approach (think first, then act)
Think: [1-2 sentences about what you'll do and why]
Act: [tool call]
"""
```

For complex tasks, a heavier template can require structured analysis:
file identification, dependency structure, change ordering, test coverage,
and risk assessment before execution begins.

### 8.3 Balancing Reasoning vs. Token Cost

Adding reasoning directives to every turn increases token consumption. The
tradeoff is significant for coding agents with long trajectories:

```
Simple task (5 turns):
  Without CoT:  ~2,000 output tokens
  With CoT:     ~3,500 output tokens (+75%)

Complex task (30 turns):
  Without CoT:  ~15,000 output tokens
  With CoT:     ~28,000 output tokens (+87%)
```

Strategies for managing this cost:

1. **Adaptive reasoning depth** — Heavier CoT for complex tasks, lighter for
   simple ones. Claude Code's dynamic thinking budget is an example.
2. **Reasoning only at decision points** — Require reasoning only before
   *consequential* actions (edits, deletions, commits), not every tool call.
3. **Structured brevity** — "Think: [1 sentence]" is more token-efficient
   than open-ended reasoning blocks.
4. **Cached reasoning** — Recurring reasoning patterns can benefit from prompt
   caching. See [prompt-caching.md](prompt-caching.md) for details.

---

## 9. Measuring the Impact of CoT in Coding Tasks

### 9.1 Benchmark Evidence

The impact of CoT on coding task performance has been measured across several
benchmarks. The improvement is most pronounced on tasks requiring multi-step
reasoning (SWE-bench) rather than single-function generation (HumanEval):

| Benchmark | Without CoT / Reasoning | With CoT / Reasoning | Improvement |
|-----------|------------------------|---------------------|-------------|
| SWE-bench Lite | ~20–30% (action-only agents) | ~35–50% (reasoning agents) | +15–20pp |
| SWE-bench Verified | ~25–35% baseline | ~40–55% with extended thinking | +15–20pp |
| HumanEval | ~85% (direct generation) | ~90% (with reasoning) | +5pp |
| MBPP | ~80% (direct generation) | ~85% (with reasoning) | +5pp |

### 9.2 When CoT Helps Most vs. When It's Wasteful

| Task Type | CoT Impact | Rationale |
|-----------|-----------|-----------|
| **Complex multi-file refactoring** | High | Many dependencies, ordering matters |
| **Debugging test failures** | High | Requires diagnosis before fix |
| **Adding new features** | Medium | Planning helps, but task is forward-only |
| **Simple single-file edits** | Low | Direct generation is often sufficient |
| **Formatting / style changes** | Minimal | Mechanical task, reasoning adds no value |
| **Dependency updates** | Low | Well-defined procedure, reasoning is overhead |

### 9.3 Token Cost vs. Accuracy Tradeoff

The relationship between reasoning investment and task accuracy follows a
diminishing-returns curve. Moderate CoT investment (explicit reasoning
directives, think-then-act patterns) provides the bulk of the accuracy
improvement. Extended thinking and high reasoning budgets offer incremental
gains at disproportionate cost — best reserved for genuinely complex tasks.

---

## 10. Cross-References

### Related Documents in This Directory

- **[system-prompts.md](system-prompts.md)** — How CoT directives are embedded
  within system prompt architectures. Section 2.3 covers behavioral constraints
  that shape reasoning patterns.

- **[model-specific-tuning.md](model-specific-tuning.md)** — Per-model
  differences in reasoning capabilities. Extended thinking is Anthropic-specific;
  reasoning tokens are OpenAI-specific. Model selection affects which CoT
  strategies are available.

- **[prompt-caching.md](prompt-caching.md)** — CoT text is generated output,
  which means it varies between runs and cannot be cached. Understanding caching
  implications is critical for agents that use heavy reasoning directives.

- **[few-shot-examples.md](few-shot-examples.md)** — Few-shot CoT is a specific
  application of few-shot prompting. The examples document covers the token
  cost and formatting challenges of including worked reasoning examples.

- **[agent-comparison.md](agent-comparison.md)** — Comparative analysis of
  reasoning strategies across all 17 agents, including quantitative benchmarks
  and architectural tradeoffs.

- **[tool-descriptions.md](tool-descriptions.md)** — Tool descriptions interact
  with CoT: well-described tools reduce the reasoning burden by making correct
  usage obvious. Poorly-described tools force the model to reason harder about
  tool selection and argument construction.

### Agent-Specific Implementations

For detailed analysis of how individual agents implement reasoning strategies,
see the agent profiles in the `../../agents/` directory. Key entries:

- **Claude Code** — Extended thinking integration, dynamic reasoning budgets
- **Aider** — Architect mode (reasoning/execution model separation)
- **Sage-Agent** — Explicit CoT emphasis in system prompts
- **Capy** — Captain/Build agent split as architectural CoT
- **OpenHands** — Micro-agent planning with delegated reasoning
- **TongAgents** — Multi-agent discussion as reasoning strategy
- **Mini-SWE-Agent** — Single-command constraint as implicit reasoning enforcer
- **ForgeCode** — Structured reasoning in tool-use prompts

---

*Last updated: July 2025*