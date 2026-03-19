# OpenCode — Tool System

## Overview

OpenCode's tool system is built around a simple Go interface pattern. Tools are defined as structs implementing the `BaseTool` interface, registered at agent construction time, and dispatched by name during the agentic loop. The system includes permission gating, file change tracking, LSP integration, and MCP (Model Context Protocol) support for external tool extensions.

## Tool Interface

The core abstraction is minimal:

```go
// internal/llm/tools/tools.go
type BaseTool interface {
    Info() ToolInfo
    Run(ctx context.Context, params ToolCall) (ToolResponse, error)
}

type ToolInfo struct {
    Name        string
    Description string
    Parameters  map[string]any   // JSON Schema
    Required    []string
}

type ToolCall struct {
    ID    string `json:"id"`
    Name  string `json:"name"`
    Input string `json:"input"`  // Raw JSON string
}

type ToolResponse struct {
    Type     toolResponseType `json:"type"`     // "text" or "image"
    Content  string           `json:"content"`
    Metadata string           `json:"metadata,omitempty"`
    IsError  bool             `json:"is_error"`
}
```

Key design decisions:
- **Input as raw JSON string**: Tools receive `Input` as a raw JSON string and unmarshal it themselves. This avoids a generic `map[string]any` that loses type information.
- **Error as content**: Tool errors are returned as `ToolResponse` with `IsError: true`, not as Go errors. Go errors are reserved for infrastructure failures (permission denied, context cancelled).
- **Metadata field**: Optional JSON metadata for structured data alongside the text response (e.g., diff statistics, timing info).

## Tool Registration

Tools are registered at agent creation time via composition functions in `internal/llm/agent/tools.go`:

```go
func CoderAgentTools(
    permissions permission.Service,
    sessions session.Service,
    messages message.Service,
    history history.Service,
    lspClients map[string]*lsp.Client,
) []tools.BaseTool {
    ctx := context.Background()
    otherTools := GetMcpTools(ctx, permissions)  // Discover MCP tools
    if len(lspClients) > 0 {
        otherTools = append(otherTools, tools.NewDiagnosticsTool(lspClients))
    }
    return append(
        []tools.BaseTool{
            tools.NewBashTool(permissions),
            tools.NewEditTool(lspClients, permissions, history),
            tools.NewFetchTool(permissions),
            tools.NewGlobTool(),
            tools.NewGrepTool(),
            tools.NewLsTool(),
            tools.NewSourcegraphTool(),
            tools.NewViewTool(lspClients),
            tools.NewPatchTool(lspClients, permissions, history),
            tools.NewWriteTool(lspClients, permissions, history),
            NewAgentTool(sessions, messages, lspClients),
        }, otherTools...,
    )
}

func TaskAgentTools(lspClients map[string]*lsp.Client) []tools.BaseTool {
    return []tools.BaseTool{
        tools.NewGlobTool(),
        tools.NewGrepTool(),
        tools.NewLsTool(),
        tools.NewSourcegraphTool(),
        tools.NewViewTool(lspClients),
    }
}
```

Notice:
- **Coder agent** gets the full tool set including write tools, bash, fetch, agent delegation, and MCP tools
- **Task agent** gets only read-only tools — it cannot modify files or execute commands
- Tools receive their dependencies via constructor injection (permissions, LSP clients, history service)
- MCP tools are dynamically discovered at startup

## Available Tools

### File and Code Tools

| Tool | File | Permission | Description |
|------|------|------------|-------------|
| `view` | `view.go` | No | Read file contents with optional offset/limit |
| `edit` | `edit.go` | Yes | String-replace editing (find old_string → replace with new_string) |
| `write` | `write.go` | Yes | Full file write/overwrite |
| `patch` | `patch.go` | Yes | Apply unified diff patches to files |
| `glob` | `glob.go` | No | Find files matching glob patterns |
| `grep` | `grep.go` | No | Search file contents with regex patterns |
| `ls` | `ls.go` | No | List directory contents (tree view) |

### Execution Tools

| Tool | File | Permission | Description |
|------|------|------------|-------------|
| `bash` | `bash.go` | Conditional | Execute shell commands (safe commands skip permission) |
| `fetch` | `fetch.go` | Yes | Fetch URLs and return content (HTML→markdown conversion) |

### Intelligence Tools

| Tool | File | Permission | Description |
|------|------|------------|-------------|
| `sourcegraph` | `sourcegraph.go` | No | Search code across public repos via Sourcegraph API |
| `diagnostics` | `diagnostics.go` | No | Get LSP diagnostics (errors, warnings) for files |
| `agent` | `agent-tool.go` | No | Delegate search tasks to a sub-agent |

### MCP Tools

| Tool | File | Permission | Description |
|------|------|------------|-------------|
| `{server}_{tool}` | `mcp-tools.go` | Yes | Dynamically discovered tools from MCP servers |

## Tool Implementation Deep-Dives

### Bash Tool (`bash.go`)

The bash tool is the most complex, with several safety mechanisms:

**Banned commands** (always blocked):
```go
var bannedCommands = []string{
    "alias", "curl", "curlie", "wget", "axel", "aria2c",
    "nc", "telnet", "lynx", "w3m", "links", "httpie", "xh",
    "http-prompt", "chrome", "firefox", "safari",
}
```

**Safe read-only commands** (skip permission):
```go
var safeReadOnlyCommands = []string{
    "ls", "echo", "pwd", "date", "cal", "uptime", "whoami", ...
    "git status", "git log", "git diff", "git show", ...
    "go version", "go help", "go list", "go env", ...
}
```

**Persistent shell**: Commands execute in a persistent shell session (via `shell.GetPersistentShell()`), meaning environment variables, virtual environments, and working directory persist across tool calls within a session.

**Timeout handling**: Default 1-minute timeout, max 10 minutes. Commands that exceed the timeout are killed.

**Output truncation**: Output is truncated at 30,000 characters, keeping the first and last halves:
```go
func truncateOutput(content string) string {
    if len(content) <= MaxOutputLength { return content }
    halfLength := MaxOutputLength / 2
    start := content[:halfLength]
    end := content[len(content)-halfLength:]
    truncatedLinesCount := countLines(content[halfLength : len(content)-halfLength])
    return fmt.Sprintf("%s\n\n... [%d lines truncated] ...\n\n%s",
        start, truncatedLinesCount, end)
}
```

**Metadata**: Returns start/end timestamps for timing information.

### Edit Tool (`edit.go`)

The edit tool uses a **string-replace strategy** (similar to Claude Code's `Edit` tool):

1. Takes `file_path`, `old_string`, `new_string`
2. Verifies `old_string` exists exactly once in the file
3. Requires the file to have been read first (via `view` tool)
4. Checks for concurrent modifications (compares file mod time vs last read time)
5. Generates a diff for the permission dialog
6. Writes the new content
7. Tracks the change in file history
8. Waits for LSP diagnostics and appends them to the response

Special cases:
- **Create new file**: `old_string` empty → creates file with `new_string` content
- **Delete content**: `new_string` empty → removes `old_string` from file

Safety checks:
```go
// Must read before editing
if getLastReadTime(filePath).IsZero() {
    return NewTextErrorResponse("you must read the file before editing it")
}

// Check for concurrent modifications
if modTime.After(lastRead) {
    return NewTextErrorResponse(fmt.Sprintf(
        "file %s has been modified since it was last read", filePath))
}

// Must be unique match
if index != lastIndex {
    return NewTextErrorResponse("old_string appears multiple times in the file")
}
```

### Patch Tool (`patch.go`)

An alternative to `edit` for larger changes. Accepts a unified diff format and applies it to the file. This is more efficient for multi-line changes than multiple `edit` calls.

### View Tool (`view.go`)

File reading with pagination support:
- `file_path`: Path to read
- `offset`: Starting line number (optional)
- `limit`: Number of lines to return (optional)

The tool tracks read times per-file to enable the edit tool's concurrent modification check.

### Agent Tool (`agent-tool.go`)

Spawns a sub-agent for search/exploration tasks:

```go
func (b *agentTool) Run(ctx context.Context, call tools.ToolCall) (tools.ToolResponse, error) {
    agent, _ := NewAgent(config.AgentTask, b.sessions, b.messages,
        TaskAgentTools(b.lspClients))
    session, _ := b.sessions.CreateTaskSession(ctx, call.ID, sessionID, "New Agent Session")
    done, _ := agent.Run(ctx, session.ID, params.Prompt)
    result := <-done
    return tools.NewTextResponse(response.Content().String()), nil
}
```

The sub-agent runs its own agentic loop with read-only tools. This enables the LLM to delegate exploratory searches without consuming the main agent's context.

### Sourcegraph Tool (`sourcegraph.go`)

Searches code across public repositories using the Sourcegraph API:
- `query`: Sourcegraph search query
- `count`: Max results (default 10)
- `context_window`: Lines of context around matches (default 3)
- `timeout`: Request timeout (default 10s)

This gives the agent access to reference implementations in open-source code.

### Fetch Tool (`fetch.go`)

Fetches URLs with HTML→markdown conversion:
- `url`: The URL to fetch
- `format`: Response format ("text" or "markdown")
- `timeout`: Optional timeout

Uses `JohannesKaufmann/html-to-markdown` for HTML conversion and `PuerkitoBio/goquery` for parsing.

### Diagnostics Tool (`diagnostics.go`)

Integrates with LSP clients to provide real-time code diagnostics:
- After file edits, waits briefly for LSP to process changes
- Returns errors, warnings, and information diagnostics
- Wraps output in `<file_diagnostics>` and `<project_diagnostics>` XML tags

## Tool Dispatch Mechanism

Tool dispatch happens in `streamAndHandleEvents()` after the LLM stream completes:

```go
for i, toolCall := range assistantMsg.ToolCalls() {
    // Linear search for matching tool
    var tool tools.BaseTool
    for _, availableTool := range a.tools {
        if availableTool.Info().Name == toolCall.Name {
            tool = availableTool
            break
        }
    }

    if tool == nil {
        toolResults[i] = message.ToolResult{
            ToolCallID: toolCall.ID,
            Content:    fmt.Sprintf("Tool not found: %s", toolCall.Name),
            IsError:    true,
        }
        continue
    }

    toolResult, toolErr := tool.Run(ctx, tools.ToolCall{
        ID: toolCall.ID, Name: toolCall.Name, Input: toolCall.Input,
    })
    // ... handle result
}
```

The dispatch is:
1. **Linear search** by tool name through the tool slice
2. **Sequential execution** — one tool at a time
3. **Error-tolerant** — tool-not-found is reported back to the LLM, not fatal
4. **Permission-aware** — permission denial cascades to cancel remaining tools

## Permission System

Tools that modify state require explicit user permission. The permission service acts as a blocking synchronization point:

```go
// permission/permission.go
func (s *permissionService) Request(opts CreatePermissionRequest) bool {
    // Auto-approve for non-interactive mode
    if slices.Contains(s.autoApproveSessions, opts.SessionID) {
        return true
    }

    // Check for existing persistent permission
    for _, p := range s.sessionPermissions {
        if p.ToolName == permission.ToolName && p.Action == permission.Action &&
           p.SessionID == permission.SessionID && p.Path == permission.Path {
            return true
        }
    }

    // Create channel and wait for user response
    respCh := make(chan bool, 1)
    s.pendingRequests.Store(permission.ID, respCh)
    defer s.pendingRequests.Delete(permission.ID)

    s.Publish(pubsub.CreatedEvent, permission)
    resp := <-respCh  // BLOCKS until user responds
    return resp
}
```

Permission levels:
- **Allow once**: Grants for this specific request only
- **Allow for session**: Grants for all similar requests (same tool, action, path, session)
- **Deny**: Rejects the request, tool returns `ErrorPermissionDenied`
- **Auto-approve**: Non-interactive mode auto-approves all permissions

## MCP Tool Discovery

External tools are discovered at startup via the Model Context Protocol:

```go
func GetMcpTools(ctx context.Context, permissions permission.Service) []tools.BaseTool {
    for name, m := range config.Get().MCPServers {
        switch m.Type {
        case config.MCPStdio:
            c, _ := client.NewStdioMCPClient(m.Command, m.Env, m.Args...)
            mcpTools = append(mcpTools, getTools(ctx, name, m, permissions, c)...)
        case config.MCPSse:
            c, _ := client.NewSSEMCPClient(m.URL, client.WithHeaders(m.Headers))
            mcpTools = append(mcpTools, getTools(ctx, name, m, permissions, c)...)
        }
    }
    return mcpTools
}
```

MCP tools:
- Are prefixed with the server name: `{server}_{tool}`
- Support both stdio and SSE transport
- Always require user permission before execution
- Initialize a fresh MCP client for each tool call (not persistent connections)

## File Change Tracking

Write tools (edit, write, patch) integrate with the history service to track changes:

```go
// In edit.go replaceContent():
// 1. Check if file exists in history
file, err := e.files.GetByPathAndSession(ctx, filePath, sessionID)
if err != nil {
    // First edit — save original content
    e.files.Create(ctx, sessionID, filePath, oldContent)
}

// 2. Check for manual changes
if file.Content != oldContent {
    e.files.CreateVersion(ctx, sessionID, filePath, oldContent)
}

// 3. Store the new version
e.files.CreateVersion(ctx, sessionID, filePath, newContent)
```

This enables the `/undo` command in the TUI to revert file changes per-session.

## Sandboxing Approach

OpenCode takes a **permission-based** approach rather than true sandboxing:

1. **No filesystem sandbox**: Tools have full filesystem access. Protection comes from the permission dialog.
2. **Command filtering**: Bash tool bans network commands (curl, wget, nc, etc.) and has a safe command whitelist.
3. **Working directory constraint**: Tools resolve relative paths against `config.WorkingDirectory()`, but absolute paths can access anywhere.
4. **Read-before-write**: Edit tool requires reading a file before modifying it, preventing blind writes.
5. **Concurrent modification detection**: Edit tool checks file modification times to prevent overwriting external changes.

This is less restrictive than Docker-based sandboxing (like some SWE-bench harnesses) but provides a practical balance for interactive use.

## LSP Integration with Tools

Several tools integrate with Language Server Protocol clients:

```go
// After edit/write/patch, wait for LSP to process changes
func waitForLspDiagnostics(ctx context.Context, filePath string,
    lspClients map[string]*lsp.Client) {
    // Brief sleep to let LSP process the file change
    // Then collect diagnostics
}

func getDiagnostics(filePath string, lspClients map[string]*lsp.Client) string {
    // Returns diagnostics wrapped in XML tags
    // <file_diagnostics>...</file_diagnostics>
    // <project_diagnostics>...</project_diagnostics>
}
```

This means the LLM gets immediate feedback on type errors, lint issues, etc., after every file modification — without needing a separate build/lint step.
