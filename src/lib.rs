//! CODEX//XTREME - Build Your Perfect Codex
//!
//! An interactive wizard for building optimized, patched Codex binaries.
//! Features both a cliclack-based CLI and a ratatui Neo Tokyo TUI.

pub mod app;
pub mod cpu_detect;
pub mod tui;

// Re-export core for TUI use (separate from main.rs)
pub mod core;
