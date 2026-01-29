//! Text input screen for clone destination

use crate::tui::theme::{self, jp};
use crate::tui::widgets::Panel;
use ratatui::{
    buffer::Buffer,
    layout::{Constraint, Layout, Rect},
    style::{Modifier, Style},
    widgets::Widget,
};

/// Text input screen
pub struct InputScreen {
    frame: u64,
    prompt: String,
    value: String,
    cursor_pos: usize,
    placeholder: String,
}

impl InputScreen {
    pub fn new(prompt: impl Into<String>) -> Self {
        Self {
            frame: 0,
            prompt: prompt.into(),
            value: String::new(),
            cursor_pos: 0,
            placeholder: String::new(),
        }
    }

    pub fn placeholder(mut self, text: impl Into<String>) -> Self {
        self.placeholder = text.into();
        self
    }

    pub fn initial_value(mut self, text: impl Into<String>) -> Self {
        self.value = text.into();
        self.cursor_pos = self.value.chars().count();
        self
    }

    pub fn tick(&mut self) {
        self.frame += 1;
    }

    /// Convert character position to byte index
    fn char_to_byte_index(&self, char_pos: usize) -> usize {
        self.value
            .char_indices()
            .nth(char_pos)
            .map(|(i, _)| i)
            .unwrap_or(self.value.len())
    }

    pub fn insert_char(&mut self, c: char) {
        let byte_idx = self.char_to_byte_index(self.cursor_pos);
        self.value.insert(byte_idx, c);
        self.cursor_pos += 1;
    }

    pub fn delete_char(&mut self) {
        if self.cursor_pos > 0 {
            self.cursor_pos -= 1;
            let byte_idx = self.char_to_byte_index(self.cursor_pos);
            self.value.remove(byte_idx);
        }
    }

    pub fn delete_forward(&mut self) {
        let char_count = self.value.chars().count();
        if self.cursor_pos < char_count {
            let byte_idx = self.char_to_byte_index(self.cursor_pos);
            self.value.remove(byte_idx);
        }
    }

    pub fn move_left(&mut self) {
        if self.cursor_pos > 0 {
            self.cursor_pos -= 1;
        }
    }

    pub fn move_right(&mut self) {
        let char_count = self.value.chars().count();
        if self.cursor_pos < char_count {
            self.cursor_pos += 1;
        }
    }

    pub fn move_home(&mut self) {
        self.cursor_pos = 0;
    }

    pub fn move_end(&mut self) {
        self.cursor_pos = self.value.chars().count();
    }

    pub fn value(&self) -> &str {
        &self.value
    }

    pub fn frame(&self) -> u64 {
        self.frame
    }
}

impl Widget for &InputScreen {
    fn render(self, area: Rect, buf: &mut Buffer) {
        // Clear background
        for y in area.y..(area.y + area.height) {
            for x in area.x..(area.x + area.width) {
                buf.set_string(x, y, " ", Style::default().bg(theme::BG_VOID));
            }
        }

        let chunks = Layout::vertical([
            Constraint::Length(4),   // Header
            Constraint::Length(1),   // Spacer
            Constraint::Length(5),   // Input panel
            Constraint::Length(2),   // Info
            Constraint::Min(2),      // Spacer
            Constraint::Length(2),   // Help
        ])
        .split(area);

        // Header
        let header_line = format!("░▒▓█ CLONE REPOSITORY //{} █▓▒░", jp::TARGET_SELECT);
        let header_x = area.x + (area.width.saturating_sub(header_line.len() as u16)) / 2;
        buf.set_string(header_x, chunks[0].y + 1, &header_line, theme::title());

        // Input panel
        let input_area = Rect {
            x: chunks[2].x + 4,
            y: chunks[2].y,
            width: chunks[2].width.saturating_sub(8),
            height: chunks[2].height,
        };

        let panel = Panel::new()
            .title(&self.prompt)
            .focused(true);
        panel.render(input_area, buf);

        // Input value
        let value_y = input_area.y + 2;
        let value_x = input_area.x + 3;
        let max_visible = input_area.width.saturating_sub(6) as usize;

        let display_value = if self.value.is_empty() {
            &self.placeholder
        } else {
            &self.value
        };

        let value_style = if self.value.is_empty() {
            theme::muted()
        } else {
            theme::normal()
        };

        // Truncate if needed, keeping cursor visible (using character counts)
        let char_count = display_value.chars().count();
        let (display, cursor_offset) = if char_count > max_visible {
            let start_char = self.cursor_pos.saturating_sub(max_visible / 2);
            let end_char = (start_char + max_visible).min(char_count);
            let start_char = end_char.saturating_sub(max_visible);

            // Convert character positions to byte indices for slicing
            let start_byte = display_value
                .char_indices()
                .nth(start_char)
                .map(|(i, _)| i)
                .unwrap_or(0);
            let end_byte = display_value
                .char_indices()
                .nth(end_char)
                .map(|(i, _)| i)
                .unwrap_or(display_value.len());

            (&display_value[start_byte..end_byte], self.cursor_pos - start_char)
        } else {
            (display_value.as_str(), self.cursor_pos)
        };

        buf.set_string(value_x, value_y, display, value_style);

        // Cursor
        let cursor_visible = (self.frame / 30) % 2 == 0;
        if cursor_visible && !self.value.is_empty() {
            let cursor_x = value_x + cursor_offset as u16;
            buf.set_string(
                cursor_x,
                value_y,
                "▎",
                Style::default().fg(theme::CYAN).add_modifier(Modifier::BOLD),
            );
        } else if self.value.is_empty() && cursor_visible {
            buf.set_string(
                value_x,
                value_y,
                "▎",
                Style::default().fg(theme::CYAN).add_modifier(Modifier::BOLD),
            );
        }

        // Info text
        let info = "Will clone: https://github.com/openai/codex.git";
        let info_x = area.x + (area.width.saturating_sub(info.len() as u16)) / 2;
        buf.set_string(info_x, chunks[3].y, info, theme::secondary());

        // Help text
        let help = "[ENTER] Clone  [ESC] Cancel";
        let help_x = area.x + (area.width.saturating_sub(help.len() as u16)) / 2;
        buf.set_string(help_x, chunks[5].y, help, theme::muted());
    }
}
