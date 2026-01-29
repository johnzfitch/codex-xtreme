//! Application state machine for CODEX//XTREME TUI

use crate::core;
use crate::tui::screens::*;
use crossterm::event::KeyCode;
use ratatui::{buffer::Buffer, layout::Rect, widgets::Widget};
use std::path::PathBuf;

/// Current screen
pub enum Screen {
    Boot(BootScreen),
    RepoSelect(RepoSelectScreen),
    CloneInput(InputScreen),
    Cloning(CloneScreen),
    VersionSelect(VersionSelectScreen),
    PatchSelect(PatchSelectScreen),
    Build(BuildScreen),
}

impl Screen {
    pub fn tick(&mut self) {
        match self {
            Screen::Boot(s) => s.tick(),
            Screen::RepoSelect(s) => s.tick(),
            Screen::CloneInput(s) => s.tick(),
            Screen::Cloning(s) => s.tick(),
            Screen::VersionSelect(s) => s.tick(),
            Screen::PatchSelect(s) => s.tick(),
            Screen::Build(s) => s.tick(),
        }
    }
}

impl Widget for &Screen {
    fn render(self, area: Rect, buf: &mut Buffer) {
        match self {
            Screen::Boot(s) => s.render(area, buf),
            Screen::RepoSelect(s) => s.render(area, buf),
            Screen::CloneInput(s) => s.render(area, buf),
            Screen::Cloning(s) => s.render(area, buf),
            Screen::VersionSelect(s) => s.render(area, buf),
            Screen::PatchSelect(s) => s.render(area, buf),
            Screen::Build(s) => s.render(area, buf),
        }
    }
}

/// Application state
pub struct App {
    pub screen: Screen,
    pub should_quit: bool,
    pub dev_mode: bool,
    // Collected data
    pub selected_repo: Option<PathBuf>,
    pub selected_version: Option<String>,
    pub selected_patches: Vec<String>,
}

impl App {
    pub fn new(dev_mode: bool) -> Self {
        let mut boot = BootScreen::new(dev_mode);

        // Real system checks
        let cpu = core::detect_cpu_target();
        boot.add_check_with_detail("CPU Target", cpu.display_name());
        boot.add_check_with_detail("Rust compiler", format!("rustc {}", core::rust_version()));
        boot.add_check_with_detail("mold linker", if core::has_mold() { "found" } else { "not found" }.to_string());
        boot.add_check_with_detail("BOLT optimizer", if core::has_bolt() { "found" } else { "not found" }.to_string());

        // Check patches
        let patches_status = match core::find_patches_dir() {
            Ok(dir) => format!("{}", dir.display()),
            Err(_) => "not found".to_string(),
        };
        boot.add_check_with_detail("Patch definitions", patches_status);

        // Check repos
        let repos = core::find_codex_repos().unwrap_or_default();
        boot.add_check_with_detail("Codex repositories", format!("{} found", repos.len()));

        Self {
            screen: Screen::Boot(boot),
            should_quit: false,
            dev_mode,
            selected_repo: None,
            selected_version: None,
            selected_patches: Vec::new(),
        }
    }

    pub fn tick(&mut self) {
        self.screen.tick();

        // Auto-advance from boot when complete
        if let Screen::Boot(ref boot) = self.screen {
            if boot.is_complete() {
                self.transition_to_repo_select();
            }
        }

        // Handle clone progress
        if let Screen::Cloning(ref mut screen) = self.screen {
            if screen.frame() == 5 && !screen.is_complete() && !screen.is_error() {
                let dest = PathBuf::from(screen.destination());
                screen.set_progress("Cloning repository...");

                // Use core::clone_codex for real cloning
                match core::clone_codex(&dest) {
                    Ok(_) => {
                        screen.set_complete();
                    }
                    Err(e) => {
                        screen.set_error(format!("{}", e));
                    }
                }
            }
        }
    }

    pub fn handle_key(&mut self, key: KeyCode) {
        match key {
            KeyCode::Char('q') | KeyCode::Char('Q') => {
                self.should_quit = true;
            }
            KeyCode::Esc => {
                self.handle_back();
            }
            _ => {
                self.handle_screen_key(key);
            }
        }
    }

    fn handle_back(&mut self) {
        match &self.screen {
            Screen::Boot(_) | Screen::RepoSelect(_) => {}
            Screen::CloneInput(_) => self.transition_to_repo_select(),
            Screen::Cloning(s) if s.is_error() => self.transition_to_repo_select(),
            Screen::Cloning(_) => {}
            Screen::VersionSelect(_) => self.transition_to_repo_select(),
            Screen::PatchSelect(_) => {
                // Would go back to version select
            }
            Screen::Build(s) if s.is_complete() || s.is_error() => {
                self.should_quit = true;
            }
            Screen::Build(_) => {}
        }
    }

    fn handle_screen_key(&mut self, key: KeyCode) {
        match &mut self.screen {
            Screen::Boot(boot) => {
                if matches!(key, KeyCode::Enter | KeyCode::Char(' ')) {
                    boot.complete();
                }
            }

            Screen::RepoSelect(screen) => match key {
                KeyCode::Up => screen.select_prev(),
                KeyCode::Down => screen.select_next(),
                KeyCode::Enter => {
                    if screen.is_clone_selected() {
                        self.transition_to_clone_input();
                    } else if let Some(repo) = screen.selected_repo() {
                        self.selected_repo = Some(repo.path.clone());
                        self.transition_to_version_select();
                    }
                }
                _ => {}
            },

            Screen::CloneInput(screen) => match key {
                KeyCode::Char(c) => screen.insert_char(c),
                KeyCode::Backspace => screen.delete_char(),
                KeyCode::Delete => screen.delete_forward(),
                KeyCode::Left => screen.move_left(),
                KeyCode::Right => screen.move_right(),
                KeyCode::Home => screen.move_home(),
                KeyCode::End => screen.move_end(),
                KeyCode::Enter => {
                    let dest = screen.value().to_string();
                    if !dest.is_empty() {
                        self.start_clone(dest);
                    }
                }
                _ => {}
            },

            Screen::Cloning(screen) => match key {
                KeyCode::Enter if screen.is_complete() => {
                    // Use the cloned repo
                    let dest = screen.destination().to_string();
                    self.selected_repo = Some(PathBuf::from(&dest));
                    self.transition_to_version_select();
                }
                KeyCode::Char('r') | KeyCode::Char('R') if screen.is_error() => {
                    let dest = screen.destination().to_string();
                    self.start_clone(dest);
                }
                _ => {}
            },

            Screen::VersionSelect(screen) => match key {
                KeyCode::Up => screen.select_prev(),
                KeyCode::Down => screen.select_next(),
                KeyCode::Enter => {
                    if let Some(ver) = screen.selected_version() {
                        self.selected_version = Some(ver.tag.clone());
                        self.transition_to_patch_select();
                    }
                }
                _ => {}
            },

            Screen::PatchSelect(screen) => match key {
                KeyCode::Up => screen.select_prev(),
                KeyCode::Down => screen.select_next(),
                KeyCode::Char(' ') => screen.toggle_current(),
                KeyCode::Char('a') | KeyCode::Char('A') => screen.select_all(),
                KeyCode::Char('n') | KeyCode::Char('N') => screen.select_none(),
                KeyCode::Enter => {
                    self.selected_patches = screen
                        .selected_patches()
                        .iter()
                        .map(|p| p.name.clone())
                        .collect();
                    self.transition_to_build();
                }
                _ => {}
            },

            Screen::Build(screen) => {
                if screen.is_complete() || screen.is_error() {
                    self.should_quit = true;
                }
            }
        }
    }

    // Transitions

    fn transition_to_clone_input(&mut self) {
        // Get default path
        let default_path = dirs::home_dir()
            .map(|h| h.join("dev/codex"))
            .unwrap_or_else(|| PathBuf::from("~/dev/codex"));

        let screen = InputScreen::new("Clone destination")
            .placeholder("Enter path (e.g., ~/dev/codex)")
            .initial_value(default_path.to_string_lossy().to_string());

        self.screen = Screen::CloneInput(screen);
    }

    fn start_clone(&mut self, destination: String) {
        // Expand ~ to home directory
        let expanded = if destination.starts_with("~/") {
            if let Some(home) = dirs::home_dir() {
                home.join(&destination[2..]).to_string_lossy().to_string()
            } else {
                destination.clone()
            }
        } else {
            destination.clone()
        };

        let mut screen = CloneScreen::new(&expanded);
        screen.set_progress("Starting git clone...");

        self.screen = Screen::Cloning(screen);

        // Note: In a real implementation, we'd spawn an async task to run git clone
        // and update the screen's progress. For now, we'll do it synchronously
        // on the next tick. See tick() method.
    }

    fn transition_to_repo_select(&mut self) {
        // Use real repo detection from core
        let core_repos = core::find_codex_repos().unwrap_or_default();

        let repos: Vec<RepoInfo> = core_repos
            .into_iter()
            .map(|r| RepoInfo {
                path: r.path,
                branch: r.branch,
                age: r.age,
                is_modified: false, // Could check git status
            })
            .collect();

        self.screen = Screen::RepoSelect(RepoSelectScreen::new(repos));
    }

    fn transition_to_version_select(&mut self) {
        // Fetch real releases from the repo
        if let Some(ref repo_path) = self.selected_repo {
            // Fetch tags first
            let _ = core::fetch_repo(repo_path);

            let current = core::get_current_version(repo_path);
            let releases = core::get_releases(repo_path).unwrap_or_default();

            let versions: Vec<VersionInfo> = releases
                .into_iter()
                .enumerate()
                .map(|(i, r)| {
                    let is_current = current.as_ref() == Some(&r.version);
                    VersionInfo {
                        tag: r.tag,
                        date: r.published,
                        is_latest: i == 0,
                        is_current,
                        changelog: Vec::new(), // Could fetch from GitHub API
                    }
                })
                .collect();

            self.screen = Screen::VersionSelect(VersionSelectScreen::new(versions));
        }
    }

    fn transition_to_patch_select(&mut self) {
        let version = self.selected_version.clone().unwrap_or_default();

        // Load real patches from codex-patcher
        let available = core::get_available_patches().unwrap_or_default();

        let patches: Vec<PatchInfo> = available
            .into_iter()
            .map(|(path, config)| {
                let name = path
                    .file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_else(|| config.meta.name.clone());

                // Auto-select privacy and common patches
                let name_lower = name.to_lowercase();
                let auto_select = name_lower.contains("privacy")
                    || name_lower.contains("subagent")
                    || name_lower.contains("undo");

                PatchInfo {
                    name,
                    description: config.meta.description.unwrap_or_else(|| config.meta.name),
                    selected: auto_select,
                    compatible: true, // Could check version_range
                }
            })
            .collect();

        self.screen = Screen::PatchSelect(PatchSelectScreen::new(patches, version));
    }

    fn transition_to_build(&mut self) {
        let mut build = BuildScreen::new();
        for patch in &self.selected_patches {
            build.add_patch(patch);
        }
        self.screen = Screen::Build(build);
    }
}
