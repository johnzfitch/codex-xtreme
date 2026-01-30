//! Cyberpunk-styled panel widget

use crate::tui::theme;
use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::Style,
    widgets::Widget,
};

/// A styled panel with customizable borders
pub struct Panel<'a> {
    title: Option<&'a str>,
    title_jp: Option<&'a str>,
    focused: bool,
    double_border: bool,
}

impl<'a> Panel<'a> {
    pub fn new() -> Self {
        Self {
            title: None,
            title_jp: None,
            focused: false,
            double_border: false,
        }
    }

    pub fn title(mut self, title: &'a str) -> Self {
        self.title = Some(title);
        self
    }

    pub fn title_jp(mut self, jp: &'a str) -> Self {
        self.title_jp = Some(jp);
        self
    }

    pub fn focused(mut self, focused: bool) -> Self {
        self.focused = focused;
        self
    }

    pub fn double_border(mut self) -> Self {
        self.double_border = true;
        self
    }
}

impl Default for Panel<'_> {
    fn default() -> Self {
        Self::new()
    }
}

impl Widget for Panel<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if area.width < 4 || area.height < 3 {
            return;
        }

        let border_style = if self.focused {
            theme::border_focused()
        } else {
            theme::border()
        };

        // Box drawing characters
        let (tl, tr, bl, br, h, v) = if self.double_border {
            ('╔', '╗', '╚', '╝', '═', '║')
        } else {
            ('┌', '┐', '└', '┘', '─', '│')
        };

        // Top border
        buf.set_string(area.x, area.y, tl.to_string(), border_style);
        for x in (area.x + 1)..(area.x + area.width - 1) {
            buf.set_string(x, area.y, h.to_string(), border_style);
        }
        buf.set_string(area.x + area.width - 1, area.y, tr.to_string(), border_style);

        // Side borders
        for y in (area.y + 1)..(area.y + area.height - 1) {
            buf.set_string(area.x, y, v.to_string(), border_style);
            buf.set_string(area.x + area.width - 1, y, v.to_string(), border_style);
        }

        // Bottom border
        buf.set_string(area.x, area.y + area.height - 1, bl.to_string(), border_style);
        for x in (area.x + 1)..(area.x + area.width - 1) {
            buf.set_string(x, area.y + area.height - 1, h.to_string(), border_style);
        }
        buf.set_string(area.x + area.width - 1, area.y + area.height - 1, br.to_string(), border_style);

        // Title
        if let Some(title) = self.title {
            let title_style = if self.focused {
                theme::title()
            } else {
                theme::secondary()
            };

            // Add decorative elements around title
            let decorated = if self.double_border {
                format!("╡ {} ╞", title)
            } else {
                format!("─ {} ─", title)
            };

            let title_x = area.x + 2;
            if title_x + decorated.len() as u16 <= area.x + area.width - 2 {
                buf.set_string(title_x, area.y, &decorated, title_style);
            }

            // Japanese subtitle
            if let Some(jp) = self.title_jp {
                let jp_x = title_x + decorated.len() as u16 + 1;
                if jp_x + jp.len() as u16 <= area.x + area.width - 2 {
                    buf.set_string(jp_x, area.y, format!("//{}", jp), theme::kanji());
                }
            }
        }
    }
}

/// Inner area helper - returns the usable area inside the panel
pub fn inner_area(area: Rect) -> Rect {
    if area.width < 4 || area.height < 3 {
        return Rect::default();
    }
    Rect {
        x: area.x + 2,
        y: area.y + 1,
        width: area.width.saturating_sub(4),
        height: area.height.saturating_sub(2),
    }
}
