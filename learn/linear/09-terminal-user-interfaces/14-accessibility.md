---
title: Accessibility
description: Making terminal applications accessible to screen readers and users with visual impairments through semantic output, keyboard navigation, and high-contrast support.
---

# Accessibility

> **What you'll learn:**
> - How screen readers interact with terminal applications and what makes terminal output accessible or inaccessible
> - Designing keyboard-only navigation that does not rely on mouse interaction or visual-only indicators
> - Supporting high-contrast modes, respecting NO_COLOR and TERM environment hints, and providing configurable color schemes

Accessibility in terminal applications is often an afterthought, but it should not be. Developers who use screen readers, who have low vision, who are colorblind, or who work in constrained environments (high-latency SSH sessions, low-resolution displays) all deserve a usable coding agent. Many accessibility improvements also make the tool better for everyone -- keyboard-first navigation, clear status messages, and configurable appearance are universally beneficial features.

## How Screen Readers Interact with Terminals

Screen readers like **NVDA** (Windows), **VoiceOver** (macOS), and **Orca** (Linux) interact with terminal applications through the terminal emulator, not through your application directly. This has important implications:

1. **Screen readers read the terminal buffer.** They access the grid of characters that the terminal emulator maintains. Your escape sequences for cursor movement and styling are interpreted by the terminal emulator; the screen reader sees the resulting text.

2. **Screen readers track cursor position.** When the cursor moves, many screen readers announce the text at the new position. Rapid cursor movement (as happens during full-screen redraw) can produce a flood of announcements.

3. **Screen readers cannot see semantic structure.** Your TUI might visually distinguish a "header" from a "list item" using colors and box-drawing characters, but the screen reader sees only flat text and box-drawing Unicode characters.

```rust
use std::env;

fn is_screen_reader_likely() -> bool {
    // Some screen readers set environment hints
    // NVDA on Windows, for instance, does not set a standard variable
    // but some setups use TERM_PROGRAM or accessibility-specific vars

    // Check for accessibility-related environment variables
    let indicators = [
        "ACCESSIBILITY",
        "SCREEN_READER",
        "TUI_ACCESSIBLE",
    ];

    for var in &indicators {
        if env::var(var).is_ok() {
            return true;
        }
    }

    // Check TERM for indicators that suggest limited display
    let term = env::var("TERM").unwrap_or_default();
    if term == "dumb" {
        return true; // Might be a screen reader or very limited terminal
    }

    false
}

fn main() {
    if is_screen_reader_likely() {
        println!("Screen reader detected. Using accessible output mode.");
    } else {
        println!("Standard output mode.");
    }

    println!();
    println!("Accessibility best practices:");
    println!("1. Provide text alternatives for visual indicators");
    println!("2. Minimize rapid screen updates");
    println!("3. Use clear, descriptive status messages");
    println!("4. Support keyboard-only navigation");
}
```

## The NO_COLOR Standard

The `NO_COLOR` convention (https://no-color.org/) is a simple but important accessibility standard. When the `NO_COLOR` environment variable is set (to any value), applications should not output ANSI color escape sequences. This helps users who:

- Use screen readers (colors are invisible to screen readers and the escape sequences can cause noise)
- Have visual impairments that make certain color combinations unreadable
- Work in terminals that do not support colors
- Pipe output to files or other programs

```rust
use std::env;

#[derive(Debug)]
struct StyleConfig {
    use_colors: bool,
    use_bold: bool,
    use_borders: bool,
    status_format: StatusFormat,
}

#[derive(Debug)]
enum StatusFormat {
    /// [OK] Success -- text-based indicators
    TextBased,
    /// Green checkmark and colored text
    Visual,
}

impl StyleConfig {
    fn from_environment() -> Self {
        let no_color = env::var("NO_COLOR").is_ok();
        let term = env::var("TERM").unwrap_or_default();
        let is_dumb = term == "dumb" || term.is_empty();

        if no_color || is_dumb {
            Self {
                use_colors: false,
                use_bold: !is_dumb, // Bold often works even without color
                use_borders: !is_dumb,
                status_format: StatusFormat::TextBased,
            }
        } else {
            Self {
                use_colors: true,
                use_bold: true,
                use_borders: true,
                status_format: StatusFormat::Visual,
            }
        }
    }
}

fn render_status(config: &StyleConfig, is_success: bool) -> String {
    match config.status_format {
        StatusFormat::TextBased => {
            if is_success {
                "[OK] Operation completed successfully".to_string()
            } else {
                "[ERROR] Operation failed".to_string()
            }
        }
        StatusFormat::Visual => {
            if is_success {
                "\x1b[32m\u{2714} Success\x1b[0m".to_string()
            } else {
                "\x1b[31m\u{2718} Failed\x1b[0m".to_string()
            }
        }
    }
}

fn main() {
    let config = StyleConfig::from_environment();
    println!("Style config: {:?}", config);
    println!();

    let success_msg = render_status(&config, true);
    let failure_msg = render_status(&config, false);
    println!("Success: {}", success_msg);
    println!("Failure: {}", failure_msg);
}
```

::: tip Coming from Python
Python's Rich library respects `NO_COLOR` automatically. When `NO_COLOR` is set, `Console()` disables all color output. Rich also detects when output is being piped (`not sys.stdout.isatty()`) and strips formatting. In Rust, you need to implement these checks yourself. The `supports-color` crate provides detection, and crossterm respects `NO_COLOR` in its styling functions, but your application logic must also adapt (for example, using text-based status indicators instead of colored symbols).
:::

## Keyboard-Only Navigation

Mouse-free navigation is an accessibility requirement and a power-user feature. Every interactive element in your TUI must be reachable and operable via keyboard alone:

```rust
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

#[derive(Debug, Clone, Copy, PartialEq)]
enum FocusTarget {
    ChatPanel,
    InputField,
    ToolPanel,
    FileExplorer,
    HelpOverlay,
}

struct AccessibleNavigation {
    focus: FocusTarget,
    focus_order: Vec<FocusTarget>,
    focus_index: usize,
}

impl AccessibleNavigation {
    fn new() -> Self {
        let focus_order = vec![
            FocusTarget::InputField,
            FocusTarget::ChatPanel,
            FocusTarget::ToolPanel,
            FocusTarget::FileExplorer,
        ];
        Self {
            focus: FocusTarget::InputField,
            focus_order,
            focus_index: 0,
        }
    }

    /// Move focus to the next element (Tab)
    fn focus_next(&mut self) {
        self.focus_index = (self.focus_index + 1) % self.focus_order.len();
        self.focus = self.focus_order[self.focus_index];
    }

    /// Move focus to the previous element (Shift+Tab)
    fn focus_previous(&mut self) {
        if self.focus_index == 0 {
            self.focus_index = self.focus_order.len() - 1;
        } else {
            self.focus_index -= 1;
        }
        self.focus = self.focus_order[self.focus_index];
    }

    /// Jump directly to a panel (keyboard shortcut)
    fn focus_target(&mut self, target: FocusTarget) {
        if let Some(idx) = self.focus_order.iter().position(|t| *t == target) {
            self.focus_index = idx;
            self.focus = target;
        }
    }

    /// Get the announcement text for the current focus
    /// (useful for screen reader users and status bar display)
    fn focus_announcement(&self) -> &str {
        match self.focus {
            FocusTarget::ChatPanel => "Chat panel focused. Use Up/Down to scroll.",
            FocusTarget::InputField => "Input field focused. Type your message.",
            FocusTarget::ToolPanel => "Tool output panel focused. Use Up/Down to scroll.",
            FocusTarget::FileExplorer => "File explorer focused. Use Up/Down to navigate.",
            FocusTarget::HelpOverlay => "Help panel. Press Escape to close.",
        }
    }

    fn handle_key(&mut self, key: KeyEvent) -> bool {
        match (key.code, key.modifiers) {
            (KeyCode::Tab, KeyModifiers::NONE) => {
                self.focus_next();
                true
            }
            (KeyCode::BackTab, KeyModifiers::SHIFT) => {
                self.focus_previous();
                true
            }
            // Direct jump shortcuts
            (KeyCode::Char('1'), KeyModifiers::ALT) => {
                self.focus_target(FocusTarget::ChatPanel);
                true
            }
            (KeyCode::Char('2'), KeyModifiers::ALT) => {
                self.focus_target(FocusTarget::InputField);
                true
            }
            (KeyCode::Char('3'), KeyModifiers::ALT) => {
                self.focus_target(FocusTarget::ToolPanel);
                true
            }
            _ => false,
        }
    }
}

fn main() {
    let mut nav = AccessibleNavigation::new();
    println!("Current focus: {:?}", nav.focus);
    println!("Announcement: {}", nav.focus_announcement());

    nav.focus_next();
    println!("\nAfter Tab:");
    println!("Focus: {:?}", nav.focus);
    println!("Announcement: {}", nav.focus_announcement());

    nav.focus_next();
    println!("\nAfter Tab:");
    println!("Focus: {:?}", nav.focus);
    println!("Announcement: {}", nav.focus_announcement());
}
```

## Visual Focus Indicators

The focused panel must be visually distinct. Color alone is not sufficient -- colorblind users and users with `NO_COLOR` set need alternative indicators:

```rust
use ratatui::{
    style::{Color, Modifier, Style},
    widgets::{Block, BorderType, Borders},
};

fn focused_block(title: &str, is_focused: bool, use_color: bool) -> Block<'_> {
    let block = Block::default()
        .borders(Borders::ALL)
        .title(title);

    if is_focused {
        if use_color {
            // Colored focus: bright border + bold title
            block
                .border_type(BorderType::Thick)
                .border_style(Style::default().fg(Color::Cyan))
                .title_style(Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD))
        } else {
            // Colorless focus: thick/double border + marker
            block
                .border_type(BorderType::Double)
                .title(format!("* {} *", title))
        }
    } else {
        if use_color {
            block
                .border_style(Style::default().fg(Color::DarkGray))
                .title_style(Style::default().fg(Color::DarkGray))
        } else {
            block
                .border_type(BorderType::Plain)
        }
    }
}

fn main() {
    let focused_color = focused_block("Chat", true, true);
    let unfocused_color = focused_block("Tools", false, true);
    let focused_no_color = focused_block("Chat", true, false);
    let unfocused_no_color = focused_block("Tools", false, false);

    println!("Focus indicators:");
    println!("  With color: thick cyan border + bold title");
    println!("  Without color: double border + asterisk markers (* Title *)");
    println!("  Both approaches work for all users.");
}
```

## Colorblind-Safe Palettes

Approximately 8% of men and 0.5% of women have some form of color vision deficiency. Avoid relying on red/green distinction as the sole indicator of status:

```rust
use ratatui::style::Color;

/// Status colors that are distinguishable for colorblind users
struct AccessiblePalette {
    success: Color,
    error: Color,
    warning: Color,
    info: Color,
}

impl AccessiblePalette {
    fn colorblind_safe() -> Self {
        Self {
            // Use blue/orange instead of green/red
            // These are distinguishable by most colorblind users
            success: Color::Rgb(0, 158, 115),    // Teal-green
            error: Color::Rgb(213, 94, 0),        // Vermillion
            warning: Color::Rgb(240, 228, 66),    // Yellow
            info: Color::Rgb(86, 180, 233),        // Sky blue
        }
    }

    fn high_contrast() -> Self {
        Self {
            success: Color::White,
            error: Color::White,
            warning: Color::White,
            info: Color::White,
        }
    }
}

/// Render a status indicator with both color and text
fn status_indicator(status: &str, is_success: bool, palette: &AccessiblePalette) -> String {
    // Always include text alongside color -- never use color alone
    if is_success {
        format!("[PASS] {}", status)
    } else {
        format!("[FAIL] {}", status)
    }
}

fn main() {
    let palette = AccessiblePalette::colorblind_safe();
    println!("Colorblind-safe palette uses blue/orange instead of green/red.");
    println!("Always pair color with text labels or shape indicators:");
    println!("  {} (colored teal-green)", status_indicator("Build complete", true, &palette));
    println!("  {} (colored vermillion)", status_indicator("Tests failed", false, &palette));
}
```

## Reducing Screen Reader Noise

Full-screen TUI redraws cause screen readers to announce a flood of text. Several strategies reduce this noise:

```rust
/// Strategies for reducing screen reader noise

struct AccessibleRenderer {
    /// Only send important updates to the status line
    /// (screen readers typically track the cursor line)
    last_status: String,

    /// Throttle updates to reduce announcement frequency
    min_update_interval_ms: u64,

    /// Track whether we are in "streaming" mode
    /// where rapid updates should be batched
    is_streaming: bool,
}

impl AccessibleRenderer {
    fn new() -> Self {
        Self {
            last_status: String::new(),
            min_update_interval_ms: 500,
            is_streaming: false,
        }
    }

    /// Update the status line only when the message changes
    fn update_status(&mut self, message: &str) -> bool {
        if message != self.last_status {
            self.last_status = message.to_string();
            true // Caller should redraw the status line
        } else {
            false
        }
    }

    /// During streaming, batch updates and only redraw periodically
    fn should_redraw_during_streaming(&self, ms_since_last: u64) -> bool {
        if !self.is_streaming {
            return true; // Always redraw when not streaming
        }
        ms_since_last >= self.min_update_interval_ms
    }
}

fn main() {
    let mut renderer = AccessibleRenderer::new();

    // First status update: triggers redraw
    let changed = renderer.update_status("Agent processing...");
    println!("Status changed (should redraw): {}", changed);

    // Same status: no redraw needed
    let changed = renderer.update_status("Agent processing...");
    println!("Status unchanged (skip redraw): {}", changed);

    // New status: triggers redraw
    let changed = renderer.update_status("Tool executing: cargo check");
    println!("Status changed (should redraw): {}", changed);
}
```

::: wild In the Wild
Claude Code provides a non-interactive output mode for environments where a full TUI is inappropriate -- piped output, CI systems, and screen reader setups. When running non-interactively, it outputs plain text with clear structure markers instead of ANSI-styled screen redraws. This dual-mode approach (rich TUI when appropriate, plain text when not) is the gold standard for accessible terminal applications.
:::

## Providing an Accessible Mode

The most pragmatic approach for accessibility is to provide an explicit accessible mode that uses simplified output:

```rust
use std::env;

#[derive(Debug, Clone, Copy)]
enum OutputMode {
    /// Full TUI with colors, borders, and widgets
    FullTui,
    /// Simplified TUI with reduced visual complexity
    SimplifiedTui,
    /// Plain text output, no escape sequences
    PlainText,
}

fn detect_output_mode() -> OutputMode {
    // Explicit user preference takes priority
    if let Ok(mode) = env::var("AGENT_OUTPUT_MODE") {
        return match mode.as_str() {
            "plain" => OutputMode::PlainText,
            "simple" => OutputMode::SimplifiedTui,
            "full" => OutputMode::FullTui,
            _ => OutputMode::FullTui,
        };
    }

    // Detect accessibility indicators
    if env::var("NO_COLOR").is_ok() {
        return OutputMode::SimplifiedTui;
    }

    let term = env::var("TERM").unwrap_or_default();
    if term == "dumb" || term.is_empty() {
        return OutputMode::PlainText;
    }

    // Check if output is not a terminal (piped)
    if !atty_check() {
        return OutputMode::PlainText;
    }

    OutputMode::FullTui
}

fn atty_check() -> bool {
    // In production, use the `atty` crate or std::io::IsTerminal
    // Simplified check for demonstration
    true
}

fn main() {
    let mode = detect_output_mode();
    println!("Detected output mode: {:?}", mode);

    match mode {
        OutputMode::FullTui => {
            println!("Starting full TUI with Ratatui...");
        }
        OutputMode::SimplifiedTui => {
            println!("Starting simplified TUI (reduced colors, text indicators)...");
        }
        OutputMode::PlainText => {
            println!("Starting plain text mode (no escape sequences)...");
        }
    }
}
```

## Key Takeaways

- Screen readers interact with terminal applications through the terminal emulator's character buffer, so your TUI's visual structure (borders, colors) is invisible to them -- always include text-based indicators alongside visual ones.
- Respect the `NO_COLOR` environment variable by disabling color output when it is set, and detect `TERM=dumb` to fall back to plain text output entirely.
- Every interactive element must be reachable via keyboard alone using Tab/Shift+Tab for focus cycling and direct-jump shortcuts (Alt+1, Alt+2, etc.) for power users.
- Never rely on color alone to convey information -- pair colors with text labels (like `[PASS]`/`[FAIL]`), shapes, or border styles. Use colorblind-safe palettes (blue/orange instead of green/red).
- Provide an explicit output mode setting (`AGENT_OUTPUT_MODE=plain`) so users can opt into simplified or plain text output when the full TUI is inaccessible or inappropriate.
