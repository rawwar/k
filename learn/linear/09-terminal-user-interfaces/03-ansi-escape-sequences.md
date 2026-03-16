---
title: ANSI Escape Sequences
description: The ANSI escape code system for controlling cursor position, text color, styling, screen clearing, and scrolling regions in terminal applications.
---

# ANSI Escape Sequences

> **What you'll learn:**
> - The CSI (Control Sequence Introducer) format and how to construct escape sequences for cursor movement, colors, and text attributes
> - SGR (Select Graphic Rendition) parameters for foreground/background colors including 256-color and 24-bit true color modes
> - Screen manipulation sequences: alternate screen buffer, scrolling regions, and terminal title setting

ANSI escape sequences are the low-level protocol that every terminal UI is built on. When Ratatui draws a colored border or moves the cursor to render a widget, it generates escape sequences under the hood. Understanding them directly gives you the power to debug rendering issues, write efficient output code, and know exactly what your framework is doing on your behalf.

## The CSI Format

Most escape sequences you will encounter follow the **Control Sequence Introducer (CSI)** format:

```
ESC [ <parameters> <final byte>
```

The ESC character is byte `0x1B` (decimal 27). The `[` is literal. Parameters are semicolon-separated decimal numbers. The final byte is a single character that identifies the command.

Here is a concrete example dissected:

```
\x1b[31m
  ^  ^^
  |  |+-- Final byte: 'm' = SGR (Select Graphic Rendition)
  |  +--- Parameter: 31 = red foreground
  +------ ESC [  = CSI introducer
```

Let's write this from Rust and see it in action:

```rust
use std::io::{self, Write};

fn main() {
    let stdout = io::stdout();
    let mut out = stdout.lock();

    // Direct escape sequence writing
    // CSI 31 m = set foreground to red
    write!(out, "\x1b[31mThis text is red\x1b[0m\n").unwrap();

    // CSI 1 m = bold, CSI 32 m = green foreground
    write!(out, "\x1b[1;32mBold green text\x1b[0m\n").unwrap();

    // CSI 0 m = reset all attributes
    // Always reset after styling, or the color bleeds into subsequent output
    write!(out, "Back to normal\n").unwrap();

    out.flush().unwrap();
}
```

The `\x1b[0m` reset sequence is critical. If your program crashes or exits without resetting terminal attributes, the user's shell prompt will inherit whatever style was last set. This is one reason TUI frameworks install panic hooks that clean up terminal state.

## Cursor Movement

Cursor movement sequences let you position text anywhere on the screen. This is the foundation of all TUI rendering -- instead of printing lines sequentially, you jump the cursor to specific coordinates and write characters there.

| Sequence | Action | Example |
|----------|--------|---------|
| `\x1b[{n}A` | Move cursor up n rows | `\x1b[3A` = up 3 |
| `\x1b[{n}B` | Move cursor down n rows | `\x1b[1B` = down 1 |
| `\x1b[{n}C` | Move cursor right n columns | `\x1b[10C` = right 10 |
| `\x1b[{n}D` | Move cursor left n columns | `\x1b[2D` = left 2 |
| `\x1b[{row};{col}H` | Move cursor to absolute position | `\x1b[1;1H` = top-left |
| `\x1b[s` | Save cursor position | |
| `\x1b[u` | Restore saved cursor position | |

```rust
use std::io::{self, Write};

fn main() {
    let stdout = io::stdout();
    let mut out = stdout.lock();

    // Move to row 5, column 10 and write text
    write!(out, "\x1b[5;10HHello at (5, 10)").unwrap();

    // Move to row 7, column 10
    write!(out, "\x1b[7;10HHello at (7, 10)").unwrap();

    // Save position, write elsewhere, then restore
    write!(out, "\x1b[s").unwrap();           // Save
    write!(out, "\x1b[1;1HTop-left corner").unwrap();  // Jump to (1,1)
    write!(out, "\x1b[u").unwrap();           // Restore to (7, 10+len)

    // Move cursor to bottom so output does not overlap
    write!(out, "\x1b[10;1H\n").unwrap();

    out.flush().unwrap();
}
```

Note that terminal coordinates are **1-indexed**: the top-left cell is row 1, column 1. This is a historical convention from the VT100 era, and it catches many developers off guard since arrays in both Python and Rust are 0-indexed.

## SGR: Colors and Text Attributes

The **Select Graphic Rendition** (SGR) command (`m` final byte) is the most complex and most-used escape sequence. It controls text color, background color, bold, italic, underline, and other visual attributes.

### Basic 16 Colors

The original 8 colors, each available in normal and bright variants:

| Code | Color | Bright Code | Bright Color |
|------|-------|-------------|--------------|
| 30 | Black | 90 | Bright Black (Gray) |
| 31 | Red | 91 | Bright Red |
| 32 | Green | 92 | Bright Green |
| 33 | Yellow | 93 | Bright Yellow |
| 34 | Blue | 94 | Bright Blue |
| 35 | Magenta | 95 | Bright Magenta |
| 36 | Cyan | 96 | Bright Cyan |
| 37 | White | 97 | Bright White |

Background colors use codes 40-47 and 100-107.

### 256-Color Mode

The extended palette uses the format `38;5;{n}` for foreground and `48;5;{n}` for background:

- 0-7: standard colors (same as 30-37)
- 8-15: bright colors (same as 90-97)
- 16-231: a 6x6x6 color cube
- 232-255: a 24-step grayscale ramp

### True Color (24-bit)

Full RGB colors use `38;2;{r};{g};{b}` for foreground and `48;2;{r};{g};{b}` for background:

```rust
use std::io::{self, Write};

fn main() {
    let stdout = io::stdout();
    let mut out = stdout.lock();

    // Basic ANSI colors
    write!(out, "\x1b[31mRed \x1b[32mGreen \x1b[34mBlue\x1b[0m\n").unwrap();

    // 256-color palette
    write!(out, "\x1b[38;5;208mOrange (256-color #208)\x1b[0m\n").unwrap();

    // True color RGB
    write!(out, "\x1b[38;2;137;180;250mCatppuccin Blue (137,180,250)\x1b[0m\n").unwrap();

    // Text attributes
    write!(out, "\x1b[1mBold\x1b[0m ").unwrap();
    write!(out, "\x1b[3mItalic\x1b[0m ").unwrap();
    write!(out, "\x1b[4mUnderline\x1b[0m ").unwrap();
    write!(out, "\x1b[9mStrikethrough\x1b[0m\n").unwrap();

    // Combine multiple attributes: bold + red foreground + white background
    write!(out, "\x1b[1;31;47mBold red on white\x1b[0m\n").unwrap();

    // Print a color gradient using true color
    write!(out, "Gradient: ").unwrap();
    for i in 0..40 {
        let r = (i * 6) as u8;
        let b = (255 - i * 6) as u8;
        write!(out, "\x1b[38;2;{r};0;{b}m\u{2588}").unwrap();
    }
    write!(out, "\x1b[0m\n").unwrap();

    out.flush().unwrap();
}
```

::: python Coming from Python
Python's `rich` library abstracts away escape sequences entirely. You write `console.print("[bold red]Error[/]")` and Rich generates the correct escape sequences based on detected terminal capabilities. In Rust, Ratatui provides similar abstraction through its `Style` type, but understanding the raw sequences helps you debug issues when a style does not render as expected. The Rust crate `owo-colors` provides a lightweight API similar to Rich's markup for simple colored output without a full TUI framework.
:::

## Screen Manipulation

Beyond cursor movement and styling, several escape sequences control the screen itself:

### Clearing

| Sequence | Action |
|----------|--------|
| `\x1b[2J` | Clear entire screen |
| `\x1b[0J` | Clear from cursor to end of screen |
| `\x1b[1J` | Clear from start of screen to cursor |
| `\x1b[2K` | Clear entire current line |
| `\x1b[0K` | Clear from cursor to end of line |

### Alternate Screen Buffer

The alternate screen buffer is one of the most important sequences for TUI applications. It switches to a separate screen, preserving the user's existing terminal content (command history, scrollback). When your application exits, switching back restores everything.

```rust
use std::io::{self, Write};
use std::thread;
use std::time::Duration;

fn main() {
    let stdout = io::stdout();
    let mut out = stdout.lock();

    // Enter alternate screen buffer
    write!(out, "\x1b[?1049h").unwrap();
    // Clear the alternate screen
    write!(out, "\x1b[2J").unwrap();
    // Move cursor to top-left
    write!(out, "\x1b[1;1H").unwrap();

    write!(out, "You are now on the alternate screen!\n").unwrap();
    write!(out, "Your scrollback is preserved behind this.\n").unwrap();
    write!(out, "Returning in 3 seconds...\n").unwrap();
    out.flush().unwrap();

    thread::sleep(Duration::from_secs(3));

    // Leave alternate screen buffer -- original content is restored
    write!(out, "\x1b[?1049l").unwrap();
    out.flush().unwrap();

    println!("Back to normal! Your scrollback is intact.");
}
```

### Scrolling Regions

You can define a region of the screen that scrolls independently. This is useful for TUI layouts where a status bar stays fixed at the top while content scrolls below:

```rust
use std::io::{self, Write};

fn main() {
    let stdout = io::stdout();
    let mut out = stdout.lock();

    // Set scrolling region to rows 3 through 20
    // Rows 1-2 and 21+ will not scroll
    write!(out, "\x1b[3;20r").unwrap();

    // Move to row 1 for a fixed header
    write!(out, "\x1b[1;1H\x1b[7m STATUS BAR - FIXED \x1b[0m").unwrap();

    // Move into the scrolling region and write content
    write!(out, "\x1b[3;1H").unwrap();
    for i in 1..=30 {
        write!(out, "Scrolling line {}\n", i).unwrap();
    }

    // Reset scrolling region to full screen
    write!(out, "\x1b[r").unwrap();

    out.flush().unwrap();
}
```

## The crossterm Abstraction

While understanding raw escape sequences is valuable, in practice you will use a library like `crossterm` to generate them. crossterm provides a type-safe Rust API and handles platform differences (Windows uses a different console API for some operations):

```rust
use crossterm::{
    cursor,
    style::{self, Color, Stylize},
    terminal::{self, ClearType},
    ExecutableCommand,
};
use std::io::{self, stdout};

fn main() -> io::Result<()> {
    let mut stdout = stdout();

    // These crossterm calls generate the same escape sequences
    // we wrote manually above, but with a type-safe API
    stdout.execute(cursor::MoveTo(10, 5))?;
    stdout.execute(style::PrintStyledContent(
        "Hello from crossterm!".with(Color::Rgb { r: 137, g: 180, b: 250 })
    ))?;

    stdout.execute(cursor::MoveTo(0, 7))?;
    stdout.execute(terminal::Clear(ClearType::CurrentLine))?;

    // crossterm handles the platform-specific details:
    // - On Unix: writes ANSI escape sequences
    // - On Windows: uses Windows Console API or Virtual Terminal Sequences

    Ok(())
}
```

::: wild In the Wild
OpenCode, a terminal-based coding agent written in Go, uses the `charmbracelet/lipgloss` library which generates ANSI escape sequences internally. Both Rust's crossterm and Go's lipgloss ultimately produce the same byte sequences -- the difference is in the API ergonomics and type safety. When debugging rendering issues across agents, you can intercept the raw escape sequences with `cat -v` or `xxd` to see exactly what bytes are being written, regardless of which framework generated them.
:::

## Terminal Title and Notifications

Two more useful sequences: setting the terminal window title and triggering an audible/visual bell:

```rust
use std::io::{self, Write};

fn main() {
    let stdout = io::stdout();
    let mut out = stdout.lock();

    // Set terminal window title using OSC (Operating System Command)
    // Format: ESC ] 2 ; <title> BEL
    write!(out, "\x1b]2;My Coding Agent v1.0\x07").unwrap();

    // Trigger a bell notification (useful when a long operation completes)
    // BEL character = 0x07
    // Most terminals will flash or make a sound
    write!(out, "\x07").unwrap();

    out.flush().unwrap();
    println!("Terminal title set and bell triggered.");
}
```

## Key Takeaways

- ANSI escape sequences follow the CSI format (`ESC [` + parameters + final byte) and are the foundation of all terminal rendering, from cursor movement to color to screen clearing.
- SGR sequences (`m` final byte) control text styling across three color depths: 16 basic colors (codes 30-37, 90-97), 256-color palette (`38;5;n`), and 24-bit true color (`38;2;r;g;b`).
- The alternate screen buffer (`\x1b[?1049h` / `\x1b[?1049l`) is essential for TUI applications -- it preserves the user's scrollback and provides a clean canvas.
- Always reset attributes with `\x1b[0m` after styling to prevent color bleed, and install panic hooks that restore terminal state on crash.
- In practice, use the `crossterm` crate for type-safe, cross-platform escape sequence generation rather than writing raw sequences, but understand the raw format for debugging.
