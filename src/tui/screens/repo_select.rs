//! Repository selection screen

use crate::tui::theme::{self, jp};
use crate::tui::widgets::{ListItem, ListStatus, Panel, SelectList};
use ratatui::{
    buffer::Buffer,
    layout::{Constraint, Layout, Rect},
    style::Style,
    widgets::Widget,
};
use std::path::PathBuf;

/// Repository information
#[derive(Clone)]
pub struct RepoInfo {
    pub path: PathBuf,
    pub branch: String,
    pub age: String,
    pub is_modified: bool,
}

impl RepoInfo {
    pub fn display_path(&self) -> String {
        self.path.to_string_lossy().to_string()
    }
}

/// Repository selection screen
pub struct RepoSelectScreen {
    frame: u64,
    repos: Vec<RepoInfo>,
    cursor: usize,
    show_clone_option: bool,
}

impl RepoSelectScreen {
    pub fn new(repos: Vec<RepoInfo>) -> Self {
        Self {
            frame: 0,
            repos,
            cursor: 0,
            show_clone_option: true,
        }
    }

    pub fn tick(&mut self) {
        self.frame += 1;
    }

    pub fn select_next(&mut self) {
        let max = self.repos.len() + if self.show_clone_option { 1 } else { 0 };
        if self.cursor < max.saturating_sub(1) {
            self.cursor += 1;
        }
    }

    pub fn select_prev(&mut self) {
        if self.cursor > 0 {
            self.cursor -= 1;
        }
    }

    pub fn is_clone_selected(&self) -> bool {
        self.show_clone_option && self.cursor == self.repos.len()
    }

    pub fn selected_repo(&self) -> Option<&RepoInfo> {
        if self.cursor < self.repos.len() {
            Some(&self.repos[self.cursor])
        } else {
            None
        }
    }

    pub fn frame(&self) -> u64 {
        self.frame
    }
}

impl Widget for &RepoSelectScreen {
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
            Constraint::Min(10),   // Repo list
            Constraint::Length(2), // Help
        ])
        .split(area);

        // Header
        render_header(
            chunks[0],
            buf,
            "SELECT TARGET",
            jp::TARGET_SELECT,
            self.frame,
        );

        // Build list items
        let mut items: Vec<ListItem> = self
            .repos
            .iter()
            .map(|repo| {
                let status = if repo.is_modified {
                    ListStatus::Modified
                } else {
                    ListStatus::Ready
                };

                let status_text = if repo.is_modified {
                    jp::MODIFIED
                } else {
                    jp::READY
                };

                ListItem::new(repo.display_path())
                    .description(format!("Branch: {} | {}", repo.branch, repo.age))
                    .status(status)
                    .secondary(status_text.to_string())
            })
            .collect();

        // Add clone option
        if self.show_clone_option {
            items.push(
                ListItem::new("+ CLONE FROM GITHUB")
                    .description("Clone fresh from openai/codex".to_string())
                    .status(ListStatus::None),
            );
        }

        // Render list in a panel
        let list_area = Rect {
            x: chunks[2].x + 2,
            y: chunks[2].y,
            width: chunks[2].width.saturating_sub(4),
            height: chunks[2].height,
        };

        let panel = Panel::new().title("REPOSITORIES").focused(true);
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

        // Help text
        let help = "[↑↓] Navigate  [ENTER] Select  [Q] Quit";
        let help_x = area.x + (area.width.saturating_sub(help.len() as u16)) / 2;
        buf.set_string(help_x, chunks[3].y, help, theme::muted());
    }
}

/// Render a screen header with title and Japanese subtitle
fn render_header(area: Rect, buf: &mut Buffer, title: &str, jp_text: &str, frame: u64) {
    // Decorative line with title
    let decoration = "░▒▓█";
    let line = format!("{} {} //{} {}", decoration, title, jp_text, decoration);
    let x = area.x + (area.width.saturating_sub(line.len() as u16)) / 2;

    // Animated color
    let style = if frame % 60 < 30 {
        theme::title()
    } else {
        Style::default().fg(theme::CYAN_DIM)
    };

    buf.set_string(x, area.y + 1, &line, style);
}
