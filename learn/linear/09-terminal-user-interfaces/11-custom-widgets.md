---
title: Custom Widgets
description: Building custom Ratatui widgets for agent-specific UI components like streaming markdown panels, tool execution logs, and conversation thread views.
---

# Custom Widgets

> **What you'll learn:**
> - How to implement the StatefulWidget trait for widgets that maintain internal state across render frames
> - Building a streaming markdown widget that incrementally renders content as tokens arrive from the LLM
> - Creating a tool execution panel widget that shows command output, status indicators, and timing information

Ratatui's built-in widgets cover common patterns, but a coding agent needs specialized components: a panel that renders streaming markdown as tokens arrive, a tool execution log with status indicators and timing, a conversation thread that visually distinguishes user messages from assistant responses. This is where custom widgets come in.

## Implementing the Widget Trait

A custom widget implements the `Widget` trait to render itself into a buffer region. Let's start with a simple status bar widget:

```rust
use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::Widget,
};

/// A status bar that shows agent state, token count, and model name
struct StatusBar<'a> {
    model_name: &'a str,
    token_count: usize,
    is_streaming: bool,
    elapsed_secs: f64,
}

impl<'a> StatusBar<'a> {
    fn new(model_name: &'a str) -> Self {
        Self {
            model_name,
            token_count: 0,
            is_streaming: false,
            elapsed_secs: 0.0,
        }
    }

    fn token_count(mut self, count: usize) -> Self {
        self.token_count = count;
        self
    }

    fn streaming(mut self, is_streaming: bool) -> Self {
        self.is_streaming = is_streaming;
        self
    }

    fn elapsed(mut self, secs: f64) -> Self {
        self.elapsed_secs = secs;
        self
    }
}

impl<'a> Widget for StatusBar<'a> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        // Only render if we have at least one row
        if area.height == 0 {
            return;
        }

        // Build the status line with styled spans
        let status_indicator = if self.is_streaming {
            Span::styled(" STREAMING ", Style::default()
                .fg(Color::Black)
                .bg(Color::Yellow)
                .add_modifier(Modifier::BOLD))
        } else {
            Span::styled(" READY ", Style::default()
                .fg(Color::Black)
                .bg(Color::Green)
                .add_modifier(Modifier::BOLD))
        };

        let model = Span::styled(
            format!(" {} ", self.model_name),
            Style::default().fg(Color::Cyan),
        );

        let tokens = Span::styled(
            format!(" Tokens: {} ", self.token_count),
            Style::default().fg(Color::White),
        );

        let elapsed = Span::styled(
            format!(" {:.1}s ", self.elapsed_secs),
            Style::default().fg(Color::DarkGray),
        );

        // Fill background
        let bg_style = Style::default().bg(Color::DarkGray);
        for x in area.x..area.x + area.width {
            buf.cell_mut(ratatui::layout::Position::new(x, area.y))
                .map(|cell| cell.set_style(bg_style));
        }

        // Render the line
        let line = Line::from(vec![status_indicator, model, tokens, elapsed]);
        buf.set_line(area.x, area.y, &line, area.width);
    }
}

fn main() {
    let _widget = StatusBar::new("claude-3-opus")
        .token_count(1234)
        .streaming(true)
        .elapsed(3.7);

    println!("Custom widget with builder pattern.");
    println!("Implements Widget trait: render(self, area, buf)");
}
```

The builder pattern (method chaining with `self` return) is idiomatic for Ratatui widgets. Since widgets are consumed on render, the builder constructs the widget and the render call consumes it in the same frame.

## The StatefulWidget Trait

Some widgets need state that persists across frames -- scroll position, selection index, animation frame. The `StatefulWidget` trait separates the renderable widget from its persistent state:

```rust
use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, StatefulWidget, Widget},
};

/// Persistent state for the streaming text widget
#[derive(Default)]
struct StreamingTextState {
    scroll_offset: usize,
    total_lines: usize,
    auto_scroll: bool,
}

/// A widget that displays streaming text with auto-scroll
struct StreamingText<'a> {
    content: &'a [String],
    block: Option<Block<'a>>,
    style: Style,
}

impl<'a> StreamingText<'a> {
    fn new(content: &'a [String]) -> Self {
        Self {
            content,
            block: None,
            style: Style::default(),
        }
    }

    fn block(mut self, block: Block<'a>) -> Self {
        self.block = Some(block);
        self
    }

    fn style(mut self, style: Style) -> Self {
        self.style = style;
        self
    }
}

impl<'a> StatefulWidget for StreamingText<'a> {
    type State = StreamingTextState;

    fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        // Calculate inner area (inside block borders)
        let inner = if let Some(ref block) = self.block {
            let inner = block.inner(area);
            block.clone().render(area, buf);
            inner
        } else {
            area
        };

        if inner.height == 0 || inner.width == 0 {
            return;
        }

        let visible_height = inner.height as usize;
        state.total_lines = self.content.len();

        // Auto-scroll: keep showing the bottom when new content arrives
        if state.auto_scroll && state.total_lines > visible_height {
            state.scroll_offset = state.total_lines - visible_height;
        }

        // Clamp scroll offset
        let max_scroll = state.total_lines.saturating_sub(visible_height);
        state.scroll_offset = state.scroll_offset.min(max_scroll);

        // Render visible lines
        for (i, y) in (inner.y..inner.y + inner.height).enumerate() {
            let line_idx = state.scroll_offset + i;
            if line_idx < self.content.len() {
                let line = &self.content[line_idx];
                buf.set_string(inner.x, y, line, self.style);
            }
        }

        // Show scroll indicator if there is content above or below
        if state.scroll_offset > 0 {
            let indicator = Span::styled(
                " ^ more above ",
                Style::default().fg(Color::DarkGray),
            );
            buf.set_span(inner.x, inner.y, &indicator, inner.width);
        }

        if state.scroll_offset + visible_height < state.total_lines {
            let indicator = Span::styled(
                " v more below ",
                Style::default().fg(Color::DarkGray),
            );
            buf.set_span(
                inner.x,
                inner.y + inner.height - 1,
                &indicator,
                inner.width,
            );
        }
    }
}

fn main() {
    let content: Vec<String> = (0..50)
        .map(|i| format!("Line {}: Some streaming content here...", i))
        .collect();

    let _widget = StreamingText::new(&content)
        .block(Block::default().borders(Borders::ALL).title("Response"))
        .style(Style::default().fg(Color::White));

    let mut state = StreamingTextState {
        scroll_offset: 0,
        total_lines: 0,
        auto_scroll: true,
    };

    // In a real app:
    // frame.render_stateful_widget(widget, area, &mut state);
    // The state persists in your Model across frames.

    println!("StatefulWidget: render(self, area, buf, &mut state)");
    println!("State lives in your Model, widget is ephemeral.");
    println!("Auto-scroll keeps showing newest content during streaming.");
}
```

::: python Coming from Python
In Python's Textual, widgets are persistent objects that hold their own state. A `TextLog` widget keeps its scroll position and text buffer internally. Ratatui's `StatefulWidget` achieves the same thing but with explicit state separation: the widget struct is ephemeral (rebuilt each frame), while the state struct persists in your Model. This pattern aligns with TEA -- all mutable state lives in the Model, and widgets are pure views.
:::

## Building a Tool Execution Panel

A coding agent needs a widget that shows tool execution status with visual indicators. Let's build one:

```rust
use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Widget},
};

#[derive(Clone)]
enum ToolStatus {
    Running,
    Success,
    Failed(String),
}

#[derive(Clone)]
struct ToolExecution {
    name: String,
    command: String,
    status: ToolStatus,
    duration_ms: Option<u64>,
    output_lines: Vec<String>,
}

struct ToolPanel<'a> {
    executions: &'a [ToolExecution],
    block: Option<Block<'a>>,
    max_output_lines: usize,
}

impl<'a> ToolPanel<'a> {
    fn new(executions: &'a [ToolExecution]) -> Self {
        Self {
            executions,
            block: None,
            max_output_lines: 5,
        }
    }

    fn block(mut self, block: Block<'a>) -> Self {
        self.block = Some(block);
        self
    }

    fn max_output_lines(mut self, n: usize) -> Self {
        self.max_output_lines = n;
        self
    }
}

impl<'a> Widget for ToolPanel<'a> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let inner = if let Some(ref block) = self.block {
            let inner = block.inner(area);
            block.clone().render(area, buf);
            inner
        } else {
            area
        };

        if inner.height == 0 {
            return;
        }

        let mut y = inner.y;

        for exec in self.executions {
            if y >= inner.y + inner.height {
                break;
            }

            // Status indicator and tool name
            let (indicator, indicator_style) = match &exec.status {
                ToolStatus::Running => (
                    " RUN ",
                    Style::default().fg(Color::Black).bg(Color::Yellow)
                        .add_modifier(Modifier::BOLD),
                ),
                ToolStatus::Success => (
                    " OK  ",
                    Style::default().fg(Color::Black).bg(Color::Green)
                        .add_modifier(Modifier::BOLD),
                ),
                ToolStatus::Failed(_) => (
                    " ERR ",
                    Style::default().fg(Color::Black).bg(Color::Red)
                        .add_modifier(Modifier::BOLD),
                ),
            };

            let duration_text = exec.duration_ms
                .map(|ms| format!(" ({:.1}s)", ms as f64 / 1000.0))
                .unwrap_or_default();

            let header_line = Line::from(vec![
                Span::styled(indicator, indicator_style),
                Span::raw(" "),
                Span::styled(&exec.name, Style::default()
                    .add_modifier(Modifier::BOLD)),
                Span::styled(duration_text, Style::default().fg(Color::DarkGray)),
            ]);

            buf.set_line(inner.x, y, &header_line, inner.width);
            y += 1;

            // Command
            if y < inner.y + inner.height {
                let cmd_line = Line::from(vec![
                    Span::styled("  $ ", Style::default().fg(Color::DarkGray)),
                    Span::styled(&exec.command, Style::default().fg(Color::Cyan)),
                ]);
                buf.set_line(inner.x, y, &cmd_line, inner.width);
                y += 1;
            }

            // Output (limited lines)
            let output_limit = self.max_output_lines.min(exec.output_lines.len());
            for line in exec.output_lines.iter().take(output_limit) {
                if y >= inner.y + inner.height {
                    break;
                }
                let output_line = Line::from(vec![
                    Span::raw("    "),
                    Span::styled(line.as_str(), Style::default().fg(Color::White)),
                ]);
                buf.set_line(inner.x, y, &output_line, inner.width);
                y += 1;
            }

            // Separator between executions
            if y < inner.y + inner.height {
                y += 1; // Empty line separator
            }
        }
    }
}

fn main() {
    let executions = vec![
        ToolExecution {
            name: "shell".to_string(),
            command: "cargo check".to_string(),
            status: ToolStatus::Success,
            duration_ms: Some(2300),
            output_lines: vec!["Compiling agent v0.1.0".to_string()],
        },
        ToolExecution {
            name: "read_file".to_string(),
            command: "src/main.rs".to_string(),
            status: ToolStatus::Running,
            duration_ms: None,
            output_lines: vec![],
        },
    ];

    let _panel = ToolPanel::new(&executions)
        .block(Block::default().borders(Borders::ALL).title("Tool Executions"))
        .max_output_lines(3);

    println!("Tool execution panel: status indicators + commands + output");
    println!("Each execution shows: [STATUS] name (duration)");
    println!("Followed by the command and truncated output.");
}
```

## Building a Message Bubble Widget

Chat interfaces often use visually distinct "bubbles" for different participants:

```rust
use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::Widget,
};

enum MessageRole {
    User,
    Assistant,
    System,
}

struct MessageBubble<'a> {
    role: MessageRole,
    content: &'a str,
    timestamp: &'a str,
}

impl<'a> MessageBubble<'a> {
    fn new(role: MessageRole, content: &'a str, timestamp: &'a str) -> Self {
        Self { role, content, timestamp }
    }
}

impl<'a> Widget for MessageBubble<'a> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if area.height == 0 || area.width < 10 {
            return;
        }

        let (label, label_style, content_style) = match self.role {
            MessageRole::User => (
                "You",
                Style::default().fg(Color::Blue).add_modifier(Modifier::BOLD),
                Style::default().fg(Color::White),
            ),
            MessageRole::Assistant => (
                "Assistant",
                Style::default().fg(Color::Green).add_modifier(Modifier::BOLD),
                Style::default().fg(Color::White),
            ),
            MessageRole::System => (
                "System",
                Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD),
                Style::default().fg(Color::DarkGray),
            ),
        };

        // Header line: role + timestamp
        let header = Line::from(vec![
            Span::styled(label, label_style),
            Span::raw("  "),
            Span::styled(self.timestamp, Style::default().fg(Color::DarkGray)),
        ]);
        buf.set_line(area.x, area.y, &header, area.width);

        // Content lines (simple word wrapping)
        let max_width = area.width.saturating_sub(2) as usize;
        let mut y = area.y + 1;

        for line in self.content.lines() {
            if y >= area.y + area.height {
                break;
            }

            // Simple wrapping by chunks
            let chars: Vec<char> = line.chars().collect();
            let mut start = 0;
            while start < chars.len() && y < area.y + area.height {
                let end = (start + max_width).min(chars.len());
                let chunk: String = chars[start..end].iter().collect();
                let content_line = Line::from(vec![
                    Span::raw("  "),
                    Span::styled(chunk, content_style),
                ]);
                buf.set_line(area.x, y, &content_line, area.width);
                y += 1;
                start = end;
            }
        }
    }
}

fn main() {
    let _msg = MessageBubble::new(
        MessageRole::Assistant,
        "Here is the implementation you requested. The function reads a file and returns its contents as a string.",
        "12:34",
    );

    println!("MessageBubble widget: role-colored headers with word-wrapped content.");
    println!("User messages in blue, assistant in green, system in yellow.");
}
```

::: wild In the Wild
Production coding agents invest heavily in custom widgets. Claude Code's interface includes specialized components for streaming markdown, diff views, permission prompts, and tool execution displays. Each of these is effectively a custom widget that understands the specific data format and renders it with appropriate styling. The key insight is that generic widgets (Paragraph, List) are starting points, but agent-specific components always require custom rendering logic.
:::

## Key Takeaways

- Custom widgets implement the `Widget` trait (`render(self, area, buf)`) for stateless rendering or `StatefulWidget` (`render(self, area, buf, &mut state)`) when state must persist across frames.
- The builder pattern (method chaining with `self` return) is idiomatic for widget construction in Ratatui, matching the style of built-in widgets.
- `StatefulWidget` cleanly separates the ephemeral widget (rebuilt each frame) from its persistent state (stored in your Model), aligning with the Elm Architecture's state ownership model.
- Agent-specific widgets like tool execution panels, message bubbles, and streaming text displays require custom rendering that goes beyond what built-in widgets provide.
- Always check `area.height` and `area.width` at the start of `render` to handle edge cases where the allocated area is zero-sized or too small to display content.
