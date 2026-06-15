#!/usr/bin/env bash
# Plan A: local-only dev — SQLite, no Remote/Docker, no cloud login.
set -euo pipefail

cd "$(dirname "$0")/.."

# Strip remote/cloud env even if set in ~/.zshrc or parent shell.
unset VK_SHARED_API_BASE
unset VK_SHARED_RELAY_API_BASE
unset VK_TUNNEL
export VITE_VK_SHARED_API_BASE=

export RUST_LOG="${RUST_LOG:-info}"
export DISABLE_WORKTREE_CLEANUP=1

if ! command -v cargo >/dev/null 2>&1; then
  echo "Rust/cargo not found. Install from https://rustup.rs/ first."
  exit 1
fi

if ! cargo watch --version >/dev/null 2>&1; then
  echo "Installing cargo-watch (one-time)..."
  cargo install cargo-watch
fi

node scripts/apply-local-lite-mode.js

# Invalidate compile-time remote API embedding from any prior build with VK_SHARED_API_BASE set.
touch crates/server/build.rs crates/local-deployment/build.rs

echo "Starting dev (frontend + backend, no cloud login)..."
exec env -u VK_SHARED_API_BASE -u VK_SHARED_RELAY_API_BASE -u VK_TUNNEL \
  VITE_VK_SHARED_API_BASE= \
  pnpm run dev:lite:run
