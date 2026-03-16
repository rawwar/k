// Chapter 9: Conversation Context Management — Code snapshot
//
// Builds on ch08 (Terminal UI) by adding context window management:
// - Token counting via word-based estimation
// - ContextManager that tracks per-message token usage
// - Context budget tracker (model limit - reserved - system prompt = available)
// - Message pruning: when approaching the limit, drop oldest low-priority messages
// - /context command showing current context usage
// - Messages stored with estimated token counts

use std::io;

use crossterm::event::{self, Event, KeyCode, KeyModifiers};
use crossterm::terminal::{disable_raw_mode, enable_raw_mode};
use ratatui::layout::{Constraint, Direction, Layout};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{Block, Borders, Paragraph, Wrap};
use ratatui::Frame;
use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Token estimation
// ---------------------------------------------------------------------------

/// Estimate token count for a string using a word-based approximation.
/// English text averages roughly 1.3 tokens per word. Code tends to run
/// higher because punctuation and operators each become separate tokens.
fn estimate_tokens(text: &str) -> usize {
    if text.is_empty() {
        return 0;
    }
    let words = text.split_whitespace().count();
    let code_chars = text
        .chars()
        .filter(|c| matches!(c, '{' | '}' | '(' | ')' | ';' | '<' | '>' | '=' | ':' | '"'))
        .count();
    // Base: ~1.3 tokens per word, plus one extra token per 3 code-punctuation chars
    let base = ((words as f64) * 1.3).ceil() as usize;
    let code_extra = code_chars / 3;
    (base + code_extra).max(1)
}

/// Estimate tokens for a full API message (role + content + framing overhead).
fn estimate_message_tokens(role: &str, content: &str) -> usize {
    let overhead = 4; // role markers and message boundary tokens
    estimate_tokens(role) + estimate_tokens(content) + overhead
}

// ---------------------------------------------------------------------------
// Budget status
// ---------------------------------------------------------------------------

/// How close the context window is to its limit.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BudgetStatus {
    /// Plenty of room
    Healthy,
    /// Over 80 % used
    Warning,
    /// Over 95 % used — compact now
    Critical,
    /// Over budget — must compact before next request
    Exceeded,
}

impl std::fmt::Display for BudgetStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BudgetStatus::Healthy => write!(f, "Healthy"),
            BudgetStatus::Warning => write!(f, "Warning"),
            BudgetStatus::Critical => write!(f, "CRITICAL"),
            BudgetStatus::Exceeded => write!(f, "EXCEEDED"),
        }
    }
}

// ---------------------------------------------------------------------------
// Message types
// ---------------------------------------------------------------------------

/// Priority level for compaction decisions. Higher = harder to remove.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
enum Priority {
    /// Verbose tool output — first to be pruned
    Low = 0,
    /// Normal assistant responses
    Normal = 1,
    /// User messages and key decisions
    High = 2,
    /// System prompt — never removed
    Pinned = 3,
}

/// A single message stored with its estimated token cost.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct Message {
    role: String,
    content: String,
    /// Estimated token count (including message-framing overhead)
    token_count: usize,
    /// Compaction priority
    priority: Priority,
    /// True if this message is a compaction summary
    is_summary: bool,
}

impl Message {
    fn new(role: &str, content: &str) -> Self {
        let token_count = estimate_message_tokens(role, content);
        let priority = match role {
            "system" => Priority::Pinned,
            "user" => Priority::High,
            "assistant" => Priority::Normal,
            "tool" => Priority::Low,
            _ => Priority::Normal,
        };
        Self {
            role: role.to_string(),
            content: content.to_string(),
            token_count,
            priority,
            is_summary: false,
        }
    }

    /// Create a compaction summary that replaces pruned messages.
    fn summary(content: &str) -> Self {
        let token_count = estimate_message_tokens("system", content);
        Self {
            role: "system".to_string(),
            content: content.to_string(),
            token_count,
            priority: Priority::Normal,
            is_summary: true,
        }
    }
}

// ---------------------------------------------------------------------------
// Context manager
// ---------------------------------------------------------------------------

/// Manages the conversation context window, including token counting,
/// budget tracking, and automatic pruning when approaching the limit.
struct ContextManager {
    messages: Vec<Message>,
    /// Hard model token limit (e.g. 200 000 for Sonnet)
    model_limit: usize,
    /// Tokens reserved for the model's response
    response_reserve: usize,
    /// Tokens consumed by tool definitions (constant per session)
    tool_definition_tokens: usize,
    /// Small safety margin to avoid edge-case overflows
    safety_margin: usize,
}

impl ContextManager {
    fn new(model_limit: usize) -> Self {
        Self {
            messages: Vec::new(),
            model_limit,
            response_reserve: 8_000,
            tool_definition_tokens: 3_000,
            safety_margin: 200,
        }
    }

    /// Add the initial system prompt (pinned, never pruned).
    fn set_system_prompt(&mut self, content: &str) {
        // Remove any previous system prompt
        self.messages.retain(|m| m.role != "system" || m.is_summary);
        let mut msg = Message::new("system", content);
        msg.priority = Priority::Pinned;
        self.messages.insert(0, msg);
    }

    /// Add a message and prune if the budget is critical or exceeded.
    fn add_message(&mut self, role: &str, content: &str) {
        let msg = Message::new(role, content);
        self.messages.push(msg);

        // Auto-prune when we cross into Critical territory
        let status = self.budget_status();
        if status == BudgetStatus::Critical || status == BudgetStatus::Exceeded {
            self.prune_to_target(0.70);
        }
    }

    // -- Budget bookkeeping --------------------------------------------------

    /// Sum of estimated tokens across all stored messages.
    fn message_tokens(&self) -> usize {
        self.messages.iter().map(|m| m.token_count).sum()
    }

    /// Tokens for the system prompt alone (the first pinned system message).
    fn system_prompt_tokens(&self) -> usize {
        self.messages
            .iter()
            .find(|m| m.role == "system" && m.priority == Priority::Pinned)
            .map_or(0, |m| m.token_count)
    }

    /// Total tokens committed (messages + response reserve + tools + margin).
    fn total_committed(&self) -> usize {
        self.message_tokens()
            + self.response_reserve
            + self.tool_definition_tokens
            + self.safety_margin
    }

    /// Tokens still available for new messages.
    fn available(&self) -> usize {
        self.model_limit.saturating_sub(self.total_committed())
    }

    /// Utilisation ratio (0.0 – 1.0+).
    fn utilization(&self) -> f64 {
        self.total_committed() as f64 / self.model_limit as f64
    }

    fn budget_status(&self) -> BudgetStatus {
        let u = self.utilization();
        if u > 1.0 {
            BudgetStatus::Exceeded
        } else if u > 0.95 {
            BudgetStatus::Critical
        } else if u > 0.80 {
            BudgetStatus::Warning
        } else {
            BudgetStatus::Healthy
        }
    }

    // -- Pruning / compaction ------------------------------------------------

    /// Prune low-priority messages until utilization drops to `target_ratio`.
    /// Strategy: remove oldest messages with the lowest priority first,
    /// then insert a one-line summary of what was dropped.
    fn prune_to_target(&mut self, target_ratio: f64) {
        let target_total = (self.model_limit as f64 * target_ratio) as usize;
        if self.total_committed() <= target_total {
            return;
        }
        let tokens_to_free = self.total_committed() - target_total;

        // Build candidate list: (index, priority, token_count). Skip pinned.
        let mut candidates: Vec<(usize, Priority, usize)> = self
            .messages
            .iter()
            .enumerate()
            .filter(|(_, m)| m.priority != Priority::Pinned)
            .map(|(i, m)| (i, m.priority, m.token_count))
            .collect();

        // Sort: lowest priority first, then oldest first (lowest index).
        candidates.sort_by(|a, b| a.1.cmp(&b.1).then(a.0.cmp(&b.0)));

        let mut freed = 0usize;
        let mut remove_set = std::collections::HashSet::new();
        for (idx, _prio, tok) in &candidates {
            if freed >= tokens_to_free {
                break;
            }
            remove_set.insert(*idx);
            freed += tok;
        }

        if remove_set.is_empty() {
            return;
        }

        let removed_count = remove_set.len();
        // Collect indices in reverse order so we can splice without shifting.
        let mut to_remove: Vec<usize> = remove_set.into_iter().collect();
        to_remove.sort_unstable_by(|a, b| b.cmp(a));
        for idx in &to_remove {
            self.messages.remove(*idx);
        }

        // Insert a brief summary so the model knows context was dropped.
        let summary_text = format!(
            "[Context compacted: {removed_count} older messages pruned to free ~{freed} tokens]"
        );
        let summary = Message::summary(&summary_text);
        // Place the summary right after the system prompt (index 1).
        let insert_pos = if self.messages.first().map_or(false, |m| {
            m.role == "system" && m.priority == Priority::Pinned
        }) {
            1
        } else {
            0
        };
        self.messages.insert(insert_pos, summary);
    }

    // -- Rendering helpers ---------------------------------------------------

    /// Build a multi-line context usage report (shown by /context).
    fn usage_report(&self) -> Vec<String> {
        let mut lines = Vec::new();
        lines.push(format!("--- Context Budget ---"));
        lines.push(format!(
            "Model limit:         {:>8} tokens",
            self.model_limit
        ));
        lines.push(format!(
            "System prompt:       {:>8} tokens",
            self.system_prompt_tokens()
        ));
        lines.push(format!(
            "Tool definitions:    {:>8} tokens",
            self.tool_definition_tokens
        ));
        lines.push(format!(
            "Conversation msgs:   {:>8} tokens  ({} messages)",
            self.message_tokens(),
            self.messages.len()
        ));
        lines.push(format!(
            "Response reserve:    {:>8} tokens",
            self.response_reserve
        ));
        lines.push(format!(
            "Safety margin:       {:>8} tokens",
            self.safety_margin
        ));
        lines.push(format!("------------------------------"));
        lines.push(format!(
            "Total committed:     {:>8} tokens",
            self.total_committed()
        ));
        lines.push(format!(
            "Available:           {:>8} tokens",
            self.available()
        ));
        lines.push(format!(
            "Utilization:         {:>7.1} %   [{}]",
            self.utilization() * 100.0,
            self.budget_status()
        ));
        lines.push(String::new());
        lines.push("Per-message breakdown:".to_string());
        for (i, msg) in self.messages.iter().enumerate() {
            let label = if msg.is_summary {
                "[summary]".to_string()
            } else {
                msg.role.clone()
            };
            let preview: String = msg.content.chars().take(50).collect();
            let preview = preview.replace('\n', " ");
            lines.push(format!(
                "  {:>3}. {:>6} tok  {:<12} {}",
                i + 1,
                msg.token_count,
                label,
                preview
            ));
        }
        lines
    }

    /// Get the messages slice (for building API requests).
    fn get_messages(&self) -> &[Message] {
        &self.messages
    }
}

// ---------------------------------------------------------------------------
// Application (builds on ch08 TUI)
// ---------------------------------------------------------------------------

struct App {
    should_quit: bool,
    input: String,
    /// Display lines — interleaved user/assistant/system output.
    display_lines: Vec<(String, Color)>,
    /// Scroll offset for the conversation pane.
    scroll: u16,
    /// The context manager that owns the conversation.
    ctx: ContextManager,
}

impl App {
    fn new() -> Self {
        let mut ctx = ContextManager::new(200_000);
        ctx.set_system_prompt(
            "You are a helpful coding assistant. Answer concisely and use code examples when appropriate."
        );

        let mut app = Self {
            should_quit: false,
            input: String::new(),
            display_lines: Vec::new(),
            scroll: 0,
            ctx,
        };
        app.push_display(
            "Chapter 9: Context Management — type a message, /context to inspect, /help for commands, Ctrl-C to quit."
                .to_string(),
            Color::DarkGray,
        );
        app
    }

    fn push_display(&mut self, line: String, color: Color) {
        self.display_lines.push((line, color));
    }

    fn handle_key(&mut self, code: KeyCode, modifiers: KeyModifiers) {
        match (code, modifiers) {
            (KeyCode::Char('c'), KeyModifiers::CONTROL) => self.should_quit = true,
            (KeyCode::Char(c), _) => self.input.push(c),
            (KeyCode::Backspace, _) => {
                self.input.pop();
            }
            (KeyCode::Enter, _) => self.submit(),
            (KeyCode::Up, _) => {
                self.scroll = self.scroll.saturating_add(1);
            }
            (KeyCode::Down, _) => {
                self.scroll = self.scroll.saturating_sub(1);
            }
            _ => {}
        }
    }

    fn submit(&mut self) {
        let input = self.input.trim().to_string();
        self.input.clear();
        if input.is_empty() {
            return;
        }

        // -- slash commands --------------------------------------------------
        if input.starts_with('/') {
            match input.as_str() {
                "/context" | "/tokens" => {
                    let report = self.ctx.usage_report();
                    for line in report {
                        self.push_display(line, Color::Cyan);
                    }
                    return;
                }
                "/clear" => {
                    self.display_lines.clear();
                    self.ctx = ContextManager::new(self.ctx.model_limit);
                    self.ctx.set_system_prompt(
                        "You are a helpful coding assistant. Answer concisely and use code examples when appropriate."
                    );
                    self.push_display("Conversation cleared.".to_string(), Color::Yellow);
                    return;
                }
                "/help" => {
                    self.push_display("/context  — show context window usage".to_string(), Color::Green);
                    self.push_display("/tokens   — alias for /context".to_string(), Color::Green);
                    self.push_display("/clear    — reset the conversation".to_string(), Color::Green);
                    self.push_display("/help     — show this help".to_string(), Color::Green);
                    self.push_display("Ctrl-C    — quit".to_string(), Color::Green);
                    self.push_display("Up/Down   — scroll conversation".to_string(), Color::Green);
                    return;
                }
                other => {
                    self.push_display(
                        format!("Unknown command: {other}. Type /help for options."),
                        Color::Red,
                    );
                    return;
                }
            }
        }

        // -- Normal user message ---------------------------------------------
        self.push_display(format!("You: {input}"), Color::White);
        self.ctx.add_message("user", &input);

        // Show a budget status hint if we are in Warning or worse.
        let status = self.ctx.budget_status();
        if status == BudgetStatus::Warning || status == BudgetStatus::Critical {
            self.push_display(
                format!(
                    "[context {:.0}% — status: {}]",
                    self.ctx.utilization() * 100.0,
                    status
                ),
                Color::Yellow,
            );
        }

        // Simulate an assistant reply (in a real agent this would call the API).
        let reply = format!(
            "(simulated) I received your message ({} tokens est). Context is at {:.1}%.",
            estimate_tokens(&input),
            self.ctx.utilization() * 100.0
        );
        self.push_display(format!("Assistant: {reply}"), Color::Magenta);
        self.ctx.add_message("assistant", &reply);
    }

    fn draw(&self, frame: &mut Frame) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Min(3),      // conversation
                Constraint::Length(3),    // input
                Constraint::Length(1),    // status bar
            ])
            .split(frame.area());

        // -- Conversation pane -----------------------------------------------
        let conv_lines: Vec<Line> = self
            .display_lines
            .iter()
            .map(|(text, color)| Line::from(Span::styled(text.clone(), Style::default().fg(*color))))
            .collect();
        let conv = Paragraph::new(Text::from(conv_lines))
            .block(Block::default().borders(Borders::ALL).title(" Conversation "))
            .wrap(Wrap { trim: false })
            .scroll((self.scroll, 0));
        frame.render_widget(conv, chunks[0]);

        // -- Input box -------------------------------------------------------
        let input_text = Line::from(vec![
            Span::styled("> ", Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)),
            Span::raw(&self.input),
        ]);
        let input_widget = Paragraph::new(input_text)
            .block(Block::default().borders(Borders::ALL).title(" Input "));
        frame.render_widget(input_widget, chunks[1]);

        // -- Status bar ------------------------------------------------------
        let status = self.ctx.budget_status();
        let status_color = match status {
            BudgetStatus::Healthy => Color::Green,
            BudgetStatus::Warning => Color::Yellow,
            BudgetStatus::Critical => Color::Red,
            BudgetStatus::Exceeded => Color::LightRed,
        };
        let bar = Line::from(vec![
            Span::raw(format!(
                " msgs:{} tokens:{}/{} ({:.0}%) ",
                self.ctx.get_messages().len(),
                self.ctx.message_tokens(),
                self.ctx.model_limit,
                self.ctx.utilization() * 100.0,
            )),
            Span::styled(format!("[{}]", status), Style::default().fg(status_color)),
            Span::raw(format!("  avail:{}", self.ctx.available())),
        ]);
        let bar_widget = Paragraph::new(bar);
        frame.render_widget(bar_widget, chunks[2]);
    }
}

// ---------------------------------------------------------------------------
// Entry point
// ---------------------------------------------------------------------------

fn main() -> io::Result<()> {
    enable_raw_mode()?;
    let mut terminal = ratatui::init();
    let mut app = App::new();

    while !app.should_quit {
        terminal.draw(|frame| app.draw(frame))?;

        if let Event::Key(key_event) = event::read()? {
            app.handle_key(key_event.code, key_event.modifiers);
        }
    }

    disable_raw_mode()?;
    ratatui::restore();
    Ok(())
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn token_estimation_nonzero() {
        assert!(estimate_tokens("hello world") > 0);
        assert!(estimate_tokens("fn main() {}") > 0);
        assert!(estimate_tokens("") == 0);
    }

    #[test]
    fn code_estimates_higher_than_prose() {
        let prose = "The quick brown fox jumps over the lazy dog";
        let code = "fn main() { let x: Vec<String> = vec![]; println!(\"{:?}\", x); }";
        // Code should produce equal or more tokens for similar-length strings
        // because of the punctuation bonus.
        let prose_tok = estimate_tokens(prose);
        let code_tok = estimate_tokens(code);
        assert!(code_tok >= prose_tok);
    }

    #[test]
    fn budget_status_transitions() {
        let mut ctx = ContextManager::new(1_000);
        ctx.response_reserve = 0;
        ctx.tool_definition_tokens = 0;
        ctx.safety_margin = 0;
        assert_eq!(ctx.budget_status(), BudgetStatus::Healthy);

        // Push enough tokens to cross 80 %
        ctx.messages.push(Message {
            role: "user".to_string(),
            content: String::new(),
            token_count: 850,
            priority: Priority::High,
            is_summary: false,
        });
        assert_eq!(ctx.budget_status(), BudgetStatus::Warning);
    }

    #[test]
    fn prune_frees_tokens() {
        let mut ctx = ContextManager::new(1_000);
        ctx.response_reserve = 0;
        ctx.tool_definition_tokens = 0;
        ctx.safety_margin = 0;

        // Add a pinned system prompt
        ctx.messages.push(Message {
            role: "system".to_string(),
            content: "system".to_string(),
            token_count: 10,
            priority: Priority::Pinned,
            is_summary: false,
        });
        // Add several low-priority messages
        for _ in 0..5 {
            ctx.messages.push(Message {
                role: "tool".to_string(),
                content: "tool output".to_string(),
                token_count: 200,
                priority: Priority::Low,
                is_summary: false,
            });
        }
        let before = ctx.message_tokens();
        assert!(before > 700);

        ctx.prune_to_target(0.50);
        let after = ctx.message_tokens();
        assert!(after < before, "pruning should reduce token count");
        // System prompt should survive
        assert!(ctx.messages.iter().any(|m| m.priority == Priority::Pinned));
    }

    #[test]
    fn context_manager_auto_prunes_on_critical() {
        let mut ctx = ContextManager::new(500);
        ctx.response_reserve = 50;
        ctx.tool_definition_tokens = 50;
        ctx.safety_margin = 0;
        ctx.set_system_prompt("sys");

        // Flood with messages until auto-prune fires
        for i in 0..30 {
            ctx.add_message("user", &format!("msg {i} with some padding text to fill tokens"));
        }
        // After auto-pruning the utilization should be back under 1.0
        assert!(
            ctx.utilization() <= 1.0,
            "auto-prune should keep utilization at or below 100%"
        );
    }

    #[test]
    fn usage_report_contains_key_fields() {
        let mut ctx = ContextManager::new(200_000);
        ctx.set_system_prompt("You are helpful.");
        ctx.add_message("user", "Hello");
        let report = ctx.usage_report();
        let joined = report.join("\n");
        assert!(joined.contains("Model limit"));
        assert!(joined.contains("System prompt"));
        assert!(joined.contains("Available"));
        assert!(joined.contains("Utilization"));
    }
}
