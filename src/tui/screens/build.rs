//! Build progress screen with patching and compilation

use crate::tui::theme::{self, jp};
use crate::tui::widgets::{Panel, ProgressBar};

/// Wrap text to fit within max_width, breaking on word boundaries
fn wrap_text(text: &str, max_width: usize) -> Vec<String> {
    let mut lines = Vec::new();

    for paragraph in text.lines() {
        if paragraph.is_empty() {
            lines.push(String::new());
            continue;
        }

        let mut current_line = String::new();
        for word in paragraph.split_whitespace() {
            if current_line.is_empty() {
                if word.len() > max_width {
                    // Word is too long, just truncate it
                    lines.push(word[..max_width].to_string());
                } else {
                    current_line = word.to_string();
                }
            } else if current_line.len() + 1 + word.len() <= max_width {
                current_line.push(' ');
                current_line.push_str(word);
            } else {
                lines.push(current_line);
                current_line = word.to_string();
            }
        }
        if !current_line.is_empty() {
            lines.push(current_line);
        }
    }

    lines
}
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
    Installing,
    Complete,
    Error,
}

impl BuildPhase {
    fn title(&self) -> &'static str {
        match self {
            BuildPhase::Patching => "PATCHING",
            BuildPhase::Compiling => "COMPILING",
            BuildPhase::Installing => "INSTALLING",
            BuildPhase::Complete => "COMPLETE",
            BuildPhase::Error => "ERROR",
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
    patches_skipped: Vec<(String, String)>, // (name, reason)
    error_message: Option<String>,
    binary_path: Option<String>,
    build_time: Option<String>,
    // Build info
    version: String,
    install_path: String,
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
            patches_skipped: Vec::new(),
            error_message: None,
            binary_path: None,
            build_time: None,
            version: String::new(),
            install_path: String::new(),
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
        // Keep only last 20 lines for better visibility
        if self.log_lines.len() > 20 {
            self.log_lines.remove(0);
        }
    }

    pub fn add_patch(&mut self, name: impl Into<String>) {
        self.patches_applied.push(name.into());
    }

    pub fn add_skipped_patch(&mut self, name: impl Into<String>, reason: impl Into<String>) {
        self.patches_skipped.push((name.into(), reason.into()));
    }

    pub fn set_version(&mut self, version: impl Into<String>) {
        self.version = version.into();
    }

    pub fn set_install_path(&mut self, path: impl Into<String>) {
        self.install_path = path.into();
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
        Constraint::Length(3), // Build info (version, path) - condensed
        Constraint::Length(3), // Progress bar with phase label
        Constraint::Length(2), // Current item
        Constraint::Min(8),    // Log output - more space now
        Constraint::Length(2), // Help
    ])
    .split(area);

    // Build info
    if !screen.version.is_empty() {
        let version_line = format!("Version: {}", screen.version);
        buf.set_string(area.x + 4, chunks[0].y, &version_line, theme::secondary());
    }
    if !screen.install_path.is_empty() {
        let path_line = format!("Target:  {}", screen.install_path);
        let max_width = area.width.saturating_sub(8) as usize;
        let display_path = if path_line.len() > max_width && max_width > 3 {
            format!("{}...", &path_line[..max_width - 3])
        } else {
            path_line
        };
        buf.set_string(area.x + 4, chunks[0].y + 1, &display_path, theme::muted());
    }

    // Phase label inline with progress bar: "COMPILING ████████░░░░ 45%"
    let phase_label = format!("{} ", screen.phase.title());
    let phase_len = phase_label.len() as u16;
    buf.set_string(area.x + 4, chunks[1].y + 1, &phase_label, theme::title());

    // Progress bar after phase label
    let progress_area = Rect {
        x: area.x + 4 + phase_len,
        y: chunks[1].y + 1,
        width: area.width.saturating_sub(8 + phase_len),
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
    for (i, line) in screen
        .log_lines
        .iter()
        .rev()
        .take(log_area.height.saturating_sub(2) as usize)
        .enumerate()
    {
        let y = log_start_y + i as u16;
        let display_line: String = line
            .chars()
            .take(log_area.width.saturating_sub(4) as usize)
            .collect();
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
        Constraint::Length(6), // Banner
        Constraint::Length(2), // Spacer
        Constraint::Length(5), // Binary info
        Constraint::Min(5),    // Patches
        Constraint::Length(2), // Exit prompt
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

    // Banner text - check if decorations fit
    let inner_width = banner_width.saturating_sub(4) as usize; // Account for borders
    let base_title = "BUILD COMPLETE";
    let decorated_title = format!("████ {} ████", base_title);
    let title = if decorated_title.len() <= inner_width {
        decorated_title
    } else {
        base_title.to_string()
    };
    let title_x = banner_x + (banner_width.saturating_sub(title.len() as u16)) / 2;
    buf.set_string(
        title_x,
        banner_area.y + 1,
        &title,
        Style::default()
            .fg(theme::GREEN)
            .add_modifier(Modifier::BOLD),
    );

    // Japanese title - only show if it fits
    let jp_title = jp::BUILD_COMPLETE;
    let jp_width = jp_title.chars().count(); // Use char count for proper Unicode width
    if jp_width <= inner_width {
        let jp_x = banner_x + (banner_width.saturating_sub(jp_width as u16)) / 2;
        buf.set_string(jp_x, banner_area.y + 2, jp_title, theme::kanji());
    }

    // Binary info
    if let Some(ref path) = screen.binary_path {
        buf.set_string(
            area.x + 8,
            chunks[3].y,
            format!("Binary: {}", path),
            theme::normal(),
        );
    }
    if let Some(ref time) = screen.build_time {
        buf.set_string(
            area.x + 8,
            chunks[3].y + 1,
            format!("Time:   {}", time),
            theme::normal(),
        );
    }

    // Patches panel
    let patches_area = Rect {
        x: area.x + 4,
        y: chunks[4].y,
        width: area.width.saturating_sub(8),
        height: chunks[4].height,
    };

    let title = if screen.patches_skipped.is_empty() {
        "INSTALLED PATCHES"
    } else {
        "PATCH RESULTS"
    };
    let patches_panel = Panel::new().title(title);
    patches_panel.render(patches_area, buf);

    let mut y_offset = 0u16;
    let max_lines = patches_area.height.saturating_sub(2) as usize;

    // Applied patches
    for patch in screen.patches_applied.iter().take(max_lines) {
        let line = format!("  ✓ {}", patch);
        buf.set_string(
            patches_area.x + 2,
            patches_area.y + 1 + y_offset,
            &line,
            theme::success(),
        );
        y_offset += 1;
    }

    // Skipped patches
    let remaining_lines = max_lines.saturating_sub(y_offset as usize);
    for (name, reason) in screen.patches_skipped.iter().take(remaining_lines) {
        let line = format!("  ⊘ {} ({})", name, reason);
        let max_width = patches_area.width.saturating_sub(4) as usize;
        let display = if line.len() > max_width {
            format!("{}...", &line[..max_width.saturating_sub(3)])
        } else {
            line
        };
        buf.set_string(
            patches_area.x + 2,
            patches_area.y + 1 + y_offset,
            &display,
            theme::muted(),
        );
        y_offset += 1;
    }

    // Exit prompt
    let prompt = "Press any key to exit...";
    let prompt_x = area.x + (area.width.saturating_sub(prompt.len() as u16)) / 2;
    let prompt_style = if (screen.frame / 30).is_multiple_of(2) {
        theme::muted()
    } else {
        theme::secondary()
    };
    buf.set_string(prompt_x, chunks[5].y, prompt, prompt_style);
}

fn render_error(screen: &BuildScreen, area: Rect, buf: &mut Buffer) {
    let chunks = Layout::vertical([
        Constraint::Min(2),
        Constraint::Length(6), // Error banner
        Constraint::Min(8),    // Error message
        Constraint::Length(2), // Help
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
    buf.set_string(title_x, banner_area.y + 1, title, theme::error());

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

        // Word wrap the error message
        let max_line_width = msg_area.width.saturating_sub(4) as usize;
        let wrapped_lines = wrap_text(msg, max_line_width);

        for (i, line) in wrapped_lines
            .iter()
            .take(msg_area.height.saturating_sub(2) as usize)
            .enumerate()
        {
            buf.set_string(
                msg_area.x + 2,
                msg_area.y + 1 + i as u16,
                line,
                theme::normal(),
            );
        }
    }

    // Help
    let help = "Press [Q] to exit or [R] to retry";
    let help_x = area.x + (area.width.saturating_sub(help.len() as u16)) / 2;
    buf.set_string(help_x, chunks[3].y, help, theme::muted());
}
