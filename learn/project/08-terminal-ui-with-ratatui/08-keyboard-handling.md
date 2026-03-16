---
title: Keyboard Handling
description: Map keyboard events to application actions including navigation, text editing, and modal shortcuts.
---

# Keyboard Handling

> **What you'll learn:**
> - How to read and match key events including modifiers like Ctrl, Alt, and Shift
> - How to implement mode-based key bindings that change behavior based on application state
> - How to handle special keys like arrow keys, Home, End, Page Up, and Page Down

Your coding agent needs to respond to many different key combinations: typing text, navigating conversations, scrolling output, submitting prompts, and quitting. The way you structure keyboard handling determines whether your agent feels intuitive or frustrating to use.

## Reading Key Events

Crossterm delivers keyboard input as `KeyEvent` structs. Each event contains a key code, modifiers (Ctrl, Alt, Shift), and an event kind (press, release, repeat):

```rust
use crossterm::event::{KeyEvent, KeyCode, KeyModifiers, KeyEventKind};

fn describe_key(event: KeyEvent) -> String {
    // Only handle key press events (not release or repeat)
    if event.kind != KeyEventKind::Press {
        return String::from("(not a press event)");
    }

    let modifiers = if event.modifiers.contains(KeyModifiers::CONTROL) {
        "Ctrl+"
    } else if event.modifiers.contains(KeyModifiers::ALT) {
        "Alt+"
    } else if event.modifiers.contains(KeyModifiers::SHIFT) {
        "Shift+"
    } else {
        ""
    };

    let key = match event.code {
        KeyCode::Char(c) => format!("{}", c),
        KeyCode::Enter => String::from("Enter"),
        KeyCode::Backspace => String::from("Backspace"),
        KeyCode::Delete => String::from("Delete"),
        KeyCode::Left => String::from("Left"),
        KeyCode::Right => String::from("Right"),
        KeyCode::Up => String::from("Up"),
        KeyCode::Down => String::from("Down"),
        KeyCode::Home => String::from("Home"),
        KeyCode::End => String::from("End"),
        KeyCode::PageUp => String::from("PageUp"),
        KeyCode::PageDown => String::from("PageDown"),
        KeyCode::Tab => String::from("Tab"),
        KeyCode::Esc => String::from("Esc"),
        KeyCode::F(n) => format!("F{}", n),
        _ => String::from("(other)"),
    };

    format!("{}{}", modifiers, key)
}
```

A critical detail: on many terminals, `KeyEventKind::Press` is the only kind you receive. But on Windows and some modern terminals that support the Kitty keyboard protocol, you also get `Release` and `Repeat` events. Always filter for `Press` unless you specifically need the others.

## Mode-Based Key Bindings

A coding agent has at least two modes of operation:

1. **Input mode** -- the user is typing in the input box. Letter keys insert characters.
2. **Normal mode** -- the user is navigating. Letter keys trigger shortcuts (like `j`/`k` for scrolling).

Your key handler should check the current mode before interpreting a keypress:

```rust
use crossterm::event::{KeyEvent, KeyCode, KeyModifiers, KeyEventKind};

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum InputMode {
    Normal,
    Editing,
}

pub fn handle_key_event(app: &App, key: KeyEvent) -> Option<Message> {
    // Ignore non-press events
    if key.kind != KeyEventKind::Press {
        return None;
    }

    // Global shortcuts (work in any mode)
    match key.code {
        KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            return Some(Message::Quit);
        }
        _ => {}
    }

    // Mode-specific handling
    match app.input_mode {
        InputMode::Normal => handle_normal_mode(key),
        InputMode::Editing => handle_editing_mode(key),
    }
}

fn handle_normal_mode(key: KeyEvent) -> Option<Message> {
    match key.code {
        // Navigation
        KeyCode::Char('j') | KeyCode::Down => Some(Message::ScrollDown),
        KeyCode::Char('k') | KeyCode::Up => Some(Message::ScrollUp),
        KeyCode::Char('g') => Some(Message::ScrollToTop),
        KeyCode::Char('G') => Some(Message::ScrollToBottom),
        KeyCode::PageUp => Some(Message::PageUp),
        KeyCode::PageDown => Some(Message::PageDown),

        // Mode switching
        KeyCode::Char('i') | KeyCode::Enter => Some(Message::EnterEditMode),
        KeyCode::Tab => Some(Message::SwitchFocus),

        // Actions
        KeyCode::Char('q') => Some(Message::Quit),
        KeyCode::Char('/') => Some(Message::StartSearch),

        _ => None,
    }
}

fn handle_editing_mode(key: KeyEvent) -> Option<Message> {
    match key.code {
        // Text input
        KeyCode::Char(c) => Some(Message::KeyPressed(c)),
        KeyCode::Backspace => Some(Message::Backspace),
        KeyCode::Delete => Some(Message::Delete),

        // Cursor movement within the input
        KeyCode::Left => Some(Message::CursorLeft),
        KeyCode::Right => Some(Message::CursorRight),
        KeyCode::Home => Some(Message::CursorHome),
        KeyCode::End => Some(Message::CursorEnd),

        // Submit with Ctrl+Enter (Enter alone adds newline for multi-line input)
        KeyCode::Enter if key.modifiers.contains(KeyModifiers::CONTROL) => {
            Some(Message::Submit)
        }
        KeyCode::Enter => Some(Message::NewLine),

        // Exit editing mode
        KeyCode::Esc => Some(Message::EnterNormalMode),

        // Editing shortcuts
        KeyCode::Char('w') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            Some(Message::DeleteWord)
        }
        KeyCode::Char('u') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            Some(Message::DeleteLine)
        }
        KeyCode::Char('a') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            Some(Message::CursorHome)
        }
        KeyCode::Char('e') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            Some(Message::CursorEnd)
        }

        _ => None,
    }
}
```

::: python Coming from Python
In Python's `curses`, you read keys with `stdscr.getch()` and compare against constants:
```python
import curses

key = stdscr.getch()
if key == curses.KEY_UP:
    scroll_up()
elif key == ord('q'):
    quit()
elif key == 27:  # ESC or start of escape sequence
    # Handling escape sequences manually is painful in curses
    pass
```
Crossterm handles escape sequence parsing for you, giving you clean `KeyCode` values instead of raw bytes. This is a huge improvement over curses, where you often have to deal with ambiguous escape sequences and platform-specific key codes.
:::

## Expanding the Message Enum

With mode-based handling, your message enum grows to include all the new actions:

```rust
pub enum Message {
    // Text editing
    KeyPressed(char),
    Backspace,
    Delete,
    DeleteWord,
    DeleteLine,
    NewLine,
    Submit,

    // Cursor movement
    CursorLeft,
    CursorRight,
    CursorHome,
    CursorEnd,

    // Scrolling
    ScrollUp,
    ScrollDown,
    ScrollToTop,
    ScrollToBottom,
    PageUp,
    PageDown,

    // Mode switching
    EnterEditMode,
    EnterNormalMode,
    SwitchFocus,
    StartSearch,

    // Application
    Quit,
    Tick,
    Resize(u16, u16),

    // Agent events
    StreamingStarted,
    TokenReceived(String),
    StreamingCompleted(String),
    ErrorOccurred(String),
}
```

## Implementing Cursor Movement in the Update Function

The cursor movement messages need corresponding logic in the update function. Here is how to handle cursor positioning in the input buffer:

```rust
impl App {
    pub fn update(&mut self, msg: Message) {
        match msg {
            Message::CursorLeft => {
                // Move cursor left, respecting character boundaries
                if self.cursor_position > 0 {
                    self.cursor_position -= 1;
                }
            }
            Message::CursorRight => {
                // Move cursor right, but not past the end of the input
                if self.cursor_position < self.input.len() {
                    self.cursor_position += 1;
                }
            }
            Message::CursorHome => {
                self.cursor_position = 0;
            }
            Message::CursorEnd => {
                self.cursor_position = self.input.len();
            }
            Message::DeleteWord => {
                // Delete backwards to the previous word boundary
                if self.cursor_position > 0 {
                    let before_cursor = &self.input[..self.cursor_position];
                    let word_start = before_cursor
                        .rfind(|c: char| c.is_whitespace())
                        .map(|i| i + 1)
                        .unwrap_or(0);
                    self.input.drain(word_start..self.cursor_position);
                    self.cursor_position = word_start;
                }
            }
            Message::DeleteLine => {
                // Delete everything before the cursor
                self.input.drain(..self.cursor_position);
                self.cursor_position = 0;
            }
            // ... other message handlers ...
            _ => {}
        }
    }
}
```

## Visual Mode Indicator

Users need to know which mode they are in. A common pattern is to change the input box's border color or add a mode label:

```rust
use ratatui::{prelude::*, widgets::{Block, Borders, Paragraph}};

fn render_input(frame: &mut Frame, app: &App, area: Rect) {
    let (border_color, mode_label) = match app.input_mode {
        InputMode::Normal => (Color::DarkGray, " Normal "),
        InputMode::Editing => (Color::Green, " Insert "),
    };

    let input = Paragraph::new(app.input.as_str())
        .block(
            Block::default()
                .title(mode_label)
                .borders(Borders::ALL)
                .border_style(Style::default().fg(border_color)),
        );

    frame.render_widget(input, area);

    // Show the cursor only in editing mode
    if app.input_mode == InputMode::Editing {
        // Position the cursor inside the input box
        // +1 for the left border
        frame.set_cursor_position((
            area.x + app.cursor_position as u16 + 1,
            area.y + 1,  // +1 for the top border
        ));
    }
}
```

The `set_cursor_position` call is important -- it tells the terminal where to put the blinking cursor. Without it, the cursor sits at position (0,0), which looks wrong.

## Key Repeat and Debouncing

When a user holds down a key, the terminal sends repeated key events. For scrolling, this is desirable -- holding the down arrow should scroll continuously. For submission (Enter/Ctrl+Enter), you generally want to ignore repeats:

```rust
fn handle_editing_mode(key: KeyEvent) -> Option<Message> {
    // For the submit action, only respond to Press, not Repeat
    if key.code == KeyCode::Enter
        && key.modifiers.contains(KeyModifiers::CONTROL)
        && key.kind == KeyEventKind::Press
    {
        return Some(Message::Submit);
    }

    // For other keys, Press and Repeat are both fine
    match key.code {
        KeyCode::Char(c) => Some(Message::KeyPressed(c)),
        KeyCode::Backspace => Some(Message::Backspace),
        KeyCode::Left => Some(Message::CursorLeft),
        KeyCode::Right => Some(Message::CursorRight),
        // ... etc
        _ => None,
    }
}
```

::: wild In the Wild
OpenCode implements vim-like keybindings in its normal mode, with `j`/`k` for scrolling and `i` to enter insert mode. Claude Code takes a simpler approach with fewer modes -- the input is always active and special actions use Ctrl-key combinations. Both approaches are valid. The mode-based approach gives power users more efficiency, while the always-editing approach has a lower learning curve. You will want to decide which model fits your target audience and document your keybindings clearly.
:::

## Key Takeaways

- **Filter for `KeyEventKind::Press`** to avoid processing release and repeat events unless you specifically need them.
- **Mode-based key handling** separates navigation shortcuts from text editing, preventing key conflicts (pressing `j` should not insert a character when you mean to scroll).
- **Global shortcuts** like Ctrl+C should work in every mode -- check for them before dispatching to mode-specific handlers.
- **Cursor positioning** with `frame.set_cursor_position()` gives users visual feedback about where their next character will be inserted.
- **A clear mode indicator** (border color change, label) prevents confusion about which mode is active.
