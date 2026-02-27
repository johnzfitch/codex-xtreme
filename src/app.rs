//! Application state machine for CODEX//XTREME TUI

use crate::core;
use crate::tui::screens::BuildPhase;
use crate::tui::screens::*;
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
    CherryPick(CherryPickScreen),
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
            Screen::CherryPick(s) => s.tick(),
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
            Screen::CherryPick(s) => s.render(area, buf),
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
    pub cargo_jobs: Option<usize>,
    // Collected data
    pub selected_repo: Option<PathBuf>,
    pub selected_version: Option<String>,
    pub cherry_pick_shas: Vec<String>,
    pub selected_patches: Vec<PathBuf>, // Now stores patch file paths
    pub build_options: Option<crate::workflow::BuildOptions>,
    pub run_tests: bool,
    pub setup_alias: bool,
    // Background task channels
    build_rx: Option<mpsc::Receiver<BuildMessage>>,
}

impl App {
    pub fn new(dev_mode: bool, cargo_jobs: Option<usize>) -> Self {
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
            cargo_jobs,
            selected_repo: None,
            selected_version: None,
            cherry_pick_shas: Vec::new(),
            selected_patches: Vec::new(),
            build_options: None,
            run_tests: true,
            setup_alias: true,
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
            Screen::CherryPick(_) => self.transition_to_version_select(),
            Screen::PatchSelect(_) => {
                if self.dev_mode {
                    self.transition_to_cherry_pick();
                } else {
                    self.transition_to_version_select();
                }
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
                        self.cherry_pick_shas.clear();
                        // Dev-mode cherry-pick happens on a clean checkout, before patching.
                        if self.dev_mode {
                            self.transition_to_cherry_pick();
                        } else {
                            self.transition_to_patch_select();
                        }
                    }
                }
                _ => {}
            },

            Screen::CherryPick(screen) => match key {
                KeyCode::Char(c) => screen.insert_char(c),
                KeyCode::Backspace => screen.delete_char(),
                KeyCode::Delete => screen.delete_forward(),
                KeyCode::Left => screen.move_left(),
                KeyCode::Right => screen.move_right(),
                KeyCode::Home => screen.move_home(),
                KeyCode::End => screen.move_end(),
                KeyCode::Enter => {
                    let input = screen.value().to_string();
                    let mut shas = Vec::new();
                    let mut invalid = Vec::new();

                    for part in input.split(',') {
                        let s = part.trim();
                        if s.is_empty() {
                            continue;
                        }
                        let valid = s.len() >= 7
                            && s.len() <= 40
                            && s.chars().all(|c| c.is_ascii_hexdigit());
                        if valid {
                            shas.push(s.to_string());
                        } else {
                            invalid.push(s.to_string());
                        }
                    }

                    if !invalid.is_empty() && screen.status().is_none() {
                        screen.set_status(Some(format!(
                            "Ignored invalid SHA(s): {}",
                            invalid
                                .iter()
                                .map(|s| s.as_str())
                                .collect::<Vec<_>>()
                                .join(", ")
                        )));
                        return;
                    }

                    self.cherry_pick_shas = shas;
                    self.transition_to_patch_select();
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
                    self.selected_patches =
                        screen.selected_patch_paths().into_iter().cloned().collect();
                    self.transition_to_build_config();
                }
                _ => {}
            },

            Screen::BuildConfig(screen) => match key {
                KeyCode::Up => screen.select_prev(),
                KeyCode::Down => screen.select_next(),
                KeyCode::Char(' ') => screen.toggle_current(),
                KeyCode::Enter => {
                    let cpu = core::detect_cpu_target();
                    let cpu_target = if screen.optimize_cpu() {
                        Some(cpu.rustc_target_cpu().to_string())
                    } else {
                        None
                    };

                    let profile = if screen.use_xtreme_profile() {
                        "xtreme".to_string()
                    } else {
                        "release".to_string()
                    };

                    self.build_options = Some(crate::workflow::BuildOptions {
                        profile,
                        cpu_target,
                        optimization: screen.optimization_flags(),
                        strip_symbols: screen.strip_symbols(),
                        cargo_jobs: self.cargo_jobs,
                    });
                    self.run_tests = screen.run_tests();
                    self.setup_alias = screen.setup_alias();
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
                let compatible =
                    core::is_patch_compatible(config.meta.version_range.as_deref(), &version);

                PatchInfo {
                    path,
                    name: config.meta.name.clone(),
                    description: config
                        .meta
                        .description
                        .unwrap_or_else(|| config.meta.name.clone()),
                    patch_count: config.patches.len(),
                    selected: compatible,
                    compatible,
                }
            })
            .collect();

        self.screen = Screen::PatchSelect(PatchSelectScreen::new(patches, version));
    }

    fn transition_to_cherry_pick(&mut self) {
        let version = self.selected_version.clone().unwrap_or_default();
        let mut screen = CherryPickScreen::new(version);
        if !self.cherry_pick_shas.is_empty() {
            screen.set_value(self.cherry_pick_shas.join(", "));
        }
        self.screen = Screen::CherryPick(screen);
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
        let cherry_pick_shas = self.cherry_pick_shas.clone();
        let workspace = repo_path.join(core::CODEX_RS_SUBDIR);
        let build_options = match &self.build_options {
            Some(o) => o.clone(),
            None => crate::workflow::BuildOptions {
                profile: "xtreme".to_string(),
                cpu_target: Some(core::detect_cpu_target().rustc_target_cpu().to_string()),
                optimization: crate::workflow::OptimizationFlags {
                    use_mold: false,
                    use_bolt: core::has_bolt(),
                },
                strip_symbols: true,
                cargo_jobs: self.cargo_jobs,
            },
        };
        let run_tests = self.run_tests;
        let setup_alias = self.setup_alias;
        let params = RunBuildParams {
            repo_path,
            workspace,
            version,
            cherry_pick_shas,
            patches,
            build_options,
            run_tests,
            setup_alias,
        };

        // Create channel for progress updates
        let (tx, rx) = mpsc::channel();
        self.build_rx = Some(rx);

        // Spawn background build thread
        thread::spawn(move || {
            run_build(tx, params);
        });
    }
}

struct RunBuildParams {
    repo_path: PathBuf,
    workspace: PathBuf,
    version: String,
    cherry_pick_shas: Vec<String>,
    patches: Vec<PathBuf>,
    build_options: crate::workflow::BuildOptions,
    run_tests: bool,
    setup_alias: bool,
}

/// Background build process
fn run_build(tx: mpsc::Sender<BuildMessage>, params: RunBuildParams) {
    let RunBuildParams {
        repo_path,
        workspace,
        version,
        cherry_pick_shas,
        patches,
        build_options,
        run_tests,
        setup_alias,
    } = params;

    let start_time = Instant::now();

    // Send helper
    let send = |msg: BuildMessage| {
        let _ = tx.send(msg);
    };

    // CLI-equivalent workflow:
    // checkout -> optional cherry-pick -> apply patches -> build -> optional BOLT -> optional strip
    // -> optional tests -> optional alias setup.
    send(BuildMessage::Version(version.clone()));
    send(BuildMessage::InstallPath("shell alias".to_string()));

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

    // Optional: cherry-pick commits (dev mode)
    if !cherry_pick_shas.is_empty() {
        send(BuildMessage::CurrentItem(format!(
            "Cherry-picking {} commits...",
            cherry_pick_shas.len()
        )));
        match core::cherry_pick_commits(&repo_path, &cherry_pick_shas) {
            Ok(outcome) => {
                if !outcome.skipped.is_empty() {
                    send(BuildMessage::Log(format!(
                        "  ⚠ skipped {} conflicting commit(s): {}",
                        outcome.skipped.len(),
                        outcome
                            .skipped
                            .iter()
                            .map(|s| &s[..7.min(s.len())])
                            .collect::<Vec<_>>()
                            .join(", ")
                    )));
                }
            }
            Err(e) => send(BuildMessage::Log(format!(
                "  ⚠ cherry-pick errored: {} (continuing)",
                e
            ))),
        }
    }

    // Phase 2: Apply patches
    if !patches.is_empty() {
        if let Err(e) = crate::workflow::apply_patches(&workspace, &patches, |ev| match ev {
            crate::workflow::Event::Phase(_) => {}
            crate::workflow::Event::Progress(p) => send(BuildMessage::Progress(0.02 + 0.08 * p)),
            crate::workflow::Event::CurrentItem(s) => send(BuildMessage::CurrentItem(s)),
            crate::workflow::Event::Log(s) => send(BuildMessage::Log(s)),
            crate::workflow::Event::PatchFileApplied(name) => {
                send(BuildMessage::PatchApplied(name))
            }
            crate::workflow::Event::PatchFileSkipped { name, reason } => {
                send(BuildMessage::PatchSkipped(name, reason))
            }
        }) {
            send(BuildMessage::Error(format!(
                "Patch application failed: {}",
                e
            )));
            return;
        }
    }

    // Phase 3: Compile (with autofix)
    if build_options.profile == "xtreme" {
        if let Err(e) = crate::workflow::inject_xtreme_profile(&workspace) {
            send(BuildMessage::Log(format!(
                "  ⚠ Failed to inject xtreme profile: {} (continuing)",
                e
            )));
        }
    }

    send(BuildMessage::Phase(BuildPhase::Compiling));
    send(BuildMessage::CurrentItem(format!(
        "Building codex-cli (profile {})...",
        build_options.profile
    )));

    let mut binary_path = match crate::workflow::build_with_autofix(
        &workspace,
        &build_options.profile,
        build_options.cpu_target.as_deref(),
        &build_options.optimization,
        build_options.cargo_jobs,
        |ev| match ev {
            crate::workflow::Event::Phase(_) => {}
            crate::workflow::Event::Progress(p) => send(BuildMessage::Progress(0.10 + 0.75 * p)),
            crate::workflow::Event::CurrentItem(s) => send(BuildMessage::CurrentItem(s)),
            crate::workflow::Event::Log(s) => send(BuildMessage::Log(s)),
            crate::workflow::Event::PatchFileApplied(_) => {}
            crate::workflow::Event::PatchFileSkipped { .. } => {}
        },
    ) {
        Ok(p) => p,
        Err(e) => {
            send(BuildMessage::Error(format!("Build failed: {}", e)));
            return;
        }
    };
    send(BuildMessage::Progress(0.85));

    // Optional: BOLT
    if build_options.optimization.use_bolt {
        match crate::workflow::run_bolt_optimization(&binary_path, |ev| match ev {
            crate::workflow::Event::Phase(_) => {}
            crate::workflow::Event::Progress(p) => send(BuildMessage::Progress(0.85 + 0.07 * p)),
            crate::workflow::Event::CurrentItem(s) => send(BuildMessage::CurrentItem(s)),
            crate::workflow::Event::Log(s) => send(BuildMessage::Log(s)),
            crate::workflow::Event::PatchFileApplied(_) => {}
            crate::workflow::Event::PatchFileSkipped { .. } => {}
        }) {
            Ok(bolted) => {
                binary_path = bolted;
                send(BuildMessage::Log("BOLT optimization complete".to_string()));
            }
            Err(e) => send(BuildMessage::Log(format!(
                "BOLT failed: {} (continuing)",
                e
            ))),
        }
    }

    // Optional: strip
    if build_options.strip_symbols {
        send(BuildMessage::CurrentItem(
            "Stripping symbols...".to_string(),
        ));
        if let Err(e) = crate::workflow::strip_binary(&binary_path) {
            send(BuildMessage::Log(format!(
                "  ⚠ strip failed: {} (continuing)",
                e
            )));
        }
    }

    // Optional: tests
    if run_tests {
        if let Err(e) =
            crate::workflow::run_verification_tests(&workspace, build_options.cargo_jobs, |ev| {
                match ev {
                    crate::workflow::Event::Phase(_) => {}
                    crate::workflow::Event::Progress(p) => {
                        send(BuildMessage::Progress(0.92 + 0.05 * p))
                    }
                    crate::workflow::Event::CurrentItem(s) => send(BuildMessage::CurrentItem(s)),
                    crate::workflow::Event::Log(s) => send(BuildMessage::Log(s)),
                    crate::workflow::Event::PatchFileApplied(_) => {}
                    crate::workflow::Event::PatchFileSkipped { .. } => {}
                }
            })
        {
            send(BuildMessage::Log(format!(
                "  ⚠ tests errored: {} (continuing)",
                e
            )));
        }
    }

    // Optional: alias setup
    if setup_alias {
        send(BuildMessage::Phase(BuildPhase::Installing));
        send(BuildMessage::CurrentItem(
            "Setting up shell alias...".to_string(),
        ));
        match crate::workflow::setup_alias(&binary_path) {
            Ok(Some(rc_file)) => {
                send(BuildMessage::InstallPath(rc_file.clone()));
                send(BuildMessage::Log(format!(
                    "  ✓ Added/updated alias in {}",
                    rc_file
                )));
            }
            Ok(None) => {
                send(BuildMessage::InstallPath("manual".to_string()));
                send(BuildMessage::Log(format!(
                    "  ⚠ Fish shell detected: add alias manually: alias codex=\"{}\"",
                    binary_path.display()
                )));
            }
            Err(e) => send(BuildMessage::Log(format!("  ⚠ alias setup failed: {}", e))),
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
