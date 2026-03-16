---
title: Layout System
description: Use Ratatui's constraint-based layout system to divide terminal space into nested, responsive rectangular regions.
---

# Layout System

> **What you'll learn:**
> - How to use `Layout::default()` with direction and constraints to split areas
> - How percentage, length, min, max, and ratio constraints control region sizing
> - How to nest layouts to create complex multi-region terminal interfaces

Every TUI application needs to divide the screen into regions -- a conversation area, an input box, a status bar, maybe a sidebar. Ratatui's layout system solves this with a constraint-based approach: you describe *what you want* (proportions, minimum sizes, fixed heights) and the layout engine figures out the exact pixel -- character cell -- coordinates.

## The Layout Primitive

The `Layout` struct splits a rectangular area into smaller rectangles along a single direction, either vertical or horizontal. You provide constraints that describe how the available space should be divided.

```rust
use ratatui::prelude::*;

fn basic_layout(area: Rect) -> Vec<Rect> {
    // Split the area vertically into three regions
    Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),    // exactly 3 rows
            Constraint::Min(1),      // at least 1 row, takes remaining space
            Constraint::Length(1),    // exactly 1 row
        ])
        .split(area)
        .to_vec()
}
```

The `split()` method takes a `Rect` (the area to divide) and returns a `Vec<Rect>` with one entry per constraint. Each `Rect` has `x`, `y`, `width`, and `height` fields describing its position and size in character cells.

## Constraint Types

Ratatui offers five constraint types, and understanding when to use each one is essential for building responsive layouts.

### Length: Fixed Size

`Constraint::Length(n)` requests exactly `n` rows (vertical) or columns (horizontal). Use this for elements with a known, fixed size like status bars or input boxes:

```rust
// A status bar that is always exactly 1 row tall
Constraint::Length(1)

// An input box that is always 3 rows tall (1 border + 1 content + 1 border)
Constraint::Length(3)
```

### Min: Minimum Size with Flexibility

`Constraint::Min(n)` requests at least `n` rows/columns but will expand to fill remaining space. This is the workhorse constraint for content areas:

```rust
// The conversation pane should be at least 5 rows
// but takes all remaining space after fixed elements are placed
Constraint::Min(5)
```

When multiple `Min` constraints compete for space, the layout engine distributes remaining space proportionally.

### Max: Maximum Size with Flexibility

`Constraint::Max(n)` requests at most `n` rows/columns. The region can shrink below this but will never exceed it:

```rust
// A sidebar that grows with content but never exceeds 40 columns
Constraint::Max(40)
```

### Percentage: Proportional Size

`Constraint::Percentage(p)` requests `p%` of the available space:

```rust
// Split the screen 70/30 between main content and sidebar
Layout::default()
    .direction(Direction::Horizontal)
    .constraints([
        Constraint::Percentage(70),
        Constraint::Percentage(30),
    ])
    .split(area)
```

### Ratio: Fractional Size

`Constraint::Ratio(num, den)` requests `num/den` of the available space. This is useful when you want exact fractions:

```rust
// Split into thirds
Layout::default()
    .direction(Direction::Horizontal)
    .constraints([
        Constraint::Ratio(1, 3),
        Constraint::Ratio(1, 3),
        Constraint::Ratio(1, 3),
    ])
    .split(area)
```

## Building a Typical Agent Layout

Let's build the layout for your coding agent step by step. The target layout has a title bar at the top, a conversation pane in the middle, an input box near the bottom, and a status bar at the very bottom:

```rust
use ratatui::prelude::*;

/// Splits the terminal into the four main regions of our agent UI.
fn agent_layout(area: Rect) -> AgentAreas {
    let main_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),    // title bar
            Constraint::Min(5),      // conversation pane (flexible)
            Constraint::Length(3),    // input box
            Constraint::Length(1),    // status bar
        ])
        .split(area);

    AgentAreas {
        title_bar: main_chunks[0],
        conversation: main_chunks[1],
        input: main_chunks[2],
        status_bar: main_chunks[3],
    }
}

struct AgentAreas {
    title_bar: Rect,
    conversation: Rect,
    input: Rect,
    status_bar: Rect,
}
```

On an 80x24 terminal, this produces:
- Title bar: 80 columns wide, 1 row tall, at row 0
- Conversation: 80 columns wide, 19 rows tall (24 - 1 - 3 - 1), starting at row 1
- Input: 80 columns wide, 3 rows tall, starting at row 20
- Status bar: 80 columns wide, 1 row tall, at row 23

The conversation pane uses `Min(5)` so it gets all the leftover space after the fixed-size elements are placed.

## Nesting Layouts

Real applications need more complex arrangements. You achieve this by splitting the output of one layout with another layout. For example, adding a sidebar to the conversation area:

```rust
use ratatui::prelude::*;

fn nested_layout(area: Rect) -> (Rect, Rect, Rect, Rect, Rect) {
    // First, split vertically into three bands
    let vertical = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(5),      // content area
            Constraint::Length(3),    // input
            Constraint::Length(1),    // status bar
        ])
        .split(area);

    // Then split the content area horizontally
    let horizontal = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(70),  // conversation
            Constraint::Percentage(30),  // sidebar
        ])
        .split(vertical[0]);

    (
        horizontal[0],  // conversation pane
        horizontal[1],  // sidebar
        vertical[1],    // input box
        vertical[2],    // status bar
        area,           // full area (for overlays)
    )
}
```

::: python Coming from Python
Python's `textual` uses CSS-like grid and dock layouts. If you have used CSS flexbox, Ratatui's constraints are the closest analog:
```python
# Textual CSS
Screen {
    layout: grid;
    grid-size: 2 3;
}
#conversation { row-span: 2; }
#sidebar { row-span: 2; }
#input { column-span: 2; }
```
Ratatui's approach is more explicit -- you manually nest layouts instead of declaring a grid. This gives you fine-grained control but requires you to think about the nesting hierarchy up front.
:::

## Margin and Spacing

You can add margins to layouts to create visual spacing between regions:

```rust
use ratatui::prelude::*;

fn layout_with_margins(area: Rect) -> Vec<Rect> {
    Layout::default()
        .direction(Direction::Vertical)
        .margin(1) // 1 cell margin on all sides
        .constraints([
            Constraint::Percentage(50),
            Constraint::Percentage(50),
        ])
        .split(area)
        .to_vec()
}
```

The `margin(1)` shrinks the available area by 1 cell on each side before splitting. On a 80x24 terminal, the splittable area becomes 78x22.

For more control, you can also shrink areas manually:

```rust
use ratatui::prelude::*;

/// Shrink a Rect by a given amount on each side.
fn shrink(area: Rect, horizontal: u16, vertical: u16) -> Rect {
    Rect {
        x: area.x + horizontal,
        y: area.y + vertical,
        width: area.width.saturating_sub(horizontal * 2),
        height: area.height.saturating_sub(vertical * 2),
    }
}
```

## Responsive Layouts

Terminals come in many sizes. Your layout should adapt gracefully. A common pattern is to check the available width and hide optional panels when the terminal is too narrow:

```rust
use ratatui::prelude::*;

fn responsive_layout(area: Rect) -> (Rect, Option<Rect>, Rect, Rect) {
    let vertical = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(5),
            Constraint::Length(3),
            Constraint::Length(1),
        ])
        .split(area);

    // Only show the sidebar if the terminal is wide enough
    let (conversation, sidebar) = if area.width >= 100 {
        let horizontal = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Percentage(70),
                Constraint::Percentage(30),
            ])
            .split(vertical[0]);

        (horizontal[0], Some(horizontal[1]))
    } else {
        // Narrow terminal: conversation takes the full width
        (vertical[0], None)
    };

    (conversation, sidebar, vertical[1], vertical[2])
}
```

This pattern gives your agent a professional feel -- it works on small terminals but takes advantage of larger ones.

## Layout as Data

A useful practice is to compute your layout once per frame and pass the resulting areas to your rendering functions:

```rust
use ratatui::prelude::*;

pub struct UiLayout {
    pub conversation: Rect,
    pub sidebar: Option<Rect>,
    pub input: Rect,
    pub status_bar: Rect,
}

impl UiLayout {
    pub fn compute(area: Rect) -> Self {
        let vertical = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Min(5),
                Constraint::Length(3),
                Constraint::Length(1),
            ])
            .split(area);

        let (conversation, sidebar) = if area.width >= 100 {
            let h = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([
                    Constraint::Percentage(70),
                    Constraint::Percentage(30),
                ])
                .split(vertical[0]);
            (h[0], Some(h[1]))
        } else {
            (vertical[0], None)
        };

        Self {
            conversation,
            sidebar,
            input: vertical[1],
            status_bar: vertical[2],
        }
    }
}

// In your view function:
fn view(frame: &mut Frame, app: &App) {
    let layout = UiLayout::compute(frame.area());

    render_conversation(frame, app, layout.conversation);
    if let Some(sidebar) = layout.sidebar {
        render_sidebar(frame, app, sidebar);
    }
    render_input(frame, app, layout.input);
    render_status_bar(frame, app, layout.status_bar);
}
```

This separates layout computation from rendering, keeping your view function clean and making it easy to adjust the layout without touching widget code.

::: wild In the Wild
OpenCode computes its layout dynamically based on terminal width, collapsing panels on narrow terminals and expanding them on wide ones. Claude Code similarly adapts its layout -- tool output sections expand and collapse depending on content and available space. The pattern of computing layout as data (a struct of `Rect`s) and making responsive decisions based on `area.width` is exactly how production agents handle the wide variety of terminal sizes they encounter in practice.
:::

## Key Takeaways

- **Layouts split a `Rect` into smaller `Rect`s** along a single direction (vertical or horizontal) based on constraints you provide.
- **Five constraint types** cover all sizing needs: `Length` (fixed), `Min` (flexible floor), `Max` (flexible ceiling), `Percentage` (proportional), and `Ratio` (fractional).
- **Nesting layouts** lets you create complex multi-region interfaces by splitting the output of one layout with another.
- **Responsive layouts** adapt to terminal size by checking dimensions and conditionally showing or hiding panels.
- **Computing layout as data** (a struct of `Rect`s) keeps your view function clean and separates layout concerns from rendering concerns.
