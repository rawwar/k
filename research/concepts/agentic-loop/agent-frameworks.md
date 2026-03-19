# Agent Frameworks

## Overview

Frameworks provide reusable infrastructure for building agentic loops — the perceive→think→act
cycle that powers modern AI coding agents. They abstract away the boilerplate of tool dispatch,
state management, error recovery, and multi-turn orchestration, letting developers focus on the
agent's behavior rather than its plumbing.

The ecosystem ranges from minimalist libraries (Smolagents, ~1,000 LOC) to full-featured platforms
(LangGraph with checkpointing, time-travel, durable execution). Each makes different trade-offs
along axes that matter deeply for production systems:

- **Abstraction level**: thin wrappers vs. opinionated runtimes
- **State management**: in-memory vs. durable, checkpointed state
- **Multi-agent support**: single agent vs. crew/conversation/hierarchical patterns
- **Paradigm**: graph-based, role-based, conversational, code-as-action, type-safe
- **Language**: Python-dominated, but TypeScript (Mastra) and polyglot (Google ADK) emerging

The key question for any team building a coding agent: **build from scratch vs. use a framework?**
The answer is nuanced. The most successful production coding agents (Claude Code, Codex CLI, Goose,
Aider) built their loops from scratch. But frameworks are catching up, and for prototyping,
experimentation, and standard patterns, they provide enormous leverage. The coding agent space has
largely built from scratch — but the gap is closing as frameworks mature.

---

## LangGraph — Graph-Based State Machines

### Philosophy

LangGraph models agents as **directed cyclic graphs**. Each node is a computation step (call the
LLM, execute a tool, evaluate a condition), and each edge is a transition between steps. Cycles
are first-class — critical for agentic loops that iterate until convergence.

Built by the LangChain team, LangGraph is the most popular agent framework by adoption. It
deliberately separates itself from LangChain's chain abstraction, recognizing that agents need
**cycles** (loops), not just **chains** (linear pipelines). The graph abstraction gives developers
explicit control over the flow while the runtime handles checkpointing, persistence, and
fault recovery.

The core insight: most agent behaviors can be modeled as state machines. A coding agent that
plans → codes → tests → debugs is a four-node graph with conditional edges. LangGraph makes
this structure explicit and inspectable.

### Key Features

- **Checkpointing**: Automatic state snapshots at each node transition. Every intermediate
  state is persisted, enabling inspection, replay, and recovery. Uses pluggable checkpoint
  backends (memory, SQLite, PostgreSQL).

- **Time-travel debugging**: Navigate to any previous checkpoint, inspect the full state
  (messages, tool results, intermediate values), modify it, and resume execution from that
  point. Invaluable for debugging why an agent took a wrong turn.

- **Human-in-the-loop**: Pause execution at any node, present state to a human for review
  or modification, then resume. Supports `interrupt_before` and `interrupt_after` node
  configurations for precise control over when humans intervene.

- **Durable execution**: Persist agent state through process crashes, deployments, and
  infrastructure failures. The agent resumes from its last checkpoint, not from scratch.
  Critical for long-running coding tasks that may take minutes or hours.

- **Subgraph composition**: Nest graphs within graphs. A high-level orchestrator graph can
  delegate to specialized subgraphs (one for code generation, one for testing, one for
  code review). Each subgraph maintains its own state while the parent coordinates.

- **Streaming**: First-class support for streaming tokens, tool calls, and state updates.
  Supports both `stream_mode="values"` (full state after each node) and
  `stream_mode="updates"` (incremental changes).

### Architecture

```python
from langgraph.graph import StateGraph, END
from langgraph.checkpoint.memory import MemorySaver
from typing import TypedDict, Annotated
import operator

class AgentState(TypedDict):
    messages: Annotated[list, operator.add]
    plan: str
    code: str
    test_results: str
    iteration: int

def plan_node(state: AgentState) -> dict:
    """Analyze the task and produce a plan."""
    response = llm.invoke([
        SystemMessage("You are a planning agent. Analyze the task and produce a step-by-step plan."),
        *state["messages"]
    ])
    return {"plan": response.content, "messages": [response]}

def code_node(state: AgentState) -> dict:
    """Generate or modify code based on the plan."""
    response = llm.invoke([
        SystemMessage(f"Implement this plan:\n{state['plan']}"),
        *state["messages"]
    ])
    return {"code": response.content, "messages": [response]}

def test_node(state: AgentState) -> dict:
    """Run tests against the generated code."""
    result = execute_tests(state["code"])
    return {"test_results": result, "messages": [HumanMessage(f"Test results: {result}")]}

def debug_node(state: AgentState) -> dict:
    """Analyze test failures and suggest fixes."""
    response = llm.invoke([
        SystemMessage("Analyze the test failure and suggest a fix."),
        HumanMessage(f"Code:\n{state['code']}\n\nTest results:\n{state['test_results']}")
    ])
    return {"messages": [response], "iteration": state["iteration"] + 1}

def should_test(state: AgentState) -> str:
    """Decide whether to test or finish."""
    if "no changes needed" in state["code"].lower():
        return "done"
    return "test"

def test_result(state: AgentState) -> str:
    """Route based on test results."""
    if "PASSED" in state["test_results"]:
        return "pass"
    if state["iteration"] >= 5:
        return "pass"  # Give up after 5 iterations
    return "fail"

# Build the graph
graph = StateGraph(AgentState)
graph.add_node("plan", plan_node)
graph.add_node("code", code_node)
graph.add_node("test", test_node)
graph.add_node("debug", debug_node)

graph.set_entry_point("plan")
graph.add_edge("plan", "code")
graph.add_conditional_edges("code", should_test, {"test": "test", "done": END})
graph.add_conditional_edges("test", test_result, {"pass": END, "fail": "debug"})
graph.add_edge("debug", "code")

# Compile with checkpointing
app = graph.compile(checkpointer=MemorySaver())

# Execute with thread-level persistence
config = {"configurable": {"thread_id": "coding-session-1"}}
result = app.invoke(
    {"messages": [HumanMessage("Fix the authentication bug in auth.py")], "iteration": 0},
    config=config
)
```

### For Coding Agents

LangGraph is a natural fit for coding agents because the plan→code→test→debug loop maps
directly to a graph with conditional edges:

- **Checkpoint-based undo**: If the agent makes a bad edit, rewind to a previous checkpoint
  and try a different approach. This mirrors how developers use version control.
- **Subgraph delegation**: Model sub-agents (file search agent, test runner agent, code
  review agent) as subgraphs that the main orchestrator invokes.
- **Iteration limits**: Conditional edges can enforce maximum iteration counts, preventing
  infinite loops that burn tokens.
- **Observability**: Every node transition is logged, making it easy to trace why the agent
  chose to debug vs. ship.

### Community & Adoption

- ~25K GitHub stars (as of early 2025)
- Python and JS/TS implementations (separate packages, shared concepts)
- LangSmith integration for production observability, tracing, and evaluation
- LangChain ecosystem integration (retrievers, tools, model adapters)
- Active Discord community, extensive documentation, LangChain Academy courses
- Used in production at enterprises; LangGraph Cloud for managed deployment

---

## CrewAI — Role-Based Multi-Agent

### Philosophy

CrewAI organizes agents into **"crews"** — teams of specialists that collaborate to accomplish
tasks. Each agent has a defined **role**, **goal**, and **backstory** that shapes its behavior.
This role-based metaphor draws from organizational design: just as a software team has a product
manager, architect, developer, and tester, a CrewAI crew assigns distinct personas to each agent.

The key insight is that **role specialization improves output quality**. A single agent trying to
plan, code, test, and review will produce worse results than four specialized agents that each
focus on what they do best. CrewAI makes this specialization explicit and configurable.

### Key Features

- **YAML-configured agent teams**: Define agents, their roles, goals, and tools in YAML files.
  Swap team compositions without changing code. This declarative approach makes experimentation
  trivial — try different role definitions, tool assignments, and process flows.

- **Sequential process**: Agents run in order, each receiving the output of the previous agent.
  Simple, predictable, easy to debug. Best for linear workflows like research → draft → review.

- **Hierarchical process**: A manager agent receives the task and delegates to workers. The
  manager decides which agent handles what, collects results, and synthesizes the final output.
  Better for complex tasks requiring dynamic routing.

- **CrewAI Flows**: Deterministic event-driven workflows that orchestrate crews with explicit
  control flow. Flows add `@listen`, `@start`, `@router` decorators for structured execution
  that goes beyond simple sequential/hierarchical patterns.

- **Memory systems**: Short-term (within task), long-term (across tasks, RAG-based), and entity
  memory (knowledge about specific entities). Enables learning from past interactions.

- **Delegation**: Agents can delegate sub-tasks to other agents, creating dynamic collaboration
  patterns beyond the predefined process flow.

### Architecture

```python
from crewai import Agent, Task, Crew, Process
from crewai.tools import tool

@tool
def read_file(path: str) -> str:
    """Read a file from the project."""
    with open(path) as f:
        return f.read()

@tool
def write_file(path: str, content: str) -> str:
    """Write content to a file."""
    with open(path, 'w') as f:
        f.write(content)
    return f"Written to {path}"

@tool
def run_tests(test_path: str) -> str:
    """Execute tests and return results."""
    result = subprocess.run(["pytest", test_path, "-v"], capture_output=True, text=True)
    return result.stdout + result.stderr

# Define specialized agents
researcher = Agent(
    role="Code Researcher",
    goal="Understand the codebase structure and find relevant files",
    backstory="You are an expert at navigating large codebases and understanding code architecture.",
    tools=[read_file],
    verbose=True
)

coder = Agent(
    role="Senior Developer",
    goal="Write clean, well-tested code that follows project conventions",
    backstory="You are a senior developer who writes production-quality code.",
    tools=[read_file, write_file],
    verbose=True
)

reviewer = Agent(
    role="Code Reviewer",
    goal="Find bugs, security issues, and suggest improvements",
    backstory="You are a meticulous code reviewer focused on correctness and security.",
    tools=[read_file, run_tests],
    verbose=True
)

# Define tasks
research_task = Task(
    description="Analyze the auth module and identify the root cause of the login bug.",
    expected_output="Root cause analysis with file paths and line numbers.",
    agent=researcher
)

code_task = Task(
    description="Fix the identified bug and update related code.",
    expected_output="Modified code files with the bug fix applied.",
    agent=coder
)

review_task = Task(
    description="Review the fix for correctness, security, and test coverage.",
    expected_output="Review report with approval or requested changes.",
    agent=reviewer
)

# Assemble and run the crew
crew = Crew(
    agents=[researcher, coder, reviewer],
    tasks=[research_task, code_task, review_task],
    process=Process.sequential,
    verbose=True
)

result = crew.kickoff()
print(result)
```

### CrewAI Flows — Structured Orchestration

```python
from crewai.flow.flow import Flow, listen, start, router

class CodingFlow(Flow):
    @start()
    def analyze_task(self):
        """Entry point: understand what needs to be done."""
        return research_crew.kickoff(inputs={"task": self.state["task"]})

    @router(analyze_task)
    def route_by_complexity(self):
        """Route based on task complexity."""
        if self.state["complexity"] == "simple":
            return "quick_fix"
        return "full_implementation"

    @listen("quick_fix")
    def apply_quick_fix(self):
        return fix_crew.kickoff(inputs=self.state)

    @listen("full_implementation")
    def implement_feature(self):
        return implementation_crew.kickoff(inputs=self.state)
```

### For Coding Agents

- **Natural team mapping**: Planner/coder/reviewer/tester maps directly to a crew with
  four specialized agents. Each focuses on its strength.
- **YAML experimentation**: Quickly test different role definitions, backstories, and tool
  assignments to find optimal team compositions.
- **Flows for structure**: CrewAI Flows add the deterministic control flow that pure
  sequential/hierarchical processes lack. Route tasks based on complexity, type, or risk.
- **Delegation chains**: The coder agent can delegate to a test-writer agent, which can
  delegate to a test-runner agent — mirroring real development workflows.

---

## AutoGen — Multi-Agent Conversation

### Philosophy

AutoGen, from Microsoft Research, models multi-agent collaboration as **conversation**. Agents
communicate by sending messages to each other, and the back-and-forth dialogue produces better
results than single-shot generation. This conversational paradigm is grounded in research showing
that multi-turn refinement reduces errors and improves solution quality.

The framework has evolved through several versions. AutoGen 0.4+ introduced a layered
architecture: **Core** (message passing, event system), **AgentChat** (high-level multi-agent
patterns), and **Extensions** (integrations, tools, model clients).

### Key Features

- **Layered architecture**: Core provides the message-passing primitives. AgentChat builds
  high-level patterns on top. Extensions add integrations. You choose your abstraction level.

- **AgentTool pattern**: Wrap an entire agent as a callable tool for another agent. This enables
  hierarchical agent compositions where a coordinator agent invokes specialist agents as if
  they were tools. Powerful for building complex coding workflows.

- **Group chat**: Multiple agents participate in a shared conversation. A GroupChatManager
  controls turn-taking (round-robin, random, or LLM-selected next speaker). Agents can see
  the full conversation history and build on each other's contributions.

- **Code execution**: Built-in support for executing generated code in Docker containers or
  local processes. The UserProxyAgent can automatically execute code blocks from assistant
  messages and return the results, creating a tight generate→execute→refine loop.

- **AutoGen Studio**: A GUI application for visually building, testing, and debugging
  multi-agent systems. Drag-and-drop agent configuration, real-time conversation monitoring,
  and interactive debugging.

- **Termination conditions**: Flexible conditions for ending conversations — max turns, text
  match, token limit, or custom functions. Prevents infinite agent conversations.

### Architecture

```python
from autogen import AssistantAgent, UserProxyAgent, GroupChat, GroupChatManager

# Configuration for the LLM
llm_config = {
    "model": "gpt-4",
    "temperature": 0,
    "config_list": config_list
}

# Create specialized agents
planner = AssistantAgent(
    name="Planner",
    system_message="""You are a software architect. Analyze tasks and create implementation plans.
    Break complex tasks into clear, ordered steps.""",
    llm_config=llm_config
)

coder = AssistantAgent(
    name="Coder",
    system_message="""You are an expert programmer. Write clean, well-documented code.
    Always include error handling and type hints.""",
    llm_config=llm_config
)

tester = AssistantAgent(
    name="Tester",
    system_message="""You are a QA engineer. Write comprehensive tests and verify correctness.
    Focus on edge cases and error conditions.""",
    llm_config=llm_config
)

# UserProxy executes code and provides human feedback
executor = UserProxyAgent(
    name="Executor",
    human_input_mode="NEVER",
    code_execution_config={
        "work_dir": "workspace",
        "use_docker": True,  # Sandboxed execution
        "timeout": 60
    },
    max_consecutive_auto_reply=10,
    is_termination_msg=lambda x: "TASK_COMPLETE" in x.get("content", "")
)

# Two-agent chat (simplest pattern)
executor.initiate_chat(
    coder,
    message="Fix the race condition in the connection pool (pool.py, line 42)."
)

# Group chat (multi-agent collaboration)
group_chat = GroupChat(
    agents=[planner, coder, tester, executor],
    messages=[],
    max_round=20,
    speaker_selection_method="auto"  # LLM decides who speaks next
)

manager = GroupChatManager(groupchat=group_chat, llm_config=llm_config)
executor.initiate_chat(manager, message="Implement caching for the API responses.")
```

### The AgentTool Pattern

```python
from autogen import register_function

# Wrap the tester agent as a tool the coder can invoke
def run_code_review(code: str) -> str:
    """Have the tester review and test this code."""
    result = executor.initiate_chat(
        tester,
        message=f"Review and test this code:\n```python\n{code}\n```"
    )
    return result.summary

# Register as a tool for the coder agent
register_function(
    run_code_review,
    caller=coder,
    executor=executor,
    description="Submit code for review and testing"
)
```

### For Coding Agents

- **Conversational debugging**: When code fails tests, agents discuss the failure, propose
  hypotheses, and iteratively refine the fix. This mimics pair programming.
- **AgentTool composition**: Build hierarchical agent setups where a coordinator invokes
  specialist agents (code search, code review, test generation) as tools.
- **Docker sandboxing**: Executing generated code in Docker containers provides security
  isolation — critical for coding agents that run arbitrary code.
- **Group dynamics**: Different speaker selection methods (round-robin for structured workflows,
  LLM-selected for dynamic collaboration) support varied interaction patterns.

---

## Smolagents — Minimalist (Hugging Face)

### Philosophy

Smolagents takes the opposite approach to framework complexity: **maximum simplicity**. The entire
library is approximately 1,000 lines of code. Its key innovation is **code-as-action**: instead
of the standard JSON tool-calling pattern (where the LLM outputs a JSON object specifying which
tool to call with what arguments), the LLM writes executable Python code as its action.

This approach yields ~30% fewer steps than traditional tool-calling. Why? Because a single line
of Python can express what would require 3-4 sequential tool calls in JSON format. Python is
inherently more expressive for composing operations, handling conditionals, and processing data.

Built by Hugging Face, Smolagents reflects their philosophy of open, simple, hackable tools.

### Architecture

```python
from smolagents import CodeAgent, ToolCallingAgent, tool, HfApiModel

@tool
def read_file(path: str) -> str:
    """Read a file from the project directory.

    Args:
        path: relative path to the file
    """
    with open(path) as f:
        return f.read()

@tool
def write_file(path: str, content: str) -> str:
    """Write content to a file.

    Args:
        path: relative path to the file
        content: the content to write
    """
    with open(path, 'w') as f:
        f.write(content)
    return f"Successfully wrote {len(content)} chars to {path}"

@tool
def run_command(command: str) -> str:
    """Execute a shell command and return the output.

    Args:
        command: the shell command to run
    """
    result = subprocess.run(command, shell=True, capture_output=True, text=True, timeout=30)
    return f"stdout: {result.stdout}\nstderr: {result.stderr}\nreturn code: {result.returncode}"

model = HfApiModel("Qwen/Qwen2.5-Coder-32B-Instruct")

# CodeAgent: the LLM writes Python code as its action
# Instead of {"tool": "read_file", "args": {"path": "main.py"}},
# the LLM writes: content = read_file("main.py")
agent = CodeAgent(
    tools=[read_file, write_file, run_command],
    model=model,
    max_steps=10
)

result = agent.run("Fix the TypeError in utils.py by reading the file, identifying the bug, and applying a fix")

# ToolCallingAgent: traditional JSON tool-calling for comparison
json_agent = ToolCallingAgent(
    tools=[read_file, write_file, run_command],
    model=model,
    max_steps=15  # Needs more steps for the same task
)
```

### Code-as-Action vs JSON Tool-Calling

```
# JSON tool-calling (traditional approach) — 4 steps:
Step 1: {"tool": "read_file", "args": {"path": "utils.py"}}
Step 2: {"tool": "read_file", "args": {"path": "tests/test_utils.py"}}
Step 3: {"tool": "write_file", "args": {"path": "utils.py", "content": "..."}}
Step 4: {"tool": "run_command", "args": {"command": "pytest tests/test_utils.py"}}

# Code-as-action (Smolagents approach) — 1 step:
source = read_file("utils.py")
tests = read_file("tests/test_utils.py")
fixed = source.replace("data.split()", "data.split() if data else []")
write_file("utils.py", fixed)
result = run_command("pytest tests/test_utils.py -v")
print(f"Fix applied. Test results: {result}")
```

### Key Insight

Code is more expressive than JSON tool calls, and this matters enormously for coding agents:

- **Composition**: Chain multiple tool calls in a single action with intermediate processing
- **Conditionals**: Branch on results without requiring the framework to route
- **Data manipulation**: Transform tool outputs using Python's full standard library
- **Natural fit**: Coding agents already think in code — code-as-action is their native language
- **Fewer round-trips**: Each round-trip to the LLM costs latency and tokens. Fewer steps = faster, cheaper.

### Limitations

- No built-in checkpointing or persistence
- No multi-agent support out of the box
- Security concerns with executing arbitrary LLM-generated Python
- Limited to Python ecosystem
- Best suited for simple, single-agent workflows

---

## PydanticAI — Type-Safe Agents

### Philosophy

PydanticAI brings the **"FastAPI feeling"** to generative AI. Just as FastAPI made building web
APIs delightful through type safety, auto-documentation, and dependency injection, PydanticAI
applies the same principles to agent development.

The core idea: agents should have **typed inputs and outputs**. An agent that fixes code should
return a `CodeFix` object with validated fields, not a raw string that might or might not contain
the right information. This catches errors at development time, not in production.

Built by the Pydantic team (the library that powers FastAPI, LangChain, and much of the Python
AI ecosystem), PydanticAI leverages deep expertise in data validation and type safety.

### Architecture

```python
from pydantic_ai import Agent, RunContext
from pydantic import BaseModel, Field
from dataclasses import dataclass

# Typed output — the agent MUST return this structure
class CodeFix(BaseModel):
    file_path: str = Field(description="Path to the file being fixed")
    old_code: str = Field(description="The original buggy code")
    new_code: str = Field(description="The corrected code")
    explanation: str = Field(description="Why this fix is correct")
    confidence: float = Field(ge=0.0, le=1.0, description="Confidence in the fix")

# Typed dependencies — injected at runtime
@dataclass
class ProjectDeps:
    repo_path: str
    test_command: str
    language: str

# Create a typed agent: Agent[DepsType, OutputType]
agent = Agent(
    model="openai:gpt-4",
    deps_type=ProjectDeps,
    result_type=CodeFix,
    system_prompt="You are a code fixer. Analyze bugs and return structured fixes."
)

# Register tools with dependency injection
@agent.tool
async def read_file(ctx: RunContext[ProjectDeps], path: str) -> str:
    """Read a file from the project."""
    full_path = os.path.join(ctx.deps.repo_path, path)
    async with aiofiles.open(full_path) as f:
        return await f.read()

@agent.tool
async def run_tests(ctx: RunContext[ProjectDeps]) -> str:
    """Run the project's test suite."""
    proc = await asyncio.create_subprocess_shell(
        ctx.deps.test_command,
        cwd=ctx.deps.repo_path,
        stdout=asyncio.subprocess.PIPE,
        stderr=asyncio.subprocess.PIPE
    )
    stdout, stderr = await proc.communicate()
    return f"Exit code: {proc.returncode}\n{stdout.decode()}\n{stderr.decode()}"

# Run with full type safety
async def fix_bug(description: str):
    deps = ProjectDeps(
        repo_path="/home/user/project",
        test_command="pytest -v",
        language="python"
    )

    result = await agent.run(description, deps=deps)
    # result.data is CodeFix — fully validated by Pydantic
    fix: CodeFix = result.data

    print(f"File: {fix.file_path}")
    print(f"Confidence: {fix.confidence:.0%}")
    print(f"Explanation: {fix.explanation}")

    # Apply the fix
    content = open(fix.file_path).read()
    content = content.replace(fix.old_code, fix.new_code)
    open(fix.file_path, 'w').write(content)
```

### Dependency Injection for Testing

```python
# In production
real_deps = ProjectDeps(repo_path="/real/project", test_command="pytest", language="python")
result = await agent.run("Fix the null pointer", deps=real_deps)

# In tests — inject mock dependencies
mock_deps = ProjectDeps(repo_path="/tmp/test_project", test_command="echo PASSED", language="python")
result = await agent.run("Fix the null pointer", deps=mock_deps)
assert result.data.confidence > 0.8
assert "null" in result.data.explanation.lower()
```

### For Coding Agents

- **Structured outputs**: Diffs, AST edits, test results, code reviews — all as validated
  Pydantic models. No more parsing raw strings.
- **Dependency injection**: Swap real file systems for mocks, real test runners for stubs.
  Makes agent testing deterministic and fast.
- **Type safety**: Catch integration errors at development time. If the agent's output schema
  changes, your IDE highlights every caller that needs updating.
- **Model-agnostic**: Works with OpenAI, Anthropic, Gemini, Groq, Ollama via a unified interface.

---

## Mastra — TypeScript-First

### Philosophy

Mastra fills a critical gap: the TypeScript ecosystem lacked a first-class agent framework.
While Python dominates the AI framework space, a huge portion of production software is built
in TypeScript/JavaScript. Mastra provides idiomatic TypeScript abstractions for agent workflows,
with native integration into the React/Next.js ecosystem.

The workflow API uses familiar patterns — `.then()`, `.branch()`, `.parallel()` — that feel
natural to TypeScript developers. No need to learn a new paradigm.

### Architecture

```typescript
import { Agent, Workflow, Step } from '@mastra/core';

// Define workflow steps
const analyzeCode = new Step({
  id: 'analyze',
  execute: async ({ context }) => {
    const files = await readProjectFiles(context.projectPath);
    return { files, analysis: await analyzeStructure(files) };
  }
});

const refactorCode = new Step({
  id: 'refactor',
  execute: async ({ context }) => {
    return await applyRefactoring(context.analysis);
  }
});

const fixBugs = new Step({
  id: 'fix-bugs',
  execute: async ({ context }) => {
    return await applyBugFixes(context.analysis);
  }
});

const runTests = new Step({
  id: 'test',
  execute: async ({ context }) => {
    return await executeTestSuite(context.projectPath);
  }
});

const lintCheck = new Step({
  id: 'lint',
  execute: async ({ context }) => {
    return await runLinter(context.projectPath);
  }
});

const typeCheck = new Step({
  id: 'typecheck',
  execute: async ({ context }) => {
    return await runTypeChecker(context.projectPath);
  }
});

// Compose into a workflow
const codingWorkflow = new Workflow({ name: 'coding-agent' })
  .then(analyzeCode)
  .branch(
    { condition: ({ context }) => context.needsRefactor, handler: refactorCode },
    { condition: ({ context }) => context.hasBugs, handler: fixBugs }
  )
  .then(runTests)
  .parallel(lintCheck, typeCheck);

// Create an agent with the workflow
const agent = new Agent({
  name: 'TypeScript Coding Agent',
  model: 'gpt-4',
  instructions: 'You are an expert TypeScript developer.',
  tools: {
    readFile: { /* ... */ },
    writeFile: { /* ... */ },
    runCommand: { /* ... */ }
  },
  workflows: { coding: codingWorkflow }
});

// Execute
const result = await agent.generate('Fix the type errors in the auth module');
```

### Key Features

- **Suspend/resume**: Long-running workflows can suspend (e.g., waiting for human approval)
  and resume later. State is persisted automatically.
- **React/Next.js integration**: Build web-based agent UIs with React hooks that connect
  directly to Mastra agents. `useAgent()` hook for real-time streaming.
- **Type safety throughout**: Full TypeScript generics for agent inputs, outputs, and state.
  Leverage the TypeScript compiler to catch errors.
- **Workflow primitives**: `.then()` for sequential, `.branch()` for conditional,
  `.parallel()` for concurrent execution. Composable and readable.

### For Coding Agents

- **Primary TypeScript option**: If you're building a coding agent in TypeScript, Mastra is
  the most mature choice. No need to bridge to Python.
- **Suspend/resume**: Critical for long-running tasks — suspend while waiting for CI results,
  resume when they arrive.
- **Next.js UIs**: Build rich agent interfaces with real-time streaming, conversation history,
  and interactive controls. Natural for web-based coding assistants.
- **npm ecosystem**: Access the entire npm package ecosystem for tools, parsers, and utilities.

---

## OpenAI Agents SDK — Official OpenAI

### Philosophy

The OpenAI Agents SDK provides **minimal abstractions** that are **production-ready**. It evolved
from Swarm (OpenAI's experimental multi-agent framework, now deprecated), keeping the core
**handoff pattern** while adding guardrails, tracing, and production hardening.

The core loop is deliberately simple: get a completion → execute tools → handle handoffs → repeat.
No graph abstraction, no role system, no conversation protocol. Just the essentials.

Despite the name, the SDK is **model-agnostic** — it works with any OpenAI-compatible API.

### Key Features

- **Handoff pattern**: An agent can "hand off" control to another agent by returning it as the
  next handler. The runtime transfers the conversation to the new agent seamlessly. This enables
  triage → specialist routing without complex orchestration.

- **Guardrails**: Input and output validation that runs in parallel with the main agent. Input
  guardrails screen messages before processing. Output guardrails validate responses before
  returning them. Both can halt execution if violations are detected.

- **Built-in tracing**: Every agent run produces a trace with timing, token usage, tool calls,
  and handoffs. Traces integrate with OpenAI's dashboard for production monitoring.

- **Context management**: A generic `context` object flows through the agent's lifecycle,
  providing dependency injection without framework complexity.

### Architecture

```python
from agents import Agent, Runner, GuardrailFunctionOutput, InputGuardrail

# Define specialist agents
coder = Agent(
    name="Coder",
    instructions="""You are an expert software engineer. Write clean, well-tested code.
    Always explain your changes.""",
    tools=[read_file_tool, write_file_tool, run_tests_tool]
)

tester = Agent(
    name="Tester",
    instructions="""You are a QA engineer. Write and run tests.
    Report results clearly with pass/fail for each test case.""",
    tools=[read_file_tool, run_tests_tool, write_file_tool]
)

reviewer = Agent(
    name="Reviewer",
    instructions="""You are a senior code reviewer. Review changes for:
    - Correctness
    - Security vulnerabilities
    - Performance issues
    - Code style""",
    tools=[read_file_tool, diff_tool]
)

# Define guardrails
async def check_no_secrets(ctx, agent, input_text):
    """Prevent processing of messages containing secrets."""
    has_secrets = any(pattern in input_text for pattern in ["API_KEY=", "SECRET=", "PASSWORD="])
    return GuardrailFunctionOutput(
        output_info={"contains_secrets": has_secrets},
        tripwire_triggered=has_secrets
    )

# Triage agent routes to specialists
triage = Agent(
    name="Triage",
    instructions="""You are a project coordinator. Analyze the task and route to the right specialist:
    - Coder: for implementation tasks
    - Tester: for testing tasks
    - Reviewer: for code review tasks""",
    handoffs=[coder, tester, reviewer],
    input_guardrails=[InputGuardrail(guardrail_function=check_no_secrets)]
)

# Run the agent system
result = Runner.run(triage, "Fix the authentication bug in auth.py and add regression tests")

# The triage agent will hand off to the coder, who writes the fix,
# then the tester adds tests, then the reviewer checks the changes.
print(result.final_output)
```

### The Handoff Pattern

The handoff pattern is the SDK's core multi-agent primitive:

1. **Agent A** (triage) receives a task
2. Agent A analyzes the task and returns **Agent B** (coder) as the next handler
3. The runtime transfers the full conversation context to Agent B
4. Agent B processes the task and may hand off to **Agent C** (tester)
5. The chain continues until an agent produces a final output without handing off

This creates clean **routing without complex orchestration**. Each agent is stateless between
calls — it receives the full conversation and decides what to do. The handoff pattern is now
standard in multi-agent design, influencing frameworks beyond OpenAI's SDK.

---

## Google ADK — Gemini-Native

### Philosophy

Google's Agent Development Kit (ADK) is **code-first**, optimized for Google's Gemini models
but designed to be **model-agnostic**. It emphasizes hierarchical multi-agent architectures
with parent→child routing, and provides unique debugging capabilities through session rewind.

ADK is notable for its multi-language support (Python, Java, Go SDKs) and native integration
with Google Cloud services.

### Key Features

- **`adk eval` CLI**: Built-in evaluation framework for testing agent performance. Define
  evaluation datasets, run agents against them, and measure quality metrics — all from the
  command line.

- **Code execution sandbox**: Secure execution of generated code in isolated environments.
  Supports both local sandboxes and cloud-based execution.

- **A2A protocol support**: Native support for the Agent-to-Agent (A2A) protocol, enabling
  communication with agents built on different frameworks.

- **Session rewind**: Roll back agent state to any previous checkpoint. Uniquely valuable for
  debugging agent trajectories — see exactly where the agent went wrong and replay from
  that point with modified inputs.

- **Multi-language SDKs**: Python, Java, and Go — the broadest language support of any
  agent framework. Choose the language that fits your team and infrastructure.

### Architecture

```python
from google.adk import Agent, Tool

# Define tools
@Tool
def search_codebase(query: str) -> str:
    """Search the codebase for relevant code."""
    # Implementation using tree-sitter, ripgrep, etc.
    return search_results

@Tool
def edit_file(path: str, old_content: str, new_content: str) -> str:
    """Edit a file by replacing old content with new content."""
    # Implementation
    return "Edit applied successfully"

@Tool
def run_tests(test_path: str) -> str:
    """Run tests and return results."""
    # Implementation
    return test_results

# Hierarchical multi-agent setup
coder = Agent(
    name="coder",
    model="gemini-2.0-flash",
    instruction="You are an expert coder. Implement features and fix bugs.",
    tools=[search_codebase, edit_file]
)

tester = Agent(
    name="tester",
    model="gemini-2.0-flash",
    instruction="You are a test engineer. Write and run tests.",
    tools=[search_codebase, run_tests, edit_file]
)

# Parent agent coordinates children
coordinator = Agent(
    name="coordinator",
    model="gemini-2.5-pro",
    instruction="""You coordinate coding tasks. Delegate to specialists:
    - coder: for implementation work
    - tester: for testing work
    Route tasks appropriately and synthesize results.""",
    sub_agents=[coder, tester]
)

# The coordinator automatically routes to children based on the task
result = coordinator.run("Implement user profile caching and add integration tests")
```

### Session Rewind — Unique Debugging Feature

```python
# Run the agent
session = coordinator.run_with_session("Fix the memory leak in the connection pool")

# Inspect the trajectory
for i, checkpoint in enumerate(session.checkpoints):
    print(f"Step {i}: {checkpoint.agent_name} — {checkpoint.action_summary}")
    # Step 0: coordinator — Analyzed task, delegating to coder
    # Step 1: coder — Searched for connection pool code
    # Step 2: coder — Found pool.py, reading file
    # Step 3: coder — Applied fix to close() method  ← Wrong fix!
    # Step 4: tester — Ran tests, 3 failures

# Rewind to step 2 (before the wrong fix)
session.rewind_to(2)

# Resume with additional context
session.resume(additional_context="The leak is in acquire(), not close(). Check the timeout path.")
```

Session rewind is similar to LangGraph's time-travel debugging but is a built-in, first-class
feature rather than an add-on. It's particularly valuable for coding agents where a wrong edit
early in the trajectory can compound through subsequent steps.

---

## Comparison Table

| Framework    | Language    | Paradigm       | Complexity | Multi-Agent     | Checkpointing    | Stars |
|-------------|-------------|----------------|------------|-----------------|-------------------|-------|
| LangGraph   | Python/JS   | Graph-based    | Medium     | Subgraphs       | ✓ (built-in)     | ~25K  |
| CrewAI      | Python      | Role-based     | Low        | ✓ (crews)       | —                 | ~25K  |
| AutoGen     | Python/.NET | Conversation   | Medium     | ✓ (group chat)  | —                 | ~40K  |
| Smolagents  | Python      | Code-as-action | Minimal    | —               | —                 | ~15K  |
| PydanticAI  | Python      | Type-safe      | Low        | —               | —                 | ~8K   |
| Mastra      | TypeScript  | Workflow       | Medium     | ✓ (workflow)    | ✓ (suspend)       | ~10K  |
| OpenAI SDK  | Python      | Handoff        | Low        | ✓ (handoffs)    | —                 | ~15K  |
| Google ADK  | Py/Java/Go  | Hierarchical   | Medium     | ✓ (parent/child)| ✓ (rewind)        | ~10K  |

### Choosing by Use Case

| Use Case                          | Best Fit                    | Why                                     |
|----------------------------------|-----------------------------|-----------------------------------------|
| Rapid prototyping                | Smolagents                  | Minimal code, fast iteration            |
| Production coding agent          | Build from scratch          | Full control over tokens, latency       |
| Multi-agent experimentation      | CrewAI                      | YAML config, easy role swapping         |
| Type-safe structured outputs     | PydanticAI                  | Pydantic validation, DI for testing     |
| Complex stateful workflows       | LangGraph                   | Checkpointing, time-travel, durability  |
| TypeScript ecosystem             | Mastra                      | First-class TS, React/Next.js           |
| OpenAI-centric stack             | OpenAI Agents SDK           | Official, minimal, production-ready     |
| Google Cloud / Gemini            | Google ADK                  | Native integration, session rewind      |
| Conversational debugging         | AutoGen                     | Group chat, multi-turn refinement       |

---

## Build vs Buy Decision

### When to Use a Framework

- **Rapid prototyping**: Get a working agent in hours, not days. Frameworks handle the
  boilerplate so you can focus on behavior.
- **Standard patterns**: If your agent follows ReAct, plan-and-execute, or multi-agent
  patterns, frameworks implement these out of the box.
- **Checkpointing/persistence**: Building durable execution from scratch is hard. LangGraph
  and Google ADK provide this for free.
- **Observability**: LangSmith, AutoGen Studio, and ADK's eval CLI provide visibility into
  agent behavior without building custom tooling.
- **Small team, need community**: Frameworks come with documentation, examples, Discord
  communities, and Stack Overflow answers.
- **Experimentation**: Try different architectures (single agent vs. multi-agent, code-as-action
  vs. tool-calling) without rewriting infrastructure.

### When to Build from Scratch

- **Performance-critical**: Frameworks add overhead — extra abstractions, serialization,
  indirection. When every millisecond and token matters, you need full control.
- **Custom loop patterns**: If your agent's behavior doesn't fit neatly into a graph, crew,
  or conversation model, frameworks constrain more than they help.
- **Specific language requirements**: Building in Rust, Go, or another language with limited
  framework support? You're building from scratch regardless.
- **Full control over token usage**: Frameworks make opinionated decisions about prompt
  construction, message history, and context management. For production systems, you often
  need to control every token.
- **The mini-SWE-agent argument**: A well-crafted 100-line agent loop often outperforms a
  framework-based agent on benchmarks. Simplicity has compounding benefits.
- **Avoiding dependency risk**: Frameworks evolve rapidly. LangChain's API changed dramatically
  between versions. Building from scratch means you control your own API surface.

### The Reality

The pattern in production is clear:

- **Most successful coding agents built from scratch**: Claude Code, Codex CLI, Goose, Aider,
  Cursor — all use custom agent loops tailored to their specific needs.
- **Frameworks excel for experimentation**: When exploring architectures, testing hypotheses,
  or building internal tools, frameworks provide enormous leverage.
- **Production systems often outgrow frameworks**: Teams start with a framework, then gradually
  replace framework components with custom implementations as they hit limitations.
- **The gap is closing**: As frameworks mature, the build-from-scratch advantage shrinks.
  LangGraph's durable execution and checkpointing would take weeks to build from scratch.

The pragmatic approach: **start with a framework for validation, then decide whether to migrate
to custom code based on where you hit friction**.

---

## The A2A Protocol

The **Agent-to-Agent (A2A) protocol** is a Google-contributed specification now under the Linux
Foundation. It addresses a critical gap: how do agents built on different frameworks communicate
with each other?

### Core Concepts

- **Agent Cards**: JSON metadata documents that describe an agent's capabilities, supported
  input/output types, and endpoint URLs. Published at `/.well-known/agent.json` for discovery.

- **JSON-RPC 2.0 over HTTP(S)**: Standard transport protocol. Agents communicate via
  well-defined RPC methods for task submission, status polling, and result retrieval.

- **Task lifecycle**: Submit → Working → Complete/Failed. Long-running tasks support
  streaming updates via Server-Sent Events (SSE).

### Relationship to MCP

A2A and MCP (Model Context Protocol) are **complementary**, not competing:

- **MCP = agent-to-tool**: How an agent discovers and invokes tools (file systems, APIs,
  databases). Standardizes the tool interface.
- **A2A = agent-to-agent**: How agents discover and communicate with each other.
  Standardizes the agent interface.

Together, they enable **heterogeneous agent ecosystems**: a LangGraph agent can delegate to
a Mastra agent via A2A, and both can use tools exposed via MCP. The protocols create a level
of interoperability that framework-specific APIs cannot.

### Implications for Coding Agents

A2A enables composing coding agents from best-of-breed components:
- Use a Rust-based agent for fast code search
- Delegate to a Python-based agent for ML code generation
- Hand off to a TypeScript-based agent for frontend work
- All communicating via A2A, regardless of framework or language

---

## The Swarm Legacy

OpenAI's **Swarm** was an experimental, educational framework that introduced the **handoff
pattern** to the broader agent community. Though now deprecated (succeeded by the Agents SDK),
its influence is visible across the ecosystem.

### Key Contributions

- **Established the handoff pattern**: Agent A returns Agent B as the next handler. Simple,
  composable, powerful. Now the standard multi-agent primitive in the OpenAI Agents SDK.
- **Proved minimalism works**: Swarm was deliberately simple — a few hundred lines of code.
  This inspired Smolagents and influenced the "less is more" philosophy.
- **Showed routines > agents**: Swarm called its agents "routines," emphasizing that each
  agent is just a set of instructions + tools + handoff targets. No magic.

### Influence on the Ecosystem

- **OpenAI Agents SDK**: Direct successor, production-hardened version of Swarm's patterns
- **CrewAI**: The delegation pattern mirrors Swarm's handoffs
- **AutoGen**: Group chat speaker selection echoes Swarm's routing logic
- **Custom agents**: Many production agents (including Claude Code's sub-agent pattern)
  use handoff-like delegation without a framework

The handoff pattern's simplicity is its greatest strength: it requires no complex orchestration
layer, no graph definition, no role configuration. Just one agent deciding that another agent
should handle the next step. This pattern has become fundamental to multi-agent system design.

---

## Summary

The agent framework landscape is evolving rapidly. Key trends:

1. **Convergence on patterns**: Handoffs, tool-calling, checkpointing, and multi-agent
   coordination appear across frameworks regardless of their paradigm.
2. **Framework maturity**: Early frameworks were brittle and opinionated. Current versions
   offer genuine production value (LangGraph's durable execution, PydanticAI's type safety).
3. **Language expansion**: Python dominance is giving way to multi-language support
   (Mastra for TypeScript, Google ADK for Java/Go).
4. **Protocol standardization**: MCP and A2A are creating framework-agnostic standards
   for tool and agent communication.
5. **The custom loop endures**: For the highest-performance coding agents, building from
   scratch remains the dominant approach — but frameworks are closing the gap.

The best framework is the one that matches your team's language, your agent's complexity,
and your tolerance for dependency. For many teams, the answer is still "no framework at all."