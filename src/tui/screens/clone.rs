//! Clone progress screen

use crate::tui::theme::{self, center_x, jp};
use crate::tui::widgets::Panel;
use ratatui::{
    buffer::Buffer,
    layout::{Constraint, Layout, Rect},
    style::Style,
    widgets::Widget,
};
use unicode_width::UnicodeWidthStr;

/// Clone status
#[derive(Clone, Copy, PartialEq)]
pub enum CloneStatus {
    Cloning,
    Complete,
    Error,
}

/// Clone progress screen
pub struct CloneScreen {
    frame: u64,
    destination: String,
    status: CloneStatus,
    progress_text: String,
    error_message: Option<String>,
    /// Frames since completion (for auto-advance)
    complete_frames: u64,
}

impl CloneScreen {
    pub fn new(destination: impl Into<String>) -> Self {
        Self {
            frame: 0,
            destination: destination.into(),
            status: CloneStatus::Cloning,
            progress_text: "Initializing...".to_string(),
            error_message: None,
            complete_frames: 0,
        }
    }

    pub fn tick(&mut self) {
        self.frame += 1;
        if self.status == CloneStatus::Complete {
            self.complete_frames += 1;
        }
    }

    /// Returns true when should auto-advance (2 seconds after completion)
    pub fn should_auto_advance(&self) -> bool {
        self.status == CloneStatus::Complete && self.complete_frames > 120 // ~2 seconds at 60fps
    }

    /// Countdown seconds remaining (2, 1, or 0 for advance)
    pub fn countdown(&self) -> u8 {
        if self.status != CloneStatus::Complete {
            return 0;
        }
        let seconds = self.complete_frames / 60;
        match seconds {
            0 => 2,
            1 => 1,
            _ => 0,
        }
    }

    pub fn set_progress(&mut self, text: impl Into<String>) {
        self.progress_text = text.into();
    }

    pub fn set_complete(&mut self) {
        self.status = CloneStatus::Complete;
        self.progress_text = "Clone complete!".to_string();
    }

    pub fn set_error(&mut self, msg: impl Into<String>) {
        self.status = CloneStatus::Error;
        self.error_message = Some(msg.into());
    }

    pub fn is_complete(&self) -> bool {
        self.status == CloneStatus::Complete
    }

    pub fn is_error(&self) -> bool {
        self.status == CloneStatus::Error
    }

    pub fn destination(&self) -> &str {
        &self.destination
    }

    pub fn frame(&self) -> u64 {
        self.frame
    }
}

impl Widget for &CloneScreen {
    fn render(self, area: Rect, buf: &mut Buffer) {
        // Clear background
        for y in area.y..(area.y + area.height) {
            for x in area.x..(area.x + area.width) {
                buf.set_string(x, y, " ", Style::default().bg(theme::BG_VOID));
            }
        }

        let chunks = Layout::vertical([
            Constraint::Length(4), // Header
            Constraint::Length(1), // Spacer
            Constraint::Length(6), // Status panel
            Constraint::Min(4),    // Log/progress
            Constraint::Length(2), // Help
        ])
        .split(area);

        // Header
        let header_text = match self.status {
            CloneStatus::Cloning => "CLONING",
            CloneStatus::Complete => "CLONE COMPLETE",
            CloneStatus::Error => "CLONE FAILED",
        };
        let header_line = format!("░▒▓█ {} //{} █▓▒░", header_text, jp::CONNECTING);
        let header_w = UnicodeWidthStr::width(header_line.as_str()) as u16;
        let header_x = center_x(area.x, area.width, header_w);
        let header_style = match self.status {
            CloneStatus::Cloning => theme::title(),
            CloneStatus::Complete => theme::success(),
            CloneStatus::Error => theme::error(),
        };
        buf.set_string(header_x, chunks[0].y + 1, &header_line, header_style);

        // Status panel
        let status_area = Rect {
            x: chunks[2].x + 4,
            y: chunks[2].y,
            width: chunks[2].width.saturating_sub(8),
            height: chunks[2].height,
        };

        let panel = Panel::new()
            .title("STATUS")
            .focused(self.status == CloneStatus::Cloning);
        panel.render(status_area, buf);

        // Destination
        let dest_line = format!("Destination: {}", self.destination);
        buf.set_string(
            status_area.x + 2,
            status_area.y + 1,
            &dest_line,
            theme::secondary(),
        );

        // Source
        buf.set_string(
            status_area.x + 2,
            status_area.y + 2,
            "Source: https://github.com/openai/codex.git",
            theme::secondary(),
        );

        // Progress or error
        match self.status {
            CloneStatus::Cloning => {
                let spinner_chars = theme::spinners::BRAILLE;
                let spinner = spinner_chars[(self.frame / 4) as usize % spinner_chars.len()];
                let progress_line = format!("{} {}", spinner, self.progress_text);
                buf.set_string(
                    status_area.x + 2,
                    status_area.y + 4,
                    &progress_line,
                    theme::active(),
                );
            }
            CloneStatus::Complete => {
                buf.set_string(
                    status_area.x + 2,
                    status_area.y + 4,
                    "✓ Repository cloned successfully",
                    theme::success(),
                );
            }
            CloneStatus::Error => {
                if let Some(ref msg) = self.error_message {
                    buf.set_string(
                        status_area.x + 2,
                        status_area.y + 4,
                        format!("✗ {}", msg),
                        theme::error(),
                    );
                }
            }
        }

        // Help
        let help = match self.status {
            CloneStatus::Cloning => "Cloning repository... Press [Q] to cancel".to_string(),
            CloneStatus::Complete => {
                let countdown = self.countdown();
                if countdown > 0 {
                    format!("Continuing in {}...", countdown)
                } else {
                    "Launching...".to_string()
                }
            }
            CloneStatus::Error => "Press [R] to retry or [ESC] to go back".to_string(),
        };
        let help_x = area.x + (area.width.saturating_sub(help.len() as u16)) / 2;
        buf.set_string(help_x, chunks[4].y, &help, theme::muted());
    }
}
