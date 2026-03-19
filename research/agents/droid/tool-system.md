# Droid — Tool System

> How Droid's tool system enables autonomous code manipulation, with categories for reading, editing, executing, searching, and extensibility via skills, slash commands, and MCP servers.

## Core Tool Categories

Factory Analytics provides visibility into Droid's tool system by categorizing every tool call. The core categories are:

### Read Tools

File reading and content inspection:
- Read file contents
- Read directory listings
- Read git diffs and history
- Read PR details, review comments, CI status

### Edit Tools

Code and file modification:
- Create new files
- Edit existing files (precise string replacement)
- Delete files
- Batch edits across multiple files

### Execute Tools

Command execution and process management:
- Run shell commands (build, test, lint)
- Execute in sandboxed environments
- Run CI/CD pipelines
- Execute database queries (for analytics)

### Grep Tools

Pattern-based code search:
- Regex search across files
- Content matching with context lines
- File-scoped and project-wide search

### Glob Tools

File discovery by pattern:
- Wildcard pattern matching for file paths
- Directory traversal with pattern filters
- File type filtering

## Skills System

Skills are higher-level, reusable capabilities that compose multiple tool calls into domain-specific workflows. From the Analytics data, we know:

- **Skills invocations** are tracked as a distinct metric in Factory Analytics.
- Skills represent a level of abstraction above raw tool calls.
- Adoption of skills is considered a "leading indicator of effective usage" per Factory's documentation.

Skills likely include patterns like:
- **Code review skill**: Read diff → analyze against guidelines → generate structured feedback.
- **Migration skill**: Discover patterns → plan changes → apply across files → verify tests pass.
- **Incident response skill**: Read error logs → trace to source → propose fix → create PR.

## Slash Commands

Slash commands provide quick access to specific Droid capabilities within interactive sessions:

- Tracked as a distinct adoption metric in Factory Analytics.
- Provide shortcuts for common workflows.
- Similar to the `/add`, `/ask`, `/code` pattern seen in other agents, but specific to Droid's multi-interface context.

## MCP Server Integration

Droid supports **MCP (Model Context Protocol) servers** as an extensibility mechanism:

- Factory Analytics tracks how many users have configured MCP servers.
- MCP servers extend Droid's tool system with custom capabilities.
- This allows enterprises to add domain-specific tools (internal APIs, proprietary systems, custom databases) without modifying the core agent.

## Tool Analytics

Factory Analytics provides deep visibility into tool usage:

### Autonomy Ratio

The ratio of tool calls to user messages — a key indicator of delegation effectiveness:
- **High ratio** (e.g., 13x): Droid is executing complex tasks autonomously.
- **Low ratio**: Developers are using Droid more conversationally, providing frequent guidance.

### Tool Call Distribution

Analytics breaks down usage by tool type, revealing:
- **Read-heavy** patterns: Teams using Droid for codebase exploration and review.
- **Edit-heavy** patterns: Teams using Droid for active code generation and refactoring.
- **Execute-heavy** patterns: Teams using Droid for testing and CI/CD workflows.

### Platform Feature Adoption

Analytics tracks adoption of advanced features as leading indicators:

| Feature | What It Indicates |
|---------|-------------------|
| Skills invocations | Usage of higher-level automation workflows |
| Slash commands | Familiarity with interactive capabilities |
| MCP servers configured | Enterprise extensibility adoption |
| Autonomy ratio trend | Growing trust in agent delegation |

## GitHub Actions Integration

Droid has specific tooling for CI/CD repair:

- **`github_action_repair`**: When enabled in `.droid.yaml`, Droid analyzes GitHub Actions failures and suggests fixes.
- This operates as an automated tool invocation triggered by CI events.
- The repair tool reads failure logs, identifies the root cause, and either comments with a fix or creates a repair PR.

## Review-Specific Tools

The code review workflow uses specialized tools:

- **PR summary generation**: Automatically creates a structured summary of changes.
- **File-level summaries**: Per-file change descriptions.
- **Guideline matching**: Applies path-specific review rules from `.droid.yaml`.
- **Skip reason commenting**: When review is skipped, explains why (WIP title, excluded label, etc.).

## Integration-Aware Tools

Droid's tool system extends beyond code to connected integrations:

- **Project management tools**: Read/update Jira tickets, Linear issues, Asana tasks.
- **Knowledge tools**: Query Notion, Confluence, Google Drive for relevant documentation.
- **Incident tools**: Read Sentry errors, PagerDuty alerts for context during incident response.
- **Communication tools**: Send status updates via Slack, Microsoft Teams.

This integration awareness allows Droid to perform "full-stack" engineering tasks — not just writing code, but understanding the task context, updating project tracking, and communicating results through appropriate channels.