---
title: Widgets
description: Explore Ratatui's built-in widgets including Paragraph, Block, List, Table, and how to create custom widgets.
---

# Widgets

> **What you'll learn:**
> - How to use built-in widgets like Paragraph, Block, List, and Table for common UI patterns
> - How to implement the `Widget` trait to create custom rendering components
> - How to compose widgets with borders, titles, and padding using the Block wrapper

Widgets are the building blocks of your UI. Each widget knows how to render itself into a rectangular area of character cells. Ratatui ships with a rich set of built-in widgets, and you can create your own by implementing a simple trait. In this subchapter, you will learn the widgets you need for your coding agent.

## The Widget Trait

Every widget in Ratatui implements the `Widget` trait:

```rust
pub trait Widget {
    fn render(self, area: Rect, buf: &mut Buffer);
}
```

The `render` method takes ownership of the widget (`self`, not `&self`), a `Rect` describing where to draw, and a mutable reference to the `Buffer` (the grid of character cells). This ownership model means widgets are cheap, temporary objects -- you create them, render them, and they are gone.

You do not call `render` directly. Instead, you use `frame.render_widget()`:

```rust
fn draw(frame: &mut Frame) {
    let widget = Paragraph::new("Hello!");
    frame.render_widget(widget, frame.area());
}
```

## Block: The Universal Container

`Block` is the most fundamental widget -- it draws a border and title around a region. Almost every other widget accepts a `Block` as a wrapper:

```rust
use ratatui::{prelude::*, widgets::{Block, Borders, Paragraph}};

fn draw(frame: &mut Frame) {
    // A block with a title and full border
    let block = Block::default()
        .title(" Conversation ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan))
        .title_style(Style::default().fg(Color::White).add_modifier(Modifier::BOLD));

    // Wrap a paragraph in the block
    let paragraph = Paragraph::new("Hello from inside a bordered box!")
        .block(block);

    frame.render_widget(paragraph, frame.area());
}
```

You can customize every aspect of a block:

```rust
use ratatui::{prelude::*, widgets::{Block, Borders, BorderType, Padding}};

fn styled_blocks(frame: &mut Frame) {
    let areas = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Ratio(1, 3),
            Constraint::Ratio(1, 3),
            Constraint::Ratio(1, 3),
        ])
        .split(frame.area());

    // Rounded corners (the default in modern Ratatui)
    let rounded = Block::default()
        .title(" Rounded ")
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded);
    frame.render_widget(rounded, areas[0]);

    // Double-line border
    let double = Block::default()
        .title(" Double ")
        .borders(Borders::ALL)
        .border_type(BorderType::Double);
    frame.render_widget(double, areas[1]);

    // Only top and bottom borders, with inner padding
    let partial = Block::default()
        .title(" Partial ")
        .borders(Borders::TOP | Borders::BOTTOM)
        .padding(Padding::horizontal(2));
    frame.render_widget(partial, areas[2]);
}
```

## Paragraph: Text Display

`Paragraph` is the widget you will use most for your agent. It renders text with styling, wrapping, alignment, and scrolling:

```rust
use ratatui::{prelude::*, widgets::{Block, Borders, Paragraph, Wrap}};

fn conversation_view(frame: &mut Frame, messages: &[String]) {
    // Build styled text with multiple lines
    let mut lines: Vec<Line> = Vec::new();

    for msg in messages {
        // Each message is a styled line
        lines.push(Line::from(vec![
            Span::styled("Agent: ", Style::default().fg(Color::Green).bold()),
            Span::raw(msg),
        ]));
        lines.push(Line::from("")); // blank line between messages
    }

    let paragraph = Paragraph::new(lines)
        .block(Block::default().title(" Conversation ").borders(Borders::ALL))
        .wrap(Wrap { trim: true })       // wrap long lines
        .scroll((0, 0));                 // (vertical_offset, horizontal_offset)

    frame.render_widget(paragraph, frame.area());
}
```

### Styled Text: Spans and Lines

Text in Ratatui is built from three types:

- **`Span`** -- a styled string fragment (like an HTML `<span>`)
- **`Line`** -- a sequence of spans forming a single line
- **`Text`** -- a sequence of lines

```rust
use ratatui::prelude::*;

fn build_styled_text() -> Text<'static> {
    // A single styled span
    let error_label = Span::styled(
        "Error: ",
        Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
    );

    // Combine spans into a line
    let error_line = Line::from(vec![
        error_label,
        Span::raw("file not found"),
    ]);

    // Build a multi-line text block
    Text::from(vec![
        Line::from("Normal text on line 1"),
        error_line,
        Line::from(vec![
            Span::styled("Hint: ", Style::default().fg(Color::Yellow)),
            Span::raw("check the file path"),
        ]),
    ])
}
```

::: python Coming from Python
This is similar to how `rich` builds styled text in Python:
```python
from rich.text import Text
text = Text()
text.append("Error: ", style="bold red")
text.append("file not found")
```
The key difference is that Ratatui's `Span` and `Line` types are owned data structures (or borrow with lifetimes), while `rich.Text` is a mutable object you append to. Ratatui's approach fits the immediate-mode model -- you build text fresh each frame.
:::

## List: Selectable Items

`List` renders a scrollable list of items, perfect for conversation history or tool output:

```rust
use ratatui::{prelude::*, widgets::{Block, Borders, List, ListItem, ListState}};

fn tool_list(frame: &mut Frame) {
    let tools = vec!["read_file", "write_file", "shell_exec", "search"];

    let items: Vec<ListItem> = tools
        .iter()
        .map(|t| {
            ListItem::new(Line::from(vec![
                Span::styled("  > ", Style::default().fg(Color::DarkGray)),
                Span::raw(*t),
            ]))
        })
        .collect();

    let list = List::new(items)
        .block(Block::default().title(" Available Tools ").borders(Borders::ALL))
        .highlight_style(
            Style::default()
                .bg(Color::DarkGray)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol(">> ");

    // ListState tracks which item is selected
    let mut state = ListState::default();
    state.select(Some(0)); // select the first item

    // Use render_stateful_widget for widgets that have state
    frame.render_stateful_widget(list, frame.area(), &mut state);
}
```

Notice the `render_stateful_widget` call -- `List` is a **stateful widget** that needs external state (`ListState`) to track the selected item. The state persists between frames in your Model.

## Table: Structured Data

`Table` displays rows and columns with headers and configurable column widths:

```rust
use ratatui::{prelude::*, widgets::{Block, Borders, Cell, Row, Table}};

fn token_usage_table(frame: &mut Frame, area: Rect) {
    let header = Row::new(vec![
        Cell::from("Model").style(Style::default().add_modifier(Modifier::BOLD)),
        Cell::from("Input").style(Style::default().add_modifier(Modifier::BOLD)),
        Cell::from("Output").style(Style::default().add_modifier(Modifier::BOLD)),
        Cell::from("Total").style(Style::default().add_modifier(Modifier::BOLD)),
    ])
    .height(1);

    let rows = vec![
        Row::new(vec!["claude-sonnet", "1,234", "567", "1,801"]),
        Row::new(vec!["claude-haiku", "890", "234", "1,124"]),
    ];

    let table = Table::new(
        rows,
        [
            Constraint::Length(15),
            Constraint::Length(10),
            Constraint::Length(10),
            Constraint::Length(10),
        ],
    )
    .header(header)
    .block(Block::default().title(" Token Usage ").borders(Borders::ALL))
    .row_highlight_style(Style::default().bg(Color::DarkGray));

    frame.render_widget(table, area);
}
```

## Creating Custom Widgets

For your coding agent, you will eventually need custom widgets -- like a message bubble or a tool execution display. Implement the `Widget` trait:

```rust
use ratatui::{prelude::*, widgets::{Block, Borders}};

/// A custom widget that renders a chat message with role-specific styling.
pub struct MessageBubble<'a> {
    role: &'a str,
    content: &'a str,
    is_streaming: bool,
}

impl<'a> MessageBubble<'a> {
    pub fn new(role: &'a str, content: &'a str) -> Self {
        Self {
            role,
            content,
            is_streaming: false,
        }
    }

    pub fn streaming(mut self, streaming: bool) -> Self {
        self.is_streaming = streaming;
        self
    }
}

impl<'a> Widget for MessageBubble<'a> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        // Choose style based on role
        let (border_color, role_style) = match self.role {
            "user" => (Color::Blue, Style::default().fg(Color::Blue).bold()),
            "assistant" => (Color::Green, Style::default().fg(Color::Green).bold()),
            _ => (Color::Gray, Style::default().fg(Color::Gray)),
        };

        // Create the block wrapper
        let title = if self.is_streaming {
            format!(" {} (streaming...) ", self.role)
        } else {
            format!(" {} ", self.role)
        };

        let block = Block::default()
            .title(title)
            .borders(Borders::ALL)
            .border_style(Style::default().fg(border_color));

        // Get the inner area (inside the border)
        let inner = block.inner(area);

        // Render the block first
        block.render(area, buf);

        // Then render the content inside
        let content_lines: Vec<Line> = self.content
            .lines()
            .map(|line| Line::from(line.to_string()))
            .collect();

        let paragraph = Paragraph::new(content_lines)
            .wrap(Wrap { trim: true });

        paragraph.render(inner, buf);
    }
}
```

Use your custom widget just like any built-in widget:

```rust
fn draw(frame: &mut Frame) {
    let bubble = MessageBubble::new("assistant", "Here's the fix for your code...")
        .streaming(false);
    frame.render_widget(bubble, frame.area());
}
```

## The Builder Pattern

Notice how every widget uses the builder pattern -- `Block::default().title(...).borders(...)`. This is idiomatic Rust for constructing objects with many optional parameters. Each method takes `self` by value and returns `Self`, allowing method chaining:

```rust
// This pattern lets you configure only what you need:
let minimal = Paragraph::new("text");
let styled = Paragraph::new("text")
    .style(Style::default().fg(Color::White))
    .block(Block::default().borders(Borders::ALL))
    .wrap(Wrap { trim: true })
    .scroll((5, 0));
```

::: wild In the Wild
Claude Code renders different tool invocations (file reads, shell commands, search results) with distinct visual treatments -- each tool type has its own rendering style with appropriate colors and formatting. Building custom widgets in Ratatui follows the same principle: you create a widget type for each distinct visual pattern in your application, keeping rendering logic encapsulated and reusable.
:::

## Key Takeaways

- **The `Widget` trait** has a single method (`render`) that draws into a `Buffer` at a given `Rect` -- widgets are lightweight, temporary objects created fresh each frame.
- **Block** is the universal container that provides borders, titles, and padding; most widgets accept a Block via their `.block()` method.
- **Paragraph** renders styled text with wrapping and scrolling -- it is the primary widget for displaying conversation content in your agent.
- **Stateful widgets** like List use `render_stateful_widget` with an external state object that persists between frames in your Model.
- **Custom widgets** implement the `Widget` trait and can compose built-in widgets internally, letting you encapsulate complex rendering logic for message bubbles, tool output, and other agent-specific UI elements.
