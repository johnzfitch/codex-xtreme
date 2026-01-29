//! Animated selection list widget

use crate::tui::theme;
use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Modifier, Style},
    widgets::Widget,
};

/// Status indicator for list items
#[derive(Clone, Copy, PartialEq)]
pub enum ListStatus {
    None,
    Ready,
    Modified,
    Active,
    Complete,
    Error,
    Current,
    Latest,
}

impl ListStatus {
    pub fn indicator(&self) -> &'static str {
        match self {
            ListStatus::None => "  ",
            ListStatus::Ready => "✓ ",
            ListStatus::Modified => "◈ ",
            ListStatus::Active => "▶ ",
            ListStatus::Complete => "✓ ",
            ListStatus::Error => "✗ ",
            ListStatus::Current => "◀ ",
            ListStatus::Latest => "★ ",
        }
    }

    pub fn style(&self) -> Style {
        match self {
            ListStatus::None => theme::muted(),
            ListStatus::Ready => theme::success(),
            ListStatus::Modified => theme::warning(),
            ListStatus::Active => theme::active(),
            ListStatus::Complete => theme::success(),
            ListStatus::Error => theme::error(),
            ListStatus::Current => theme::secondary(),
            ListStatus::Latest => theme::warning(),
        }
    }
}

/// A list item with optional status and metadata
pub struct ListItem {
    pub label: String,
    pub description: Option<String>,
    pub status: ListStatus,
    pub secondary_status: Option<String>,
}

impl ListItem {
    pub fn new(label: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            description: None,
            status: ListStatus::None,
            secondary_status: None,
        }
    }

    pub fn description(mut self, desc: impl Into<String>) -> Self {
        self.description = Some(desc.into());
        self
    }

    pub fn status(mut self, status: ListStatus) -> Self {
        self.status = status;
        self
    }

    pub fn secondary(mut self, text: impl Into<String>) -> Self {
        self.secondary_status = Some(text.into());
        self
    }
}

/// A selectable list with cursor animation
pub struct SelectList<'a> {
    items: &'a [ListItem],
    selected: usize,
    frame: u64,
    show_indices: bool,
}

impl<'a> SelectList<'a> {
    pub fn new(items: &'a [ListItem]) -> Self {
        Self {
            items,
            selected: 0,
            frame: 0,
            show_indices: false,
        }
    }

    pub fn selected(mut self, idx: usize) -> Self {
        self.selected = idx;
        self
    }

    pub fn frame(mut self, frame: u64) -> Self {
        self.frame = frame;
        self
    }

    pub fn show_indices(mut self, show: bool) -> Self {
        self.show_indices = show;
        self
    }
}

impl Widget for SelectList<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let mut y = area.y;

        for (idx, item) in self.items.iter().enumerate() {
            if y >= area.y + area.height {
                break;
            }

            let is_selected = idx == self.selected;
            let mut x = area.x;

            // Cursor indicator with animation
            if is_selected {
                let cursor_chars = ['▸', '▹', '▸', '▹'];
                let cursor = cursor_chars[(self.frame / 6) as usize % cursor_chars.len()];
                buf.set_string(x, y, cursor.to_string(), theme::cursor());
            } else {
                buf.set_string(x, y, " ", theme::normal());
            }
            x += 2;

            // Index if shown
            if self.show_indices {
                let index_str = format!("{:>2}. ", idx + 1);
                buf.set_string(x, y, &index_str, theme::muted());
                x += 4;
            }

            // Label
            let label_style = if is_selected {
                Style::default()
                    .fg(theme::CYAN)
                    .add_modifier(Modifier::BOLD)
            } else {
                theme::normal()
            };
            buf.set_string(x, y, &item.label, label_style);
            x += item.label.len() as u16 + 1;

            // Status indicator
            if item.status != ListStatus::None {
                buf.set_string(x, y, item.status.indicator(), item.status.style());
                x += 2;
            }

            // Secondary status (right-aligned if room)
            if let Some(ref secondary) = item.secondary_status {
                let sec_x = area.x + area.width - secondary.len() as u16 - 1;
                if sec_x > x {
                    buf.set_string(sec_x, y, secondary, theme::muted());
                }
            }

            y += 1;

            // Description on next line if present
            if let Some(ref desc) = item.description {
                if y < area.y + area.height {
                    let desc_style = if is_selected {
                        theme::secondary()
                    } else {
                        theme::muted()
                    };
                    let desc_x = area.x + 4;
                    let prefix = if is_selected { "└─ " } else { "   " };
                    buf.set_string(desc_x, y, prefix, theme::dim());
                    buf.set_string(desc_x + 3, y, desc, desc_style);
                    y += 1;
                }
            }

            // Add spacing between items
            if item.description.is_some() {
                y += 1;
            }
        }
    }
}

/// Checkbox list for multi-select
pub struct CheckList<'a> {
    items: &'a [(String, bool)],
    cursor: usize,
    frame: u64,
}

impl<'a> CheckList<'a> {
    pub fn new(items: &'a [(String, bool)]) -> Self {
        Self {
            items,
            cursor: 0,
            frame: 0,
        }
    }

    pub fn cursor(mut self, idx: usize) -> Self {
        self.cursor = idx;
        self
    }

    pub fn frame(mut self, frame: u64) -> Self {
        self.frame = frame;
        self
    }
}

impl Widget for CheckList<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        for (idx, (label, checked)) in self.items.iter().enumerate() {
            let y = area.y + idx as u16;
            if y >= area.y + area.height {
                break;
            }

            let is_cursor = idx == self.cursor;
            let mut x = area.x;

            // Cursor
            if is_cursor {
                let cursor_chars = ['▸', '▹'];
                let cursor = cursor_chars[(self.frame / 8) as usize % cursor_chars.len()];
                buf.set_string(x, y, cursor.to_string(), theme::cursor());
            } else {
                buf.set_string(x, y, " ", theme::normal());
            }
            x += 2;

            // Checkbox
            let checkbox = if *checked { "[✓]" } else { "[ ]" };
            let checkbox_style = if *checked {
                theme::success()
            } else {
                theme::muted()
            };
            buf.set_string(x, y, checkbox, checkbox_style);
            x += 4;

            // Label
            let label_style = if is_cursor {
                theme::focused()
            } else if *checked {
                theme::normal()
            } else {
                theme::secondary()
            };
            buf.set_string(x, y, label, label_style);
        }
    }
}
