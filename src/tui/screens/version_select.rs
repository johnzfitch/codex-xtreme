//! Version/tag selection screen

use crate::tui::theme::{self, center_x, jp};
use crate::tui::widgets::{ListItem, ListStatus, Panel, SelectList};
use ratatui::{
    buffer::Buffer,
    layout::{Constraint, Layout, Rect},
    style::Style,
    widgets::Widget,
};
use unicode_width::UnicodeWidthStr;

/// Version/release information
#[derive(Clone)]
pub struct VersionInfo {
    pub tag: String,
    pub date: String,
    pub is_latest: bool,
    pub is_current: bool,
    pub changelog: Vec<String>,
}

/// Version selection screen
pub struct VersionSelectScreen {
    frame: u64,
    versions: Vec<VersionInfo>,
    cursor: usize,
}

impl VersionSelectScreen {
    pub fn new(versions: Vec<VersionInfo>) -> Self {
        Self {
            frame: 0,
            versions,
            cursor: 0,
        }
    }

    pub fn tick(&mut self) {
        self.frame += 1;
    }

    pub fn select_next(&mut self) {
        if self.cursor < self.versions.len().saturating_sub(1) {
            self.cursor += 1;
        }
    }

    pub fn select_prev(&mut self) {
        if self.cursor > 0 {
            self.cursor -= 1;
        }
    }

    pub fn selected_version(&self) -> Option<&VersionInfo> {
        self.versions.get(self.cursor)
    }

    pub fn frame(&self) -> u64 {
        self.frame
    }
}

impl Widget for &VersionSelectScreen {
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
            Constraint::Min(8),    // Version list
            Constraint::Length(6), // Changelog panel
            Constraint::Length(2), // Help
        ])
        .split(area);

        // Header
        let header_line = format!("░▒▓█ TARGET VERSION //{} █▓▒░", jp::VERSION_SELECT);
        let header_w = UnicodeWidthStr::width(header_line.as_str()) as u16;
        let header_x = center_x(area.x, area.width, header_w);
        buf.set_string(header_x, chunks[0].y + 1, &header_line, theme::title());

        // Build list items
        let items: Vec<ListItem> = self
            .versions
            .iter()
            .map(|ver| {
                let mut item = ListItem::new(&ver.tag).secondary(ver.date.clone());

                if ver.is_latest {
                    item = item.status(ListStatus::Latest);
                } else if ver.is_current {
                    item = item.status(ListStatus::Current);
                }

                item
            })
            .collect();

        // Version list panel
        let list_area = Rect {
            x: chunks[2].x + 2,
            y: chunks[2].y,
            width: chunks[2].width.saturating_sub(4),
            height: chunks[2].height,
        };

        let panel = Panel::new().title("VERSIONS").focused(true);
        panel.render(list_area, buf);

        let inner_area = Rect {
            x: list_area.x + 2,
            y: list_area.y + 1,
            width: list_area.width.saturating_sub(4),
            height: list_area.height.saturating_sub(2),
        };

        let list = SelectList::new(&items)
            .selected(self.cursor)
            .frame(self.frame);
        list.render(inner_area, buf);

        // Changelog panel
        let changelog_area = Rect {
            x: chunks[3].x + 2,
            y: chunks[3].y,
            width: chunks[3].width.saturating_sub(4),
            height: chunks[3].height,
        };

        let changelog_panel = Panel::new().title("CHANGELOG").title_jp(jp::CHANGELOG);
        changelog_panel.render(changelog_area, buf);

        // Changelog content
        if let Some(version) = self.selected_version() {
            for (i, line) in version.changelog.iter().take(4).enumerate() {
                let y = changelog_area.y + 1 + i as u16;
                let text = format!("  • {}", line);
                buf.set_string(changelog_area.x + 2, y, &text, theme::secondary());
            }
        }

        // Help text
        let help = "[↑↓] Navigate  [ENTER] Select  [ESC] Back  [Q] Quit";
        let help_x = area.x + (area.width.saturating_sub(help.len() as u16)) / 2;
        buf.set_string(help_x, chunks[4].y, help, theme::muted());
    }
}
