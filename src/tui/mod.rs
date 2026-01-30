//! CODEX//XTREME TUI - Neo Tokyo Y2K Edition
//!
//! A cyberpunk-themed terminal interface for building patched Codex binaries.

pub mod effects;
pub mod screens;
pub mod theme;
pub mod widgets;

use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEventKind},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Terminal};
use std::io::{self, Stdout};
use std::time::Duration;
use tokio::sync::mpsc;

/// Terminal wrapper with RAII cleanup
pub struct Tui {
    terminal: Terminal<CrosstermBackend<Stdout>>,
}

impl Tui {
    /// Initialize the terminal in raw mode
    pub fn new() -> io::Result<Self> {
        enable_raw_mode()?;
        let mut stdout = io::stdout();
        execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
        let backend = CrosstermBackend::new(stdout);
        let terminal = Terminal::new(backend)?;
        Ok(Self { terminal })
    }

    /// Get mutable reference to terminal
    pub fn terminal(&mut self) -> &mut Terminal<CrosstermBackend<Stdout>> {
        &mut self.terminal
    }

    /// Restore terminal to normal state
    pub fn restore(&mut self) -> io::Result<()> {
        disable_raw_mode()?;
        execute!(
            self.terminal.backend_mut(),
            LeaveAlternateScreen,
            DisableMouseCapture
        )?;
        self.terminal.show_cursor()?;
        Ok(())
    }
}

impl Drop for Tui {
    fn drop(&mut self) {
        let _ = self.restore();
    }
}

/// Terminal events
#[derive(Debug, Clone)]
pub enum TermEvent {
    Key(KeyCode),
    Resize(u16, u16),
    Tick,
}

/// Spawn async event reader for animations and input
pub fn spawn_event_reader() -> mpsc::UnboundedReceiver<TermEvent> {
    let (tx, rx) = mpsc::unbounded_channel();
    let tx_tick = tx.clone();

    // Tick sender (60fps for smooth animations)
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_millis(16));
        loop {
            interval.tick().await;
            if tx_tick.send(TermEvent::Tick).is_err() {
                break;
            }
        }
    });

    // Event reader
    tokio::spawn(async move {
        loop {
            if event::poll(Duration::from_millis(50)).unwrap_or(false) {
                if let Ok(event) = event::read() {
                    let term_event = match event {
                        Event::Key(key) if key.kind == KeyEventKind::Press => {
                            Some(TermEvent::Key(key.code))
                        }
                        Event::Resize(w, h) => Some(TermEvent::Resize(w, h)),
                        _ => None,
                    };
                    if let Some(e) = term_event {
                        if tx.send(e).is_err() {
                            break;
                        }
                    }
                }
            }
        }
    });

    rx
}
