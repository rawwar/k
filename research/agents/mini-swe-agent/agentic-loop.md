# The Agentic Loop

> The entire agent is a two-line `step()` method inside a while loop. This file is the annotated deep dive.

## The Core Insight

mini-SWE-agent's agentic loop is a textbook **ReAct (Reasoning + Acting)** implementation stripped to its absolute minimum. The entire loop fits in one sentence: *query the model, execute the actions, repeat until done.*

What makes it remarkable is not what it does -- it's what it deliberately doesn't do. There is no planning step, no reflection step, no retrieval step, no tool selection step. Just query -> execute -> append -> repeat.

## The Complete Agent Source (Annotated)

Below is the actual `DefaultAgent` class from `src/minisweagent/agents/default.py`, annotated section by section.

### Configuration

```python
class AgentConfig(BaseModel):
    """Check the config files in minisweagent/config for example settings."""

    system_template: str
    """Template for the system message (the first message)."""
    instance_template: str
    """Template for the first user message specifying the task (the second message overall)."""
    step_limit: int = 0
    """Maximum number of steps the agent can take."""
    cost_limit: float = 3.0
    """Stop agent after exceeding (!) this cost."""
    output_path: Path | None = None
    """Save the trajectory to this path."""
```

Only 5 configuration fields. Compare this to SWE-agent which has dozens of config options for tools, history processing, guardrails, etc.

### Initialization

```python
class DefaultAgent:
    def __init__(self, model: Model, env: Environment, *, config_class: type = AgentConfig, **kwargs):
        self.config = config_class(**kwargs)
        self.messages: list[dict] = []
        self.model = model
        self.env = env
        self.extra_template_vars = {}
        self.logger = logging.getLogger("agent")
        self.cost = 0.0
        self.n_calls = 0
```

The agent takes a `Model` and an `Environment` -- that's it. No tool registry, no memory store, no planner. The entire mutable state is `self.messages`, `self.cost`, and `self.n_calls`.

### The Run Loop

```python
def run(self, task: str = "", **kwargs) -> dict:
    """Run step() until agent is finished. Returns dictionary with exit_status, submission keys."""
    self.extra_template_vars |= {"task": task, **kwargs}
    self.messages = []
    self.add_messages(
        self.model.format_message(role="system", content=self._render_template(self.config.system_template)),
        self.model.format_message(role="user", content=self._render_template(self.config.instance_template)),
    )
    while True:
        try:
            self.step()
        except InterruptAgentFlow as e:
            self.add_messages(*e.messages)
        except Exception as e:
            self.handle_uncaught_exception(e)
            raise
        finally:
            self.save(self.config.output_path)
        if self.messages[-1].get("role") == "exit":
            break
    return self.messages[-1].get("extra", {})
```

**Walk-through:**

1. **Initialize messages** with the system prompt and the task description (rendered from Jinja2 templates)
2. **Enter the main loop** -- call `step()` repeatedly
3. **Handle `InterruptAgentFlow`** -- exceptions like `Submitted`, `LimitsExceeded`, `FormatError` carry messages that get appended to the trajectory
4. **Handle uncaught exceptions** -- add an exit message and re-raise
5. **Save after every step** -- trajectory is persisted even if the agent crashes
6. **Check for exit** -- if the last message has `role="exit"`, we're done

### The Step Method -- The Heart of the Agent

```python
def step(self) -> list[dict]:
    """Query the LM, execute actions."""
    return self.execute_actions(self.query())
```

**This is the entire step method.** Two function calls composed together. This is the philosophical core of mini-SWE-agent: a step is nothing more than "ask the LM what to do, then do it."

### Query

```python
def query(self) -> dict:
    """Query the model and return model messages. Override to add hooks."""
    if 0 < self.config.step_limit <= self.n_calls or 0 < self.config.cost_limit <= self.cost:
        raise LimitsExceeded(
            {
                "role": "exit",
                "content": "LimitsExceeded",
                "extra": {"exit_status": "LimitsExceeded", "submission": ""},
            }
        )
    self.n_calls += 1
    message = self.model.query(self.messages)
    self.cost += message.get("extra", {}).get("cost", 0.0)
    self.add_messages(message)
    return message
```

**Walk-through:**

1. **Check limits** -- if we've exceeded step or cost limits, raise `LimitsExceeded` (which carries an exit message)
2. **Increment counter**
3. **Query the model** -- pass the ENTIRE message history (no truncation, no summarization)
4. **Track cost** from the response metadata
5. **Append the assistant's message** to the history
6. **Return the message** for action execution

### Execute Actions

```python
def execute_actions(self, message: dict) -> list[dict]:
    """Execute actions in message, add observation messages, return them."""
    outputs = [self.env.execute(action) for action in message.get("extra", {}).get("actions", [])]
    return self.add_messages(*self.model.format_observation_messages(message, outputs, self.get_template_vars()))
```

**Walk-through:**

1. **Extract actions** from the model's response (`message["extra"]["actions"]`)
2. **Execute each action** via the environment (typically `subprocess.run`)
3. **Format observations** -- the model wrapper renders the output into the observation template
4. **Append observation messages** to the history

### Message Management

```python
def add_messages(self, *messages: dict) -> list[dict]:
    self.logger.debug(messages)
    self.messages.extend(messages)
    return list(messages)
```

Just `list.extend`. That's it. No filtering, no summarization, no priority queue, no sliding window. Messages go in, they never come out.

## Control Flow Diagram

```
run(task)
|
+-- Initialize: [system_msg, user_msg]
|
+-- while True:
    |
    +-- step()
    |   |
    |   +-- query()
    |   |   +-- Check limits -> LimitsExceeded?
    |   |   +-- model.query(ALL messages)
    |   |   +-- Track cost
    |   |   +-- add_messages(assistant_response)
    |   |
    |   +-- execute_actions(response)
    |       +-- env.execute(action) for each action
    |       |   +-- subprocess.run(command) -> output
    |       |   +-- _check_finished(output) -> Submitted?
    |       +-- model.format_observation_messages(outputs)
    |       +-- add_messages(observations)
    |
    +-- catch InterruptAgentFlow -> add_messages(e.messages)
    +-- catch Exception -> handle_uncaught_exception(e), raise
    +-- finally: save(trajectory)
    |
    +-- if messages[-1].role == "exit": break
```

## What a Typical Run Looks Like

For a SWE-bench task, a typical trajectory has ~20-40 steps:

```
Step  Role        Content (abbreviated)
----  ----------  -----------------------------------------
  0   system      "You are a helpful assistant..."
  1   user        "Please solve this issue: ..."
  2   assistant   "THOUGHT: Let me explore... ```find . -name '*.py'```"
  3   tool        "<returncode>0</returncode><output>src/main.py...</output>"
  4   assistant   "THOUGHT: Let me read the file... ```cat src/main.py```"
  5   tool        "<returncode>0</returncode><output>def foo():..."
  ...
 38   assistant   "THOUGHT: Fix verified. ```echo COMPLETE_TASK_AND_SUBMIT_FINAL_OUTPUT```"
 39   exit        "Submitted"
```

Every message is preserved. The trajectory saved to disk is identical to what was passed to the LM at the last step. This is invaluable for:
- **Debugging**: You can replay exactly what the LM saw
- **Fine-tuning**: The trajectory IS the training data
- **RL**: Reward signals can be attached directly to trajectories

## Why No Planning Step?

Many agent frameworks include explicit planning phases (e.g., "first create a plan, then execute it"). mini-SWE-agent deliberately omits this because:

1. **The LM plans implicitly** -- the system prompt includes a "Recommended Workflow" that guides the LM through exploration -> reproduction -> fix -> verification
2. **Explicit plans become stale** -- in a dynamic coding environment, plans made before exploration are often wrong
3. **The THOUGHT prefix** in each response serves as step-level reasoning without a separate planning mechanism

The system prompt's "Recommended Workflow" section acts as a soft plan:

```yaml
instance_template: |
  ## Recommended Workflow
  1. Analyze the codebase by finding and reading relevant files
  2. Create a script to reproduce the issue
  3. Edit the source code to resolve the issue
  4. Verify your fix works by running your script again
  5. Test edge cases to ensure your fix is robust
  6. Submit your changes
```

## Why No Reflection Step?

Similarly, there's no explicit "reflect on what happened" step:

1. **The LM sees the full history** -- it can naturally reflect on failures when it sees error output
2. **Linear history preserves context** -- unlike agents that compact history, mini-SWE-agent's LM always has complete information about what it tried and what went wrong
3. **Fewer moving parts** -- reflection prompts are another source of prompt engineering complexity

## Extending the Loop

The loop is designed for easy extension via subclassing:

```python
class MyCustomAgent(DefaultAgent):
    def step(self) -> list[dict]:
        # Add pre-step logic (e.g., retrieval)
        result = super().step()
        # Add post-step logic (e.g., reflection)
        return result

    def query(self) -> dict:
        # Add pre-query hooks
        message = super().query()
        # Add post-query hooks
        return message
```

The `query()` docstring explicitly says "Override to add hooks" -- the minimal implementation is meant to be a foundation, not a ceiling.

## The Pseudocode Version

From the official tutorial at minimal-agent.com, the entire agent concept in ~15 lines:

```python
messages = [{"role": "user", "content": "Help me fix the ValueError in main.py"}]
while True:
    lm_output = query_lm(messages)
    messages.append({"role": "assistant", "content": lm_output})
    action = parse_action(lm_output)
    if action == "exit":
        break
    output = execute_action(action)
    messages.append({"role": "user", "content": output})
```

The production `DefaultAgent` adds error handling, cost tracking, serialization, and template rendering -- but the fundamental loop is identical to this pseudocode. That's the point.
