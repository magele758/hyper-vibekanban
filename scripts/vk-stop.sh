#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
STATE_DIR="${VK_STATE_DIR:-${HOME}/.vk-kanban}"
PID_DIR="${STATE_DIR}/pids"

# shellcheck source=vk-dev-lib.sh
source "${ROOT}/scripts/vk-dev-lib.sh"

vk_stop_local_dev "${ROOT}" "${PID_DIR}"

if [[ -f "${PID_DIR}/caddy.pid" ]]; then
  if command -v caddy >/dev/null 2>&1; then
    caddy stop --pidfile "${PID_DIR}/caddy.pid" 2>/dev/null || true
  fi
  if [[ -f "${PID_DIR}/caddy.pid" ]]; then
    pid="$(cat "${PID_DIR}/caddy.pid")"
    kill "${pid}" 2>/dev/null || true
    rm -f "${PID_DIR}/caddy.pid"
  fi
fi

if [[ "${1:-}" == "--remote" || "${1:-}" == "--all" ]]; then
  cd "${ROOT}/crates/remote"
  if [[ "${1:-}" == "--all" ]]; then
    docker compose --env-file .env.remote --profile relay down -v
  else
    docker compose --env-file .env.remote --profile relay down
  fi
  echo "Remote Docker stack stopped."
else
  echo "Remote Docker 仍在运行 (vk-stop --remote 停止 / vk-stop --all 删数据)"
fi

echo "vk-stop 完成."
