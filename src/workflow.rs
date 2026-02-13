//! Shared build workflow used by both the CLI wizard and the TUI.
//!
//! Goal: keep behavior identical across frontends; only presentation differs.

use anyhow::{bail, Context, Result};
use cargo_metadata::Message;
use codex_patcher::{
    apply_patches as patcher_apply,
    compiler::{try_autofix_all, CompileDiagnostic},
    load_from_path, Edit, PatchConfig, PatchResult,
};
use std::ffi::OsStr;
use std::io::BufRead;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

/// The package name (for cargo -p)
pub const CODEX_PACKAGE: &str = "codex-cli";

/// The binary name (output file)
pub const CODEX_BINARY: &str = "codex";

/// High-level build phase for UI progress reporting.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase {
    Patching,
    Compiling,
    Optimizing,
    Testing,
    Installing,
}

/// Optimization intent: a single selector that maps to concrete knobs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OptimizationMode {
    /// Prefer faster *builds* (link with mold). No runtime BOLT pass.
    BuildFast,
    /// Prefer faster *runtime* (BOLT). Disables mold (perf2bolt incompatibility).
    RunFast,
    /// Let the user pick; we still enforce BOLT => no mold on x86_64.
    Custom,
}

#[derive(Clone, Debug)]
pub struct OptimizationFlags {
    pub use_mold: bool,
    pub use_bolt: bool,
}

impl OptimizationFlags {
    pub fn from_mode(mode: OptimizationMode, has_mold: bool, has_bolt: bool) -> Self {
        match mode {
            OptimizationMode::BuildFast => Self {
                use_mold: has_mold,
                use_bolt: false,
            },
            OptimizationMode::RunFast => Self {
                use_mold: false,
                use_bolt: has_bolt,
            },
            OptimizationMode::Custom => Self {
                use_mold: has_mold,
                use_bolt: has_bolt,
            },
        }
    }

    pub fn enforce_invariants(&mut self) {
        // perf2bolt currently fails on mold-linked x86_64 binaries due to PLT layout.
        if self.use_bolt {
            self.use_mold = false;
        }
    }
}

#[derive(Clone, Debug)]
pub struct BuildOptions {
    pub profile: String, // "xtreme" or "release"
    pub cpu_target: Option<String>,
    pub optimization: OptimizationFlags,
    pub strip_symbols: bool,
    /// Optional throttle for cargo parallelism (`cargo --jobs N`).
    /// This limits rustc processes spawned concurrently, which reduces peak CPU usage.
    pub cargo_jobs: Option<usize>,
}

/// Emitted events allow the frontend to keep the user informed without
/// hardcoding output formatting into the workflow.
#[derive(Clone, Debug)]
pub enum Event {
    Phase(Phase),
    Progress(f64),
    CurrentItem(String),
    Log(String),
    PatchFileApplied(String),
    PatchFileSkipped { name: String, reason: String },
}

fn resolve_command_path(name: &str) -> Result<PathBuf> {
    which::which(name).map_err(|_| anyhow::anyhow!("Required command not found in PATH: {name}"))
}

/// Read the workspace version from Cargo.toml.
pub fn read_workspace_version(workspace: &Path) -> Result<String> {
    let cargo_toml = workspace.join("Cargo.toml");
    let content =
        std::fs::read_to_string(&cargo_toml).context("Failed to read workspace Cargo.toml")?;

    // Look for version = "x.y.z" in [workspace.package] or top-level.
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

    // Fallback: try to get from git tag.
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
        if let Some(version) = tag.strip_prefix("rust-v").or_else(|| tag.strip_prefix("v")) {
            return Ok(version.to_string());
        }
        if !tag.is_empty() {
            return Ok(tag);
        }
    }

    Ok("0.0.0".to_string())
}

/// Find and load all available patch configs from a patches directory.
pub fn get_available_patches(patches_dir: &Path) -> Result<Vec<(PathBuf, PatchConfig)>> {
    let mut patches = Vec::new();

    for entry in std::fs::read_dir(patches_dir)? {
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

    patches.sort_by(|a, b| a.1.meta.name.cmp(&b.1.meta.name));
    Ok(patches)
}

/// Apply selected patches using codex-patcher library.
pub fn apply_patches(
    workspace: &Path,
    selected_files: &[PathBuf],
    mut emit: impl FnMut(Event),
) -> Result<()> {
    emit(Event::Phase(Phase::Patching));
    emit(Event::Progress(0.0));
    let workspace_version = read_workspace_version(workspace)?;

    for (idx, patch_file) in selected_files.iter().enumerate() {
        let patch_file_name = patch_file
            .file_stem()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_else(|| patch_file.display().to_string());
        emit(Event::CurrentItem(format!(
            "Applying patch file {}/{}: {}",
            idx + 1,
            selected_files.len(),
            patch_file_name
        )));

        let config = load_from_path(patch_file)
            .with_context(|| format!("Failed to load patch: {}", patch_file.display()))?;
        // Defensive: patch application is user-extensible and has historically had panics.
        let results = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            patcher_apply(&config, workspace, &workspace_version)
        }))
        .map_err(|panic_info| {
            let msg = if let Some(s) = panic_info.downcast_ref::<&str>() {
                s.to_string()
            } else if let Some(s) = panic_info.downcast_ref::<String>() {
                s.clone()
            } else {
                "Unknown panic".to_string()
            };
            anyhow::anyhow!("Patch application panicked: {}", msg)
        })?;

        let mut applied_count = 0usize;
        let mut skipped_count = 0usize;
        let mut first_skip_reason: Option<String> = None;

        for (patch_id, result) in results {
            match result {
                Ok(PatchResult::Applied { file }) => {
                    emit(Event::Log(format!(
                        "  ✓ Applied {}: {}",
                        patch_id,
                        file.display()
                    )));
                    applied_count += 1;
                }
                Ok(PatchResult::AlreadyApplied { file }) => {
                    emit(Event::Log(format!(
                        "  ○ Already applied {}: {}",
                        patch_id,
                        file.display()
                    )));
                    applied_count += 1;
                }
                Ok(PatchResult::SkippedVersion { reason }) => {
                    emit(Event::Log(format!("  ⊘ Skipped {}: {}", patch_id, reason)));
                    skipped_count += 1;
                    first_skip_reason.get_or_insert(reason);
                }
                Ok(PatchResult::Failed { file, reason }) => {
                    emit(Event::Log(format!(
                        "  ✗ Failed {}: {} - {}",
                        patch_id,
                        file.display(),
                        reason
                    )));
                    skipped_count += 1;
                    first_skip_reason.get_or_insert(reason);
                }
                Err(e) => {
                    emit(Event::Log(format!(
                        "  ✗ Error applying {}: {}",
                        patch_id, e
                    )));
                    skipped_count += 1;
                    first_skip_reason.get_or_insert(e.to_string());
                }
            }
        }

        if applied_count > 0 {
            emit(Event::PatchFileApplied(patch_file_name.clone()));
        } else if skipped_count > 0 {
            emit(Event::PatchFileSkipped {
                name: patch_file_name.clone(),
                reason: first_skip_reason.unwrap_or_else(|| "skipped".to_string()),
            });
        }

        emit(Event::Progress(
            (idx + 1) as f64 / selected_files.len().max(1) as f64,
        ));
    }

    Ok(())
}

pub fn inject_xtreme_profile(workspace: &Path) -> Result<()> {
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
    Ok(())
}

/// Build error with captured diagnostics for auto-fix.
#[derive(Debug)]
pub enum BuildError {
    CompileError { diagnostics: Vec<CompileDiagnostic> },
    Other(anyhow::Error),
}

/// Build with automatic fix loop for compiler errors.
pub fn build_with_autofix(
    workspace: &Path,
    profile: &str,
    cpu_target: Option<&str>,
    optimization: &OptimizationFlags,
    cargo_jobs: Option<usize>,
    mut emit: impl FnMut(Event),
) -> Result<PathBuf> {
    const MAX_FIX_ATTEMPTS: usize = 5;

    for attempt in 1..=MAX_FIX_ATTEMPTS {
        match run_cargo_build(workspace, profile, cpu_target, optimization, cargo_jobs, |msg| {
            emit(Event::CurrentItem(msg))
        }) {
            Ok(path) => return Ok(path),
            Err(BuildError::Other(e)) => return Err(e),
            Err(BuildError::CompileError { diagnostics }) => {
                emit(Event::Log(format!(
                    "Build failed (attempt {}/{}), trying auto-fixes...",
                    attempt, MAX_FIX_ATTEMPTS
                )));

                let (edits, unfixable) = try_autofix_all(&diagnostics, workspace);
                if edits.is_empty() {
                    let mut msg = format!(
                        "Build failed with {} unfixable error(s).",
                        unfixable.len().max(1)
                    );
                    // Show a small amount of context to keep CLI/TUI useful without dumping huge logs.
                    for (i, diag) in unfixable.iter().take(2).enumerate() {
                        msg.push_str(&format!("\n\n--- error {} ---\n", i + 1));
                        if let Some(rendered) = &diag.rendered {
                            msg.push_str(rendered);
                        } else {
                            msg.push_str(&diag.message);
                        }
                    }
                    bail!(msg);
                }

                // Apply all edits
                Edit::apply_batch(edits).context("Failed to apply auto-fixes")?;
                if !unfixable.is_empty() {
                    emit(Event::Log(format!(
                        "{} error(s) could not be auto-fixed; retrying build anyway",
                        unfixable.len()
                    )));
                }
            }
        }
    }

    bail!("Build failed after {MAX_FIX_ATTEMPTS} auto-fix attempts.")
}

fn run_cargo_build(
    workspace: &Path,
    profile: &str,
    cpu_target: Option<&str>,
    optimization: &OptimizationFlags,
    cargo_jobs: Option<usize>,
    mut on_current_item: impl FnMut(String),
) -> std::result::Result<PathBuf, BuildError> {
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
        // Avoid leaking raw cargo output into the TUI. Diagnostics are captured from JSON.
        .stderr(Stdio::null());

    if let Some(jobs) = cargo_jobs {
        cmd.arg("--jobs").arg(jobs.to_string());
    }

    let mut rustflags = Vec::new();
    if let Some(cpu) = cpu_target {
        rustflags.push(format!("-C target-cpu={}", cpu));
    }
    if optimization.use_mold {
        rustflags.push("-C link-arg=-fuse-ld=mold".into());
    }
    if optimization.use_bolt {
        // Required for BOLT to rewrite the binary.
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
                "Failed to capture cargo stdout"
            )))
        }
    };
    let reader = std::io::BufReader::new(stdout);

    let mut artifact_count = 0;
    let mut binary_path: Option<PathBuf> = None;
    let mut compiler_errors: Vec<cargo_metadata::diagnostic::Diagnostic> = Vec::new();

    for line in reader.lines() {
        let line = match line {
            Ok(l) => l,
            Err(e) => return Err(BuildError::Other(e.into())),
        };
        if !line.starts_with('{') {
            continue;
        }
        if let Ok(message) = serde_json::from_str::<Message>(&line) {
            match message {
                Message::CompilerArtifact(art) => {
                    artifact_count += 1;
                    on_current_item(format!("[{}] {}", artifact_count, art.target.name));

                    if art.target.name == CODEX_BINARY {
                        for path in &art.filenames {
                            let p = PathBuf::from(path);
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
                    if matches!(
                        msg.message.level,
                        cargo_metadata::diagnostic::DiagnosticLevel::Error
                    ) {
                        compiler_errors.push(msg.message);
                    }
                }
                Message::BuildFinished(fin) => {
                    if !fin.success {
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
        let diagnostics: Vec<CompileDiagnostic> = compiler_errors
            .iter()
            .map(|e| CompileDiagnostic::from_cargo(e, workspace))
            .collect();
        return Err(BuildError::CompileError { diagnostics });
    }

    if let Some(path) = binary_path {
        return Ok(path);
    }

    // Fallback: construct expected path.
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

/// Run BOLT optimization on a binary.
pub fn run_bolt_optimization(binary_path: &Path, mut emit: impl FnMut(Event)) -> Result<PathBuf> {
    emit(Event::Phase(Phase::Optimizing));
    let binary_dir = binary_path.parent().context("Binary has no parent dir")?;
    let binary_name = binary_path.file_name().context("Binary has no filename")?;
    let bolted_binary = binary_dir.join(format!("{}-bolt", binary_name.to_string_lossy()));
    let perf_data = binary_dir.join("perf.data");
    let bolt_profile = binary_dir.join("perf.fdata");
    let mut use_lbr = true;

    let perf_path = resolve_command_path("perf").context("perf is required for BOLT")?;
    let perf2bolt_path = resolve_command_path("perf2bolt").context("perf2bolt is required")?;
    let bolt_path = resolve_command_path("llvm-bolt").context("llvm-bolt is required")?;

    emit(Event::CurrentItem(
        "Profiling binary with perf LBR (run some typical commands)...".to_string(),
    ));

    let perf_output = Command::new(&perf_path)
        .args([
            "record",
            "-e",
            "cycles:u",
            "-j",
            "any,u",
            "-o",
            perf_data.to_str().unwrap(),
            "--",
        ])
        .arg(binary_path)
        .args(["--version"])
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .output();

    let perf_failed = match &perf_output {
        Ok(output) => !output.status.success(),
        Err(_) => true,
    };
    if perf_failed {
        use_lbr = false;
        emit(Event::Log(
            "perf LBR record failed; falling back to non-LBR profiling".to_string(),
        ));

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

    emit(Event::CurrentItem(
        "Converting perf profile (perf2bolt)...".to_string(),
    ));
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
        if stderr.contains("unable to disassemble instruction in PLT section .plt at offset 0x10") {
            bail!(
                "perf2bolt conversion failed: {} (known issue with mold-linked binaries; rebuild without mold to use BOLT)",
                stderr
            );
        }
        if stderr.is_empty() {
            bail!("perf2bolt conversion failed: {}", perf2bolt_output.status);
        }
        bail!("perf2bolt conversion failed: {}", stderr);
    }

    emit(Event::CurrentItem(
        "Optimizing with llvm-bolt...".to_string(),
    ));
    let temp_output = binary_dir.join(format!("{}.bolt.tmp", binary_name.to_string_lossy()));
    let bolt_output = Command::new(bolt_path)
        .arg(binary_path)
        .args(["-o", temp_output.to_str().unwrap()])
        .args(["-data", bolt_profile.to_str().unwrap()])
        .args([
            "-reorder-blocks=ext-tsp",
            "-reorder-functions=cdsort",
            "-split-functions",
            "-split-all-cold",
            "-dyno-stats",
        ])
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .output()
        .context("llvm-bolt failed")?;

    if !bolt_output.status.success() {
        std::fs::remove_file(&temp_output).ok();
        let stderr = String::from_utf8_lossy(&bolt_output.stderr);
        let stderr = stderr.trim();
        if stderr.is_empty() {
            bail!("llvm-bolt optimization failed: {}", bolt_output.status);
        }
        bail!("llvm-bolt optimization failed: {}", stderr);
    }

    std::fs::rename(&temp_output, &bolted_binary)
        .context("Failed to rename BOLT output to final location")?;
    std::fs::remove_file(&perf_data).ok();
    std::fs::remove_file(&bolt_profile).ok();
    Ok(bolted_binary)
}

pub fn strip_binary(binary_path: &Path) -> Result<()> {
    // Prefer llvm-strip if present, otherwise fall back to GNU strip.
    let strip = which::which("llvm-strip").or_else(|_| which::which("strip"))?;
    let status = Command::new(strip)
        .arg("--strip-all")
        .arg(binary_path)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .context("strip failed to spawn")?;
    if !status.success() {
        bail!("strip failed: {status}");
    }
    Ok(())
}

pub fn run_verification_tests(
    workspace: &Path,
    cargo_jobs: Option<usize>,
    mut emit: impl FnMut(Event),
) -> Result<()> {
    emit(Event::Phase(Phase::Testing));
    let tests = [
        ("cargo check", vec!["check", "--all"]),
        (
            "codex-common tests",
            vec!["test", "-p", "codex-common", "--lib"],
        ),
    ];

    for (name, args) in tests {
        emit(Event::CurrentItem(format!("Running {}...", name)));
        let mut cmd = Command::new(resolve_command_path("cargo")?);
        cmd.current_dir(workspace).args(&args);
        if let Some(jobs) = cargo_jobs {
            cmd.arg("--jobs").arg(jobs.to_string());
        }
        let status = cmd
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()?;

        if status.success() {
            emit(Event::Log(format!("  ✓ {}", name)));
        } else {
            emit(Event::Log(format!("  ✗ {} (failed)", name)));
        }
    }

    Ok(())
}

pub fn setup_alias(binary_path: &Path) -> Result<Option<String>> {
    let shell = std::env::var("SHELL").unwrap_or_default();

    let rc_file = if shell.contains("zsh") {
        shellexpand::tilde("~/.zshrc").to_string()
    } else if shell.contains("fish") {
        return Ok(None);
    } else {
        shellexpand::tilde("~/.bashrc").to_string()
    };

    let alias_line = format!("alias codex=\"{}\"", binary_path.display());

    if let Ok(contents) = std::fs::read_to_string(&rc_file) {
        if contents.contains("alias codex=") {
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

    Ok(Some(rc_file))
}
