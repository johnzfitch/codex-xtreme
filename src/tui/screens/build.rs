//! Build progress screen with patching and compilation

use crate::tui::theme::{self, jp};
use crate::tui::widgets::{Panel, ProgressBar};
use ratatui::{
    buffer::Buffer,
    layout::{Constraint, Layout, Rect},
    style::{Modifier, Style},
    widgets::Widget,
};

/// Current build phase
#[derive(Clone, Copy, PartialEq)]
pub enum BuildPhase {
    Patching,
    Compiling,
    Optimizing,
    Complete,
    Error,
}

impl BuildPhase {
    fn title(&self) -> &'static str {
        match self {
            BuildPhase::Patching => "PATCHING",
            BuildPhase::Compiling => "COMPILING",
            BuildPhase::Optimizing => "OPTIMIZING",
            BuildPhase::Complete => "COMPLETE",
            BuildPhase::Error => "ERROR",
        }
    }

    fn jp(&self) -> &'static str {
        match self {
            BuildPhase::Patching => jp::INJECTING,
            BuildPhase::Compiling => jp::COMPILING,
            BuildPhase::Optimizing => "最適化中",
            BuildPhase::Complete => jp::BUILD_COMPLETE,
            BuildPhase::Error => "エラー",
        }
    }
}

/// Build progress screen
pub struct BuildScreen {
    frame: u64,
    phase: BuildPhase,
    progress: f64,
    current_item: String,
    log_lines: Vec<String>,
    patches_applied: Vec<String>,
    error_message: Option<String>,
    binary_path: Option<String>,
    build_time: Option<String>,
}

impl BuildScreen {
    pub fn new() -> Self {
        Self {
            frame: 0,
            phase: BuildPhase::Patching,
            progress: 0.0,
            current_item: String::new(),
            log_lines: Vec::new(),
            patches_applied: Vec::new(),
            error_message: None,
            binary_path: None,
            build_time: None,
        }
    }

    pub fn tick(&mut self) {
        self.frame += 1;
    }

    pub fn set_phase(&mut self, phase: BuildPhase) {
        self.phase = phase;
        self.progress = 0.0;
    }

    pub fn set_progress(&mut self, progress: f64) {
        self.progress = progress.clamp(0.0, 1.0);
    }

    pub fn set_current_item(&mut self, item: impl Into<String>) {
        self.current_item = item.into();
    }

    pub fn add_log(&mut self, line: impl Into<String>) {
        self.log_lines.push(line.into());
        // Keep only last 10 lines
        if self.log_lines.len() > 10 {
            self.log_lines.remove(0);
        }
    }

    pub fn add_patch(&mut self, name: impl Into<String>) {
        self.patches_applied.push(name.into());
    }

    pub fn set_error(&mut self, msg: impl Into<String>) {
        self.phase = BuildPhase::Error;
        self.error_message = Some(msg.into());
    }

    pub fn set_complete(&mut self, binary_path: String, build_time: String) {
        self.phase = BuildPhase::Complete;
        self.progress = 1.0;
        self.binary_path = Some(binary_path);
        self.build_time = Some(build_time);
    }

    pub fn is_complete(&self) -> bool {
        self.phase == BuildPhase::Complete
    }

    pub fn is_error(&self) -> bool {
        self.phase == BuildPhase::Error
    }

    pub fn frame(&self) -> u64 {
        self.frame
    }
}

impl Default for BuildScreen {
    fn default() -> Self {
        Self::new()
    }
}

impl Widget for &BuildScreen {
    fn render(self, area: Rect, buf: &mut Buffer) {
        // Clear background
        for y in area.y..(area.y + area.height) {
            for x in area.x..(area.x + area.width) {
                buf.set_string(x, y, " ", Style::default().bg(theme::BG_VOID));
            }
        }

        if self.phase == BuildPhase::Complete {
            render_complete(self, area, buf);
        } else if self.phase == BuildPhase::Error {
            render_error(self, area, buf);
        } else {
            render_progress(self, area, buf);
        }
    }
}

fn render_progress(screen: &BuildScreen, area: Rect, buf: &mut Buffer) {
    let chunks = Layout::vertical([
        Constraint::Length(4),   // Header
        Constraint::Length(3),   // Progress bar
        Constraint::Length(2),   // Current item
        Constraint::Min(8),      // Log output
        Constraint::Length(2),   // Help
    ])
    .split(area);

    // Header with phase
    let header_line = format!(
        "░▒▓█ {} //{} █▓▒░",
        screen.phase.title(),
        screen.phase.jp()
    );
    let header_x = area.x + (area.width.saturating_sub(header_line.len() as u16)) / 2;
    buf.set_string(header_x, chunks[0].y + 1, &header_line, theme::title());

    // Progress bar
    let progress_area = Rect {
        x: area.x + 4,
        y: chunks[1].y + 1,
        width: area.width.saturating_sub(8),
        height: 1,
    };
    let progress = ProgressBar::new(screen.progress)
        .frame(screen.frame)
        .show_percentage(true);
    progress.render(progress_area, buf);

    // Current item with spinner
    if !screen.current_item.is_empty() {
        let spinner_chars = theme::spinners::BRAILLE;
        let spinner = spinner_chars[(screen.frame / 4) as usize % spinner_chars.len()];

        let line = format!("{} {}", spinner, screen.current_item);
        let x = area.x + 4;
        buf.set_string(x, chunks[2].y, &line, theme::active());
    }

    // Log panel
    let log_area = Rect {
        x: chunks[3].x + 2,
        y: chunks[3].y,
        width: chunks[3].width.saturating_sub(4),
        height: chunks[3].height,
    };

    let log_panel = Panel::new().title("OUTPUT");
    log_panel.render(log_area, buf);

    // Log lines
    let log_start_y = log_area.y + 1;
    for (i, line) in screen.log_lines.iter().rev().take(log_area.height.saturating_sub(2) as usize).enumerate() {
        let y = log_start_y + i as u16;
        let display_line: String = line.chars().take(log_area.width.saturating_sub(4) as usize).collect();
        buf.set_string(log_area.x + 2, y, &display_line, theme::code());
    }

    // Help
    let help = "Building... Press [Q] to cancel";
    let help_x = area.x + (area.width.saturating_sub(help.len() as u16)) / 2;
    buf.set_string(help_x, chunks[4].y, help, theme::muted());
}

fn render_complete(screen: &BuildScreen, area: Rect, buf: &mut Buffer) {
    let chunks = Layout::vertical([
        Constraint::Min(2),
        Constraint::Length(6),   // Banner
        Constraint::Length(2),   // Spacer
        Constraint::Length(5),   // Binary info
        Constraint::Min(5),      // Patches
        Constraint::Length(2),   // Exit prompt
    ])
    .split(area);

    // Success banner
    let banner_width = 40u16.min(area.width - 4);
    let banner_x = area.x + (area.width - banner_width) / 2;
    let banner_area = Rect {
        x: banner_x,
        y: chunks[1].y,
        width: banner_width,
        height: 4,
    };

    let panel = Panel::new().double_border().focused(true);
    panel.render(banner_area, buf);

    // Banner text
    let title = format!("████ {} ████", "BUILD COMPLETE");
    let title_x = banner_x + (banner_width.saturating_sub(title.len() as u16)) / 2;
    buf.set_string(
        title_x,
        banner_area.y + 1,
        &title,
        Style::default().fg(theme::GREEN).add_modifier(Modifier::BOLD),
    );

    let jp_title = jp::BUILD_COMPLETE;
    let jp_x = banner_x + (banner_width.saturating_sub(jp_title.len() as u16)) / 2;
    buf.set_string(jp_x, banner_area.y + 2, jp_title, theme::kanji());

    // Binary info
    if let Some(ref path) = screen.binary_path {
        buf.set_string(area.x + 8, chunks[3].y, format!("Binary: {}", path), theme::normal());
    }
    if let Some(ref time) = screen.build_time {
        buf.set_string(area.x + 8, chunks[3].y + 1, format!("Time:   {}", time), theme::normal());
    }

    // Patches panel
    let patches_area = Rect {
        x: area.x + 4,
        y: chunks[4].y,
        width: area.width.saturating_sub(8),
        height: chunks[4].height,
    };

    let patches_panel = Panel::new().title("INSTALLED PATCHES");
    patches_panel.render(patches_area, buf);

    for (i, patch) in screen.patches_applied.iter().take(patches_area.height.saturating_sub(2) as usize).enumerate() {
        let line = format!("  ✓ {}", patch);
        buf.set_string(patches_area.x + 2, patches_area.y + 1 + i as u16, &line, theme::success());
    }

    // Exit prompt
    let prompt = "Press any key to exit...";
    let prompt_x = area.x + (area.width.saturating_sub(prompt.len() as u16)) / 2;
    let prompt_style = if (screen.frame / 30) % 2 == 0 {
        theme::muted()
    } else {
        theme::secondary()
    };
    buf.set_string(prompt_x, chunks[5].y, prompt, prompt_style);
}

fn render_error(screen: &BuildScreen, area: Rect, buf: &mut Buffer) {
    let chunks = Layout::vertical([
        Constraint::Min(2),
        Constraint::Length(6),   // Error banner
        Constraint::Min(8),      // Error message
        Constraint::Length(2),   // Help
    ])
    .split(area);

    // Error banner
    let banner_width = 40u16.min(area.width - 4);
    let banner_x = area.x + (area.width - banner_width) / 2;
    let banner_area = Rect {
        x: banner_x,
        y: chunks[1].y,
        width: banner_width,
        height: 4,
    };

    // Draw error border
    let panel = Panel::new().double_border();
    panel.render(banner_area, buf);

    let title = "BUILD FAILED";
    let title_x = banner_x + (banner_width.saturating_sub(title.len() as u16)) / 2;
    buf.set_string(
        title_x,
        banner_area.y + 1,
        title,
        theme::error(),
    );

    // Error message
    if let Some(ref msg) = screen.error_message {
        let msg_area = Rect {
            x: area.x + 4,
            y: chunks[2].y,
            width: area.width.saturating_sub(8),
            height: chunks[2].height,
        };

        let error_panel = Panel::new().title("ERROR");
        error_panel.render(msg_area, buf);

        // Word wrap would be nice here
        let lines: Vec<&str> = msg.lines().collect();
        for (i, line) in lines.iter().take(msg_area.height.saturating_sub(2) as usize).enumerate() {
            buf.set_string(msg_area.x + 2, msg_area.y + 1 + i as u16, *line, theme::normal());
        }
    }

    // Help
    let help = "Press [Q] to exit or [R] to retry";
    let help_x = area.x + (area.width.saturating_sub(help.len() as u16)) / 2;
    buf.set_string(help_x, chunks[3].y, help, theme::muted());
}
