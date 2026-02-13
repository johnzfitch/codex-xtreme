//! codex-xtreme: Interactive wizard for building patched Codex
//!
//! Builds an optimized, patched version of OpenAI's Codex CLI.
//! The Codex workspace is at {repo}/codex-rs/, and the binary is codex-cli.

use anyhow::{bail, Context, Result};
use cliclack::{confirm, input, intro, log, multiselect, outro, select, spinner};
use codex_xtreme::core::check_prerequisites;
use codex_xtreme::cpu_detect::detect_cpu_target;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::SystemTime;
use tracing::{debug, info, instrument, warn};

/// CLI arguments
struct Args {
    /// Developer mode - enables cherry-pick UI and other advanced options
    dev_mode: bool,
    /// Print CPU detection result and exit
    detect_cpu_only: bool,
    /// Run the Neo Tokyo TUI (same behavior as the CLI wizard, different presentation)
    tui: bool,
    /// Limit parallel cargo jobs (reduces peak CPU usage during builds/tests).
    cargo_jobs: Option<usize>,
}

fn resolve_command_path(name: &str) -> Result<PathBuf> {
    which::which(name).map_err(|_| anyhow::anyhow!("Required command not found in PATH: {name}"))
}

// ═══════════════════════════════════════════════════════════════════════════
// CONSTANTS
// ═══════════════════════════════════════════════════════════════════════════

/// The Rust workspace lives in this subdirectory of the repo root
const CODEX_RS_SUBDIR: &str = "codex-rs";

/// GitHub repo URL
const CODEX_REPO_URL: &str = "https://github.com/openai/codex.git";

// ═══════════════════════════════════════════════════════════════════════════
// TYPES
// ═══════════════════════════════════════════════════════════════════════════

#[derive(Debug)]
struct RepoInfo {
    /// Path to the repo root (not codex-rs)
    path: PathBuf,
    age: String,
    branch: String,
}

impl RepoInfo {
    /// Returns the path to the codex-rs workspace
    fn workspace_path(&self) -> PathBuf {
        self.path.join(CODEX_RS_SUBDIR)
    }
}

#[derive(Debug, Clone)]
struct Release {
    tag: String,
    version: String,
    published: String,
}

// ═══════════════════════════════════════════════════════════════════════════
// MAIN WIZARD FLOW
// ═══════════════════════════════════════════════════════════════════════════

fn parse_args() -> Args {
    let args: Vec<String> = std::env::args().collect();

    // Show help
    if args.iter().any(|a| a == "--help" || a == "-h") {
        eprintln!("codex-xtreme - Build your perfect Codex binary\n");
        eprintln!("Usage: codex-xtreme [OPTIONS]\n");
        eprintln!("Options:");
        eprintln!("  --dev, -d    Developer mode (cherry-pick commits, extra options)");
        eprintln!("  --tui        Run the full-screen TUI (same workflow, different UI)");
        eprintln!("  --detect-cpu-only   Print CPU detection result and exit");
        eprintln!("  --jobs, -j N Limit parallel cargo jobs (reduces CPU usage)");
        eprintln!("  --help, -h   Show this help message");
        eprintln!("\nEnvironment:");
        eprintln!("  RUST_LOG=debug    Enable debug logging");
        std::process::exit(0);
    }

    fn parse_cargo_jobs(args: &[String]) -> std::result::Result<Option<usize>, String> {
        let mut found: Option<usize> = None;

        for (idx, arg) in args.iter().enumerate() {
            let value: Option<&str> = if arg == "--jobs" || arg == "-j" {
                Some(
                    args.get(idx + 1)
                        .ok_or_else(|| format!("Missing value for {arg}"))?
                        .as_str(),
                )
            } else if let Some(rest) = arg.strip_prefix("--jobs=") {
                Some(rest)
            } else if let Some(rest) = arg.strip_prefix("-j") {
                if rest.is_empty() { None } else { Some(rest) }
            } else {
                None
            };

            let Some(value) = value else { continue };
            let jobs: usize = value
                .parse()
                .map_err(|_| format!("Invalid value for --jobs/-j: {value}"))?;
            if jobs == 0 {
                return Err("Invalid value for --jobs/-j: must be >= 1".to_string());
            }
            if found.replace(jobs).is_some() {
                return Err("Multiple --jobs/-j values provided; use only one".to_string());
            }
        }

        Ok(found)
    }

    let cargo_jobs = match parse_cargo_jobs(&args) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("error: {e}");
            std::process::exit(2);
        }
    };

    Args {
        dev_mode: args.iter().any(|a| a == "--dev" || a == "-d"),
        detect_cpu_only: args.iter().any(|a| a == "--detect-cpu-only"),
        tui: args.iter().any(|a| a == "--tui"),
        cargo_jobs,
    }
}

fn main() -> Result<()> {
    let args = parse_args();

    if args.detect_cpu_only {
        let cpu_target = detect_cpu_target();
        println!(
            "cpu.name={} detected_by={}",
            cpu_target.name, cpu_target.detected_by
        );
        return Ok(());
    }

    // `codex-xtreme --tui` runs the same workflow via the ratatui UI.
    if args.tui {
        if let Err(err) = check_prerequisites() {
            eprintln!("{err}");
            std::process::exit(1);
        }

        let rt = tokio::runtime::Runtime::new().context("Failed to create tokio runtime")?;
        return rt
            .block_on(codex_xtreme::tui::run_app(args.dev_mode, args.cargo_jobs))
            .map_err(|e| anyhow::anyhow!(e));
    }

    // Initialize tracing - use RUST_LOG env var (e.g., RUST_LOG=debug)
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive(tracing::Level::WARN.into()),
        )
        .with_target(false)
        .init();

    info!("Starting codex-xtreme");

    // Setup Ctrl-C handler for graceful exit
    ctrlc::set_handler(|| {
        std::process::exit(130);
    })
    .ok();

    if let Err(err) = check_prerequisites() {
        eprintln!("{err}");
        std::process::exit(1);
    }

    if args.dev_mode {
        intro("🚀 CODEX XTREME [DEV MODE] - Build Your Perfect Codex")?;
    } else {
        intro("🚀 CODEX XTREME - Build Your Perfect Codex")?;
    }

    // ───────────────────────────────────────────────────────────────────────
    // PHASE 1: System Detection
    // ───────────────────────────────────────────────────────────────────────
    let sp = spinner();
    sp.start("Detecting system configuration...");

    let cpu_target = detect_cpu_target();
    let has_mold = which::which("mold").is_ok();
    let rust_ver = rustc_version::version()
        .map(|v| format!("{}", v))
        .unwrap_or_else(|_| "unknown".into());

    sp.stop(format!(
        "System: {} | mold: {} | rustc {}",
        cpu_target.display_name(),
        if has_mold { "yes" } else { "no" },
        rust_ver
    ));

    // ───────────────────────────────────────────────────────────────────────
    // PHASE 2: Repository Selection
    // ───────────────────────────────────────────────────────────────────────
    let repos = find_codex_repos()?;

    let repo = if repos.is_empty() {
        log::info("No existing Codex repositories found")?;
        if confirm("Clone fresh from GitHub?")
            .initial_value(true)
            .interact()?
        {
            clone_codex()?
        } else {
            bail!("No repository selected");
        }
    } else {
        let mut items: Vec<(String, String, String)> = repos
            .iter()
            .map(|r| {
                (
                    r.path.display().to_string(),
                    format!("{}", r.path.display()),
                    format!("{} | {}", r.branch, r.age),
                )
            })
            .collect();
        items.push((
            "__clone__".into(),
            "Clone fresh".into(),
            "Get latest from GitHub".into(),
        ));

        let selected: String = select("Select Codex repository").items(&items).interact()?;

        if selected == "__clone__" {
            clone_codex()?
        } else {
            repos
                .into_iter()
                .find(|r| r.path.display().to_string() == selected)
                .expect("Selected repo not found")
        }
    };

    let workspace = repo.workspace_path();
    if !workspace.exists() {
        bail!(
            "Codex workspace not found at {}. Is this a valid Codex repo?",
            workspace.display()
        );
    }

    log::info(format!(
        "Using: {} (workspace: {})",
        repo.path.display(),
        CODEX_RS_SUBDIR
    ))?;

    // ───────────────────────────────────────────────────────────────────────
    // PHASE 3: Version Selection
    // ───────────────────────────────────────────────────────────────────────
    let sp = spinner();
    sp.start("Fetching releases from GitHub...");
    fetch_repo(&repo.path)?;
    let releases = get_github_releases(&repo.path)?;
    let current_version = get_current_version(&repo.path);
    sp.stop(format!(
        "Found {} releases (current: {})",
        releases.len(),
        current_version.as_deref().unwrap_or("unknown")
    ));

    // Let user select target version
    let target_tag = if releases.is_empty() {
        log::warning("No releases found, using main branch")?;
        "main".to_string()
    } else {
        // Separate stable and pre-release versions
        let stable_releases: Vec<_> = releases
            .iter()
            .filter(|r| {
                !r.version.contains("alpha")
                    && !r.version.contains("beta")
                    && !r.version.contains("rc")
            })
            .collect();

        let display_releases = if stable_releases.is_empty() {
            // No stable releases, show all
            &releases
        } else {
            // Show stable first, then add option for pre-releases
            &releases
        };

        let mut release_items: Vec<(String, String, String)> = display_releases
            .iter()
            .enumerate()
            .map(|(i, r)| {
                let is_prerelease = r.version.contains("alpha")
                    || r.version.contains("beta")
                    || r.version.contains("rc");
                let label = if i == 0 && !is_prerelease {
                    format!("{} (latest stable)", r.version)
                } else if i == 0 {
                    format!("{} (latest)", r.version)
                } else {
                    r.version.clone()
                };
                let hint = if Some(&r.version) == current_version.as_ref() {
                    format!("{} - CURRENT", r.published)
                } else {
                    r.published.clone()
                };
                (r.tag.clone(), label, hint)
            })
            .collect();

        // Limit to reasonable number (last 15 releases)
        release_items.truncate(15);

        select("Select target version")
            .items(&release_items)
            .interact()?
            .to_string()
    };

    // Checkout the target version
    let sp = spinner();
    sp.start(format!("Checking out {}...", target_tag));
    codex_xtreme::core::checkout_version(&repo.path, &target_tag)?;
    sp.stop(format!("Checked out {}", target_tag));

    // ───────────────────────────────────────────────────────────────────────
    // PHASE 4: Cherry-pick Commits (--dev mode only)
    // ───────────────────────────────────────────────────────────────────────
    //
    // NOTE: This happens immediately after checkout while the working tree is clean.
    // Cherry-picking after applying patches can fail because git refuses to operate
    // with local modifications.
    if args.dev_mode {
        log::info(format!(
            "Dev mode: View commits at https://github.com/openai/codex/compare/{}...main",
            target_tag
        ))?;

        let cherry_pick_input: String =
            input("Cherry-pick commits (comma-separated SHAs, or empty to skip)")
                .placeholder("abc1234, def5678")
                .default_input("")
                .interact()?;

        let cherry_pick_shas: Vec<String> = cherry_pick_input
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .filter(|s| {
                // Validate SHA format: 7-40 hex characters
                let valid =
                    s.len() >= 7 && s.len() <= 40 && s.chars().all(|c| c.is_ascii_hexdigit());
                if !valid {
                    eprintln!("Warning: Invalid SHA format '{}', skipping", s);
                }
                valid
            })
            .collect();

        if !cherry_pick_shas.is_empty() {
            let sp = spinner();
            sp.start(format!(
                "Cherry-picking {} commits...",
                cherry_pick_shas.len()
            ));
            let outcome = codex_xtreme::core::cherry_pick_commits(&repo.path, &cherry_pick_shas)?;
            sp.stop("Cherry-pick complete");

            if !outcome.skipped.is_empty() {
                log::warning(format!(
                    "Skipped {} conflicting commit(s): {}",
                    outcome.skipped.len(),
                    outcome
                        .skipped
                        .iter()
                        .map(|s| &s[..7.min(s.len())])
                        .collect::<Vec<_>>()
                        .join(", ")
                ))?;
            }
        }
    }

    // ───────────────────────────────────────────────────────────────────────
    // PHASE 5: Patch Selection
    // ───────────────────────────────────────────────────────────────────────
    let available_patches = codex_xtreme::core::get_available_patches()?;

    if available_patches.is_empty() {
        log::warning("No patches found. Skipping patch selection.")?;
    } else {
        let patch_items: Vec<(PathBuf, String, String)> = available_patches
            .iter()
            .map(|(path, config)| {
                let compatible = codex_xtreme::core::is_patch_compatible(
                    config.meta.version_range.as_deref(),
                    &target_tag,
                );
                (
                    path.clone(),
                    format!(
                        "{} ({}){}",
                        config.meta.name,
                        config.patches.len(),
                        if compatible { "" } else { " [incompatible]" }
                    ),
                    config.meta.description.clone().unwrap_or_default(),
                )
            })
            .collect();

        // Default: select only compatible patches (matches TUI behavior).
        let defaults: Vec<PathBuf> = available_patches
            .iter()
            .filter(|(_, config)| {
                codex_xtreme::core::is_patch_compatible(
                    config.meta.version_range.as_deref(),
                    &target_tag,
                )
            })
            .map(|(p, _)| p.clone())
            .collect();

        let selected_patches: Vec<PathBuf> = multiselect("Select patches to apply")
            .items(&patch_items)
            .initial_values(defaults)
            .required(false)
            .interact()?;

        if !selected_patches.is_empty() {
            let sp = spinner();
            sp.start(format!("Applying {} patches...", selected_patches.len()));
            codex_xtreme::workflow::apply_patches(&workspace, &selected_patches, |ev| match ev {
                codex_xtreme::workflow::Event::Phase(_) => {}
                codex_xtreme::workflow::Event::Progress(_) => {}
                codex_xtreme::workflow::Event::CurrentItem(s) => sp.set_message(s),
                codex_xtreme::workflow::Event::Log(s) => {
                    let _ = log::info(s);
                }
                codex_xtreme::workflow::Event::PatchFileApplied(name) => {
                    let _ = log::success(format!("Applied patch file: {}", name));
                }
                codex_xtreme::workflow::Event::PatchFileSkipped { name, reason } => {
                    let _ = log::warning(format!("Skipped patch file: {} ({})", name, reason));
                }
            })?;
            sp.stop("Patches applied");
        }
    }

    // ───────────────────────────────────────────────────────────────────────
    // PHASE 6: Build Configuration
    // ───────────────────────────────────────────────────────────────────────
    let profile: String = select("Build profile")
        .item(
            "xtreme",
            "Xtreme (Recommended)",
            "Thin LTO + parallel codegen, ~5min build, BOLT-ready",
        )
        .item(
            "release",
            "Standard Release",
            "Default cargo release, ~3min build",
        )
        .interact()?
        .to_string();

    let use_cpu_opt = confirm(format!(
        "Optimize for your CPU? ({})",
        cpu_target.display_name()
    ))
    .initial_value(true)
    .interact()?;

    let has_bolt = codex_xtreme::core::has_bolt();

    // Single selector (shared intent with the TUI):
    // - Build fast: mold
    // - Run fast: BOLT (disables mold)
    // - Custom: explicit toggles (still enforces BOLT => no mold)
    let mut opt_select = select("Optimization mode");
    if has_bolt {
        opt_select = opt_select.item(
            "run_fast",
            "Run fast (BOLT)",
            "Profile + optimize the final binary for runtime performance",
        );
    }
    opt_select = opt_select.item(
        "build_fast",
        "Build fast (mold)",
        if has_mold {
            "Faster linking; does not change runtime performance much"
        } else {
            "mold not found (mode will be equivalent to no linker optimization)"
        },
    );
    opt_select = opt_select.item(
        "custom",
        "Custom",
        "Choose mold/BOLT manually (BOLT disables mold)",
    );
    let optimization_mode: String = opt_select.interact()?.to_string();

    let opt_mode = match optimization_mode.as_str() {
        "run_fast" => codex_xtreme::workflow::OptimizationMode::RunFast,
        "build_fast" => codex_xtreme::workflow::OptimizationMode::BuildFast,
        _ => codex_xtreme::workflow::OptimizationMode::Custom,
    };

    let mut optimization =
        codex_xtreme::workflow::OptimizationFlags::from_mode(opt_mode, has_mold, has_bolt);

    if opt_mode == codex_xtreme::workflow::OptimizationMode::Custom {
        if has_mold {
            optimization.use_mold = confirm("Use mold linker? (faster linking)")
                .initial_value(true)
                .interact()?;
        } else {
            optimization.use_mold = false;
        }

        if has_bolt {
            optimization.use_bolt = confirm("Use BOLT optimization? (runtime performance)")
                .initial_value(true)
                .interact()?;
        } else {
            optimization.use_bolt = false;
        }
    }

    optimization.enforce_invariants();

    let strip_symbols = confirm("Strip symbols? (smaller binary)")
        .initial_value(true)
        .interact()?;

    // ───────────────────────────────────────────────────────────────────────
    // PHASE 7: Build
    // ───────────────────────────────────────────────────────────────────────
    if profile == "xtreme" {
        codex_xtreme::workflow::inject_xtreme_profile(&workspace)?;
    }

    log::info("Starting build (this may take a while)...")?;

    let build_sp = spinner();
    build_sp.start("Compiling...");
    let mut binary_path = codex_xtreme::workflow::build_with_autofix(
        &workspace,
        &profile,
        if use_cpu_opt {
            Some(cpu_target.rustc_target_cpu())
        } else {
            None
        },
        &optimization,
        args.cargo_jobs,
        |ev| match ev {
            codex_xtreme::workflow::Event::Phase(_) => {}
            codex_xtreme::workflow::Event::Progress(_) => {}
            codex_xtreme::workflow::Event::CurrentItem(s) => build_sp.set_message(s),
            codex_xtreme::workflow::Event::Log(_) => {}
            codex_xtreme::workflow::Event::PatchFileApplied(_) => {}
            codex_xtreme::workflow::Event::PatchFileSkipped { .. } => {}
        },
    )?;
    build_sp.stop("Compiled");

    log::success(format!("Build complete: {}", binary_path.display()))?;

    // BOLT post-link optimization
    if optimization.use_bolt {
        let sp = spinner();
        sp.start("Running BOLT optimization (profile + reoptimize)...");
        match codex_xtreme::workflow::run_bolt_optimization(&binary_path, |ev| match ev {
            codex_xtreme::workflow::Event::Phase(_) => {}
            codex_xtreme::workflow::Event::Progress(_) => {}
            codex_xtreme::workflow::Event::CurrentItem(s) => sp.set_message(s),
            codex_xtreme::workflow::Event::Log(_) => {}
            codex_xtreme::workflow::Event::PatchFileApplied(_) => {}
            codex_xtreme::workflow::Event::PatchFileSkipped { .. } => {}
        }) {
            Ok(bolted_path) => {
                binary_path = bolted_path;
                sp.stop("BOLT optimization complete");
            }
            Err(e) => {
                sp.stop(format!("BOLT failed: {} (using non-BOLT binary)", e));
            }
        }
    }

    if strip_symbols {
        let sp = spinner();
        sp.start("Stripping symbols...");
        match codex_xtreme::workflow::strip_binary(&binary_path) {
            Ok(_) => sp.stop("Stripped symbols"),
            Err(e) => sp.stop(format!("Strip failed: {} (continuing)", e)),
        }
    }

    // ───────────────────────────────────────────────────────────────────────
    // PHASE 8: Test & Finish
    // ───────────────────────────────────────────────────────────────────────
    if confirm("Run quick verification tests?")
        .initial_value(true)
        .interact()?
    {
        let sp = spinner();
        sp.start("Running verification tests...");
        codex_xtreme::workflow::run_verification_tests(&workspace, args.cargo_jobs, |ev| match ev {
            codex_xtreme::workflow::Event::Phase(_) => {}
            codex_xtreme::workflow::Event::Progress(_) => {}
            codex_xtreme::workflow::Event::CurrentItem(s) => sp.set_message(s),
            codex_xtreme::workflow::Event::Log(s) => {
                // Tests are a side step; keep output concise.
                let _ = log::info(s);
            }
            codex_xtreme::workflow::Event::PatchFileApplied(_) => {}
            codex_xtreme::workflow::Event::PatchFileSkipped { .. } => {}
        })?;
        sp.stop("Verification tests finished");
    }

    if confirm("Set up shell alias?")
        .initial_value(true)
        .interact()?
    {
        let sp = spinner();
        sp.start("Setting up shell alias...");
        match codex_xtreme::workflow::setup_alias(&binary_path)? {
            Some(rc_file) => sp.stop(format!("Updated alias in {}", rc_file)),
            None => sp.stop("Fish shell detected: add alias manually"),
        }
    }

    outro(format!(
        "✨ Done! Your optimized Codex is ready at:\n   {}",
        binary_path.display()
    ))?;

    Ok(())
}

// ═══════════════════════════════════════════════════════════════════════════
// REPOSITORY MANAGEMENT
// ═══════════════════════════════════════════════════════════════════════════

#[instrument]
fn find_codex_repos() -> Result<Vec<RepoInfo>> {
    let candidates = [
        "~/dev/codex",
        "~/dev/cod3x",
        "~/codex",
        "~/src/codex",
        "~/.local/src/codex",
        "~/projects/codex",
        "~/code/codex",
    ];

    let mut repos = Vec::new();

    for candidate in &candidates {
        let expanded = shellexpand::tilde(candidate);
        let path = PathBuf::from(expanded.as_ref());

        let workspace_cargo = path.join(CODEX_RS_SUBDIR).join("Cargo.toml");
        if workspace_cargo.exists() {
            if let Ok(contents) = std::fs::read_to_string(&workspace_cargo) {
                if contents.contains("codex-cli") || contents.contains("codex-common") {
                    let branch = get_current_branch(&path).unwrap_or_else(|_| "unknown".into());
                    let age = get_repo_age(&path);
                    repos.push(RepoInfo { path, age, branch });
                }
            }
        }
    }

    Ok(repos)
}

fn get_current_branch(repo: &Path) -> Result<String> {
    let output = Command::new(resolve_command_path("git")?)
        .current_dir(repo)
        .args(["branch", "--show-current"])
        .output()?;
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

fn get_repo_age(repo: &Path) -> String {
    let git_dir = repo.join(".git");
    let fetch_head = git_dir.join("FETCH_HEAD");

    let mtime = fetch_head
        .metadata()
        .and_then(|m| m.modified())
        .or_else(|_| git_dir.metadata().and_then(|m| m.modified()));

    match mtime {
        Ok(time) => {
            let duration = SystemTime::now().duration_since(time).unwrap_or_default();
            let secs = duration.as_secs();
            if secs < 60 {
                format!("{}s ago", secs)
            } else if secs < 3600 {
                format!("{}m ago", secs / 60)
            } else if secs < 86400 {
                format!("{}h ago", secs / 3600)
            } else {
                format!("{}d ago", secs / 86400)
            }
        }
        Err(_) => "unknown".into(),
    }
}

fn clone_codex() -> Result<RepoInfo> {
    let dest = shellexpand::tilde("~/dev/codex-xtreme-build");
    let dest_path = PathBuf::from(dest.as_ref());

    if dest_path.exists() {
        std::fs::remove_dir_all(&dest_path)?;
    }

    let sp = spinner();
    sp.start("Cloning Codex from GitHub...");

    let status = Command::new(resolve_command_path("git")?)
        .args(["clone", "--depth=100", CODEX_REPO_URL])
        .arg(&dest_path)
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .status()?;

    if !status.success() {
        bail!("Failed to clone repository");
    }

    sp.stop("Repository cloned");

    Ok(RepoInfo {
        path: dest_path,
        age: "just now".into(),
        branch: "main".into(),
    })
}

fn fetch_repo(repo: &Path) -> Result<()> {
    Command::new(resolve_command_path("git")?)
        .current_dir(repo)
        .args(["fetch", "--tags", "--quiet"])
        .status()?;
    Ok(())
}

/// Get all rust-v* releases from the repo (sorted newest first)
#[instrument(skip(repo), fields(repo = %repo.display()))]
fn get_github_releases(repo: &Path) -> Result<Vec<Release>> {
    // Get all tags matching rust-v*
    let output = Command::new(resolve_command_path("git")?)
        .current_dir(repo)
        .args([
            "tag",
            "-l",
            "rust-v*",
            "--sort=-v:refname", // Sort by version, newest first
            "--format=%(refname:short)|%(creatordate:short)",
        ])
        .output()?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    debug!(raw_tags = %stdout.lines().count(), "Fetched tags from git");

    let mut seen = std::collections::HashSet::new();
    let mut releases = Vec::new();
    for line in stdout.lines() {
        let parts: Vec<&str> = line.split('|').collect();
        let tag = match parts.first() {
            Some(tag) => tag.to_string(),
            None => continue,
        };

        // Filter out malformed tags (like rust-vv*, rust-vrust-v*)
        if !tag.starts_with("rust-v") || tag.starts_with("rust-vv") || tag.starts_with("rust-vrust")
        {
            debug!(tag = %tag, "Skipping malformed tag");
            continue;
        }

        if !seen.insert(tag.clone()) {
            debug!(tag = %tag, "Skipping duplicate tag");
            continue;
        }

        let published = parts.get(1).unwrap_or(&"").to_string();
        let version = tag.strip_prefix("rust-v").unwrap_or(&tag).to_string();

        debug!(tag = %tag, version = %version, published = %published, "Found release");

        releases.push(Release {
            tag,
            version,
            published,
        });
    }

    info!(count = releases.len(), "Found releases");
    Ok(releases)
}

/// Get the current version of the repo (from git describe or Cargo.toml)
#[instrument(skip(repo), fields(repo = %repo.display()))]
fn get_current_version(repo: &Path) -> Option<String> {
    // Delegate to the shared core implementation so CLI and TUI stay aligned.
    codex_xtreme::core::get_current_version(repo)
}

// Patch/build logic lives in codex_xtreme::core and codex_xtreme::workflow.
