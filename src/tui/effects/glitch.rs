//! Text glitch effects

use crate::tui::theme;
use rand::Rng;
use ratatui::{buffer::Buffer, layout::Rect, style::Style, widgets::Widget};

/// Glitched text that can animate from corrupted to resolved
pub struct GlitchText<'a> {
    text: &'a str,
    intensity: f64,
    frame: u64,
    style: Style,
}

impl<'a> GlitchText<'a> {
    pub fn new(text: &'a str) -> Self {
        Self {
            text,
            intensity: 0.0,
            frame: 0,
            style: theme::normal(),
        }
    }

    pub fn intensity(mut self, intensity: f64) -> Self {
        self.intensity = intensity.clamp(0.0, 1.0);
        self
    }

    pub fn frame(mut self, frame: u64) -> Self {
        self.frame = frame;
        self
    }

    pub fn style(mut self, style: Style) -> Self {
        self.style = style;
        self
    }
}

impl Widget for GlitchText<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if self.intensity <= 0.0 {
            buf.set_string(area.x, area.y, self.text, self.style);
            return;
        }

        let mut rng = rand::thread_rng();
        let glitched: String = self
            .text
            .chars()
            .enumerate()
            .map(|(i, c)| {
                let seed = (self.frame as usize + i * 7) % 100;
                let threshold = (self.intensity * 100.0) as usize;

                if seed < threshold {
                    random_glitch_char(&mut rng, c)
                } else {
                    c
                }
            })
            .collect();

        buf.set_string(area.x, area.y, &glitched, self.style);
    }
}

fn random_glitch_char(rng: &mut impl Rng, _original: char) -> char {
    const GLITCH_CHARS: &[char] = &[
        '█', '▓', '▒', '░', '▄', '▀', '■', '□', '/', '\\', '|', '-', '_', '=', '+', '?', '#', '@',
        '%', '&', '*',
    ];
    GLITCH_CHARS[rng.gen_range(0..GLITCH_CHARS.len())]
}
