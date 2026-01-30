//! Application state machine for CODEX//XTREME TUI

use crate::core;
use crate::tui::screens::BuildPhase;
use crate::tui::screens::*;
use codex_patcher::{apply_patches, load_from_path, PatchResult};
use crossterm::event::KeyCode;
use ratatui::{buffer::Buffer, layout::Rect, widgets::Widget};
use std::path::PathBuf;
use std::sync::mpsc;
use std::thread;
use std::time::Instant;

/// Current screen
pub enum Screen {
    Boot(BootScreen),
    RepoSelect(RepoSelectScreen),
    CloneInput(InputScreen),
    Cloning(CloneScreen),
    VersionSelect(VersionSelectScreen),
    PatchSelect(PatchSelectScreen),
    BuildConfig(BuildConfigScreen),
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
            Screen::BuildConfig(s) => s.tick(),
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
            Screen::BuildConfig(s) => s.render(area, buf),
            Screen::Build(s) => s.render(area, buf),
        }
    }
}

/// Build progress message from background thread
pub enum BuildMessage {
    Phase(BuildPhase),
    Progress(f64),
    CurrentItem(String),
    Log(String),
    PatchApplied(String),
    PatchSkipped(String, String), // (name, reason)
    Version(String),
    InstallPath(String),
    Complete {
        binary_path: String,
        build_time: String,
    },
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
    pub selected_patches: Vec<PathBuf>, // Now stores patch file paths
    // Background task channels
    build_rx: Option<mpsc::Receiver<BuildMessage>>,
}

impl App {
    pub fn new(dev_mode: bool) -> Self {
        let mut boot = BootScreen::new(dev_mode);

        // Real system checks
        let cpu = core::detect_cpu_target();
        boot.add_check_with_detail("CPU Target", cpu.display_name());
        boot.add_check_with_detail("Rust compiler", format!("rustc {}", core::rust_version()));
        boot.add_check_with_detail(
            "mold linker",
            if core::has_mold() {
                "found"
            } else {
                "not found"
            }
            .to_string(),
        );
        boot.add_check_with_detail(
            "BOLT optimizer",
            if core::has_bolt() {
                "found"
            } else {
                "not found"
            }
            .to_string(),
        );

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
            build_rx: None,
        }
    }

    pub fn tick(&mut self) {
        self.screen.tick();

        // Auto-advance from boot after countdown
        if let Screen::Boot(ref boot) = self.screen {
            if boot.should_auto_advance() {
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

            // Auto-advance after clone completes
            if screen.should_auto_advance() {
                let dest = screen.destination().to_string();
                self.selected_repo = Some(PathBuf::from(&dest));
                self.transition_to_version_select();
            }
        }

        // Handle build progress from background thread
        if let Some(rx) = self.build_rx.take() {
            // Collect all available messages first
            let mut messages = Vec::new();
            while let Ok(msg) = rx.try_recv() {
                messages.push(msg);
            }

            // Check if we're done
            let mut done = false;
            for msg in &messages {
                if matches!(msg, BuildMessage::Complete { .. } | BuildMessage::Error(_)) {
                    done = true;
                    break;
                }
            }

            // Process messages
            if let Screen::Build(ref mut screen) = self.screen {
                for msg in messages {
                    match msg {
                        BuildMessage::Phase(phase) => screen.set_phase(phase),
                        BuildMessage::Progress(p) => screen.set_progress(p),
                        BuildMessage::CurrentItem(item) => screen.set_current_item(item),
                        BuildMessage::Log(line) => screen.add_log(line),
                        BuildMessage::PatchApplied(name) => screen.add_patch(name),
                        BuildMessage::PatchSkipped(name, reason) => {
                            screen.add_skipped_patch(name, reason)
                        }
                        BuildMessage::Version(v) => screen.set_version(v),
                        BuildMessage::InstallPath(p) => screen.set_install_path(p),
                        BuildMessage::Complete {
                            binary_path,
                            build_time,
                        } => {
                            screen.set_complete(binary_path, build_time);
                        }
                        BuildMessage::Error(err) => {
                            screen.set_error(err);
                        }
                    }
                }
            }

            // Put receiver back if not done
            if !done {
                self.build_rx = Some(rx);
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
                self.transition_to_version_select();
            }
            Screen::BuildConfig(_) => {
                self.transition_to_patch_select();
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
                    // Collect patch names first to avoid borrow issues
                    let patch_names: Vec<String> = screen
                        .selected_patches()
                        .iter()
                        .map(|p| p.name.clone())
                        .collect();

                    // Now resolve paths (no longer borrowing screen)
                    self.selected_patches = patch_names
                        .iter()
                        .filter_map(|name| self.resolve_patch_path(name))
                        .collect();
                    self.transition_to_build_config();
                }
                _ => {}
            },

            Screen::BuildConfig(screen) => match key {
                KeyCode::Up => screen.select_prev(),
                KeyCode::Down => screen.select_next(),
                KeyCode::Char(' ') => screen.toggle_current(),
                KeyCode::Enter => {
                    self.start_build();
                }
                _ => {}
            },

            Screen::Build(screen) => match key {
                KeyCode::Char('r') | KeyCode::Char('R') if screen.is_error() => {
                    // Retry build
                    self.start_build();
                }
                _ if screen.is_complete() || screen.is_error() => {
                    self.should_quit = true;
                }
                _ => {}
            },
        }
    }

    /// Resolve patch name to full path
    fn resolve_patch_path(&self, name: &str) -> Option<PathBuf> {
        let patches_dir = core::find_patches_dir().ok()?;
        let path = patches_dir.join(format!("{}.toml", name));
        if path.exists() {
            Some(path)
        } else {
            // Try without adding .toml
            let path = patches_dir.join(name);
            if path.exists() {
                Some(path)
            } else {
                None
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
        let expanded = if let Some(stripped) = destination.strip_prefix("~/") {
            if let Some(home) = dirs::home_dir() {
                home.join(stripped).to_string_lossy().to_string()
            } else {
                destination.clone()
            }
        } else {
            destination.clone()
        };

        let mut screen = CloneScreen::new(&expanded);
        screen.set_progress("Starting git clone...");

        self.screen = Screen::Cloning(screen);
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
                is_modified: false,
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
                        changelog: Vec::new(),
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
                    .file_stem()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_else(|| config.meta.name.clone());

                PatchInfo {
                    name,
                    description: config
                        .meta
                        .description
                        .unwrap_or_else(|| config.meta.name.clone()),
                    selected: true, // Auto-select all patches
                    compatible: true,
                }
            })
            .collect();

        self.screen = Screen::PatchSelect(PatchSelectScreen::new(patches, version));
    }

    fn transition_to_build_config(&mut self) {
        let cpu = core::detect_cpu_target();
        let has_mold = core::has_mold();
        let has_bolt = core::has_bolt();

        self.screen = Screen::BuildConfig(BuildConfigScreen::new(
            cpu.display_name(),
            format!("{:?}", cpu.detected_by),
            has_mold,
            has_bolt,
        ));
    }

    fn start_build(&mut self) {
        let mut build = BuildScreen::new();

        // Add patch names to display
        for patch_path in &self.selected_patches {
            if let Some(name) = patch_path.file_stem() {
                build.add_patch(name.to_string_lossy().to_string());
            }
        }

        self.screen = Screen::Build(build);

        // Get build parameters
        let repo_path = match &self.selected_repo {
            Some(p) => p.clone(),
            None => {
                if let Screen::Build(ref mut s) = self.screen {
                    s.set_error("No repository selected".to_string());
                }
                return;
            }
        };

        let version = match &self.selected_version {
            Some(v) => v.clone(),
            None => {
                if let Screen::Build(ref mut s) = self.screen {
                    s.set_error("No version selected".to_string());
                }
                return;
            }
        };

        let patches = self.selected_patches.clone();
        let workspace = repo_path.join(core::CODEX_RS_SUBDIR);

        // Create channel for progress updates
        let (tx, rx) = mpsc::channel();
        self.build_rx = Some(rx);

        // Spawn background build thread
        thread::spawn(move || {
            run_build(tx, repo_path, workspace, version, patches);
        });
    }
}

/// Background build process
fn run_build(
    tx: mpsc::Sender<BuildMessage>,
    repo_path: PathBuf,
    workspace: PathBuf,
    version: String,
    patches: Vec<PathBuf>,
) {
    let start_time = Instant::now();

    // Send helper
    let send = |msg: BuildMessage| {
        let _ = tx.send(msg);
    };

    // Calculate install path
    let install_path = dirs::home_dir()
        .map(|h| h.join(".cargo/bin/codex"))
        .unwrap_or_else(|| PathBuf::from("~/.cargo/bin/codex"));

    // Send version and install path info
    send(BuildMessage::Version(version.clone()));
    send(BuildMessage::InstallPath(install_path.to_string_lossy().to_string()));

    // Get changelog/release notes (git log between current HEAD and target version)
    if let Ok(output) = std::process::Command::new("git")
        .current_dir(&repo_path)
        .args(["log", "--oneline", "-10", &format!("{}..HEAD", version)])
        .output()
    {
        let log_output = String::from_utf8_lossy(&output.stdout);
        for line in log_output.lines().take(5) {
            if !line.trim().is_empty() {
                send(BuildMessage::Log(format!("  {}", line)));
            }
        }
    }

    // Phase 1: Checkout version
    send(BuildMessage::Phase(BuildPhase::Patching));
    send(BuildMessage::CurrentItem(format!(
        "Checking out {}",
        version
    )));
    send(BuildMessage::Log(format!("git checkout {}", version)));

    if let Err(e) = core::checkout_version(&repo_path, &version) {
        send(BuildMessage::Error(format!("Checkout failed: {}", e)));
        return;
    }
    send(BuildMessage::Progress(0.02));
    send(BuildMessage::Log("Checkout complete".to_string()));

    // Phase 2: Apply patches
    if !patches.is_empty() {
        send(BuildMessage::CurrentItem("Applying patches...".to_string()));

        // Read workspace version for patch compatibility
        let workspace_version =
            read_workspace_version(&workspace).unwrap_or_else(|_| "0.0.0".to_string());

        let total_patches = patches.len();
        for (i, patch_path) in patches.iter().enumerate() {
            let patch_name = patch_path
                .file_stem()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_else(|| "unknown".to_string());

            send(BuildMessage::CurrentItem(format!(
                "Applying {}...",
                patch_name
            )));
            send(BuildMessage::Log(format!(
                "Loading {}",
                patch_path.display()
            )));

            match load_from_path(patch_path) {
                Ok(config) => {
                    let results = apply_patches(&config, &workspace, &workspace_version);

                    let mut applied_count = 0;
                    let mut skipped_count = 0;
                    let mut skip_reason = String::new();

                    for (patch_id, result) in results {
                        match result {
                            Ok(PatchResult::Applied { file }) => {
                                send(BuildMessage::Log(format!(
                                    "  ✓ {} → {}",
                                    patch_id,
                                    file.display()
                                )));
                                applied_count += 1;
                            }
                            Ok(PatchResult::AlreadyApplied { .. }) => {
                                send(BuildMessage::Log(format!(
                                    "  ○ {} (already applied)",
                                    patch_id
                                )));
                                // Already applied counts as success
                                applied_count += 1;
                            }
                            Ok(PatchResult::SkippedVersion { reason }) => {
                                send(BuildMessage::Log(format!("  ⊘ {} ({})", patch_id, reason)));
                                skipped_count += 1;
                                if skip_reason.is_empty() {
                                    skip_reason = reason;
                                }
                            }
                            Ok(PatchResult::Failed { reason, .. }) => {
                                send(BuildMessage::Log(format!("  ✗ {} ({})", patch_id, reason)));
                                skipped_count += 1;
                                if skip_reason.is_empty() {
                                    skip_reason = reason;
                                }
                            }
                            Err(e) => {
                                send(BuildMessage::Log(format!("  ✗ {} ({})", patch_id, e)));
                                skipped_count += 1;
                                if skip_reason.is_empty() {
                                    skip_reason = e.to_string();
                                }
                            }
                        }
                    }

                    if applied_count > 0 {
                        send(BuildMessage::PatchApplied(patch_name.clone()));
                    }
                    if skipped_count > 0 && applied_count == 0 {
                        // Only show as skipped if ALL patches in this file were skipped
                        send(BuildMessage::PatchSkipped(patch_name, skip_reason));
                    }
                }
                Err(e) => {
                    send(BuildMessage::Log(format!("  ✗ Failed to load: {}", e)));
                    send(BuildMessage::PatchSkipped(
                        patch_name,
                        format!("load error: {}", e),
                    ));
                }
            }

            // Patching is 2-5% of total progress
            let progress = 0.02 + (0.03 * (i + 1) as f64 / total_patches as f64);
            send(BuildMessage::Progress(progress));
        }
    }

    // Phase 3: Compile
    send(BuildMessage::Phase(BuildPhase::Compiling));
    send(BuildMessage::Progress(0.05));
    send(BuildMessage::CurrentItem(
        "Building codex-cli...".to_string(),
    ));

    // Use xtreme profile (thin LTO) if available, otherwise release
    let profile = "xtreme";
    send(BuildMessage::Log(format!(
        "cargo build --profile {} -p codex-cli",
        profile
    )));

    // Run cargo build
    let mut cmd = std::process::Command::new("cargo");
    cmd.current_dir(&workspace)
        .args(["build", "--profile", profile, "-p", "codex-cli"])
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());

    match cmd.spawn() {
        Ok(mut child) => {
            // Read stderr for progress
            if let Some(stderr) = child.stderr.take() {
                use std::io::{BufRead, BufReader};
                let reader = BufReader::new(stderr);
                let mut compile_count = 0;
                // Codex has ~350 crates to compile
                let estimated_total_crates = 350.0;

                for line in reader.lines().map_while(Result::ok) {
                    // Parse cargo output for progress
                    if line.contains("Compiling") {
                        compile_count += 1;

                        // Use ease-out curve: progress slows as we approach end
                        // Linear progress from crate count
                        let linear = (compile_count as f64 / estimated_total_crates).min(1.0);
                        // Apply ease-out: fast start, slow finish (matches real build times)
                        // Using cubic ease-out: 1 - (1-x)^3
                        let eased = 1.0 - (1.0 - linear).powi(3);
                        // Map to 5-98% range
                        let progress = 0.05 + (0.93 * eased);
                        send(BuildMessage::Progress(progress));

                        // Extract crate name
                        if let Some(crate_name) = line.split_whitespace().nth(1) {
                            send(BuildMessage::CurrentItem(format!(
                                "Compiling {} ({}/{})...",
                                crate_name,
                                compile_count,
                                estimated_total_crates as i32
                            )));
                        }
                    } else if line.contains("error") || line.contains("Error") {
                        send(BuildMessage::Log(line));
                    }
                }
            }

            match child.wait() {
                Ok(status) if status.success() => {
                    send(BuildMessage::Progress(0.98));
                }
                Ok(status) => {
                    send(BuildMessage::Error(format!(
                        "Build failed with exit code: {:?}",
                        status.code()
                    )));
                    return;
                }
                Err(e) => {
                    send(BuildMessage::Error(format!("Build process error: {}", e)));
                    return;
                }
            }
        }
        Err(e) => {
            send(BuildMessage::Error(format!("Failed to start cargo: {}", e)));
            return;
        }
    }

    // Find the built binary (profile xtreme outputs to target/xtreme/)
    let binary_path = workspace.join(format!("target/{}/codex", profile));

    // Phase 4: Verify
    send(BuildMessage::Phase(BuildPhase::Installing)); // Reuse as "Verifying"
    send(BuildMessage::Progress(0.95));
    send(BuildMessage::CurrentItem("Verifying build...".to_string()));
    send(BuildMessage::Log("Running codex --version".to_string()));

    // Quick verification - just check the binary runs
    if binary_path.exists() {
        match std::process::Command::new(&binary_path)
            .arg("--version")
            .output()
        {
            Ok(output) if output.status.success() => {
                let version = String::from_utf8_lossy(&output.stdout);
                send(BuildMessage::Log(format!("  ✓ {}", version.trim())));
            }
            Ok(_) => {
                send(BuildMessage::Log("  ⚠ Binary runs but --version failed".to_string()));
            }
            Err(e) => {
                send(BuildMessage::Log(format!("  ✗ Failed to run binary: {}", e)));
            }
        }
    } else {
        send(BuildMessage::Log(format!(
            "  ✗ Binary not found at {}",
            binary_path.display()
        )));
    }

    // Phase 5: Install to PATH
    send(BuildMessage::Progress(0.98));
    send(BuildMessage::CurrentItem("Installing to PATH...".to_string()));

    #[cfg(unix)]
    {
        // Use ~/.local/bin on Unix (Linux/macOS)
        let local_bin = dirs::home_dir()
            .map(|h| h.join(".local/bin"))
            .unwrap_or_else(|| std::path::PathBuf::from("/usr/local/bin"));

        // Create ~/.local/bin if it doesn't exist
        if !local_bin.exists() {
            let _ = std::fs::create_dir_all(&local_bin);
            send(BuildMessage::Log(format!(
                "  Created {}",
                local_bin.display()
            )));
        }

        let symlink_path = local_bin.join("codex");

        // Remove old symlink/file if exists
        if symlink_path.exists() || symlink_path.is_symlink() {
            let _ = std::fs::remove_file(&symlink_path);
        }

        // Create symlink
        match std::os::unix::fs::symlink(&binary_path, &symlink_path) {
            Ok(_) => {
                send(BuildMessage::Log(format!(
                    "  ✓ Linked {} → codex",
                    local_bin.display()
                )));

                // Check if ~/.local/bin is in PATH
                let path_var = std::env::var("PATH").unwrap_or_default();
                let local_bin_str = local_bin.to_string_lossy();
                if !path_var.contains(local_bin_str.as_ref()) {
                    send(BuildMessage::Log(format!(
                        "  ⚠ {} not in PATH - add to your shell rc:",
                        local_bin.display()
                    )));
                    send(BuildMessage::Log(
                        "    export PATH=\"$HOME/.local/bin:$PATH\"".to_string(),
                    ));
                }
            }
            Err(e) => {
                send(BuildMessage::Log(format!(
                    "  ✗ Symlink failed: {}",
                    e
                )));
                send(BuildMessage::Log(format!(
                    "    Run: ln -sf {} {}",
                    binary_path.display(),
                    symlink_path.display()
                )));
            }
        }
    }

    #[cfg(windows)]
    {
        // On Windows, copy binary to %LOCALAPPDATA%\Programs\codex-xtreme
        let install_dir = dirs::data_local_dir()
            .map(|d| d.join("Programs").join("codex-xtreme"))
            .unwrap_or_else(|| std::path::PathBuf::from("C:\\codex-xtreme"));

        if !install_dir.exists() {
            let _ = std::fs::create_dir_all(&install_dir);
        }

        let dest_path = install_dir.join("codex.exe");

        match std::fs::copy(&binary_path, &dest_path) {
            Ok(_) => {
                send(BuildMessage::Log(format!(
                    "  ✓ Copied to {}",
                    dest_path.display()
                )));

                // Check if already in PATH
                let path_var = std::env::var("PATH").unwrap_or_default();
                let install_dir_str = install_dir.to_string_lossy();

                if path_var.contains(install_dir_str.as_ref()) {
                    send(BuildMessage::Log("  ✓ Already in PATH".to_string()));
                } else {
                    // Try setx automatically
                    send(BuildMessage::Log("  Adding to PATH...".to_string()));

                    let setx_result = std::process::Command::new("setx")
                        .args(["PATH", &format!("{};{}", path_var, install_dir.display())])
                        .output();

                    match setx_result {
                        Ok(output) if output.status.success() => {
                            send(BuildMessage::Log(
                                "  ✓ Added to PATH (restart terminal to use)".to_string()
                            ));
                        }
                        _ => {
                            // setx failed - show manual options
                            send(BuildMessage::Log(
                                "  ⚠ Auto-add failed. Manual options:".to_string()
                            ));
                            send(BuildMessage::Log(String::new()));
                            send(BuildMessage::Log(
                                "  [PowerShell] Paste this command:".to_string()
                            ));
                            send(BuildMessage::Log(format!(
                                "    [Environment]::SetEnvironmentVariable(\"Path\", $env:Path + \";{}\", \"User\")",
                                install_dir.display()
                            )));
                            send(BuildMessage::Log(String::new()));
                            send(BuildMessage::Log(
                                "  [Settings] Windows Settings → System → About →".to_string()
                            ));
                            send(BuildMessage::Log(
                                "    Advanced system settings → Environment Variables".to_string()
                            ));
                        }
                    }
                }
            }
            Err(e) => {
                send(BuildMessage::Log(format!(
                    "  ✗ Copy failed: {}",
                    e
                )));
            }
        }
    }

    let elapsed = start_time.elapsed();
    let build_time = format!("{:.1}s", elapsed.as_secs_f64());

    send(BuildMessage::Phase(BuildPhase::Complete));
    send(BuildMessage::Progress(1.0));
    send(BuildMessage::Complete {
        binary_path: binary_path.to_string_lossy().to_string(),
        build_time,
    });
}

/// Read workspace version from Cargo.toml
fn read_workspace_version(workspace: &std::path::Path) -> anyhow::Result<String> {
    let cargo_toml = workspace.join("Cargo.toml");
    let contents = std::fs::read_to_string(&cargo_toml)?;

    // Try to parse version from workspace package
    for line in contents.lines() {
        if line.trim().starts_with("version") && line.contains('=') {
            if let Some(version) = line.split('=').nth(1) {
                let version = version.trim().trim_matches('"').trim_matches('\'');
                if !version.is_empty() {
                    return Ok(version.to_string());
                }
            }
        }
    }

    Ok("0.0.0".to_string())
}
