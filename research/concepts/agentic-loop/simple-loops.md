# Simple Loops

> *"The best agent architecture is the one you can fit in your head."*

The most counterintuitive finding from studying 17 production and research agents is that
**the simplest loops often perform best**. This document dissects why, anchored in
mini-SWE-agent — a ~100-line implementation that scores 65 % on SWE-bench Verified —
and explores the design principles that make simple loops surprisingly competitive.

---

## The Case for Simplicity

### Raw Numbers

| Agent | Lines of scaffold | SWE-bench Verified |
|---|---|---|
| mini-SWE-agent | ~100 | 65 % |
| SWE-agent (full) | ~4 000 | 23 % (original) |
| Moatless Tools | ~8 000 | 26 % |
| OpenHands CodeAct | ~12 000 | 53 % |

mini-SWE-agent achieves a higher solve rate than agents 40–120× its size. The gap is
not a fluke — it reflects a fundamental asymmetry:

**Simple loop + good model > complex loop + weak model**

### Why Scaffold Complexity Has Diminishing Returns

1. **The model does the heavy lifting.** Tool selection, code generation, error recovery,
   planning — the frontier model already handles all of these. The scaffold's job is to
   *stay out of the way* and feed observations back.

2. **Every abstraction costs tokens.** A routing layer that decides which sub-agent to
   invoke adds system-prompt overhead, tool-description tokens, and output-parsing logic.
   Those tokens could instead carry file contents or error messages.

3. **Debugging compounds.** When an agent fails, you need to understand *what it saw*.
   A linear message list is trivially inspectable. A graph of sub-agents with shared
   state is not.

4. **Latency stacks.** Each "thinking" step (planner → executor → validator) is a
   separate LLM call. Three-step pipelines triple wall-clock time with no guarantee of
   better answers.

The core insight: **most agent capabilities come from the MODEL, not the scaffold.**
The scaffold is plumbing. Good plumbing matters, but gold-plated plumbing does not
make the water taste better.

---

## mini-SWE-agent: The ~100-Line Implementation

Below is the full annotated source of mini-SWE-agent, the reference implementation
from the SWE-agent team that demonstrates how little code is actually needed.

### AgentConfig — 5 Fields

```python
@dataclass
class AgentConfig:
    """Everything the agent needs to know before it starts."""
    model: str                    # e.g. "claude-sonnet-4-20250514" or "gpt-4o"
    max_cost: float = 2.0        # hard budget cap in USD
    max_steps: int = 50          # hard cap on agentic turns
    template_dir: Path = Path("templates")  # Jinja2 prompt templates
    tools: list[str] = field(default_factory=lambda: [
        "bash", "edit", "view", "submit"
    ])
```

**Why it works:** Five fields cover the entire configuration surface. There is no
`RetryPolicy`, no `RoutingStrategy`, no `MemoryBackend`. The model, a budget, a step
limit, prompt templates, and a tool list — that is the full specification of the agent.

Compare this to a typical enterprise agent config with 40+ fields spanning rate-limit
policies, vector-store connections, sub-agent definitions, and callback URLs. Every
field is a decision point, a potential bug, and a paragraph in the docs.

### DefaultAgent.__init__

```python
class DefaultAgent:
    def __init__(self, config: AgentConfig, problem_statement: str):
        self.config = config
        self.cost = 0.0
        self.steps = 0

        # Build the initial message list from Jinja2 templates
        system_prompt = self._render("system.j2", tools=config.tools)
        instance_prompt = self._render("instance.j2", problem=problem_statement)

        self.messages: list[dict] = [
            {"role": "system", "content": system_prompt},
            {"role": "user", "content": instance_prompt},
        ]
        self.trajectory: list[dict] = []  # saved after every step
```

**Why it works:** The entire agent state is a flat list of messages plus two scalars
(cost, steps). There is no hidden state, no mutable graph, no session object with
dozens of attributes. You can serialize the agent to JSON with `json.dumps(vars(agent))`
and reconstruct it perfectly.

The `trajectory` list is a separate concern: it records what happened for later analysis,
training-data extraction, or crash recovery. It is append-only.

### run() — The While-True Loop

```python
def run(self) -> dict:
    """Main loop. Returns the final trajectory."""
    try:
        while True:
            self.step()
    except SubmitAction as e:
        return {"patch": e.patch, "trajectory": self.trajectory, "cost": self.cost}
    except LimitsExceeded as e:
        return {"error": str(e), "trajectory": self.trajectory, "cost": self.cost}
```

This is the entire orchestration layer. Read it again — it is seven lines.

**Why it works:**

- **`while True`**: The loop does not decide when to stop. The *model* decides, by
  issuing a `submit` command. This is a critical design choice: the agent has autonomy
  over its own termination.

- **Exception-based control flow**: `SubmitAction` and `LimitsExceeded` are not error
  conditions — they are *structured exits*. Using exceptions for this is idiomatic Python
  and avoids polluting `step()` with exit-checking logic.

- **Single return shape**: Whether the agent succeeds or hits a limit, the caller gets
  a dict with the trajectory and cost. Uniform interfaces simplify downstream processing.

### step() — The Two-Line Method

```python
def step(self):
    """One turn of the agent loop."""
    response = self.query()            # call the model
    self.execute_actions(response)     # run whatever it said
```

This is not a simplification for exposition. This is the actual method. Two lines.

**Why it works:** `step()` is a *coordination point*, not a *decision point*. It does
not branch on the response type, does not check conditions, does not route to
sub-agents. It queries, then executes. The model made all the decisions already.

The extreme simplicity of `step()` has a practical benefit: when debugging, you never
need to ask "which branch did the agent take in step()?" There is only one path.

### query() — Check Limits, Call Model, Track Cost

```python
def query(self) -> dict:
    """Send the message history to the model and get a response."""
    # Guard: are we over budget?
    if self.cost > self.config.max_cost:
        raise LimitsExceeded(f"Cost {self.cost:.2f} > {self.config.max_cost}")
    if self.steps >= self.config.max_steps:
        raise LimitsExceeded(f"Steps {self.steps} >= {self.config.max_steps}")

    # The actual API call — one line of real work
    response = litellm.completion(
        model=self.config.model,
        messages=self.messages,
        temperature=0.0,
    )

    # Bookkeeping
    self.cost += response.usage.total_cost
    self.steps += 1

    assistant_msg = {"role": "assistant", "content": response.choices[0].message.content}
    self.messages.append(assistant_msg)
    self.trajectory.append({"step": self.steps, "message": assistant_msg, "cost": self.cost})

    return response.choices[0].message
```

**Why it works:**

- **Limit checks first.** The guard clauses at the top guarantee that the agent cannot
  overspend or run forever, regardless of what the model says. This is a *hard* safety
  boundary — not a suggestion, not a soft limit.

- **`litellm.completion` as the universal adapter.** By using litellm, the agent is
  model-agnostic. Swap `"claude-sonnet-4-20250514"` for `"gpt-4o"` and nothing else changes.
  This is what enables the "roulette" experiment (see below).

- **Cost tracking is a first-class concern.** The cost is updated *immediately* after
  every call. It is not deferred to a billing webhook or estimated later — it is part
  of the core loop. This matters because agent costs can spiral: a single stuck loop
  can burn $50 in minutes.

- **The message is appended to both `messages` and `trajectory`.** The message list is
  the *working memory* (what the model sees next turn). The trajectory is the *audit
  log* (what a human reviews later). They diverge when you implement context management,
  but in the simple loop they are identical.

### execute_actions() — Extract and Run

```python
def execute_actions(self, response) -> None:
    """Parse the model's response for tool calls and execute them."""
    content = response.content

    # Extract action blocks (e.g., ```bash\nls -la\n```)
    actions = self._parse_actions(content)

    results = []
    for action in actions:
        if action.tool == "submit":
            raise SubmitAction(patch=action.args)

        # Execute in a subprocess — fresh environment every time
        result = subprocess.run(
            action.command,
            shell=True,
            capture_output=True,
            text=True,
            timeout=30,
            cwd=self.workdir,
        )
        output = result.stdout + result.stderr
        # Truncate long outputs to preserve context window
        if len(output) > 10_000:
            output = output[:5_000] + "\n... (truncated) ...\n" + output[-5_000:]
        results.append(output)

    # Feed all results back as a single user message
    observation = "\n---\n".join(results)
    self.add_messages([{"role": "user", "content": observation}])
```

**Why it works:**

- **`subprocess.run` for isolation.** Each command runs in a fresh process. There is no
  shared shell state to corrupt, no `cd` that silently changes the working directory for
  all future commands.

- **Hard timeout.** `timeout=30` prevents hung processes from stalling the agent
  indefinitely. This is crude but effective — 30 seconds is enough for `grep`, `cat`,
  `python test.py`, and most build commands.

- **Output truncation.** The context window is finite. Dumping 500 KB of compiler output
  into the message list would push out the problem statement and earlier observations.
  The 10 000-character cap with head/tail preservation is a pragmatic trade-off.

- **`submit` as a raised exception.** When the model issues `submit`, execution does not
  continue to the next action. The exception unwinds the stack all the way back to
  `run()`. This is clean and avoids partial-execution bugs.

### add_messages() — Just list.extend

```python
def add_messages(self, messages: list[dict]) -> None:
    """Append messages to the conversation history."""
    self.messages.extend(messages)
    self.trajectory.extend(messages)
```

One line of real work (the second line is bookkeeping). This method exists as a named
concept so subclasses can override it — for example, to inject retrieval results or
apply context-window management.

---

## The "Roulette" Experiment

One of the most surprising findings in agent research comes from the SWE-agent team's
"roulette" experiment:

> **Randomly switching models between turns can IMPROVE results.**

The experiment: instead of using a single model for all turns, each `query()` call
randomly selects from a pool of frontier models (Claude, GPT-4o, etc.). The resulting
agent *outperforms* any single model on certain benchmarks.

### What This Tells Us

1. **The model matters more than the loop.** If the loop structure were the critical
   factor, randomly perturbing the "engine" would degrade performance. Instead, it
   sometimes helps — proving that the loop is a thin wrapper and the model is the
   substance.

2. **Ensemble effects.** Different models have different failure modes. Claude might
   struggle with a particular regex but nail the test generation. GPT-4o might
   over-engineer the fix but correctly identify the root cause. By mixing models,
   the agent gets a *diverse committee* instead of a single expert.

3. **Error recovery through diversity.** When model A produces a broken edit, model B
   (seeing the error message on the next turn) may recover more effectively than
   model A would, because it has a different prior on how to fix things.

### Implications for Agent Design

- **Invest in model selection, not loop complexity.** The highest-ROI improvement to
  most agents is upgrading the model, not adding a planning phase.

- **Multi-model strategies are underexplored.** Most agents hardcode a single model.
  The roulette result suggests that model mixing — even naive random mixing — is a
  promising research direction.

- **Benchmark results are model-specific.** When comparing agents, always control for
  the model. A "better" agent might just be using a better model.

---

## subprocess.run vs Persistent Shell

The two fundamental approaches to executing commands from an agent:

### Approach 1: subprocess.run (New Process Per Command)

```python
result = subprocess.run(
    ["bash", "-c", command],
    capture_output=True,
    text=True,
    timeout=30,
    cwd=workdir,
    env={**os.environ, "AGENT": "true"},
)
```

**Used by:** mini-SWE-agent, Pi (Anthropic's internal agent)

**Characteristics:**
- Fresh environment for every command
- No state carries over between commands (no `cd`, no exported variables)
- Each command explicitly specifies `cwd`
- Deterministic: same input → same output (no hidden shell state)
- Easy to timeout: `subprocess.run(timeout=N)` just works
- Easy to debug: reproduce any step by running the command in isolation

### Approach 2: Persistent Shell (Maintained Bash Session)

```python
class PersistentShell:
    def __init__(self):
        self.process = subprocess.Popen(
            ["bash", "--norc", "--noprofile"],
            stdin=subprocess.PIPE,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            text=True,
        )

    def run(self, command: str, timeout: float = 30) -> str:
        # Write command with a unique end-marker
        marker = f"__END_{uuid4().hex}__"
        self.process.stdin.write(f"{command}; echo {marker}\n")
        self.process.stdin.flush()

        # Read until we see the marker
        output = []
        deadline = time.time() + timeout
        for line in self.process.stdout:
            if marker in line:
                break
            if time.time() > deadline:
                raise TimeoutError(f"Command timed out: {command}")
            output.append(line)
        return "".join(output)
```

**Used by:** OpenCode, Goose, Claude Code (variants of this pattern)

**Characteristics:**
- Shell state persists: `cd /foo` affects all subsequent commands
- Environment variables, aliases, and shell functions carry over
- Faster: no process-startup overhead per command
- Can maintain complex environments (virtualenvs, nvm, etc.)
- Harder to timeout: killing the command without killing the shell is tricky
- Risk of hung processes or zombie children
- Non-deterministic replay: step N depends on the shell state from steps 1..N-1

### Trade-off Summary

| Concern | subprocess.run | Persistent Shell |
|---|---|---|
| Isolation | ✅ Perfect | ❌ Shared state |
| Speed | ❌ Process startup overhead | ✅ Fast |
| Timeout handling | ✅ Trivial | ⚠️ Complex |
| Debugging | ✅ Replay any step | ❌ Need full history |
| `cd` / env vars | ❌ Must be explicit | ✅ Natural |
| Long-running tasks | ❌ Timeout kills all | ✅ Can manage |
| Crash recovery | ✅ Stateless restart | ❌ Must rebuild state |

### What This Means for Agent Capabilities

The choice of execution model has downstream consequences:

- **subprocess.run agents** must include `cwd` context in every command. The model
  learns to write `cd /repo && grep -r "foo" src/` as a single command rather than
  relying on previous `cd` calls. This makes trajectories self-documenting.

- **Persistent-shell agents** can build up complex environments incrementally. This is
  essential for tasks like "set up a Python virtualenv, install dependencies, then run
  tests" — doing this with subprocess.run requires chaining everything into one command
  or managing the venv path explicitly.

- **For SWE-bench**, subprocess.run is sufficient because tasks are short (20–40 steps)
  and the repo is already set up. For open-ended coding tasks, persistent shells win.

---

## When to Choose Simple Over Complex

### Task Characteristics That Favor Simple Loops

| Characteristic | Why simple works |
|---|---|
| Single-file edits | No coordination needed |
| Short tasks (< 30 steps) | Context window is ample |
| Well-defined success criteria | Model can self-evaluate |
| Single-model capability | No orchestration needed |
| Research / fine-tuning | Clean trajectories = better training data |

### Research and Fine-Tuning: Trajectory IS Training Data

This is an underappreciated advantage. When your agent loop is simple, the trajectory
(the message list) is exactly what the model saw during inference. This makes it
*directly usable as training data* for fine-tuning:

```python
# After a successful run, the trajectory IS the training example
trajectory = agent.trajectory
if trajectory_solved_task(trajectory):
    training_data.append({
        "messages": agent.messages,  # exactly what the model saw
        "reward": 1.0,
    })
```

Complex agents with routing, sub-agents, and memory retrieval produce trajectories
that are *not* representable as a simple message list. You cannot fine-tune on them
without lossy reconstruction.

### Rapid Prototyping

A simple loop can be built in an afternoon:

1. Write the `while True` loop (10 minutes)
2. Wire up `litellm.completion` (5 minutes)
3. Parse actions with a regex (20 minutes)
4. Add `subprocess.run` (5 minutes)
5. Write prompt templates (2 hours — this is where the real work is)
6. Test on 3 examples (1 hour)

Total: one afternoon. You now have a working agent. It may not be *optimal*, but it is
*operational*, and you can iterate from a working baseline.

### When to Upgrade

Move to a more complex architecture when you observe *specific, measured* shortcomings:

- **Context overflow**: steps > 40, need context management → add summarization
- **Multi-file coordination**: edits span 5+ files → add a planning phase
- **Long-running tasks**: sessions last hours → add persistent state + resumption
- **Parallelism**: independent sub-tasks exist → add a fan-out/fan-in pattern
- **Specialized tools**: need browser, DB, API calls → add tool-specific execution

The key word is *measured*. Do not add a planning phase because "it seems like a good
idea." Add it because you have 10 failure cases where the agent edited file A before
reading file B, and a plan would have prevented that.

---

## Performance Implications

### Benchmark Data

The relationship between scaffold complexity and performance is not monotonic:

```
Scaffold Complexity →
Performance ↑

    │          ╭──── diminishing returns ────╮
    │         ╱                               ╲
    │        ╱                                 ── plateau
    │       ╱
    │      ╱
    │     ╱  ← sweet spot (mini-SWE-agent lives here)
    │    ╱
    │   ╱
    │  ╱
    │ ╱
    │╱
    └──────────────────────────────────────────────
```

After a certain threshold, adding scaffold complexity yields marginal gains while
adding significant maintenance burden, token overhead, and debugging difficulty.

### Token Efficiency

Simple loops have inherently lower token overhead:

```
Complex agent per-step overhead:
  System prompt (planner):     ~800 tokens
  System prompt (executor):    ~600 tokens  
  System prompt (validator):   ~500 tokens
  Routing logic:               ~200 tokens
  Total overhead per step:     ~2,100 tokens

Simple agent per-step overhead:
  System prompt:               ~800 tokens
  Total overhead per step:     ~800 tokens

Savings per step: ~1,300 tokens
Over 30 steps:   ~39,000 tokens saved
```

Those 39 000 tokens can carry approximately 30 KB of source code — enough context to
include the entire file being edited plus related test files.

### Latency

Each LLM call in a complex pipeline adds:
- Network round-trip: 100–500 ms
- Time to first token: 500–2 000 ms
- Full generation: 2 000–10 000 ms

A three-stage pipeline (plan → execute → validate) triples this per logical step.
A simple loop makes **one** call per step. For a 30-step task:

- Simple: 30 calls × ~5 s = ~2.5 minutes
- Three-stage: 90 calls × ~5 s = ~7.5 minutes

The simple agent is 3× faster with comparable accuracy.

### Cost

At $3/M input tokens and $15/M output tokens (Claude Sonnet-class pricing):

- Simple agent, 30 steps: ~$0.50–$2.00
- Three-stage pipeline, 30 steps: ~$1.50–$6.00

The cost difference compounds across thousands of benchmark evaluations or production
runs. mini-SWE-agent's cost tracking (`self.cost`) exists precisely because this matters.

---

## The Linear Message History

### Messages Go In, They Never Come Out

The simplest possible context-management strategy: **append-only**.

```python
# This is the entire "memory system" in mini-SWE-agent
self.messages.append(assistant_msg)   # what the model said
self.messages.extend(observations)    # what happened
# That's it. No eviction, no summarization, no RAG.
```

### Advantages

1. **Perfect reproducibility.** Given the same initial messages, the agent produces the
   same trajectory (assuming temperature=0). There is no hidden state that varies between
   runs.

2. **Trajectory = what model saw.** When debugging, you open the trajectory JSON and
   see *exactly* what the model received at each step. No reconstruction needed.

3. **Training-data quality.** The trajectory is a valid conversation that a model can
   learn from directly. No lossy projection from a complex state space.

### Limitations

The context window is finite. With a 200 K context window and ~800 tokens per step
(model response + observation), you get:

```
Available context after system prompt:  ~195,000 tokens
Tokens per step (average):             ~800
Maximum steps before overflow:         ~240 steps
```

This is ample for SWE-bench (20–40 steps typical) but insufficient for:
- Long debugging sessions (100+ steps)
- Exploratory coding with many file reads
- Tasks requiring large file contexts (> 50 KB files)

### Why It Works for SWE-bench

SWE-bench tasks have a specific profile:
- Single bug or feature in a well-defined location
- Typically requires reading 2–5 files
- Fix is usually < 50 lines of code
- 20–40 agent steps to locate, understand, fix, and verify

This fits comfortably within a linear message history. The context window never fills up
because the task is *bounded*.

### Why It Doesn't Work for Long Sessions

In an IDE assistant that runs for hundreds of turns across multiple tasks, a linear
history would:
- Exceed the context window within 30 minutes of active use
- Carry stale context from earlier tasks (confusing the model)
- Include irrelevant file contents from abandoned approaches

This is why production agents (Claude Code, Cursor, Copilot) implement context
management: sliding windows, summarization, selective eviction. But those are
*optimizations* — they add complexity to handle a specific failure mode of the simple
approach.

---

## Extending the Simple Loop

### Subclassing DefaultAgent

mini-SWE-agent is designed for extension through subclassing:

```python
class EnhancedAgent(DefaultAgent):
    """Extended agent with retrieval and reflection."""

    def step(self):
        self.pre_step_hooks()
        super().step()
        self.post_step_hooks()

    def pre_step_hooks(self):
        # Inject relevant context before the model sees the messages
        if self.steps > 0:
            relevant_files = self.retriever.search(self.last_observation)
            context = "\n".join(f.read_text() for f in relevant_files[:3])
            self.add_messages([{
                "role": "user",
                "content": f"[Context] Potentially relevant files:\n{context}"
            }])

    def post_step_hooks(self):
        # Validate the model's edit before continuing
        if self.last_action_was_edit():
            lint_result = subprocess.run(
                ["ruff", "check", self.last_edited_file],
                capture_output=True, text=True,
            )
            if lint_result.returncode != 0:
                self.add_messages([{
                    "role": "user",
                    "content": f"[Lint Error] {lint_result.stdout}\nPlease fix."
                }])
```

The core loop (`while True → step → query → execute`) is unchanged. The extensions
hook into the *boundaries* of each step, not the internals.

### Pi's Extension Model

Anthropic's internal agent Pi uses a similar philosophy:

```
Core loop (unchangeable):
  while not done:
    response = model.generate(messages)
    result = execute(response)
    messages.append(result)

Extensions (pluggable):
  on_before_generate:  inject context, modify messages
  on_after_generate:   validate, log, check guardrails
  on_before_execute:   sandbox setup, permission checks
  on_after_execute:    output filtering, cost tracking
  on_error:            retry logic, fallback strategies
```

The core stays simple. Extensions hook into *events*, not *control flow*. This is the
key distinction: extensions observe and augment, but they cannot change the fundamental
`generate → execute → append` cycle.

### The Philosophy

> **Start simple. Add complexity only when measured improvement justifies it.**

Every extension should come with:
1. A **specific failure mode** it addresses (not "it might help")
2. A **measured improvement** on a representative benchmark
3. An **understanding of the cost** (tokens, latency, code complexity)

If an extension cannot demonstrate all three, it is premature.

---

## Design Patterns from Simple Loops

### Pattern: Exit by Special Command

```python
# The model decides when it's done by issuing a special command
SUBMIT_PATTERN = re.compile(r"```submit\n(.*?)```", re.DOTALL)

def execute_actions(self, response):
    submit_match = SUBMIT_PATTERN.search(response.content)
    if submit_match:
        raise SubmitAction(patch=submit_match.group(1))
    # ... execute other actions
```

**Why this pattern:** The alternative — having the scaffold decide when to stop (e.g.,
"stop after the model says 'I'm done'") — requires natural-language parsing of intent.
A structured command (`submit`) is unambiguous. The model explicitly signals "I believe
the task is complete, here is my output."

**Variants:**
- `submit` with a patch (mini-SWE-agent)
- `attempt_completion` with a result message (Roo Code)
- `finish` with a summary (some custom agents)

### Pattern: Cost Tracking as a First-Class Concern

```python
class CostTracker:
    def __init__(self, max_cost: float):
        self.max_cost = max_cost
        self.total_cost = 0.0
        self.cost_per_step: list[float] = []

    def record(self, response) -> None:
        step_cost = response.usage.prompt_tokens * INPUT_PRICE + \
                    response.usage.completion_tokens * OUTPUT_PRICE
        self.total_cost += step_cost
        self.cost_per_step.append(step_cost)

    def check(self) -> None:
        if self.total_cost > self.max_cost:
            raise LimitsExceeded(
                f"Cost ${self.total_cost:.2f} exceeds limit ${self.max_cost:.2f}. "
                f"Average step cost: ${self.average_step_cost:.4f}"
            )

    @property
    def average_step_cost(self) -> float:
        return self.total_cost / len(self.cost_per_step) if self.cost_per_step else 0.0
```

**Why this pattern:** Agent costs are unpredictable. A model stuck in a loop can burn
through budget in minutes. Tracking cost *per step* (not just total) lets you detect
anomalies: if step 15 costs 10× the average, something went wrong.

### Pattern: Trajectory Saving After Every Step (Crash Recovery)

```python
def step(self):
    response = self.query()
    self.execute_actions(response)

    # Save after EVERY step — not just at the end
    self._save_trajectory()

def _save_trajectory(self):
    path = self.output_dir / f"trajectory_step_{self.steps:03d}.json"
    path.write_text(json.dumps({
        "steps": self.steps,
        "cost": self.cost,
        "messages": self.messages,
        "trajectory": self.trajectory,
    }, indent=2))
```

**Why this pattern:** Agents crash. Models return malformed responses, subprocesses
segfault, the network drops. By saving the trajectory after every step, you lose at
most one step of work. Combined with the linear message history, you can resume by
loading the last trajectory and continuing the loop.

**Practical benefit:** When running 500 SWE-bench evaluations overnight, some will
crash. Per-step saving means you can analyze partial trajectories (useful for
understanding *where* the agent went wrong) and potentially resume.

### Pattern: Exception-Based Control Flow

```python
class LimitsExceeded(Exception):
    """Raised when cost or step limits are hit."""
    pass

class SubmitAction(Exception):
    """Raised when the model submits a solution."""
    def __init__(self, patch: str):
        self.patch = patch

class FormatError(Exception):
    """Raised when the model's response can't be parsed."""
    pass

# Usage in the main loop
def run(self):
    try:
        while True:
            try:
                self.step()
            except FormatError as e:
                # Recoverable: tell the model about the format error
                self.add_messages([{
                    "role": "user",
                    "content": f"[Format Error] {e}. Please use the correct format."
                }])
    except (SubmitAction, LimitsExceeded) as e:
        return self._build_result(e)
```

**Why this pattern:** There are two kinds of exits: *recoverable* (format errors) and
*terminal* (submit, limits). Using exceptions with a nested try/except cleanly
separates these: the inner block catches and recovers, the outer block catches and
terminates.

This is cleaner than the alternative (checking return codes from `step()`) because:
- The happy path (`step()` succeeds) has zero overhead
- Terminal conditions propagate automatically through any call depth
- New exit conditions can be added without modifying `run()`

### Pattern: Template-Based Prompts (Jinja2)

```python
# templates/system.j2
You are an autonomous software engineering agent.

You have access to the following tools:
{% for tool in tools %}
## {{ tool.name }}
{{ tool.description }}
Usage: {{ tool.usage_example }}
{% endfor %}

# Rules
- Always explain your reasoning before acting
- Run tests after making changes
- Use `submit` when you believe the task is complete

# templates/instance.j2
## Problem Statement
{{ problem }}

## Repository Structure
{{ repo_structure }}

## Relevant Files (from BM25 retrieval)
{% for file in relevant_files %}
### {{ file.path }}
```{{ file.language }}
{{ file.content }}
```
{% endfor %}
```

**Why this pattern:** Prompts are the most important part of an agent — and the most
frequently iterated. Using Jinja2 templates:

1. **Separates prompt engineering from Python code.** Prompt authors do not need to touch
   the agent implementation.
2. **Enables A/B testing.** Swap `system_v2.j2` for `system_v1.j2` without code changes.
3. **Supports conditional content.** `{% if tool.name == "edit" %}` can add
   tool-specific instructions only when that tool is available.
4. **Makes prompts version-controllable.** `git diff templates/system.j2` shows exactly
   what changed in the prompt between experiments.

The template is rendered once at `__init__` time (for the system prompt) and the result
is a plain string in the message list. There is no ongoing template engine overhead.

---

## Summary

The simple agentic loop is not a toy. It is a *design choice* backed by empirical
evidence:

1. **~100 lines** is enough for a competitive agent
2. **The model does the work** — the scaffold is plumbing
3. **Linear message history** provides reproducibility and training-data quality
4. **subprocess.run** gives isolation at the cost of statefulness
5. **Exception-based exits** cleanly separate recoverable and terminal conditions
6. **Template prompts** decouple engineering from experimentation
7. **Cost tracking** is not optional — it is a safety boundary

Start here. Measure. Add complexity only when the data demands it.
