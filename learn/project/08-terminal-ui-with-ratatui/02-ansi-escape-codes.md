---
title: ANSI Escape Codes
description: Learn the ANSI escape sequences used for cursor movement, color, styling, and screen manipulation in terminal applications.
---

# ANSI Escape Codes

> **What you'll learn:**
> - How CSI (Control Sequence Introducer) escape codes control cursor position and screen clearing
> - How SGR (Select Graphic Rendition) codes apply colors, bold, underline, and other text styles
> - How 256-color and truecolor (24-bit) modes extend beyond the basic 16 ANSI colors

When Ratatui draws a colored, formatted interface on your screen, it is not using any special API. Under the hood, it writes bytes to stdout -- ordinary text interspersed with special byte sequences that the terminal interprets as commands. These are ANSI escape codes, and understanding them gives you insight into what your TUI framework is actually doing.

## The Escape Character

Every ANSI escape sequence starts with the ESC character, which is byte `0x1B` (decimal 27). In string literals, you write it as `\x1B` or `\u{001B}`. The ESC character tells the terminal "the next bytes are a command, not text to display."

The most common sequences use the **CSI** (Control Sequence Introducer) format, which is ESC followed by `[`:

```
ESC [ <parameters> <command letter>
\x1B[          ...               X
```

For example, `\x1B[2J` means "clear the entire screen" and `\x1B[10;5H` means "move the cursor to row 10, column 5."

## Cursor Movement

Cursor movement codes let you position text anywhere on screen. Without them, you could only write text sequentially from left to right, top to bottom.

```rust
use std::io::{self, Write};

fn main() -> io::Result<()> {
    let mut stdout = io::stdout();

    // Clear the entire screen
    write!(stdout, "\x1B[2J")?;

    // Move cursor to row 1, column 1 (top-left corner)
    write!(stdout, "\x1B[1;1H")?;
    write!(stdout, "Top left corner")?;

    // Move cursor to row 10, column 20
    write!(stdout, "\x1B[10;20H")?;
    write!(stdout, "Middle of the screen")?;

    // Move cursor up 3 rows from current position
    write!(stdout, "\x1B[3A")?;
    write!(stdout, "Three rows up")?;

    // Move cursor down 1 row
    write!(stdout, "\x1B[1B")?;
    // Move cursor right 5 columns
    write!(stdout, "\x1B[5C")?;
    write!(stdout, "Offset text")?;

    stdout.flush()?;
    Ok(())
}
```

Here are the most important cursor movement codes:

| Code | Meaning | Example |
|------|---------|---------|
| `\x1B[H` | Move to home (1,1) | `\x1B[H` |
| `\x1B[{r};{c}H` | Move to row r, column c | `\x1B[5;10H` |
| `\x1B[{n}A` | Move up n rows | `\x1B[3A` |
| `\x1B[{n}B` | Move down n rows | `\x1B[1B` |
| `\x1B[{n}C` | Move right n columns | `\x1B[5C` |
| `\x1B[{n}D` | Move left n columns | `\x1B[2D` |
| `\x1B[2J` | Clear entire screen | `\x1B[2J` |
| `\x1B[K` | Clear from cursor to end of line | `\x1B[K` |

## SGR: Colors and Text Styling

SGR (Select Graphic Rendition) codes control how text looks. They use the format `\x1B[{parameters}m`, where `m` is the command letter for "set graphic rendition."

### Text Attributes

```rust
use std::io::{self, Write};

fn main() -> io::Result<()> {
    let mut stdout = io::stdout();

    // Bold text
    write!(stdout, "\x1B[1mThis is bold\x1B[0m\n")?;

    // Dim (faint) text
    write!(stdout, "\x1B[2mThis is dim\x1B[0m\n")?;

    // Italic text
    write!(stdout, "\x1B[3mThis is italic\x1B[0m\n")?;

    // Underlined text
    write!(stdout, "\x1B[4mThis is underlined\x1B[0m\n")?;

    // Strikethrough text
    write!(stdout, "\x1B[9mThis is strikethrough\x1B[0m\n")?;

    // Combine multiple attributes: bold + underline + red
    write!(stdout, "\x1B[1;4;31mBold, underlined, and red\x1B[0m\n")?;

    stdout.flush()?;
    Ok(())
}
```

The `\x1B[0m` at the end of each line is the **reset code** -- it turns off all styling. Forgetting to reset is a common bug that causes all subsequent terminal output to inherit the wrong style.

### The Basic 16 Colors

The original ANSI standard defines 8 colors, each with a normal and bright variant:

| Color | Foreground | Background | Bright FG | Bright BG |
|-------|-----------|------------|-----------|-----------|
| Black | 30 | 40 | 90 | 100 |
| Red | 31 | 41 | 91 | 101 |
| Green | 32 | 42 | 92 | 102 |
| Yellow | 33 | 43 | 93 | 103 |
| Blue | 34 | 44 | 94 | 104 |
| Magenta | 35 | 45 | 95 | 105 |
| Cyan | 36 | 46 | 96 | 106 |
| White | 37 | 47 | 97 | 107 |

```rust
use std::io::{self, Write};

fn main() -> io::Result<()> {
    let mut stdout = io::stdout();

    // Red foreground text
    write!(stdout, "\x1B[31mRed text\x1B[0m\n")?;

    // Green text on blue background
    write!(stdout, "\x1B[32;44mGreen on blue\x1B[0m\n")?;

    // Bright yellow (bold yellow on most terminals)
    write!(stdout, "\x1B[93mBright yellow\x1B[0m\n")?;

    stdout.flush()?;
    Ok(())
}
```

### 256-Color Mode

The basic 16 colors are configurable by the user's terminal theme, which means "red" might look different on every system. The 256-color mode adds 240 fixed colors with predictable appearance:

```rust
use std::io::{self, Write};

fn main() -> io::Result<()> {
    let mut stdout = io::stdout();

    // 256-color foreground: \x1B[38;5;{color_number}m
    // 256-color background: \x1B[48;5;{color_number}m

    // Colors 0-7: standard colors
    // Colors 8-15: bright colors
    // Colors 16-231: 6x6x6 color cube
    // Colors 232-255: grayscale ramp

    // Print the 216-color cube (colors 16-231)
    for i in 16..232 {
        write!(stdout, "\x1B[48;5;{i}m  ")?;
        if (i - 15) % 36 == 0 {
            write!(stdout, "\x1B[0m\n")?;
        }
    }
    write!(stdout, "\x1B[0m\n")?;

    // A specific color: orange-ish (color 208)
    write!(stdout, "\x1B[38;5;208mOrange text\x1B[0m\n")?;

    stdout.flush()?;
    Ok(())
}
```

### Truecolor (24-bit Color)

Modern terminals support full RGB colors with 16.7 million possible values:

```rust
use std::io::{self, Write};

fn main() -> io::Result<()> {
    let mut stdout = io::stdout();

    // Truecolor foreground: \x1B[38;2;{r};{g};{b}m
    // Truecolor background: \x1B[48;2;{r};{g};{b}m

    // Catppuccin Mocha Blue (#89b4fa)
    write!(stdout, "\x1B[38;2;137;180;250mCatppuccin Blue\x1B[0m\n")?;

    // Catppuccin Mocha Green (#a6e3a1) on Mocha Base (#1e1e2e)
    write!(
        stdout,
        "\x1B[38;2;166;227;161;48;2;30;30;46mGreen on dark background\x1B[0m\n"
    )?;

    // Draw a simple gradient
    for i in 0..80 {
        let r = (i as f64 / 80.0 * 255.0) as u8;
        let b = 255 - r;
        write!(stdout, "\x1B[48;2;{r};0;{b}m ")?;
    }
    write!(stdout, "\x1B[0m\n")?;

    stdout.flush()?;
    Ok(())
}
```

::: tip Coming from Python
In Python, libraries like `colorama` and `rich` handle escape codes for you. Rich in particular provides a markup syntax:
```python
from rich import print
print("[bold red]Error:[/bold red] something went wrong")
```
In Rust, Ratatui provides the same abstraction through its `Style` type -- you never write raw escape codes in your application. But understanding what is happening under the hood helps you debug rendering issues when text shows up with garbled `^[[31m` prefixes instead of colors.
:::

## Why Ratatui Abstracts This Away

You will rarely write raw escape codes in your agent. Ratatui converts its high-level `Style` objects into the correct escape sequences for the terminal backend. A `Style::new().fg(Color::Red).bold()` becomes `\x1B[1;31m` when written to the screen.

However, understanding escape codes is valuable for three reasons:

1. **Debugging** -- when your terminal output looks garbled, you can inspect the raw bytes and understand what went wrong.
2. **Performance** -- Ratatui's diffing engine only writes escape codes for cells that changed between frames. Knowing what those codes are helps you understand the performance implications of your layout decisions.
3. **Custom rendering** -- if you ever need to render something outside Ratatui's widget system (like raw output from a subprocess), you need to know how escape codes work to sanitize or pass them through correctly.

Here is a quick example showing how Ratatui's `Style` maps to the escape codes you just learned:

```rust
use ratatui::style::{Color, Modifier, Style};

fn describe_style(style: Style) -> String {
    // This is conceptual -- Ratatui handles the actual encoding internally.
    // A style like this:
    let example = Style::default()
        .fg(Color::Rgb(137, 180, 250))  // Catppuccin Blue
        .bg(Color::Rgb(30, 30, 46))     // Catppuccin Base
        .add_modifier(Modifier::BOLD);

    // Would generate escape codes roughly equivalent to:
    // \x1B[1;38;2;137;180;250;48;2;30;30;46m
    //  ^bold  ^fg truecolor      ^bg truecolor

    format!("Style produces the appropriate ANSI sequences for the terminal")
}
```

::: tip In the Wild
Production coding agents must handle a wide range of terminal capabilities. Claude Code detects whether the terminal supports truecolor, 256 colors, or only basic 16 colors, and adjusts its escape code output accordingly. OpenCode takes a similar approach, using truecolor when available but falling back gracefully. The ANSI color detection you learned in the previous subchapter feeds directly into this decision -- your agent should never send truecolor escape codes to a terminal that cannot interpret them.
:::

## Screen Clearing and Redrawing

TUI applications need to redraw the screen frequently. There are two strategies:

**Full clear and redraw** -- write `\x1B[2J` (clear screen) then `\x1B[H` (cursor home) then draw everything. This is simple but causes visible flicker.

**Differential update** -- only update the cells that changed since the last frame. This is what Ratatui does. It maintains two buffers (current and previous), diffs them, and writes only the changed cells with precise cursor positioning. This eliminates flicker and minimizes the bytes written to stdout.

```rust
// Ratatui handles this internally. Each frame:
// 1. You draw widgets into a fresh buffer
// 2. Ratatui diffs the new buffer against the previous frame
// 3. Only changed cells are written to the terminal
// 4. The new buffer becomes the "previous" for the next frame

// This is why Ratatui is fast -- a frame that changes 3 cells
// out of 10,000 only writes ~30 bytes to stdout instead of
// redrawing the entire screen.
```

## Key Takeaways

- **ANSI escape codes** are byte sequences starting with `\x1B[` that control cursor position, text color, and styling -- every terminal UI framework uses them under the hood.
- **SGR codes** (`\x1B[...m`) handle all visual styling: basic 16 colors, 256-color mode (`38;5;N`), and truecolor (`38;2;R;G;B`) for foreground and background.
- **Always reset** with `\x1B[0m` after styled text to prevent style leakage into subsequent output.
- **Ratatui abstracts escape codes** into `Style` objects, but understanding the underlying codes helps you debug rendering issues and appreciate the framework's performance optimizations.
- **Differential rendering** (updating only changed cells) is far more efficient than full-screen redraws and is the strategy Ratatui uses by default.
