---
title: Raw vs Cooked Mode
description: Terminal input modes explained — how cooked mode provides line editing while raw mode gives character-by-character control needed for interactive TUI applications.
---

# Raw vs Cooked Mode

> **What you'll learn:**
> - How cooked (canonical) mode buffers input until newline and provides built-in line editing via the kernel terminal driver
> - Why TUI applications must enter raw mode to receive individual keystrokes, control sequences, and mouse events
> - The crossterm and termion APIs for switching terminal modes and the cleanup responsibilities when exiting raw mode

Every TUI application must grapple with a fundamental question: who processes the user's keystrokes? In the default terminal mode, the kernel handles line editing, backspace, and buffering. In raw mode, your application receives every single keystroke the instant it happens. Understanding the difference -- and the responsibilities that come with raw mode -- is essential before building any interactive terminal interface.

## Cooked Mode: The Default

When you open a terminal and type a command, you are in **cooked mode** (also called **canonical mode**). The kernel's terminal driver handles:

- **Line buffering** -- keystrokes are collected in a buffer until you press Enter. Your program's `read()` call blocks until a complete line is available.
- **Line editing** -- backspace deletes the previous character, Ctrl+W deletes the previous word, Ctrl+U clears the line. These are processed by the kernel, not your program.
- **Echo** -- characters you type are automatically displayed on screen. Your program does not need to print them.
- **Signal generation** -- Ctrl+C sends SIGINT, Ctrl+Z sends SIGTSTP, Ctrl+\ sends SIGQUIT. These are intercepted by the terminal driver and never reach your program as input.

This is perfectly adequate for command-line tools that read line-by-line input. But for a TUI application, cooked mode is unusable. You cannot respond to individual keystrokes. You cannot detect arrow keys, Ctrl+key combinations, or mouse events. You cannot update the screen while waiting for input.

```rust
use std::io::{self, BufRead};

fn main() {
    println!("Cooked mode demo: type something and press Enter");
    println!("(Backspace and Ctrl+W work thanks to the kernel driver)");
    println!();

    let stdin = io::stdin();
    let mut line = String::new();

    // This blocks until the user presses Enter.
    // The kernel has already handled backspace, Ctrl+W, etc.
    // Your program only sees the finished line.
    stdin.lock().read_line(&mut line).unwrap();

    println!("You entered: {:?}", line.trim());
    println!("Length: {} characters", line.trim().len());

    // Notice: if the user pressed backspace while typing,
    // those deleted characters are NOT in the string.
    // The kernel consumed them.
}
```

## Raw Mode: Full Control

**Raw mode** (also called **non-canonical mode**) disables all of the kernel's input processing. Your application receives:

- Every individual keystroke, immediately, without waiting for Enter
- Escape sequences from special keys (arrows, function keys, Home, End)
- Mouse events (if mouse reporting is enabled)
- Paste events (if bracketed paste mode is enabled)
- No echo -- you must display characters yourself
- No signal generation from Ctrl+C -- you receive it as a regular keystroke

This is what Ratatui, vim, and every interactive TUI application uses. The trade-off is that you take on full responsibility for input handling, display, and cleanup.

Here is how to enter and exit raw mode using crossterm:

```rust
use crossterm::{
    event::{self, Event, KeyCode, KeyEvent, KeyModifiers},
    terminal::{self, EnterAlternateScreen, LeaveAlternateScreen},
    ExecutableCommand,
};
use std::io::{self, stdout, Write};

fn main() -> io::Result<()> {
    // Enter raw mode -- disables line buffering, echo, and signal handling
    terminal::enable_raw_mode()?;

    // Enter alternate screen -- gives us a clean canvas
    stdout().execute(EnterAlternateScreen)?;

    // Print instructions
    let mut out = stdout();
    write!(out, "\x1b[1;1HRaw mode active! Press keys to see events.\r\n")?;
    write!(out, "Press 'q' to quit.\r\n")?;
    write!(out, "\r\n")?;
    out.flush()?;

    let mut row = 4;

    loop {
        // event::read() blocks until an event arrives.
        // In raw mode, each keystroke is an immediate event.
        if let Ok(event) = event::read() {
            match &event {
                Event::Key(KeyEvent {
                    code: KeyCode::Char('q'),
                    modifiers: KeyModifiers::NONE,
                    ..
                }) => break,

                Event::Key(KeyEvent {
                    code: KeyCode::Char('c'),
                    modifiers: KeyModifiers::CONTROL,
                    ..
                }) => break, // Handle Ctrl+C ourselves since kernel won't

                _ => {}
            }

            // Display the raw event
            write!(out, "\x1b[{row};1HEvent: {:?}\x1b[0K\r\n", event)?;
            out.flush()?;
            row += 1;

            if row > 20 {
                row = 4; // Wrap around
            }
        }
    }

    // CRITICAL: Always clean up terminal state
    stdout().execute(LeaveAlternateScreen)?;
    terminal::disable_raw_mode()?;

    println!("Terminal restored to normal mode.");
    Ok(())
}
```

Notice the `\r\n` instead of just `\n`. In raw mode, the terminal driver does not translate `\n` (line feed) into `\r\n` (carriage return + line feed). If you write only `\n`, the cursor moves down one row but stays in the same column, producing a staircase effect. You must explicitly include `\r` to move the cursor back to column 1.

::: python Coming from Python
Python's `curses` module enters raw mode automatically when you call `curses.wrapper()`. The wrapper also handles cleanup when your program exits. Python's `textual` framework (from the Rich family) does the same thing internally. In Rust, `crossterm::terminal::enable_raw_mode()` is the equivalent, but you must manage cleanup yourself -- either with explicit calls or by using RAII patterns as shown below.
:::

## The termios Interface

Under the hood on Unix systems, raw mode is configured through the `termios` struct. This is the kernel interface for terminal settings. crossterm abstracts this, but knowing the underlying mechanism helps with debugging:

```rust
// This shows what crossterm does internally on Unix.
// You would NOT normally write this directly -- use crossterm instead.
// Shown here for educational purposes.

#[cfg(unix)]
fn explain_termios_flags() {
    println!("Key termios flags affected by raw mode:");
    println!();
    println!("ICANON (canonical mode):");
    println!("  ON  = cooked mode: line buffering, line editing");
    println!("  OFF = raw mode: character-by-character delivery");
    println!();
    println!("ECHO:");
    println!("  ON  = typed characters appear on screen automatically");
    println!("  OFF = application must echo characters itself");
    println!();
    println!("ISIG (signal generation):");
    println!("  ON  = Ctrl+C -> SIGINT, Ctrl+Z -> SIGTSTP");
    println!("  OFF = Ctrl+C delivered as byte 0x03 to application");
    println!();
    println!("IEXTEN (extended input processing):");
    println!("  ON  = Ctrl+V quotes next character, Ctrl+O discards");
    println!("  OFF = these keys delivered as raw bytes");
    println!();
    println!("OPOST (output processing):");
    println!("  ON  = \\n translated to \\r\\n on output");
    println!("  OFF = \\n written as-is (causes staircase effect)");
}

fn main() {
    #[cfg(unix)]
    explain_termios_flags();

    #[cfg(not(unix))]
    println!("On Windows, terminal mode is configured via SetConsoleMode.");
}
```

The key flags that raw mode disables are:

- **ICANON** -- disables canonical (line-buffered) mode
- **ECHO** -- disables automatic character echoing
- **ISIG** -- disables signal generation from Ctrl+C/Ctrl+Z
- **IEXTEN** -- disables extended input processing
- **OPOST** -- disables output post-processing (newline translation)

## Safe Cleanup with RAII

The most dangerous aspect of raw mode is forgetting to restore the terminal when your program exits. If your program panics, gets killed, or exits through an unexpected path while in raw mode, the user's terminal will be left in a broken state -- no echo, no line editing, no Ctrl+C.

The idiomatic Rust solution is an RAII guard that restores terminal state when dropped:

```rust
use crossterm::{
    terminal::{self, EnterAlternateScreen, LeaveAlternateScreen},
    ExecutableCommand,
};
use std::io::{self, stdout};

/// RAII guard that restores terminal state on drop
struct TerminalGuard;

impl TerminalGuard {
    fn new() -> io::Result<Self> {
        terminal::enable_raw_mode()?;
        stdout().execute(EnterAlternateScreen)?;
        Ok(TerminalGuard)
    }
}

impl Drop for TerminalGuard {
    fn drop(&mut self) {
        // Best-effort cleanup -- we cannot propagate errors from Drop
        let _ = stdout().execute(LeaveAlternateScreen);
        let _ = terminal::disable_raw_mode();
    }
}

fn run_app() -> io::Result<()> {
    // The guard ensures cleanup even on panic or early return
    let _guard = TerminalGuard::new()?;

    // Your TUI application logic here...
    // If anything in this function panics, the guard's Drop
    // implementation will still run and restore the terminal.

    println!("TUI running...\r");
    Ok(())
}

fn main() {
    // Also install a panic hook for extra safety
    let original_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |panic_info| {
        // Restore terminal before printing the panic message
        let _ = stdout().execute(LeaveAlternateScreen);
        let _ = terminal::disable_raw_mode();
        original_hook(panic_info);
    }));

    if let Err(e) = run_app() {
        eprintln!("Application error: {}", e);
    }
}
```

The panic hook is the second line of defense. While the `TerminalGuard` handles normal exits and panics in the same thread, the panic hook catches panics before the standard panic handler prints the backtrace -- ensuring the backtrace is readable and the terminal is usable.

::: wild In the Wild
Every production TUI application -- including terminal-based coding agents like Claude Code -- installs both a Drop guard and a panic hook to restore terminal state. Some also install signal handlers for SIGTERM and SIGHUP to clean up when killed by the system. Failing to restore the terminal is one of the most common and most frustrating bugs in TUI applications, because the user must manually run `reset` or `stty sane` to recover.
:::

## Mouse Reporting

Raw mode is a prerequisite for receiving mouse events, but you must also explicitly enable mouse reporting. This is done through additional escape sequences:

```rust
use crossterm::{
    event::{
        self, DisableMouseCapture, EnableMouseCapture,
        Event, MouseEvent, MouseEventKind,
    },
    terminal::{self, EnterAlternateScreen, LeaveAlternateScreen},
    ExecutableCommand,
};
use std::io::{self, stdout, Write};

fn main() -> io::Result<()> {
    terminal::enable_raw_mode()?;
    let mut out = stdout();
    out.execute(EnterAlternateScreen)?;
    out.execute(EnableMouseCapture)?;

    write!(out, "\x1b[1;1HMouse reporting enabled! Click or scroll.\r\n")?;
    write!(out, "Press 'q' to quit.\r\n")?;
    out.flush()?;

    loop {
        match event::read()? {
            Event::Mouse(MouseEvent { kind, column, row, .. }) => {
                let description = match kind {
                    MouseEventKind::Down(_) => "Click",
                    MouseEventKind::Up(_) => "Release",
                    MouseEventKind::Drag(_) => "Drag",
                    MouseEventKind::Moved => "Move",
                    MouseEventKind::ScrollDown => "Scroll Down",
                    MouseEventKind::ScrollUp => "Scroll Up",
                    MouseEventKind::ScrollLeft => "Scroll Left",
                    MouseEventKind::ScrollRight => "Scroll Right",
                };
                write!(
                    out,
                    "\x1b[4;1H{} at ({}, {})\x1b[0K\r\n",
                    description, column, row
                )?;
                out.flush()?;
            }
            Event::Key(key) => {
                if key.code == crossterm::event::KeyCode::Char('q') {
                    break;
                }
            }
            _ => {}
        }
    }

    out.execute(DisableMouseCapture)?;
    out.execute(LeaveAlternateScreen)?;
    terminal::disable_raw_mode()?;
    Ok(())
}
```

Mouse events arrive as escape sequences that crossterm parses into structured `MouseEvent` values. Without raw mode, these sequences would be interpreted as text by the terminal driver.

## Bracketed Paste Mode

One more capability unlocked by raw mode: **bracketed paste mode**. When enabled, the terminal wraps pasted text in special markers (`\x1b[200~` and `\x1b[201~`). This lets your application distinguish between typed input and pasted content -- crucial for a coding agent where the user might paste code blocks:

```rust
use std::io::{self, Write};

fn main() {
    let mut out = io::stdout();

    // Enable bracketed paste mode
    write!(out, "\x1b[?2004h").unwrap();
    out.flush().unwrap();

    // When the user pastes text, the terminal sends:
    // \x1b[200~ <pasted content> \x1b[201~
    //
    // crossterm exposes this as Event::Paste(String)
    // so you can handle it differently from typed input

    // Disable bracketed paste mode
    write!(out, "\x1b[?2004l").unwrap();
    out.flush().unwrap();

    println!("Bracketed paste mode demonstrated.");
    println!("crossterm handles the enable/disable for you.");
}
```

## Key Takeaways

- Cooked (canonical) mode lets the kernel handle line editing, echo, and signal generation -- suitable for line-oriented CLI tools but insufficient for interactive TUIs.
- Raw mode delivers every keystroke immediately to your application, but you take on full responsibility for echo, line editing, signal handling (Ctrl+C), and the `\r\n` newline convention.
- Always restore terminal state on exit using an RAII guard (Drop implementation) combined with a panic hook -- a broken terminal is the most common TUI bug.
- Mouse reporting and bracketed paste mode are additional capabilities that require raw mode and must be explicitly enabled via crossterm's API or the corresponding escape sequences.
- Under the hood, raw mode on Unix is configured through the `termios` struct by clearing the `ICANON`, `ECHO`, `ISIG`, `IEXTEN`, and `OPOST` flags.
