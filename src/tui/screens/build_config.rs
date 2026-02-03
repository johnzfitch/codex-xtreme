//! Build configuration screen for CPU target, linker, and optimization options

use crate::tui::theme;
use crate::tui::widgets::Panel;
use ratatui::{
    buffer::Buffer,
    layout::{Constraint, Layout, Rect},
    style::Style,
    widgets::Widget,
};

/// Build configuration option
#[derive(Clone)]
pub struct ConfigOption {
    pub name: String,
    pub description: String,
    pub enabled: bool,
    pub available: bool,
    pub detail: String,
}

/// Build configuration screen
pub struct BuildConfigScreen {
    frame: u64,
    cpu_target: String,
    cpu_detected_by: String,
    options: Vec<ConfigOption>,
    cursor: usize,
}

impl BuildConfigScreen {
    pub fn new(
        cpu_target: String,
        cpu_detected_by: String,
        has_mold: bool,
        has_bolt: bool,
    ) -> Self {
        let options = vec![
            ConfigOption {
                name: "Use mold linker".to_string(),
                description: "Faster linking (5-10x speedup)".to_string(),
                enabled: has_mold,
                available: has_mold,
                detail: if has_mold {
                    "found".to_string()
                } else {
                    "not installed".to_string()
                },
            },
            ConfigOption {
                name: "Use BOLT optimization".to_string(),
                description: "Post-link binary optimization".to_string(),
                enabled: has_bolt, // Auto-select if available
                available: has_bolt,
                detail: if has_bolt {
                    "found".to_string()
                } else {
                    "not installed".to_string()
                },
            },
            ConfigOption {
                name: "Release build".to_string(),
                description: "Optimized release profile with LTO".to_string(),
                enabled: true,
                available: true,
                detail: "recommended".to_string(),
            },
            ConfigOption {
                name: "Strip symbols".to_string(),
                description: "Remove debug symbols for smaller binary".to_string(),
                enabled: true,
                available: true,
                detail: "~50% size reduction".to_string(),
            },
        ];

        Self {
            frame: 0,
            cpu_target,
            cpu_detected_by,
            options,
            cursor: 0,
        }
    }

    pub fn tick(&mut self) {
        self.frame += 1;
    }

    pub fn select_next(&mut self) {
        if self.cursor < self.options.len().saturating_sub(1) {
            self.cursor += 1;
        }
    }

    pub fn select_prev(&mut self) {
        if self.cursor > 0 {
            self.cursor -= 1;
        }
    }

    pub fn toggle_current(&mut self) {
        if let Some(opt) = self.options.get_mut(self.cursor) {
            if opt.available {
                opt.enabled = !opt.enabled;
            }
        }
    }

    pub fn cpu_target(&self) -> &str {
        &self.cpu_target
    }

    pub fn use_mold(&self) -> bool {
        self.options.first().map(|o| o.enabled).unwrap_or(false)
    }

    pub fn use_bolt(&self) -> bool {
        self.options.get(1).map(|o| o.enabled).unwrap_or(false)
    }

    pub fn release_build(&self) -> bool {
        self.options.get(2).map(|o| o.enabled).unwrap_or(true)
    }

    pub fn strip_symbols(&self) -> bool {
        self.options.get(3).map(|o| o.enabled).unwrap_or(true)
    }

    pub fn frame(&self) -> u64 {
        self.frame
    }
}

impl Widget for &BuildConfigScreen {
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
            Constraint::Length(5), // CPU info panel
            Constraint::Length(1), // Spacer
            Constraint::Min(10),   // Options list
            Constraint::Length(2), // Help
        ])
        .split(area);

        // Header
        let header_line = "░▒▓█ BUILD CONFIG //ビルド設定 █▓▒░".to_string();
        let header_x = area.x + (area.width.saturating_sub(header_line.len() as u16)) / 2;
        buf.set_string(header_x, chunks[0].y + 1, &header_line, theme::title());

        // CPU panel
        let cpu_area = Rect {
            x: chunks[2].x + 2,
            y: chunks[2].y,
            width: chunks[2].width.saturating_sub(4),
            height: chunks[2].height,
        };

        let cpu_panel = Panel::new().title("CPU TARGET").title_jp("CPU対象");
        cpu_panel.render(cpu_area, buf);

        // CPU info
        buf.set_string(
            cpu_area.x + 2,
            cpu_area.y + 1,
            format!("Target: {}", self.cpu_target),
            theme::success(),
        );
        buf.set_string(
            cpu_area.x + 2,
            cpu_area.y + 2,
            format!("Detected by: {}", self.cpu_detected_by),
            theme::muted(),
        );

        // Options panel
        let opts_area = Rect {
            x: chunks[4].x + 2,
            y: chunks[4].y,
            width: chunks[4].width.saturating_sub(4),
            height: chunks[4].height,
        };

        let opts_panel = Panel::new()
            .title("OPTIONS")
            .title_jp("オプション")
            .focused(true);
        opts_panel.render(opts_area, buf);

        // Options list
        let inner_y = opts_area.y + 1;
        let inner_x = opts_area.x + 2;

        for (idx, opt) in self.options.iter().enumerate() {
            let y = inner_y + (idx as u16 * 2);
            if y >= opts_area.y + opts_area.height - 2 {
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
            let checkbox = if opt.enabled { "[✓]" } else { "[ ]" };
            let checkbox_style = if !opt.available {
                theme::muted()
            } else if opt.enabled {
                theme::success()
            } else {
                theme::secondary()
            };
            buf.set_string(inner_x + 2, y, checkbox, checkbox_style);

            // Name
            let name_style = if !opt.available {
                theme::muted()
            } else if is_cursor {
                theme::focused()
            } else {
                theme::normal()
            };
            buf.set_string(inner_x + 6, y, &opt.name, name_style);

            // Detail (right side)
            let detail_x = opts_area.x + opts_area.width - 2 - opt.detail.len() as u16;
            let detail_style = if opt.available {
                theme::secondary()
            } else {
                theme::muted()
            };
            buf.set_string(detail_x, y, &opt.detail, detail_style);

            // Description
            let desc = format!("      └─ {}", opt.description);
            buf.set_string(inner_x + 2, y + 1, &desc, theme::muted());
        }

        // Help text
        let help = "[↑↓] Navigate  [SPACE] Toggle  [ENTER] Build  [ESC] Back  [Q] Quit";
        let help_x = area.x + (area.width.saturating_sub(help.len() as u16)) / 2;
        buf.set_string(help_x, chunks[5].y, help, theme::muted());
    }
}
