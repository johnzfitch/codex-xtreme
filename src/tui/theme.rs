//! Neo Tokyo Y2K color palette and styling
//!
//! A refined cyberpunk aesthetic with careful attention to contrast and hierarchy.

use ratatui::style::{Color, Modifier, Style};

// ============================================================================
// Color Palette - Neo Tokyo 2077
// ============================================================================

/// Primary cyan accent - electric, alive
pub const CYAN: Color = Color::Rgb(0, 255, 255);
pub const CYAN_DIM: Color = Color::Rgb(0, 180, 180);
pub const CYAN_DARK: Color = Color::Rgb(0, 100, 100);

/// Secondary magenta accent - warnings, transitions
pub const MAGENTA: Color = Color::Rgb(255, 0, 255);
pub const MAGENTA_DIM: Color = Color::Rgb(180, 0, 180);

/// Matrix green - success, code, data
pub const GREEN: Color = Color::Rgb(0, 255, 100);
pub const GREEN_DIM: Color = Color::Rgb(0, 180, 70);
pub const GREEN_DARK: Color = Color::Rgb(0, 80, 40);

/// Hot pink - errors, urgent
pub const PINK: Color = Color::Rgb(255, 51, 102);

/// Near-black backgrounds with subtle blue tint
pub const BG_VOID: Color = Color::Rgb(8, 8, 14);
pub const BG: Color = Color::Rgb(12, 12, 20);
pub const BG_ELEVATED: Color = Color::Rgb(18, 18, 28);
pub const BG_HIGHLIGHT: Color = Color::Rgb(25, 25, 40);

/// Text colors
pub const TEXT_PRIMARY: Color = Color::Rgb(230, 230, 240);
pub const TEXT_SECONDARY: Color = Color::Rgb(160, 160, 180);
pub const TEXT_MUTED: Color = Color::Rgb(80, 80, 100);
pub const TEXT_DIM: Color = Color::Rgb(50, 50, 65);

/// Accent colors
pub const YELLOW: Color = Color::Rgb(255, 220, 0);
pub const ORANGE: Color = Color::Rgb(255, 140, 0);
pub const WHITE: Color = Color::Rgb(255, 255, 255);

// ============================================================================
// Style Presets
// ============================================================================

/// Main title style - bold cyan
pub fn title() -> Style {
    Style::default()
        .fg(CYAN)
        .add_modifier(Modifier::BOLD)
}

/// Large banner title
pub fn banner() -> Style {
    Style::default()
        .fg(CYAN)
        .add_modifier(Modifier::BOLD)
}

/// Highlighted/selected item
pub fn highlight() -> Style {
    Style::default()
        .fg(WHITE)
        .bg(CYAN_DARK)
        .add_modifier(Modifier::BOLD)
}

/// Active/focused element
pub fn focused() -> Style {
    Style::default()
        .fg(CYAN)
        .add_modifier(Modifier::BOLD)
}

/// Normal text
pub fn normal() -> Style {
    Style::default().fg(TEXT_PRIMARY)
}

/// Secondary text
pub fn secondary() -> Style {
    Style::default().fg(TEXT_SECONDARY)
}

/// Muted/inactive text
pub fn muted() -> Style {
    Style::default().fg(TEXT_MUTED)
}

/// Very dim text
pub fn dim() -> Style {
    Style::default().fg(TEXT_DIM)
}

/// Success messages
pub fn success() -> Style {
    Style::default()
        .fg(GREEN)
        .add_modifier(Modifier::BOLD)
}

/// Error messages
pub fn error() -> Style {
    Style::default()
        .fg(PINK)
        .add_modifier(Modifier::BOLD)
}

/// Warning messages
pub fn warning() -> Style {
    Style::default().fg(YELLOW)
}

/// Active/in-progress
pub fn active() -> Style {
    Style::default()
        .fg(MAGENTA)
        .add_modifier(Modifier::BOLD)
}

/// Code/technical content
pub fn code() -> Style {
    Style::default().fg(GREEN_DIM)
}

/// Border style (inactive)
pub fn border() -> Style {
    Style::default().fg(TEXT_DIM)
}

/// Border style (active/focused)
pub fn border_focused() -> Style {
    Style::default().fg(CYAN_DIM)
}

/// Japanese accent text
pub fn kanji() -> Style {
    Style::default()
        .fg(MAGENTA_DIM)
        .add_modifier(Modifier::DIM)
}

/// Cursor/selection indicator
pub fn cursor() -> Style {
    Style::default()
        .fg(CYAN)
        .add_modifier(Modifier::BOLD)
}

// ============================================================================
// Japanese Text Constants
// ============================================================================

pub mod jp {
    pub const SYSTEM_BOOT: &str = "システム起動中";
    pub const TARGET_SELECT: &str = "ターゲット選択";
    pub const VERSION_SELECT: &str = "バージョン選択";
    pub const PATCH_LOAD: &str = "パッチ読込";
    pub const BUILD_CONFIG: &str = "ビルド設定";
    pub const COMPILING: &str = "コンパイル中";
    pub const COMPLETE: &str = "完了";
    pub const READY: &str = "準備完了";
    pub const MODIFIED: &str = "変更あり";
    pub const INJECTING: &str = "注入中";
    pub const PATCH_COMPLETE: &str = "パッチ完了";
    pub const BUILD_COMPLETE: &str = "ビルド完了";
    pub const NEO_TOKYO: &str = "ネオ東京";
    pub const XTREME: &str = "エクストリーム";
    pub const CHANGELOG: &str = "変更履歴";
    pub const COMPATIBILITY: &str = "互換性";
    pub const CONNECTING: &str = "接続中";
    pub const CLONING: &str = "クローン中";
}

// ============================================================================
// Block Characters
// ============================================================================

pub mod blocks {
    pub const FULL: char = '█';
    pub const DARK: char = '▓';
    pub const MEDIUM: char = '▒';
    pub const LIGHT: char = '░';

    /// Progress bar segments
    pub const PROGRESS_FULL: &str = "█";
    pub const PROGRESS_PARTIAL: &[&str] = &["▏", "▎", "▍", "▌", "▋", "▊", "▉", "█"];
    pub const PROGRESS_EMPTY: &str = "░";
}

// ============================================================================
// Spinners
// ============================================================================

pub mod spinners {
    pub const BRAILLE: &[char] = &['⠋', '⠙', '⠹', '⠸', '⠼', '⠴', '⠦', '⠧', '⠇', '⠏'];
    pub const DOTS: &[&str] = &["⠁", "⠂", "⠄", "⡀", "⢀", "⠠", "⠐", "⠈"];
    pub const BLOCKS: &[char] = &['▖', '▘', '▝', '▗'];
    pub const ARROWS: &[char] = &['←', '↖', '↑', '↗', '→', '↘', '↓', '↙'];
}

// ============================================================================
// ASCII Art
// ============================================================================

/// Main banner (centered, 6 lines)
pub const BANNER_LINES: &[&str] = &[
    "  ██████╗ ██████╗ ██████╗ ███████╗██╗  ██╗",
    " ██╔════╝██╔═══██╗██╔══██╗██╔════╝╚██╗██╔╝",
    " ██║     ██║   ██║██║  ██║█████╗   ╚███╔╝ ",
    " ██║     ██║   ██║██║  ██║██╔══╝   ██╔██╗ ",
    " ╚██████╗╚██████╔╝██████╔╝███████╗██╔╝ ██╗",
    "  ╚═════╝ ╚═════╝ ╚═════╝ ╚══════╝╚═╝  ╚═╝",
];

/// Banner width for centering
pub const BANNER_WIDTH: u16 = 43;
