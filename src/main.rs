//! codex-xtreme: Interactive wizard for building patched Codex
//!
//! Builds an optimized, patched version of OpenAI's Codex CLI.
//! The Codex workspace is at {repo}/codex-rs/, and the binary is codex-cli.

use anyhow::{bail, Context, Result};
use cargo_metadata::Message;
use cliclack::{confirm, input, intro, log, multiselect, outro, select, spinner};
use codex_patcher::{
    apply_patches as patcher_apply,
    compiler::{try_autofix_all, CompileDiagnostic},
    load_from_path, Edit, PatchConfig, PatchResult,
};
use codex_xtreme::core::check_prerequisites;
use codex_xtreme::cpu_detect::detect_cpu_target;
use std::ffi::OsStr;
use std::io::BufRead;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::SystemTime;
use tracing::{debug, info, instrument, warn};

/// Build error with captured diagnostics for auto-fix.
#[derive(Debug)]
enum BuildError {
    /// Compilation failed with diagnostics that may be auto-fixable
    CompileError { diagnostics: Vec<CompileDiagnostic> },
    /// Other build failure (spawn failed, etc.)
    Other(anyhow::Error),
}

/// CLI arguments
struct Args {
    /// Developer mode - enables cherry-pick UI and other advanced options
    dev_mode: bool,
    /// Print CPU detection result and exit
    detect_cpu_only: bool,
}

fn resolve_command_path(name: &str) -> Result<PathBuf> {
    which::which(name).map_err(|_| anyhow::anyhow!("Required command not found in PATH: {name}"))
}

// ═══════════════════════════════════════════════════════════════════════════
// CONSTANTS
// ═══════════════════════════════════════════════════════════════════════════

/// The Rust workspace lives in this subdirectory of the repo root
const CODEX_RS_SUBDIR: &str = "codex-rs";

/// The package name (for cargo -p)
const CODEX_PACKAGE: &str = "codex-cli";

/// The binary name (output file)
const CODEX_BINARY: &str = "codex";

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
        eprintln!("  --detect-cpu-only   Print CPU detection result and exit");
        eprintln!("  --help, -h   Show this help message");
        eprintln!("\nEnvironment:");
        eprintln!("  RUST_LOG=debug    Enable debug logging");
        std::process::exit(0);
    }

    Args {
        dev_mode: args.iter().any(|a| a == "--dev" || a == "-d"),
        detect_cpu_only: args.iter().any(|a| a == "--detect-cpu-only"),
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
    checkout_version(&repo.path, &target_tag)?;
    sp.stop(format!("Checked out {}", target_tag));

    // ───────────────────────────────────────────────────────────────────────
    // PHASE 4: Patch Selection
    // ───────────────────────────────────────────────────────────────────────
    let available_patches = get_available_patches()?;

    if available_patches.is_empty() {
        log::warning("No patches found. Skipping patch selection.")?;
    } else {
        let patch_items: Vec<(PathBuf, String, String)> = available_patches
            .iter()
            .map(|(path, config)| {
                (
                    path.clone(),
                    config.meta.name.clone(),
                    config.meta.description.clone().unwrap_or_default(),
                )
            })
            .collect();

        // Default: patches with "privacy", "subagent", or "undo" in the name
        let defaults: Vec<PathBuf> = available_patches
            .iter()
            .filter(|(_, c)| {
                let name = c.meta.name.to_lowercase();
                name.contains("privacy") || name.contains("subagent") || name.contains("undo")
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
            apply_patches(&workspace, &selected_patches)?;
            sp.stop("Patches applied");
        }
    }

    // ───────────────────────────────────────────────────────────────────────
    // PHASE 5: Build Configuration
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

    let use_mold = if has_mold {
        confirm("Use mold linker? (faster linking, same binary)")
            .initial_value(true)
            .interact()?
    } else {
        false
    };

    // BOLT optimization (xtreme profile only, requires llvm-bolt)
    let use_bolt = if profile == "xtreme" && which::which("llvm-bolt").is_ok() {
        confirm("Run BOLT optimization? (profile + reoptimize for +10-15% speed)")
            .initial_value(false)
            .interact()?
    } else {
        false
    };

    // ───────────────────────────────────────────────────────────────────────
    // PHASE 6: Cherry-pick Commits (--dev mode only)
    // ───────────────────────────────────────────────────────────────────────
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
            .collect();

        if !cherry_pick_shas.is_empty() {
            let sp = spinner();
            sp.start(format!(
                "Cherry-picking {} commits...",
                cherry_pick_shas.len()
            ));
            cherry_pick_commits(&repo.path, &cherry_pick_shas)?;
            sp.stop("Commits applied");
        }
    }

    // ───────────────────────────────────────────────────────────────────────
    // PHASE 7: Build (renumbered from removing old cherry-pick phase)
    // ───────────────────────────────────────────────────────────────────────
    if profile == "xtreme" {
        inject_xtreme_profile(&workspace)?;
    }

    log::info("Starting build (this may take a while)...")?;

    let mut binary_path = build_with_autofix(
        &workspace,
        &profile,
        if use_cpu_opt {
            Some(cpu_target.rustc_target_cpu())
        } else {
            None
        },
        use_mold,
        use_bolt, // Pass emit-relocs flag if BOLT is enabled
    )?;

    log::success(format!("Build complete: {}", binary_path.display()))?;

    // BOLT post-link optimization
    if use_bolt {
        let sp = spinner();
        sp.start("Running BOLT optimization (profile + reoptimize)...");
        match run_bolt_optimization(&binary_path) {
            Ok(bolted_path) => {
                binary_path = bolted_path;
                sp.stop("BOLT optimization complete");
            }
            Err(e) => {
                sp.stop(format!("BOLT failed: {} (using non-BOLT binary)", e));
            }
        }
    }

    // ───────────────────────────────────────────────────────────────────────
    // PHASE 8: Test & Finish
    // ───────────────────────────────────────────────────────────────────────
    if confirm("Run quick verification tests?")
        .initial_value(true)
        .interact()?
    {
        run_verification_tests(&workspace)?;
    }

    if confirm("Set up shell alias?")
        .initial_value(true)
        .interact()?
    {
        setup_alias(&binary_path)?;
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
    // Try git describe first
    let git = match resolve_command_path("git") {
        Ok(path) => path,
        Err(_) => {
            let workspace = repo.join(CODEX_RS_SUBDIR);
            return read_workspace_version(&workspace).ok();
        }
    };
    let output = Command::new(git)
        .current_dir(repo)
        .args(["describe", "--tags", "--abbrev=0", "--match", "rust-v*"])
        .output()
        .ok()?;

    let tag = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if !tag.is_empty() {
        return Some(tag.strip_prefix("rust-v").unwrap_or(&tag).to_string());
    }

    // Fallback to workspace Cargo.toml version
    let workspace = repo.join(CODEX_RS_SUBDIR);
    read_workspace_version(&workspace).ok()
}

/// Checkout a specific version (tag or branch)
#[instrument(skip(repo), fields(repo = %repo.display()))]
fn checkout_version(repo: &Path, version: &str) -> Result<()> {
    // First, stash any local changes
    Command::new(resolve_command_path("git")?)
        .current_dir(repo)
        .args(["stash", "--include-untracked"])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .ok();

    // Checkout the version
    let status = Command::new(resolve_command_path("git")?)
        .current_dir(repo)
        .args(["checkout", version])
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .status()?;

    if !status.success() {
        bail!("Failed to checkout {}", version);
    }

    Ok(())
}

fn cherry_pick_commits(repo: &Path, shas: &[String]) -> Result<()> {
    for sha in shas {
        let status = Command::new(resolve_command_path("git")?)
            .current_dir(repo)
            .args(["cherry-pick", "--no-commit", sha])
            .status()?;

        if !status.success() {
            Command::new(resolve_command_path("git")?)
                .current_dir(repo)
                .args(["cherry-pick", "--abort"])
                .status()
                .ok();
            log::warning(format!(
                "Skipped conflicting commit: {}",
                &sha[..7.min(sha.len())]
            ))?;
        }
    }
    Ok(())
}

// ═══════════════════════════════════════════════════════════════════════════
// PATCHES (via codex-patcher library)
// ═══════════════════════════════════════════════════════════════════════════

/// Find the patches directory from codex-patcher
#[instrument]
fn find_patches_dir() -> Result<PathBuf> {
    // Check known locations in priority order
    let candidates = [
        // Development: sibling directory (where codex-patcher lives)
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../codex-patcher/patches"),
        // Installed: ~/.config/codex-patcher/patches
        dirs::config_dir()
            .unwrap_or_default()
            .join("codex-patcher/patches"),
    ];

    // Also check env override
    if let Ok(env_path) = std::env::var("CODEX_PATCHER_PATCHES") {
        let path = PathBuf::from(env_path);
        if path.exists() && path.is_dir() {
            return Ok(path.canonicalize()?);
        }
    }

    for path in candidates {
        debug!(path = %path.display(), exists = path.exists(), "Checking patches dir candidate");
        if path.exists() && path.is_dir() {
            let canonical = path.canonicalize()?;
            info!(path = %canonical.display(), "Found patches directory");
            return Ok(canonical);
        }
    }

    warn!("Could not find patches directory");
    bail!("Could not find patches directory. Set CODEX_PATCHER_PATCHES env var.")
}

/// Load all available patches from the patches directory
#[instrument]
fn get_available_patches() -> Result<Vec<(PathBuf, PatchConfig)>> {
    let patches_dir = find_patches_dir()?;
    let mut patches = Vec::new();

    for entry in std::fs::read_dir(&patches_dir)? {
        let path = entry?.path();
        if path.extension() == Some(OsStr::new("toml")) {
            debug!(path = %path.display(), "Loading patch file");
            // Skip non-patch files by checking if they contain patches
            match load_from_path(&path) {
                Ok(config) if !config.patches.is_empty() => {
                    debug!(name = %config.meta.name, patch_count = config.patches.len(), "Loaded patch");
                    patches.push((path, config));
                }
                Ok(config) => {
                    // Has meta but no patches - likely a template or WIP
                    debug!(path = %path.display(), name = %config.meta.name, "Skipping patch file with no patches");
                }
                Err(e) => {
                    // Only warn if it looks like a real patch file (not a template)
                    debug!(path = %path.display(), error = %e, "Failed to load patch file");
                }
            }
        }
    }

    // Sort by name for consistent ordering
    patches.sort_by(|a, b| a.1.meta.name.cmp(&b.1.meta.name));

    info!(count = patches.len(), "Loaded patches");
    Ok(patches)
}

/// Read the workspace version from Cargo.toml
fn read_workspace_version(workspace: &Path) -> Result<String> {
    let cargo_toml = workspace.join("Cargo.toml");
    let content =
        std::fs::read_to_string(&cargo_toml).context("Failed to read workspace Cargo.toml")?;

    // Look for version = "x.y.z" in [workspace.package] or top-level
    for line in content.lines() {
        let line = line.trim();
        if line.starts_with("version") && line.contains('=') {
            if let Some(version) = line.split('=').nth(1) {
                let version = version.trim().trim_matches('"').trim_matches('\'');
                if !version.is_empty() {
                    return Ok(version.to_string());
                }
            }
        }
    }

    // Fallback: try to get from git tag
    let output = match resolve_command_path("git") {
        Ok(path) => Command::new(path)
            .current_dir(workspace)
            .args(["describe", "--tags", "--abbrev=0"])
            .output()
            .ok(),
        Err(_) => None,
    };

    if let Some(output) = output {
        let tag = String::from_utf8_lossy(&output.stdout).trim().to_string();
        // Extract version from tag like "rust-v0.88.0" or "v0.88.0"
        if let Some(version) = tag.strip_prefix("rust-v").or_else(|| tag.strip_prefix("v")) {
            return Ok(version.to_string());
        }
        if !tag.is_empty() {
            return Ok(tag);
        }
    }

    // Ultimate fallback
    Ok("0.0.0".to_string())
}

/// Apply selected patches using codex-patcher library
#[instrument(skip(workspace, selected_files), fields(workspace = %workspace.display(), count = selected_files.len()))]
fn apply_patches(workspace: &Path, selected_files: &[PathBuf]) -> Result<()> {
    let workspace_version = read_workspace_version(workspace)?;

    for patch_file in selected_files {
        let config = load_from_path(patch_file)
            .with_context(|| format!("Failed to load patch: {}", patch_file.display()))?;

        let results = patcher_apply(&config, workspace, &workspace_version);

        for (patch_id, result) in results {
            match result {
                Ok(PatchResult::Applied { file }) => {
                    log::success(format!("Applied {}: {}", patch_id, file.display()))?;
                }
                Ok(PatchResult::AlreadyApplied { file }) => {
                    log::info(format!("Already applied {}: {}", patch_id, file.display()))?;
                }
                Ok(PatchResult::SkippedVersion { reason }) => {
                    log::warning(format!("Skipped {}: {}", patch_id, reason))?;
                }
                Ok(PatchResult::Failed { file, reason }) => {
                    log::warning(format!(
                        "Failed {}: {} - {}",
                        patch_id,
                        file.display(),
                        reason
                    ))?;
                }
                Err(e) => {
                    log::warning(format!("Error applying {}: {}", patch_id, e))?;
                }
            }
        }
    }

    Ok(())
}

// ═══════════════════════════════════════════════════════════════════════════
// BUILD SYSTEM
// ═══════════════════════════════════════════════════════════════════════════

fn inject_xtreme_profile(workspace: &Path) -> Result<()> {
    let cargo_toml = workspace.join("Cargo.toml");
    let contents = std::fs::read_to_string(&cargo_toml)?;

    if contents.contains("[profile.xtreme]") {
        return Ok(());
    }

    let profile = r#"

# Injected by codex-xtreme
[profile.xtreme]
inherits = "release"
lto = "fat"
codegen-units = 1
opt-level = 3
strip = false
debug = 1
panic = "abort"
overflow-checks = false

[profile.xtreme.build-override]
opt-level = 3

[profile.xtreme.package."*"]
opt-level = 3
"#;

    std::fs::write(&cargo_toml, format!("{}{}", contents, profile))?;
    log::step("Injected xtreme profile into Cargo.toml")?;

    Ok(())
}

/// Build with automatic fix loop for compiler errors.
///
/// When a build fails:
/// 1. Extract diagnostics from build output (no separate cargo check needed)
/// 2. Attempt to auto-fix E0063 (missing struct fields) and machine-applicable fixes
/// 3. Retry the build (up to MAX_FIX_ATTEMPTS times)
/// 4. If unfixable, display errors and fail
fn build_with_autofix(
    workspace: &Path,
    profile: &str,
    cpu_target: Option<&str>,
    use_mold: bool,
    emit_relocs: bool,
) -> Result<PathBuf> {
    const MAX_FIX_ATTEMPTS: usize = 5;

    for attempt in 1..=MAX_FIX_ATTEMPTS {
        match run_cargo_build(workspace, profile, cpu_target, use_mold, emit_relocs) {
            Ok(path) => return Ok(path),
            Err(BuildError::Other(e)) => {
                // Non-compile error (spawn failed, etc.) - can't auto-fix
                return Err(e);
            }
            Err(BuildError::CompileError { diagnostics }) => {
                if diagnostics.is_empty() {
                    // No diagnostics captured, can't auto-fix
                    log::error("Build failed but no diagnostics captured")?;
                    bail!("Build failed with unknown error");
                }

                log::warning(format!(
                    "Build failed (attempt {}/{}), checking for auto-fixable errors...",
                    attempt, MAX_FIX_ATTEMPTS
                ))?;

                // Attempt auto-fix using diagnostics from build output
                let (edits, unfixable) = try_autofix_all(&diagnostics, workspace);

                if edits.is_empty() {
                    // No fixes available - display errors and fail
                    log::error(format!(
                        "No auto-fixes available for {} error(s)",
                        unfixable.len()
                    ))?;
                    for diag in &unfixable {
                        if let Some(rendered) = &diag.rendered {
                            eprint!("{}", rendered);
                        } else {
                            eprintln!("error: {}", diag.message);
                        }
                    }
                    bail!("Build failed with {} unfixable error(s)", unfixable.len());
                }

                // Apply fixes
                log::step(format!(
                    "Applying {} auto-fix(es) (attempt {})",
                    edits.len(),
                    attempt
                ))?;

                for edit in &edits {
                    debug!(
                        "Applying fix to {}: {}..{}",
                        edit.file.display(),
                        edit.byte_start,
                        edit.byte_end
                    );
                }

                // Apply all edits
                match Edit::apply_batch(edits) {
                    Ok(results) => {
                        let applied = results
                            .iter()
                            .filter(|r| {
                                matches!(r, codex_patcher::edit::EditResult::Applied { .. })
                            })
                            .count();
                        log::info(format!("Applied {} fix(es)", applied))?;
                    }
                    Err(edit_err) => {
                        log::error(format!("Failed to apply fixes: {}", edit_err))?;
                        bail!("Failed to apply auto-fixes: {}", edit_err);
                    }
                }

                // Show remaining unfixable errors
                if !unfixable.is_empty() {
                    log::warning(format!(
                        "{} error(s) could not be auto-fixed",
                        unfixable.len()
                    ))?;
                }

                // Loop will retry the build
            }
        }
    }

    bail!(
        "Build failed after {} auto-fix attempts. Manual intervention required.",
        MAX_FIX_ATTEMPTS
    )
}

fn run_cargo_build(
    workspace: &Path,
    profile: &str,
    cpu_target: Option<&str>,
    use_mold: bool,
    emit_relocs: bool, // For BOLT optimization
) -> Result<PathBuf, BuildError> {
    let mut cmd = Command::new(resolve_command_path("cargo").map_err(BuildError::Other)?);
    cmd.current_dir(workspace)
        .args([
            "build",
            "--profile",
            profile,
            "-p",
            CODEX_PACKAGE,
            "--message-format=json",
        ])
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit());

    let mut rustflags = Vec::new();
    if let Some(cpu) = cpu_target {
        rustflags.push(format!("-C target-cpu={}", cpu));
    }
    if use_mold {
        rustflags.push("-C link-arg=-fuse-ld=mold".into());
    }
    if emit_relocs {
        // Required for BOLT to rewrite the binary
        rustflags.push("-C link-arg=-Wl,--emit-relocs".into());
    }
    if !rustflags.is_empty() {
        cmd.env("RUSTFLAGS", rustflags.join(" "));
    }

    let child = cmd.spawn();
    let mut child = match child {
        Ok(c) => c,
        Err(e) => return Err(BuildError::Other(e.into())),
    };

    let stdout = match child.stdout.take() {
        Some(s) => s,
        None => {
            return Err(BuildError::Other(anyhow::anyhow!(
                "Failed to capture stdout"
            )))
        }
    };
    let reader = std::io::BufReader::new(stdout);

    let sp = spinner();
    sp.start("Compiling...");

    let mut artifact_count = 0;
    let mut binary_path: Option<PathBuf> = None;
    let mut compiler_errors: Vec<cargo_metadata::diagnostic::Diagnostic> = Vec::new();

    for line in reader.lines() {
        let line = match line {
            Ok(l) => l,
            Err(e) => return Err(BuildError::Other(e.into())),
        };
        // Defensive: proc macros can print garbage, only parse JSON lines
        if !line.starts_with('{') {
            continue;
        }
        if let Ok(message) = serde_json::from_str::<Message>(&line) {
            match message {
                Message::CompilerArtifact(art) => {
                    artifact_count += 1;
                    sp.set_message(format!("[{}] {}", artifact_count, art.target.name));

                    if art.target.name == CODEX_BINARY {
                        for path in &art.filenames {
                            let p = PathBuf::from(path);
                            // Accept executable: no extension (Unix) or .exe (Windows)
                            let is_executable = p
                                .extension()
                                .is_none_or(|e| e.is_empty() || e.eq_ignore_ascii_case("exe"));
                            if is_executable {
                                binary_path = Some(p);
                            }
                        }
                    }
                }
                Message::CompilerMessage(msg) => {
                    // Collect error-level diagnostics for auto-fix
                    if matches!(
                        msg.message.level,
                        cargo_metadata::diagnostic::DiagnosticLevel::Error
                    ) {
                        compiler_errors.push(msg.message);
                    }
                }
                Message::BuildFinished(fin) => {
                    if !fin.success {
                        sp.stop("Build failed!");
                        // Convert to CompileDiagnostic and return for auto-fix
                        let diagnostics: Vec<CompileDiagnostic> = compiler_errors
                            .iter()
                            .map(|e| CompileDiagnostic::from_cargo(e, workspace))
                            .collect();
                        return Err(BuildError::CompileError { diagnostics });
                    }
                }
                _ => {}
            }
        }
    }

    let status = match child.wait() {
        Ok(s) => s,
        Err(e) => return Err(BuildError::Other(e.into())),
    };

    if !status.success() {
        sp.stop("Build failed!");
        // Convert to CompileDiagnostic and return for auto-fix
        let diagnostics: Vec<CompileDiagnostic> = compiler_errors
            .iter()
            .map(|e| CompileDiagnostic::from_cargo(e, workspace))
            .collect();
        return Err(BuildError::CompileError { diagnostics });
    }

    sp.stop(format!("Compiled {} crates", artifact_count));

    if let Some(path) = binary_path {
        return Ok(path);
    }

    // Fallback: construct expected path
    let target_dir = workspace.join("target");
    #[cfg(target_os = "windows")]
    let binary_name = format!("{}.exe", CODEX_BINARY);
    #[cfg(not(target_os = "windows"))]
    let binary_name = CODEX_BINARY;
    let binary = target_dir.join(profile).join(binary_name);
    if binary.exists() {
        return Ok(binary);
    }

    Err(BuildError::Other(anyhow::anyhow!(
        "Built binary not found. Expected at: {}",
        binary.display()
    )))
}

/// Run BOLT optimization on a binary
///
/// BOLT (Binary Optimization and Layout Tool) reorders code based on profiling
/// data for better cache locality and branch prediction.
///
/// Steps:
/// 1. Profile the binary with perf (using hardware LBR counters)
/// 2. Convert perf data to BOLT format
/// 3. Reoptimize binary with llvm-bolt
fn run_bolt_optimization(binary_path: &Path) -> Result<PathBuf> {
    let binary_dir = binary_path.parent().context("Binary has no parent dir")?;
    let binary_name = binary_path.file_name().context("Binary has no filename")?;
    let bolted_binary = binary_dir.join(format!("{}-bolt", binary_name.to_string_lossy()));
    let perf_data = binary_dir.join("perf.data");
    let bolt_profile = binary_dir.join("perf.fdata");
    let mut use_lbr = true;
    let perf_path =
        resolve_command_path("perf").context("perf is required for BOLT optimization")?;
    let perf2bolt_path =
        resolve_command_path("perf2bolt").context("perf2bolt is required for BOLT optimization")?;
    let bolt_path =
        resolve_command_path("llvm-bolt").context("llvm-bolt is required for BOLT optimization")?;

    // Step 1: Profile with perf using LBR (Last Branch Record) sampling
    // This has zero runtime overhead compared to instrumentation
    log::info("Profiling binary with perf LBR (run some typical commands)...")?;

    let perf_output = Command::new(&perf_path)
        .args([
            "record",
            "-e",
            "cycles:u",
            "-j",
            "any,u", // LBR sampling (AMD BRS / Intel LBR)
            "-o",
            perf_data.to_str().unwrap(),
            "--",
        ])
        .arg(binary_path)
        .args(["--version"]) // Quick workload for demo
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .output();

    let perf_failed = match &perf_output {
        Ok(output) => !output.status.success(),
        Err(_) => true,
    };
    if perf_failed {
        use_lbr = false;
        if let Ok(output) = perf_output {
            let stderr = String::from_utf8_lossy(&output.stderr);
            let stderr = stderr.trim();
            if !stderr.is_empty() {
                log::warning(format!("perf LBR record failed: {stderr}"))?;
            } else {
                log::warning(format!("perf LBR record failed: {}", output.status))?;
            }
        } else {
            log::warning("perf LBR record failed (failed to spawn perf).")?;
        }
        // Try without LBR (fallback for systems without hardware support)
        let perf_fallback_output = Command::new(&perf_path)
            .args([
                "record",
                "-e",
                "cycles:u",
                "-o",
                perf_data.to_str().unwrap(),
                "--",
            ])
            .arg(binary_path)
            .args(["--version"])
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .output()
            .context("perf record failed")?;
        if !perf_fallback_output.status.success() {
            let stderr = String::from_utf8_lossy(&perf_fallback_output.stderr);
            let stderr = stderr.trim();
            if stderr.is_empty() {
                bail!("perf record failed: {}", perf_fallback_output.status);
            }
            bail!("perf record failed: {}", stderr);
        }
    }

    // Step 2: Convert perf data to BOLT format
    let mut perf2bolt_cmd = Command::new(perf2bolt_path);
    perf2bolt_cmd.args([
        "-p",
        perf_data.to_str().unwrap(),
        "-o",
        bolt_profile.to_str().unwrap(),
    ]);
    if !use_lbr {
        perf2bolt_cmd.arg("--nl");
    }
    perf2bolt_cmd.arg(binary_path);
    let perf2bolt_output = perf2bolt_cmd
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .output()
        .context("perf2bolt failed")?;

    if !perf2bolt_output.status.success() {
        let stderr = String::from_utf8_lossy(&perf2bolt_output.stderr);
        let stderr = stderr.trim();
        if stderr.is_empty() {
            bail!("perf2bolt conversion failed: {}", perf2bolt_output.status);
        }
        bail!("perf2bolt conversion failed: {}", stderr);
    }

    // Step 3: Optimize with llvm-bolt
    let bolt_output = Command::new(bolt_path)
        .arg(binary_path)
        .args(["-o", bolted_binary.to_str().unwrap()])
        .args(["-data", bolt_profile.to_str().unwrap()])
        .args([
            "-reorder-blocks=ext-tsp",   // Extended TSP for block ordering
            "-reorder-functions=cdsort", // Call-graph directed sort
            "-split-functions",          // Split hot/cold code
            "-split-all-cold",           // Aggressively split cold code
            "-dyno-stats",               // Print optimization stats
        ])
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .output()
        .context("llvm-bolt failed")?;

    if !bolt_output.status.success() {
        let stderr = String::from_utf8_lossy(&bolt_output.stderr);
        let stderr = stderr.trim();
        if stderr.is_empty() {
            bail!("llvm-bolt optimization failed: {}", bolt_output.status);
        }
        bail!("llvm-bolt optimization failed: {}", stderr);
    }

    // Cleanup temp files
    std::fs::remove_file(&perf_data).ok();
    std::fs::remove_file(&bolt_profile).ok();

    Ok(bolted_binary)
}

// ═══════════════════════════════════════════════════════════════════════════
// VERIFICATION & SETUP
// ═══════════════════════════════════════════════════════════════════════════

fn run_verification_tests(workspace: &Path) -> Result<()> {
    let tests = [
        ("cargo check", vec!["check", "--all"]),
        (
            "codex-common tests",
            vec!["test", "-p", "codex-common", "--lib"],
        ),
    ];

    for (name, args) in tests {
        let sp = spinner();
        sp.start(format!("Running {}...", name));

        let status = Command::new(resolve_command_path("cargo")?)
            .current_dir(workspace)
            .args(&args)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()?;

        if status.success() {
            sp.stop(format!("✓ {}", name));
        } else {
            sp.stop(format!("✗ {} (failed)", name));
            log::warning("Test failed, but continuing...")?;
        }
    }

    Ok(())
}

fn setup_alias(binary_path: &Path) -> Result<()> {
    let shell = std::env::var("SHELL").unwrap_or_default();

    let rc_file = if shell.contains("zsh") {
        shellexpand::tilde("~/.zshrc").to_string()
    } else if shell.contains("fish") {
        log::warning("Fish shell detected - please add alias manually:")?;
        log::info(format!("  alias codex=\"{}\"", binary_path.display()))?;
        return Ok(());
    } else {
        shellexpand::tilde("~/.bashrc").to_string()
    };

    let alias_line = format!("alias codex=\"{}\"", binary_path.display());

    if let Ok(contents) = std::fs::read_to_string(&rc_file) {
        if contents.contains("alias codex=") {
            log::step("Alias already exists, updating...")?;
            let mut updated_lines = Vec::new();
            for line in contents.lines() {
                if line.trim_start().starts_with("alias codex=") {
                    updated_lines.push(alias_line.clone());
                } else {
                    updated_lines.push(line.to_string());
                }
            }
            let updated = updated_lines.join("\n");
            std::fs::write(&rc_file, format!("{updated}\n"))?;
        } else {
            std::fs::write(
                &rc_file,
                format!("{}\n\n# Added by codex-xtreme\n{}\n", contents, alias_line),
            )?;
        }
    }

    log::success(format!("Added alias to {}", rc_file))?;
    log::info("Run `source ~/.zshrc` or restart your shell")?;

    Ok(())
}
