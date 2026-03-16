---
title: Summary
description: Review the complete terminal UI implementation and reflect on the architectural decisions that make it maintainable and extensible.
---

# Summary

> **What you'll learn:**
> - How all the UI components integrate into a cohesive terminal application
> - Which Ratatui patterns and abstractions are most valuable for real-world TUI development
> - What extension points exist for adding new features like split views or file browsers

You have built a complete terminal UI for your coding agent. Let's step back and see how all the pieces fit together, what design decisions made this manageable, and where you can take it from here.

## What You Built

Over the course of this chapter, you transformed a plain-text REPL into a professional terminal application with these components:

**Foundation layer:**
- Raw mode and alternate screen management with safe teardown (panic hooks, RAII)
- An understanding of ANSI escape codes and how Ratatui abstracts them into `Style` objects

**Architecture layer:**
- The Elm architecture (Model-Update-View) keeping all state in a single `App` struct, all events in a `Message` enum, and all rendering in a pure view function
- A multi-pane layout computed from constraints each frame, adapting to terminal size

**Interaction layer:**
- An async event loop using `tokio::select!` to handle keyboard input, streaming tokens, and timer ticks simultaneously
- Mode-based keyboard handling with global shortcuts, normal mode navigation, and editing mode text input
- A text input box with cursor movement, word deletion, multi-line support, and command history

**Rendering layer:**
- Markdown parsing with pulldown-cmark, converting AST events into styled `Line`s and `Span`s
- Syntax highlighting with syntect, translating language-aware colorization into Ratatui styles
- Scrolling with auto-follow during streaming and manual scroll-back navigation

**Polish layer:**
- A status bar showing model name, token usage, streaming indicators, and input mode
- A theming system with presets (dark, light, basic), configuration overrides, and auto-detection

## The Architecture in One Page

Here is the complete flow, end to end:

```rust
// The entire application architecture in pseudocode

struct App {
    messages: Vec<CachedMessage>,     // conversation history
    input_state: InputState,          // text input with cursor, history
    conversation_scroll: ScrollState, // scroll position and auto-scroll
    status: StatusInfo,               // model, tokens, spinner
    focused_pane: FocusedPane,        // which pane has focus
    input_mode: InputMode,            // Normal or Editing
    show_sidebar: bool,               // tool output panel visibility
    theme: Theme,                     // color scheme
    highlighter: Arc<Highlighter>,    // syntax highlighting engine
    should_quit: bool,                // exit flag
}

enum Message {
    // Keyboard: KeyPressed(char), Backspace, Submit, ScrollUp, ...
    // Agent: TokenReceived(String), StreamingCompleted(String), ...
    // System: Tick, Resize(u16, u16), Quit, ...
}

fn update(app: &mut App, msg: Message) {
    // Pure state transition: match on message, modify app
}

fn view(frame: &mut Frame, app: &App) {
    let layout = AgentLayout::compute(frame.area(), app.show_sidebar);
    render_conversation(frame, app, layout.conversation, &app.theme);
    render_input_box(frame, app, layout.input, &app.theme);
    render_status_bar(frame, &app.status, layout.status_bar, &app.theme);
    // ... optional sidebar
}

async fn event_loop(terminal: &mut Terminal<...>, app: &mut App) {
    loop {
        terminal.draw(|frame| view(frame, app))?;
        let msg = tokio::select! {
            // keyboard events, streaming tokens, timer ticks
        };
        update(app, msg);
        if app.should_quit { break; }
    }
}
```

Three functions. One state struct. One message enum. That is the entire skeleton. Every feature you added -- scrolling, markdown, syntax highlighting, theming -- plugs into this skeleton without changing its shape.

## Key Design Decisions

### Immediate Mode Rendering

Rebuilding the UI from scratch every frame eliminated entire categories of bugs. You never had to worry about a widget being out of sync with the state because widgets are constructed from state every frame. The cost (rebuilding widget trees each frame) is negligible for a terminal application -- Ratatui's diffing ensures only changed cells are written to the terminal.

### Message-Based State Transitions

Routing all events through a `Message` enum and a single `update` function made the application predictable. You could add streaming support, tool execution events, and resize handling without restructuring existing code. Each new event type is just a new `Message` variant and a new `match` arm.

### Layout as Data

Computing the layout into a struct of `Rect`s each frame, then passing those rects to dedicated rendering functions, kept the code organized. Adding the sidebar toggle was a one-line change to the layout computation -- the rendering functions did not need to change at all.

### Theme as a Parameter

Passing the `Theme` struct to rendering functions instead of hardcoding colors meant that adding a light theme required zero changes to rendering logic. The theme struct acts as an indirection layer between "what should this look like" (semantic role) and "what color should it be" (specific RGB value).

::: python Coming from Python
If you have worked with Python's MVC frameworks (Django, Flask) or state management libraries (Redux via JavaScript), the Elm architecture should feel familiar. The key difference in Rust is that ownership and borrowing enforce the separation naturally. You *cannot* accidentally mutate state inside a view function because the view receives `&App` (immutable reference). In Python, discipline is needed to avoid mutating state in a renderer -- in Rust, the compiler enforces it.
:::

## Exercises

1. **(Easy)** Add a help overlay that shows keybindings when the user presses `?` in normal mode. Render it as a `Paragraph` with a `Block` centered on the screen, overlaying the conversation pane.

2. **(Medium)** Implement a search function (triggered by `/` in normal mode) that highlights matching text in the conversation. The search input should appear in the status bar area, and matching spans should have a highlighted background.

3. **(Medium)** Add mouse support for scrolling. Crossterm provides `MouseEvent` variants for scroll wheel events. Map `ScrollUp` and `ScrollDown` mouse events to your existing scroll messages.

4. **(Hard)** Implement a split view that shows a file preview alongside the conversation. When the agent reads or writes a file, display the file content with syntax highlighting in a new pane. Use a horizontal split of the conversation area, similar to the tool sidebar.

5. **(Hard)** Add a command palette (triggered by Ctrl+P) that lets users search through available actions (switch theme, toggle sidebar, clear history, change model). Use a `List` widget with fuzzy filtering as the user types.

## What's Next

In the next chapter, you will tackle **conversation context management** -- tracking token usage, compacting long conversations to fit within the model's context window, and managing session persistence. The UI you built here will display that token information in the status bar and handle the visual feedback when the agent compacts the conversation.

The TUI is also where you will integrate the **permission system** from Chapter 12. When the agent requests permission to execute a tool, a focused confirmation dialog will appear in the conversation pane, and the keyboard handling will temporarily switch to a confirmation mode.

::: wild In the Wild
Production coding agents continuously evolve their UIs. Claude Code has gone through multiple iterations of its terminal interface, refining the layout, adding features like collapsible tool output sections, and improving the streaming rendering. OpenCode similarly iterates on its Bubble Tea-based UI. The architecture you have built -- with its clean separation of state, events, and rendering -- is designed to support this kind of iterative improvement. Adding a new pane, a new keybinding, or a new visual element follows the same pattern every time: add state to the Model, add a Message variant, handle it in Update, and render it in View.
:::

## Key Takeaways

- **The Elm architecture scales** -- from a simple hello-world to a multi-pane agent UI with streaming, scrolling, and theming, the Model-Update-View pattern remained the same three-function skeleton throughout.
- **Ratatui's immediate-mode rendering** trades widget persistence for simplicity, making state synchronization bugs impossible and keeping the rendering code straightforward.
- **Layered design** (foundation, architecture, interaction, rendering, polish) lets you build incrementally -- each layer adds capabilities without rewriting the previous one.
- **The complete system** (layout, widgets, events, markdown, syntax highlighting, scrolling, input, status bar, theming) is roughly 1500-2000 lines of Rust -- manageable for a single developer to understand and maintain.
- **Extension points** (help overlays, search, mouse support, split views, command palettes) follow the established patterns, making the UI a platform for future features rather than a one-time implementation.
