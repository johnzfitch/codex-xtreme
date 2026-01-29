//! Core logic for codex-xtreme
//!
//! Shared functions used by both the cliclack UI and ratatui TUI.

use anyhow::{bail, Result};
use codex_patcher::{load_from_path, PatchConfig};
use std::ffi::OsStr;
use std::fmt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::SystemTime;

pub use crate::cpu_detect::{detect_cpu_target, CpuTarget};

/// The Rust workspace lives in this subdirectory of the repo root
pub const CODEX_RS_SUBDIR: &str = "codex-rs";

/// GitHub repo URL
pub const CODEX_REPO_URL: &str = "https://github.com/openai/codex.git";

// ═══════════════════════════════════════════════════════════════════════════
// TYPES
// ═══════════════════════════════════════════════════════════════════════════

#[derive(Debug, Clone)]
pub struct RepoInfo {
    pub path: PathBuf,
    pub age: String,
    pub branch: String,
}

impl RepoInfo {
    /// Returns the path to the codex-rs workspace
    pub fn workspace_path(&self) -> PathBuf {
        self.path.join(CODEX_RS_SUBDIR)
    }
}

#[derive(Debug, Clone)]
pub struct Release {
    pub tag: String,
    pub version: String,
    pub published: String,
}

// ═══════════════════════════════════════════════════════════════════════════
// SYSTEM DETECTION
// ═══════════════════════════════════════════════════════════════════════════

pub fn has_mold() -> bool {
    which::which("mold").is_ok()
}

pub fn has_bolt() -> bool {
    which::which("llvm-bolt").is_ok()
}

#[derive(Debug)]
pub enum PrerequisiteError {
    GitMissing(&'static str),
}

impl fmt::Display for PrerequisiteError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PrerequisiteError::GitMissing(message) => write!(f, "{message}"),
        }
    }
}

impl std::error::Error for PrerequisiteError {}

pub fn check_prerequisites() -> std::result::Result<(), PrerequisiteError> {
    if which::which("git").is_err() {
        #[cfg(target_os = "windows")]
        return Err(PrerequisiteError::GitMissing(
            "Git is required. Install from https://git-scm.com/download/win",
        ));

        #[cfg(target_os = "macos")]
        return Err(PrerequisiteError::GitMissing(
            "Git is required. Install via: xcode-select --install",
        ));

        #[cfg(target_os = "linux")]
        return Err(PrerequisiteError::GitMissing(
            "Git is required. Install via your package manager",
        ));
    }

    Ok(())
}

pub fn rust_version() -> String {
    rustc_version::version()
        .map(|v| format!("{}", v))
        .unwrap_or_else(|_| "unknown".into())
}

// ═══════════════════════════════════════════════════════════════════════════
// REPOSITORY MANAGEMENT
// ═══════════════════════════════════════════════════════════════════════════

/// Find existing Codex repositories
pub fn find_codex_repos() -> Result<Vec<RepoInfo>> {
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
    let output = Command::new("git")
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
            if secs < 60 { format!("{}s ago", secs) }
            else if secs < 3600 { format!("{}m ago", secs / 60) }
            else if secs < 86400 { format!("{}h ago", secs / 3600) }
            else { format!("{}d ago", secs / 86400) }
        }
        Err(_) => "unknown".into(),
    }
}

/// Clone the Codex repository to a destination
pub fn clone_codex(dest: &Path) -> Result<RepoInfo> {
    if dest.exists() {
        // Safety checks before removing
        if dest.is_symlink() {
            bail!(
                "Destination {} is a symlink. Remove it manually if you want to clone here.",
                dest.display()
            );
        }

        // Only remove if it looks like a git repo or empty directory
        let is_git_repo = dest.join(".git").exists();
        let is_empty = dest.read_dir().map(|mut d| d.next().is_none()).unwrap_or(false);

        if !is_git_repo && !is_empty {
            bail!(
                "Destination {} exists but doesn't look like a git repository. \
                 Remove it manually if you want to clone here.",
                dest.display()
            );
        }

        std::fs::remove_dir_all(dest)?;
    }

    let status = Command::new("git")
        .args(["clone", "--depth=100", CODEX_REPO_URL])
        .arg(dest)
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .status()?;

    if !status.success() {
        bail!("Failed to clone repository");
    }

    Ok(RepoInfo {
        path: dest.to_path_buf(),
        age: "just now".into(),
        branch: "main".into(),
    })
}

/// Fetch updates from remote
pub fn fetch_repo(repo: &Path) -> Result<()> {
    Command::new("git")
        .current_dir(repo)
        .args(["fetch", "--tags", "--quiet"])
        .status()?;
    Ok(())
}

/// Get all rust-v* releases from the repo (sorted newest first)
pub fn get_releases(repo: &Path) -> Result<Vec<Release>> {
    let output = Command::new("git")
        .current_dir(repo)
        .args([
            "tag",
            "-l",
            "rust-v*",
            "--sort=-v:refname",
            "--format=%(refname:short)|%(creatordate:short)",
        ])
        .output()?;

    let stdout = String::from_utf8_lossy(&output.stdout);

    let mut seen = std::collections::HashSet::new();
    let mut releases = Vec::new();
    for line in stdout.lines() {
        let parts: Vec<&str> = line.split('|').collect();
        let tag = match parts.first() {
            Some(tag) => tag.to_string(),
            None => continue,
        };

        // Filter out malformed tags
        if !tag.starts_with("rust-v") || tag.starts_with("rust-vv") || tag.starts_with("rust-vrust") {
            continue;
        }

        if !seen.insert(tag.clone()) {
            continue;
        }

        let published = parts.get(1).unwrap_or(&"").to_string();
        let version = tag.strip_prefix("rust-v").unwrap_or(&tag).to_string();

        releases.push(Release { tag, version, published });
    }

    Ok(releases)
}

/// Get the current version of the repo
pub fn get_current_version(repo: &Path) -> Option<String> {
    let output = Command::new("git")
        .current_dir(repo)
        .args(["describe", "--tags", "--abbrev=0", "--match", "rust-v*"])
        .output()
        .ok()?;

    let tag = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if !tag.is_empty() {
        return Some(tag.strip_prefix("rust-v").unwrap_or(&tag).to_string());
    }

    None
}

/// Check if repository has uncommitted changes
pub fn has_uncommitted_changes(repo: &Path) -> bool {
    let output = Command::new("git")
        .current_dir(repo)
        .args(["status", "--porcelain"])
        .output();

    match output {
        Ok(out) => !out.stdout.is_empty(),
        Err(_) => false,
    }
}

/// Checkout a specific version (tag or branch)
///
/// Returns an error if there are uncommitted changes to prevent accidental data loss.
pub fn checkout_version(repo: &Path, version: &str) -> Result<()> {
    // Check for uncommitted changes first
    if has_uncommitted_changes(repo) {
        bail!(
            "Repository has uncommitted changes. Please commit or stash them first:\n  \
             cd {} && git stash",
            repo.display()
        );
    }

    // Checkout the version
    let status = Command::new("git")
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

// ═══════════════════════════════════════════════════════════════════════════
// PATCHES
// ═══════════════════════════════════════════════════════════════════════════

/// Find the patches directory
pub fn find_patches_dir() -> Result<PathBuf> {
    let candidates = [
        // Development: sibling directory
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../codex-patcher/patches"),
        // Installed: ~/.config/codex-patcher/patches
        dirs::config_dir()
            .unwrap_or_default()
            .join("codex-patcher/patches"),
    ];

    if let Ok(env_path) = std::env::var("CODEX_PATCHER_PATCHES") {
        let path = PathBuf::from(env_path);
        if path.exists() && path.is_dir() {
            return Ok(path.canonicalize()?);
        }
    }

    for path in candidates {
        if path.exists() && path.is_dir() {
            return Ok(path.canonicalize()?);
        }
    }

    bail!("Could not find patches directory. Set CODEX_PATCHER_PATCHES env var.")
}

/// Load all available patches
pub fn get_available_patches() -> Result<Vec<(PathBuf, PatchConfig)>> {
    let patches_dir = find_patches_dir()?;
    let mut patches = Vec::new();

    for entry in std::fs::read_dir(&patches_dir)? {
        let path = entry?.path();
        if path.extension() == Some(OsStr::new("toml")) {
            match load_from_path(&path) {
                Ok(config) if !config.patches.is_empty() => {
                    patches.push((path, config));
                }
                _ => {}
            }
        }
    }

    Ok(patches)
}
