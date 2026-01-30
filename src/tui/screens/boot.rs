//! Boot sequence screen with animated system checks

use crate::tui::theme::{self, jp, truncate_str, BANNER_LINES, BANNER_WIDTH};
use crate::tui::widgets::ProgressBar;
use ratatui::{
    buffer::Buffer,
    layout::{Constraint, Layout, Rect},
    style::{Modifier, Style},
    widgets::Widget,
};

/// System check item
#[derive(Clone)]
pub struct SystemCheck {
    pub name: String,
    pub status: CheckStatus,
    pub detail: Option<String>,
}

#[derive(Clone, Copy, PartialEq)]
pub enum CheckStatus {
    Pending,
    Checking,
    Ok,
    Warning,
    Error,
}

impl CheckStatus {
    fn indicator(&self, frame: u64) -> &'static str {
        match self {
            CheckStatus::Pending => "○",
            CheckStatus::Checking => {
                let dots = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];
                dots[(frame / 4) as usize % dots.len()]
            }
            CheckStatus::Ok => "✓",
            CheckStatus::Warning => "!",
            CheckStatus::Error => "✗",
        }
    }

    fn style(&self) -> Style {
        match self {
            CheckStatus::Pending => theme::muted(),
            CheckStatus::Checking => theme::active(),
            CheckStatus::Ok => theme::success(),
            CheckStatus::Warning => theme::warning(),
            CheckStatus::Error => theme::error(),
        }
    }
}

/// Boot sequence screen
pub struct BootScreen {
    frame: u64,
    checks: Vec<SystemCheck>,
    current_check: usize,
    complete: bool,
    dev_mode: bool,
    /// Frames since completion (for countdown)
    complete_frames: u64,
}

impl BootScreen {
    pub fn new(dev_mode: bool) -> Self {
        Self {
            frame: 0,
            checks: Vec::new(),
            current_check: 0,
            complete: false,
            dev_mode,
            complete_frames: 0,
        }
    }

    pub fn add_check(&mut self, name: impl Into<String>) {
        self.checks.push(SystemCheck {
            name: name.into(),
            status: CheckStatus::Pending,
            detail: None,
        });
    }

    pub fn add_check_with_detail(&mut self, name: impl Into<String>, detail: impl Into<String>) {
        self.checks.push(SystemCheck {
            name: name.into(),
            status: CheckStatus::Ok, // Pre-completed since we already have the result
            detail: Some(detail.into()),
        });
        // Advance current_check since this check is already complete
        self.current_check = self.checks.len();
    }

    pub fn tick(&mut self) {
        self.frame += 1;

        // Auto-advance checks for demo
        if !self.complete && self.frame.is_multiple_of(20) && self.current_check < self.checks.len()
        {
            if let Some(check) = self.checks.get_mut(self.current_check) {
                match check.status {
                    CheckStatus::Pending => {
                        check.status = CheckStatus::Checking;
                    }
                    CheckStatus::Checking => {
                        check.status = CheckStatus::Ok;
                        self.current_check += 1;
                    }
                    _ => {}
                }
            }
        }

        // Mark complete when all checks done
        if self.current_check >= self.checks.len() && !self.checks.is_empty() {
            if !self.complete {
                self.complete = true;
                self.complete_frames = 0;
            } else {
                self.complete_frames += 1;
            }
        }
    }

    /// Returns countdown number (3, 2, 1) or 0 if should advance
    pub fn countdown(&self) -> u8 {
        if !self.complete {
            return 0;
        }
        // ~60fps, so 60 frames = 1 second per number
        let seconds_elapsed = self.complete_frames / 60;
        match seconds_elapsed {
            0 => 3,
            1 => 2,
            2 => 1,
            _ => 0, // Ready to advance
        }
    }

    /// Returns true when countdown is done and should auto-advance
    pub fn should_auto_advance(&self) -> bool {
        self.complete && self.countdown() == 0
    }

    pub fn set_check_status(&mut self, idx: usize, status: CheckStatus, detail: Option<String>) {
        if let Some(check) = self.checks.get_mut(idx) {
            check.status = status;
            check.detail = detail;
            if status == CheckStatus::Ok
                || status == CheckStatus::Warning
                || status == CheckStatus::Error
            {
                self.current_check = self.current_check.max(idx + 1);
            }
        }
    }

    pub fn is_complete(&self) -> bool {
        self.complete
    }

    pub fn complete(&mut self) {
        for check in &mut self.checks {
            if check.status == CheckStatus::Pending || check.status == CheckStatus::Checking {
                check.status = CheckStatus::Ok;
            }
        }
        self.complete = true;
    }

    pub fn frame(&self) -> u64 {
        self.frame
    }

    pub fn progress(&self) -> f64 {
        if self.checks.is_empty() {
            return 1.0;
        }
        let completed = self
            .checks
            .iter()
            .filter(|c| {
                matches!(
                    c.status,
                    CheckStatus::Ok | CheckStatus::Warning | CheckStatus::Error
                )
            })
            .count();
        completed as f64 / self.checks.len() as f64
    }
}

impl Widget for &BootScreen {
    fn render(self, area: Rect, buf: &mut Buffer) {
        // Clear with background
        for y in area.y..(area.y + area.height) {
            for x in area.x..(area.x + area.width) {
                buf.set_string(x, y, " ", Style::default().bg(theme::BG_VOID));
            }
        }

        // Layout
        let chunks = Layout::vertical([
            Constraint::Min(2),    // Top padding
            Constraint::Length(8), // Banner
            Constraint::Length(2), // Subtitle
            Constraint::Length(1), // Spacer
            Constraint::Length(2), // Status line
            Constraint::Length(1), // Spacer
            Constraint::Min(5),    // System checks
            Constraint::Length(1), // Spacer
            Constraint::Length(1), // Progress bar
            Constraint::Min(2),    // Bottom padding
        ])
        .split(area);

        // Center the banner
        let banner_x = area.x + (area.width.saturating_sub(BANNER_WIDTH)) / 2;

        // Draw banner with color animation
        for (i, line) in BANNER_LINES.iter().enumerate() {
            let y = chunks[1].y + i as u16 + 1;
            if y < chunks[1].y + chunks[1].height {
                // Animated color per line
                let color = match (self.frame / 8 + i as u64) % 3 {
                    0 => theme::CYAN,
                    1 => theme::CYAN_DIM,
                    _ => theme::CYAN,
                };
                buf.set_string(
                    banner_x,
                    y,
                    *line,
                    Style::default().fg(color).add_modifier(Modifier::BOLD),
                );
            }
        }

        // Subtitle
        let subtitle = if self.dev_mode {
            format!("｜{}｜ XTREME EDITION [DEV]", jp::NEO_TOKYO)
        } else {
            format!("｜{}｜ XTREME EDITION", jp::NEO_TOKYO)
        };
        let subtitle_x = area.x + (area.width.saturating_sub(subtitle.len() as u16)) / 2;
        buf.set_string(subtitle_x, chunks[2].y, &subtitle, theme::kanji());

        // Status line with countdown
        let (status, status_style) = if self.complete {
            let countdown = self.countdown();
            if countdown > 0 {
                (
                    format!("SYSTEM READY //{}  [ {} ]", jp::SYSTEM_BOOT, countdown),
                    theme::success(),
                )
            } else {
                ("LAUNCHING //起動中".to_string(), theme::active())
            }
        } else {
            (
                format!("INITIALIZING SYSTEM //{}", jp::SYSTEM_BOOT),
                theme::active(),
            )
        };
        let status_x = area.x + (area.width.saturating_sub(status.len() as u16)) / 2;
        buf.set_string(status_x, chunks[4].y, &status, status_style);

        // System checks - responsive width
        let max_check_width = area.width.saturating_sub(8).min(80) as usize;
        let checks_x = area.x + (area.width.saturating_sub(max_check_width as u16)) / 2;
        let checks_y = chunks[6].y;

        // Calculate column widths
        let name_col_width = 20.min(max_check_width / 3);
        let detail_col_x = checks_x + name_col_width as u16 + 4;

        for (i, check) in self.checks.iter().enumerate() {
            let y = checks_y + i as u16;
            if y >= chunks[6].y + chunks[6].height {
                break;
            }

            // Indicator
            let indicator = check.status.indicator(self.frame);
            buf.set_string(checks_x, y, indicator, check.status.style());

            // Name (truncate if needed)
            let name_style = match check.status {
                CheckStatus::Pending => theme::muted(),
                CheckStatus::Checking => theme::active(),
                _ => theme::normal(),
            };
            let display_name = truncate_str(&check.name, name_col_width);
            buf.set_string(checks_x + 3, y, &display_name, name_style);

            // Detail (truncate to fit remaining space)
            if let Some(ref detail) = check.detail {
                let detail_max_width =
                    (area.x + area.width).saturating_sub(detail_col_x + 2) as usize;
                let display_detail = truncate_str(detail, detail_max_width);
                buf.set_string(detail_col_x, y, &display_detail, theme::secondary());
            }
        }

        // Progress bar
        let progress_area = Rect {
            x: area.x + 4,
            y: chunks[8].y,
            width: area.width.saturating_sub(8),
            height: 1,
        };
        let progress = ProgressBar::new(self.progress())
            .frame(self.frame)
            .show_percentage(true);
        progress.render(progress_area, buf);
    }
}
