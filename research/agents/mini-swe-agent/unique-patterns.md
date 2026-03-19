# Unique Patterns & Design Insights

> This is the key file. mini-SWE-agent's value is not in what it does, but in what it proves you don't need.

## The Central Thesis

**As language models improve, scaffold complexity has diminishing returns.**

The SWE-agent team built one of the most sophisticated coding agents in 2024. Then they asked: what if we threw away 99% of it? The answer is mini-SWE-agent — and the fact that it performs within a few percentage points of vastly more complex systems is the most important finding.

This has implications beyond coding agents. It suggests a general principle for AI agent design: **invest in the model, not the framework**.

## Pattern 1: Why Minimal Works

### The Diminishing Returns Curve

In 2024, SWE-agent with its custom file editor, search tools, and navigation commands scored significantly better than a naive "just use bash" approach. The tool interfaces were genuinely valuable — they compensated for model limitations.

By 2025, this gap had nearly closed:
- Frontier models (Claude 4, GPT-5, Gemini 3 Pro) already know how to use `sed`, `grep`, `find`, `git`
- They can write multi-line file edits with `cat <<'EOF'`
- They understand error messages and can debug their own commands
- They can chain commands with `&&` and `||`

The tools that SWE-agent pioneered were essentially **training wheels** — critical for early models, unnecessary for capable ones.

### What the Mini Agent Proves

By scoring >74% on SWE-bench Verified with just bash, mini-SWE-agent provides empirical evidence that:

1. **Custom tools don't help much anymore** — the gap between "bash only" and "custom tools" is small for frontier models
2. **History processing isn't needed** — linear history works fine within modern context windows
3. **Stateless execution is sufficient** — persistent shells are a source of complexity, not capability
4. **Simple prompts are enough** — a clear system prompt with examples outperforms elaborate prompting schemes

### The Counter-Argument (And Why It's Weak)

"But complex agents score higher!" — Yes, by a few percentage points. But consider:
- **Maintenance cost**: Every custom tool is code to maintain, test, and debug
- **Brittleness**: More components = more failure modes
- **Overfitting**: Complex scaffolds may overfit to benchmark quirks
- **Deployment burden**: Custom tools need to be installed in every sandbox
- **Opacity**: Complex agents are harder to debug and understand

mini-SWE-agent's position is that the last few percentage points of performance are usually not worth the 100x increase in complexity.

## Pattern 2: `subprocess.run` vs Persistent Shell Sessions

This is one of mini-SWE-agent's most opinionated — and most important — design decisions.

### The Problem with Persistent Shells

Most coding agents (including SWE-agent, OpenHands, Devin) maintain a running shell session. Actions are typed into the session and output is read back. This seems natural but creates severe engineering challenges:

**1. When Has a Command Finished?**

You're reading a stream of bytes from a pseudo-terminal. How do you know when the command has terminated? The SWE-agent team tried multiple heuristics:
- Watching for the shell prompt to reappear (fragile — prompts vary)
- Monitoring PIDs (fragile — subprocesses complicate things)
- Using sentinel markers (fragile — programs might print them)

All were flaky. Every heuristic had edge cases that caused silent failures.

**2. Bad Commands Can Kill the Session**

If the LM issues a command that corrupts the shell session (e.g., `exec bash`, broken pipes, OOM kills), all subsequent commands fail. You need session recovery logic, health checks, and restart mechanisms.

**3. Command Interruption Corrupts State**

Timeout-killing a command in a persistent session can leave the shell in a broken state — half-written output, corrupted terminal settings, background processes still running.

### The `subprocess.run` Solution

mini-SWE-agent sidesteps ALL of these problems:

```python
result = subprocess.run(
    command,
    shell=True,
    text=True,
    cwd=cwd,
    env=os.environ | self.config.env,
    timeout=timeout,
    stdout=subprocess.PIPE,
    stderr=subprocess.STDOUT,
)
```

Each command runs in a **completely fresh subprocess**:
- It starts, runs, and exits — no ambiguity about when it's done
- A bad command can't corrupt future commands
- Timeout just kills the subprocess; nothing else is affected
- Stdout/stderr are captured cleanly via pipes

### The Trade-Off

The cost of stateless execution is that **state doesn't persist between commands**:
- `cd /some/dir` has no effect on the next command
- `export FOO=bar` doesn't set the variable for later commands
- Running a server in the background doesn't work

**But this isn't actually a problem in practice.** The LM learns to:
- Prefix commands: `cd /project && python test.py`
- Inline env vars: `PYTHONPATH=/project python script.py`
- Write to files for persistence across commands

The system prompt explicitly teaches this:

```
Directory or environment variable changes are not persistent. Every action is
executed in a new subshell. However, you can prefix any action with
`MY_ENV_VAR=MY_VALUE cd /path/to/working/dir && ...`
```

Models like Claude learn this pattern quickly and apply it consistently — some even do it without being told.

### Why This Is "A Big Deal" for Sandboxing

Stateless execution makes sandboxing **trivial**. To run in Docker instead of locally, you literally just swap the execution call:

```python
# Local execution
subprocess.run(command, shell=True, ...)

# Docker execution (conceptually)
subprocess.run(f"docker exec {container_id} bash -c '{command}'", ...)
```

No session management, no PTY forwarding, no container state tracking. This is why mini-SWE-agent supports Docker, Podman, Singularity, Apptainer, Bubblewrap, and Contree — each is just a different prefix to the command.

For SWE-bench evaluations at scale (running hundreds of instances in parallel), this stateless design means:
- Each instance is completely independent
- No shared state between parallel runs
- Failures in one instance can't affect others
- Scaling is just "run more processes"

## Pattern 3: Linear History for Debugging and Fine-Tuning

### The Identity Property

In mini-SWE-agent:

```
trajectory == messages == LM input == training data
```

These are all the same object. This seemingly simple property has profound consequences.

### For Debugging

When a run fails, you open the saved trajectory JSON and see exactly what the LM was prompted with at every step. There's no "the agent summarized the history at step 12 and this is what the LM actually saw" — the trajectory IS what the LM saw.

This makes debugging a simple matter of reading a JSON file:

```json
{
  "messages": [
    {"role": "system", "content": "You are a helpful assistant..."},
    {"role": "user", "content": "Please solve this issue: ..."},
    {"role": "assistant", "content": "THOUGHT: Let me explore..."},
    {"role": "tool", "content": "<returncode>0</returncode><output>...</output>"}
  ]
}
```

### For Fine-Tuning

Each trajectory is already in the standard chat format used for supervised fine-tuning:
- `messages[:-1]` = input context
- `messages[-1]` = target output (for each step)

No preprocessing needed. No format conversion. No "what did the agent actually see at this step?" guesswork.

### For RL Training

The linear history means reward signals (e.g., "did the task succeed?") can be attributed to individual steps without worrying about history compaction changing the context between steps:
- Every step saw exactly the history that preceded it
- No hidden state outside the message list
- Step N's context is deterministically `messages[0:N]`

This is why organizations doing RL for coding agents (including researchers at the institutions using mini-SWE-agent) favor the linear history design.

## Pattern 4: Stateless Execution Enables Trivial Sandboxing

This pattern extends beyond just "swap subprocess.run for docker exec." The statelessness principle enables:

### Parallel Evaluation

Each SWE-bench instance runs in complete isolation. Running 20 instances in parallel is the same as running 1 — just with 20 processes:

```bash
mini-extra swebench --subset verified --workers 20
```

There's no shared state to coordinate, no race conditions, no lock contention.

### Reproducibility

Because each command starts from a clean state, runs are more reproducible. A persistent shell accumulates state (env vars, directory changes, background processes) that can differ between runs.

### Fault Isolation

If one command hangs or crashes, it doesn't affect subsequent commands. The subprocess is killed, an error is recorded, and the agent continues with a fresh shell for the next action.

### Security

Stateless execution in a sandbox means the agent can't accumulate privileges or modify the host. Each command sees the same clean environment.

## Pattern 5: Put the LM in the Center, Not the Scaffold

This is the philosophical heart of mini-SWE-agent, and arguably its most important contribution to agent design thinking.

### The Scaffold-Centric Approach (2024)

The first generation of coding agents invested heavily in the scaffold:
- Custom tools with carefully designed interfaces
- History processors that curate what the LM sees
- Planning modules that structure the LM's approach
- Guardrails that prevent the LM from making mistakes
- Recovery mechanisms that fix the LM's errors

The implicit assumption: **the LM is unreliable, so we need to build intelligence around it.**

### The LM-Centric Approach (2025)

mini-SWE-agent inverts this assumption: **the LM is capable, so get out of its way.**

Instead of:
- Custom file editor → let the LM use `sed` and `cat`
- Search tool → let the LM use `grep` and `find`
- History compression → let the LM see everything
- Planning module → let the LM plan in its THOUGHT section
- Error recovery → let the LM see the error and fix it

The scaffold's job is minimal: query the model, execute its commands, show it the output, repeat. Everything else is the LM's responsibility.

### Why This Philosophy Matters

1. **Model improvements directly translate to agent improvements** — no scaffold ceiling
2. **No scaffold-model coupling** — switching models doesn't require scaffold changes
3. **No overfitting to scaffold** — fine-tuning on mini-SWE-agent trajectories teaches general coding skills, not scaffold-specific tool usage
4. **Simpler development** — less code to write, test, and debug
5. **Better research signal** — when comparing models, you're comparing models, not scaffold-model interactions

## Pattern 6: Exception-Based Control Flow

Using exceptions to interrupt the agent loop is an underappreciated design pattern:

```python
class InterruptAgentFlow(Exception):
    def __init__(self, *messages: dict):
        self.messages = messages
```

This allows any component at any call depth to signal important events:
- The environment detects completion → `raise Submitted(...)`
- A limit is exceeded → `raise LimitsExceeded(...)`
- The LM output is malformed → `raise FormatError(...)`
- The user presses Ctrl+C → `raise UserInterruption(...)`

The alternative (return values and flags) would require threading state through every function call. Exceptions make the control flow both simpler and more extensible — new exception types can be added without modifying existing code.

## Pattern 7: The Roulette Experiment — Emergent Ensemble Behavior

One of the most surprising findings: **randomly switching between models at each step outperforms either model alone.**

The implementation is comically simple:

```python
# Instead of: model.query(history)
random.choice([model1, model2]).query(history)
```

With GPT-5 + Sonnet 4, this achieved higher SWE-bench scores than either model separately (~66.6% vs ~63% each). The hypothesis is that different models have complementary strengths — when one model gets stuck, the other might approach the problem differently.

This is only possible because of mini-SWE-agent's design:
- Linear history means both models see the same context
- Stateless execution means no model-specific state accumulates
- Simple scaffold means the only variable IS the model

The roulette experiment is a vivid illustration of the LM-centric philosophy: when the scaffold is minimal, interesting model-level phenomena become visible.

## Summary: The Mini Principles

1. **Minimize the scaffold** — every line of scaffold code is a line that could have a bug
2. **Trust the LM** — modern models are capable; stop trying to compensate for them
3. **Keep state simple** — append-only message list, nothing hidden
4. **Keep execution stateless** — subprocess.run, not persistent sessions
5. **Keep history linear** — what the LM sees = what you save = what you train on
6. **Use exceptions for flow control** — simpler than threading return values
7. **Design for composability** — swap the environment, swap the model, swap the config
