//! Patch selection screen with checkboxes

use crate::tui::theme::{self, center_x, jp, truncate_str};
use crate::tui::widgets::Panel;
use ratatui::{
    buffer::Buffer,
    layout::{Constraint, Layout, Rect},
    style::Style,
    widgets::Widget,
};
use std::path::PathBuf;
use unicode_width::UnicodeWidthStr;

/// Patch file information
#[derive(Clone)]
pub struct PatchInfo {
    pub path: PathBuf,
    pub name: String,
    pub description: String,
    pub patch_count: usize,
    pub selected: bool,
    pub compatible: bool,
}

/// Patch selection screen
pub struct PatchSelectScreen {
    frame: u64,
    patches: Vec<PatchInfo>,
    cursor: usize,
    target_version: String,
}

impl PatchSelectScreen {
    pub fn new(patches: Vec<PatchInfo>, target_version: String) -> Self {
        Self {
            frame: 0,
            patches,
            cursor: 0,
            target_version,
        }
    }

    pub fn tick(&mut self) {
        self.frame += 1;
    }

    pub fn select_next(&mut self) {
        if self.cursor < self.patches.len().saturating_sub(1) {
            self.cursor += 1;
        }
    }

    pub fn select_prev(&mut self) {
        if self.cursor > 0 {
            self.cursor -= 1;
        }
    }

    pub fn toggle_current(&mut self) {
        if let Some(patch) = self.patches.get_mut(self.cursor) {
            // Match CLI behavior: allow selecting "incompatible" patches too.
            // The UI will surface incompatibility in the compatibility panel styling.
            patch.selected = !patch.selected;
        }
    }

    pub fn select_all(&mut self) {
        for patch in &mut self.patches {
            patch.selected = true;
        }
    }

    pub fn select_none(&mut self) {
        for patch in &mut self.patches {
            patch.selected = false;
        }
    }

    pub fn selected_patches(&self) -> Vec<&PatchInfo> {
        self.patches.iter().filter(|p| p.selected).collect()
    }

    pub fn selected_patch_paths(&self) -> Vec<&PathBuf> {
        self.patches
            .iter()
            .filter(|p| p.selected)
            .map(|p| &p.path)
            .collect()
    }

    pub fn frame(&self) -> u64 {
        self.frame
    }
}

impl Widget for &PatchSelectScreen {
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
            Constraint::Min(10),   // Patch list
            Constraint::Length(4), // Compatibility info
            Constraint::Length(2), // Help
        ])
        .split(area);

        // Header
        let header_line = format!("░▒▓█ LOAD PATCHES //{} █▓▒░", jp::PATCH_LOAD);
        let header_w = UnicodeWidthStr::width(header_line.as_str()) as u16;
        let header_x = center_x(area.x, area.width, header_w);
        buf.set_string(header_x, chunks[0].y + 1, &header_line, theme::title());

        // Patch list panel
        let list_area = Rect {
            x: chunks[2].x + 2,
            y: chunks[2].y,
            width: chunks[2].width.saturating_sub(4),
            height: chunks[2].height,
        };

        let panel = Panel::new().title("PATCHES").focused(true);
        panel.render(list_area, buf);

        // Patch list content (compact 1-line per patch)
        let inner_y = list_area.y + 1;
        let inner_x = list_area.x + 2;

        for (idx, patch) in self.patches.iter().enumerate() {
            let y = inner_y + idx as u16;
            if y >= list_area.y + list_area.height - 1 {
                break;
            }

            let is_cursor = idx == self.cursor;

            // Cursor indicator
            let cursor_char = if is_cursor {
                let chars = ['▸', '▹'];
                chars[(self.frame / 8) as usize % chars.len()]
            } else {
                ' '
            };
            buf.set_string(inner_x, y, cursor_char.to_string(), theme::cursor());

            // Checkbox
            let checkbox = if patch.selected { "[x]" } else { "[ ]" };
            let checkbox_style = if !patch.compatible {
                theme::muted()
            } else if patch.selected {
                theme::success()
            } else {
                theme::secondary()
            };
            buf.set_string(inner_x + 2, y, checkbox, checkbox_style);

            // Name with patch count
            let name_with_count = format!("{} ({})", patch.name, patch.patch_count);
            let name_style = if !patch.compatible {
                theme::muted()
            } else if is_cursor {
                theme::focused()
            } else {
                theme::normal()
            };
            buf.set_string(inner_x + 6, y, &name_with_count, name_style);

            // Description (truncate to fit remaining width)
            let name_end = inner_x + 6 + name_with_count.len() as u16 + 2;
            let available_width = list_area
                .x
                .saturating_add(list_area.width)
                .saturating_sub(name_end + 2) as usize;
            if available_width > 4 {
                let truncated = truncate_str(&patch.description, available_width);
                buf.set_string(name_end, y, truncated, theme::muted());
            }
        }

        // Compatibility panel
        let compat_area = Rect {
            x: chunks[3].x + 2,
            y: chunks[3].y,
            width: chunks[3].width.saturating_sub(4),
            height: chunks[3].height,
        };

        let compat_panel = Panel::new()
            .title("COMPATIBILITY")
            .title_jp(jp::COMPATIBILITY);
        compat_panel.render(compat_area, buf);

        // Compatibility info
        let selected = self.patches.iter().filter(|p| p.selected).count();
        let compatible = self.patches.iter().filter(|p| p.compatible).count();
        let total_patches: usize = self
            .patches
            .iter()
            .filter(|p| p.selected)
            .map(|p| p.patch_count)
            .sum();
        let compat_msg = format!(
            "  {} files / {} patches selected / {} compatible with {}",
            selected, total_patches, compatible, self.target_version
        );

        let all_ok = self
            .patches
            .iter()
            .filter(|p| p.selected)
            .all(|p| p.compatible);
        let compat_style = if all_ok && selected > 0 {
            theme::success()
        } else if selected == 0 {
            theme::muted()
        } else {
            theme::warning()
        };
        buf.set_string(
            compat_area.x + 2,
            compat_area.y + 1,
            &compat_msg,
            compat_style,
        );

        // Help text
        let help = "[SPACE] Toggle  [A] All  [N] None  [ENTER] Apply  [ESC] Back  [Q] Quit";
        let help_x = area.x + (area.width.saturating_sub(help.len() as u16)) / 2;
        buf.set_string(help_x, chunks[4].y, help, theme::muted());
    }
}
