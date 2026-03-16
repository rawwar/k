---
title: Layout Engine
description: Ratatui's constraint-based layout system for dividing terminal space into rows, columns, and nested regions that adapt to different terminal sizes.
---

# Layout Engine

> **What you'll learn:**
> - How Ratatui's Layout struct splits a Rect into sub-regions using directional constraints (horizontal/vertical)
> - The constraint types -- Percentage, Length, Min, Max, Ratio -- and how the solver resolves competing constraints
> - Building responsive layouts that reflow gracefully when the terminal is resized during execution

Layout is the spatial backbone of every TUI application. It determines where each widget appears, how much space it gets, and how the interface adapts when the terminal is resized. Ratatui's layout engine uses a **constraint-based** approach where you describe what you want ("the sidebar should be 30% width, the main panel should take the rest") and the solver figures out the exact pixel-column boundaries.

## The Basics: Layout, Direction, and Constraints

A `Layout` takes a rectangular area (`Rect`) and splits it into smaller rectangles. You specify the direction (vertical or horizontal) and a list of constraints that describe how to divide the space:

```rust
use ratatui::layout::{Constraint, Direction, Layout, Rect};

fn main() {
    // Simulate a terminal that is 80 columns x 24 rows
    let terminal_area = Rect::new(0, 0, 80, 24);

    // Vertical split: header (3 rows), content (fill), footer (1 row)
    let vertical_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),    // Exactly 3 rows for header
            Constraint::Min(0),       // Content fills remaining space
            Constraint::Length(1),    // Exactly 1 row for footer
        ])
        .split(terminal_area);

    println!("Header:  {:?}", vertical_chunks[0]);
    // Rect { x: 0, y: 0, width: 80, height: 3 }

    println!("Content: {:?}", vertical_chunks[1]);
    // Rect { x: 0, y: 3, width: 80, height: 20 }

    println!("Footer:  {:?}", vertical_chunks[2]);
    // Rect { x: 0, y: 23, width: 80, height: 1 }

    // Horizontal split of the content area: sidebar + main
    let horizontal_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Length(20),   // Sidebar: fixed 20 columns
            Constraint::Min(0),       // Main: fills remaining space
        ])
        .split(vertical_chunks[1]);

    println!("\nSidebar: {:?}", horizontal_chunks[0]);
    // Rect { x: 0, y: 3, width: 20, height: 20 }

    println!("Main:    {:?}", horizontal_chunks[1]);
    // Rect { x: 20, y: 3, width: 60, height: 20 }
}
```

The output is a `Vec<Rect>`, one rectangle per constraint. Each `Rect` has `x`, `y`, `width`, and `height` fields that tell you exactly where to render.

## Constraint Types

Ratatui provides five constraint types that you combine to describe your layout:

| Constraint | Meaning | Use Case |
|-----------|---------|----------|
| `Length(n)` | Exactly n cells | Fixed-size headers, footers, input fields |
| `Min(n)` | At least n cells, can grow | Content areas that should fill space |
| `Max(n)` | At most n cells, can shrink | Sidebars that should not be too wide |
| `Percentage(n)` | n% of the parent area | Proportional splits (70/30) |
| `Ratio(a, b)` | a/b of the parent area | Precise ratios (1/3, 2/5) |

```rust
use ratatui::layout::{Constraint, Direction, Layout, Rect};

fn main() {
    let area = Rect::new(0, 0, 100, 40);

    // Percentage-based split
    let pct = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(30),
            Constraint::Percentage(70),
        ])
        .split(area);
    println!("30/70 split: {} | {} columns", pct[0].width, pct[1].width);
    // Output: 30/70 split: 30 | 70 columns

    // Ratio-based split (useful for precise fractions)
    let ratio = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Ratio(1, 3),
            Constraint::Ratio(2, 3),
        ])
        .split(area);
    println!("1:2 ratio:   {} | {} columns", ratio[0].width, ratio[1].width);
    // Output: 1:2 ratio:   33 | 67 columns

    // Mixed constraints: fixed + fill
    let mixed = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),     // Fixed header
            Constraint::Min(10),       // Content: at least 10, takes remaining
            Constraint::Length(3),     // Fixed input field
        ])
        .split(area);
    println!(
        "Mixed: header={}, content={}, input={}",
        mixed[0].height, mixed[1].height, mixed[2].height
    );
    // Output: Mixed: header=3, content=34, input=3
}
```

## How the Solver Works

When you provide constraints, the layout solver must decide how to allocate space, especially when constraints conflict or when there is leftover space. The solver follows these rules:

1. **`Length` constraints are satisfied first** -- they get exactly the space they request.
2. **`Min` and `Max` constraints set bounds** -- the solver allocates within these bounds.
3. **Remaining space is distributed** among `Min` constraints proportionally.
4. **`Percentage` and `Ratio`** are computed relative to the total available space.

When constraints cannot all be satisfied (the terminal is too small), the solver degrades gracefully by shrinking flexible constraints:

```rust
use ratatui::layout::{Constraint, Direction, Layout, Rect};

fn main() {
    // What happens when the terminal is too small?
    let small_area = Rect::new(0, 0, 80, 10); // Only 10 rows!

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),    // Wants 3 rows
            Constraint::Min(5),       // Wants at least 5 rows
            Constraint::Length(3),    // Wants 3 rows
        ])
        .split(small_area);

    // Total requested: 3 + 5 + 3 = 11 rows, but we only have 10
    // The solver reduces the Min constraint to fit
    println!("Header:  height = {}", chunks[0].height); // 3
    println!("Content: height = {}", chunks[1].height); // 4 (reduced from 5)
    println!("Footer:  height = {}", chunks[2].height); // 3

    // Even smaller terminal
    let tiny_area = Rect::new(0, 0, 80, 5);

    let tiny_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(5),
            Constraint::Length(3),
        ])
        .split(tiny_area);

    println!("\nTiny terminal (5 rows):");
    for (i, chunk) in tiny_chunks.iter().enumerate() {
        println!("  Chunk {}: height = {}", i, chunk.height);
    }
    // Length constraints may also shrink when space is extremely limited
}
```

::: python Coming from Python
If you have used Textual's CSS-based layout, Ratatui's constraints serve a similar purpose but with a different syntax. Textual lets you write `width: 30%;` or `height: 3;` in CSS. Ratatui uses `Constraint::Percentage(30)` or `Constraint::Length(3)` in code. The mental model is the same: declare desired sizes and let the engine resolve them. The main difference is that Textual supports CSS features like margins, padding in CSS syntax, and grid layout, while Ratatui's layout is limited to sequential horizontal/vertical splits.
:::

## Margins

Layout supports margins to add spacing between chunks:

```rust
use ratatui::layout::{Constraint, Direction, Layout, Margin, Rect};

fn main() {
    let area = Rect::new(0, 0, 80, 24);

    // Add 1-cell margin between chunks
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(1) // 1-cell margin on all sides of the entire layout
        .constraints([
            Constraint::Length(3),
            Constraint::Min(0),
            Constraint::Length(3),
        ])
        .split(area);

    println!("With margin=1:");
    for (i, chunk) in chunks.iter().enumerate() {
        println!("  Chunk {}: x={}, y={}, w={}, h={}",
            i, chunk.x, chunk.y, chunk.width, chunk.height);
    }
    // The entire layout is inset by 1 cell from the parent area

    // You can also apply margins to individual Rects
    let inner = chunks[1].inner(Margin {
        horizontal: 2,
        vertical: 1,
    });
    println!("\nContent inner area (after margins): {:?}", inner);
}
```

## Nested Layouts: Building Complex UIs

Real agent interfaces require nested layouts. You split the screen into major regions, then split those regions further:

```rust
use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout},
    style::{Color, Style},
    widgets::{Block, Borders, Paragraph},
};

fn render_complex_layout(frame: &mut Frame) {
    let area = frame.area();

    // Level 1: Vertical split into header, body, status bar
    let outer = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),    // Header bar
            Constraint::Min(5),       // Body
            Constraint::Length(1),    // Status bar
        ])
        .split(area);

    // Header
    frame.render_widget(
        Paragraph::new(" Agent | Model: claude-3 | Session: abc123")
            .style(Style::default().bg(Color::DarkGray)),
        outer[0],
    );

    // Level 2: Horizontal split of body into sidebar and main
    let body = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Length(25),   // Sidebar
            Constraint::Min(30),      // Main panel
        ])
        .split(outer[1]);

    // Sidebar
    frame.render_widget(
        Paragraph::new("Files\n  src/\n    main.rs\n    lib.rs\n  Cargo.toml")
            .block(Block::default().borders(Borders::ALL).title("Explorer")),
        body[0],
    );

    // Level 3: Vertical split of main panel into chat and tool output
    let main_panel = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage(65),  // Chat
            Constraint::Percentage(35),  // Tool output
        ])
        .split(body[1]);

    // Chat area
    frame.render_widget(
        Paragraph::new("You: Please read main.rs\nAssistant: Reading...")
            .block(Block::default().borders(Borders::ALL).title("Chat")),
        main_panel[0],
    );

    // Tool output
    frame.render_widget(
        Paragraph::new("$ cat src/main.rs\nfn main() { ... }")
            .block(Block::default().borders(Borders::ALL).title("Tool Output")),
        main_panel[1],
    );

    // Status bar
    frame.render_widget(
        Paragraph::new(" Ready | Tokens: 1,234 | Cost: $0.02")
            .style(Style::default().bg(Color::DarkGray)),
        outer[2],
    );
}

fn main() {
    println!("Nested layouts create complex multi-panel interfaces.");
    println!("Level 1: header / body / status (vertical)");
    println!("Level 2: sidebar / main (horizontal)");
    println!("Level 3: chat / tool output (vertical)");
}
```

## Responsive Design: Handling Resize

Terminal windows can be resized at any time. Your layout must adapt. Since Ratatui re-runs your render function every frame, and the frame's `area()` reflects the current terminal size, responsive layout comes naturally -- as long as you use flexible constraints:

```rust
use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout},
    widgets::{Block, Borders, Paragraph},
};

fn render_responsive(frame: &mut Frame) {
    let area = frame.area();

    // Adapt layout based on terminal width
    if area.width >= 120 {
        // Wide terminal: three-column layout
        let cols = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Length(25),    // File explorer
                Constraint::Min(40),       // Chat (flexible)
                Constraint::Length(35),    // Tool output
            ])
            .split(area);

        frame.render_widget(
            Paragraph::new("Explorer").block(Block::default().borders(Borders::ALL)),
            cols[0],
        );
        frame.render_widget(
            Paragraph::new("Chat").block(Block::default().borders(Borders::ALL)),
            cols[1],
        );
        frame.render_widget(
            Paragraph::new("Tools").block(Block::default().borders(Borders::ALL)),
            cols[2],
        );
    } else if area.width >= 80 {
        // Medium terminal: two-column layout, tools below chat
        let cols = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Length(25),
                Constraint::Min(30),
            ])
            .split(area);

        frame.render_widget(
            Paragraph::new("Explorer").block(Block::default().borders(Borders::ALL)),
            cols[0],
        );

        let right = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Percentage(60),
                Constraint::Percentage(40),
            ])
            .split(cols[1]);

        frame.render_widget(
            Paragraph::new("Chat").block(Block::default().borders(Borders::ALL)),
            right[0],
        );
        frame.render_widget(
            Paragraph::new("Tools").block(Block::default().borders(Borders::ALL)),
            right[1],
        );
    } else {
        // Narrow terminal: single column, stacked
        let rows = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Min(5),
                Constraint::Length(8),
            ])
            .split(area);

        frame.render_widget(
            Paragraph::new("Chat").block(Block::default().borders(Borders::ALL)),
            rows[0],
        );
        frame.render_widget(
            Paragraph::new("Tools").block(Block::default().borders(Borders::ALL)),
            rows[1],
        );
    }
}

fn main() {
    println!("Responsive layouts adapt to terminal width:");
    println!("  >= 120 cols: three-column layout");
    println!("  >= 80  cols: two-column with stacked right panel");
    println!("  < 80   cols: single-column stacked layout");
}
```

::: wild In the Wild
Claude Code adapts its interface based on terminal width. When the terminal is wide enough, diffs are shown side-by-side; in narrow terminals, they switch to a unified diff format. This kind of responsive behavior is straightforward with Ratatui's constraint system because the layout solver runs every frame, automatically adapting to the current terminal size provided by `frame.area()`.
:::

## Layout Caching

Ratatui caches layout computations. If you call `Layout::split()` with the same constraints and the same input area, it returns the cached result without recomputing. This means you do not need to cache layout results yourself -- the framework handles it:

```rust
use ratatui::layout::{Constraint, Direction, Layout, Rect};

fn main() {
    let area = Rect::new(0, 0, 80, 24);

    // These two calls return the same result, and the second
    // one hits the cache internally
    let first = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(3), Constraint::Min(0)])
        .split(area);

    let second = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(3), Constraint::Min(0)])
        .split(area);

    assert_eq!(first[0], second[0]);
    assert_eq!(first[1], second[1]);

    println!("Layout results are cached automatically.");
    println!("Repeated calls with same parameters are O(1).");
}
```

## Key Takeaways

- Ratatui's layout engine splits a `Rect` into sub-regions using `Layout` with `Direction` (vertical or horizontal) and a list of `Constraint` values.
- Five constraint types cover all layout needs: `Length` (exact size), `Min` (at least), `Max` (at most), `Percentage`, and `Ratio` -- the solver distributes remaining space and degrades gracefully when the terminal is too small.
- Nested layouts create complex multi-panel interfaces by splitting regions further, using the output `Rect` of one layout as the input for the next.
- Responsive design comes naturally from Ratatui's immediate-mode approach: since `frame.area()` reflects the current terminal size every frame, you can branch on width/height to choose different layout strategies.
- Layout results are cached automatically by Ratatui, so repeated calls with the same constraints and area are effectively free.
