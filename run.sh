#!/usr/bin/env bash
# Always runs the latest build of codex-xtreme
# Rebuilds if source changed, then executes

set -e
cd "$(dirname "$0")"

# Build in release mode (fast enough, much faster execution)
cargo build --release --quiet

# Run with all arguments passed through
exec ./target/release/codex-xtreme "$@"
