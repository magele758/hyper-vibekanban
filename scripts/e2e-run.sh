#!/usr/bin/env bash
# Run E2E against the live vk-start main stack (default ports).
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

BASE_URL="${VK_E2E_BASE_URL:-http://localhost:13001}"
API_BASE="${VK_E2E_API_BASE:-http://127.0.0.1:13002}"

echo "==> Preflight: vk-status (main stack)"
if ! bash scripts/vk-status.sh; then
  echo "ERROR: stack unhealthy. Start with: bash scripts/vk-start.sh" >&2
  exit 1
fi

echo "==> Preflight: HTTP probes"
curl -fsS "$API_BASE/api/health" >/dev/null
curl -fsS -o /dev/null "$BASE_URL/"

export VK_E2E_BASE_URL="$BASE_URL"
export VK_E2E_API_BASE="$API_BASE"
export VK_E2E_REMOTE_BASE="${VK_E2E_REMOTE_BASE:-http://127.0.0.1:13000}"
export VK_E2E_RELAY_BASE="${VK_E2E_RELAY_BASE:-http://127.0.0.1:18082}"

ARGS=("$@")
if [[ ${#ARGS[@]} -eq 0 ]]; then
  ARGS=(--config e2e/playwright.config.ts)
else
  ARGS=(--config e2e/playwright.config.ts "${ARGS[@]}")
fi

echo "==> Playwright against $BASE_URL"
exec pnpm exec playwright test "${ARGS[@]}"
