<p align="center">
  <img src=".github/assets/icons/rocket.png" width="64" alt="Codex Xtreme"/>
</p>

<h1 align="center">Codex Xtreme</h1>

<p align="center">
  <strong>Interactive wizard for building optimized, patched Codex binaries</strong>
</p>

<p align="center">
  <a href="https://github.com/johnzfitch/codex-xtreme/actions"><img src="https://img.shields.io/github/actions/workflow/status/johnzfitch/codex-xtreme/ci.yml?branch=master&style=flat-square" alt="CI Status"></a>
  <a href="https://github.com/johnzfitch/codex-patcher"><img src="https://img.shields.io/badge/powered%20by-codex--patcher-blue?style=flat-square" alt="Powered by codex-patcher"></a>
  <a href="LICENSE-MIT"><img src="https://img.shields.io/badge/license-MIT%2FApache--2.0-blue?style=flat-square" alt="License"></a>
</p>

---

## <img src=".github/assets/icons/eye.png" width="16" height="16" alt=""/> Overview

**Codex Xtreme** is a one-command wizard that builds a customized, optimized version of [OpenAI's Codex CLI](https://github.com/openai/codex). It handles everything:

- Detects your CPU and applies optimal compiler flags
- Finds existing Codex repos or clones fresh
- Applies privacy patches (removes telemetry)
- Applies performance patches (optimized builds)
- Builds with LTO, native CPU targeting, and more

### <img src=".github/assets/icons/star.png" width="16" height="16" alt=""/> Key Features

| Feature | Description |
|---------|-------------|
| <img src=".github/assets/icons/lightning.png" width="14" alt=""/> **One Command** | Interactive wizard guides you through everything |
| <img src=".github/assets/icons/shield-security-protection-16x16.png" width="14" alt=""/> **Privacy First** | Removes Statsig telemetry and tracking |
| <img src=".github/assets/icons/rocket.png" width="14" alt=""/> **Optimized Builds** | CPU-native, LTO, mold linker support |
| <img src=".github/assets/icons/layers.png" width="14" alt=""/> **Patch System** | Powered by [codex-patcher](https://github.com/johnzfitch/codex-patcher) |
| <img src=".github/assets/icons/console.png" width="14" alt=""/> **Beautiful CLI** | Clean, interactive UI with progress indicators |

---

## <img src=".github/assets/icons/lightning.png" width="16" height="16" alt=""/> Quick Start

### Installation

```bash
# Clone and build
git clone https://github.com/johnzfitch/codex-xtreme
cd codex-xtreme
cargo install --path .

# Or directly from GitHub
cargo install --git https://github.com/johnzfitch/codex-xtreme
```

### Usage

```bash
# Run the wizard
codex-xtreme

# Developer mode (cherry-pick commits, extra options)
codex-xtreme --dev
```

---

## <img src=".github/assets/icons/magic-wand.png" width="16" height="16" alt=""/> Wizard Flow

The wizard guides you through these phases:

### 1. <img src=".github/assets/icons/search.png" width="14" alt=""/> System Detection
Automatically detects:
- CPU architecture (Zen 5, Intel Alder Lake, Apple Silicon, etc.)
- Mold linker availability
- Rust toolchain version

### 2. <img src=".github/assets/icons/folder.png" width="14" alt=""/> Repository Selection
- Finds existing Codex repos in `~/dev/`
- Option to clone fresh from GitHub
- Shows repo age and branch info

### 3. <img src=".github/assets/icons/tree.png" width="14" alt=""/> Cherry-Pick Commits (Dev Mode)
- Lists commits since latest release
- Multi-select commits to include
- Automatically handles cherry-picking

### 4. <img src=".github/assets/icons/layers.png" width="14" alt=""/> Patch Selection
Choose from available patches:

| Patch | Description | Default |
|-------|-------------|---------|
| `privacy-patches` | Remove Statsig telemetry | On |
| `subagent-limit` | Increase to 8 parallel agents | On |
| `approvals-ui` | Simplified 4-preset approval system | On |
| `cargo-config` | Linux x86_64 build optimizations | Off |

### 5. <img src=".github/assets/icons/gear-24x24.png" width="14" alt=""/> Build Configuration
- Release profile selection (release, zack, etc.)
- CPU target optimization
- LTO and codegen units

### 6. <img src=".github/assets/icons/rocket.png" width="14" alt=""/> Build & Install
- Cargo build with progress streaming
- Automatic installation to `~/.cargo/bin/`
- Summary of what was built

---

## <img src=".github/assets/icons/console.png" width="16" height="16" alt=""/> CLI Reference

```
codex-xtreme - Build your perfect Codex binary

Usage: codex-xtreme [OPTIONS]

Options:
  --dev, -d    Developer mode (cherry-pick commits, extra options)
  --help, -h   Show help message

Environment:
  RUST_LOG=debug    Enable debug logging
```

---

## <img src=".github/assets/icons/shield-security-protection-16x16.png" width="16" height="16" alt=""/> Privacy Patches

The default patches remove:

| Component | What's Removed |
|-----------|----------------|
| **Statsig Telemetry** | All phone-home to `ab.chatgpt.com` |
| **API Keys** | Hardcoded Statsig API keys |
| **Exporter Config** | Telemetry exporter set to `None` |

Your Codex binary will never contact external analytics services.

---

## <img src=".github/assets/icons/rocket.png" width="16" height="16" alt=""/> Performance Optimizations

When built with Codex Xtreme:

| Optimization | Effect |
|--------------|--------|
| **Native CPU** | Uses your exact CPU features |
| **LTO (fat)** | Whole-program optimization |
| **Single codegen unit** | Better optimization opportunities |
| **Mold linker** | 5-10x faster linking (if available) |
| **Panic abort** | Smaller binary, no unwinding |
| **Strip symbols** | Reduced binary size |

Typical results:
- **Build time**: ~2-3 minutes (with mold)
- **Binary size**: ~15-20 MB (stripped)
- **Performance**: Up to 10-15% faster execution

---

## <img src=".github/assets/icons/diagram.png" width="16" height="16" alt=""/> Architecture

```
codex-xtreme
     │
     ├── System Detection
     │   ├── CPU target (rustc_version, cpuid)
     │   ├── Linker (mold detection)
     │   └── Toolchain (rustc version)
     │
     ├── Repository Management
     │   ├── Find existing repos (glob ~/dev/codex*)
     │   ├── Clone from GitHub
     │   └── Git operations (cherry-pick, etc.)
     │
     ├── Patch System (via codex-patcher)
     │   ├── Load patch definitions
     │   ├── Version filtering
     │   └── Apply patches
     │
     └── Build System
         ├── Cargo build with --message-format=json
         ├── Progress streaming
         └── Binary installation
```

---

## <img src=".github/assets/icons/layers.png" width="16" height="16" alt=""/> Dependencies

| Crate | Purpose |
|-------|---------|
| [codex-patcher](https://github.com/johnzfitch/codex-patcher) | Patch application |
| [cliclack](https://crates.io/crates/cliclack) | Interactive CLI UI |
| [cargo_metadata](https://crates.io/crates/cargo_metadata) | Build output parsing |
| [rustc_version](https://crates.io/crates/rustc_version) | Rust version detection |

---

## <img src=".github/assets/icons/toolbox.png" width="16" height="16" alt=""/> Development

### Building

```bash
# Debug build
cargo build

# Release build
cargo build --release

# Run tests
cargo test

# Run clippy
cargo clippy --all-targets -- -D warnings
```

### Project Structure

```
codex-xtreme/
├── src/
│   └── main.rs          # Complete wizard implementation
├── Cargo.toml           # Dependencies
├── docs/
│   └── patches.md       # Available patches documentation
└── .github/
    └── workflows/ci.yml # CI pipeline
```

---

## <img src=".github/assets/icons/globe.png" width="16" height="16" alt=""/> Contributing

Contributions welcome! See [CONTRIBUTING.md](.github/CONTRIBUTING.md).

### Adding New Patches

1. Create patch definition in [codex-patcher](https://github.com/johnzfitch/codex-patcher)
2. Add patch to the wizard's patch list in `main.rs`
3. Test with `codex-xtreme --dev`

---

## <img src=".github/assets/icons/key.png" width="16" height="16" alt=""/> License

Licensed under either of:
- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE))
- MIT License ([LICENSE-MIT](LICENSE-MIT))

at your option.

---

## <img src=".github/assets/icons/star.png" width="16" height="16" alt=""/> Related Projects

- [codex-patcher](https://github.com/johnzfitch/codex-patcher) - The patching engine
- [OpenAI Codex](https://github.com/openai/codex) - The upstream CLI

---

<p align="center">
  <sub>Build your perfect Codex, your way.</sub>
</p>
