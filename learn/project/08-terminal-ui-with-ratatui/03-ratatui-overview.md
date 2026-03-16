---
title: Ratatui Overview
description: Introduction to the Ratatui framework, its immediate-mode rendering model, and how it compares to other Rust TUI libraries.
---

# Ratatui Overview

> **What you'll learn:**
> - How Ratatui's immediate-mode rendering model differs from retained-mode GUI frameworks
> - How the Terminal, Frame, and Backend abstractions work together
> - How to set up a minimal Ratatui application with crossterm as the backend

Ratatui is the Rust ecosystem's standard library for building terminal user interfaces. It started as a fork of the `tui-rs` crate (which is now archived) and has become the actively maintained successor with a growing ecosystem of extensions and widgets. For your coding agent, Ratatui provides the rendering engine that turns your application state into pixels (well, character cells) on the screen.

## Immediate Mode vs. Retained Mode

The most important concept to understand about Ratatui is its **immediate-mode rendering** model. This determines how you think about your entire UI.

**Retained mode** (what most GUI frameworks use) works like this: you create UI objects (buttons, labels, text fields), add them to a tree, and the framework keeps them around, updating the screen when their state changes. React, Qt, and GTK all work this way. You create a widget once and mutate it later.

**Immediate mode** works differently: every frame, you describe the *entire* UI from scratch based on your current application state. There are no persistent widget objects. You do not create a "Paragraph widget" and keep a reference to it -- instead, every time the screen needs to update, you construct a new Paragraph from your current data and tell Ratatui to render it.

```rust
// Immediate mode: every frame, describe the full UI
fn draw(frame: &mut Frame, app_state: &AppState) {
    // No widget objects are stored between frames.
    // Every frame creates fresh widgets from the current state.
    let paragraph = Paragraph::new(app_state.current_text.as_str())
        .block(Block::default().title("Output").borders(Borders::ALL));

    frame.render_widget(paragraph, frame.area());
}
```

This might sound wasteful, but it has huge advantages:

1. **No state synchronization bugs** -- your UI always reflects your current state because it is rebuilt from that state every frame.
2. **Simple mental model** -- rendering is a pure function from state to UI. No callbacks, no observers, no event listeners on widgets.
3. **Easy testing** -- you can test your rendering logic by constructing a state and checking what widgets it produces.

::: python Coming from Python
If you have used Python's `curses` library, Ratatui's model will feel familiar -- `curses` also redraws the screen each frame. But if you have used `textual` (the modern Python TUI framework), that is *retained mode*: you create widget classes, mount them in a DOM-like tree, and update them reactively. Ratatui's approach is closer to Python's `rich` library used in "live" mode, where you pass a renderable to `Live.update()` each tick.
:::

## The Core Abstractions

Ratatui has three key abstractions that work together:

### Backend

The backend is the bridge between Ratatui and the terminal. It handles the actual writing of bytes to stdout. Ratatui supports multiple backends:

- **CrosstermBackend** (recommended) -- uses the `crossterm` crate, works on macOS, Linux, and Windows.
- **TermionBackend** -- uses the `termion` crate, Unix only.
- **TermwizBackend** -- uses Meta's `termwiz` crate.
- **TestBackend** -- an in-memory backend for testing (no actual terminal needed).

For your agent, you will use CrosstermBackend. It provides the widest platform support and the most active maintenance.

### Terminal

The `Terminal` struct wraps a backend and manages the double-buffering system. It holds two buffers: the current frame and the previous frame. When you call `terminal.draw()`, it:

1. Gives you a `Frame` to draw widgets into (the current buffer).
2. After your drawing closure returns, diffs the current buffer against the previous buffer.
3. Writes only the changed cells to the backend.
4. Swaps the buffers so the current frame becomes "previous" for next time.

### Frame

The `Frame` is what you interact with inside the `terminal.draw()` closure. It provides:

- `frame.area()` -- the total drawable area (the terminal's dimensions).
- `frame.render_widget(widget, area)` -- renders a widget into a rectangular region.
- `frame.set_cursor_position(x, y)` -- positions the cursor (useful for text input).

## Setting Up a Minimal Application

Let's build the smallest possible Ratatui application. First, add the dependencies to your `Cargo.toml`:

```toml
[dependencies]
ratatui = "0.29"
crossterm = "0.28"
```

Now here is a complete, runnable application:

```rust
use std::io::{self, stdout};
use crossterm::{
    execute,
    terminal::{
        enable_raw_mode, disable_raw_mode,
        EnterAlternateScreen, LeaveAlternateScreen,
    },
    event::{self, Event, KeyCode},
};
use ratatui::{
    prelude::*,
    widgets::{Block, Borders, Paragraph},
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 1. Setup: enter raw mode and alternate screen
    enable_raw_mode()?;
    let mut stdout = stdout();
    execute!(stdout, EnterAlternateScreen)?;

    // 2. Create the terminal with a crossterm backend
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // 3. Run the main loop
    loop {
        // Draw the UI
        terminal.draw(|frame| {
            let area = frame.area();
            let paragraph = Paragraph::new("Hello, Ratatui! Press 'q' to quit.")
                .block(
                    Block::default()
                        .title(" My Agent ")
                        .borders(Borders::ALL)
                        .border_style(Style::default().fg(Color::Cyan)),
                )
                .style(Style::default().fg(Color::White));

            frame.render_widget(paragraph, area);
        })?;

        // Handle input
        if let Event::Key(key) = event::read()? {
            if key.code == KeyCode::Char('q') {
                break;
            }
        }
    }

    // 4. Teardown: restore the terminal
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;

    Ok(())
}
```

This is the skeleton that every Ratatui application shares: setup, loop (draw + handle events), teardown.

## Safe Terminal Cleanup with RAII

The setup/teardown pattern from the previous example is fragile -- if your application panics between setup and teardown, the terminal is left in a broken state. A better approach wraps the terminal lifecycle in a struct that implements `Drop`:

```rust
use std::io::{self, stdout, Stdout};
use crossterm::{
    execute,
    terminal::{
        enable_raw_mode, disable_raw_mode,
        EnterAlternateScreen, LeaveAlternateScreen,
    },
};
use ratatui::prelude::*;

/// Wraps the terminal and ensures cleanup on drop.
pub struct Tui {
    terminal: Terminal<CrosstermBackend<Stdout>>,
}

impl Tui {
    pub fn new() -> Result<Self, Box<dyn std::error::Error>> {
        enable_raw_mode()?;
        let mut stdout = stdout();
        execute!(stdout, EnterAlternateScreen)?;

        let backend = CrosstermBackend::new(stdout);
        let terminal = Terminal::new(backend)?;

        // Install a panic hook for safety
        let original_hook = std::panic::take_hook();
        std::panic::set_hook(Box::new(move |info| {
            let _ = disable_raw_mode();
            let _ = execute!(io::stdout(), LeaveAlternateScreen);
            original_hook(info);
        }));

        Ok(Self { terminal })
    }

    pub fn draw<F>(&mut self, f: F) -> Result<(), Box<dyn std::error::Error>>
    where
        F: FnOnce(&mut Frame),
    {
        self.terminal.draw(f)?;
        Ok(())
    }

    pub fn terminal_mut(&mut self) -> &mut Terminal<CrosstermBackend<Stdout>> {
        &mut self.terminal
    }
}

impl Drop for Tui {
    fn drop(&mut self) {
        let _ = disable_raw_mode();
        let _ = execute!(self.terminal.backend_mut(), LeaveAlternateScreen);
    }
}
```

With this wrapper, terminal cleanup happens automatically when the `Tui` struct is dropped, whether the application exits normally or panics. This is the Rust equivalent of Python's context manager pattern (`with` statements), but enforced by the compiler.

## The TestBackend for Unit Testing

One of Ratatui's best features is the `TestBackend`, which lets you test your rendering logic without a real terminal:

```rust
#[cfg(test)]
mod tests {
    use ratatui::{prelude::*, widgets::Paragraph};

    #[test]
    fn test_renders_greeting() {
        // Create a test backend with a fixed size
        let backend = backend::TestBackend::new(40, 10);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal.draw(|frame| {
            let greeting = Paragraph::new("Hello, world!");
            frame.render_widget(greeting, frame.area());
        }).unwrap();

        // Inspect the buffer to verify rendering
        let buffer = terminal.backend().buffer().clone();
        let content = buffer.content().iter()
            .map(|cell| cell.symbol())
            .collect::<String>();

        assert!(content.contains("Hello, world!"));
    }
}
```

This is extremely useful for ensuring your agent's UI renders correctly without needing to run it in a real terminal.

::: wild In the Wild
Claude Code and OpenCode both use full-screen terminal UIs built on immediate-mode rendering models. Claude Code uses the Ink framework (React for terminals in JavaScript), which is retained-mode, while OpenCode uses the Bubble Tea framework in Go, which follows the same immediate-mode Elm architecture that you will implement with Ratatui. The patterns you learn here translate directly to understanding how production agents build their interfaces.
:::

## Key Takeaways

- **Immediate-mode rendering** rebuilds the entire UI from state every frame -- this eliminates state synchronization bugs and makes rendering a pure function.
- **Ratatui's three core abstractions** are the Backend (writes bytes), the Terminal (manages double-buffering and diffing), and the Frame (what you draw widgets into).
- **CrosstermBackend** is the recommended backend for cross-platform TUI applications in Rust.
- **The RAII pattern** (wrapping terminal setup/teardown in a struct with `Drop`) ensures the terminal is always restored, even on panic.
- **TestBackend** enables unit testing of rendering logic without a real terminal, making your UI code as testable as your business logic.
