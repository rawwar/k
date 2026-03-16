---
title: Input Box
description: Build a text input widget with cursor movement, line editing, history navigation, and multi-line support.
---

# Input Box

> **What you'll learn:**
> - How to implement a text input field with cursor positioning and basic editing operations
> - How to support multi-line input with Enter for newline and a separate key combo for submit
> - How to add input history navigation with up/down arrow keys

The input box is where users type their prompts. It needs to feel like a real text editor -- character insertion at the cursor position, word deletion, cursor movement, multi-line support, and command history. Building a good input experience is what makes your agent feel professional rather than like a toy REPL.

## Input State

The input box needs more state than just a string. You need to track the cursor position, support multi-line text, and maintain a command history:

```rust
pub struct InputState {
    /// The current input text.
    pub content: String,
    /// The byte offset of the cursor within `content`.
    pub cursor: usize,
    /// Command history (newest at the end).
    pub history: Vec<String>,
    /// Current position in history (-1 means "current input").
    pub history_index: Option<usize>,
    /// The input being composed before history navigation started.
    pub saved_input: Option<String>,
}

impl InputState {
    pub fn new() -> Self {
        Self {
            content: String::new(),
            cursor: 0,
            history: Vec::new(),
            history_index: None,
            saved_input: None,
        }
    }
}
```

## Character Insertion and Deletion

The core editing operations insert and remove characters at the cursor position:

```rust
impl InputState {
    /// Insert a character at the cursor position.
    pub fn insert_char(&mut self, c: char) {
        self.content.insert(self.cursor, c);
        self.cursor += c.len_utf8();
    }

    /// Insert a newline at the cursor position.
    pub fn insert_newline(&mut self) {
        self.insert_char('\n');
    }

    /// Delete the character before the cursor (Backspace).
    pub fn delete_char_before(&mut self) {
        if self.cursor > 0 {
            // Find the previous character boundary
            let prev = self.content[..self.cursor]
                .char_indices()
                .last()
                .map(|(i, _)| i)
                .unwrap_or(0);
            self.content.drain(prev..self.cursor);
            self.cursor = prev;
        }
    }

    /// Delete the character at the cursor (Delete key).
    pub fn delete_char_at(&mut self) {
        if self.cursor < self.content.len() {
            let next = self.content[self.cursor..]
                .char_indices()
                .nth(1)
                .map(|(i, _)| self.cursor + i)
                .unwrap_or(self.content.len());
            self.content.drain(self.cursor..next);
        }
    }

    /// Delete the word before the cursor (Ctrl+W).
    pub fn delete_word_before(&mut self) {
        if self.cursor == 0 {
            return;
        }

        let before = &self.content[..self.cursor];

        // Skip trailing whitespace
        let trimmed_end = before.trim_end().len();

        // Find the start of the current word
        let word_start = before[..trimmed_end]
            .rfind(|c: char| c.is_whitespace())
            .map(|i| i + 1)
            .unwrap_or(0);

        self.content.drain(word_start..self.cursor);
        self.cursor = word_start;
    }

    /// Delete everything from cursor to start of line (Ctrl+U).
    pub fn delete_to_line_start(&mut self) {
        // Find the start of the current line
        let line_start = self.content[..self.cursor]
            .rfind('\n')
            .map(|i| i + 1)
            .unwrap_or(0);

        self.content.drain(line_start..self.cursor);
        self.cursor = line_start;
    }
}
```

Note the careful use of `char_indices()` and `len_utf8()` throughout. Rust strings are UTF-8, so a character can be 1 to 4 bytes. You cannot just do `cursor -= 1` -- you need to find the previous character boundary. This is a common pitfall for Python developers, where `string[n]` just works because Python strings index by code point.

::: tip Coming from Python
In Python, strings are sequences of Unicode code points, so cursor movement is simple:
```python
text = "hello"
cursor = 3
# Move left: cursor -= 1
# Delete at cursor: text = text[:cursor] + text[cursor+1:]
```
In Rust, `String` is UTF-8 encoded bytes. Moving the cursor means finding the next or previous character boundary, not just adding or subtracting 1. The `char_indices()` method is your best friend for safe cursor positioning. Ratatui's ecosystem includes the `tui-input` and `tui-textarea` crates that handle these complexities if you prefer a pre-built solution.
:::

## Cursor Movement

Cursor movement respects character boundaries and line structure:

```rust
impl InputState {
    /// Move cursor left by one character.
    pub fn move_left(&mut self) {
        if self.cursor > 0 {
            self.cursor = self.content[..self.cursor]
                .char_indices()
                .last()
                .map(|(i, _)| i)
                .unwrap_or(0);
        }
    }

    /// Move cursor right by one character.
    pub fn move_right(&mut self) {
        if self.cursor < self.content.len() {
            self.cursor = self.content[self.cursor..]
                .char_indices()
                .nth(1)
                .map(|(i, _)| self.cursor + i)
                .unwrap_or(self.content.len());
        }
    }

    /// Move cursor to the start of the current line (Home / Ctrl+A).
    pub fn move_to_line_start(&mut self) {
        self.cursor = self.content[..self.cursor]
            .rfind('\n')
            .map(|i| i + 1)
            .unwrap_or(0);
    }

    /// Move cursor to the end of the current line (End / Ctrl+E).
    pub fn move_to_line_end(&mut self) {
        self.cursor = self.content[self.cursor..]
            .find('\n')
            .map(|i| self.cursor + i)
            .unwrap_or(self.content.len());
    }

    /// Move cursor to the very beginning (Ctrl+Home).
    pub fn move_to_start(&mut self) {
        self.cursor = 0;
    }

    /// Move cursor to the very end (Ctrl+End).
    pub fn move_to_end(&mut self) {
        self.cursor = self.content.len();
    }
}
```

## Command History

History navigation lets users recall previous prompts with the up/down arrow keys:

```rust
impl InputState {
    /// Submit the current input, returning it and adding it to history.
    pub fn submit(&mut self) -> Option<String> {
        let text = self.content.trim().to_string();
        if text.is_empty() {
            return None;
        }

        // Add to history (avoid duplicates of the last entry)
        if self.history.last().map(|h| h.as_str()) != Some(&text) {
            self.history.push(text.clone());
        }

        // Reset state
        self.content.clear();
        self.cursor = 0;
        self.history_index = None;
        self.saved_input = None;

        Some(text)
    }

    /// Navigate to the previous history entry (Up arrow).
    pub fn history_previous(&mut self) {
        if self.history.is_empty() {
            return;
        }

        let new_index = match self.history_index {
            None => {
                // Entering history mode: save current input
                self.saved_input = Some(self.content.clone());
                self.history.len() - 1
            }
            Some(0) => return, // Already at the oldest entry
            Some(i) => i - 1,
        };

        self.history_index = Some(new_index);
        self.content = self.history[new_index].clone();
        self.cursor = self.content.len();
    }

    /// Navigate to the next history entry (Down arrow).
    pub fn history_next(&mut self) {
        let current_index = match self.history_index {
            None => return, // Not in history mode
            Some(i) => i,
        };

        if current_index >= self.history.len() - 1 {
            // Moving past the newest entry: restore saved input
            self.history_index = None;
            self.content = self.saved_input.take().unwrap_or_default();
        } else {
            self.history_index = Some(current_index + 1);
            self.content = self.history[current_index + 1].clone();
        }

        self.cursor = self.content.len();
    }
}
```

## Rendering the Input Box

The input box widget renders the text content, highlights the cursor position, and shows mode information:

```rust
use ratatui::{prelude::*, widgets::{Block, Borders, Paragraph, Wrap}};

fn render_input(frame: &mut Frame, app: &App, area: Rect) {
    let is_focused = app.focused_pane == FocusedPane::Input;

    let block = Block::default()
        .title(input_title(app))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(
            if is_focused { Color::Green } else { Color::DarkGray }
        ));

    // Build the display text with a visible cursor indicator
    let display_text = if app.input_mode == InputMode::Editing {
        &app.input_state.content
    } else {
        if app.input_state.content.is_empty() {
            "Press 'i' to start typing..."
        } else {
            &app.input_state.content
        }
    };

    let style = if app.input_mode == InputMode::Editing {
        Style::default().fg(Color::White)
    } else {
        Style::default().fg(Color::DarkGray)
    };

    let paragraph = Paragraph::new(display_text)
        .block(block)
        .style(style)
        .wrap(Wrap { trim: false });

    frame.render_widget(paragraph, area);

    // Position the blinking cursor
    if app.input_mode == InputMode::Editing && is_focused {
        let inner = area.inner(Margin { horizontal: 1, vertical: 1 });
        let (cursor_x, cursor_y) = compute_cursor_position(
            &app.input_state.content,
            app.input_state.cursor,
            inner.width,
        );
        frame.set_cursor_position((
            inner.x + cursor_x,
            inner.y + cursor_y,
        ));
    }
}

fn input_title(app: &App) -> String {
    match app.input_mode {
        InputMode::Normal => String::from(" Input "),
        InputMode::Editing => {
            let line_count = app.input_state.content.matches('\n').count() + 1;
            if line_count > 1 {
                format!(" Input ({} lines) | Ctrl+Enter to send ", line_count)
            } else {
                String::from(" Input | Ctrl+Enter to send ")
            }
        }
    }
}

/// Compute the x,y position of the cursor in the wrapped text.
fn compute_cursor_position(text: &str, byte_offset: usize, width: u16) -> (u16, u16) {
    let text_before_cursor = &text[..byte_offset];

    let mut x: u16 = 0;
    let mut y: u16 = 0;

    for ch in text_before_cursor.chars() {
        if ch == '\n' {
            x = 0;
            y += 1;
        } else {
            x += 1;
            if x >= width {
                x = 0;
                y += 1;
            }
        }
    }

    (x, y)
}
```

## Testing the Input State

Because `InputState` is pure data with pure methods, testing is straightforward:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_insert_and_delete() {
        let mut input = InputState::new();
        input.insert_char('h');
        input.insert_char('e');
        input.insert_char('l');
        input.insert_char('l');
        input.insert_char('o');

        assert_eq!(input.content, "hello");
        assert_eq!(input.cursor, 5);

        input.delete_char_before();
        assert_eq!(input.content, "hell");
        assert_eq!(input.cursor, 4);
    }

    #[test]
    fn test_cursor_movement() {
        let mut input = InputState::new();
        input.content = String::from("hello world");
        input.cursor = 5;

        input.move_left();
        assert_eq!(input.cursor, 4);

        input.move_right();
        assert_eq!(input.cursor, 5);

        input.move_to_line_start();
        assert_eq!(input.cursor, 0);

        input.move_to_line_end();
        assert_eq!(input.cursor, 11);
    }

    #[test]
    fn test_history_navigation() {
        let mut input = InputState::new();

        // Submit two commands
        input.content = String::from("first");
        input.cursor = 5;
        input.submit();

        input.content = String::from("second");
        input.cursor = 6;
        input.submit();

        // Type something new, then navigate history
        input.content = String::from("third");
        input.cursor = 5;

        input.history_previous(); // should show "second"
        assert_eq!(input.content, "second");

        input.history_previous(); // should show "first"
        assert_eq!(input.content, "first");

        input.history_next(); // should show "second"
        assert_eq!(input.content, "second");

        input.history_next(); // should restore "third"
        assert_eq!(input.content, "third");
    }
}
```

::: tip In the Wild
Claude Code features a sophisticated input box with multi-line editing, syntax awareness, and command history. OpenCode's input similarly supports multi-line input with special handling for Shift+Enter versus Enter. The pattern of saving the in-progress input before entering history mode (so it is restored when the user navigates back) is standard across shell-like interfaces and coding agents alike.
:::

## Key Takeaways

- **UTF-8 awareness** is critical for cursor operations in Rust -- always use `char_indices()` and `len_utf8()` to find character boundaries rather than simple byte arithmetic.
- **Multi-line input** uses Enter for newlines and Ctrl+Enter for submission, with the input title showing the line count and submission hint.
- **Command history** saves previous inputs and supports up/down navigation with saved-input restoration when exiting history mode.
- **Cursor rendering** requires computing the x,y position from the byte offset by walking through characters and handling newlines and wrapping.
- **Pure state operations** on `InputState` are easily testable without any terminal or UI setup, following the Elm architecture principle.
