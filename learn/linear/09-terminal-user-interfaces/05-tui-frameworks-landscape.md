---
title: TUI Frameworks Landscape
description: Survey of TUI frameworks across languages — ncurses, tui-rs, Ratatui, Ink, Bubbletea — comparing their architectures, strengths, and ecosystem maturity.
---

# TUI Frameworks Landscape

> **What you'll learn:**
> - The major TUI framework families: C-based ncurses descendants, Rust-native frameworks, and modern frameworks in Go and JavaScript
> - How Ratatui evolved from tui-rs and why it became the dominant choice for Rust TUI applications
> - Architectural differences between immediate-mode rendering (Ratatui) and retained-mode/component frameworks (Ink, Bubbletea)

Before diving into Ratatui's architecture, let's survey the broader landscape of TUI frameworks. Understanding how different ecosystems solve the same problems will give you better intuition for the design decisions Ratatui makes and help you evaluate whether Ratatui is the right choice for your agent's interface.

## The ncurses Legacy

The oldest and most widespread TUI framework family descends from **curses**, originally developed at Berkeley in the late 1970s, and its GNU successor **ncurses** (new curses) from the early 1990s.

ncurses provides a C API for terminal-independent screen painting. It uses the `terminfo` database to look up escape sequences for the current terminal, manages windows and sub-windows on the screen, and handles input in both cooked and raw modes. Nearly every classic Unix TUI application -- `htop`, `mc` (Midnight Commander), `mutt` -- is built on ncurses.

The strengths of ncurses are its ubiquity (it is installed on virtually every Unix system) and its decades of battle-tested terminal compatibility. Its weaknesses are a mutable, imperative API with global state, manual memory management in C, and an architecture that predates modern UI patterns.

::: tip Coming from Python
Python's `curses` module is a thin wrapper around the C ncurses library. If you have written Python TUI code with `curses.wrapper()`, `stdscr.addstr()`, and `curses.color_pair()`, you have used ncurses directly. The experience is low-level and imperative: you manually position the cursor, write characters, and call `refresh()` to push changes to the screen. Ratatui's approach is declarative by comparison -- you describe what the screen should look like, and the framework figures out the minimal updates.
:::

```rust
// This is NOT how you build TUIs in Rust today, but it shows
// the ncurses style for comparison. The `pancurses` crate
// wraps ncurses for Rust.

// ncurses style (conceptual -- do not use for new projects):
//   let window = initscr();
//   window.mvaddstr(5, 10, "Hello from ncurses");
//   window.refresh();
//   endwin();

// Ratatui style (declarative):
// frame.render_widget(
//     Paragraph::new("Hello from Ratatui"),
//     layout[0],
// );
//
// The framework diffs the frame buffer and writes only changed cells.

fn main() {
    println!("ncurses: imperative, position-then-draw");
    println!("Ratatui: declarative, describe-then-render");
    println!();
    println!("We use Ratatui for its safety, composability, and Elm architecture.");
}
```

## The Rust TUI Ecosystem

The Rust TUI ecosystem has converged around a clear winner, but the journey involved several frameworks:

### tui-rs (2017-2022)

**tui-rs** was the first major Rust TUI framework, created by Florian Dehau. It introduced the immediate-mode rendering model to the Rust TUI world: your application provides a closure that draws the entire screen each frame, and the framework diffs the output against the previous frame to minimize terminal writes.

tui-rs was well-designed and gained significant adoption, but by 2022 the original maintainer had less time for the project, and PRs accumulated without being merged.

### Ratatui (2023-present)

**Ratatui** is a community fork of tui-rs that took over active development in early 2023. It kept the core architecture but brought:

- Active, frequent releases (monthly cadence)
- A growing ecosystem of companion crates (`tui-textarea`, `tui-input`, `ratatui-image`)
- Improved documentation and examples
- New widgets and layout features
- An explicit stability commitment

Ratatui is now the clear standard for Rust TUI development. When you see tui-rs in older blog posts or Stack Overflow answers, the advice generally applies to Ratatui as well, since the API is largely compatible.

### Cursive

**Cursive** takes a different approach -- it is a callback-based, retained-mode framework more like traditional GUI toolkits. You create views, register event callbacks on them, and Cursive manages layout and rendering. This feels more like building a GUI application:

```rust
// Cursive style (conceptual):
// let mut siv = cursive::default();
// siv.add_layer(
//     Dialog::text("Hello from Cursive!")
//         .title("Greeting")
//         .button("Quit", |s| s.quit()),
// );
// siv.run();

fn main() {
    println!("Cursive: retained-mode with callbacks, like a GUI toolkit");
    println!("Ratatui: immediate-mode with Elm architecture, like a game loop");
    println!();
    println!("Ratatui gives more control over rendering and is");
    println!("better suited for complex, dynamic UIs like a coding agent.");
}
```

Cursive is solid for dialog-heavy applications (installer wizards, simple forms) but less suited for the streaming, dynamic content that a coding agent displays.

## Cross-Language Comparison

The TUI space has interesting entries in other languages that influence Rust framework design:

### Bubbletea (Go)

**Bubbletea** from Charm is the leading Go TUI framework and a major influence on how Ratatui applications are structured. Bubbletea implements the Elm Architecture (Model-View-Update) explicitly:

```go
// Go Bubbletea structure (for comparison):
// type model struct {
//     cursor int
//     choices []string
// }
//
// func (m model) Update(msg tea.Msg) (tea.Model, tea.Cmd) {
//     switch msg := msg.(type) {
//     case tea.KeyMsg:
//         switch msg.String() {
//         case "up":
//             m.cursor--
//         case "down":
//             m.cursor++
//         case "q":
//             return m, tea.Quit
//         }
//     }
//     return m, nil
// }
//
// func (m model) View() string {
//     // Return a string that IS the entire screen
//     return renderScreen(m)
// }
```

The key difference: Bubbletea's `View()` returns a **string** (the entire screen as text), while Ratatui's view function renders **widgets into a buffer**. Ratatui's approach is more efficient because it can diff at the cell level rather than the string level.

### Ink (JavaScript/React)

**Ink** brings React's component model to the terminal. You write JSX components that render to terminal output. Ink is retained-mode and uses a virtual DOM (technically a virtual terminal buffer) for efficient updates:

```javascript
// Ink (React for terminal) style:
// const App = () => (
//   <Box flexDirection="column">
//     <Text color="green">Hello from Ink!</Text>
//     <Text bold>This is React, but for terminals.</Text>
//   </Box>
// );
// render(<App />);
```

Ink is powerful for JavaScript developers but introduces the full React runtime and Node.js dependency. For a systems-level coding agent, the overhead and startup time are significant drawbacks.

### Textual (Python)

**Textual** from the Rich ecosystem is Python's most sophisticated TUI framework. It uses a CSS-like layout system, an async event loop, and a component architecture inspired by modern web development:

```python
# Textual style (Python):
# from textual.app import App
# from textual.widgets import Header, Footer, Static
#
# class AgentApp(App):
#     def compose(self):
#         yield Header()
#         yield Static("Agent output here")
#         yield Footer()
#
# app = AgentApp()
# app.run()
```

::: tip Coming from Python
If you have used Textual, you will find Ratatui's immediate-mode rendering quite different. Textual maintains a widget tree that it updates incrementally -- you modify a widget's properties and Textual re-renders just that widget. In Ratatui, you redraw the entire screen every frame, and the framework's buffer diffing ensures only changed cells are written. The immediate-mode approach is simpler to reason about (no stale widget state) but requires you to keep all display data in your model.
:::

## Immediate Mode vs Retained Mode

This is the most important architectural distinction in the TUI framework landscape:

**Retained mode** (Cursive, Textual, Ink):
- You create a tree of widget objects that persist across frames
- You update widget properties; the framework re-renders affected widgets
- State lives in the widget tree, and your application and the framework share ownership
- Risk: stale state, inconsistent updates, widget lifecycle management complexity

**Immediate mode** (Ratatui, Bubbletea):
- You provide a render function called every frame
- The render function reads your application state and produces a complete frame
- No persistent widget objects -- widgets are constructed and consumed each frame
- Your application owns all state; the framework is stateless between frames

```rust
// Immediate mode mental model:
// Every frame, your render function runs from scratch.

struct AppState {
    messages: Vec<String>,
    input: String,
    scroll_offset: usize,
}

fn render(state: &AppState) {
    // This function is called every frame.
    // It reads state and renders the ENTIRE screen.
    // No persistent widget objects exist between frames.
    // Ratatui diffs the output buffer to minimize terminal writes.

    println!("Messages: {} (showing from offset {})",
        state.messages.len(),
        state.scroll_offset);
    println!("Input: [{}]", state.input);
}

fn main() {
    let state = AppState {
        messages: vec!["Hello".into(), "World".into()],
        input: "typing...".into(),
        scroll_offset: 0,
    };
    render(&state);
}
```

For a coding agent, immediate mode is the better choice. Agent UIs have highly dynamic content (streaming LLM responses, live tool output, changing status indicators), and the simplicity of "redraw everything from state" makes it easier to ensure the UI is always consistent with the application state.

::: wild In the Wild
Most production coding agents that have TUI interfaces use immediate-mode or hybrid approaches. Claude Code renders its interface fresh on each update from the LLM stream. OpenCode uses Bubbletea (Go's Elm Architecture framework) for its TUI, which is also immediate-mode. The common thread is that streaming, dynamic content maps naturally to the immediate-mode pattern where each new token triggers a redraw.
:::

## Why Ratatui for Our Agent

Given this landscape, Ratatui is the right choice for building a Rust coding agent TUI for several reasons:

1. **Immediate mode matches streaming content** -- new tokens from the LLM trigger a state update and redraw, with no widget lifecycle to manage.
2. **Rust ownership model** -- in retained-mode frameworks, shared widget state creates borrowing headaches. Immediate mode keeps all state in your model struct, which you own entirely.
3. **Performance** -- Ratatui's buffer diffing writes only changed cells, making it efficient even at high redraw rates during streaming.
4. **Ecosystem** -- Ratatui has the largest Rust TUI ecosystem with companion crates for text areas, syntax highlighting, images, and more.
5. **Community** -- active development, responsive maintainers, and a growing collection of examples and documentation.

In the next subchapter, we will look at Ratatui's internal architecture in detail: how the Terminal manages frame buffers, how backends abstract over crossterm and termion, and how the rendering pipeline works from your widgets to the terminal screen.

## Key Takeaways

- The TUI framework landscape spans from the legacy ncurses C library to modern frameworks like Ratatui (Rust), Bubbletea (Go), Ink (JavaScript), and Textual (Python), each with different trade-offs.
- Ratatui is the community fork of tui-rs and has become the dominant Rust TUI framework, with active development, a growing ecosystem, and monthly releases.
- The fundamental architectural choice is immediate mode (Ratatui, Bubbletea) versus retained mode (Cursive, Textual, Ink) -- immediate mode is simpler to reason about and better suited for streaming, dynamic content.
- Immediate-mode rendering means your application owns all state and provides a render function called every frame, while the framework handles diffing and minimal terminal updates.
- For a coding agent with streaming LLM output and dynamic tool panels, Ratatui's immediate-mode rendering with the Elm Architecture is the most natural fit.
