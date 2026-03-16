---
title: "Chapter 9: Terminal User Interfaces"
description: Building rich terminal user interfaces with Ratatui, from ANSI fundamentals to the Elm architecture, custom widgets, and accessible design.
---

# Terminal User Interfaces

This chapter covers the art and engineering of building rich, interactive terminal user interfaces. While web and desktop UIs get most of the attention, terminal UIs occupy a unique niche for developer tools: they work over SSH, inside tmux, on remote servers, and in CI environments. For a coding agent, the terminal is the natural habitat, and a well-crafted TUI can provide an experience that rivals graphical interfaces.

We start with the fundamentals: the history of terminals from hardware teletypes to modern GPU-accelerated emulators, ANSI escape sequences that control cursor position and text styling, and the critical distinction between raw and cooked input modes. Understanding these foundations will help you debug issues that arise when your TUI misbehaves in certain terminals or over certain connections.

The bulk of the chapter focuses on Ratatui, the leading Rust TUI framework. You will learn its architecture — the Elm-inspired model-view-update pattern — and how to compose complex interfaces from widgets, layout constraints, and event handlers. We cover building custom widgets for agent-specific needs like streaming markdown rendering, syntax-highlighted code blocks, and tool execution panels. The chapter closes with often-neglected but critical topics: accessibility for screen readers and performance optimization for smooth 60fps rendering.

## Learning Objectives
- Understand terminal fundamentals from ANSI escape codes to raw mode and alternate screen buffers
- Apply the Elm architecture (model-view-update) pattern to structure TUI applications
- Build layouts with Ratatui's constraint-based layout engine for responsive terminal interfaces
- Create custom widgets for agent-specific UI components like streaming text and tool panels
- Implement keyboard event handling with support for key chords, mouse events, and paste detection
- Address accessibility and performance concerns in terminal applications

## Subchapters
1. [Terminal History](/linear/09-terminal-user-interfaces/01-terminal-history)
2. [Terminal Emulators](/linear/09-terminal-user-interfaces/02-terminal-emulators)
3. [ANSI Escape Sequences](/linear/09-terminal-user-interfaces/03-ansi-escape-sequences)
4. [Raw vs Cooked Mode](/linear/09-terminal-user-interfaces/04-raw-vs-cooked-mode)
5. [TUI Frameworks Landscape](/linear/09-terminal-user-interfaces/05-tui-frameworks-landscape)
6. [Ratatui Architecture](/linear/09-terminal-user-interfaces/06-ratatui-architecture)
7. [The Elm Architecture](/linear/09-terminal-user-interfaces/07-the-elm-architecture)
8. [Widget System](/linear/09-terminal-user-interfaces/08-widget-system)
9. [Layout Engine](/linear/09-terminal-user-interfaces/09-layout-engine)
10. [Event Handling](/linear/09-terminal-user-interfaces/10-event-handling)
11. [Custom Widgets](/linear/09-terminal-user-interfaces/11-custom-widgets)
12. [Text Input](/linear/09-terminal-user-interfaces/12-text-input)
13. [Syntax Highlighting in Terminal](/linear/09-terminal-user-interfaces/13-syntax-highlighting-in-terminal)
14. [Accessibility](/linear/09-terminal-user-interfaces/14-accessibility)
15. [Performance Rendering](/linear/09-terminal-user-interfaces/15-performance-rendering)
16. [Summary](/linear/09-terminal-user-interfaces/16-summary)

## Prerequisites
- Chapter 8 (streaming and real-time data handling for driving live terminal displays)
