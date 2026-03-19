# Claude Code — Tool System

> Complete catalog of known tools, their permission requirements, and the MCP integration layer.

## Tool Catalog

Claude Code's tools are the mechanism that make it agentic. Without tools, Claude can only respond with text. With tools, it can act: read code, edit files, run commands, search the web, and interact with external services.

### Core Tools Reference

| Tool | Description | Permission Required | Category |
|------|-------------|-------------------|----------|
| `Read` | Reads file contents | No | File Operations |
| `Edit` | Makes targeted edits to specific files | Yes | File Operations |
| `Write` | Creates or overwrites files | Yes | File Operations |
| `NotebookEdit` | Modifies Jupyter notebook cells | Yes | File Operations |
| `Bash` | Executes shell commands | Yes | Execution |
| `Grep` | Searches for patterns in file contents (regex) | No | Search |
| `Glob` | Finds files based on pattern matching | No | Search |
| `WebFetch` | Fetches content from a URL | Yes | Web |
| `WebSearch` | Performs web searches | Yes | Web |
| `Agent` | Spawns a sub-agent with its own context window | No | Orchestration |
| `AskUserQuestion` | Asks multiple-choice questions to gather requirements | No | Orchestration |
| `EnterPlanMode` | Switches to plan mode for analysis before coding | No | Orchestration |
| `ExitPlanMode` | Presents a plan for approval and exits plan mode | Yes | Orchestration |
| `EnterWorktree` | Creates an isolated git worktree and switches into it | No | Orchestration |
| `ExitWorktree` | Exits a worktree session | No | Orchestration |
| `Skill` | Executes a skill within the main conversation | Yes | Orchestration |
| `LSP` | Code intelligence via language servers (type errors, go-to-definition, find references) | No | Code Intelligence |
| `ToolSearch` | Searches for deferred MCP tools on demand | No | MCP |
| `ListMcpResourcesTool` | Lists resources exposed by MCP servers | No | MCP |
| `ReadMcpResourceTool` | Reads a specific MCP resource by URI | No | MCP |
| `CronCreate` | Schedules a recurring/one-shot prompt within the session | No | Scheduling |
| `CronDelete` | Cancels a scheduled task by ID | No | Scheduling |
| `CronList` | Lists all scheduled tasks | No | Scheduling |
| `TaskCreate` | Creates a new task in the task list | No | Task Management |
| `TaskGet` | Retrieves task details | No | Task Management |
| `TaskList` | Lists tasks with status | No | Task Management |
| `TaskUpdate` | Updates task status/dependencies | No | Task Management |
| `TaskOutput` | Retrieves output from a background task | No | Task Management |
| `TaskStop` | Kills a running background task | No | Task Management |
| `TodoWrite` | Manages session task checklist (non-interactive/SDK mode) | No | Task Management |

### Tool Categories

**File Operations** — Read, search, and modify files:
- `Read` is permission-free; `Edit` requires session-level approval; `Write` requires per-use approval
- `Edit` applies to all built-in tools that edit files
- `Read` rules also apply (best-effort) to Grep and Glob

**Execution** — Run shell commands:
- `Bash` runs each command in a separate process
- Working directory persists across commands; environment variables do not
- Compound commands (`&&`) save separate permission rules per subcommand
- Virtualenv/conda must be activated before launching Claude Code

**Search** — Find files and content:
- `Grep` for content search (regex), `Glob` for file name patterns
- Both are read-only and permission-free

**Web** — External information:
- `WebFetch` for specific URLs, `WebSearch` for queries
- Both require permission; `WebFetch` supports domain-based rules

**Code Intelligence** — Via language servers:
- `LSP` reports type errors automatically after file edits
- Supports: go-to-definition, find references, type info, symbols, implementations, call hierarchies
- Requires a code intelligence plugin with the relevant language server binary

**Orchestration** — Agent coordination:
- `Agent` spawns sub-agents in separate context windows
- `AskUserQuestion` for structured requirement gathering
- Plan mode tools for analysis-before-action workflow
- Worktree tools for parallel isolated sessions

## Permission System

### Permission Tiers

| Tool Type | Examples | Approval Required | "Don't ask again" Behavior |
|-----------|----------|-------------------|---------------------------|
| Read-only | File reads, Grep, Glob | No | N/A |
| Bash commands | Shell execution | Yes | Permanently per project + command |
| File modification | Edit, Write | Yes | Until session end |

### Permission Modes

| Mode | Description | How to Activate |
|------|-------------|-----------------|
| `default` | Prompts for permission on first use of each tool | Default behavior |
| `acceptEdits` | Auto-accepts file edits; still asks for Bash | `Shift+Tab` once |
| `plan` | Read-only — Claude can analyze but not modify | `Shift+Tab` twice |
| `dontAsk` | Auto-denies unless pre-approved via allow rules | Settings |
| `bypassPermissions` | Skips all prompts except protected directories | `--dangerously-skip-permissions` |

### Permission Rule Syntax

Rules follow the format `Tool` or `Tool(specifier)`:

```
# Match all uses of a tool
Bash          # All bash commands
WebFetch      # All web fetches

# Exact match with specifier
Bash(npm run build)      # Only this specific command
Read(./.env)             # Only this file

# Wildcard patterns (glob-style)
Bash(npm run *)          # npm run anything
Bash(git commit *)       # git commit with any args
Bash(* --version)        # Any --version command
Read(src/**)             # All files under src/

# Domain-based web fetch
WebFetch(domain:example.com)

# MCP tool permissions
mcp__puppeteer                      # All puppeteer tools
mcp__puppeteer__puppeteer_navigate  # Specific tool

# Sub-agent permissions
Agent(Explore)         # The Explore sub-agent
Agent(my-custom-agent) # A custom sub-agent
```

### Rule Evaluation Order

Rules evaluate in strict order: **deny → ask → allow**. First match wins.

- A deny in managed settings **cannot** be overridden by any other level
- A deny at project level overrides an allow at user level
- Managed settings have highest precedence, then CLI args, local, project, user

### Permission Configuration Example

```json
{
  "permissions": {
    "allow": [
      "Bash(npm run lint)",
      "Bash(npm run test *)",
      "Read(~/.zshrc)"
    ],
    "deny": [
      "Bash(curl *)",
      "Read(./.env)",
      "Read(./.env.*)",
      "Read(./secrets/**)"
    ],
    "defaultMode": "acceptEdits"
  }
}
```

### Read/Edit Path Patterns

Path rules follow gitignore specification with four pattern types:

| Pattern | Meaning | Example |
|---------|---------|---------|
| `//path` | Absolute from filesystem root | `Read(//Users/alice/secrets/**)` |
| `~/path` | From home directory | `Read(~/Documents/*.pdf)` |
| `/path` | Relative to project root | `Edit(/src/**/*.ts)` |
| `path` or `./path` | Relative to current directory | `Read(*.env)` |

## Bash Tool Behavior

Key details about the Bash tool:

- **Process isolation**: Each command runs in a separate process
- **Working directory persists**: `cd` in one command affects the next
- **Environment variables do NOT persist**: `export` in one command is lost in the next
- **Workaround for env vars**: Set `CLAUDE_ENV_FILE` to a shell script, or use a SessionStart hook
- **Reset to project dir**: Set `CLAUDE_BASH_MAINTAIN_PROJECT_WORKING_DIR=1`

## MCP Integration

### What MCP Provides

MCP (Model Context Protocol) is an open standard for AI-tool integrations. Claude Code connects to MCP servers to access external tools:

- **Issue trackers**: Jira, Linear, GitHub Issues
- **Databases**: PostgreSQL, MySQL via dbhub
- **Monitoring**: Sentry, Datadog
- **Design**: Figma
- **Communication**: Slack, Gmail
- **Browsers**: Playwright, Puppeteer
- **Code platforms**: GitHub, GitLab

### MCP Transports

| Transport | Use Case |
|-----------|----------|
| `http` | Remote HTTP servers (recommended for cloud services) |
| `sse` | Server-Sent Events (older remote protocol) |
| `stdio` | Local process servers (direct system access) |
| `ws` | WebSocket connections |

### MCP Scopes

| Scope | Location | Shared? |
|-------|----------|---------|
| **Local** | `~/.claude.json` per project path | No |
| **Project** | `.mcp.json` in project root (git-committed) | Yes |
| **User** | `~/.claude.json` globally | No |

### MCP Tool Search

When many MCP servers are configured, tool definitions consume significant context. **Tool Search** solves this:

- Automatically enabled when MCP tools exceed 10% of context window
- MCP tools are deferred rather than preloaded
- Claude uses a search tool to discover relevant MCP tools on demand
- Only tools actually needed are loaded into context
- Configurable via `ENABLE_TOOL_SEARCH` environment variable

### Using Claude Code AS an MCP Server

Claude Code itself can serve as an MCP server for other applications:

```bash
claude mcp serve   # Start Claude as a stdio MCP server
```

This allows Claude Desktop or other MCP clients to use Claude Code as a tool provider.

## Hooks System

Hooks are deterministic shell scripts that run at lifecycle points. Unlike CLAUDE.md (advisory), hooks are enforced:

### Hook Events

| Event | When It Fires |
|-------|---------------|
| `PreToolUse` | Before a tool call — can deny, force prompt, or allow |
| `PostToolUse` | After a tool call completes |
| `SessionStart` | When a session begins |
| `Notification` | When Claude needs attention (permission, idle, auth) |
| `WorktreeCreate` | When a worktree is created |
| `WorktreeRemove` | When a worktree is removed |
| `Elicitation` | When MCP server requests structured input |

### Hook Precedence with Permissions

PreToolUse hooks run before the permission prompt. A hook can:
- **Deny** the tool call (exit code 2)
- **Force a prompt** — override auto-allow to ask
- **Skip the prompt** — but deny/ask rules still override

This means hooks extend but cannot bypass the permission system.

## Key Observations

1. **Tool-use is native API**: Claude Code uses Anthropic's built-in tool-use (function-calling) protocol, not prompt injection. Tools are defined in the API request, and the model returns structured `tool_use` blocks.

2. **Permission granularity is exceptional**: The allow/ask/deny system with glob patterns, domain matching, path patterns, and MCP tool matching is the most granular permission system in any coding agent.

3. **Bash is the power tool and the risk**: Most capability flows through Bash — builds, tests, git, package managers. The permission system and sandboxing are primarily designed to control Bash.

4. **LSP integration via plugins**: Code intelligence (type checking, go-to-definition) requires separate plugin installation, not built into core. This is a modular design choice.

5. **MCP Tool Search is context-aware**: Dynamically loading MCP tools only when needed is a sophisticated approach to the context budget problem. It's analogous to lazy-loading in web applications.
