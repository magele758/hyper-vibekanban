#!/usr/bin/env bash
# Restart Vite when it exits unexpectedly (OOM, HMR crash, port glitch, etc.).
# concurrently keeps the backend alive; without this wrapper a dead Vite leaves
# the dev stack in a zombie state where vk-status shows OK but :13001 is down.
set -uo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "${ROOT}"

backoff=2
max_backoff=30

while true; do
  echo "[vite-supervisor] $(date -u +%Y-%m-%dT%H:%M:%SZ) starting Vite on port ${FRONTEND_PORT:-3000}"
  pnpm run local-web:dev
  code=$?
  echo "[vite-supervisor] $(date -u +%Y-%m-%dT%H:%M:%SZ) Vite exited with code ${code}, restarting in ${backoff}s" >&2
  sleep "${backoff}"
  if (( backoff < max_backoff )); then
    backoff=$((backoff * 2))
  fi
done
