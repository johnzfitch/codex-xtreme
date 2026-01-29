//! CRT scanline effect

use crate::tui::theme;
use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::Style,
    widgets::Widget,
};

/// Subtle CRT scanline overlay
pub struct Scanlines {
    offset: u16,
    intensity: f64,
}

impl Scanlines {
    pub fn new() -> Self {
        Self {
            offset: 0,
            intensity: 0.3,
        }
    }

    pub fn offset(mut self, offset: u16) -> Self {
        self.offset = offset;
        self
    }

    #[allow(dead_code)]
    pub fn intensity(mut self, intensity: f64) -> Self {
        self.intensity = intensity.clamp(0.0, 1.0);
        self
    }
}

impl Default for Scanlines {
    fn default() -> Self {
        Self::new()
    }
}

impl Widget for Scanlines {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if self.intensity <= 0.0 {
            return;
        }

        // Apply dim overlay on alternating lines
        for y in area.y..(area.y + area.height) {
            let line_offset = (y + self.offset) % 3;
            if line_offset == 0 {
                for x in area.x..(area.x + area.width) {
                    // Only dim non-empty cells
                    if let Some(cell) = buf.cell((x, y)) {
                        if cell.symbol() != " " {
                            // Apply subtle darkening
                            buf.set_style(
                                Rect::new(x, y, 1, 1),
                                Style::default().fg(theme::TEXT_DIM),
                            );
                        }
                    }
                }
            }
        }
    }
}
