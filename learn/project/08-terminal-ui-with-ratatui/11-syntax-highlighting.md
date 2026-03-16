---
title: Syntax Highlighting
description: Add syntax highlighting to code blocks in the terminal using syntect for language-aware colorization.
---

# Syntax Highlighting

> **What you'll learn:**
> - How to integrate syntect for syntax highlighting with Ratatui's styled text system
> - How to detect programming languages from code fence annotations and file extensions
> - How to map syntax highlighting themes to terminal color palettes for consistent appearance

Code blocks are the most important part of an LLM coding agent's output. When the agent suggests a fix or generates code, syntax highlighting makes the difference between a wall of white text and readable, color-coded source code. In this subchapter, you will integrate the `syntect` crate to add real syntax highlighting to your markdown renderer's code blocks.

## Setting Up syntect

The `syntect` crate bundles Sublime Text's syntax definitions and themes. It can parse source code in hundreds of languages and produce styled output:

```toml
[dependencies]
syntect = "5"
```

syntect has two main components:

1. **SyntaxSet** -- a collection of syntax definitions (one per language)
2. **ThemeSet** -- a collection of color themes (like Mocha, Dracula, One Dark)

Loading these at startup gives you everything you need:

```rust
use syntect::highlighting::ThemeSet;
use syntect::parsing::SyntaxSet;

pub struct Highlighter {
    syntax_set: SyntaxSet,
    theme_set: ThemeSet,
    theme_name: String,
}

impl Highlighter {
    pub fn new() -> Self {
        Self {
            syntax_set: SyntaxSet::load_defaults_newlines(),
            theme_set: ThemeSet::load_defaults(),
            theme_name: String::from("base16-ocean.dark"),
        }
    }
}
```

The `load_defaults_newlines()` method loads bundled syntax definitions that preserve newline characters -- important for line-by-line rendering.

## Highlighting Code for Ratatui

The core task is converting syntect's highlighted output into Ratatui `Span`s. syntect produces a `Vec<(Style, &str)>` for each line, where `Style` contains foreground and background colors:

```rust
use syntect::easy::HighlightLines;
use syntect::highlighting::Style as SyntectStyle;
use syntect::util::LinesWithEndings;
use ratatui::prelude::*;

impl Highlighter {
    /// Highlight source code and return Ratatui Lines.
    pub fn highlight(&self, code: &str, language: &str) -> Vec<Line<'static>> {
        // Find the syntax definition for the language
        let syntax = self.syntax_set
            .find_syntax_by_token(language)
            .unwrap_or_else(|| self.syntax_set.find_syntax_plain_text());

        let theme = &self.theme_set.themes[&self.theme_name];
        let mut highlighter = HighlightLines::new(syntax, theme);

        let mut lines = Vec::new();

        for line in LinesWithEndings::from(code) {
            // syntect returns styled segments for each line
            let segments = highlighter
                .highlight_line(line, &self.syntax_set)
                .unwrap_or_default();

            let spans: Vec<Span<'static>> = segments
                .iter()
                .map(|(style, text)| {
                    Span::styled(
                        text.to_string(),
                        syntect_to_ratatui_style(style),
                    )
                })
                .collect();

            lines.push(Line::from(spans));
        }

        lines
    }
}

/// Convert a syntect Style to a Ratatui Style.
fn syntect_to_ratatui_style(style: &SyntectStyle) -> Style {
    let fg = Color::Rgb(
        style.foreground.r,
        style.foreground.g,
        style.foreground.b,
    );

    let bg = Color::Rgb(
        style.background.r,
        style.background.g,
        style.background.b,
    );

    Style::default().fg(fg).bg(bg)
}
```

The conversion is direct: syntect uses RGBA colors, and Ratatui supports RGB colors via `Color::Rgb`. The alpha channel is ignored since terminals do not support transparency.

## Language Detection

The markdown code fence tells you what language to highlight. Your markdown renderer already extracts this from the `` ```rust `` annotation. But sometimes the language is missing or uses a non-standard name. You need a fallback strategy:

```rust
impl Highlighter {
    /// Find the best syntax definition for a language hint.
    pub fn find_syntax(&self, language: &str) -> &syntect::parsing::SyntaxReference {
        // First, try the language token directly (e.g., "rust", "python", "js")
        if let Some(syntax) = self.syntax_set.find_syntax_by_token(language) {
            return syntax;
        }

        // Try common aliases
        let normalized = match language.to_lowercase().as_str() {
            "js" | "jsx" => "javascript",
            "ts" | "tsx" => "typescript",
            "py" => "python",
            "rb" => "ruby",
            "sh" | "bash" | "zsh" => "bash",
            "yml" => "yaml",
            "md" => "markdown",
            "rs" => "rust",
            other => other,
        };

        self.syntax_set
            .find_syntax_by_token(normalized)
            .unwrap_or_else(|| self.syntax_set.find_syntax_plain_text())
    }
}
```

::: python Coming from Python
Python's `pygments` library serves the same role as `syntect`:
```python
from pygments import highlight
from pygments.lexers import get_lexer_by_name
from pygments.formatters import TerminalTrueColorFormatter

code = 'fn main() { println!("hello"); }'
lexer = get_lexer_by_name("rust")
result = highlight(code, lexer, TerminalTrueColorFormatter())
print(result)
```
The main difference is that `pygments` outputs ANSI escape codes directly to the terminal, while `syntect` gives you structured style data that you convert to Ratatui's `Style` type. This structured approach integrates better with the immediate-mode rendering model -- you get `Span`s you can place in layouts rather than raw terminal output.
:::

## Integrating with the Markdown Renderer

Now let's update the `MarkdownRenderer` to use syntax highlighting for code blocks. The change is in the `End(TagEnd::CodeBlock)` handler:

```rust
use std::sync::Arc;

pub struct MarkdownRenderer {
    // ... previous fields ...
    highlighter: Arc<Highlighter>,
}

impl MarkdownRenderer {
    pub fn new(highlighter: Arc<Highlighter>) -> Self {
        Self {
            lines: Vec::new(),
            current_spans: Vec::new(),
            style_stack: vec![Style::default()],
            in_code_block: false,
            code_language: None,
            code_content: String::new(),
            list_depth: 0,
            highlighter,
        }
    }

    fn process_event(&mut self, event: Event) {
        match event {
            // ... other handlers unchanged ...

            Event::End(TagEnd::CodeBlock) => {
                // Use syntax highlighting if a language was specified
                let highlighted_lines = if let Some(ref lang) = self.code_language {
                    self.highlighter.highlight(&self.code_content, lang)
                } else {
                    // No language: render as plain monospace text
                    let plain_style = Style::default()
                        .fg(Color::White)
                        .bg(Color::Rgb(30, 30, 46));
                    self.code_content
                        .lines()
                        .map(|line| Line::from(Span::styled(
                            line.to_string(),
                            plain_style,
                        )))
                        .collect()
                };

                // Add indentation and code block background
                for line in highlighted_lines {
                    let mut indented_spans = vec![Span::raw("  ")];
                    indented_spans.extend(line.spans);
                    self.lines.push(Line::from(indented_spans));
                }

                self.in_code_block = false;
                self.code_language = None;
                self.code_content.clear();
                self.lines.push(Line::from(""));
            }

            // ... rest unchanged ...
            _ => {}
        }
    }
}
```

## Performance: Caching the Highlighter

Creating `SyntaxSet` and `ThemeSet` is expensive -- they parse bundled binary data. Create them once at application startup and share them:

```rust
use std::sync::Arc;

pub struct App {
    pub highlighter: Arc<Highlighter>,
    // ... other fields
}

impl App {
    pub fn new() -> Self {
        let highlighter = Arc::new(Highlighter::new());
        Self {
            highlighter,
            // ...
        }
    }
}
```

The `Arc<Highlighter>` lets you share the highlighter between the markdown renderer and any other component that needs syntax highlighting (like a file preview sidebar).

## Line Numbers for Code Blocks

Adding line numbers makes code blocks more useful, especially when the agent references specific lines:

```rust
impl Highlighter {
    /// Highlight with line numbers.
    pub fn highlight_with_line_numbers(
        &self,
        code: &str,
        language: &str,
    ) -> Vec<Line<'static>> {
        let syntax = self.find_syntax(language);
        let theme = &self.theme_set.themes[&self.theme_name];
        let mut hl = HighlightLines::new(syntax, theme);

        let total_lines = code.lines().count();
        let gutter_width = total_lines.to_string().len();

        let mut lines = Vec::new();

        for (i, line) in LinesWithEndings::from(code).enumerate() {
            let line_num = format!("{:>width$} ", i + 1, width = gutter_width);

            // Gutter style: dim color
            let mut spans = vec![Span::styled(
                line_num,
                Style::default().fg(Color::DarkGray),
            )];

            // Highlighted code
            let segments = hl
                .highlight_line(line, &self.syntax_set)
                .unwrap_or_default();

            for (style, text) in segments {
                spans.push(Span::styled(
                    text.to_string(),
                    syntect_to_ratatui_style(&style),
                ));
            }

            lines.push(Line::from(spans));
        }

        lines
    }
}
```

## Handling Streaming Code Blocks

During streaming, code blocks arrive incrementally. The agent might send:

1. `` ```rust ``
2. `fn main() {`
3. `    println!("hello");`
4. `}`
5. `` ``` ``

Your markdown renderer re-renders the full accumulated content each time a token arrives. syntect handles partial code gracefully -- it highlights whatever it has so far. A half-complete function will still have keywords colored and strings highlighted, even if the block is not yet closed.

```rust
impl CachedMessage {
    /// Called when a new streaming token arrives.
    pub fn append_token(&mut self, token: &str, highlighter: &Arc<Highlighter>) {
        self.source.push_str(token);
        // Re-render the full content including partial code blocks
        let renderer = MarkdownRenderer::new(Arc::clone(highlighter));
        self.rendered = renderer.render(&self.source);
    }
}
```

::: wild In the Wild
Claude Code renders syntax-highlighted code blocks in real time as tokens stream in. The highlighting updates incrementally, so users see colored code appearing token by token. OpenCode similarly highlights code blocks using its Go-based syntax highlighting libraries. Both agents support dozens of programming languages, matching the language from the markdown fence annotation. The experience of watching highlighted code appear in real time is one of the features that makes a coding agent feel premium.
:::

## Key Takeaways

- **syntect** provides Sublime Text-quality syntax highlighting for hundreds of languages, with its `SyntaxSet` and `ThemeSet` loaded once at startup and shared via `Arc`.
- **The conversion from syntect to Ratatui** is straightforward: each highlighted segment's RGBA color maps directly to `Color::Rgb` in a `Span`'s `Style`.
- **Language detection** uses the code fence annotation with fallback aliases (e.g., "js" to "javascript", "py" to "python") and falls back to plain text when no language is specified.
- **Line numbers** enhance code block readability by prepending a styled gutter to each highlighted line.
- **Streaming code blocks** highlight correctly even when incomplete -- syntect processes whatever source text is available, and re-rendering on each new token produces progressively better output.
