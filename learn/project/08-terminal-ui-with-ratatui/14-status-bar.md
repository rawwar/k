---
title: Status Bar
description: Create an informative status bar showing model name, token usage, session state, and streaming indicators.
---

# Status Bar

> **What you'll learn:**
> - How to design a status bar layout with left, center, and right-aligned sections
> - How to display dynamic information like token count, model name, and connection status
> - How to animate status bar elements like spinners to indicate active streaming

The status bar is the information-dense strip at the bottom of your terminal UI. In a single row, it communicates the active model, token usage, current mode, connection status, and whether the agent is streaming. It is the equivalent of a GUI application's footer or an IDE's status line -- small but essential for keeping the user informed.

## Status Bar Data

First, define what information your status bar needs to display:

```rust
pub struct StatusInfo {
    /// The name of the active model (e.g., "claude-sonnet-4-20250514")
    pub model_name: String,
    /// Input tokens used in the current session
    pub input_tokens: usize,
    /// Output tokens used in the current session
    pub output_tokens: usize,
    /// Whether the agent is currently streaming a response
    pub is_streaming: bool,
    /// The current input mode
    pub input_mode: InputMode,
    /// The elapsed time of the current streaming response
    pub stream_elapsed: Option<std::time::Duration>,
    /// Current spinner frame index (for animation)
    pub spinner_frame: usize,
}
```

## A Three-Section Layout

A status bar typically has three sections: left-aligned information, a center section, and right-aligned information. You can implement this using Ratatui's layout system within the single-row status bar area:

```rust
use ratatui::{prelude::*, widgets::Paragraph};

fn render_status_bar(frame: &mut Frame, status: &StatusInfo, area: Rect) {
    // Background color for the entire status bar
    let bg_style = Style::default()
        .bg(Color::Rgb(49, 50, 68))    // Catppuccin Surface0
        .fg(Color::Rgb(205, 214, 244)); // Catppuccin Text

    // Split the status bar into three sections
    let sections = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(40),  // left section
            Constraint::Percentage(30),  // center section
            Constraint::Percentage(30),  // right section
        ])
        .split(area);

    // Left section: mode and model name
    let left = build_left_section(status);
    let left_widget = Paragraph::new(left).style(bg_style);
    frame.render_widget(left_widget, sections[0]);

    // Center section: streaming status
    let center = build_center_section(status);
    let center_widget = Paragraph::new(center)
        .style(bg_style)
        .alignment(Alignment::Center);
    frame.render_widget(center_widget, sections[1]);

    // Right section: token usage
    let right = build_right_section(status);
    let right_widget = Paragraph::new(right)
        .style(bg_style)
        .alignment(Alignment::Right);
    frame.render_widget(right_widget, sections[2]);
}
```

## Building Each Section

### Left Section: Mode and Model

```rust
fn build_left_section(status: &StatusInfo) -> Line<'static> {
    let mode_span = match status.input_mode {
        InputMode::Normal => Span::styled(
            " NORMAL ",
            Style::default()
                .fg(Color::Black)
                .bg(Color::Rgb(137, 180, 250))  // Catppuccin Blue
                .add_modifier(Modifier::BOLD),
        ),
        InputMode::Editing => Span::styled(
            " INSERT ",
            Style::default()
                .fg(Color::Black)
                .bg(Color::Rgb(166, 227, 161))  // Catppuccin Green
                .add_modifier(Modifier::BOLD),
        ),
    };

    // Shorten model name for display
    let model_display = shorten_model_name(&status.model_name);

    Line::from(vec![
        mode_span,
        Span::raw(" "),
        Span::styled(
            model_display,
            Style::default().fg(Color::Rgb(180, 190, 254)), // Catppuccin Lavender
        ),
    ])
}

fn shorten_model_name(name: &str) -> String {
    // "claude-sonnet-4-20250514" -> "sonnet-4"
    // "claude-3-5-haiku-20241022" -> "haiku-3.5"
    if name.contains("sonnet") {
        String::from("sonnet-4")
    } else if name.contains("haiku") {
        String::from("haiku-3.5")
    } else if name.contains("opus") {
        String::from("opus-4")
    } else {
        // Truncate to 20 characters for unknown models
        name.chars().take(20).collect()
    }
}
```

### Center Section: Streaming Indicator

The center section shows a spinner during streaming and elapsed time:

```rust
fn build_center_section(status: &StatusInfo) -> Line<'static> {
    if status.is_streaming {
        let spinner_chars = ['|', '/', '-', '\\'];
        let spinner = spinner_chars[status.spinner_frame % spinner_chars.len()];

        let elapsed_text = if let Some(elapsed) = status.stream_elapsed {
            format!(" {:.1}s", elapsed.as_secs_f64())
        } else {
            String::new()
        };

        Line::from(vec![
            Span::styled(
                format!(" {} ", spinner),
                Style::default()
                    .fg(Color::Rgb(249, 226, 175))  // Catppuccin Yellow
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                "Streaming",
                Style::default().fg(Color::Rgb(249, 226, 175)),
            ),
            Span::styled(
                elapsed_text,
                Style::default().fg(Color::DarkGray),
            ),
        ])
    } else {
        Line::from(Span::styled(
            "Ready",
            Style::default().fg(Color::Rgb(166, 227, 161)), // Catppuccin Green
        ))
    }
}
```

The spinner advances each time a `Tick` message is processed:

```rust
impl App {
    pub fn update(&mut self, msg: Message) {
        match msg {
            Message::Tick => {
                if self.status.is_streaming {
                    self.status.spinner_frame += 1;
                    // Update elapsed time
                    if let Some(start) = self.stream_start_time {
                        self.status.stream_elapsed = Some(start.elapsed());
                    }
                }
            }
            // ... other handlers
            _ => {}
        }
    }
}
```

### Right Section: Token Usage

```rust
fn build_right_section(status: &StatusInfo) -> Line<'static> {
    let total = status.input_tokens + status.output_tokens;

    // Format with K suffix for readability
    let total_display = if total >= 1000 {
        format!("{:.1}K", total as f64 / 1000.0)
    } else {
        format!("{}", total)
    };

    Line::from(vec![
        Span::styled(
            "tokens: ",
            Style::default().fg(Color::DarkGray),
        ),
        Span::styled(
            format!("{} ", total_display),
            Style::default().fg(Color::Rgb(205, 214, 244)), // Catppuccin Text
        ),
        Span::styled(
            format!("({}+{}) ", status.input_tokens, status.output_tokens),
            Style::default().fg(Color::DarkGray),
        ),
    ])
}
```

::: tip Coming from Python
Python's `rich` library has a built-in `Status` spinner:
```python
from rich.console import Console
console = Console()
with console.status("Streaming..."):
    # do work
    pass
```
In `textual`, you would create a `Footer` widget with reactive properties that update automatically. In Ratatui, you build the status bar by hand each frame from your model state. This is more code but gives you complete control over every character in the status line -- you can pack exactly the information you want into the limited horizontal space.
:::

## Alternative Spinner Styles

The simple ASCII spinner works, but you can use Unicode characters for a more polished look:

```rust
/// Different spinner styles for different moods.
pub enum SpinnerStyle {
    Ascii,      // | / - \
    Dots,       // . .. ...
    Braille,    // Unicode braille characters
    Blocks,     // Unicode block characters
}

impl SpinnerStyle {
    pub fn frame(&self, index: usize) -> &'static str {
        match self {
            SpinnerStyle::Ascii => {
                ["| ", "/ ", "- ", "\\ "][index % 4]
            }
            SpinnerStyle::Dots => {
                [".  ", ".. ", "...", "   "][index % 4]
            }
            SpinnerStyle::Braille => {
                ["\u{28F7}", "\u{28EF}", "\u{28DF}", "\u{287F}",
                 "\u{28BF}", "\u{28FB}", "\u{28FD}", "\u{28FE}"][index % 8]
            }
            SpinnerStyle::Blocks => {
                ["\u{2581}", "\u{2582}", "\u{2583}", "\u{2584}",
                 "\u{2585}", "\u{2586}", "\u{2587}", "\u{2588}"][index % 8]
            }
        }
    }
}
```

## Putting It Together

Here is the complete status bar rendering integrated into the main view function:

```rust
pub fn view(frame: &mut Frame, app: &App) {
    let layout = AgentLayout::compute(frame.area(), app.show_sidebar);

    // ... render other panes ...

    // Status bar
    let status = StatusInfo {
        model_name: app.model_name.clone(),
        input_tokens: app.input_tokens,
        output_tokens: app.output_tokens,
        is_streaming: app.is_streaming,
        input_mode: app.input_mode,
        stream_elapsed: app.status.stream_elapsed,
        spinner_frame: app.status.spinner_frame,
    };

    render_status_bar(frame, &status, layout.status_bar);
}
```

::: tip In the Wild
Claude Code's status bar displays the active model, token usage, and cost estimate in real time. OpenCode shows similar information plus the current git branch and working directory. Both agents use the status bar as a persistent information display that never obscures the conversation. The density of information in a single row is what makes the status bar valuable -- users can glance at it without switching context from the conversation pane.
:::

## Key Takeaways

- **A three-section layout** (left, center, right) uses Ratatui's `Layout` within the single-row status bar area, with `Alignment` controlling text positioning in each section.
- **The mode indicator** uses contrasting background colors (blue for Normal, green for Insert) to make the current mode immediately visible.
- **Spinners animate** by incrementing a frame counter on each `Tick` message, cycling through a sequence of characters at the tick rate.
- **Token usage display** formats large numbers with a K suffix and shows the input/output breakdown in a compact format.
- **The status bar data** is computed from the App model each frame, following the immediate-mode pattern -- no persistent status bar state needed beyond the spinner frame counter.
