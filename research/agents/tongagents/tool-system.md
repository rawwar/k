# TongAgents — Tool System

> ⚠️ **Limited information available.** No source code or API documentation has been published. Tool capabilities are inferred from Terminal-Bench 2.0 task requirements.

## What Terminal-Bench Tasks Require

Terminal-Bench 2.0 covers 89 tasks across several categories. To score 80.2%, TongAgents must have tools for:

### Shell Execution (Required)
Every Terminal-Bench task involves CLI interaction. The agent must have:
- **Command execution** — run arbitrary shell commands and capture stdout/stderr
- **Exit code handling** — detect success vs failure from return codes
- **Interactive command support** — handle commands that prompt for input (e.g., `apt install -y`)
- **Long-running command management** — timeouts, background processes

### File System Operations (Required)
Many tasks involve reading and writing configuration files:
- **File reading** — view file contents, search within files
- **File writing/editing** — create, modify, append to files
- **Directory navigation** — list, create, traverse directory structures
- **Permission management** — chmod, chown operations

### Task Categories and Implied Tools

| Terminal-Bench Category | Implied Tool Capabilities |
|------------------------|--------------------------|
| Server configuration | Service management, config file editing, networking |
| Debugging | Log analysis, process inspection, strace/ltrace |
| Security | User management, firewall rules, certificate handling |
| Data science | Python/R execution, package management, data parsing |
| Compilation | Build systems (make, cmake), dependency resolution |

## Likely Tool Architecture

Given the multi-agent hypothesis, tools are probably:

1. **Centralized** — a shared tool interface that any agent can call
2. **Sandboxed** — executed within the Terminal-Bench Docker environment
3. **Instrumented** — tool calls and outputs are logged for the observation step

### Minimal Tool Set (High Confidence)

```
- shell_execute(command, timeout)    → Run a shell command
- file_read(path)                    → Read file contents
- file_write(path, content)          → Write/overwrite a file
- file_edit(path, old, new)          → Surgical file editing
```

### Extended Tool Set (Moderate Confidence)

More sophisticated agents often include:
```
- shell_execute_background(command)  → Run without blocking
- file_search(pattern, directory)    → Find files by name/content
- process_list()                     → List running processes
- network_check(host, port)          → Verify service availability
```

## Model-Specific Tool Calling

The performance gap between Gemini 3.1 Pro (80.2%) and Claude Opus 4.6 (~71.9%) could be partly explained by differences in **tool calling reliability**:

- Gemini and Claude have different tool-calling APIs and conventions
- Some models are better at structured output (JSON tool calls) than others
- The agent must translate between its internal tool representation and each model's format

## What We Don't Know

- Whether tools are defined as function schemas or embedded in prompts
- The exact set of tools available to the agent
- Whether the agent can dynamically discover or create tools
- How tool errors are surfaced to the reasoning loop
- Whether there are domain-specific tools beyond basic shell/file operations
- If the agent uses any form of tool documentation or few-shot examples for tool use