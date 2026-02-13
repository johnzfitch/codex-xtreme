//! Dev-mode cherry-pick screen (comma-separated SHAs)

use crate::tui::theme::{self, center_x, jp};
use crate::tui::widgets::Panel;
use ratatui::{
    buffer::Buffer,
    layout::{Constraint, Layout, Rect},
    style::{Modifier, Style},
    widgets::Widget,
};
use unicode_width::UnicodeWidthStr;

/// Text input screen for cherry-picking commit SHAs.
pub struct CherryPickScreen {
    frame: u64,
    target_tag: String,
    value: String,
    cursor_pos: usize,
    placeholder: String,
    status: Option<String>,
}

impl CherryPickScreen {
    pub fn new(target_tag: impl Into<String>) -> Self {
        Self {
            frame: 0,
            target_tag: target_tag.into(),
            value: String::new(),
            cursor_pos: 0,
            placeholder: "abc1234, def5678".to_string(),
            status: None,
        }
    }

    pub fn set_value(&mut self, text: impl Into<String>) {
        self.value = text.into();
        self.cursor_pos = self.value.chars().count();
    }

    pub fn tick(&mut self) {
        self.frame += 1;
    }

    /// Convert character position to byte index.
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

    pub fn status(&self) -> Option<&str> {
        self.status.as_deref()
    }

    pub fn set_status(&mut self, status: Option<String>) {
        self.status = status;
    }
}

impl Widget for &CherryPickScreen {
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
            Constraint::Length(5), // Input panel
            Constraint::Length(3), // Info
            Constraint::Min(2),    // Spacer
            Constraint::Length(2), // Help
        ])
        .split(area);

        // Header
        let header_line = format!("░▒▓█ CHERRY-PICK //{} █▓▒░", jp::CHERRY_PICK);
        let header_w = UnicodeWidthStr::width(header_line.as_str()) as u16;
        let header_x = center_x(area.x, area.width, header_w);
        buf.set_string(header_x, chunks[0].y + 1, &header_line, theme::title());

        // Input panel
        let input_area = Rect {
            x: chunks[2].x + 4,
            y: chunks[2].y,
            width: chunks[2].width.saturating_sub(8),
            height: chunks[2].height,
        };

        let panel = Panel::new()
            .title("Cherry-pick commits (comma-separated SHAs, empty to skip)")
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

        // Truncate if needed, keeping cursor visible (using character counts).
        let char_count = display_value.chars().count();
        let (display, cursor_offset) = if char_count > max_visible {
            let start_char = self.cursor_pos.saturating_sub(max_visible / 2);
            let end_char = (start_char + max_visible).min(char_count);
            let start_char = end_char.saturating_sub(max_visible);

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

            (
                &display_value[start_byte..end_byte],
                self.cursor_pos - start_char,
            )
        } else {
            (display_value.as_str(), self.cursor_pos)
        };

        buf.set_string(value_x, value_y, display, value_style);

        // Cursor
        let cursor_visible = (self.frame / 30).is_multiple_of(2);
        if cursor_visible {
            let cursor_x = value_x + cursor_offset as u16;
            buf.set_string(
                cursor_x,
                value_y,
                "▎",
                Style::default()
                    .fg(theme::CYAN)
                    .add_modifier(Modifier::BOLD),
            );
        }

        // Info text
        let url = format!(
            "Compare: https://github.com/openai/codex/compare/{}...main",
            self.target_tag
        );
        let url_w = UnicodeWidthStr::width(url.as_str()) as u16;
        let url_x = center_x(area.x, area.width, url_w);
        buf.set_string(url_x, chunks[3].y, &url, theme::secondary());

        if let Some(status) = &self.status {
            let status_w = UnicodeWidthStr::width(status.as_str()) as u16;
            let status_x = center_x(area.x, area.width, status_w);
            buf.set_string(status_x, chunks[3].y + 1, status, theme::warning());
        } else {
            let hint = "Tip: use 7+ hex chars per SHA; invalid entries will be ignored";
            let hint_w = UnicodeWidthStr::width(hint) as u16;
            let hint_x = center_x(area.x, area.width, hint_w);
            buf.set_string(hint_x, chunks[3].y + 1, hint, theme::muted());
        }

        // Help text
        let help = "[ENTER] Continue  [ESC] Back";
        let help_w = UnicodeWidthStr::width(help) as u16;
        let help_x = center_x(area.x, area.width, help_w);
        buf.set_string(help_x, chunks[5].y, help, theme::muted());
    }
}
