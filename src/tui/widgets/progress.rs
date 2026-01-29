//! Animated progress bar with glow effects

use crate::tui::theme::{self, blocks};
use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::Style,
    widgets::Widget,
};

/// A glowing progress bar
pub struct ProgressBar {
    progress: f64,
    label: Option<String>,
    frame: u64,
    show_percentage: bool,
}

impl ProgressBar {
    pub fn new(progress: f64) -> Self {
        Self {
            progress: progress.clamp(0.0, 1.0),
            label: None,
            frame: 0,
            show_percentage: true,
        }
    }

    pub fn label(mut self, label: impl Into<String>) -> Self {
        self.label = Some(label.into());
        self
    }

    pub fn frame(mut self, frame: u64) -> Self {
        self.frame = frame;
        self
    }

    pub fn show_percentage(mut self, show: bool) -> Self {
        self.show_percentage = show;
        self
    }
}

impl Widget for ProgressBar {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if area.width < 10 || area.height < 1 {
            return;
        }

        // Calculate bar dimensions
        let percentage_width = if self.show_percentage { 6 } else { 0 }; // " 100%"
        let label_width = self.label.as_ref().map(|l| l.len() as u16 + 1).unwrap_or(0);
        let bar_width = area.width.saturating_sub(percentage_width + label_width + 4);
        let bar_x = area.x + label_width + 2;

        // Draw label
        if let Some(label) = &self.label {
            buf.set_string(area.x, area.y, label, theme::secondary());
        }

        // Draw bar frame
        buf.set_string(bar_x, area.y, "[", theme::border());
        buf.set_string(bar_x + bar_width + 1, area.y, "]", theme::border());

        // Calculate filled portion
        let filled = ((bar_width as f64) * self.progress) as u16;
        let partial = (((bar_width as f64) * self.progress) * 8.0) as usize % 8;

        // Draw filled portion with gradient effect
        for i in 0..bar_width {
            let x = bar_x + 1 + i;
            if i < filled {
                // Filled - use block with slight color variation for "glow"
                let intensity = ((self.frame + i as u64) % 20) as f32 / 20.0;
                let color = if intensity > 0.8 {
                    theme::CYAN
                } else {
                    theme::CYAN_DIM
                };
                buf.set_string(x, area.y, blocks::PROGRESS_FULL, Style::default().fg(color));
            } else if i == filled && partial > 0 {
                // Partial block
                let partial_char = blocks::PROGRESS_PARTIAL[partial];
                buf.set_string(x, area.y, partial_char, Style::default().fg(theme::CYAN_DIM));
            } else {
                // Empty
                buf.set_string(x, area.y, blocks::PROGRESS_EMPTY, theme::dim());
            }
        }

        // Draw percentage
        if self.show_percentage {
            let pct = format!("{:>3}%", (self.progress * 100.0) as u8);
            let pct_x = bar_x + bar_width + 3;
            let pct_style = if self.progress >= 1.0 {
                theme::success()
            } else {
                theme::normal()
            };
            buf.set_string(pct_x, area.y, &pct, pct_style);
        }
    }
}

/// Indeterminate spinner progress
pub struct Spinner {
    frame: u64,
    label: Option<String>,
}

impl Spinner {
    pub fn new() -> Self {
        Self {
            frame: 0,
            label: None,
        }
    }

    pub fn frame(mut self, frame: u64) -> Self {
        self.frame = frame;
        self
    }

    pub fn label(mut self, label: impl Into<String>) -> Self {
        self.label = Some(label.into());
        self
    }
}

impl Default for Spinner {
    fn default() -> Self {
        Self::new()
    }
}

impl Widget for Spinner {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let spinner_chars = theme::spinners::BRAILLE;
        let spinner = spinner_chars[(self.frame / 4) as usize % spinner_chars.len()];

        buf.set_string(area.x, area.y, spinner.to_string(), theme::active());

        if let Some(label) = &self.label {
            buf.set_string(area.x + 2, area.y, label, theme::normal());
        }
    }
}
