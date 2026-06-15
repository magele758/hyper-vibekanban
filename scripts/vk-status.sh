#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
STATE_DIR="${VK_STATE_DIR:-${HOME}/.vk-kanban}"
PORTS_FILE="${ROOT}/.dev-ports.json"

# shellcheck source=vk-dev-lib.sh
source "${ROOT}/scripts/vk-dev-lib.sh"

check() {
  local label="$1"
  local url="$2"
  if vk_http_ok "${url}"; then
    echo "  OK  ${label}  ${url}"
  else
    local code
    code="$(curl -s -o /dev/null -w '%{http_code}' --max-time 3 "${url}" 2>/dev/null || echo 000)"
    echo "  --  ${label}  ${url} (${code})"
  fi
}

FE="${VK_FRONTEND_PORT}"
BE="${VK_BACKEND_PORT}"
if [[ -f "${PORTS_FILE}" ]]; then
  FE="$(python3 -c "import json; print(json.load(open('${PORTS_FILE}'))['frontend'])")"
  BE="$(python3 -c "import json; print(json.load(open('${PORTS_FILE}'))['backend'])")"
fi
runtime_be="$(vk_read_runtime_backend_port || true)"
[[ -n "${runtime_be}" ]] && BE="${runtime_be}"

echo "Vibe Kanban status:"
check "Remote" "http://127.0.0.1:${VK_REMOTE_PORT}/v1/health"
check "Relay" "http://127.0.0.1:${VK_RELAY_PORT}/health"
check "Local web" "$(vk_local_url "${FE}")"
check "Local API" "http://127.0.0.1:${BE}/health"

if command -v tailscale >/dev/null 2>&1 && tailscale status >/dev/null 2>&1; then
  TS="$(tailscale status --json | python3 -c "import sys,json; print(json.load(sys.stdin)['Self']['DNSName'].rstrip('.'))" 2>/dev/null || true)"
  if [[ -n "${TS}" ]]; then
    check "Mobile HTTPS" "https://${TS}:${VK_MOBILE_HTTPS_PORT}/"
  fi
fi

if docker ps --format '{{.Names}}' 2>/dev/null | grep -q remote-remote-server; then
  echo "  OK  Docker remote stack"
else
  echo "  --  Docker remote stack"
fi

if vk_dev_running; then
  echo "  OK  local dev (concurrently)"
elif [[ -f "${STATE_DIR}/pids/dev.pid" ]] && kill -0 "$(cat "${STATE_DIR}/pids/dev.pid")" 2>/dev/null; then
  echo "  OK  local dev pid $(cat "${STATE_DIR}/pids/dev.pid")"
else
  echo "  --  local dev not running (Relay 配对 / 打开 workspace 需要本机 13001+13002)"
fi

if vk_http_ok "http://127.0.0.1:${BE}/health"; then
  echo "  OK  Relay client prerequisite (local API up — relay tunnel can register)"
else
  echo "  --  Relay client offline (Remote/手机 会报 spake2/start 404)"
fi

if [[ -f "${STATE_DIR}/pids/caddy.pid" ]] && kill -0 "$(cat "${STATE_DIR}/pids/caddy.pid")" 2>/dev/null; then
  echo "  OK  Caddy pid $(cat "${STATE_DIR}/pids/caddy.pid")"
fi
