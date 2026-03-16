---
title: Theming
description: Implement a theming system that lets users customize colors, borders, and styles to match their terminal preferences.
---

# Theming

> **What you'll learn:**
> - How to define a theme struct that maps semantic roles to colors and styles
> - How to load themes from configuration files and support light and dark presets
> - How to detect terminal background color and automatically select an appropriate theme

A good terminal application respects its environment. Some users have dark backgrounds, others have light. Some terminals render bold as bright colors, others use actual bold fonts. A theming system lets your agent look great everywhere by separating *what* gets colored from *how* it gets colored.

## The Theme Struct

Instead of hardcoding `Color::Rgb(137, 180, 250)` throughout your rendering code, define a theme struct with semantic color names:

```rust
use ratatui::prelude::*;

/// A complete color theme for the agent UI.
#[derive(Debug, Clone)]
pub struct Theme {
    // Base colors
    pub background: Color,
    pub foreground: Color,
    pub surface: Color,         // slightly lighter than background
    pub overlay: Color,         // for popups and overlays

    // Semantic colors
    pub primary: Color,         // main accent color
    pub secondary: Color,       // secondary accent
    pub success: Color,         // success states, assistant messages
    pub warning: Color,         // warnings, pending actions
    pub error: Color,           // errors, failures
    pub info: Color,            // informational elements

    // Text colors
    pub text: Color,            // primary text
    pub text_muted: Color,      // secondary/dimmed text
    pub text_accent: Color,     // highlighted text

    // UI element colors
    pub border: Color,          // default border color
    pub border_focused: Color,  // focused pane border
    pub status_bg: Color,       // status bar background
    pub input_bg: Color,        // input box background
    pub code_bg: Color,         // code block background

    // Role colors
    pub user_color: Color,      // user message accent
    pub assistant_color: Color, // assistant message accent
    pub tool_color: Color,      // tool output accent
    pub system_color: Color,    // system message accent

    // Border style
    pub border_type: ratatui::widgets::BorderType,
}
```

## Built-in Theme Presets

Define a few built-in themes that users can choose from. Catppuccin Mocha is a popular choice for dark terminals:

```rust
use ratatui::widgets::BorderType;

impl Theme {
    /// Catppuccin Mocha -- a warm dark theme.
    pub fn catppuccin_mocha() -> Self {
        Self {
            background: Color::Rgb(30, 30, 46),     // Base
            foreground: Color::Rgb(205, 214, 244),   // Text
            surface: Color::Rgb(49, 50, 68),         // Surface0
            overlay: Color::Rgb(69, 71, 90),         // Surface1

            primary: Color::Rgb(137, 180, 250),      // Blue
            secondary: Color::Rgb(180, 190, 254),    // Lavender
            success: Color::Rgb(166, 227, 161),      // Green
            warning: Color::Rgb(249, 226, 175),      // Yellow
            error: Color::Rgb(243, 139, 168),        // Red
            info: Color::Rgb(137, 220, 235),         // Sapphire

            text: Color::Rgb(205, 214, 244),         // Text
            text_muted: Color::Rgb(108, 112, 134),   // Overlay0
            text_accent: Color::Rgb(249, 226, 175),  // Yellow

            border: Color::Rgb(69, 71, 90),          // Surface1
            border_focused: Color::Rgb(137, 180, 250), // Blue
            status_bg: Color::Rgb(49, 50, 68),       // Surface0
            input_bg: Color::Rgb(30, 30, 46),        // Base
            code_bg: Color::Rgb(24, 24, 37),         // Mantle

            user_color: Color::Rgb(137, 180, 250),   // Blue
            assistant_color: Color::Rgb(166, 227, 161), // Green
            tool_color: Color::Rgb(249, 226, 175),   // Yellow
            system_color: Color::Rgb(108, 112, 134), // Overlay0

            border_type: BorderType::Rounded,
        }
    }

    /// A light theme for terminals with white backgrounds.
    pub fn light() -> Self {
        Self {
            background: Color::Rgb(239, 241, 245),   // Latte Base
            foreground: Color::Rgb(76, 79, 105),     // Latte Text
            surface: Color::Rgb(230, 233, 239),      // Latte Surface0
            overlay: Color::Rgb(220, 224, 232),      // Latte Surface1

            primary: Color::Rgb(30, 102, 245),       // Latte Blue
            secondary: Color::Rgb(114, 135, 253),    // Latte Lavender
            success: Color::Rgb(64, 160, 43),        // Latte Green
            warning: Color::Rgb(223, 142, 29),       // Latte Yellow
            error: Color::Rgb(210, 15, 57),          // Latte Red
            info: Color::Rgb(32, 159, 181),          // Latte Sapphire

            text: Color::Rgb(76, 79, 105),           // Latte Text
            text_muted: Color::Rgb(140, 143, 161),   // Latte Overlay0
            text_accent: Color::Rgb(223, 142, 29),   // Latte Yellow

            border: Color::Rgb(172, 176, 190),       // Latte Overlay2
            border_focused: Color::Rgb(30, 102, 245), // Latte Blue
            status_bg: Color::Rgb(230, 233, 239),    // Latte Surface0
            input_bg: Color::Rgb(239, 241, 245),     // Latte Base
            code_bg: Color::Rgb(230, 233, 239),      // Latte Surface0

            user_color: Color::Rgb(30, 102, 245),    // Latte Blue
            assistant_color: Color::Rgb(64, 160, 43), // Latte Green
            tool_color: Color::Rgb(223, 142, 29),    // Latte Yellow
            system_color: Color::Rgb(140, 143, 161), // Latte Overlay0

            border_type: BorderType::Rounded,
        }
    }

    /// A minimal theme using only the basic 16 terminal colors.
    /// Works on every terminal regardless of color support.
    pub fn basic() -> Self {
        Self {
            background: Color::Reset,
            foreground: Color::Reset,
            surface: Color::Reset,
            overlay: Color::DarkGray,

            primary: Color::Cyan,
            secondary: Color::Blue,
            success: Color::Green,
            warning: Color::Yellow,
            error: Color::Red,
            info: Color::Cyan,

            text: Color::Reset,
            text_muted: Color::DarkGray,
            text_accent: Color::Yellow,

            border: Color::DarkGray,
            border_focused: Color::Cyan,
            status_bg: Color::DarkGray,
            input_bg: Color::Reset,
            code_bg: Color::Reset,

            user_color: Color::Blue,
            assistant_color: Color::Green,
            tool_color: Color::Yellow,
            system_color: Color::DarkGray,

            border_type: BorderType::Plain,
        }
    }
}
```

## Using the Theme in Rendering

Pass the theme to every rendering function, replacing hardcoded colors:

```rust
fn pane_block<'a>(title: &'a str, is_focused: bool, theme: &Theme) -> Block<'a> {
    let border_color = if is_focused {
        theme.border_focused
    } else {
        theme.border
    };

    Block::default()
        .title(format!(" {} ", title))
        .borders(Borders::ALL)
        .border_type(theme.border_type)
        .border_style(Style::default().fg(border_color))
        .title_style(Style::default().fg(
            if is_focused { theme.primary } else { theme.text_muted }
        ))
}

fn render_conversation(frame: &mut Frame, app: &App, area: Rect, theme: &Theme) {
    let block = pane_block(
        "Conversation",
        app.focused_pane == FocusedPane::Conversation,
        theme,
    );

    let mut lines: Vec<Line> = Vec::new();

    for msg in &app.messages {
        let (label, color) = match msg.role {
            Role::User => ("You", theme.user_color),
            Role::Assistant => ("Agent", theme.assistant_color),
        };

        lines.push(Line::from(Span::styled(
            label.to_string(),
            Style::default().fg(color).add_modifier(Modifier::BOLD),
        )));

        for rendered_line in &msg.rendered {
            lines.push(rendered_line.clone());
        }

        lines.push(Line::from(Span::styled(
            "\u{2500}".repeat(40),
            Style::default().fg(theme.border),
        )));
        lines.push(Line::from(""));
    }

    let paragraph = Paragraph::new(lines)
        .block(block)
        .wrap(Wrap { trim: false })
        .scroll((app.conversation_scroll.offset, 0));

    frame.render_widget(paragraph, area);
}
```

::: tip Coming from Python
Python's `textual` framework has a full CSS theming system:
```python
class AgentApp(App):
    CSS = """
    Screen { background: $surface; }
    #conversation { border: solid $primary; }
    .user-message { color: $accent; }
    """
```
Ratatui does not have CSS or stylesheets. Instead, you pass a `Theme` struct through your rendering functions. This is more explicit but achieves the same goal -- centralizing color definitions so you can switch themes without touching rendering logic.
:::

## Loading Themes from Configuration

Let users specify their theme in a configuration file. Serde makes this easy:

```rust
use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct ThemeConfig {
    pub preset: Option<String>,
    pub overrides: Option<ThemeOverrides>,
}

#[derive(Debug, Deserialize)]
pub struct ThemeOverrides {
    pub primary: Option<String>,
    pub background: Option<String>,
    pub user_color: Option<String>,
    pub assistant_color: Option<String>,
    // ... other overridable fields
}

impl ThemeConfig {
    /// Load theme from configuration, with fallbacks.
    pub fn into_theme(self) -> Theme {
        // Start with a preset
        let mut theme = match self.preset.as_deref() {
            Some("light") => Theme::light(),
            Some("basic") => Theme::basic(),
            Some("mocha") | None => Theme::catppuccin_mocha(),
            Some(unknown) => {
                eprintln!("Unknown theme '{}', using default", unknown);
                Theme::catppuccin_mocha()
            }
        };

        // Apply overrides
        if let Some(overrides) = self.overrides {
            if let Some(color) = overrides.primary.and_then(|s| parse_hex_color(&s)) {
                theme.primary = color;
            }
            if let Some(color) = overrides.background.and_then(|s| parse_hex_color(&s)) {
                theme.background = color;
            }
            if let Some(color) = overrides.user_color.and_then(|s| parse_hex_color(&s)) {
                theme.user_color = color;
            }
            if let Some(color) = overrides.assistant_color.and_then(|s| parse_hex_color(&s)) {
                theme.assistant_color = color;
            }
        }

        theme
    }
}

/// Parse a hex color string like "#89b4fa" into a Ratatui Color.
fn parse_hex_color(hex: &str) -> Option<Color> {
    let hex = hex.trim_start_matches('#');
    if hex.len() != 6 {
        return None;
    }

    let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
    let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
    let b = u8::from_str_radix(&hex[4..6], 16).ok()?;

    Some(Color::Rgb(r, g, b))
}
```

A user's configuration file (in TOML) might look like:

```toml
[theme]
preset = "mocha"

[theme.overrides]
primary = "#cba6f7"    # Use purple instead of blue for primary
```

## Auto-Detecting Dark vs. Light

Some terminals support querying the background color via the `\x1B]11;?\x07` escape sequence. This is unreliable across terminals, so a practical approach checks environment variables:

```rust
impl Theme {
    /// Select a theme based on terminal environment.
    pub fn auto_detect() -> Self {
        // Check for explicit theme preference
        if let Ok(theme) = std::env::var("AGENT_THEME") {
            return match theme.as_str() {
                "light" => Self::light(),
                "dark" | "mocha" => Self::catppuccin_mocha(),
                "basic" => Self::basic(),
                _ => Self::catppuccin_mocha(),
            };
        }

        // Check COLORFGBG (set by some terminals like rxvt)
        // Format: "foreground;background" where higher background = light
        if let Ok(colorfgbg) = std::env::var("COLORFGBG") {
            if let Some(bg) = colorfgbg.split(';').last() {
                if let Ok(bg_num) = bg.parse::<u8>() {
                    if bg_num > 8 {
                        return Self::light();
                    }
                }
            }
        }

        // Check for macOS light mode
        if cfg!(target_os = "macos") {
            if let Ok(output) = std::process::Command::new("defaults")
                .args(["read", "-g", "AppleInterfaceStyle"])
                .output()
            {
                // If the command fails or returns empty, it means light mode
                if !output.status.success() {
                    return Self::light();
                }
            }
        }

        // Default to dark theme
        Self::catppuccin_mocha()
    }
}
```

## Storing the Theme in the App

The theme is part of your application state. Store it in the `App` struct and pass it to rendering functions:

```rust
pub struct App {
    pub theme: Theme,
    // ... other fields
}

pub fn view(frame: &mut Frame, app: &App) {
    let layout = AgentLayout::compute(frame.area(), app.show_sidebar);

    render_conversation(frame, app, layout.conversation, &app.theme);
    render_input_box(frame, app, layout.input, &app.theme);
    render_status_bar(frame, &app.status, layout.status_bar, &app.theme);

    if let Some(sidebar) = layout.tool_sidebar {
        render_tool_sidebar(frame, app, sidebar, &app.theme);
    }
}
```

::: tip In the Wild
Claude Code adapts to the user's terminal automatically, detecting whether the terminal has a dark or light background and adjusting colors accordingly. OpenCode supports multiple theme presets (catppuccin, dracula, tokyonight) that users can select in their configuration file. Both agents understand that a theme mismatch (dark colors on a light background) makes the tool unusable, so automatic detection with manual override is the standard approach.
:::

## Key Takeaways

- **A Theme struct** maps semantic roles (primary, success, error, user, assistant) to colors, replacing hardcoded color values throughout rendering code.
- **Built-in presets** (dark, light, basic) provide sensible defaults; the basic preset uses only the 16 standard ANSI colors for maximum terminal compatibility.
- **Configuration-driven overrides** let users customize individual colors while inheriting the rest from a preset, using hex color strings parsed into `Color::Rgb`.
- **Auto-detection** checks environment variables (`AGENT_THEME`, `COLORFGBG`) and OS settings to select an appropriate theme without user intervention.
- **Passing the theme** through rendering functions keeps color decisions centralized and makes it easy to switch themes at runtime or add new presets.
