# The Augmented LLM

> The foundational building block of all agentic systems — an LLM enhanced with
> retrieval, tools, and memory to interact with the world beyond its training data.

---

## Overview

Before we talk about agents, we need to talk about the **augmented LLM**. In
Anthropic's "Building Effective Agents" framework, the augmented LLM is not a
pattern in itself — it is the **atomic unit** from which all patterns are built.

Every node in an orchestrator-workers system is an augmented LLM. Every step in
a prompt chain is an augmented LLM. Every evaluator in an evaluator-optimizer
loop is an augmented LLM. Understanding this building block is prerequisite to
understanding everything else.

An augmented LLM extends a base language model with three capabilities:

1. **Retrieval** — the ability to fetch relevant context from external sources
2. **Tools** — the ability to take actions in the environment
3. **Memory** — the ability to persist information across interactions

```
  +-------------------------------------------------------+
  |                    Augmented LLM                       |
  |                                                        |
  |   +--------------+  +-----------+  +--------------+   |
  |   |  Retrieval   |  |   Tools   |  |    Memory    |   |
  |   |              |  |           |  |              |   |
  |   | - Vector DB  |  | - File IO |  | - Short-term |   |
  |   | - Keyword    |  | - Shell   |  | - Long-term  |   |
  |   | - Hybrid     |  | - LSP     |  | - Episodic   |   |
  |   | - Code index |  | - Search  |  | - Semantic   |   |
  |   +------+-------+  +-----+-----+  +------+-------+  |
  |          |                |               |            |
  |          +--------+-------+-------+-------+            |
  |                   v               v                    |
  |              +---------------------+                   |
  |              |     LLM Core        |                   |
  |              |                     |                   |
  |              |  Reasoning Engine   |                   |
  |              |  Token Prediction   |                   |
  |              |  Context Window     |                   |
  |              +---------------------+                   |
  |                                                        |
  +-------------------------------------------------------+
```

### Why "Augmented"?

A raw LLM can only:
- Process the tokens in its context window
- Generate tokens based on its training data
- Do nothing else

An *augmented* LLM can:
- Pull in fresh, relevant information (retrieval)
- Execute actions and observe results (tools)
- Remember past interactions and preferences (memory)

This transforms the LLM from a passive text generator into an active
participant in a computational workflow. The augmented LLM is what makes
agentic systems possible.

---

## Architecture

The detailed architecture of an augmented LLM in a CLI coding context:

```
                          User Prompt
                              |
                              v
                    +-----------------+
                    |  System Prompt  |
                    |  + Instructions |
                    +--------+--------+
                             |
                             v
  +---------------------------------------------------------+
  |                    AUGMENTED LLM                         |
  |                                                          |
  |  +---------------------------------------------+        |
  |  |              Context Assembly                |        |
  |  |                                              |        |
  |  |  +-----------+ +-----------+ +------------+  |        |
  |  |  |  Memory   | | Retrieved | |  Tool Defs |  |        |
  |  |  |  Context  | |  Context  | |  & Schemas |  |        |
  |  |  +-----------+ +-----------+ +------------+  |        |
  |  +---------------------+------------------------+        |
  |                        |                                 |
  |                        v                                 |
  |  +---------------------------------------------+        |
  |  |                 LLM Inference                |        |
  |  |                                              |        |
  |  |   Input tokens --> Model --> Output tokens   |        |
  |  |                                              |        |
  |  |   Output may contain:                        |        |
  |  |   - Natural language response                |        |
  |  |   - Tool call request(s)                     |        |
  |  |   - Retrieval queries                        |        |
  |  +---------------------+------------------------+        |
  |                        |                                 |
  |                        v                                 |
  |  +---------------------------------------------+        |
  |  |              Tool Execution Layer            |        |
  |  |                                              |        |
  |  |  +---------+ +--------+ +------+ +--------+  |       |
  |  |  |File I/O | | Shell  | | LSP  | | Search |  |       |
  |  |  +---------+ +--------+ +------+ +--------+  |       |
  |  |  +---------+ +--------+ +------+ +--------+  |       |
  |  |  |  Git    | |Browser | | MCP  | | Custom |  |       |
  |  |  +---------+ +--------+ +------+ +--------+  |       |
  |  +-----------------------------------------------+      |
  |                                                          |
  +----------------------------------------------------------+
                              |
                              v
                    Response to User
```

### The Single-Inference Model

The critical distinction between an augmented LLM and an agent: the augmented
LLM performs a **single inference pass** (possibly with tool calls that trigger
follow-up inferences). An agent wraps this in a **loop**.

```
  Augmented LLM:
    Input --> [Inference + Tool Calls] --> Output

  Agent:
    Input --> [Inference + Tool Calls] --> Observe --> [Inference + Tool Calls] --> ...
              +------------------------ Loop until done --------------------------+
```

In practice, the line is blurry. When a model makes a tool call and then
processes the result, is that one inference or two? Most frameworks treat the
tool-call-and-response cycle as a single "turn" of the augmented LLM.

---

## Retrieval-Augmented Generation (RAG)

RAG is the augmented LLM's ability to pull in relevant external information
before generating a response. For coding agents, this is essential — the
model needs to understand the *specific codebase* it's working with.

### Retrieval Strategies

| Strategy         | How It Works                              | Strengths                 | Weaknesses               |
|------------------|------------------------------------------|---------------------------|--------------------------|
| **Vector Search**| Embed query -> find similar embeddings   | Semantic similarity       | Misses exact matches     |
| **Keyword Search**| BM25 / TF-IDF over token index          | Exact symbol matching     | Misses semantic intent   |
| **Hybrid Search**| Combine vector + keyword                 | Best of both worlds       | More complex to tune     |
| **AST-Based**   | Parse code -> search syntax tree         | Structure-aware           | Language-specific        |
| **Graph-Based**  | Build dependency graph -> traverse       | Relationship-aware        | Expensive to maintain    |

### How Coding Agents Use RAG

#### Aider: The Repo-Map Approach

Aider pioneered the **repo-map** — a compressed representation of the entire
repository that fits within the context window. It uses tree-sitter to parse
every file and extract:
- Function/method definitions and signatures
- Class hierarchies
- Import relationships

This repo-map is included in every prompt, giving the LLM a "table of contents"
for the codebase without consuming the entire context window.

```
  Repository                          Repo-Map
  ----------                          --------
  src/
  +-- auth.py (200 lines)    -->    src/auth.py:
  |   +-- class AuthManager          |  class AuthManager
  |   |   +-- login()                |    login(username, password) -> Token
  |   |   +-- logout()               |    logout(token) -> bool
  |   |   +-- refresh()              |    refresh(token) -> Token
  |   +-- verify_token()             |  verify_token(token) -> Claims
  +-- models.py (150 lines)  -->    src/models.py:
  |   +-- class User                 |  class User(BaseModel)
  |   +-- class Token                |  class Token(BaseModel)
  ...                                ...
```

#### Claude Code & ForgeCode: Codebase Search Tools

Rather than pre-computing a repo-map, Claude Code and ForgeCode provide
**search tools** that the LLM invokes on demand:

- `grep` / `ripgrep` — text pattern matching
- `glob` — file pattern matching
- `find_references` — LSP-based symbol lookup
- `code_search` — semantic code search

The LLM decides *what* to search for based on the task. This is more flexible
than a static repo-map but requires the LLM to know *what to look for*.

#### Codex CLI: Repository Context

Codex CLI collects repository context including file listings, git status,
and relevant file contents, then includes this as part of the system prompt.
This is a simpler form of retrieval that relies on the model to identify
what additional files need to be read.

#### OpenHands: Document Store

OpenHands maintains a document store that can be queried for relevant code
snippets, documentation, and past interactions. This provides a richer
retrieval layer that combines multiple sources.

### Chunking Strategies for Code

Code has unique properties that affect chunking:

```
  +----------------------------------------------+
  |           Code Chunking Strategies            |
  |                                               |
  |  1. Naive (line-based)                        |
  |     Split every N lines                       |
  |     X Breaks functions mid-body               |
  |                                               |
  |  2. Function-level                            |
  |     One chunk per function/method             |
  |     + Semantic boundaries                     |
  |     X Large functions exceed chunk size       |
  |                                               |
  |  3. Class-level                               |
  |     One chunk per class                       |
  |     + Preserves class context                 |
  |     X Large classes exceed chunk size         |
  |                                               |
  |  4. AST-aware                                 |
  |     Use syntax tree to find natural boundaries|
  |     + Respects language structure              |
  |     + Handles nested structures               |
  |     X Requires parser per language            |
  |                                               |
  |  5. Sliding window with overlap               |
  |     Overlapping chunks of N lines             |
  |     + No information loss at boundaries       |
  |     X Redundant tokens                        |
  +----------------------------------------------+
```

Most coding agents use a combination: AST-aware chunking for indexed search,
with function-level retrieval for specific lookups.

---

## Tool-Augmented Generation

Tools are the augmented LLM's hands. While retrieval brings information *in*,
tools push actions *out* into the environment.

### Tool Categories in Coding Agents

```
  +------------------------------------------------------------+
  |                    Tool Taxonomy                            |
  |                                                            |
  |  +------------------+  +------------------+                |
  |  | Read Operations  |  | Write Operations |                |
  |  |                  |  |                  |                |
  |  | - read_file      |  | - write_file     |                |
  |  | - list_directory |  | - apply_diff     |                |
  |  | - search_files   |  | - create_file    |                |
  |  | - grep           |  | - delete_file    |                |
  |  | - git_log        |  | - git_commit     |                |
  |  +------------------+  +------------------+                |
  |                                                            |
  |  +------------------+  +------------------+                |
  |  | Execute Actions  |  | Analysis Tools   |                |
  |  |                  |  |                  |                |
  |  | - run_command    |  | - lsp_diagnostics|                |
  |  | - run_tests      |  | - type_check     |                |
  |  | - install_deps   |  | - lint           |                |
  |  | - start_server   |  | - parse_ast      |                |
  |  +------------------+  +------------------+                |
  +------------------------------------------------------------+
```

### How Tools Differ from RAG

| Aspect            | RAG (Retrieval)                    | Tools                              |
|-------------------|------------------------------------|------------------------------------|
| **Direction**     | Information flows *in* to context  | Actions flow *out* to environment  |
| **Side Effects**  | None (read-only)                   | May modify state                   |
| **Timing**        | Before or during inference         | During or after inference          |
| **Reversibility** | Always safe                        | May need undo/rollback             |
| **Examples**      | Search code, fetch docs            | Edit file, run command, git commit |

### Tool Definition and Dispatch

Modern LLMs interact with tools through a structured protocol:

```python
# Tool definition (simplified)
tool_definition = {
    "name": "edit_file",
    "description": "Edit a file by replacing old text with new text",
    "parameters": {
        "type": "object",
        "properties": {
            "path": {
                "type": "string",
                "description": "Absolute path to the file"
            },
            "old_str": {
                "type": "string",
                "description": "The exact text to find and replace"
            },
            "new_str": {
                "type": "string",
                "description": "The replacement text"
            }
        },
        "required": ["path", "old_str", "new_str"]
    }
}
```

The LLM generates a structured tool call, the framework dispatches it, and the
result is fed back into the context for the next inference step.

### MCP: Model Context Protocol

MCP (Model Context Protocol) is Anthropic's open standard for connecting LLMs
to external tools and data sources. It provides a standardized interface that
any tool provider can implement.

```
  +--------------+       +--------------+       +--------------+
  |  LLM Client  |<----->|  MCP Server  |<----->|  External    |
  |  (Agent)     |  MCP  |  (Adapter)   |       |  Service     |
  +--------------+ Proto +--------------+       +--------------+

  Examples:
  - Claude Code --> MCP --> GitHub API
  - Claude Code --> MCP --> Database
  - Goose       --> MCP --> Custom extensions
  - OpenCode    --> MCP --> Custom tools
```

MCP is particularly relevant for CLI coding agents because it allows:
- **Standardized tool discovery** — agents can list available tools dynamically
- **Cross-agent compatibility** — tools work with any MCP-compatible agent
- **Extension ecosystems** — Goose and Claude Code both use MCP for plugins

### ACI: Agent-Computer Interface (Anthropic's Tool Design Principles)

Anthropic emphasizes that the **Agent-Computer Interface (ACI)** is as important
as the user interface. Key principles from their appendix:

1. **Think from the model's perspective.** Is it obvious how to use this tool
   based on the name and description? Will the model be able to interpret the
   output correctly?

2. **Keep interfaces simple.** Fewer parameters with clear semantics. Avoid
   requiring the model to construct complex nested objects.

3. **Return structured, parseable output.** The model needs to understand
   tool results. JSON is better than free-form text. Error messages should
   be actionable.

4. **Anticipate misuse.** Models will try to use tools in creative ways.
   Design for graceful failure with clear error messages.

5. **Test empirically.** Run your tools with real LLM interactions, not just
   unit tests. The model may interpret parameters differently than expected.

```
  Good ACI Design                    Poor ACI Design
  ----------------                   -----------------
  edit_file(path, old, new)          modify(target, operations=[...])
  search(query, path_glob)           find(params={complex nested obj})
  run_command(cmd, cwd)              execute(spec={cmd, env, stdin, ...})
```

---

## Memory Systems

Memory allows the augmented LLM to maintain state and learn from past
interactions. There are three distinct types:

### Short-Term Memory (Conversation Context)

The simplest form of memory: the conversation history within a single session.
This is managed by the context window and is lost when the session ends.

```
  Turn 1: User asks to fix a bug
  Turn 2: Agent reads file, identifies issue
  Turn 3: Agent applies fix        <-- Remembers turns 1 & 2
  Turn 4: Agent runs tests         <-- Remembers turns 1, 2 & 3
```

**Context window management** is critical here. CLI coding agents often deal
with large files and long sessions that can exceed the context window. Strategies
include:
- **Summarization** — compress old turns into summaries
- **Sliding window** — drop oldest turns
- **Selective retention** — keep important turns, drop routine ones
- **Compaction** — Claude Code's approach of summarizing the conversation
  when the context window fills up

### Long-Term Memory (Persistent Instructions)

Most CLI coding agents implement long-term memory through persistent
configuration files:

| Agent              | Memory File(s)              | Scope                          |
|--------------------|-----------------------------|--------------------------------|
| **Claude Code**    | `CLAUDE.md`                 | Per-project and global         |
| **Gemini CLI**     | `GEMINI.md`                 | Per-project and global         |
| **Codex CLI**      | `AGENTS.md`, `codex.md`     | Per-project instructions       |
| **Aider**          | `.aider.conf.yml`           | Per-project conventions        |
| **ForgeCode**      | `FORGE.md`                  | Per-project instructions       |
| **Goose**          | `.goosehints`               | Per-directory hints            |
| **OpenCode**       | `OPENCODE.md`               | Per-project instructions       |
| **Droid**          | `.droid/`                   | Per-project configuration      |
| **Ante**           | `.ante/`                    | Per-project memory             |
| **Warp**           | Warp's rule system          | Per-workspace rules            |

These files typically contain:
- Project-specific coding conventions
- Architecture decisions and constraints
- Preferred tools and libraries
- Common commands (build, test, lint)
- Known issues and workarounds

### Episodic Memory

Some agents are developing more sophisticated memory that learns from past
interactions:

- **Task outcomes** — what worked and what didn't for similar tasks
- **User preferences** — inferred coding style and patterns
- **Error patterns** — common mistakes and their fixes

This is still an emerging capability. Most coding agents today rely on
explicit long-term memory files rather than automated episodic learning.

### Memory Architecture Diagram

```
  +----------------------------------------------------------+
  |                    Memory Systems                         |
  |                                                           |
  |  +-----------------+                                      |
  |  |  Short-Term      | <-- Conversation history            |
  |  |  (Context Window) |     Lost on session end             |
  |  +--------+---------+                                     |
  |           | Summarize                                      |
  |           v on overflow                                    |
  |  +-----------------+                                      |
  |  |  Long-Term       | <-- CLAUDE.md, AGENTS.md, etc.      |
  |  |  (Persistent)    |     Survives across sessions         |
  |  +--------+---------+                                     |
  |           |                                                |
  |           v                                                |
  |  +-----------------+                                      |
  |  |  Episodic        | <-- Task logs, outcome history       |
  |  |  (Learned)       |     Inferred patterns                |
  |  +-----------------+                                      |
  |                                                           |
  +----------------------------------------------------------+
```

---

## When Augmented LLM Is Sufficient

Not every task needs an agent loop. The augmented LLM — a single inference
pass with retrieval and tools — is sufficient for a surprising range of coding
tasks:

### Single-Turn Tasks

```
  "Explain this function"
  "Write a unit test for this class"
  "Convert this Python to TypeScript"
  "Add error handling to this function"
  "What does this regex do?"
```

### Well-Scoped Code Generation

When the task is clear and bounded, a single LLM call with the right context
can produce a complete, correct result:

```
  Generate a REST endpoint given a schema
  Implement a function given its signature and docstring
  Write a database migration
  Create a configuration file from a template
```

### When You Don't Need Agentic Loops

The augmented LLM is preferred when:
- The task requires **at most one round of tool use**
- The output quality is **high on first attempt** (no iteration needed)
- The task is **well-defined** with a clear expected output
- **Latency matters** more than exhaustive exploration
- The task **doesn't require reacting** to intermediate results

**Warp** is a good example of an agent that leans heavily on the augmented LLM
pattern. Many of its interactions are single-turn: the user asks a question or
requests a command, and Warp provides it with appropriate context.

---

## Augmented LLM vs. Agents

The line between an augmented LLM and an agent is the **loop**:

```
  +------------------------------------------------------------------+
  |                                                                  |
  |  Augmented LLM:                                                  |
  |                                                                  |
  |    User --> [Context + LLM + Tools] --> Response                 |
  |                                                                  |
  |    - Single pass (may include tool-call sub-steps)               |
  |    - Deterministic control flow                                  |
  |    - Predictable cost and latency                                |
  |    - No ability to recover from errors                           |
  |                                                                  |
  +------------------------------------------------------------------+
  |                                                                  |
  |  Agent:                                                          |
  |                                                                  |
  |    User --> [Context + LLM + Tools] --> Observe --> [Decide] --+ |
  |                  ^                                     |        |
  |                  +---------------- Loop <---------------+       |
  |                                                                  |
  |    - Multiple passes with observation                            |
  |    - Dynamic, model-directed control flow                        |
  |    - Variable cost and latency                                   |
  |    - Can recover from errors and adapt                           |
  |                                                                  |
  +------------------------------------------------------------------+
```

### Cost and Latency Tradeoffs

| Aspect                | Augmented LLM              | Agent                     |
|-----------------------|----------------------------|---------------------------|
| **API calls**         | 1-3 (with tool use)        | 5-50+ (varies widely)     |
| **Latency**           | 1-10 seconds               | 30 seconds to minutes     |
| **Token cost**        | Predictable                | Highly variable           |
| **Reliability**       | High (simple flow)         | Lower (more failure modes)|
| **Error recovery**    | None (fails or succeeds)   | Can retry, adapt, pivot   |
| **Task complexity**   | Single-step tasks          | Multi-step, open-ended    |

### When to Graduate from Augmented LLM to Agent

```
  Should you use an agent loop?

  +-- Does the task require multiple tools in sequence?
  |   +-- NO --> Augmented LLM is sufficient
  |   +-- YES -> Can you predefine the sequence?
  |              +-- YES --> Use prompt chaining (workflow)
  |              +-- NO --> Does the task require reacting to results?
  |                         +-- NO --> Use parallelization (workflow)
  |                         +-- YES -> You need an agent loop
  +----------------------------------------------------------
```

---

## Implementation Across the 17 Agents

Every CLI coding agent implements the augmented LLM as its core building block.
Here's how each one approaches the three pillars:

### Tier 1 Agents

| Agent          | Retrieval                        | Tools                                    | Memory                    |
|----------------|----------------------------------|------------------------------------------|---------------------------|
| **ForgeCode**  | Grep, glob, code search          | File I/O, shell, git, LSP               | FORGE.md                  |
| **Claude Code**| Grep, glob, semantic search      | File I/O, shell, git, MCP, sub-agents   | CLAUDE.md (project+global)|
| **Codex CLI**  | File listing, git context        | File I/O, shell (sandboxed)             | AGENTS.md                 |
| **Droid**      | Codebase indexing                | File I/O, shell, background tasks        | .droid/ directory         |
| **Ante**       | Context-aware search             | File I/O, shell, multi-tool              | .ante/ directory          |
| **OpenCode**   | Grep, glob                       | File I/O, shell, MCP                     | OPENCODE.md               |
| **OpenHands**  | Document store, code search      | Sandboxed execution, browser, file I/O   | Session history           |

### Tier 2 Agents

| Agent              | Retrieval                    | Tools                              | Memory                  |
|--------------------|------------------------------|------------------------------------|-------------------------|
| **Warp**           | Terminal history, context    | Command generation, shell          | Warp rules              |
| **Gemini CLI**     | Google Search, code search   | File I/O, shell, Google tools      | GEMINI.md               |
| **Goose**          | Extension-based              | MCP extensions, shell, file I/O    | .goosehints             |
| **Junie CLI**      | Project analysis             | File I/O, shell, test runner       | Project config          |
| **mini-SWE-agent** | Grep, file viewing           | File edit, shell                   | Conversation only       |
| **Pi Coding Agent**| Basic file search            | File I/O, shell                    | Conversation only       |
| **Aider**          | Repo-map (tree-sitter)       | File edit, shell, git, lint        | .aider.conf.yml         |

### Tier 3 Agents

| Agent          | Retrieval                    | Tools                          | Memory              |
|----------------|------------------------------|--------------------------------|----------------------|
| **Sage Agent** | Research-focused search      | File I/O, analysis tools       | Session logs         |
| **TongAgents** | Multi-agent shared context   | Delegated to specialized agents| Shared state         |
| **Capy**       | Basic file search            | File I/O, shell                | Minimal              |

### Patterns Observed

1. **Retrieval is the differentiator.** Agents with better retrieval (Aider's
   repo-map, Claude Code's multi-strategy search) consistently perform better
   on tasks requiring codebase understanding.

2. **Tool breadth varies enormously.** Tier 1 agents typically offer 15-30+
   tools; Tier 3 agents may offer fewer than 10. Tool quality matters more
   than tool quantity.

3. **Memory is converging.** The `CLAUDE.md` / `AGENTS.md` pattern of a
   per-project markdown file is becoming an industry standard.

---

## Code Examples

### Basic Augmented LLM Pattern

The following pseudocode shows the core augmented LLM pattern that underlies
every coding agent:

```python
class AugmentedLLM:
    """The foundational building block of all agentic systems."""

    def __init__(self, model, tools, retriever, memory):
        self.model = model          # The base LLM
        self.tools = tools          # Available tool definitions
        self.retriever = retriever  # RAG system
        self.memory = memory        # Persistent memory

    def run(self, user_message: str) -> str:
        # 1. Assemble context
        context = self._build_context(user_message)

        # 2. Single inference pass (may include tool calls)
        response = self._inference_with_tools(context)

        # 3. Update memory
        self.memory.store(user_message, response)

        return response.text

    def _build_context(self, user_message: str) -> Context:
        """Assemble all context for the LLM."""
        return Context(
            system_prompt=self._get_system_prompt(),
            memory=self.memory.retrieve_relevant(user_message),
            retrieved=self.retriever.search(user_message),
            tool_definitions=self.tools.schemas(),
            conversation=self.memory.get_conversation(),
            user_message=user_message,
        )

    def _inference_with_tools(self, context: Context) -> Response:
        """Run inference, executing any tool calls the model makes."""
        messages = context.to_messages()

        while True:
            response = self.model.generate(messages)

            if not response.has_tool_calls:
                return response

            # Execute tool calls and append results
            for tool_call in response.tool_calls:
                result = self.tools.execute(tool_call)
                messages.append(tool_call_message(tool_call, result))

            # Continue inference with tool results in context
```

### Graduating to an Agent Loop

To convert this augmented LLM into an agent, wrap it in a loop:

```python
class Agent:
    """An augmented LLM wrapped in an autonomous loop."""

    def __init__(self, augmented_llm: AugmentedLLM, max_steps: int = 50):
        self.llm = augmented_llm
        self.max_steps = max_steps

    def run(self, task: str) -> str:
        """Execute a task autonomously."""
        observation = task
        steps = 0

        while steps < self.max_steps:
            # Think: LLM decides what to do
            response = self.llm.run(observation)

            # Check: Is the task complete?
            if self._is_done(response):
                return response

            # Act: Execute tools, observe results
            observation = self._execute_and_observe(response)
            steps += 1

        return "Max steps reached without completing task"
```

### Tool Definition Example

```python
# How tools are defined for the augmented LLM
tools = ToolRegistry([
    Tool(
        name="read_file",
        description="Read the contents of a file at the given path",
        parameters={
            "path": Parameter(type="string", required=True,
                            description="Absolute path to the file")
        },
        handler=lambda path: open(path).read()
    ),
    Tool(
        name="edit_file",
        description="Replace old_str with new_str in a file",
        parameters={
            "path": Parameter(type="string", required=True),
            "old_str": Parameter(type="string", required=True),
            "new_str": Parameter(type="string", required=True),
        },
        handler=edit_file_handler
    ),
    Tool(
        name="run_command",
        description="Execute a shell command and return output",
        parameters={
            "command": Parameter(type="string", required=True,
                               description="The shell command to run"),
        },
        handler=lambda cmd: subprocess.run(cmd, capture_output=True)
    ),
])
```

---

## Relationship to Other Patterns

The augmented LLM is the foundation. Every other pattern in the
[Agent Design Patterns](README.md) catalog builds on it:

```
  Augmented LLM
       |
       +--> Prompt Chaining     = Sequence of augmented LLM calls
       +--> Routing             = Augmented LLM as classifier + handlers
       +--> Parallelization     = Concurrent augmented LLM calls
       +--> Orchestrator-Workers = Augmented LLM managing augmented LLMs
       +--> Evaluator-Optimizer  = Two augmented LLMs in a feedback loop
       +--> Autonomous Agent     = Augmented LLM in an autonomous loop
```

Improving the augmented LLM — better retrieval, better tools, better memory —
improves *every* pattern built on top of it. This is why Anthropic emphasizes
investing in the building block before investing in orchestration.

---

## Key Takeaways

1. **The augmented LLM is the atom of agentic systems.** Every pattern,
   every agent, every workflow is composed of augmented LLMs. Master this
   building block first.

2. **Three pillars: Retrieval, Tools, Memory.** Each pillar independently
   makes the LLM more capable. Combined, they transform a text generator
   into a capable coding assistant.

3. **Retrieval is the competitive advantage.** Among the 17 agents studied,
   the quality of codebase retrieval is the strongest predictor of performance
   on repository-level tasks. Aider's repo-map and Claude Code's multi-strategy
   search are standout implementations.

4. **Tool design is as important as model quality.** Anthropic's ACI principles
   — simplicity, clear semantics, structured output — are essential. A mediocre
   model with well-designed tools often outperforms a frontier model with
   poorly designed tools.

5. **Memory is converging on markdown files.** The CLAUDE.md / AGENTS.md
   pattern has become an industry standard. This simple approach works because
   it leverages the LLM's strength (understanding natural language) while
   giving users full control over the memory content.

6. **MCP is the emerging standard.** The Model Context Protocol is rapidly
   being adopted across agents (Claude Code, Goose, OpenCode, and others) as
   the standard way to connect tools. This is driving tool interoperability
   across the ecosystem.

7. **Know when augmented LLM is enough.** Not every task needs an agent loop.
   Single-turn tasks, well-scoped generation, and simple queries are better
   served by a single augmented LLM inference — faster, cheaper, and more
   reliable.

8. **The gap between augmented LLM and agent is the loop.** This single
   architectural decision — whether to iterate or not — is the most important
   design choice in building a coding agent. Everything else is optimization.

---

*This document is part of the [Agent Design Patterns](README.md) research
series. For the complete pattern catalog, see the [README](README.md). For
comparative analysis of all 17 agents, see
[agent-comparison.md](agent-comparison.md).*
