---
title: Syntax Highlighting in Terminal
description: Applying syntax highlighting to code blocks in the terminal using syntect or tree-sitter, mapping highlight spans to terminal colors and styles.
---

# Syntax Highlighting in Terminal

> **What you'll learn:**
> - How syntect and tree-sitter produce highlight spans and how to map them to Ratatui's Style and Color types
> - Efficiently highlighting code blocks that arrive incrementally during streaming without re-highlighting the entire block
> - Managing syntax theme palettes that look correct across terminals with different color capabilities (16, 256, true color)

A coding agent displays a lot of code -- in assistant responses, tool outputs, file contents, and diffs. Rendering this code with syntax highlighting dramatically improves readability. In this subchapter, you will learn how to integrate the `syntect` library with Ratatui to produce highlighted code blocks that look good across different terminal environments.

## syntect: Syntax Highlighting in Rust

`syntect` is the standard Rust library for syntax highlighting. It uses TextMate-compatible grammar files (the same grammars used by VS Code) and theme files to produce colored output. The library is mature, supports hundreds of languages, and includes a default set of themes.

```rust
use syntect::highlighting::{ThemeSet, Style as SyntectStyle};
use syntect::parsing::SyntaxSet;
use syntect::easy::HighlightLines;

fn main() {
    // Load the default syntax definitions and themes
    let syntax_set = SyntaxSet::load_defaults_newlines();
    let theme_set = ThemeSet::load_defaults();

    // Choose a theme -- "base16-ocean.dark" works well in terminals
    let theme = &theme_set.themes["base16-ocean.dark"];

    // Find the syntax for Rust
    let syntax = syntax_set
        .find_syntax_by_extension("rs")
        .expect("Rust syntax not found");

    // Create a highlighter
    let mut highlighter = HighlightLines::new(syntax, theme);

    // Highlight some code
    let code = r#"fn main() {
    let message = "Hello, world!";
    println!("{}", message);
}"#;

    for line in code.lines() {
        // highlight_line returns a Vec of (Style, &str) pairs
        let ranges = highlighter
            .highlight_line(line, &syntax_set)
            .expect("highlight failed");

        for (style, text) in &ranges {
            // style.foreground is an RGBA color
            let fg = style.foreground;
            print!(
                "\x1b[38;2;{};{};{}m{}\x1b[0m",
                fg.r, fg.g, fg.b, text
            );
        }
        println!();
    }
}
```

::: tip Coming from Python
Python's `pygments` library is the standard for syntax highlighting and serves a similar role to `syntect`. Rich uses pygments internally when you call `console.print(syntax)`. The key difference is that syntect uses TextMate grammars (the VS Code format) while pygments uses its own grammar system. If you have customized pygments styles, you will find syntect's theme system similar -- both map token types to colors.
:::

## Mapping syntect Styles to Ratatui

The bridge between syntect and Ratatui is converting syntect's `Style` (which uses RGBA colors) to Ratatui's `Style` (which uses the `Color` enum):

```rust
use ratatui::style::{Color, Modifier, Style as RatatuiStyle};
use syntect::highlighting::{
    FontStyle, Style as SyntectStyle, Color as SyntectColor,
};

/// Convert a syntect color to a Ratatui color
fn syntect_color_to_ratatui(color: SyntectColor) -> Color {
    Color::Rgb(color.r, color.g, color.b)
}

/// Convert a syntect style to a Ratatui style
fn syntect_to_ratatui_style(syntect_style: SyntectStyle) -> RatatuiStyle {
    let mut style = RatatuiStyle::default()
        .fg(syntect_color_to_ratatui(syntect_style.foreground));

    // Only set background if it is not fully transparent
    if syntect_style.background.a > 0 {
        style = style.bg(syntect_color_to_ratatui(syntect_style.background));
    }

    // Map font style flags
    if syntect_style.font_style.contains(FontStyle::BOLD) {
        style = style.add_modifier(Modifier::BOLD);
    }
    if syntect_style.font_style.contains(FontStyle::ITALIC) {
        style = style.add_modifier(Modifier::ITALIC);
    }
    if syntect_style.font_style.contains(FontStyle::UNDERLINE) {
        style = style.add_modifier(Modifier::UNDERLINED);
    }

    style
}

fn main() {
    // Example: create a syntect style and convert it
    let syntect_style = SyntectStyle {
        foreground: SyntectColor { r: 137, g: 180, b: 250, a: 255 },
        background: SyntectColor { r: 0, g: 0, b: 0, a: 0 },
        font_style: FontStyle::BOLD,
    };

    let ratatui_style = syntect_to_ratatui_style(syntect_style);
    println!("Converted style: fg=Rgb(137,180,250), bold=true");
    println!("Style: {:?}", ratatui_style);
}
```

## Building Highlighted Lines for Ratatui

Now let's combine syntect highlighting with Ratatui's `Line` and `Span` types to produce highlighted code that can be rendered as a `Paragraph`:

```rust
use ratatui::{
    style::{Color, Modifier, Style as RatatuiStyle},
    text::{Line, Span, Text},
    widgets::{Block, Borders, Paragraph, Wrap},
};
use syntect::highlighting::{
    Color as SyntectColor, FontStyle, Style as SyntectStyle,
    ThemeSet,
};
use syntect::parsing::SyntaxSet;
use syntect::easy::HighlightLines;

fn syntect_to_ratatui(syntect_style: SyntectStyle) -> RatatuiStyle {
    let mut style = RatatuiStyle::default()
        .fg(Color::Rgb(
            syntect_style.foreground.r,
            syntect_style.foreground.g,
            syntect_style.foreground.b,
        ));

    if syntect_style.font_style.contains(FontStyle::BOLD) {
        style = style.add_modifier(Modifier::BOLD);
    }
    if syntect_style.font_style.contains(FontStyle::ITALIC) {
        style = style.add_modifier(Modifier::ITALIC);
    }
    style
}

/// Highlight a code string and return a Ratatui Text
fn highlight_code<'a>(
    code: &str,
    extension: &str,
    syntax_set: &SyntaxSet,
    theme_set: &ThemeSet,
) -> Text<'a> {
    let theme = &theme_set.themes["base16-ocean.dark"];

    let syntax = syntax_set
        .find_syntax_by_extension(extension)
        .unwrap_or_else(|| syntax_set.find_syntax_plain_text());

    let mut highlighter = HighlightLines::new(syntax, theme);
    let mut lines = Vec::new();

    for line in code.lines() {
        let ranges = highlighter
            .highlight_line(line, syntax_set)
            .unwrap_or_default();

        let spans: Vec<Span> = ranges
            .into_iter()
            .map(|(style, text)| {
                Span::styled(text.to_string(), syntect_to_ratatui(style))
            })
            .collect();

        lines.push(Line::from(spans));
    }

    Text::from(lines)
}

fn main() {
    let syntax_set = SyntaxSet::load_defaults_newlines();
    let theme_set = ThemeSet::load_defaults();

    let code = r#"fn greet(name: &str) -> String {
    format!("Hello, {}!", name)
}

fn main() {
    let greeting = greet("Rustacean");
    println!("{}", greeting);
}"#;

    let highlighted = highlight_code(code, "rs", &syntax_set, &theme_set);

    println!("Generated {} highlighted lines", highlighted.lines.len());
    println!("Each line contains styled Spans for Ratatui rendering.");

    // In a real app:
    // let paragraph = Paragraph::new(highlighted)
    //     .block(Block::default().borders(Borders::ALL).title("main.rs"));
    // frame.render_widget(paragraph, area);
}
```

## Caching Highlighted Output

Syntax highlighting is computationally expensive. For a coding agent that displays the same code blocks across multiple frames, you should cache the highlighted output:

```rust
use std::collections::HashMap;
use ratatui::text::Text;

/// Cache for syntax-highlighted code blocks
struct HighlightCache {
    /// Maps (code_hash, language) -> highlighted Text
    cache: HashMap<(u64, String), Vec<CachedLine>>,
}

/// Cached representation of a highlighted line
#[derive(Clone)]
struct CachedLine {
    spans: Vec<(String, CachedStyle)>,
}

#[derive(Clone)]
struct CachedStyle {
    fg_r: u8,
    fg_g: u8,
    fg_b: u8,
    bold: bool,
    italic: bool,
}

impl HighlightCache {
    fn new() -> Self {
        Self {
            cache: HashMap::new(),
        }
    }

    fn get(&self, code_hash: u64, language: &str) -> Option<&Vec<CachedLine>> {
        self.cache.get(&(code_hash, language.to_string()))
    }

    fn insert(&mut self, code_hash: u64, language: &str, lines: Vec<CachedLine>) {
        self.cache.insert((code_hash, language.to_string()), lines);
    }

    /// Remove entries to keep cache bounded
    fn evict_if_needed(&mut self, max_entries: usize) {
        while self.cache.len() > max_entries {
            // Simple eviction: remove an arbitrary entry
            if let Some(key) = self.cache.keys().next().cloned() {
                self.cache.remove(&key);
            }
        }
    }
}

fn simple_hash(s: &str) -> u64 {
    // A basic hash function for demonstration
    // In production, use std::hash or a proper hasher
    let mut hash: u64 = 5381;
    for byte in s.bytes() {
        hash = hash.wrapping_mul(33).wrapping_add(byte as u64);
    }
    hash
}

fn main() {
    let mut cache = HighlightCache::new();

    let code = "fn main() { println!(\"hello\"); }";
    let hash = simple_hash(code);

    // First time: cache miss, must highlight
    if cache.get(hash, "rs").is_none() {
        println!("Cache miss -- highlighting code");
        let lines = vec![CachedLine {
            spans: vec![("fn main()...".to_string(), CachedStyle {
                fg_r: 137, fg_g: 180, fg_b: 250,
                bold: true, italic: false,
            })],
        }];
        cache.insert(hash, "rs", lines);
    }

    // Second time: cache hit
    if cache.get(hash, "rs").is_some() {
        println!("Cache hit -- reusing highlighted output");
    }

    cache.evict_if_needed(1000);
    println!("Cache entries: {}", cache.cache.len());
}
```

## Incremental Highlighting for Streaming

When the LLM streams a code block token by token, you cannot wait for the complete block before highlighting. You need to highlight incrementally as tokens arrive:

```rust
use syntect::highlighting::ThemeSet;
use syntect::parsing::{ParseState, ScopeStack, SyntaxSet};

/// Maintains highlighting state across incremental updates
struct IncrementalHighlighter {
    syntax_set: SyntaxSet,
    theme_set: ThemeSet,
    /// Accumulated code so far
    buffer: String,
    /// The language extension for syntax detection
    language: String,
    /// Number of lines already highlighted
    highlighted_lines: usize,
}

impl IncrementalHighlighter {
    fn new(language: &str) -> Self {
        Self {
            syntax_set: SyntaxSet::load_defaults_newlines(),
            theme_set: ThemeSet::load_defaults(),
            buffer: String::new(),
            language: language.to_string(),
            highlighted_lines: 0,
        }
    }

    /// Append new tokens from the stream
    fn push(&mut self, token: &str) {
        self.buffer.push_str(token);
    }

    /// Get the number of complete lines available
    fn complete_lines(&self) -> usize {
        self.buffer.lines().count()
    }

    /// Highlight only the new lines since last call.
    /// Returns (line_index, highlighted_line) pairs.
    fn highlight_new_lines(&mut self) -> Vec<(usize, Vec<(String, u8, u8, u8)>)> {
        let theme = &self.theme_set.themes["base16-ocean.dark"];
        let syntax = self.syntax_set
            .find_syntax_by_extension(&self.language)
            .unwrap_or_else(|| self.syntax_set.find_syntax_plain_text());

        let mut highlighter = syntect::easy::HighlightLines::new(syntax, theme);
        let lines: Vec<&str> = self.buffer.lines().collect();

        let mut new_highlighted = Vec::new();

        // Re-highlight from the beginning to maintain parser state.
        // This is necessary because syntect's parser is stateful --
        // a multi-line string or comment started on line 1 affects
        // highlighting of line 10.
        for (idx, line) in lines.iter().enumerate() {
            let ranges = highlighter
                .highlight_line(line, &self.syntax_set)
                .unwrap_or_default();

            if idx >= self.highlighted_lines {
                let styled: Vec<(String, u8, u8, u8)> = ranges
                    .into_iter()
                    .map(|(style, text)| {
                        (text.to_string(), style.foreground.r,
                         style.foreground.g, style.foreground.b)
                    })
                    .collect();
                new_highlighted.push((idx, styled));
            }
        }

        self.highlighted_lines = lines.len();
        new_highlighted
    }
}

fn main() {
    let mut highlighter = IncrementalHighlighter::new("rs");

    // Simulate streaming tokens
    highlighter.push("fn main() {\n");
    highlighter.push("    let x = 42;\n");

    let new_lines = highlighter.highlight_new_lines();
    println!("Highlighted {} new lines", new_lines.len());

    // More tokens arrive
    highlighter.push("    println!(\"{}\", x);\n");
    highlighter.push("}\n");

    let more_lines = highlighter.highlight_new_lines();
    println!("Highlighted {} more lines", more_lines.len());
}
```

Note the important caveat: syntect's parser is stateful. A multi-line string or block comment started on an early line affects the highlighting of later lines. This means you cannot highlight line N in isolation -- you must run the parser from the beginning. For code blocks under a few hundred lines (the typical case in agent responses), this is fast enough. For very large files, you can checkpoint the parser state periodically and restart from the nearest checkpoint.

## Color Degradation for Limited Terminals

Not all terminals support true color. Your highlighting should degrade gracefully:

```rust
use ratatui::style::Color;

#[derive(Clone, Copy)]
enum ColorCapability {
    TrueColor,
    Extended256,
    Basic16,
    None,
}

/// Convert an RGB color to the appropriate terminal representation
fn adapt_color(r: u8, g: u8, b: u8, capability: ColorCapability) -> Color {
    match capability {
        ColorCapability::TrueColor => Color::Rgb(r, g, b),

        ColorCapability::Extended256 => {
            // Map RGB to the closest 256-color palette entry
            // The 216-color cube uses indices 16-231
            // Each channel has 6 levels: 0, 95, 135, 175, 215, 255
            let r_idx = rgb_to_ansi_channel(r);
            let g_idx = rgb_to_ansi_channel(g);
            let b_idx = rgb_to_ansi_channel(b);
            let index = 16 + 36 * r_idx + 6 * g_idx + b_idx;
            Color::Indexed(index)
        }

        ColorCapability::Basic16 => {
            // Map to the closest basic ANSI color
            rgb_to_basic_color(r, g, b)
        }

        ColorCapability::None => Color::Reset,
    }
}

fn rgb_to_ansi_channel(value: u8) -> u8 {
    // Map 0-255 to 0-5 for the 6x6x6 color cube
    match value {
        0..=47 => 0,
        48..=115 => 1,
        116..=155 => 2,
        156..=195 => 3,
        196..=235 => 4,
        236..=255 => 5,
    }
}

fn rgb_to_basic_color(r: u8, g: u8, b: u8) -> Color {
    let brightness = (r as u16 + g as u16 + b as u16) / 3;
    let is_bright = brightness > 128;

    // Determine dominant channel
    if r > g && r > b {
        if is_bright { Color::LightRed } else { Color::Red }
    } else if g > r && g > b {
        if is_bright { Color::LightGreen } else { Color::Green }
    } else if b > r && b > g {
        if is_bright { Color::LightBlue } else { Color::Blue }
    } else if brightness > 200 {
        Color::White
    } else if brightness > 100 {
        Color::Gray
    } else {
        Color::DarkGray
    }
}

fn main() {
    // Catppuccin Mocha blue: RGB(137, 180, 250)
    let true_color = adapt_color(137, 180, 250, ColorCapability::TrueColor);
    let extended = adapt_color(137, 180, 250, ColorCapability::Extended256);
    let basic = adapt_color(137, 180, 250, ColorCapability::Basic16);

    println!("True color: {:?}", true_color);
    println!("256-color:  {:?}", extended);
    println!("Basic 16:   {:?}", basic);
}
```

::: wild In the Wild
Claude Code renders syntax-highlighted code blocks in its responses using terminal escape sequences. When running in a terminal that supports true color, the highlighting matches VS Code themes closely. In more limited terminals, the colors degrade but remain readable. This adaptive behavior ensures that the agent is usable across the wide range of terminal environments developers use -- from cutting-edge GPU terminals to SSH sessions through legacy infrastructure.
:::

## Key Takeaways

- `syntect` uses TextMate-compatible grammars (the same as VS Code) and produces `(Style, &str)` spans that you convert to Ratatui's `Span` type with `Style::fg(Color::Rgb(r, g, b))`.
- Cache highlighted output by hashing the code string, since syntax highlighting is expensive and the same code block renders across many frames.
- Incremental highlighting during streaming requires re-running the parser from the beginning of the code block because syntect's parser is stateful (multi-line strings and comments carry state across lines).
- Degrade colors gracefully from true color (24-bit RGB) to 256-color palette to basic 16 colors based on detected terminal capabilities, ensuring readable output everywhere.
- For production use, consider pre-built crates like `syntect` combined with Ratatui's `Paragraph` widget to build syntax-highlighted code panels with scrolling and line numbers.
