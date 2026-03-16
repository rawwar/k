---
title: Widget System
description: Ratatui's built-in widget library including Paragraph, List, Table, Block, Tabs, and Gauge — their APIs, styling options, and composition patterns.
---

# Widget System

> **What you'll learn:**
> - The Widget trait interface and how widgets render themselves into a rectangular buffer area
> - Built-in widgets (Paragraph, List, Table, Block, Tabs, Gauge) and how to configure their styles and content
> - Composing widgets by nesting them inside Block borders and combining them with layout constraints

Widgets are the building blocks of every Ratatui interface. Each widget knows how to render itself into a rectangular region of the terminal buffer. Ratatui ships with a rich set of built-in widgets that cover most common UI patterns, and when they are not enough, you can build custom widgets by implementing the same trait. In this subchapter, we focus on the built-in widgets and how to compose them.

## The Widget Trait

At the core of Ratatui's widget system is a simple trait:

```rust
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;

// Simplified view of the Widget trait:
trait Widget {
    fn render(self, area: Rect, buf: &mut Buffer);
}

// The widget receives:
// - area: the rectangular region it should draw into (x, y, width, height)
// - buf: the terminal buffer to write cells into
//
// Notice that `self` is consumed (moved), not borrowed.
// Widgets are ephemeral -- they are constructed and consumed each frame.
// This is the immediate-mode pattern: no persistent widget objects.

fn main() {
    println!("Widget trait: render(self, area, buf)");
    println!("Widgets are consumed on render -- no persistent state.");
    println!("Build a new widget from your Model each frame.");
}
```

The `self` by-value is a key design decision. Since widgets are consumed on render, they cannot persist between frames. You build them fresh from your Model each time `terminal.draw()` runs. This is the immediate-mode approach in action.

## Block: The Universal Container

`Block` is the most used widget -- it draws borders, titles, and padding around other content. Nearly every other widget accepts a `Block` to wrap itself:

```rust
use ratatui::{
    layout::Rect,
    style::{Color, Style},
    widgets::{Block, Borders, BorderType, Padding},
};

fn demonstrate_blocks() {
    // Basic block with all borders
    let basic = Block::default()
        .borders(Borders::ALL)
        .title("Simple Block");

    // Styled block with rounded borders
    let styled = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(Color::Cyan))
        .title("Styled Block")
        .title_style(Style::default().fg(Color::Yellow));

    // Block with padding (inner margin)
    let padded = Block::default()
        .borders(Borders::ALL)
        .padding(Padding::new(2, 2, 1, 1)); // left, right, top, bottom

    // Block with only top and bottom borders
    let partial = Block::default()
        .borders(Borders::TOP | Borders::BOTTOM)
        .title("Horizontal Divider");

    println!("Blocks provide borders, titles, and padding for other widgets.");
}

fn main() {
    demonstrate_blocks();
}
```

The border types available are: `Plain` (`─│┌┐└┘`), `Rounded` (`─│╭╮╰╯`), `Double` (`═║╔╗╚╝`), `Thick` (`━┃┏┓┗┛`), and `QuadrantOutside`/`QuadrantInside` for block-character borders.

## Paragraph: Text Display

`Paragraph` is the workhorse for displaying text. It supports styled spans, line wrapping, alignment, and scrolling:

```rust
use ratatui::{
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, Borders, Paragraph, Wrap},
};

fn build_agent_response() -> Paragraph<'static> {
    // Build styled text with multiple spans per line
    let text = Text::from(vec![
        Line::from(vec![
            Span::styled("Assistant", Style::default()
                .fg(Color::Green)
                .add_modifier(Modifier::BOLD)),
            Span::raw(": "),
        ]),
        Line::from("Here is the implementation you requested:"),
        Line::from(""),
        Line::from(vec![
            Span::styled("fn ", Style::default().fg(Color::Blue)),
            Span::styled("main", Style::default().fg(Color::Yellow)),
            Span::raw("() {"),
        ]),
        Line::from(vec![
            Span::raw("    println!("),
            Span::styled("\"Hello!\"", Style::default().fg(Color::Green)),
            Span::raw(");"),
        ]),
        Line::from("}"),
    ]);

    Paragraph::new(text)
        .block(Block::default().borders(Borders::ALL).title("Response"))
        .wrap(Wrap { trim: false })
        .scroll((0, 0)) // (vertical_offset, horizontal_offset)
}

fn main() {
    let widget = build_agent_response();
    println!("Paragraph supports styled spans, wrapping, and scrolling.");
    println!("Each Line can contain multiple Spans with different styles.");
}
```

::: python Coming from Python
If you have used Rich's `Text` object with `Text.from_markup("[bold red]Error[/]")`, Ratatui's `Span` and `Line` types serve the same purpose but with explicit construction rather than markup parsing. Rich's approach is more concise for simple cases. Ratatui's approach is more type-safe and avoids the overhead of parsing a markup language at runtime. Some Ratatui companion crates like `tui-markup` do provide Rich-style markup if you prefer that workflow.
:::

## List: Scrollable Item Lists

`List` renders a collection of items with optional selection highlighting:

```rust
use ratatui::{
    style::{Color, Modifier, Style},
    widgets::{Block, Borders, List, ListItem, ListState},
};

fn build_tool_list() -> (List<'static>, ListState) {
    let items = vec![
        ListItem::new("  read_file   - Read contents of a file"),
        ListItem::new("  write_file  - Write content to a file"),
        ListItem::new("  shell       - Execute a shell command"),
        ListItem::new("  search      - Search for patterns in files"),
        ListItem::new("  list_files  - List directory contents"),
    ];

    let list = List::new(items)
        .block(Block::default().borders(Borders::ALL).title("Available Tools"))
        .highlight_style(
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD)
        )
        .highlight_symbol(">> ");

    // ListState tracks which item is selected
    let mut state = ListState::default();
    state.select(Some(0)); // Select the first item

    (list, state)
}

fn main() {
    let (list, state) = build_tool_list();
    println!("List widget with {} items", 5);
    println!("Selected index: {:?}", state.selected());
    println!("Use frame.render_stateful_widget(list, area, &mut state)");
    println!("to render a list with selection tracking.");
}
```

Note that `List` uses `render_stateful_widget` instead of `render_widget`. The `ListState` persists across frames in your Model, tracking which item is selected and the scroll position.

## Table: Structured Data

`Table` renders columnar data with headers, configurable column widths, and row selection:

```rust
use ratatui::{
    layout::Constraint,
    style::{Color, Modifier, Style},
    widgets::{Block, Borders, Cell, Row, Table, TableState},
};

fn build_execution_table() -> (Table<'static>, TableState) {
    let header = Row::new(vec![
        Cell::from("Tool").style(Style::default().add_modifier(Modifier::BOLD)),
        Cell::from("Status").style(Style::default().add_modifier(Modifier::BOLD)),
        Cell::from("Duration").style(Style::default().add_modifier(Modifier::BOLD)),
    ]).height(1);

    let rows = vec![
        Row::new(vec![
            Cell::from("shell"),
            Cell::from("Success").style(Style::default().fg(Color::Green)),
            Cell::from("1.2s"),
        ]),
        Row::new(vec![
            Cell::from("read_file"),
            Cell::from("Success").style(Style::default().fg(Color::Green)),
            Cell::from("0.1s"),
        ]),
        Row::new(vec![
            Cell::from("shell"),
            Cell::from("Failed").style(Style::default().fg(Color::Red)),
            Cell::from("5.0s"),
        ]),
    ];

    let table = Table::new(rows, [
            Constraint::Length(15),     // Tool column: fixed 15 chars
            Constraint::Length(10),     // Status column: fixed 10 chars
            Constraint::Min(8),         // Duration: at least 8, takes remaining
        ])
        .header(header)
        .block(Block::default().borders(Borders::ALL).title("Tool Executions"))
        .highlight_style(Style::default().add_modifier(Modifier::REVERSED));

    let mut state = TableState::default();
    state.select(Some(0));

    (table, state)
}

fn main() {
    let (table, _state) = build_execution_table();
    println!("Table widget: columnar data with headers and row selection.");
    println!("Column widths use the same Constraint system as layouts.");
}
```

## Tabs and Gauge

Two more widgets round out common agent UI needs:

```rust
use ratatui::{
    style::{Color, Style},
    widgets::{Block, Borders, Gauge, Tabs},
};

fn build_tab_bar(active: usize) -> Tabs<'static> {
    let titles = vec!["Chat", "Tools", "Files", "Settings"];
    Tabs::new(titles)
        .block(Block::default().borders(Borders::BOTTOM))
        .select(active)
        .style(Style::default().fg(Color::White))
        .highlight_style(Style::default().fg(Color::Yellow))
        .divider("|")
}

fn build_progress_gauge(progress: f64, label: &str) -> Gauge<'_> {
    Gauge::default()
        .block(Block::default().borders(Borders::ALL).title("Progress"))
        .gauge_style(Style::default().fg(Color::Cyan))
        .ratio(progress.clamp(0.0, 1.0))
        .label(label)
}

fn main() {
    let tabs = build_tab_bar(0);
    let gauge = build_progress_gauge(0.65, "Processing: 65%");
    println!("Tabs: navigation between views");
    println!("Gauge: progress bars and completion indicators");
}
```

## Composing Widgets

The real power of Ratatui's widget system comes from composition. You combine Layout splits with Block containers and content widgets to build complex interfaces:

```rust
use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    widgets::{Block, Borders, List, ListItem, Paragraph, Wrap},
};

fn render_agent_ui(frame: &mut Frame) {
    // Top-level vertical split: header, main content, input
    let main_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),    // Header bar
            Constraint::Min(5),       // Main content area
            Constraint::Length(3),    // Input field
        ])
        .split(frame.area());

    // Header
    let header = Paragraph::new(" Agent v1.0 | Model: claude | Tokens: 1,234")
        .style(Style::default().fg(Color::White).bg(Color::DarkGray));
    frame.render_widget(header, main_chunks[0]);

    // Split main content: chat on left, tools on right
    let content_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(70),  // Chat panel
            Constraint::Percentage(30),  // Tool panel
        ])
        .split(main_chunks[1]);

    // Chat messages
    let messages = vec![
        ListItem::new("System: Agent ready."),
        ListItem::new("You: Can you read main.rs?"),
        ListItem::new("Assistant: Reading the file now..."),
    ];
    let chat = List::new(messages)
        .block(Block::default().borders(Borders::ALL).title("Chat"));
    frame.render_widget(chat, content_chunks[0]);

    // Tool panel
    let tool_output = Paragraph::new("$ cat src/main.rs\nfn main() {\n    println!(\"hello\");\n}")
        .block(Block::default().borders(Borders::ALL).title("Tool Output"))
        .wrap(Wrap { trim: false });
    frame.render_widget(tool_output, content_chunks[1]);

    // Input field
    let input = Paragraph::new("Type your message here...")
        .style(Style::default().fg(Color::DarkGray))
        .block(Block::default().borders(Borders::ALL).title("Input"));
    frame.render_widget(input, main_chunks[2]);
}

fn main() {
    println!("Widget composition: Layout splits + Block containers + content widgets");
    println!("Each widget renders into its allocated Rect from the Layout.");
    println!("Nesting creates complex, responsive interfaces from simple pieces.");
}
```

This composition pattern -- layout splits to divide space, Block for borders and titles, content widgets for the actual data -- is the fundamental technique for building any Ratatui interface. In the next subchapter, we will explore the Layout engine in depth.

## Key Takeaways

- The `Widget` trait's `render(self, area, buf)` method consumes the widget, enforcing the immediate-mode pattern where widgets are built fresh from your Model each frame.
- `Block` is the universal container that provides borders, titles, and padding; nearly every other widget accepts a Block to wrap its content.
- `Paragraph` handles text display with styled `Span`s, line wrapping, alignment, and scrolling; `List` and `Table` handle collections with selection state.
- Stateful widgets (List, Table) use `render_stateful_widget` with a separate state struct that persists in your Model across frames to track selection and scroll position.
- Complex interfaces are built by composing Layout splits (for space division) with Block containers (for borders) and content widgets (for data), nesting them to create multi-panel layouts.
