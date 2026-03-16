---
title: Markdown Rendering
description: Parse and render markdown content in the terminal with support for headings, lists, code blocks, bold, italic, and links.
---

# Markdown Rendering

> **What you'll learn:**
> - How to parse markdown into an AST using pulldown-cmark
> - How to convert markdown AST nodes into Ratatui styled text spans
> - How to handle block-level elements like headings, lists, and code fences in terminal rendering

LLM responses are markdown. They contain headings, bold text, code blocks, lists, and inline code. If you render them as plain text, you lose all the visual structure that makes agent output readable. In this subchapter, you will build a markdown-to-Ratatui converter that transforms markdown text into styled terminal output.

## Parsing Markdown with pulldown-cmark

The `pulldown-cmark` crate is the standard markdown parser in the Rust ecosystem. It produces a stream of events as it walks through the markdown AST, similar to a SAX XML parser:

```toml
[dependencies]
pulldown-cmark = "0.12"
```

```rust
use pulldown_cmark::{Event, Parser, Tag, TagEnd};

fn parse_demo() {
    let markdown = "# Hello\n\nThis is **bold** and *italic* text.\n\n- Item one\n- Item two\n";

    let parser = Parser::new(markdown);

    for event in parser {
        println!("{:?}", event);
    }
}
```

This produces events like:

```text
Start(Heading { level: H1, .. })
Text("Hello")
End(Heading(H1))
Start(Paragraph)
Text("This is ")
Start(Emphasis)
Text("bold")
End(Emphasis)
Text(" and ")
Start(Emphasis)
Text("italic")
End(Emphasis)
Text(" text.")
End(Paragraph)
Start(List(..))
Start(Item)
Text("Item one")
End(Item)
Start(Item)
Text("Item two")
End(Item)
End(List(..))
```

The key insight: pulldown-cmark gives you a flat stream of Start/End/Text events, and you need to track a style stack to know what styles are currently active.

## Building a Markdown Renderer

Let's build a struct that converts pulldown-cmark events into Ratatui `Line`s and `Span`s:

```rust
use pulldown_cmark::{Event, Parser, Tag, TagEnd, HeadingLevel, CodeBlockKind};
use ratatui::prelude::*;

pub struct MarkdownRenderer {
    /// The accumulated lines of styled output.
    lines: Vec<Line<'static>>,
    /// The spans being built for the current line.
    current_spans: Vec<Span<'static>>,
    /// Stack of active styles (bold, italic, etc.)
    style_stack: Vec<Style>,
    /// Whether we are inside a code block.
    in_code_block: bool,
    /// The language of the current code block (if any).
    code_language: Option<String>,
    /// Accumulated code block content.
    code_content: String,
    /// Current list nesting depth.
    list_depth: usize,
}

impl MarkdownRenderer {
    pub fn new() -> Self {
        Self {
            lines: Vec::new(),
            current_spans: Vec::new(),
            style_stack: vec![Style::default()],
            in_code_block: false,
            code_language: None,
            code_content: String::new(),
            list_depth: 0,
        }
    }

    /// Render markdown text into styled Ratatui lines.
    pub fn render(mut self, markdown: &str) -> Vec<Line<'static>> {
        let parser = Parser::new(markdown);

        for event in parser {
            self.process_event(event);
        }

        // Flush any remaining spans
        self.flush_line();
        self.lines
    }

    fn current_style(&self) -> Style {
        *self.style_stack.last().unwrap_or(&Style::default())
    }

    fn push_style(&mut self, modifier: Modifier) {
        let new_style = self.current_style().add_modifier(modifier);
        self.style_stack.push(new_style);
    }

    fn pop_style(&mut self) {
        if self.style_stack.len() > 1 {
            self.style_stack.pop();
        }
    }

    fn flush_line(&mut self) {
        if !self.current_spans.is_empty() {
            let spans = std::mem::take(&mut self.current_spans);
            self.lines.push(Line::from(spans));
        }
    }

    fn process_event(&mut self, event: Event) {
        match event {
            // Block-level elements
            Event::Start(Tag::Heading { level, .. }) => {
                self.flush_line();
                let style = match level {
                    HeadingLevel::H1 => Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::BOLD | Modifier::UNDERLINED),
                    HeadingLevel::H2 => Style::default()
                        .fg(Color::Blue)
                        .add_modifier(Modifier::BOLD),
                    HeadingLevel::H3 => Style::default()
                        .fg(Color::Magenta)
                        .add_modifier(Modifier::BOLD),
                    _ => Style::default().add_modifier(Modifier::BOLD),
                };
                self.style_stack.push(style);
            }
            Event::End(TagEnd::Heading(_)) => {
                self.pop_style();
                self.flush_line();
                self.lines.push(Line::from("")); // blank line after headings
            }

            Event::Start(Tag::Paragraph) => {
                self.flush_line();
            }
            Event::End(TagEnd::Paragraph) => {
                self.flush_line();
                self.lines.push(Line::from("")); // blank line between paragraphs
            }

            // Inline elements
            Event::Start(Tag::Strong) => {
                self.push_style(Modifier::BOLD);
            }
            Event::End(TagEnd::Strong) => {
                self.pop_style();
            }

            Event::Start(Tag::Emphasis) => {
                self.push_style(Modifier::ITALIC);
            }
            Event::End(TagEnd::Emphasis) => {
                self.pop_style();
            }

            Event::Start(Tag::Strikethrough) => {
                self.push_style(Modifier::CROSSED_OUT);
            }
            Event::End(TagEnd::Strikethrough) => {
                self.pop_style();
            }

            Event::Code(code) => {
                // Inline code: render with a distinct background
                self.current_spans.push(Span::styled(
                    format!(" {} ", code),
                    Style::default()
                        .fg(Color::Yellow)
                        .bg(Color::Rgb(49, 50, 68)), // Catppuccin Surface0
                ));
            }

            // Code blocks
            Event::Start(Tag::CodeBlock(kind)) => {
                self.flush_line();
                self.in_code_block = true;
                self.code_language = match kind {
                    CodeBlockKind::Fenced(lang) => {
                        let lang = lang.to_string();
                        if lang.is_empty() { None } else { Some(lang) }
                    }
                    CodeBlockKind::Indented => None,
                };
                self.code_content.clear();

                // Code block header
                let lang_label = self.code_language
                    .as_deref()
                    .unwrap_or("text");
                self.lines.push(Line::from(Span::styled(
                    format!(" {} ", lang_label),
                    Style::default()
                        .fg(Color::Black)
                        .bg(Color::DarkGray),
                )));
            }
            Event::End(TagEnd::CodeBlock) => {
                // Render accumulated code content
                let code_style = Style::default()
                    .fg(Color::White)
                    .bg(Color::Rgb(30, 30, 46)); // Catppuccin Base

                for line in self.code_content.lines() {
                    self.lines.push(Line::from(Span::styled(
                        format!("  {}", line),
                        code_style,
                    )));
                }

                self.in_code_block = false;
                self.code_language = None;
                self.code_content.clear();
                self.lines.push(Line::from("")); // blank line after code block
            }

            // Lists
            Event::Start(Tag::List(_)) => {
                self.flush_line();
                self.list_depth += 1;
            }
            Event::End(TagEnd::List(_)) => {
                self.list_depth = self.list_depth.saturating_sub(1);
                if self.list_depth == 0 {
                    self.lines.push(Line::from("")); // blank line after top-level list
                }
            }
            Event::Start(Tag::Item) => {
                self.flush_line();
                let indent = "  ".repeat(self.list_depth.saturating_sub(1));
                let bullet = if self.list_depth <= 1 { "  * " } else { "  - " };
                self.current_spans.push(Span::styled(
                    format!("{}{}", indent, bullet),
                    Style::default().fg(Color::DarkGray),
                ));
            }
            Event::End(TagEnd::Item) => {
                self.flush_line();
            }

            // Links
            Event::Start(Tag::Link { dest_url, .. }) => {
                self.style_stack.push(
                    Style::default()
                        .fg(Color::Blue)
                        .add_modifier(Modifier::UNDERLINED),
                );
            }
            Event::End(TagEnd::Link) => {
                self.pop_style();
            }

            // Text content
            Event::Text(text) => {
                if self.in_code_block {
                    self.code_content.push_str(&text);
                } else {
                    self.current_spans.push(Span::styled(
                        text.to_string(),
                        self.current_style(),
                    ));
                }
            }

            Event::SoftBreak => {
                if !self.in_code_block {
                    self.current_spans.push(Span::raw(" "));
                }
            }
            Event::HardBreak => {
                self.flush_line();
            }

            // Horizontal rule
            Event::Rule => {
                self.flush_line();
                self.lines.push(Line::from(Span::styled(
                    "─".repeat(40),
                    Style::default().fg(Color::DarkGray),
                )));
                self.lines.push(Line::from(""));
            }

            _ => {} // Ignore other events for now
        }
    }
}
```

## Using the Renderer

Integrating the markdown renderer into your conversation view is straightforward:

```rust
use ratatui::{prelude::*, widgets::{Block, Borders, Paragraph, Wrap}};

fn render_conversation(frame: &mut Frame, app: &App, area: Rect) {
    let block = Block::default()
        .title(" Conversation ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan));

    let mut all_lines: Vec<Line> = Vec::new();

    for msg in &app.messages {
        // Role header
        let (label, color) = match msg.role {
            Role::User => ("You", Color::Blue),
            Role::Assistant => ("Agent", Color::Green),
        };
        all_lines.push(Line::from(Span::styled(
            format!("{}", label),
            Style::default().fg(color).add_modifier(Modifier::BOLD),
        )));
        all_lines.push(Line::from(""));

        // Render the message content as markdown
        let renderer = MarkdownRenderer::new();
        let rendered_lines = renderer.render(&msg.content);
        all_lines.extend(rendered_lines);

        // Separator between messages
        all_lines.push(Line::from(Span::styled(
            "─".repeat(40),
            Style::default().fg(Color::Rgb(69, 71, 90)), // Catppuccin Surface1
        )));
        all_lines.push(Line::from(""));
    }

    let paragraph = Paragraph::new(all_lines)
        .block(block)
        .wrap(Wrap { trim: false })
        .scroll((app.scroll_offset, 0));

    frame.render_widget(paragraph, area);
}
```

::: tip Coming from Python
Python's `rich` library has built-in markdown rendering:
```python
from rich.markdown import Markdown
from rich.console import Console

console = Console()
md = Markdown("# Hello\n\nThis is **bold** text.")
console.print(md)
```
In Rust, you build this yourself by combining `pulldown-cmark` with Ratatui's styled text system. The upside is full control over styling. The downside is more code. The `MarkdownRenderer` above covers the same use cases as `rich.Markdown` but integrates directly with your TUI layout rather than printing to stdout.
:::

## Caching Rendered Output

Parsing and rendering markdown every frame is wasteful if the content has not changed. Cache the rendered lines and only re-render when the content updates:

```rust
pub struct CachedMessage {
    /// The raw markdown source.
    pub source: String,
    /// The rendered lines (computed once, reused each frame).
    pub rendered: Vec<Line<'static>>,
}

impl CachedMessage {
    pub fn new(source: String) -> Self {
        let renderer = MarkdownRenderer::new();
        let rendered = renderer.render(&source);
        Self { source, rendered }
    }

    /// Update the source and re-render. Called when streaming appends new content.
    pub fn update_source(&mut self, new_source: String) {
        self.source = new_source;
        let renderer = MarkdownRenderer::new();
        self.rendered = renderer.render(&self.source);
    }
}
```

During streaming, you call `update_source()` each time new tokens arrive. Between streaming updates, the cached `rendered` lines are reused without re-parsing.

::: tip In the Wild
Claude Code renders markdown in assistant responses with full syntax highlighting and styled headings, lists, and inline code. It uses a streaming-aware markdown renderer that can handle partial markdown -- for example, rendering a heading even when only `# Hel` has arrived so far, then re-rendering as more tokens come in. Your cached rendering approach accomplishes the same thing by re-rendering the full content each time the source changes during streaming.
:::

## Key Takeaways

- **pulldown-cmark** parses markdown into a stream of events (Start, End, Text, Code) that you walk through to build styled terminal output.
- **A style stack** tracks active formatting (bold, italic, heading style) as you enter and exit nested markdown elements, applying the correct style to each text span.
- **Block-level elements** (headings, paragraphs, code blocks, lists) map to groups of `Line`s with appropriate styling and spacing.
- **Inline elements** (bold, italic, inline code, links) map to `Span`s within a `Line`, each with their own style applied from the style stack.
- **Caching rendered output** avoids re-parsing unchanged markdown every frame, with selective re-rendering during streaming when content updates.
