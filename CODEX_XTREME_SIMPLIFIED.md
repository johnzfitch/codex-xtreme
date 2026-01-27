# codex-xtreme: Simplified Wizard Architecture

## The Problem with the Original Plan

The ratatui-based TUI requires:
- 7+ screen modules with state machines
- Complex terminal setup/teardown  
- Manual keyboard handling
- Async rendering coordination
- ~2000+ lines of UI boilerplate

**For a linear wizard flow, this is massive overkill.**

---

## The Solution: `cliclack` Wizard

**One file. ~300 lines. Same UX.**

```rust
// src/bin/codex_xtreme.rs
use cliclack::{intro, outro, select, multiselect, spinner, progress_bar, confirm, log};
use cargo_metadata::Message;
use std::process::{Command, Stdio};

fn main() -> anyhow::Result<()> {
    intro("🚀 CODEX XTREME")?;

    // ═══════════════════════════════════════════════════════════
    // PHASE 1: System Detection (no interaction needed)
    // ═══════════════════════════════════════════════════════════
    let sp = spinner();
    sp.start("Detecting system...");
    
    let cpu_target = detect_cpu_target();  // ~20 lines
    let has_mold = which::which("mold").is_ok();
    let rust_ver = rustc_version::version()?;
    
    sp.stop(format!("✓ {} | mold: {} | rustc {}", 
        cpu_target, 
        if has_mold { "yes" } else { "no" },
        rust_ver
    ));

    // ═══════════════════════════════════════════════════════════
    // PHASE 2: Repository Selection
    // ═══════════════════════════════════════════════════════════
    let repos = find_codex_repos()?;  // ~30 lines: glob + git status
    let repo_options: Vec<_> = repos.iter()
        .map(|r| (r.path.display().to_string(), format!("{} ({})", r.path.display(), r.age)))
        .chain(std::iter::once(("clone".into(), "Clone fresh".into())))
        .collect();

    let selected_repo: String = select("Select Codex repository")
        .items(&repo_options)
        .interact()?;

    let repo_path = if selected_repo == "clone" {
        clone_codex()?  // ~15 lines: git clone with spinner
    } else {
        PathBuf::from(selected_repo)
    };

    // ═══════════════════════════════════════════════════════════
    // PHASE 3: Commit Cherry-picking
    // ═══════════════════════════════════════════════════════════
    let latest_tag = get_latest_release_tag(&repo_path)?;
    let commits = get_commits_since(&repo_path, &latest_tag)?;  // ~20 lines: git log --oneline
    
    if !commits.is_empty() {
        let commit_options: Vec<_> = commits.iter()
            .map(|c| (c.sha.clone(), format!("{} - {}", &c.sha[..7], c.message), true))
            .collect();

        let selected_commits: Vec<String> = multiselect("Cherry-pick commits beyond release")
            .items(&commit_options)
            .interact()?;

        if !selected_commits.is_empty() {
            cherry_pick_commits(&repo_path, &selected_commits)?;  // ~10 lines
        }
    }

    // ═══════════════════════════════════════════════════════════
    // PHASE 4: Patch Selection
    // ═══════════════════════════════════════════════════════════
    let patches = vec![
        ("privacy-patches", "Remove Statsig telemetry", true),
        ("subagent-limit", "Increase to 8 threads", true),
        ("approvals-ui", "Simplified 4-preset system", true),
        ("cargo-config", "Linux x86_64 optimizations", false),
    ];

    let selected_patches: Vec<&str> = multiselect("Select patches to apply")
        .items(&patches)
        .interact()?;

    // ═══════════════════════════════════════════════════════════
    // PHASE 5: Build Configuration
    // ═══════════════════════════════════════════════════════════
    let profile = select("Build profile")
        .item("xtreme", "Xtreme (LTO=fat, codegen-units=1)", "Slowest build, fastest binary")
        .item("release", "Release (standard)", "Fast build")
        .interact()?;

    let use_cpu_opt = confirm(format!("Use target-cpu={}?", cpu_target))
        .initial_value(true)
        .interact()?;

    // ═══════════════════════════════════════════════════════════
    // PHASE 6: Apply Patches & Build
    // ═══════════════════════════════════════════════════════════
    apply_patches(&repo_path, &selected_patches)?;  // Call existing codex-patcher lib
    inject_xtreme_profile(&repo_path)?;  // ~20 lines: append to Cargo.toml

    // Build with progress
    let pb = progress_bar(100);
    pb.start("Building codex...");
    
    let binary_path = run_cargo_build(&repo_path, profile, use_cpu_opt, |progress| {
        pb.set_position(progress.percent);
        pb.set_message(progress.current_crate);
    })?;
    
    pb.stop("Build complete!");

    // ═══════════════════════════════════════════════════════════
    // PHASE 7: Test & Finish
    // ═══════════════════════════════════════════════════════════
    if confirm("Run verification tests?").initial_value(true).interact()? {
        run_tests(&repo_path)?;  // ~30 lines: cargo test with multi-spinner
    }

    if confirm("Set up alias?").initial_value(true).interact()? {
        setup_alias(&binary_path)?;  // ~20 lines
    }

    outro(format!("✨ Done! Binary at: {}", binary_path.display()))?;
    Ok(())
}
```

---

## Side-by-Side Comparison

| Aspect | Original (ratatui) | Simplified (cliclack) |
|--------|-------------------|----------------------|
| **Lines of code** | ~2000+ | ~300 |
| **Files** | 12+ | 1-2 |
| **Dependencies** | ratatui, crossterm, tokio (async) | cliclack, cargo_metadata |
| **State machine** | Required | Not needed |
| **Keyboard handling** | Manual | Built-in |
| **Progress bars** | Manual indicatif | Built-in |
| **Multi-select** | Manual render | One function call |
| **Terminal cleanup** | Manual | Automatic |
| **Error recovery** | Complex | Try/catch per phase |
| **Time to implement** | 2-3 days | 2-3 hours |

---

## Minimal Dependencies

```toml
[dependencies]
cliclack = "0.3"           # All UI (select, multiselect, spinner, progress)
cargo_metadata = "0.18"    # Parse cargo JSON output
anyhow = "1"               # Error handling
which = "6"                # Check for mold
rustc_version = "0.4"      # Get rustc version

# Existing deps from codex-patcher
# ... (patch application logic)
```

**Total new deps: 4 crates** vs original plan's 8+

---

## Helper Functions (the actual work)

### Key Architecture Notes

**The Codex repo structure:**
```
codex/                    # Git repo root
├── codex-rs/            # Rust workspace (this is where we build!)
│   ├── Cargo.toml       # Workspace manifest
│   ├── cli/             # codex-cli binary
│   ├── core/
│   ├── otel/            # Where Statsig lives
│   └── ...
└── sdk/                 # TypeScript SDK (ignored)
```

The `RepoInfo` struct tracks the repo root, but all build operations happen in `repo.workspace_path()` which returns `{repo}/codex-rs/`.

### CPU Detection (~20 lines)
```rust
fn detect_cpu_target() -> String {
    // Method 1: Ask rustc what native means
    let output = Command::new("rustc")
        .args(["--print=target-cpus"])
        .output()
        .ok();
    
    if let Some(out) = output {
        let stdout = String::from_utf8_lossy(&out.stdout);
        for line in stdout.lines() {
            if line.contains("native") && line.contains("currently") {
                // "native - Select the CPU of the current host (currently znver5)."
                if let Some(cpu) = line.split("currently ").nth(1) {
                    return cpu.trim_end_matches(").").to_string();
                }
            }
        }
    }
    
    // Fallback: parse /proc/cpuinfo for AMD family
    if let Ok(cpuinfo) = std::fs::read_to_string("/proc/cpuinfo") {
        if cpuinfo.contains("AuthenticAMD") {
            if cpuinfo.contains("cpu family\t: 26") { return "znver5".into(); }
            if cpuinfo.contains("cpu family\t: 25") { return "znver4".into(); }
            if cpuinfo.contains("cpu family\t: 23") { return "znver2".into(); }
        }
    }
    
    "native".into()
}
```

### Cargo Build with Progress (~40 lines)
```rust
fn run_cargo_build(
    repo: &Path, 
    profile: &str, 
    cpu_opt: bool,
    on_progress: impl Fn(BuildProgress)
) -> anyhow::Result<PathBuf> {
    let mut cmd = Command::new("cargo");
    cmd.current_dir(repo)
        .args(["build", "--profile", profile, "-p", "codex-cli", "--message-format=json"])  // Note: package is codex-cli
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit());
    
    if cpu_opt {
        cmd.env("RUSTFLAGS", format!("-C target-cpu={}", detect_cpu_target()));
    }
    
    let mut child = cmd.spawn()?;
    let reader = std::io::BufReader::new(child.stdout.take().unwrap());
    
    let mut artifacts_seen = 0;
    let estimated_total = 150; // approximate crate count
    
    for message in Message::parse_stream(reader) {
        match message? {
            Message::CompilerArtifact(art) => {
                artifacts_seen += 1;
                on_progress(BuildProgress {
                    percent: (artifacts_seen * 100 / estimated_total).min(99),
                    current_crate: art.target.name,
                });
            }
            Message::BuildFinished(fin) => {
                if !fin.success {
                    anyhow::bail!("Build failed");
                }
            }
            _ => {}
        }
    }
    
    child.wait()?;
    Ok(repo.join("target").join(profile).join("codex"))
}
```

### Git Operations (~50 lines total)
```rust
fn find_codex_repos() -> anyhow::Result<Vec<RepoInfo>> {
    let candidates = ["~/dev/codex", "~/codex", "~/src/codex", "~/.local/src/codex"];
    // ... glob and check for Cargo.toml with codex workspace
}

fn get_latest_release_tag(repo: &Path) -> anyhow::Result<String> {
    let output = Command::new("git")
        .current_dir(repo)
        .args(["describe", "--tags", "--abbrev=0", "--match", "rust-v*"])
        .output()?;
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

fn get_commits_since(repo: &Path, tag: &str) -> anyhow::Result<Vec<Commit>> {
    let output = Command::new("git")
        .current_dir(repo)
        .args(["log", "--oneline", &format!("{}..HEAD", tag)])
        .output()?;
    // Parse "abc1234 commit message" lines
}

fn cherry_pick_commits(repo: &Path, shas: &[String]) -> anyhow::Result<()> {
    for sha in shas {
        Command::new("git")
            .current_dir(repo)
            .args(["cherry-pick", "--no-commit", sha])
            .status()?;
    }
    Ok(())
}
```

---

## What You Keep from the Original Plan

1. **Build profile definition** - Same TOML, just inject it
2. **Patch system** - Use existing codex-patcher lib functions
3. **Test verification strategy** - Same test commands
4. **Alias setup** - Same shell detection + sed logic

---

## What Gets Deleted

1. ❌ `app.rs` - No state machine needed
2. ❌ `ui.rs` - cliclack handles rendering  
3. ❌ `screens/` - All 7 screen modules gone
4. ❌ Terminal setup/teardown code
5. ❌ Keyboard shortcut handling
6. ❌ Async task coordination
7. ❌ ratatui, crossterm deps

---

## Implementation Path

**Phase 1 (30 min):** Scaffold + system detection
```bash
cargo new codex-xtreme
# Add deps, write detect_cpu_target(), has_mold check
```

**Phase 2 (30 min):** Repository selection flow
```bash
# find_codex_repos(), clone option, git fetch
```

**Phase 3 (30 min):** Commit + patch selection
```bash  
# git log parsing, multiselect UI, cherry-pick
```

**Phase 4 (1 hour):** Build system
```bash
# Profile injection, cargo JSON parsing, progress
```

**Phase 5 (30 min):** Test + finish
```bash
# cargo test wrapper, alias setup, outro
```

**Total: ~3 hours** vs 2-3 days for full TUI

---

## Optional Enhancements (if you want the polish later)

1. **Custom cliclack theme** - Match the ASCII art aesthetic
2. **Config file** - Save preferences for next run
3. **`--non-interactive` flag** - For CI/scripting
4. **Update checker** - Compare local tag vs remote

---

## The Bottom Line

The original plan is a *TUI toolkit* disguised as a build tool.

`cliclack` gives you:
- ✅ Same visual quality
- ✅ Same workflow
- ✅ 90% less code
- ✅ Zero async complexity
- ✅ Automatic Ctrl-C handling
- ✅ Cross-platform terminal support

**Ship the wizard, iterate from there.**
