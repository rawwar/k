---
title: "Execution Models"
---

# Execution Models

How coding agent tools actually run — from synchronous blocking calls to persistent shell sessions and parallel dispatch.

Every coding agent must decide *how* to execute the tools the LLM requests. This seemingly simple
question — "run this command" — branches into a tree of design decisions that profoundly affect
reliability, performance, and capability. The execution model determines whether an agent can run
tests while editing files, whether environment variables survive between commands, whether a hung
process halts the entire agent loop, and whether the agent can recover from partial failures.

---

## 1. Synchronous Execution

The simplest execution model: the agent loop blocks until the tool completes, then feeds the
result back to the LLM. Most agents use this as their default or only model.

### How It Works

```
Agent Loop:
  1. LLM generates tool_call(name, args)
  2. Agent dispatches to tool handler
  3. Tool handler BLOCKS until completion
  4. Result returned to agent loop
  5. Result appended to conversation
  6. Back to step 1
```

In OpenCode, tools return a `ToolResponse` synchronously. The tool executor calls the tool's
`Run()` method and waits for it to return:

```go
// OpenCode tool execution (simplified)
func (e *Executor) Execute(call ToolCall) ToolResponse {
    tool := e.registry.Get(call.Name)
    result, err := tool.Run(call.Params)  // blocks here
    if err != nil {
        return ToolResponse{Error: err.Error()}
    }
    return ToolResponse{Content: result}
}
```

In mini-SWE-agent, the pattern is even more explicit — `subprocess.run` is inherently blocking:

```python
# mini-SWE-agent: blocking subprocess execution
result = subprocess.run(
    command,
    shell=True,
    capture_output=True,
    timeout=timeout,
    text=True
)
return result.stdout + result.stderr
```

### Trade-offs

**Advantages:**
- Dead simple to implement and reason about
- Deterministic ordering — tool A always completes before tool B starts
- Easy to debug: stack traces are linear, logs are sequential
- No race conditions, no shared state conflicts
- Error handling is straightforward (try/catch around the call)

**Disadvantages:**
- Slow for I/O-heavy workflows (e.g., running tests then reading results)
- A single slow tool blocks the entire agent loop
- Cannot overlap independent operations (edit file A while running tests on file B)
- Timeout handling becomes critical — a hung tool freezes everything

Most agents start synchronous and add async capabilities only when performance demands it.

---

## 2. Asynchronous Execution

Non-blocking tool execution allows the agent to dispatch work and continue processing without
waiting for each tool to complete. This is architecturally more complex but enables parallel
operations and better throughput.

### Goose: Async via Tokio Runtime

Goose is written in Rust and uses the `tokio` async runtime for tool dispatch. MCP tool calls
are sent asynchronously, and the runtime manages concurrent execution:

```rust
// Goose: async MCP tool invocation (conceptual)
async fn call_tool(&self, name: &str, params: Value) -> Result<Value> {
    let client = self.mcp_client.clone();
    let response = client.call_tool(name, params).await?;
    Ok(response.content)
}
```

The tokio runtime allows Goose to dispatch multiple MCP calls concurrently when the LLM
requests parallel tool use. Each call is a future that resolves independently.

### OpenHands: Event-Driven Async

OpenHands uses an event stream architecture. Actions are dispatched as events, and observations
come back asynchronously through the `EventStreamRuntime`:

```python
# OpenHands: event-driven execution (conceptual)
class EventStreamRuntime:
    async def execute(self, action: Action) -> Observation:
        event_id = self.event_stream.publish(action)
        observation = await self.event_stream.wait_for(event_id)
        return observation
```

This decouples tool invocation from result handling. The runtime can process multiple actions
concurrently, and the agent controller manages result ordering.

### Handling Async Results

Agents use several patterns to handle asynchronous results:

- **Futures/Promises**: Goose awaits futures directly. The agent loop suspends at await points
  and resumes when results arrive.
- **Event Streams**: OpenHands publishes actions and subscribes to observations. The event bus
  handles routing and ordering.
- **Callbacks**: Some frameworks register completion callbacks that fire when a tool finishes,
  appending results to the conversation.

The key challenge is *ordering*: when tools complete out of order, the agent must decide how to
present results to the LLM. Most agents collect all parallel results and present them together
in the order they were requested, not the order they completed.

---

## 3. Persistent Shell Sessions

Most production agents keep a bash shell session alive across multiple tool calls. This is one
of the most consequential design decisions — it determines whether state accumulates naturally
(like a human developer's terminal) or must be explicitly reconstructed each time.

### Why Persistence Matters

A developer working in a terminal builds up state: they `cd` into a project, set environment
variables, activate virtual environments, define aliases. Persistent shells let agent tools
behave the same way:

```bash
# Call 1: Navigate and setup
cd /workspace/myproject && source venv/bin/activate

# Call 2: This works because CWD and venv persist
pip install -r requirements.txt

# Call 3: Environment is still active
python -m pytest tests/
```

Without persistence, each of those commands would need the full context repeated.

### Implementation Patterns

**OpenCode** creates a persistent shell via `GetPersistentShell()`. The shell process stays
alive between tool invocations, preserving working directory, environment variables, and
shell state:

```go
// OpenCode: persistent shell (simplified from shell.go)
func GetPersistentShell() *Shell {
    if persistentShell == nil {
        persistentShell = &Shell{
            cmd: exec.Command("bash", "--norc", "--noprofile"),
        }
        persistentShell.cmd.Start()
    }
    return persistentShell
}
```

Commands are written to the shell's stdin, and output is read from stdout/stderr. Sentinel
markers delimit where one command's output ends and the next begins.

**Gemini CLI** similarly maintains a persistent shell session. Commands execute within the
same process, so `export FOO=bar` in one call makes `$FOO` available in the next.

**OpenHands** runs a persistent shell *inside a Docker container*. The shell persists for the
entire agent session, and because it's containerized, the accumulated state is isolated from
the host system.

### PTY (Pseudo-Terminal) Management

Some agents use PTY allocation for their persistent shells. A PTY provides proper terminal
emulation, which matters for:

- **Interactive programs**: Tools like `git log`, `less`, or `top` behave differently without
  a TTY. Many programs disable color output or change formatting when they detect no terminal.
- **Line discipline**: PTYs handle line editing, signal delivery (Ctrl+C), and terminal
  resizing — important for programs that use ncurses or similar libraries.
- **Output capture**: PTY-based output includes ANSI escape codes for colors and formatting.
  Agents must decide whether to strip these or pass them through.

Warp terminal pioneered a *block-based* PTY model where each command's input and output is
treated as a discrete block. This concept influenced how some agents structure their tool
output — each command invocation produces a self-contained block of output rather than a
continuous stream.

Agents that capture PTY output typically:
1. Allocate a PTY pair (master/slave)
2. Run the shell connected to the slave end
3. Read from the master end to capture output
4. Strip ANSI escape codes before sending to the LLM (most models cannot use them)
5. Preserve raw output for display to the user

### State Accumulation

Persistent shells accumulate state in ways that can be both powerful and dangerous:

**Useful accumulation:**
- Working directory changes (`cd` persists)
- Environment variables (`export PATH=$PATH:/new/dir`)
- Shell functions and aliases
- Virtual environment activation
- SSH agent forwarding

**Dangerous accumulation:**
- Background processes left running
- Modified shell options (`set -e`, `shopt` changes)
- Overridden PATH entries hiding expected binaries
- Resource leaks (open file descriptors, temp files)

### Comparison Table

| Agent | Shell Persistence | Env Vars Persist? | CWD Persists? | Isolation |
|-------|------------------|-------------------|---------------|-----------|
| OpenCode | Persistent process | Yes | Yes | Host process |
| Claude Code | Separate per command | No | Yes (tracked) | Process-level |
| Codex | Sandboxed per command | Per-sandbox | Per-sandbox | Network-disabled sandbox |
| Gemini CLI | Persistent process | Yes | Yes | Host process |
| OpenHands | Persistent in Docker | Yes | Yes | Container-level |
| mini-SWE-agent | New subprocess each | No | No | Process-level |
| Goose | Persistent shell | Yes | Yes | Host process |

Claude Code is notable for tracking CWD *without* a persistent shell — it maintains the
current directory in agent state and prepends `cd /tracked/dir &&` to each command. This gives
the *illusion* of persistence for directory navigation while maintaining process isolation.

---

## 4. Stateless Subprocess Model

The opposite extreme from persistent shells: every tool invocation spawns a fresh subprocess.
mini-SWE-agent is the canonical example of this approach.

### How mini-SWE-agent Does It

Each action creates a brand-new subprocess with no memory of previous commands:

```python
# mini-SWE-agent: stateless subprocess execution
def execute_command(command: str, timeout: int) -> str:
    result = subprocess.run(
        command,
        shell=True,
        capture_output=True,
        timeout=timeout,
        text=True,
        cwd=self.working_dir  # explicitly set each time
    )
    return result.stdout + result.stderr
```

Because nothing persists, the system prompt teaches the model to chain commands:

```
# This WON'T work (cd is lost between calls):
# Call 1: cd /workspace/project
# Call 2: ls src/  <-- runs in wrong directory

# This WILL work (single command chain):
cd /workspace/project && ls src/
```

### Environment Configuration

Since environment variables do not persist between calls, mini-SWE-agent configures them
once via YAML in the action space definition:

```yaml
# mini-SWE-agent: environment configuration
env_variables:
  PAGER: "cat"               # Prevent interactive pagers
  PIP_PROGRESS_BAR: "off"    # Disable progress bars
  GIT_TERMINAL_PROMPT: "0"   # Prevent git auth prompts
  PYTHONDONTWRITEBYTECODE: "1"
```

These are injected into every subprocess's environment, ensuring consistent behavior
without relying on shell state.

### Trade-offs: Stateless vs Persistent

| Dimension | Stateless Subprocess | Persistent Shell |
|-----------|---------------------|------------------|
| **Isolation** | Natural — each command is fresh | Must be managed (cleanup, reset) |
| **Simplicity** | No shell state management needed | Complex lifecycle management |
| **Performance** | Process creation overhead per call | One-time setup, fast subsequent calls |
| **Debugging** | No session history to inspect | Rich history within session |
| **Reproducibility** | High — same input, same output | State-dependent (order matters) |
| **State management** | Explicit (pass everything each time) | Implicit (state accumulates) |
| **Error recovery** | Trivial — just spawn a new process | May need to reset corrupted state |
| **Multi-step workflows** | Verbose (repeat context each time) | Natural (state carries forward) |

The stateless model trades convenience for reliability. In production SWE-bench evaluations,
mini-SWE-agent's stateless approach performs well because each command is self-contained —
there is no risk of accumulated state causing subtle bugs across a long agent trajectory.

The persistent model trades reliability for ergonomics. It is closer to how humans work and
requires fewer tokens (no repeated `cd` and `export` commands), but introduces a class of
bugs that only manifest after many tool calls.

---

## 5. Parallel Tool Execution

When an LLM returns multiple tool calls in a single response, should the agent execute them
sequentially or in parallel? This decision affects both performance and correctness.

### Agent Support Matrix

| Agent | Parallel? | Mechanism | Notes |
|-------|-----------|-----------|-------|
| OpenHands | Yes | Multiple tool_calls processed together | Event stream handles concurrency |
| Claude Code | Yes | Multiple tool_use blocks | Read operations parallelized |
| Codex | Yes | Multiple FunctionCall items | Sandbox isolation enables safety |
| Goose | Yes | Async dispatch via tokio | MCP calls run concurrently |
| Gemini CLI | Yes | Multiple tool calls supported | Implementation-dependent |
| OpenCode | No | Sequential, one at a time | Simpler but slower |
| mini-SWE-agent | No | Single command per step | By design — one action per turn |
| Aider | N/A | No function calling | Uses edit blocks in text |

### Benefits of Parallel Execution

1. **Throughput**: Reading 5 files in parallel takes ~1x the time instead of ~5x
2. **Latency hiding**: Slow I/O operations overlap with fast computations
3. **Better LLM utilization**: The model can request all information it needs at once
4. **Natural batching**: "Read these 3 files and run the tests" executes optimally

### Challenges

**Resource contention**: Two parallel file edits to the same file create a race condition.
Agents must detect conflicting operations and serialize them:

```python
# Pseudocode: conflict detection for parallel tool calls
def can_parallelize(calls: list) -> list:
    """Group tool calls into parallelizable batches."""
    batches = []
    current_batch = []
    written_files = set()

    for call in calls:
        target = call.get_target_file()
        if target and target in written_files:
            # Conflict -- start new batch
            batches.append(current_batch)
            current_batch = [call]
            written_files = {target}
        else:
            current_batch.append(call)
            if target:
                written_files.add(target)

    if current_batch:
        batches.append(current_batch)
    return batches
```

**Ordering dependencies**: "Create the file, then run the tests" must be sequential. The agent
(or the LLM) must understand causal dependencies between operations.

**Error handling**: If 3 of 5 parallel operations succeed and 2 fail, should the agent:
- Report all results (including partial successes)?
- Roll back the successes?
- Retry only the failures?

Most agents report all results and let the LLM decide how to handle partial failures.

**Determinism**: Parallel execution introduces non-determinism in completion order. Agents
typically restore the *request order* when presenting results to the LLM, regardless of which
tool finished first.

### How Agents Determine Parallelizability

Most agents delegate this decision to the LLM. When the model returns multiple `tool_use`
blocks in a single response, the agent treats them as parallelizable. The model is expected
to understand that "read file A" and "read file B" are independent, while "write file A"
and "read file A" are not.

Some agents add guardrails:
- **Read-only operations** are always safe to parallelize
- **Write operations** to different files can run in parallel
- **Write operations** to the same file are serialized
- **Shell commands** may or may not be safe depending on side effects

---

## 6. Timeout Management

Timeouts are critical safety mechanisms. Without them, an LLM-generated infinite loop or a
network request to a dead server would hang the agent indefinitely.

### Timeout Configuration Across Agents

| Agent | Default Timeout | Max Timeout | Termination Behavior |
|-------|----------------|-------------|----------------------|
| OpenCode | 60s | 10min | Force stop process |
| OpenHands | Configurable | Per parameter | Force terminate container cmd |
| Codex | Configurable | Per policy | Terminate + report to LLM |
| mini-SWE-agent | Configurable | Per YAML config | subprocess.run raises TimeoutExpired |
| Ante | 30s | Configurable | Returns ToolError::Timeout |
| ForgeCode | 300s | FORGE_TOOL_TIMEOUT env | Terminate hung commands |
| Goose | Per-extension | 300s default | MCP protocol timeout |

### Why Timeouts Matter

LLMs can and do generate commands that run forever:

```python
# LLM might generate this trying to "watch" for changes
while True:
    check_status()
    time.sleep(1)
```

```bash
# Or a network request to an unreachable host
curl https://internal-service.company.com/api  # hangs for minutes
```

Without timeouts, any of these would freeze the agent. The timeout is the last line of defense
against runaway execution.

### Graceful vs Hard Termination

**Graceful shutdown** (SIGTERM, then wait, then force stop):

```python
# Two-phase termination pattern
import signal

def terminate_gracefully(process, timeout=5):
    process.send_signal(signal.SIGTERM)    # ask nicely first
    try:
        process.wait(timeout=timeout)      # give it time to clean up
    except subprocess.TimeoutExpired:
        process.send_signal(signal.SIGKILL)  # force if unresponsive
```

This gives the process a chance to clean up (flush buffers, remove temp files, release locks)
before forcibly terminating it. Most production agents use this pattern.

**Hard termination** (immediate forced stop):
Simpler but risks:
- Corrupted files (partial writes)
- Leaked resources (temp files, network sockets)
- Zombie child processes

### Cascading Timeouts

Production agents often implement multiple timeout layers:

```
Per-command timeout:  60s  (individual tool call)
Per-step timeout:    300s  (entire agent step including LLM call)
Per-task timeout:   3600s  (entire task/conversation)
```

If a per-command timeout fires, only that command is stopped. If a per-step timeout fires,
the entire step is abandoned. If a per-task timeout fires, the agent shuts down gracefully.

---

## 7. Resource Limits

Beyond timeouts, agents need to constrain CPU, memory, disk, and network usage to prevent
runaway processes from destabilizing the host system.

### CPU Limits

Docker-based agents (OpenHands, Codex in container mode) use cgroups to limit CPU:

```bash
# Docker CPU limits
docker run --cpus="2.0" --cpu-shares=1024 agent-sandbox

# Equivalent cgroup configuration
echo 200000 > /sys/fs/cgroup/cpu/agent/cpu.cfs_quota_us
echo 100000 > /sys/fs/cgroup/cpu/agent/cpu.cfs_period_us
```

This prevents a CPU-intensive build from starving other processes on the host.

### Memory Limits

Memory limits prevent OOM (Out of Memory) conditions from crashing the host:

```bash
# Docker memory limits
docker run --memory="4g" --memory-swap="4g" agent-sandbox
```

When a process exceeds its memory limit, the OOM handler terminates it. Agents must handle
this gracefully — detecting OOM events and reporting them to the LLM as actionable errors
rather than opaque crashes.

### Disk Limits

Agents constrain disk usage through:
- **tmpfs mounts**: In-memory filesystems with size limits for temp directories
- **Quota enforcement**: Filesystem quotas on the workspace directory
- **Cleanup policies**: Periodic removal of build artifacts and cache files

```bash
# tmpfs with size limit for temp directory
docker run --tmpfs /tmp:size=512m agent-sandbox
```

### Network Limits

Network restrictions are perhaps the most important resource limit for security:

| Approach | Used By | Restrictions |
|----------|---------|-------------|
| No network | Codex (default) | All outbound blocked |
| Allowlist | Codex (--full-auto) | Only approved domains |
| Docker network | OpenHands | Configurable per container |
| Host network | OpenCode, Gemini CLI | No restrictions (host trust) |

Codex's default of *no network access* is the most restrictive. This prevents data
exfiltration, dependency confusion attacks, and accidental API calls. The trade-off is that
the agent cannot install packages or access external services during execution.

### Compound Resource Policies

Production deployments typically combine multiple limits:

```yaml
# Example: composite resource policy
resources:
  cpu:
    cores: 2
    throttle_at: 90%
  memory:
    limit: 4GB
    swap: 0          # No swap -- fail fast on OOM
  disk:
    workspace: 10GB
    temp: 512MB
  network:
    mode: allowlist
    allowed:
      - "*.npmjs.org"
      - "*.pypi.org"
      - "github.com"
  timeout:
    per_command: 120s
    per_session: 3600s
```

The interaction between these limits matters. A memory-constrained process might swap to disk,
consuming disk I/O. A CPU-limited build might take longer, hitting timeouts. Good resource
policies consider these interactions and set limits that work together rather than creating
cascading failures.

---

## Summary

The execution model is a foundational architectural decision that ripples through every aspect
of an agent's behavior:

| Model | Best For | Watch Out For |
|-------|----------|---------------|
| Synchronous | Simple agents, debugging | Blocking on slow operations |
| Asynchronous | High-throughput, parallel I/O | Complexity, ordering bugs |
| Persistent Shell | Natural developer workflows | State accumulation bugs |
| Stateless Subprocess | Reproducibility, isolation | Verbosity, repeated setup |
| Parallel Dispatch | Multi-file operations | Race conditions, conflicts |

The trend in production agents is toward **persistent shells with async dispatch and
comprehensive resource limits** — combining the ergonomics of a real terminal with the
safety guarantees needed for autonomous operation.
