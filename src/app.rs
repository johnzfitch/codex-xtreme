//! Application state machine for CODEX//XTREME TUI

use crate::core;
use crate::tui::screens::*;
use crate::tui::screens::build;
use crossterm::event::KeyCode;
use ratatui::{buffer::Buffer, layout::Rect, widgets::Widget};
use std::path::PathBuf;
use std::sync::mpsc;
use std::thread;

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

/// Result type for background operations
type AsyncResult<T> = Result<T, String>;

/// Build progress messages sent from background thread
pub enum BuildMessage {
    Phase(build::BuildPhase),
    Progress(f64),
    CurrentItem(String),
    Log(String),
    PatchApplied(String),
    Complete { binary_path: String, build_time: String },
    Error(String),
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
    // Async operation receivers
    clone_rx: Option<mpsc::Receiver<AsyncResult<PathBuf>>>,
    fetch_rx: Option<mpsc::Receiver<AsyncResult<()>>>,
    build_rx: Option<mpsc::Receiver<BuildMessage>>,
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
            clone_rx: None,
            fetch_rx: None,
            build_rx: None,
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

        // Handle async clone operation
        if let Screen::Cloning(ref mut screen) = self.screen {
            // Start clone if not already started
            if screen.frame() == 5 && !screen.is_complete() && !screen.is_error() && self.clone_rx.is_none() {
                let dest = PathBuf::from(screen.destination());
                screen.set_progress("Cloning repository...");

                // Spawn clone in background thread
                let (tx, rx) = mpsc::channel();
                self.clone_rx = Some(rx);

                thread::spawn(move || {
                    let result = core::clone_codex(&dest)
                        .map(|info| info.path)
                        .map_err(|e| e.to_string());
                    let _ = tx.send(result);
                });
            }

            // Poll for clone completion (non-blocking)
            if let Some(ref rx) = self.clone_rx {
                if let Ok(result) = rx.try_recv() {
                    match result {
                        Ok(path) => {
                            screen.set_complete();
                            self.selected_repo = Some(path);
                        }
                        Err(e) => {
                            screen.set_error(e);
                        }
                    }
                    self.clone_rx = None;
                }
            }
        }

        // Handle async fetch when entering version select
        if let Some(ref rx) = self.fetch_rx {
            if let Ok(result) = rx.try_recv() {
                if let Err(e) = result {
                    // Log fetch error but continue anyway
                    eprintln!("Fetch warning: {}", e);
                }
                self.fetch_rx = None;
            }
        }

        // Handle build screen - check if we need to start build
        let should_start_build = if let Screen::Build(ref screen) = self.screen {
            !screen.is_started() && self.build_rx.is_none()
        } else {
            false
        };

        if should_start_build {
            self.start_build();
        }

        // Handle build screen - poll for build messages
        if let Screen::Build(ref mut screen) = self.screen {
            // Poll for build messages (non-blocking)
            let mut done = false;
            if let Some(ref rx) = self.build_rx {
                // Process all available messages
                while let Ok(msg) = rx.try_recv() {
                    match msg {
                        BuildMessage::Phase(phase) => screen.set_phase(phase),
                        BuildMessage::Progress(p) => screen.set_progress(p),
                        BuildMessage::CurrentItem(item) => screen.set_current_item(item),
                        BuildMessage::Log(line) => screen.add_log(line),
                        BuildMessage::PatchApplied(name) => screen.add_patch(name),
                        BuildMessage::Complete { binary_path, build_time } => {
                            screen.set_complete(binary_path, build_time);
                            done = true;
                            break;
                        }
                        BuildMessage::Error(e) => {
                            screen.set_error(e);
                            done = true;
                            break;
                        }
                    }
                }
            }
            if done {
                self.build_rx = None;
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
            // Start async fetch in background (non-blocking)
            let repo = repo_path.clone();
            let (tx, rx) = mpsc::channel();
            self.fetch_rx = Some(rx);
            thread::spawn(move || {
                let result = core::fetch_repo(&repo).map_err(|e| e.to_string());
                let _ = tx.send(result);
            });

            // Load cached releases immediately (fetch will update for next time)
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

    fn start_build(&mut self) {
        // Mark build as started
        if let Screen::Build(ref mut screen) = self.screen {
            screen.mark_started();
        }

        let repo_path = match &self.selected_repo {
            Some(p) => p.clone(),
            None => {
                if let Screen::Build(ref mut screen) = self.screen {
                    screen.set_error("No repository selected".to_string());
                }
                return;
            }
        };

        let version = self.selected_version.clone();
        let patches = self.selected_patches.clone();
        let use_mold = core::has_mold();
        let cpu_target = core::detect_cpu_target();

        let (tx, rx) = mpsc::channel();
        self.build_rx = Some(rx);

        thread::spawn(move || {
            let start_time = std::time::Instant::now();

            // Phase 1: Checkout version
            if let Some(ref ver) = version {
                let _ = tx.send(BuildMessage::CurrentItem(format!("Checking out {}", ver)));
                if let Err(e) = core::checkout_version(&repo_path, ver) {
                    let _ = tx.send(BuildMessage::Error(format!("Checkout failed: {}", e)));
                    return;
                }
            }

            // Phase 2: Apply patches (simplified - would need full patch logic)
            let _ = tx.send(BuildMessage::Phase(build::BuildPhase::Patching));
            let _ = tx.send(BuildMessage::Progress(0.1));

            for (i, patch_name) in patches.iter().enumerate() {
                let _ = tx.send(BuildMessage::CurrentItem(format!("Applying {}", patch_name)));
                let _ = tx.send(BuildMessage::PatchApplied(patch_name.clone()));
                let progress = 0.1 + (0.2 * (i + 1) as f64 / patches.len().max(1) as f64);
                let _ = tx.send(BuildMessage::Progress(progress));
            }

            // Phase 3: Build (simplified - actual build would use cargo)
            let _ = tx.send(BuildMessage::Phase(build::BuildPhase::Compiling));
            let _ = tx.send(BuildMessage::Progress(0.3));
            let _ = tx.send(BuildMessage::CurrentItem(format!(
                "Building for {} with {}",
                cpu_target.display_name(),
                if use_mold { "mold" } else { "default linker" }
            )));

            // Simulate build progress (actual implementation would parse cargo output)
            let workspace = repo_path.join("codex-rs");
            let _ = tx.send(BuildMessage::Log(format!("Workspace: {}", workspace.display())));

            // For now, show that TUI build is not fully implemented
            let _ = tx.send(BuildMessage::Log("Note: TUI build runs basic cargo build".to_string()));
            let _ = tx.send(BuildMessage::Log("For full features, use CLI: codex-xtreme".to_string()));

            // Run actual cargo build
            let output = std::process::Command::new("cargo")
                .current_dir(&workspace)
                .args(["build", "--release", "-p", "codex"])
                .output();

            match output {
                Ok(out) if out.status.success() => {
                    let _ = tx.send(BuildMessage::Progress(1.0));
                    let elapsed = start_time.elapsed();
                    let binary_path = workspace.join("target/release/codex");
                    let _ = tx.send(BuildMessage::Complete {
                        binary_path: binary_path.display().to_string(),
                        build_time: format!("{:.1}s", elapsed.as_secs_f64()),
                    });
                }
                Ok(out) => {
                    let stderr = String::from_utf8_lossy(&out.stderr);
                    let _ = tx.send(BuildMessage::Error(format!("Build failed:\n{}", stderr)));
                }
                Err(e) => {
                    let _ = tx.send(BuildMessage::Error(format!("Failed to run cargo: {}", e)));
                }
            }
        });
    }
}
