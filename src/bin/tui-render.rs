//! Render a few TUI screens to plain text using ratatui's TestBackend.
//!
//! This is a developer utility to visually sanity-check layout proportions
//! without needing an interactive terminal session.

use codex_xtreme::tui::screens::{
    BuildConfigScreen, CherryPickScreen, PatchInfo, PatchSelectScreen, RepoInfo, RepoSelectScreen,
    VersionInfo, VersionSelectScreen,
};
use ratatui::{backend::TestBackend, buffer::Buffer, layout::Rect, prelude::Widget, Terminal};
use std::path::PathBuf;

fn buffer_to_text(buf: &Buffer, area: Rect) -> String {
    let mut out = String::new();
    for y in area.y..area.y + area.height {
        for x in area.x..area.x + area.width {
            let cell = &buf[(x, y)];
            if cell.skip {
                out.push(' ');
                continue;
            }
            let sym = cell.symbol();
            if sym.is_empty() {
                out.push(' ');
            } else {
                out.push_str(sym);
            }
        }
        out.push('\n');
    }
    out
}

fn render_screen(
    width: u16,
    height: u16,
    name: &str,
    render: impl FnOnce(Rect, &mut Buffer),
) -> anyhow::Result<String> {
    let backend = TestBackend::new(width, height);
    let mut terminal = Terminal::new(backend)?;
    terminal.draw(|frame| {
        let area = frame.area();
        render(area, frame.buffer_mut());
    })?;

    let buf = terminal.backend().buffer().clone();
    let size = terminal.size()?;
    let area = Rect {
        x: 0,
        y: 0,
        width: size.width,
        height: size.height,
    };
    Ok(format!(
        "=== {} ({}x{}) ===\n{}",
        name,
        area.width,
        area.height,
        buffer_to_text(&buf, area)
    ))
}

fn main() -> anyhow::Result<()> {
    let width: u16 = std::env::var("TUI_W")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(100);
    let height: u16 = std::env::var("TUI_H")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(35);

    let repo_select = RepoSelectScreen::new(vec![
        RepoInfo {
            path: PathBuf::from("~/dev/codex"),
            age: "2h ago".to_string(),
            branch: "main".to_string(),
            is_modified: false,
        },
        RepoInfo {
            path: PathBuf::from("~/dev/codex-worktrees/rust-v0.99.0-alpha.6"),
            age: "1d ago".to_string(),
            branch: "rust-v0.99.0-alpha.6".to_string(),
            is_modified: true,
        },
    ]);

    let version_select = VersionSelectScreen::new(vec![
        VersionInfo {
            tag: "rust-v0.99.0-alpha.6".to_string(),
            date: "2026-02-01".to_string(),
            is_latest: true,
            is_current: false,
            changelog: Vec::new(),
        },
        VersionInfo {
            tag: "rust-v0.98.0".to_string(),
            date: "2026-01-15".to_string(),
            is_latest: false,
            is_current: true,
            changelog: Vec::new(),
        },
    ]);

    let mut cherry_pick = CherryPickScreen::new("rust-v0.99.0-alpha.6");
    cherry_pick.set_value("abc1234, deadbeef, not-a-sha");

    let patch_select = PatchSelectScreen::new(
        vec![
            PatchInfo {
                path: PathBuf::from("patches/foo.toml"),
                name: "Foo Patch".to_string(),
                description: "Adjusts something".to_string(),
                patch_count: 3,
                selected: true,
                compatible: true,
            },
            PatchInfo {
                path: PathBuf::from("patches/bar.toml"),
                name: "Bar Patch".to_string(),
                description: "Incompatible demo".to_string(),
                patch_count: 2,
                selected: false,
                compatible: false,
            },
        ],
        "rust-v0.99.0-alpha.6".to_string(),
    );

    let build_config =
        BuildConfigScreen::new("x86-64-v3".to_string(), "Cpuid".to_string(), true, true);

    let mut out = String::new();
    out.push_str(&render_screen(width, height, "RepoSelect", |a, b| {
        (&repo_select).render(a, b)
    })?);
    out.push('\n');
    out.push_str(&render_screen(width, height, "VersionSelect", |a, b| {
        (&version_select).render(a, b)
    })?);
    out.push('\n');
    out.push_str(&render_screen(width, height, "CherryPick", |a, b| {
        (&cherry_pick).render(a, b)
    })?);
    out.push('\n');
    out.push_str(&render_screen(width, height, "PatchSelect", |a, b| {
        (&patch_select).render(a, b)
    })?);
    out.push('\n');
    out.push_str(&render_screen(width, height, "BuildConfig", |a, b| {
        (&build_config).render(a, b)
    })?);

    print!("{}", out);
    Ok(())
}
