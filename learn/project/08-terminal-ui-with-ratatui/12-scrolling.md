---
title: Scrolling
description: Implement smooth scrolling for long conversations with auto-scroll on new content and manual scroll-back navigation.
---

# Scrolling

> **What you'll learn:**
> - How to track scroll position and viewport size for content that exceeds the visible area
> - How to implement auto-scroll that follows new content but pauses when the user scrolls up
> - How to render a scroll indicator or scrollbar widget to show position in long content

A coding agent conversation can produce hundreds of lines of output -- explanations, code blocks, tool results, error messages. Your UI needs smooth scrolling that lets users read back through the conversation while automatically following new content as it streams in.

## Scroll State

The fundamental scrolling problem is: you have N lines of content and a viewport that shows M lines (where N > M). You need to track which M-line window of the content is visible. Ratatui's `Paragraph` widget accepts a `scroll` parameter for this:

```rust
use ratatui::{prelude::*, widgets::{Paragraph, Wrap}};

pub struct ScrollState {
    /// Current vertical scroll offset (number of lines scrolled from top).
    pub offset: u16,
    /// Total number of content lines (computed after rendering).
    pub total_lines: u16,
    /// Height of the viewport (computed from the layout).
    pub viewport_height: u16,
    /// Whether auto-scroll is enabled (follows new content).
    pub auto_scroll: bool,
}

impl ScrollState {
    pub fn new() -> Self {
        Self {
            offset: 0,
            total_lines: 0,
            viewport_height: 0,
            auto_scroll: true,
        }
    }

    /// The maximum valid scroll offset.
    pub fn max_offset(&self) -> u16 {
        self.total_lines.saturating_sub(self.viewport_height)
    }

    /// Whether we are scrolled to the bottom.
    pub fn is_at_bottom(&self) -> bool {
        self.offset >= self.max_offset()
    }
}
```

## Scroll Operations

Each scroll action updates the offset with bounds checking:

```rust
impl ScrollState {
    pub fn scroll_up(&mut self, lines: u16) {
        self.offset = self.offset.saturating_sub(lines);
        // User scrolled up manually -- disable auto-scroll
        self.auto_scroll = false;
    }

    pub fn scroll_down(&mut self, lines: u16) {
        self.offset = (self.offset + lines).min(self.max_offset());
        // If user scrolled to the bottom, re-enable auto-scroll
        if self.is_at_bottom() {
            self.auto_scroll = true;
        }
    }

    pub fn scroll_to_top(&mut self) {
        self.offset = 0;
        self.auto_scroll = false;
    }

    pub fn scroll_to_bottom(&mut self) {
        self.offset = self.max_offset();
        self.auto_scroll = true;
    }

    pub fn page_up(&mut self) {
        let page = self.viewport_height.saturating_sub(2); // overlap 2 lines
        self.scroll_up(page);
    }

    pub fn page_down(&mut self) {
        let page = self.viewport_height.saturating_sub(2);
        self.scroll_down(page);
    }

    /// Called when new content is added (e.g., streaming tokens).
    pub fn on_content_changed(&mut self, new_total_lines: u16) {
        self.total_lines = new_total_lines;
        if self.auto_scroll {
            self.offset = self.max_offset();
        }
    }
}
```

The auto-scroll behavior is the key user experience detail: when the user is watching new tokens stream in, the view should follow. But the moment they scroll up to re-read something, auto-scroll pauses. It resumes only when they scroll back down to the bottom.

## Integrating Scroll State with the App

Add the scroll state to your application model:

```rust
pub struct App {
    pub messages: Vec<CachedMessage>,
    pub conversation_scroll: ScrollState,
    pub input: String,
    pub cursor_position: usize,
    // ... other fields
}

impl App {
    pub fn update(&mut self, msg: Message) {
        match msg {
            Message::ScrollUp => self.conversation_scroll.scroll_up(1),
            Message::ScrollDown => self.conversation_scroll.scroll_down(1),
            Message::ScrollToTop => self.conversation_scroll.scroll_to_top(),
            Message::ScrollToBottom => self.conversation_scroll.scroll_to_bottom(),
            Message::PageUp => self.conversation_scroll.page_up(),
            Message::PageDown => self.conversation_scroll.page_down(),

            Message::TokenReceived(token) => {
                // Append to the last message
                if let Some(last) = self.messages.last_mut() {
                    last.append_token(&token, &self.highlighter);
                }
                // Update scroll with new content height
                let total = self.compute_total_lines();
                self.conversation_scroll.on_content_changed(total);
            }

            // ... other handlers
            _ => {}
        }
    }

    fn compute_total_lines(&self) -> u16 {
        self.messages
            .iter()
            .map(|m| m.rendered.len() as u16 + 2) // +2 for header and separator
            .sum()
    }
}
```

## Rendering with Scroll Offset

The `Paragraph` widget's `scroll()` method takes a `(vertical_offset, horizontal_offset)` tuple:

```rust
fn render_conversation(frame: &mut Frame, app: &mut App, area: Rect) {
    let block = Block::default()
        .title(" Conversation ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan));

    // Compute the inner area (inside the border) for viewport height
    let inner_height = block.inner(area).height;
    app.conversation_scroll.viewport_height = inner_height;

    // Build all the lines
    let all_lines = build_conversation_lines(&app.messages);

    // Update total lines
    let total = all_lines.len() as u16;
    app.conversation_scroll.total_lines = total;

    // Auto-scroll adjustment
    if app.conversation_scroll.auto_scroll {
        app.conversation_scroll.offset = app.conversation_scroll.max_offset();
    }

    let paragraph = Paragraph::new(all_lines)
        .block(block)
        .wrap(Wrap { trim: false })
        .scroll((app.conversation_scroll.offset, 0));

    frame.render_widget(paragraph, area);
}
```

::: python Coming from Python
In `curses`, you handle scrolling manually by maintaining a `pad` (a virtual screen larger than the terminal) and calling `pad.refresh()` with the visible region coordinates:
```python
import curses

pad = curses.newpad(1000, 80)
# ... write content to pad ...
# Display rows 50-73 of the pad in the terminal window
pad.refresh(scroll_offset, 0, 0, 0, 23, 79)
```
Ratatui's `Paragraph::scroll()` serves the same purpose but is more declarative -- you set the offset and the widget handles clipping to the visible region. Python's `textual` framework provides a `ScrollView` widget that handles scrolling automatically, which is closer to the experience of wrapping Ratatui's scroll state in a reusable component.
:::

## Rendering a Scrollbar

A scrollbar gives users a visual indication of where they are in the content. Ratatui provides a `Scrollbar` widget:

```rust
use ratatui::widgets::{Scrollbar, ScrollbarOrientation, ScrollbarState};

fn render_conversation_with_scrollbar(
    frame: &mut Frame,
    app: &mut App,
    area: Rect,
) {
    // Render the conversation paragraph (as before)
    let block = Block::default()
        .title(" Conversation ")
        .borders(Borders::ALL);

    let inner = block.inner(area);

    let all_lines = build_conversation_lines(&app.messages);
    let total = all_lines.len();

    let paragraph = Paragraph::new(all_lines)
        .block(block)
        .wrap(Wrap { trim: false })
        .scroll((app.conversation_scroll.offset, 0));

    frame.render_widget(paragraph, area);

    // Render the scrollbar on the right edge
    if total as u16 > inner.height {
        let mut scrollbar_state = ScrollbarState::new(total)
            .position(app.conversation_scroll.offset as usize)
            .viewport_content_length(inner.height as usize);

        let scrollbar = Scrollbar::new(ScrollbarOrientation::VerticalRight)
            .begin_symbol(Some("^"))
            .end_symbol(Some("v"))
            .track_symbol(Some("|"))
            .thumb_symbol("*");

        // Render in the inner area (next to the content)
        frame.render_stateful_widget(scrollbar, inner, &mut scrollbar_state);
    }
}
```

The scrollbar is a stateful widget that uses `ScrollbarState` to compute the thumb position and size based on the total content length, viewport size, and current scroll offset.

## Scroll Position Indicator

An alternative to a full scrollbar is a simple position indicator in the title or border. This uses less screen space:

```rust
fn conversation_block(scroll: &ScrollState) -> Block {
    let position_text = if scroll.total_lines > scroll.viewport_height {
        let percentage = if scroll.max_offset() == 0 {
            100
        } else {
            (scroll.offset as u32 * 100 / scroll.max_offset() as u32) as u16
        };
        format!(" Conversation [{}%] ", percentage)
    } else {
        String::from(" Conversation ")
    };

    Block::default()
        .title(position_text)
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan))
}
```

## Handling Line Wrapping

There is a subtlety with scrolling and line wrapping. When `Wrap` is enabled, a single logical line might occupy multiple physical rows on screen. The `scroll` offset in `Paragraph` counts *logical* lines, not physical wrapped rows. This means:

- If your content has a 200-character line and the terminal is 80 columns wide, that line wraps into 3 physical rows.
- Scrolling down by 1 skips all 3 physical rows of that line, not just one.

For most agent output (markdown with short lines), this is acceptable. But if you need pixel-perfect physical-row scrolling, you would need to pre-wrap the text yourself:

```rust
/// Pre-wrap text to a given width, returning one Line per physical row.
fn pre_wrap_lines(lines: &[Line<'static>], width: u16) -> Vec<Line<'static>> {
    let mut wrapped = Vec::new();

    for line in lines {
        let line_text: String = line.spans.iter()
            .map(|s| s.content.as_ref())
            .collect();

        if line_text.len() <= width as usize {
            wrapped.push(line.clone());
        } else {
            // Simple character-based wrapping
            // (production code would handle grapheme clusters and span boundaries)
            for chunk in line_text.as_bytes().chunks(width as usize) {
                let text = String::from_utf8_lossy(chunk).to_string();
                wrapped.push(Line::from(text));
            }
        }
    }

    wrapped
}
```

For your coding agent, the standard `Paragraph::scroll()` with `Wrap` is usually sufficient. Reserve pre-wrapping for cases where smooth scrolling through long lines is critical.

::: wild In the Wild
Claude Code implements smart auto-scrolling that stays pinned to the bottom during streaming but immediately pauses when the user scrolls up. It also shows a "scroll to bottom" indicator when the user is not at the bottom of the conversation, making it easy to jump back. This auto-scroll with manual override pattern is the standard approach across production agents and the one you have implemented here with the `auto_scroll` flag.
:::

## Key Takeaways

- **Scroll state** tracks offset, total content lines, and viewport height -- the maximum offset is `total_lines - viewport_height`.
- **Auto-scroll** follows new content automatically but pauses when the user scrolls up, resuming when they return to the bottom.
- **`Paragraph::scroll()`** takes a `(vertical, horizontal)` offset tuple that clips the rendered content to the visible region.
- **The Scrollbar widget** is a stateful widget that visualizes scroll position; a percentage indicator in the title is a lighter alternative.
- **Line wrapping** means the scroll offset counts logical lines, not physical rows -- for most agent output this is fine, but pre-wrapping is available when finer control is needed.
