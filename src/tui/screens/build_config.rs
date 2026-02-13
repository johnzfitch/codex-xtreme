//! Build configuration screen for CPU target, linker, and optimization options

use crate::tui::theme::{self, center_x};
use crate::tui::widgets::Panel;
use crate::workflow::{OptimizationFlags, OptimizationMode};
use ratatui::{
    buffer::Buffer,
    layout::{Constraint, Layout, Rect},
    style::Style,
    widgets::Widget,
};
use unicode_width::UnicodeWidthStr;

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
    optimization_mode: OptimizationMode,
    has_mold: bool,
    has_bolt: bool,
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
        let optimization_mode = if has_bolt {
            OptimizationMode::RunFast
        } else if has_mold {
            OptimizationMode::BuildFast
        } else {
            OptimizationMode::Custom
        };

        let options = vec![
            ConfigOption {
                name: "Optimization mode".to_string(),
                description: "Build fast (mold) vs run fast (BOLT) vs custom".to_string(),
                enabled: true,
                available: true,
                detail: String::new(), // filled in by sync_from_mode()
            },
            ConfigOption {
                name: "Optimize for CPU".to_string(),
                description: "Use -C target-cpu=native (best runtime performance)".to_string(),
                enabled: true,
                available: true,
                detail: "recommended".to_string(),
            },
            ConfigOption {
                name: "Use mold linker".to_string(),
                description: "Faster linking (custom mode only)".to_string(),
                enabled: false,
                available: false,
                detail: if has_mold {
                    "found".to_string()
                } else {
                    "not installed".to_string()
                },
            },
            ConfigOption {
                name: "Use BOLT optimization".to_string(),
                description: "Post-link binary optimization (custom mode only)".to_string(),
                enabled: false,
                available: false,
                detail: if has_bolt {
                    "found".to_string()
                } else {
                    "not installed".to_string()
                },
            },
            ConfigOption {
                name: "Use xtreme profile".to_string(),
                description: "Thin LTO + 1 codegen unit (slower build, faster runtime)".to_string(),
                enabled: true,
                available: true,
                detail: "recommended".to_string(), // matches CLI default
            },
            ConfigOption {
                name: "Strip symbols".to_string(),
                description: "Remove debug symbols for smaller binary".to_string(),
                enabled: true,
                available: true,
                detail: "~50% size reduction".to_string(),
            },
            ConfigOption {
                name: "Run verification tests".to_string(),
                description: "cargo check + core library tests".to_string(),
                enabled: true,
                available: true,
                detail: "recommended".to_string(),
            },
            ConfigOption {
                name: "Set up shell alias".to_string(),
                description: "Add/update alias in your shell rc file".to_string(),
                enabled: true,
                available: true,
                detail: "recommended".to_string(),
            },
        ];

        let mut s = Self {
            frame: 0,
            cpu_target,
            cpu_detected_by,
            optimization_mode,
            has_mold,
            has_bolt,
            options,
            cursor: 0,
        };
        s.sync_from_mode();
        s
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
        // Optimization mode is a selector (cycles), not a checkbox.
        if self.cursor == 0 {
            self.optimization_mode = match self.optimization_mode {
                OptimizationMode::BuildFast => {
                    if self.has_bolt {
                        OptimizationMode::RunFast
                    } else {
                        OptimizationMode::Custom
                    }
                }
                OptimizationMode::RunFast => OptimizationMode::Custom,
                OptimizationMode::Custom => OptimizationMode::BuildFast,
            };
            self.sync_from_mode();
            return;
        }

        if let Some(opt) = self.options.get_mut(self.cursor) {
            if opt.available {
                opt.enabled = !opt.enabled;
            }
        }

        // If we're in custom mode, enforce invariants after any toggle.
        self.sync_from_mode();
    }

    pub fn cpu_target(&self) -> &str {
        &self.cpu_target
    }

    pub fn optimization_mode(&self) -> OptimizationMode {
        self.optimization_mode
    }

    pub fn optimization_flags(&self) -> OptimizationFlags {
        let mut flags = OptimizationFlags {
            use_mold: self.options.get(2).map(|o| o.enabled).unwrap_or(false),
            use_bolt: self.options.get(3).map(|o| o.enabled).unwrap_or(false),
        };
        flags.enforce_invariants();
        flags
    }

    pub fn optimize_cpu(&self) -> bool {
        self.options.get(1).map(|o| o.enabled).unwrap_or(true)
    }

    pub fn use_mold(&self) -> bool {
        self.options.get(2).map(|o| o.enabled).unwrap_or(false)
    }

    pub fn use_bolt(&self) -> bool {
        self.options.get(3).map(|o| o.enabled).unwrap_or(false)
    }

    pub fn use_xtreme_profile(&self) -> bool {
        self.options.get(4).map(|o| o.enabled).unwrap_or(true)
    }

    pub fn strip_symbols(&self) -> bool {
        self.options.get(5).map(|o| o.enabled).unwrap_or(true)
    }

    pub fn run_tests(&self) -> bool {
        self.options.get(6).map(|o| o.enabled).unwrap_or(true)
    }

    pub fn setup_alias(&self) -> bool {
        self.options.get(7).map(|o| o.enabled).unwrap_or(true)
    }

    pub fn frame(&self) -> u64 {
        self.frame
    }

    fn sync_from_mode(&mut self) {
        // Keep the UI in sync with the selected mode and tool availability.
        let (mut use_mold, use_bolt) = match self.optimization_mode {
            OptimizationMode::BuildFast => (self.has_mold, false),
            OptimizationMode::RunFast => (false, self.has_bolt),
            OptimizationMode::Custom => (
                self.options.get(2).map(|o| o.enabled).unwrap_or(false),
                self.options.get(3).map(|o| o.enabled).unwrap_or(false),
            ),
        };

        // BOLT => no mold (perf2bolt incompatibility on mold-linked binaries).
        if use_bolt {
            use_mold = false;
        }

        // Update the mode detail line.
        let mode_label = match self.optimization_mode {
            OptimizationMode::BuildFast => "Build fast (mold)",
            OptimizationMode::RunFast => "Run fast (BOLT)",
            OptimizationMode::Custom => "Custom",
        };
        if let Some(mode_opt) = self.options.first_mut() {
            mode_opt.detail = match self.optimization_mode {
                OptimizationMode::Custom => format!(
                    "{}  mold:{}  BOLT:{}",
                    mode_label,
                    if use_mold { "on" } else { "off" },
                    if use_bolt { "on" } else { "off" }
                ),
                _ => mode_label.to_string(),
            };
        }

        // Custom-only knobs.
        let custom = self.optimization_mode == OptimizationMode::Custom;
        if let Some(mold_opt) = self.options.get_mut(2) {
            mold_opt.available = custom && self.has_mold;
            mold_opt.enabled = if custom {
                use_mold
            } else {
                use_mold && self.has_mold
            };
            if !self.has_mold {
                mold_opt.detail = "not installed".to_string();
            } else if !custom {
                mold_opt.detail = "managed by mode".to_string();
            } else {
                mold_opt.detail = "found".to_string();
            }
        }
        if let Some(bolt_opt) = self.options.get_mut(3) {
            bolt_opt.available = custom && self.has_bolt;
            bolt_opt.enabled = if custom {
                use_bolt
            } else {
                use_bolt && self.has_bolt
            };
            if !self.has_bolt {
                bolt_opt.detail = "not installed".to_string();
            } else if !custom {
                bolt_opt.detail = "managed by mode".to_string();
            } else {
                bolt_opt.detail = "found".to_string();
            }
        }
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
        let header_w = UnicodeWidthStr::width(header_line.as_str()) as u16;
        let header_x = center_x(area.x, area.width, header_w);
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

            // Checkbox / selector glyph
            let (checkbox, checkbox_style) = if idx == 0 {
                // Optimization mode is a selector (cycle), not a boolean toggle.
                ("[<>]".to_string(), theme::secondary())
            } else {
                let checkbox = if opt.enabled { "[✓]" } else { "[ ]" };
                let style = if !opt.available {
                    theme::muted()
                } else if opt.enabled {
                    theme::success()
                } else {
                    theme::secondary()
                };
                (checkbox.to_string(), style)
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
