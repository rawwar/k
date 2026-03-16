---
title: Incremental Rendering
description: Rendering streamed content progressively to the user without flickering, with techniques for markdown parsing, code block detection, and smooth text display.
---

# Incremental Rendering

> **What you'll learn:**
> - How to render streamed text incrementally while handling markdown formatting that spans multiple chunks
> - Detecting and buffering code blocks, tables, and other structured elements that should not be partially displayed
> - Techniques for smooth character-by-character and word-by-word rendering that feels natural to users

You have SSE events flowing through your parser and text deltas arriving one by one. Now comes the part the user actually sees: rendering that text to the terminal. Incremental rendering sounds simple -- just print each token as it arrives -- but the moment you try it with real LLM output, you hit problems. Markdown formatting spans multiple tokens. Code blocks open but do not close for dozens of tokens. ANSI escape sequences get cut in half. This subchapter covers the techniques for rendering streamed content that looks good *while* it is streaming, not just when it is done.

## The Naive Approach and Its Problems

The simplest possible renderer just prints each text delta:

```rust
use std::io::{self, Write};

fn render_token(token: &str) {
    print!("{}", token);
    io::stdout().flush().unwrap();
}
```

This works for plain text, and it is how many early LLM interfaces operated. But LLM output is rarely plain text -- it contains markdown headers, bold text, code blocks, lists, and links. Consider what happens when the LLM generates `**important**`:

| Delta | Screen Shows | Problem? |
|-------|-------------|----------|
| `**` | `**` | User sees raw asterisks |
| `import` | `**import` | Still looks like broken markdown |
| `ant` | `**important` | Still raw asterisks |
| `**` | `**important**` | Now it could be rendered as bold, but the screen already showed `**` |

If you are rendering to a TUI that interprets markdown, you need to decide: do you show raw asterisks and then re-render the line as bold when you see the closing `**`? Or do you buffer until you know whether the asterisks are markdown formatting? Each approach has trade-offs.

## Rendering Strategies

There are three main approaches, each used by different production agents:

### Strategy 1: Raw Text with Post-Processing

Print tokens as plain text, then re-render the complete response with markdown formatting after the stream ends. This is the simplest approach and avoids all mid-stream formatting issues:

```rust
pub struct RawRenderer {
    buffer: String,
}

impl RawRenderer {
    pub fn new() -> Self {
        Self {
            buffer: String::new(),
        }
    }

    pub fn append(&mut self, token: &str) {
        print!("{}", token);
        std::io::stdout().flush().unwrap();
        self.buffer.push_str(token);
    }

    pub fn finalize(&self) -> &str {
        // After streaming completes, the full buffer could be
        // re-rendered with markdown formatting
        &self.buffer
    }
}
```

The downside: code blocks look like plain text during streaming, headers lack formatting, and bold/italic is invisible. The upside: zero flickering, zero re-rendering, and the simplest possible implementation.

### Strategy 2: Line-Based Buffering

Buffer tokens until you have a complete line (terminated by `\n`), then render the whole line with formatting. This catches most markdown constructs because they operate at the line level -- headers, list items, and horizontal rules are all line-delimited:

```rust
pub struct LineBufferedRenderer {
    line_buffer: String,
    in_code_block: bool,
}

impl LineBufferedRenderer {
    pub fn new() -> Self {
        Self {
            line_buffer: String::new(),
            in_code_block: false,
        }
    }

    pub fn append(&mut self, token: &str) {
        self.line_buffer.push_str(token);

        // Process complete lines
        while let Some(newline_pos) = self.line_buffer.find('\n') {
            let line = self.line_buffer[..newline_pos].to_string();
            self.line_buffer = self.line_buffer[newline_pos + 1..].to_string();
            self.render_line(&line);
        }
    }

    fn render_line(&mut self, line: &str) {
        // Track code block boundaries
        if line.starts_with("```") {
            self.in_code_block = !self.in_code_block;
        }

        if self.in_code_block {
            // Inside a code block: render without markdown processing
            println!("{}", line);
        } else if line.starts_with("# ") {
            // Render as header (could apply ANSI bold/color)
            println!("\x1b[1m{}\x1b[0m", line);
        } else if line.starts_with("- ") || line.starts_with("* ") {
            // Render as list item
            println!("  {}", line);
        } else {
            println!("{}", line);
        }
    }

    /// Flush any remaining partial line
    pub fn flush(&mut self) {
        if !self.line_buffer.is_empty() {
            let remaining = std::mem::take(&mut self.line_buffer);
            self.render_line(&remaining);
        }
    }
}
```

Line buffering introduces a small latency -- you do not see characters until the LLM generates a newline -- but it handles most markdown correctly and avoids the flickering of re-rendering mid-line.

### Strategy 3: Dual-Mode Rendering

The most sophisticated approach: render tokens immediately in a "streaming mode" with minimal formatting, but maintain a shadow buffer that tracks the complete response. When the stream ends (or at periodic intervals), re-render the visible portion with full markdown formatting.

```rust
pub struct DualModeRenderer {
    /// Complete response accumulated so far
    full_buffer: String,
    /// Number of characters already rendered with full formatting
    formatted_up_to: usize,
    /// Current line being accumulated
    current_line: String,
}

impl DualModeRenderer {
    pub fn new() -> Self {
        Self {
            full_buffer: String::new(),
            formatted_up_to: 0,
            current_line: String::new(),
        }
    }

    pub fn append(&mut self, token: &str) {
        self.full_buffer.push_str(token);
        self.current_line.push_str(token);

        // Stream tokens to screen immediately (plain text)
        print!("{}", token);
        std::io::stdout().flush().unwrap();

        // When we hit a paragraph break, we could re-render the
        // completed paragraph with full formatting
        if self.current_line.contains("\n\n") {
            self.reformat_completed_section();
            self.current_line.clear();
        }
    }

    fn reformat_completed_section(&mut self) {
        // Move cursor up, clear the raw text, and reprint with formatting.
        // This is TUI-specific and requires knowing how many lines to clear.
        // In a full TUI (like ratatui), you would just re-render the widget.
        self.formatted_up_to = self.full_buffer.len();
    }
}
```

::: python Coming from Python
Python's `rich` library provides a `Live` context manager that handles incremental rendering elegantly:
```python
from rich.live import Live
from rich.markdown import Markdown

buffer = ""
with Live(refresh_per_second=10) as live:
    for token in stream:
        buffer += token
        live.update(Markdown(buffer))
```
`rich` re-renders the entire markdown on each update, relying on terminal differential updates to minimize flickering. Rust TUI libraries like `ratatui` take a similar approach -- you re-render the entire widget tree on each frame, and the library computes the minimal terminal updates. The difference is that Rust gives you explicit control over the render cycle, so you can choose when to trigger re-renders rather than relying on a timer.
:::

## Handling Code Blocks

Code blocks deserve special attention because they are the most common structured element in coding agent output, and they interact poorly with naive rendering. A code block starts with ` ``` ` and ends with ` ``` `, but between those markers, every character should be rendered as-is, with no markdown interpretation.

The challenge: when you see ` ``` `, you do not know if it is the start or end of a code block until you see the matching one. And the closing ` ``` ` might be 500 tokens away. Your renderer must track this state:

```rust
#[derive(Debug, PartialEq)]
enum RenderContext {
    Normal,
    CodeBlock { language: Option<String> },
}

pub struct CodeAwareRenderer {
    context: RenderContext,
    line_buffer: String,
}

impl CodeAwareRenderer {
    pub fn new() -> Self {
        Self {
            context: RenderContext::Normal,
            line_buffer: String::new(),
        }
    }

    pub fn append(&mut self, token: &str) {
        self.line_buffer.push_str(token);

        while let Some(newline_pos) = self.line_buffer.find('\n') {
            let line = self.line_buffer[..newline_pos].to_string();
            self.line_buffer = self.line_buffer[newline_pos + 1..].to_string();
            self.process_line(&line);
        }
    }

    fn process_line(&mut self, line: &str) {
        let trimmed = line.trim();

        match &self.context {
            RenderContext::Normal => {
                if trimmed.starts_with("```") {
                    let language = trimmed.strip_prefix("```").map(|s| s.to_string());
                    self.context = RenderContext::CodeBlock {
                        language: if language.as_deref() == Some("") {
                            None
                        } else {
                            language
                        },
                    };
                    // Print code block header with language hint
                    if let RenderContext::CodeBlock { language: Some(ref lang) } = self.context {
                        println!("\x1b[90m--- {} ---\x1b[0m", lang);
                    } else {
                        println!("\x1b[90m--- code ---\x1b[0m");
                    }
                } else {
                    println!("{}", line);
                }
            }
            RenderContext::CodeBlock { .. } => {
                if trimmed == "```" {
                    self.context = RenderContext::Normal;
                    println!("\x1b[90m--- end ---\x1b[0m");
                } else {
                    // Print code lines with a gutter indicator
                    println!("\x1b[90m|\x1b[0m {}", line);
                }
            }
        }
    }

    pub fn flush(&mut self) {
        if !self.line_buffer.is_empty() {
            let remaining = std::mem::take(&mut self.line_buffer);
            // Print remaining partial line
            print!("{}", remaining);
            std::io::stdout().flush().unwrap();
        }
    }
}
```

This renderer tracks whether it is inside a code block and adjusts its formatting accordingly. Code lines get a gutter marker, and the language hint is displayed at the start of the block.

## Flush Timing and Perceived Smoothness

How often you flush to the terminal affects perceived smoothness. There are three common strategies:

**Immediate flush (per token):** Call `stdout().flush()` after every token. This gives the smoothest character-by-character appearance but can be expensive if the LLM generates very small tokens (1-2 characters each). The syscall overhead of frequent flushes can become measurable.

**Line flush:** Flush after each newline. This batches characters within a line but shows each line immediately when complete.

**Timed flush:** Flush at a fixed interval (e.g., every 16ms for ~60fps). This amortizes the syscall cost and gives consistent rendering smoothness regardless of token size:

```rust
use std::io::{self, Write};
use tokio::time::{interval, Duration};

pub struct TimedFlushRenderer {
    buffer: String,
    dirty: bool,
}

impl TimedFlushRenderer {
    pub fn new() -> Self {
        Self {
            buffer: String::new(),
            dirty: false,
        }
    }

    pub fn append(&mut self, token: &str) {
        self.buffer.push_str(token);
        self.dirty = true;
    }

    pub fn flush_if_dirty(&mut self) {
        if self.dirty {
            print!("{}", self.buffer);
            io::stdout().flush().unwrap();
            self.buffer.clear();
            self.dirty = false;
        }
    }
}

// Usage with a timed flush loop:
async fn render_loop(
    renderer: &mut TimedFlushRenderer,
    rx: &mut tokio::sync::mpsc::Receiver<String>,
) {
    let mut tick = interval(Duration::from_millis(16)); // ~60fps

    loop {
        tokio::select! {
            token = rx.recv() => {
                match token {
                    Some(t) => renderer.append(&t),
                    None => {
                        renderer.flush_if_dirty();
                        break;
                    }
                }
            }
            _ = tick.tick() => {
                renderer.flush_if_dirty();
            }
        }
    }
}
```

::: wild In the Wild
Claude Code renders tokens immediately to the terminal as they arrive, with stdout flushing after each chunk. The rendering is plain-text during streaming -- markdown formatting is not applied to in-flight text. Once the response completes, the full text is available for the conversation history, but the terminal output remains as-is. This approach prioritizes simplicity and low latency over visual polish during streaming.
:::

## Key Takeaways

- **Naive token-by-token rendering** works for plain text but produces visual artifacts with markdown formatting (visible `**`, broken code block boundaries).
- **Line-based buffering** catches most markdown constructs with a small latency trade-off and is the best balance of simplicity and correctness for CLI agents.
- **Code block state tracking** is essential for any renderer that processes LLM output, since code blocks are the most common structured element in coding agent responses.
- **Flush timing** affects perceived smoothness: per-token flush is smoothest but has syscall overhead; timed flush (every 16ms) amortizes cost and gives consistent ~60fps rendering.
- In practice, most CLI agents use **plain text streaming with post-render formatting**, prioritizing low latency over mid-stream visual polish.
