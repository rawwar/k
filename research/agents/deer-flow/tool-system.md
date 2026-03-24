---
title: DeerFlow Tool System
status: complete
---

# DeerFlow Tool System

## Overview

DeerFlow's tool system is organized around two complementary concepts:

1. **Skills** — Markdown files (`.md`) that define high-level workflows, best practices, and references to supporting resources. A skill is a declarative specification that the LLM reads to understand *how* to approach a class of tasks.
2. **Tools** — Python functions / MCP servers that give the agent concrete capabilities: web search, file I/O, bash execution, etc.

Skills orchestrate tools. Tools do the actual work.

---

## Skills: Capability Modules as Markdown

### What a Skill Is

A skill is a structured Markdown file stored at `/mnt/skills/public/<skill-name>/SKILL.md` (built-in) or `/mnt/skills/custom/<skill-name>/SKILL.md` (user-defined).

Example structure of a skill file:

```markdown
---
name: research
version: 1.0.0
author: DeerFlow Team
---

# Research Skill

## Purpose
Conduct deep, multi-source research on any topic.

## Workflow
1. Decompose the research question into sub-questions
2. Search for each sub-question using web_search
3. Fetch and read relevant pages with web_fetch
4. Cross-reference sources
5. Synthesize findings into a structured report

## Best Practices
- Always verify claims with at least 2 sources
- Prefer primary sources (papers, official docs) over aggregators
- Save intermediate findings to workspace/research-notes.md

## Tools Required
- web_search
- web_fetch
- file_write
- file_read

## Output Format
Produce a structured report with: Executive Summary, Findings (by section),
Sources (with URLs), and Confidence Assessment.
```

### Built-In Skills

| Skill | Path | Purpose |
|-------|------|---------|
| `research` | `/mnt/skills/public/research/SKILL.md` | Deep multi-source research |
| `report-generation` | `/mnt/skills/public/report-generation/SKILL.md` | Structured report writing |
| `slide-creation` | `/mnt/skills/public/slide-creation/SKILL.md` | Presentation slide decks |
| `web-page` | `/mnt/skills/public/web-page/SKILL.md` | Generate web pages |
| `image-generation` | `/mnt/skills/public/image-generation/SKILL.md` | Generate images |
| `claude-to-deerflow` | `/mnt/skills/public/claude-to-deerflow/SKILL.md` | Bridge: Claude Code → DeerFlow |

### Progressive Skill Loading

Skills load **on-demand** when the task needs them, not all at once at startup:

```
User: "Write a research report on X"
    │
    ├── Task classifier detects "research + report" intent
    ├── Loads: research/SKILL.md → added to context
    ├── Loads: report-generation/SKILL.md → added to context
    └── Does NOT load: slide-creation, web-page, image-generation
```

**Why this matters:**
- Each SKILL.md can be ~1,000–5,000 tokens
- Loading all skills at startup wastes context budget
- Token-sensitive models (e.g., those with 32K windows) need lean contexts
- Progressive loading keeps the active context focused on the current task

### Custom Skills

Users can install custom skills in two ways:

1. **Drop a `SKILL.md`** into `/mnt/skills/custom/<name>/`
2. **Install a `.skill` archive** via the Gateway API:
   ```
   POST /api/gateway/skills/install
   Content-Type: multipart/form-data
   file: my-skill.skill
   ```
   `.skill` archives support optional frontmatter: `version`, `author`, `compatibility`.

### The `claude-to-deerflow` Skill

A special skill that makes DeerFlow accessible directly from Claude Code:

```bash
npx skills add https://github.com/bytedance/deer-flow --skill claude-to-deerflow
```

Then from within a Claude Code session:
```
/claude-to-deerflow research the competitive landscape for [X] and produce a report
```

This creates a bidirectional bridge: Claude Code (CLI coding agent) can delegate research-heavy tasks to DeerFlow (super agent harness) without leaving the terminal.

---

## Core Tools

The built-in toolset available in the sandbox:

| Tool | Description | Runs In |
|------|-------------|---------|
| `web_search` | Web search via Tavily or InfoQuest | API call from sandbox |
| `web_fetch` | Fetch and parse a URL | HTTP from sandbox |
| `bash` | Execute shell commands | Docker container |
| `file_read` | Read file from workspace | `/mnt/user-data/` |
| `file_write` | Write file to workspace | `/mnt/user-data/` |
| `file_list` | List files in a directory | `/mnt/user-data/` |
| `python` | Execute Python code | Docker container |

### InfoQuest Integration

DeerFlow integrates **InfoQuest**, BytePlus's intelligent search and crawling toolset:

```yaml
# .env
INFOQUEST_API_KEY=your-infoquest-api-key
```

InfoQuest provides more structured search results than general-purpose search APIs, particularly useful for multi-source research tasks.

---

## MCP Server Support

DeerFlow supports configuring external MCP servers to extend its toolset:

```yaml
# config.yaml
mcp_servers:
  - name: my-db-server
    use: streamable_http
    url: https://my-mcp-server.example.com/mcp
    auth:
      type: client_credentials
      token_url: https://auth.example.com/token
      client_id: $CLIENT_ID
      client_secret: $CLIENT_SECRET
```

**OAuth flows supported**: `client_credentials`, `refresh_token`.

Any tool exposed by an MCP server becomes available to DeerFlow's agents. This allows organizations to connect DeerFlow to internal databases, APIs, or custom capabilities without modifying the core harness.

---

## Tool Execution Safety

Tools execute inside an isolated Docker container per session:

```
Docker container (per task session)
├── Network: controlled (outbound HTTP allowed; inbound blocked)
├── Filesystem: mounted volumes only (/mnt/skills, /mnt/user-data)
├── Resources: configurable CPU/memory limits
└── Lifetime: created at session start, destroyed at session end
```

**What the agent can do:**
- Run arbitrary bash commands in the container
- Read/write to `/mnt/user-data/workspace/` and `/mnt/user-data/outputs/`
- Make outbound HTTP requests for search and fetch

**What the agent cannot do:**
- Modify the host filesystem (only mounted volumes)
- Access other sessions' data
- Persist state beyond the mounted `/mnt/user-data/` volume

---

## Comparison with Other Agents' Tool Systems

| Aspect | DeerFlow | Claude Code | Goose | Codex CLI |
|--------|----------|-------------|-------|-----------|
| Tool definition | Skills (Markdown) + Python fns | TypeScript functions | MCP servers | Rust trait impls |
| Extensibility | Custom SKILL.md + MCP | MCP servers | MCP servers (native) | Custom tools |
| Skill/workflow spec | Markdown files | No equivalent | No equivalent | No equivalent |
| Progressive loading | Yes (skills on-demand) | No | No | No |
| Sandbox | Docker container | Host process | Host process | bubblewrap + Landlock |
| Skill format | `.md` + optional `.skill` archive | `.md` (CLAUDE.md, agent files) | GOOSE_INSTRUCTIONS | No equivalent |
