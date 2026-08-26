#!/usr/bin/env bash
# Cloud Agent install script for vibe-kanban.
# Idempotent: refreshes JS deps, ensures cargo-watch is present, and warms the
# Rust server build so the first `pnpm run dev` boots quickly.
set -euo pipefail

cd "$(dirname "$0")/.."

# Frontend + workspace JS dependencies (pnpm workspace).
pnpm install --frozen-lockfile

# cargo-watch powers `pnpm run backend:dev:watch`; install only if missing.
if ! command -v cargo-watch >/dev/null 2>&1; then
  cargo install cargo-watch --locked
fi

# Pre-build the backend using the committed SQLx offline cache so no live
# database is required at compile time.
SQLX_OFFLINE=true cargo build --bin server
