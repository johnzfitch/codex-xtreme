# Dependency Replacement Guide

## What Gets Replaced

| Original Plan Dep | Lines of Code | Replacement | Lines Needed |
|-------------------|---------------|-------------|--------------|
| `ratatui = "0.29"` | ~500 (rendering) | `cliclack = "0.3"` | ~10 |
| `crossterm = "0.28"` | ~100 (terminal) | (bundled in cliclack) | 0 |
| `tokio = { features = ["full"] }` | ~200 (async) | Not needed | 0 |
| `indicatif = "0.17"` | ~50 (progress) | (bundled in cliclack) | 0 |
| `gix = "0.68"` | ~100 (git ops) | `std::process::Command` | ~80 |
| Manual state machine | ~300 | Sequential flow | 0 |
| 7 screen modules | ~700 | Single main.rs | ~300 |

**Total reduction: ~1950 lines → ~400 lines (80% less code)**

## cliclack Functionality Matrix

| Feature | cliclack function | Example |
|---------|------------------|---------|
| Welcome banner | `intro()` | `intro("🚀 CODEX XTREME")?;` |
| Goodbye banner | `outro()` | `outro("✨ Done!")?;` |
| Single select | `select()` | `select("Pick one").items(&opts).interact()?` |
| Multi select | `multiselect()` | `multiselect("Pick many").items(&opts).interact()?` |
| Yes/No | `confirm()` | `confirm("Continue?").interact()?` |
| Text input | `input()` | `input("Name:").interact()?` |
| Spinner | `spinner()` | `sp.start("Loading..."); sp.stop("Done");` |
| Progress bar | `progress_bar(100)` | `pb.inc(1); pb.set_message("crate");` |
| Multi-progress | `multi_progress()` | Multiple spinners/bars |
| Info message | `log::info()` | `log::info("Hello")?;` |
| Success message | `log::success()` | `log::success("Worked")?;` |
| Warning | `log::warning()` | `log::warning("Careful")?;` |
| Step marker | `log::step()` | `log::step("Step 1")?;` |

## What cliclack Handles Automatically

1. **Terminal raw mode** - Enter/exit automatically
2. **Ctrl-C** - Graceful cleanup (just need handler to exit)
3. **Cursor visibility** - Hidden during prompts, restored after
4. **ANSI escape sequences** - Cross-platform terminal support
5. **Unicode handling** - Works with emoji, box-drawing chars
6. **Theming** - Customizable colors/symbols
7. **Validation** - Built-in input validators

## cargo_metadata vs Manual Parsing

Instead of:
```rust
// Manual JSON line parsing
for line in stdout.lines() {
    if let Ok(json) = serde_json::from_str::<Value>(&line) {
        if json["reason"] == "compiler-artifact" {
            // Extract fields manually
        }
    }
}
```

Use:
```rust
// cargo_metadata does all the work
for message in Message::parse_stream(reader) {
    match message? {
        Message::CompilerArtifact(art) => {
            println!("Building: {}", art.target.name);
        }
        Message::BuildFinished(fin) => {
            if !fin.success { bail!("Build failed"); }
        }
        _ => {}
    }
}
```

## Git: gix vs CLI

**gix (original plan):**
```rust
let repo = gix::discover(".")?;
let remote = repo.find_remote("origin")?;
let refs = remote.fetch(&mut progress)?;
// ... 50+ more lines for cherry-pick
```

**CLI (simplified):**
```rust
Command::new("git")
    .args(["cherry-pick", "--no-commit", sha])
    .current_dir(repo)
    .status()?;
```

**Why CLI is fine here:**
- Git is always installed (it's a dev tool for devs)
- No need for library overhead for simple operations
- Cherry-pick conflict handling is trivial with CLI
- Tag parsing is one `git describe` call

## CPU Detection: raw-cpuid vs rustc

**raw-cpuid approach (complex):**
```rust
let cpuid = CpuId::new();
let vendor = cpuid.get_vendor_info();
let features = cpuid.get_feature_info();
// Map family/model/stepping to znver target
// Requires maintaining a mapping table
```

**rustc approach (simple):**
```rust
let output = Command::new("rustc")
    .args(["--print=target-cpus"])
    .output()?;
// Parse "native - ... (currently znver5)"
```

**Why rustc is better:**
- Rustc already knows the mapping
- Always matches what your build will use
- Zero additional dependencies
- Future-proof (rustc updates mappings)
