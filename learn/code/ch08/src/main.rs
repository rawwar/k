// Chapter 8: Terminal UI — Code snapshot

use std::io;

use crossterm::event::{self, Event, KeyCode};
use crossterm::terminal::{disable_raw_mode, enable_raw_mode};
use ratatui::DefaultTerminal;
use ratatui::text::Text;
use ratatui::widgets::Paragraph;

/// Application state.
struct App {
    should_quit: bool,
    // TODO: Add conversation history
    // TODO: Add input buffer
    // TODO: Add scroll state
}

impl App {
    fn new() -> Self {
        Self {
            should_quit: false,
        }
    }

    fn handle_key(&mut self, key: KeyCode) {
        match key {
            KeyCode::Char('q') => self.should_quit = true,
            // TODO: Handle text input, enter to submit, scrolling
            _ => {}
        }
    }

    fn draw(&self, frame: &mut ratatui::Frame) {
        // TODO: Layout with input area, conversation area, status bar
        let text = Text::raw("Chapter 8: Terminal UI — Press 'q' to quit");
        let paragraph = Paragraph::new(text);
        frame.render_widget(paragraph, frame.area());
    }
}

fn main() -> io::Result<()> {
    enable_raw_mode()?;
    let mut terminal = ratatui::init();
    let mut app = App::new();

    while !app.should_quit {
        terminal.draw(|frame| app.draw(frame))?;

        if let Event::Key(key) = event::read()? {
            app.handle_key(key.code);
        }
    }

    disable_raw_mode()?;
    ratatui::restore();
    Ok(())
}
