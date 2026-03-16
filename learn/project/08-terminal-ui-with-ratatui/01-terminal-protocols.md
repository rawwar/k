---
title: Terminal Protocols
description: Understand the foundational terminal protocols including raw mode, alternate screen, and terminal capability detection.
---

# Terminal Protocols

> **What you'll learn:**
> - How cooked mode, raw mode, and cbreak mode differ in terminal input handling
> - How the alternate screen buffer lets TUI apps render without destroying shell history
> - How to detect terminal capabilities and dimensions using crossterm

Before you can build a beautiful terminal UI, you need to understand what a terminal actually *is* and how it processes your input and output. When you type a character in your shell, it does not go directly to your program. The terminal driver -- a piece of kernel software -- intercepts it, processes it, and decides what your program sees. The mode that terminal driver operates in determines everything about how your TUI application behaves.

## How Terminal Input Works

Every Unix-like system has a terminal driver (the `tty` subsystem) sitting between the keyboard and your program. This driver can operate in several modes, each offering a different level of control to your application.

### Cooked Mode (Canonical Mode)

This is the default mode your shell uses. In cooked mode, the terminal driver:

- **Buffers input line by line** -- your program does not receive any characters until the user presses Enter.
- **Handles line editing** -- Backspace, Ctrl+W (delete word), and Ctrl+U (delete line) are processed by the driver, not your program.
- **Interprets signals** -- Ctrl+C sends SIGINT, Ctrl+Z sends SIGTSTP, Ctrl+D sends EOF.
- **Echoes characters** -- what you type appears on screen automatically.

This is perfect for a simple REPL where the user types a line and presses Enter. But it is useless for a TUI. You cannot detect individual keypresses, you cannot handle arrow keys, and you cannot prevent the terminal from echoing characters you want to handle yourself.

### Raw Mode

Raw mode is the opposite extreme. The terminal driver steps aside almost completely:

- **Every keypress is delivered immediately** -- no line buffering. Your program sees each character the instant it is typed.
- **No line editing** -- Backspace is just another byte (0x7F or 0x08). Your program must handle it.
- **No signal interpretation** -- Ctrl+C arrives as byte 0x03, not as a SIGINT signal. Your program decides what to do with it.
- **No echo** -- characters are not displayed unless your program explicitly writes them to the screen.

This is what TUI applications need. When a user presses the up arrow key, your program receives the raw escape sequence (`\x1B[A`) and can interpret it as "scroll up" or "previous history item" or anything else.

### Cbreak Mode (Rare Mode)

Cbreak mode is a middle ground: keypresses are delivered immediately (no line buffering), but signal characters like Ctrl+C still generate signals. Some applications use this, but most TUI frameworks prefer full raw mode because they want complete control over every key combination.

::: tip Coming from Python
In Python, you would use the `tty` module to enter raw mode:
```python
import tty, sys, termios

old_settings = termios.tcgetattr(sys.stdin)
try:
    tty.setraw(sys.stdin.fileno())
    ch = sys.stdin.read(1)  # reads a single keypress immediately
finally:
    termios.tcsetattr(sys.stdin, termios.TCSADRAIN, old_settings)
```
The `curses` library handles this automatically with `curses.initscr()`. In Rust, the `crossterm` crate provides the same abstraction with `enable_raw_mode()` -- and it works on Windows too, which `tty`/`termios` does not.
:::

## Entering Raw Mode with Crossterm

Crossterm is the cross-platform terminal manipulation crate that Ratatui uses as its backend. Here is how you enter and exit raw mode:

```rust
use crossterm::terminal::{enable_raw_mode, disable_raw_mode};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Enter raw mode -- terminal driver stops buffering and echoing
    enable_raw_mode()?;

    // Your TUI runs here...
    println!("Raw mode is active. Press 'q' to quit.");

    // Read a single byte from stdin
    use crossterm::event::{self, Event, KeyCode};
    loop {
        if let Event::Key(key_event) = event::read()? {
            if key_event.code == KeyCode::Char('q') {
                break;
            }
        }
    }

    // ALWAYS restore the terminal before exiting
    disable_raw_mode()?;
    Ok(())
}
```

The critical rule: **always restore the terminal**. If your program panics or exits without calling `disable_raw_mode()`, the user's terminal is left in raw mode and becomes unusable. They will have to type `reset` blindly or close the terminal window. You will see how to handle this safely with RAII patterns in the Ratatui overview subchapter.

## The Alternate Screen Buffer

Terminals actually have two screen buffers:

1. **Main screen** -- where your shell history, previous commands, and output live.
2. **Alternate screen** -- a completely separate drawing surface with no scroll history.

When you run `vim`, `htop`, or `less`, they switch to the alternate screen. When they exit, the terminal switches back to the main screen and your shell history is exactly where you left it. This is what makes TUI apps feel like they "take over" the terminal without destroying anything.

```rust
use crossterm::{
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen},
};
use std::io::stdout;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut stdout = stdout();

    // Switch to the alternate screen
    execute!(stdout, EnterAlternateScreen)?;

    // Everything you draw here is on a separate buffer.
    // The user's shell history is preserved underneath.

    // Switch back to the main screen
    execute!(stdout, LeaveAlternateScreen)?;

    // Shell history is restored as if nothing happened.
    Ok(())
}
```

For your coding agent, the alternate screen is essential. Users expect to see their shell history when they exit the agent, not a screen full of conversation fragments.

## Terminal Capability Detection

Not all terminals are created equal. Some support 256 colors, others support true color (16 million colors), and some are limited to the basic 16 ANSI colors. Your application should detect and adapt to these capabilities.

### Detecting Terminal Size

The terminal's width and height in character cells determines your layout. Crossterm makes this straightforward:

```rust
use crossterm::terminal::size;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let (cols, rows) = size()?;
    println!("Terminal is {} columns x {} rows", cols, rows);

    // Ratatui uses this internally, but you may need it
    // for layout decisions before the TUI starts
    if cols < 80 || rows < 24 {
        eprintln!("Warning: terminal is smaller than 80x24. The UI may not display correctly.");
    }

    Ok(())
}
```

### Detecting Color Support

Color detection is trickier. There is no universal standard. In practice, applications check environment variables:

```rust
fn detect_color_support() -> ColorSupport {
    // COLORTERM is the most reliable signal for truecolor
    if let Ok(ct) = std::env::var("COLORTERM") {
        if ct == "truecolor" || ct == "24bit" {
            return ColorSupport::TrueColor;
        }
    }

    // TERM can hint at 256-color support
    if let Ok(term) = std::env::var("TERM") {
        if term.contains("256color") {
            return ColorSupport::Color256;
        }
    }

    ColorSupport::Basic16
}

enum ColorSupport {
    Basic16,
    Color256,
    TrueColor,
}
```

::: tip In the Wild
Claude Code detects terminal capabilities at startup and adjusts its rendering accordingly. On terminals with limited color support, it falls back to a simpler color palette. OpenCode uses a similar approach, checking `COLORTERM` and `TERM` environment variables to determine whether to use its full theme or a reduced-color variant.
:::

## The Terminal Setup/Teardown Pattern

Every TUI application follows the same lifecycle:

1. **Save terminal state** (enter raw mode, switch to alternate screen)
2. **Run the application** (event loop, rendering)
3. **Restore terminal state** (leave alternate screen, disable raw mode)

Step 3 must happen even if the application panics. Here is the robust pattern:

```rust
use crossterm::{
    execute,
    terminal::{
        enable_raw_mode, disable_raw_mode,
        EnterAlternateScreen, LeaveAlternateScreen,
    },
};
use std::io::stdout;

fn setup_terminal() -> Result<(), Box<dyn std::error::Error>> {
    enable_raw_mode()?;
    execute!(stdout(), EnterAlternateScreen)?;
    Ok(())
}

fn restore_terminal() -> Result<(), Box<dyn std::error::Error>> {
    disable_raw_mode()?;
    execute!(stdout(), LeaveAlternateScreen)?;
    Ok(())
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    setup_terminal()?;

    // Install a panic hook that restores the terminal
    let original_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |panic_info| {
        let _ = restore_terminal();
        original_hook(panic_info);
    }));

    // Run the TUI application
    let result = run_app();

    // Normal cleanup
    restore_terminal()?;
    result
}

fn run_app() -> Result<(), Box<dyn std::error::Error>> {
    // Your TUI logic goes here
    Ok(())
}
```

The panic hook ensures that even if your code panics deep inside a widget rendering function, the terminal is restored to a usable state before the panic message prints.

## Key Takeaways

- **Cooked mode** buffers input by line and handles editing; **raw mode** delivers every keypress immediately and gives your application full control -- TUI apps require raw mode.
- **The alternate screen buffer** is a separate drawing surface that preserves the user's shell history; always use it for fullscreen TUI applications.
- **Terminal capability detection** lets your application adapt its color palette and layout to the user's environment.
- **Always restore terminal state** on exit, including on panic -- the setup/teardown pattern with a panic hook is the standard approach.
- **Crossterm** provides cross-platform abstractions for raw mode, alternate screen, terminal size detection, and event reading that work on macOS, Linux, and Windows.
