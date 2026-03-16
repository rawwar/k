---
title: Text Input
description: Implementing text input fields in the terminal with cursor movement, selection, clipboard integration, multi-line editing, and history navigation.
---

# Text Input

> **What you'll learn:**
> - Building a text input widget with cursor positioning, insertion, deletion, and word-level navigation
> - Handling multi-line input with line wrapping, scrolling, and the Enter key for submission versus newlines
> - Integrating with the system clipboard for paste support and implementing input history with up/down arrow navigation

Text input is one of the most complex components in any TUI application. A coding agent's input field is where users type their prompts -- it needs to feel as responsive and familiar as a regular text editor. This means supporting cursor movement, word-level navigation, character and word deletion, clipboard operations, multi-line editing, and input history.

## The Input State Model

Following TEA, all input state lives in your Model struct. The input widget reads this state to render and the update function modifies it:

```rust
/// Text input state that persists in the Model
#[derive(Clone)]
struct InputState {
    /// The text content, stored as a Vec<char> for efficient
    /// cursor-based insertion and deletion
    chars: Vec<char>,

    /// Cursor position as a character index (not byte index)
    cursor: usize,

    /// History of previous inputs for up/down navigation
    history: Vec<String>,

    /// Current position in history (-1 means current input)
    history_index: Option<usize>,

    /// Saved current input when navigating history
    saved_input: String,
}

impl InputState {
    fn new() -> Self {
        Self {
            chars: Vec::new(),
            cursor: 0,
            history: Vec::new(),
            history_index: None,
            saved_input: String::new(),
        }
    }

    /// Get the current text as a String
    fn text(&self) -> String {
        self.chars.iter().collect()
    }

    /// Get the number of characters
    fn len(&self) -> usize {
        self.chars.len()
    }

    fn is_empty(&self) -> bool {
        self.chars.is_empty()
    }
}

fn main() {
    let state = InputState::new();
    println!("Input state: cursor at {}, {} chars", state.cursor, state.len());
    println!("Using Vec<char> instead of String for O(1) cursor operations.");
}
```

We store text as `Vec<char>` rather than `String` because Rust's `String` is a `Vec<u8>` encoded as UTF-8, which means operations at a character position require O(n) scanning. `Vec<char>` gives us O(1) indexing at the cost of using 4 bytes per character.

::: tip Coming from Python
In Python, strings are sequences of Unicode code points, and you can index them directly with `s[i]`. In Rust, `String` is UTF-8 encoded, so `s[i]` is not available (it would be byte indexing, which could land in the middle of a multi-byte character). Using `Vec<char>` gives us the Python-like indexing behavior where position `i` is always the i-th character.
:::

## Input Operations

Each editing operation is a method on the input state, called from the update function:

```rust
#[derive(Clone)]
struct InputState {
    chars: Vec<char>,
    cursor: usize,
    history: Vec<String>,
    history_index: Option<usize>,
    saved_input: String,
}

impl InputState {
    fn new() -> Self {
        Self {
            chars: Vec::new(),
            cursor: 0,
            history: Vec::new(),
            history_index: None,
            saved_input: String::new(),
        }
    }

    fn text(&self) -> String {
        self.chars.iter().collect()
    }

    /// Insert a character at the cursor position
    fn insert_char(&mut self, c: char) {
        self.chars.insert(self.cursor, c);
        self.cursor += 1;
    }

    /// Insert a string at the cursor position (for paste)
    fn insert_str(&mut self, s: &str) {
        for c in s.chars() {
            self.chars.insert(self.cursor, c);
            self.cursor += 1;
        }
    }

    /// Delete the character before the cursor (Backspace)
    fn delete_backward(&mut self) {
        if self.cursor > 0 {
            self.cursor -= 1;
            self.chars.remove(self.cursor);
        }
    }

    /// Delete the character at the cursor (Delete key)
    fn delete_forward(&mut self) {
        if self.cursor < self.chars.len() {
            self.chars.remove(self.cursor);
        }
    }

    /// Delete the word before the cursor (Ctrl+W)
    fn delete_word_backward(&mut self) {
        // Skip any spaces before the word
        while self.cursor > 0 && self.chars[self.cursor - 1] == ' ' {
            self.cursor -= 1;
            self.chars.remove(self.cursor);
        }
        // Delete the word itself
        while self.cursor > 0 && self.chars[self.cursor - 1] != ' ' {
            self.cursor -= 1;
            self.chars.remove(self.cursor);
        }
    }

    /// Clear from cursor to end of line (Ctrl+K)
    fn kill_to_end(&mut self) {
        self.chars.truncate(self.cursor);
    }

    /// Clear from cursor to start of line (Ctrl+U)
    fn kill_to_start(&mut self) {
        self.chars.drain(0..self.cursor);
        self.cursor = 0;
    }

    /// Move cursor left one character
    fn move_left(&mut self) {
        self.cursor = self.cursor.saturating_sub(1);
    }

    /// Move cursor right one character
    fn move_right(&mut self) {
        if self.cursor < self.chars.len() {
            self.cursor += 1;
        }
    }

    /// Move cursor left one word (Ctrl+Left or Alt+B)
    fn move_word_left(&mut self) {
        // Skip spaces
        while self.cursor > 0 && self.chars[self.cursor - 1] == ' ' {
            self.cursor -= 1;
        }
        // Skip word characters
        while self.cursor > 0 && self.chars[self.cursor - 1] != ' ' {
            self.cursor -= 1;
        }
    }

    /// Move cursor right one word (Ctrl+Right or Alt+F)
    fn move_word_right(&mut self) {
        let len = self.chars.len();
        // Skip current word
        while self.cursor < len && self.chars[self.cursor] != ' ' {
            self.cursor += 1;
        }
        // Skip spaces
        while self.cursor < len && self.chars[self.cursor] == ' ' {
            self.cursor += 1;
        }
    }

    /// Move cursor to start of line (Home or Ctrl+A)
    fn move_to_start(&mut self) {
        self.cursor = 0;
    }

    /// Move cursor to end of line (End or Ctrl+E)
    fn move_to_end(&mut self) {
        self.cursor = self.chars.len();
    }

    /// Submit the input: return the text and clear the buffer
    fn submit(&mut self) -> String {
        let text = self.text();
        if !text.is_empty() {
            self.history.push(text.clone());
        }
        self.chars.clear();
        self.cursor = 0;
        self.history_index = None;
        text
    }
}

fn main() {
    let mut input = InputState::new();

    // Simulate typing "hello world"
    for c in "hello world".chars() {
        input.insert_char(c);
    }
    println!("After typing: '{}' (cursor at {})", input.text(), input.cursor);

    // Move cursor left 5, then delete word backward
    for _ in 0..5 {
        input.move_left();
    }
    println!("After move left 5: cursor at {}", input.cursor);

    input.delete_word_backward();
    println!("After Ctrl+W: '{}' (cursor at {})", input.text(), input.cursor);

    // Submit
    let submitted = input.submit();
    println!("Submitted: '{}'", submitted);
    println!("After submit: '{}' (cursor at {})", input.text(), input.cursor);
}
```

## Wiring Input to Key Events

The update function maps key events to input operations:

```rust
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

struct InputState {
    chars: Vec<char>,
    cursor: usize,
}

impl InputState {
    fn new() -> Self {
        Self { chars: Vec::new(), cursor: 0 }
    }

    fn text(&self) -> String { self.chars.iter().collect() }

    fn insert_char(&mut self, c: char) {
        self.chars.insert(self.cursor, c);
        self.cursor += 1;
    }

    fn delete_backward(&mut self) {
        if self.cursor > 0 {
            self.cursor -= 1;
            self.chars.remove(self.cursor);
        }
    }

    fn move_left(&mut self) { self.cursor = self.cursor.saturating_sub(1); }
    fn move_right(&mut self) {
        if self.cursor < self.chars.len() { self.cursor += 1; }
    }
    fn move_to_start(&mut self) { self.cursor = 0; }
    fn move_to_end(&mut self) { self.cursor = self.chars.len(); }
}

enum InputAction {
    Consumed,       // Key was handled by input
    Submit(String), // Enter was pressed, here is the text
    Cancel,         // Escape was pressed
    Unhandled,      // Key not relevant to input
}

fn handle_input_key(input: &mut InputState, key: KeyEvent) -> InputAction {
    match (key.code, key.modifiers) {
        // Character input
        (KeyCode::Char(c), KeyModifiers::NONE | KeyModifiers::SHIFT) => {
            input.insert_char(c);
            InputAction::Consumed
        }

        // Deletion
        (KeyCode::Backspace, KeyModifiers::NONE) => {
            input.delete_backward();
            InputAction::Consumed
        }

        // Navigation
        (KeyCode::Left, KeyModifiers::NONE) => {
            input.move_left();
            InputAction::Consumed
        }
        (KeyCode::Right, KeyModifiers::NONE) => {
            input.move_right();
            InputAction::Consumed
        }
        (KeyCode::Home, KeyModifiers::NONE) => {
            input.move_to_start();
            InputAction::Consumed
        }
        (KeyCode::End, KeyModifiers::NONE) => {
            input.move_to_end();
            InputAction::Consumed
        }

        // Emacs-style bindings
        (KeyCode::Char('a'), KeyModifiers::CONTROL) => {
            input.move_to_start();
            InputAction::Consumed
        }
        (KeyCode::Char('e'), KeyModifiers::CONTROL) => {
            input.move_to_end();
            InputAction::Consumed
        }

        // Submit
        (KeyCode::Enter, KeyModifiers::NONE) => {
            let text: String = input.chars.iter().collect();
            input.chars.clear();
            input.cursor = 0;
            InputAction::Submit(text)
        }

        // Cancel
        (KeyCode::Esc, KeyModifiers::NONE) => InputAction::Cancel,

        _ => InputAction::Unhandled,
    }
}

fn main() {
    let mut input = InputState::new();

    // Simulate typing
    let h_key = KeyEvent::new(KeyCode::Char('h'), KeyModifiers::NONE);
    let i_key = KeyEvent::new(KeyCode::Char('i'), KeyModifiers::NONE);
    let enter = KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE);

    handle_input_key(&mut input, h_key);
    handle_input_key(&mut input, i_key);
    println!("Buffer: '{}'", input.text());

    match handle_input_key(&mut input, enter) {
        InputAction::Submit(text) => println!("Submitted: '{}'", text),
        _ => {}
    }
}
```

## Input History

Users expect to press Up/Down to cycle through previous inputs, just like in a shell:

```rust
struct InputWithHistory {
    chars: Vec<char>,
    cursor: usize,
    history: Vec<String>,
    history_index: Option<usize>,
    saved_current: String,
}

impl InputWithHistory {
    fn new() -> Self {
        Self {
            chars: Vec::new(),
            cursor: 0,
            history: Vec::new(),
            history_index: None,
            saved_current: String::new(),
        }
    }

    fn text(&self) -> String {
        self.chars.iter().collect()
    }

    fn set_text(&mut self, text: &str) {
        self.chars = text.chars().collect();
        self.cursor = self.chars.len();
    }

    fn navigate_history_up(&mut self) {
        if self.history.is_empty() {
            return;
        }

        match self.history_index {
            None => {
                // Save current input before navigating into history
                self.saved_current = self.text();
                self.history_index = Some(self.history.len() - 1);
            }
            Some(0) => return, // Already at oldest entry
            Some(idx) => {
                self.history_index = Some(idx - 1);
            }
        }

        if let Some(idx) = self.history_index {
            self.set_text(&self.history[idx].clone());
        }
    }

    fn navigate_history_down(&mut self) {
        match self.history_index {
            None => return, // Not in history
            Some(idx) => {
                if idx + 1 < self.history.len() {
                    self.history_index = Some(idx + 1);
                    self.set_text(&self.history[idx + 1].clone());
                } else {
                    // Return to current input
                    self.history_index = None;
                    let saved = self.saved_current.clone();
                    self.set_text(&saved);
                }
            }
        }
    }

    fn submit(&mut self) -> String {
        let text = self.text();
        if !text.is_empty() {
            self.history.push(text.clone());
        }
        self.chars.clear();
        self.cursor = 0;
        self.history_index = None;
        self.saved_current.clear();
        text
    }
}

fn main() {
    let mut input = InputWithHistory::new();

    // Simulate previous submissions
    input.chars = "first message".chars().collect();
    input.submit();
    input.chars = "second message".chars().collect();
    input.submit();
    input.chars = "third message".chars().collect();
    input.submit();

    // Now type something new
    input.set_text("current typing");

    // Navigate up through history
    input.navigate_history_up();
    println!("History up 1: '{}'", input.text()); // "third message"

    input.navigate_history_up();
    println!("History up 2: '{}'", input.text()); // "second message"

    // Navigate back down
    input.navigate_history_down();
    println!("History down: '{}'", input.text()); // "third message"

    input.navigate_history_down();
    println!("Back to current: '{}'", input.text()); // "current typing"
}
```

## Rendering the Input Widget

The input widget renders the text and positions the cursor:

```rust
use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Modifier, Style},
    widgets::{Block, Borders, Widget},
};

struct TextInput<'a> {
    text: &'a [char],
    cursor: usize,
    placeholder: &'a str,
    block: Option<Block<'a>>,
    focused: bool,
}

impl<'a> TextInput<'a> {
    fn new(text: &'a [char], cursor: usize) -> Self {
        Self {
            text,
            cursor,
            placeholder: "",
            block: None,
            focused: true,
        }
    }

    fn placeholder(mut self, placeholder: &'a str) -> Self {
        self.placeholder = placeholder;
        self
    }

    fn block(mut self, block: Block<'a>) -> Self {
        self.block = Some(block);
        self
    }

    fn focused(mut self, focused: bool) -> Self {
        self.focused = focused;
        self
    }

    /// Returns the cursor position in terminal coordinates
    /// for use with frame.set_cursor_position()
    fn cursor_position(&self, area: Rect) -> (u16, u16) {
        let inner_x = if self.block.is_some() { area.x + 1 } else { area.x };
        let inner_y = if self.block.is_some() { area.y + 1 } else { area.y };
        (inner_x + self.cursor as u16, inner_y)
    }
}

impl<'a> Widget for TextInput<'a> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let inner = if let Some(ref block) = self.block {
            let border_style = if self.focused {
                Style::default().fg(Color::Cyan)
            } else {
                Style::default().fg(Color::DarkGray)
            };
            let styled_block = block.clone().border_style(border_style);
            let inner = styled_block.inner(area);
            styled_block.render(area, buf);
            inner
        } else {
            area
        };

        if inner.height == 0 || inner.width == 0 {
            return;
        }

        if self.text.is_empty() {
            // Show placeholder
            let style = Style::default().fg(Color::DarkGray);
            buf.set_string(inner.x, inner.y, self.placeholder, style);
        } else {
            // Render the text
            let text_style = Style::default().fg(Color::White);
            let visible_width = inner.width as usize;

            // Calculate visible window (scroll horizontally if text is wider)
            let (start, _display_cursor) = if self.cursor > visible_width.saturating_sub(1) {
                let start = self.cursor - visible_width + 1;
                (start, visible_width - 1)
            } else {
                (0, self.cursor)
            };

            let end = (start + visible_width).min(self.text.len());
            let visible: String = self.text[start..end].iter().collect();
            buf.set_string(inner.x, inner.y, &visible, text_style);
        }
    }
}

fn main() {
    let text: Vec<char> = "hello world".chars().collect();
    let area = Rect::new(0, 0, 40, 3);

    let _widget = TextInput::new(&text, 5)
        .placeholder("Type your message...")
        .block(Block::default().borders(Borders::ALL).title("Input"))
        .focused(true);

    let cursor_pos = _widget.cursor_position(area);
    println!("Cursor terminal position: ({}, {})", cursor_pos.0, cursor_pos.1);
    println!("Horizontal scrolling kicks in when text exceeds widget width.");
}
```

::: wild In the Wild
Production coding agents like Claude Code use multi-line input fields that support Shift+Enter for newlines and Enter for submission. The tui-textarea crate provides a ready-made multi-line text input widget for Ratatui with features like syntax highlighting, search and replace, and configurable key bindings. For a coding agent, starting with `tui-textarea` and customizing it is often faster than building text input from scratch.
:::

## The tui-textarea Crate

For production use, the `tui-textarea` crate provides a battle-tested text area widget:

```rust
// In Cargo.toml:
// [dependencies]
// tui-textarea = "0.6"

// Usage (conceptual -- requires ratatui setup):
// use tui_textarea::{Input, Key, TextArea};
//
// let mut textarea = TextArea::default();
// textarea.set_placeholder_text("Type your message...");
// textarea.set_cursor_line_style(Style::default());
// textarea.set_block(Block::default().borders(Borders::ALL).title("Input"));
//
// // In event handling:
// match crossterm::event::read()?.into() {
//     Input { key: Key::Enter, .. } => {
//         let lines = textarea.lines().to_vec();
//         // Process input
//     }
//     input => {
//         textarea.input(input); // Handles all editing internally
//     }
// }
//
// // In rendering:
// frame.render_widget(&textarea, area);

fn main() {
    println!("tui-textarea provides a full-featured text area for Ratatui:");
    println!("  - Multi-line editing");
    println!("  - Cursor movement (arrows, word-level, Home/End)");
    println!("  - Undo/redo");
    println!("  - Search and replace");
    println!("  - Syntax highlighting");
    println!("  - Configurable key bindings");
}
```

## Key Takeaways

- Text input state (characters, cursor position, history) lives in the Model as a dedicated struct, following TEA's centralized state principle.
- Using `Vec<char>` instead of `String` for the input buffer provides O(1) character-level indexing and insertion, avoiding UTF-8 byte-boundary issues that arise with Rust's `String`.
- A complete input implementation needs character insertion/deletion, word-level navigation (Ctrl+Left/Right), line-level operations (Ctrl+A/E/K/U), and history navigation (Up/Down arrows).
- The input widget renders the text, handles horizontal scrolling when the text exceeds the widget width, shows a placeholder when empty, and provides cursor coordinates for `frame.set_cursor_position()`.
- For production use, the `tui-textarea` crate provides a mature, full-featured text area widget with multi-line editing, undo/redo, and configurable key bindings out of the box.
