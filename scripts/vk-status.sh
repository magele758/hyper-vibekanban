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

if [[ "${VK_DESKTOP_H2:-1}" == "1" ]]; then
  h2_url="https://localhost:${VK_DESKTOP_HTTPS_PORT}/"
  h2_code="$(curl -sk -o /dev/null -w '%{http_code}' --max-time 3 "${h2_url}" 2>/dev/null || echo 000)"
  h2_proto="$(curl -sk -o /dev/null -w '%{http_version}' --max-time 3 "${h2_url}" 2>/dev/null || echo '?')"
  if [[ "${h2_code}" =~ ^[23][0-9]{2}$ ]]; then
    echo "  OK  Desktop h2  ${h2_url} (HTTP/${h2_proto})"
  else
    echo "  --  Desktop h2  ${h2_url} (${h2_code})"
  fi

  relay_url="https://localhost:${VK_DESKTOP_RELAY_HTTPS_PORT}/health"
  relay_code="$(curl -sk -o /dev/null -w '%{http_code}' --max-time 3 "${relay_url}" 2>/dev/null || echo 000)"
  relay_proto="$(curl -sk -o /dev/null -w '%{http_version}' --max-time 3 "${relay_url}" 2>/dev/null || echo '?')"
  if [[ "${relay_code}" =~ ^[23][0-9]{2}$ ]]; then
    echo "  OK  Desktop relay h2  ${relay_url} (HTTP/${relay_proto})"
  else
    echo "  --  Desktop relay h2  ${relay_url} (${relay_code})"
  fi
fi

if [[ "${VK_MOBILE:-0}" == "1" ]] && vk_tailscale_ok; then
  TS="$(vk_detect_tailscale_hostname)"
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
