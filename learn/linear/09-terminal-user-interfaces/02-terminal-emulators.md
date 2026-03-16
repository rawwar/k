---
title: Terminal Emulators
description: Modern terminal emulators compared — feature support, performance characteristics, and how differences between iTerm2, Alacritty, Kitty, and others affect TUI applications.
---

# Terminal Emulators

> **What you'll learn:**
> - The feature landscape of modern terminal emulators including true color, ligatures, image protocols, and GPU rendering
> - How to detect terminal capabilities at runtime using TERM, COLORTERM, and capability queries
> - Why your TUI must degrade gracefully across terminals with different feature sets and how to implement fallbacks

The terminal emulator is the lens through which your users see your application. While the previous subchapter traced how we got from hardware teletypes to software emulators, this one focuses on the present: the diverse ecosystem of modern terminal emulators, what they can and cannot do, and how your Rust TUI application should adapt.

## The Modern Emulator Landscape

Modern terminal emulators have diverged significantly in their capabilities. Here are the major categories:

**Legacy/system terminals** like macOS Terminal.app and the GNOME Terminal provide a reliable baseline. They support 256 colors, basic mouse reporting, and the standard xterm escape sequences. They do not typically support GPU-accelerated rendering, image display protocols, or advanced features like sixel graphics.

**Power-user terminals** like **iTerm2** (macOS) and **Windows Terminal** push beyond the baseline with true color (24-bit RGB), image display via proprietary protocols (iTerm2 inline images), ligature rendering for programming fonts, and split panes managed by the emulator itself.

**GPU-accelerated terminals** like **Alacritty**, **Kitty**, and **WezTerm** use the GPU for rendering, achieving significantly higher throughput and lower latency. Kitty goes furthest with its own extensions: the Kitty graphics protocol for inline images, the Kitty keyboard protocol for unambiguous key reporting, and progressive enhancement of terminal capabilities.

**Multiplexers** like **tmux** and **GNU Screen** sit between your application and the actual terminal emulator. They introduce their own PTY layer, which can filter or modify escape sequences. A TUI application running inside tmux may behave differently than one running directly in the terminal emulator, because tmux intercepts and reinterprets certain sequences.

## Capability Detection

Your TUI application must discover what the terminal can do before rendering. You cannot simply assume true color support or mouse reporting. Here is how to probe the environment:

```rust
use std::env;

/// Describes the color capability level of the current terminal
#[derive(Debug, Clone, Copy, PartialEq)]
enum ColorSupport {
    /// No color support (TERM=dumb or NO_COLOR set)
    None,
    /// Basic 16 ANSI colors
    Basic,
    /// 256-color palette
    Extended,
    /// Full 24-bit true color (16 million colors)
    TrueColor,
}

fn detect_color_support() -> ColorSupport {
    // Respect the NO_COLOR convention (https://no-color.org/)
    if env::var("NO_COLOR").is_ok() {
        return ColorSupport::None;
    }

    let term = env::var("TERM").unwrap_or_default();
    let colorterm = env::var("COLORTERM").unwrap_or_default();

    // COLORTERM=truecolor or 24bit is the most reliable signal
    if colorterm == "truecolor" || colorterm == "24bit" {
        return ColorSupport::TrueColor;
    }

    // Many modern terminals set TERM to something with 256color
    if term.contains("256color") {
        return ColorSupport::Extended;
    }

    // TERM=dumb means no escape sequence support at all
    if term == "dumb" || term.is_empty() {
        return ColorSupport::None;
    }

    // Default assumption for recognized terminal types
    ColorSupport::Basic
}

fn main() {
    let support = detect_color_support();
    println!("Detected color support: {:?}", support);

    match support {
        ColorSupport::TrueColor => {
            // Use full RGB colors from your theme palette
            println!("Using true color theme");
        }
        ColorSupport::Extended => {
            // Map your theme colors to the closest 256-color equivalents
            println!("Using 256-color approximation");
        }
        ColorSupport::Basic => {
            // Use only the 16 standard ANSI colors
            println!("Using basic ANSI colors");
        }
        ColorSupport::None => {
            // No colors at all -- use plain text with spacing/indentation
            println!("Using plain text output");
        }
    }
}
```

::: tip Coming from Python
Python's `rich` library performs similar detection automatically. When you create a `Console()` object, Rich inspects `TERM`, `COLORTERM`, and even queries the terminal for its capabilities. In Rust, the `crossterm` crate provides some detection, but for fine-grained capability discovery you often need to check environment variables yourself as shown above. The `supports-color` crate provides a focused API for just this purpose.
:::

## True Color: The 16 Million Color Question

The jump from 256 colors to true color (24-bit, 16.7 million colors) is the single biggest visual capability difference between terminals. With true color, your TUI can use exact RGB values from design systems, match syntax highlighting themes pixel-perfectly, and render smooth gradients.

True color is specified using SGR escape sequences with a different parameter format:

```rust
fn main() {
    // 256-color mode: ESC[38;5;{n}m where n is 0-255
    print!("\x1b[38;5;208m");  // Orange in 256-color palette
    println!("This is 256-color orange");
    print!("\x1b[0m");  // Reset

    // True color mode: ESC[38;2;{r};{g};{b}m
    print!("\x1b[38;2;255;165;0m");  // Exact RGB orange
    println!("This is true color orange (255, 165, 0)");
    print!("\x1b[0m");  // Reset

    // Terminals that do not support true color will either:
    // - Ignore the sequence entirely (showing unstyled text)
    // - Approximate it using the closest 256-color entry
    // - Display garbage characters
}
```

Not every terminal supports true color. Here is a practical compatibility table:

| Terminal | True Color | GPU Rendering | Image Protocol | Kitty Keyboard |
|----------|-----------|--------------|----------------|----------------|
| Alacritty | Yes | Yes | No | No |
| Kitty | Yes | Yes | Kitty protocol | Yes |
| iTerm2 | Yes | No | iTerm2 protocol | No |
| WezTerm | Yes | Yes | Kitty + iTerm2 | Yes |
| Windows Terminal | Yes | Yes | No | No |
| macOS Terminal.app | No* | No | No | No |
| GNOME Terminal | Yes | No | No | No |
| tmux | Yes** | N/A | Passthrough | No |

*Terminal.app supports 256 colors only. **tmux requires `set -g default-terminal "tmux-256color"` in its config.

## The Multiplexer Problem

tmux and GNU Screen deserve special attention because they are extremely common in development environments, especially when your agent runs on a remote server accessed via SSH.

Multiplexers create their own virtual terminals. When your application writes escape sequences, those sequences go to tmux, which interprets them, updates its internal screen buffer, and then writes *its own* escape sequences to the actual terminal emulator. This double interpretation can cause problems:

1. **Color degradation** -- tmux may not pass through true color sequences unless configured correctly
2. **Mouse event filtering** -- tmux intercepts mouse events for its own UI (selecting panes, scrolling) and only forwards them in specific modes
3. **Escape sequence timing** -- tmux adds latency to escape sequence delivery, which can cause partial-sequence issues
4. **Alternate screen nesting** -- if your application uses the alternate screen buffer and tmux also manages screen buffers, the interaction can cause flickering or incorrect restoration

```rust
use std::env;

fn detect_multiplexer() -> Option<String> {
    // tmux sets TMUX environment variable
    if env::var("TMUX").is_ok() {
        return Some("tmux".to_string());
    }

    // GNU Screen sets STY
    if env::var("STY").is_ok() {
        return Some("screen".to_string());
    }

    // Check TERM for multiplexer indicators
    let term = env::var("TERM").unwrap_or_default();
    if term.starts_with("screen") || term.starts_with("tmux") {
        return Some(format!("detected via TERM={}", term));
    }

    None
}

fn main() {
    match detect_multiplexer() {
        Some(mux) => {
            println!("Running inside multiplexer: {}", mux);
            println!("Consider adjusting escape sequence behavior");
        }
        None => {
            println!("Running directly in terminal emulator");
        }
    }
}
```

## Terminal Size and Resize Events

Every terminal has a size measured in columns and rows. Your TUI layout depends entirely on knowing this size and responding when it changes. The `crossterm` crate provides a cross-platform way to query this:

```rust
use crossterm::terminal;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let (cols, rows) = terminal::size()?;
    println!("Terminal size: {}x{} (columns x rows)", cols, rows);

    // Under the hood, this calls the ioctl TIOCGWINSZ on Unix
    // or GetConsoleScreenBufferInfo on Windows.

    // When the user resizes their terminal window, the kernel
    // sends SIGWINCH to the foreground process group. Crossterm
    // converts this into a Resize event in its event stream.

    if cols < 80 || rows < 24 {
        println!(
            "Warning: terminal is smaller than 80x24. \
             UI may not render correctly."
        );
    }

    Ok(())
}
```

::: wild In the Wild
Claude Code adapts its output based on terminal width. Wide terminals get side-by-side diff views, while narrow terminals fall back to unified diffs. The agent also detects when it is running in a non-interactive context (piped output, CI environments) and switches to plain text output with no escape sequences. This kind of adaptive behavior is essential for a tool that developers use across many different terminal setups.
:::

## Feature Detection Queries

Some modern terminals support **query-response** protocols where your application can ask the terminal about its capabilities and receive an answer. The most common is the Device Attributes query:

```rust
use std::io::{self, Write};

fn main() {
    // Send Primary Device Attributes request
    // The terminal responds with ESC [ ? ... c
    // This is a standard VT100 query
    print!("\x1b[c");
    io::stdout().flush().unwrap();

    // In practice, reading the response requires raw mode
    // (covered in a later subchapter) because the response
    // comes through stdin as if the user typed it.

    // More modern query: request terminal name via XTVERSION
    // Supported by xterm, Kitty, WezTerm, foot
    print!("\x1b[>0q");
    io::stdout().flush().unwrap();

    println!("(Queries sent -- responses require raw mode to read)");
}
```

These queries are powerful but tricky. You must be in raw mode to read the response. You must handle the case where the terminal does not respond at all (imposing a timeout). And the response format varies between terminal emulators. For most TUI applications, environment variable detection is sufficient and much simpler.

## Choosing a Target Baseline

For your coding agent, you need to decide on a minimum capability baseline. A pragmatic choice is:

- **Color**: require 256-color support, enhance with true color when available
- **Unicode**: require UTF-8 support (virtually universal in 2024+)
- **Mouse**: optional -- support it when available, but ensure full keyboard navigation
- **Alternate screen**: require it -- this is universally supported and keeps the user's scrollback clean
- **Terminal size**: minimum 80x24, adaptive layout above that

This baseline covers all major terminal emulators on macOS, Linux, and Windows while excluding only the most limited environments (serial consoles, TERM=dumb).

## Key Takeaways

- Modern terminal emulators vary dramatically in capabilities, from macOS Terminal.app's 256-color limit to Kitty's GPU rendering with image protocols and extended keyboard support.
- Always detect capabilities at runtime using `TERM`, `COLORTERM`, and `NO_COLOR` environment variables rather than assuming a fixed feature set.
- Terminal multiplexers like tmux add a translation layer that can degrade colors, intercept mouse events, and add latency -- test your TUI both with and without tmux.
- Choose a minimum capability baseline for your TUI (256-color, UTF-8, alternate screen) and progressively enhance when richer features are available.
- Terminal size detection and resize handling via `crossterm::terminal::size()` is foundational for any responsive TUI layout.
