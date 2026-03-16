# LLM Content Authoring Guide

This guide describes how to write, extend, and maintain learning content for the **Build a CLI Coding Agent** platform. It is addressed to LLM authors and human contributors alike. Follow every section carefully — consistency across chapters is what makes the platform feel cohesive.

The platform is a VitePress site with two parallel learning tracks:

- **Project track** (`project/`) — build-first, one feature per chapter
- **Linear track** (`linear/`) — concept-first, topic-oriented chapters

Both tracks share the same file conventions, content standards, and code project.

---

## 1. Content Standards

### Length and scope

Each subchapter targets **800 – 2000 words** of prose (excluding code blocks). If a topic needs more than 2000 words, split it into two subchapters. If it comes in under 800, merge it with an adjacent topic.

### Required structural elements

Every subchapter must include these elements in order:

1. **Frontmatter** with `title` and `description` fields.
2. **"What you'll learn" box** — a blockquote immediately after the `# Heading`, containing 2 – 3 bullet points.
3. **Body content** — explanations, code examples, callout boxes.
4. **"Key Takeaways" section** — an `## Key Takeaways` heading with 3 – 5 bullet points summarizing the subchapter.

Example skeleton:

```markdown
---
title: Process Spawning
description: Learn how to spawn child processes in Rust using std::process::Command.
---

# Process Spawning

> **What you'll learn:**
> - How to use `std::process::Command` to spawn synchronous child processes
> - How to use `tokio::process::Command` for async, non-blocking process execution
> - How to handle exit statuses and propagate errors from spawned processes

Your prose goes here...

## Key Takeaways

- Bullet one
- Bullet two
- Bullet three
```

### Code examples

All code examples must be **complete and runnable**. Do not use snippets with ellipses (`...`) or unexplained omissions. If a function depends on imports or helper code, include them. Every code block must specify its language for syntax highlighting:

````markdown
```rust
use std::process::Command;

fn main() {
    let output = Command::new("echo")
        .arg("hello from the agent")
        .output()
        .expect("failed to execute process");

    println!("{}", String::from_utf8_lossy(&output.stdout));
}
```
````

When a complete program would be excessively long, show the full function and state which file it belongs to:

```markdown
In `src/tools/shell.rs`, add the following function:
```

### "Coming from Python" callout boxes

Whenever Rust introduces a concept that diverges significantly from Python, include a comparison callout using the custom `::: python` container. These are styled with a yellow border and a snake emoji.

```markdown
::: python Coming from Python
In Python you would run a subprocess with `subprocess.run()`:
```python
import subprocess
result = subprocess.run(["echo", "hello"], capture_output=True, text=True)
print(result.stdout)
```
Rust's `Command` API serves the same purpose but returns a `Result` that you must handle explicitly — there is no equivalent of silently swallowing errors.
:::
```

Include at least one per subchapter when the Rust approach meaningfully differs from the Python way.

### "In the wild" callout boxes

Use the `::: wild` container to reference how production coding agents implement the pattern being discussed. Draw from the research notes in `research/agents/` and `research/concepts/`.

```markdown
::: wild In the Wild
Claude Code spawns all shell commands through a sandboxed executor that intercepts
dangerous patterns like `rm -rf /` before the command reaches the OS. OpenCode takes
a different approach, relying on its permission system to block dangerous commands
at the approval step rather than at execution time.
:::
```

### Writing style

- **Conversational tone** — write as if you are pair-programming with the reader.
- **Second person, present tense** — "you'll build", "let's implement", "you pass the command to".
- **Active voice** — "Rust checks the borrow at compile time" not "the borrow is checked at compile time by Rust".
- **No filler** — every sentence should teach something or motivate what comes next.
- **Target audience** — a Python developer who is comfortable writing production Python but is learning Rust and agent architecture for the first time.

---

## 2. How to Write a New Subchapter

### Step 1: Create the file

Subchapter files live inside their chapter folder and follow the naming convention:

```
NN-slug-name.md
```

- `NN` is a two-digit, zero-padded number (`01`, `02`, ..., `12`).
- `slug-name` is a lowercase, hyphenated description.

Example path:

```
learn/project/06-shell-execution/04-timeouts.md
```

The sidebar is **auto-generated from the filesystem** (see `.vitepress/config.ts`). The `title` field in frontmatter becomes the sidebar label. No manual sidebar configuration is needed — just make sure the filename sorts correctly.

### Step 2: Add frontmatter

Every subchapter requires exactly two frontmatter fields:

```yaml
---
title: Timeouts and Cancellation
description: Implement timeout logic for shell commands to prevent runaway processes.
---
```

- `title` — appears in the sidebar, browser tab, and search results. Keep it concise (2 – 6 words).
- `description` — one sentence summarizing what the reader will learn. Used by search engines and link previews.

### Step 3: Write the content

Follow this internal structure:

1. `# Title` heading (must match or closely mirror the frontmatter `title`).
2. "What you'll learn" blockquote (2 – 3 bullets).
3. Introductory paragraph connecting this topic to what came before.
4. Core explanation with code examples.
5. "Coming from Python" callout where Rust diverges from Python.
6. "In the wild" callout referencing production agents (when relevant).
7. `## Key Takeaways` section (3 – 5 bullets).

### Step 4: Update the code project

If the subchapter introduces new code, update the corresponding code snapshot in `learn/code/chNN/`. The code project must compile after your changes:

```bash
cd learn/code/ch06
cargo check
```

### Step 5: Cross-reference

Use **relative links** for cross-references between chapters and subchapters:

```markdown
As you saw in [Error Handling Basics](/project/01-hello-rust-cli/08-error-handling-basics),
Rust uses `Result<T, E>` instead of exceptions.
```

Link to the other track when covering the same concept from a different angle:

```markdown
For a deeper conceptual treatment, see the linear track's
[Unix Process Model](/linear/07-process-management-and-shell/01-unix-process-model).
```

---

## 3. How to Create a New Chapter

### Step 1: Create the chapter folder

Chapters live under `project/` or `linear/` with the naming convention:

```
NN-chapter-name/
```

Example:

```
learn/project/08-terminal-ui-with-ratatui/
```

### Step 2: Create `index.md`

Every chapter needs an `index.md` that serves as the chapter landing page. It must contain:

1. **Frontmatter** with a chapter-style title and description.
2. **Opening paragraph** (2 – 3 sentences) explaining what the chapter covers and why it matters.
3. **Learning Objectives** — a bulleted list of 4 – 6 concrete skills the reader will gain.
4. **Subchapters** — a numbered list linking to each subchapter.
5. **Prerequisites** — what the reader needs before starting this chapter.

Template:

```markdown
---
title: "Chapter 8: Terminal UI with Ratatui"
description: Build a rich terminal interface for the coding agent using Ratatui.
---

# Terminal UI with Ratatui

Two to three sentences introducing the chapter. Explain what capability the reader
will add to the agent and why it matters.

## Learning Objectives
- Objective one
- Objective two
- Objective three
- Objective four

## Subchapters
1. [Subchapter Title](/project/08-terminal-ui-with-ratatui/01-slug)
2. [Subchapter Title](/project/08-terminal-ui-with-ratatui/02-slug)
...

## Prerequisites
- Completion of Chapter 7, or equivalent knowledge of streaming responses
- Familiarity with terminal escape codes (helpful but not required)
```

### Step 3: Plan at least 10 subchapters

Every chapter must contain a **minimum of 10 subchapters** to provide adequate depth. The final subchapter should be a summary or summary-and-exercises page.

### Step 4: Create the `diagrams/` subfolder

```bash
mkdir learn/project/08-terminal-ui-with-ratatui/diagrams/
```

This is where Excalidraw source files and exported SVGs live.

### Step 5: Create the code snapshot

```bash
mkdir -p learn/code/ch08/src
```

Initialize a Cargo project or copy the previous chapter's code as a starting point:

```bash
cp -r learn/code/ch07/* learn/code/ch08/
```

Each chapter's code is cumulative — it builds on everything before it.

### Step 6: Cross-track references

If a project track chapter covers the same concept as a linear track chapter, add cross-references in both directions. Readers should be able to jump between tracks for the same topic.

---

## 4. How to Extend Existing Content

### Adding examples

Place new examples **after** the concept explanation and **before** the Key Takeaways section. Introduce each example with a sentence explaining what it demonstrates:

```markdown
Let's see how this works with a more realistic command that produces both stdout and
stderr output:

\```rust
// complete runnable example here
\```
```

### Adding exercises

Use an `## Exercises` section placed **before** the Key Takeaways section. Number the exercises and indicate difficulty:

```markdown
## Exercises

1. **(Easy)** Modify the shell tool to print the command's exit code after each execution.
2. **(Medium)** Add a `--dry-run` flag that prints the command without executing it.
3. **(Hard)** Implement a command history that stores the last 10 commands and their outputs.
```

### Adding deep dives

Use VitePress `:::details` containers for optional deep-dive content that would interrupt the main flow:

```markdown
::: details How does Tokio schedule async processes under the hood?
Tokio uses an epoll/kqueue-based reactor to monitor file descriptors returned by
the OS when you spawn a child process. When the child writes to its stdout pipe,
the reactor wakes the task that is awaiting the output...
:::
```

### Maintaining consistency

Before extending a subchapter, read the **two subchapters before it** and the **two after it**. Match their:

- **Tone** — if surrounding content is casual, do not switch to academic prose.
- **Depth** — if nearby subchapters explain concepts in 2 – 3 paragraphs, do not write 8.
- **Structure** — if the chapter uses a pattern of "explain, then code, then callout", follow it.

---

## 5. Diagrams and Visuals (Excalidraw)

### File organization

Store Excalidraw source files (`.excalidraw` JSON) in each chapter's `diagrams/` folder:

```
learn/project/06-shell-execution/diagrams/process-lifecycle.excalidraw
learn/project/06-shell-execution/diagrams/process-lifecycle.svg
```

### Embedding in markdown

Export each diagram as SVG and embed it with an alt-text description:

```markdown
![Process lifecycle: spawn, execute, wait, collect output](./diagrams/process-lifecycle.svg)
```

### Catppuccin Mocha color palette

All diagrams must use the Catppuccin Mocha palette to match the site theme:

| Role       | Color Name | Hex       | Usage                                |
|------------|------------|-----------|--------------------------------------|
| Background | Base       | `#1e1e2e` | Diagram canvas                       |
| Text       | Text       | `#cdd6f4` | All labels and annotations           |
| Primary    | Blue       | `#89b4fa` | Primary elements, arrows, highlights |
| Secondary  | Green      | `#a6e3a1` | Success states, positive flows       |
| Accent     | Yellow     | `#f9e2af` | Warnings, callouts, emphasis         |
| Border     | Surface1   | `#45475a` | Box borders, dividers                |

### Excalidraw template

Save the following as `learn/diagrams/template.excalidraw` and use it as a starting point for every new diagram. It pre-configures the Catppuccin palette:

```json
{
  "type": "excalidraw",
  "version": 2,
  "source": "cli-coding-agent-guide",
  "elements": [],
  "appState": {
    "gridSize": null,
    "viewBackgroundColor": "#1e1e2e",
    "currentItemStrokeColor": "#cdd6f4",
    "currentItemBackgroundColor": "#89b4fa",
    "currentItemFillStyle": "solid",
    "currentItemStrokeWidth": 2,
    "currentItemFontFamily": 3,
    "currentItemFontSize": 16,
    "currentItemTextAlign": "center"
  },
  "files": {}
}
```

### Frequency

Aim for **1 – 2 diagrams per chapter**. Good candidates:

- Architecture diagrams (component relationships)
- Flow charts (request lifecycle, agentic loop iteration)
- State machines (connection states, permission states)
- Sequence diagrams (LLM call, tool execution, response streaming)

---

## 6. Code Project Continuity

### Directory structure

The running codebase lives in chapter-level snapshots:

```
learn/code/
  ch01/          # Cargo.toml + src/main.rs — basic REPL
  ch02/          # Adds HTTP client, API call
  ch03/          # Adds agentic loop
  ...
  ch15/          # Full production agent
```

Each directory is a **self-contained, compilable Rust project**. You can verify any chapter with:

```bash
cd learn/code/ch06
cargo check
```

### Cumulative code

Every chapter builds on the previous one. `ch06/` contains everything from `ch01/` through `ch05/` plus the new code introduced in chapter 6. When you write content for chapter 6, you are describing changes applied to the `ch06/` codebase.

### Referencing code from content

Subchapters reference specific files and locations in the code project. Be explicit:

```markdown
Open `learn/code/ch06/src/tools/shell.rs`. You'll add the timeout logic to the
`execute` method starting at the point where we spawn the child process:
```

Then show the complete function, not a diff. Readers should be able to copy-paste the code and have it compile.

### Keeping code in sync

When you write or edit a subchapter that introduces new code:

1. Apply the same change to the chapter's code snapshot.
2. Run `cargo check` (at minimum) to verify it compiles.
3. If the chapter has tests, run `cargo test` as well.

Never publish content that references code which does not exist in the corresponding snapshot.

---

## 7. Research Integration

### Source material

Research notes live in two directories:

```
research/agents/       # Analysis of production coding agents
  claude-code.md       #   Anthropic's CLI coding agent
  opencode.md          #   Open-source Go-based coding agent
  pi-coding-agent.md   #   Pi's coding agent implementation
  codex.md             #   OpenAI's Codex CLI agent

research/concepts/     # Deep-dive concept analysis
  agentic-loop.md      #   The core loop pattern
  tool-systems.md      #   Tool registration, dispatch, execution
  context-management.md #  Token counting, compaction, sessions
  streaming.md         #   SSE, chunked transfer, incremental rendering
```

### Using "In the wild" callouts

Pull specific patterns and observations from the research notes into learning content. Always attribute the observation to a specific agent:

```markdown
::: wild In the Wild
Claude Code approaches tool dispatch with a static registry — every tool is known
at compile time and dispatched through a match statement. OpenCode's solution is
more dynamic: tools register themselves at startup, and the dispatcher looks them
up by name in a HashMap. We'll start with the static approach for simplicity and
refactor to dynamic dispatch in Chapter 14.
:::
```

Good patterns to surface:

- How different agents solve the same problem differently.
- Trade-offs between approaches (performance vs. flexibility, safety vs. convenience).
- Real-world constraints that shaped design decisions.

### Attribution style

Use conversational attribution:

- "Claude Code approaches this by..."
- "OpenCode's solution is..."
- "The Pi agent takes a different route, opting for..."
- "Codex solves this with..."

Do not use footnotes or academic citation style. The callout box provides sufficient context.

---

## 8. Quality Checklist

Before considering any content complete, verify every item:

- [ ] **Code compiles** — `cargo check` passes for the chapter's code project in `learn/code/chNN/`
- [ ] **No placeholder content** — no remaining `<!-- TODO -->` markers anywhere in the file
- [ ] **Proper frontmatter** — both `title` and `description` fields present and descriptive
- [ ] **Sidebar correct** — file naming follows `NN-slug-name.md` format (sidebar is auto-generated)
- [ ] **Cross-references valid** — all internal links point to existing files and use correct paths
- [ ] **Diagrams exported** — every `.excalidraw` file has a corresponding `.svg` in the same folder
- [ ] **"What you'll learn" box** — blockquote with 2 – 3 bullets immediately after the `# Heading`
- [ ] **"Key Takeaways" section** — `## Key Takeaways` with 3 – 5 bullets at the end of the file
- [ ] **At least one code example** — complete and runnable, with language specified on the code fence
- [ ] **"Coming from Python" callout** — present in every subchapter where Rust meaningfully diverges from Python patterns
- [ ] **Consistent tone and depth** — matches the two subchapters before and after in the same chapter

### Quick verification commands

```bash
# Check for remaining TODO markers across all content
grep -r "<!-- TODO" learn/project/ learn/linear/

# Verify all code projects compile
for dir in learn/code/ch*/; do
  echo "Checking $dir..."
  (cd "$dir" && cargo check 2>&1) || echo "FAILED: $dir"
done

# List subchapters missing frontmatter title
grep -rL "^title:" learn/project/**/*.md learn/linear/**/*.md
```
