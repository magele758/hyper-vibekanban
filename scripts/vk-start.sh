#!/usr/bin/env bash
# Start Vibe Kanban: Remote (Docker) + local desktop client + optional Tailscale mobile.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
STATE_DIR="${VK_STATE_DIR:-${HOME}/.vk-kanban}"
LOG_DIR="${STATE_DIR}/logs"
PID_DIR="${STATE_DIR}/pids"
CERT_DIR="${STATE_DIR}/certs"
CADDYFILE="${STATE_DIR}/Caddyfile"

# shellcheck source=vk-dev-lib.sh
source "${ROOT}/scripts/vk-dev-lib.sh"

mkdir -p "${LOG_DIR}" "${PID_DIR}" "${CERT_DIR}"

VK_START_LOCK="${PID_DIR}/vk-start.lock.d"
if ! mkdir "${VK_START_LOCK}" 2>/dev/null; then
  echo "ERROR: 另一个 vk-start 正在运行" >&2
  exit 1
fi
trap 'rm -rf "${VK_START_LOCK}"' EXIT

cd "${ROOT}"

if [[ ! -f crates/remote/.env.remote ]]; then
  echo "Missing crates/remote/.env.remote — copy from crates/remote/README.md"
  exit 1
fi

echo "==> Stopping stale local dev processes..."
vk_stop_local_dev "${ROOT}" "${PID_DIR}"

FRONTEND_PORT="${VK_FRONTEND_PORT}"
BACKEND_PORT="${VK_BACKEND_PORT}"
PREVIEW_PROXY_PORT="${VK_PREVIEW_PROXY_PORT}"
vk_write_ports "${ROOT}" "${FRONTEND_PORT}" "${BACKEND_PORT}" "${PREVIEW_PROXY_PORT}"
export FRONTEND_PORT BACKEND_PORT PREVIEW_PROXY_PORT
unset PORT
node scripts/setup-dev-environment.js get >/dev/null 2>&1 || true

TS_HOSTNAME=""
MOBILE=0
if command -v tailscale >/dev/null 2>&1 && tailscale status >/dev/null 2>&1; then
  TS_HOSTNAME="$(tailscale status --json | python3 -c "import sys,json; print(json.load(sys.stdin)['Self']['DNSName'].rstrip('.'))" 2>/dev/null || true)"
  [[ -n "${TS_HOSTNAME}" ]] && MOBILE=1
fi

echo "==> Starting Remote stack (Docker)..."
if command -v orbctl >/dev/null 2>&1; then
  orbctl config set network_proxy http://host.orb.internal:7897 2>/dev/null || true
fi

cd crates/remote
export DOCKER_BUILDKIT=1
export COMPOSE_DOCKER_CLI_BUILD=1
export REMOTE_DB_PORTS="127.0.0.1:${VK_REMOTE_DB_PORT}:5432"
export REMOTE_SERVER_PORTS="${VK_BIND_ADDR}:${VK_REMOTE_PORT}:8081"
export REMOTE_RELAY_PORTS="${VK_BIND_ADDR}:${VK_RELAY_PORT}:8082"

LAN_IP="$(vk_detect_lan_ip)"
vk_configure_public_urls "${FRONTEND_PORT}"

if [[ "${MOBILE}" -eq 1 ]]; then
  export PUBLIC_BASE_URL="https://${TS_HOSTNAME}:${VK_MOBILE_HTTPS_PORT}"
fi

export VITE_RELAY_API_BASE_URL="${VITE_RELAY_API_BASE_URL:-http://${LAN_IP:-localhost}:${VK_RELAY_PORT}}"

echo "==> 监听 ${VK_BIND_ADDR}（Remote :${VK_REMOTE_PORT}, Relay :${VK_RELAY_PORT}）"

if [[ "${VK_REBUILD:-0}" == "1" ]]; then
  docker compose --env-file .env.remote --profile relay up --build -d --force-recreate
else
  docker compose --env-file .env.remote --profile relay up -d --remove-orphans --force-recreate
fi

remote_ok=0
echo -n "==> Waiting for Remote health"
for _ in $(seq 1 30); do
  if curl -sf "http://127.0.0.1:${VK_REMOTE_PORT}/v1/health" >/dev/null 2>&1 \
    && curl -sf "http://127.0.0.1:${VK_RELAY_PORT}/health" >/dev/null 2>&1; then
    remote_ok=1
    echo " OK"
    break
  fi
  echo -n "."
  sleep 2
done
echo ""
if [[ "${remote_ok}" -ne 1 ]]; then
  echo "ERROR: Remote/Relay 健康检查超时" >&2
  docker compose --env-file .env.remote --profile relay ps
  exit 1
fi
cd "${ROOT}"

CADDY_STARTED=0
if [[ "${MOBILE}" -eq 1 ]]; then
  if ! command -v caddy >/dev/null 2>&1; then
    echo "WARN: Tailscale 已连接但未安装 Caddy — 手机 HTTPS 不可用。运行: brew install caddy"
  else
    if [[ ! -f "${CERT_DIR}/${TS_HOSTNAME}.crt" ]]; then
      echo "==> 生成 Tailscale 证书 (${TS_HOSTNAME})..."
      (cd "${CERT_DIR}" && tailscale cert "${TS_HOSTNAME}")
    fi
    bash scripts/vk-render-caddyfile.sh "${TS_HOSTNAME}" "${FRONTEND_PORT}" "${BACKEND_PORT}" "${VK_REMOTE_PORT}" "${VK_RELAY_PORT}" "${VK_MOBILE_HTTPS_PORT}" "${VK_MOBILE_RELAY_HTTPS_PORT}" "${CADDYFILE}" "${CERT_DIR}"

    if [[ -f "${PID_DIR}/caddy.pid" ]] && kill -0 "$(cat "${PID_DIR}/caddy.pid")" 2>/dev/null; then
      caddy reload --config "${CADDYFILE}" 2>/dev/null || true
    else
      caddy start --config "${CADDYFILE}" --pidfile "${PID_DIR}/caddy.pid" \
        >> "${LOG_DIR}/caddy.log" 2>&1 || echo "WARN: Caddy 启动失败，见 ${LOG_DIR}/caddy.log"
    fi
    if [[ -f "${PID_DIR}/caddy.pid" ]] && kill -0 "$(cat "${PID_DIR}/caddy.pid")" 2>/dev/null; then
      CADDY_STARTED=1
    fi
  fi
fi

# Always use local Docker Remote unless mobile HTTPS front door is up.
if [[ "${MOBILE}" -eq 1 && "${CADDY_STARTED}" -eq 1 ]]; then
  export VITE_VK_SHARED_API_BASE="https://${TS_HOSTNAME}:${VK_MOBILE_HTTPS_PORT}"
  export VK_SHARED_API_BASE="https://${TS_HOSTNAME}:${VK_MOBILE_HTTPS_PORT}"
  export VITE_RELAY_API_BASE_URL="https://${TS_HOSTNAME}:${VK_MOBILE_RELAY_HTTPS_PORT}"
  export VK_SHARED_RELAY_API_BASE="https://${TS_HOSTNAME}:${VK_MOBILE_RELAY_HTTPS_PORT}"
  export VK_ALLOWED_ORIGINS="http://localhost:${FRONTEND_PORT},https://${TS_HOSTNAME}:${VK_MOBILE_HTTPS_PORT}"
fi

# Docker --force-recreate drops in-memory relay tunnels; backend must reconnect.
if vk_dev_running; then
  if vk_local_dev_healthy "${BACKEND_PORT}"; then
    echo "==> 重启 local dev（Remote/Relay 容器已重建，需重连 relay 隧道）..."
    vk_stop_local_dev "${ROOT}" "${PID_DIR}"
  else
    echo "==> 清理异常 dev 进程（backend 未响应）..."
    vk_stop_local_dev "${ROOT}" "${PID_DIR}"
  fi
fi

if vk_local_dev_healthy "${BACKEND_PORT}"; then
  echo "==> dev 已在运行"
  if [[ "${MOBILE}" -eq 1 && "${CADDY_STARTED}" -eq 1 ]]; then
    echo "WARN: 手机 HTTPS 已启用。若刚改过环境变量，请 vk-stop 后重新 vk-start 以重启 dev/backend。"
  fi
else
  echo "==> Starting dev (log: ${LOG_DIR}/dev.log)..."
  : > "${LOG_DIR}/dev.log"
  export FRONTEND_PORT BACKEND_PORT PREVIEW_PROXY_PORT
  unset PORT
  export VK_SHARED_API_BASE VK_SHARED_RELAY_API_BASE VITE_VK_SHARED_API_BASE VK_ALLOWED_ORIGINS
  export VK_DEV_HOST VITE_RELAY_PORT VITE_RELAY_API_BASE_URL
  touch crates/server/build.rs crates/local-deployment/build.rs
  dev_pid="$(vk_launch_dev_background "${ROOT}" "${LOG_DIR}/dev.log")"
  echo "${dev_pid}" > "${PID_DIR}/dev.pid"

  web_ok=0
  api_ok=0
  wait_started=$SECONDS
  echo -n "==> Waiting for local dev"
  for _ in $(seq 1 90); do
    read -r FRONTEND_PORT BACKEND_PORT PREVIEW_PROXY_PORT <<< "$(vk_read_ports "${ROOT}" || echo "${FRONTEND_PORT} ${BACKEND_PORT} ${PREVIEW_PROXY_PORT}")"
    runtime_be="$(vk_read_runtime_backend_port || true)"
    [[ -n "${runtime_be}" ]] && BACKEND_PORT="${runtime_be}"
    vk_http_ok "$(vk_local_url "${FRONTEND_PORT}")" && web_ok=1
    vk_http_ok "http://127.0.0.1:${BACKEND_PORT}/health" && api_ok=1
    if [[ "${web_ok}" -eq 1 && "${api_ok}" -eq 1 ]]; then
      echo " OK"
      break
    fi
    if ! vk_dev_supervisor_alive "${PID_DIR}"; then
      if [[ "${web_ok}" -eq 1 && "${api_ok}" -eq 1 ]]; then
        echo " OK"
        break
      fi
      if (( SECONDS - wait_started < 45 )); then
        echo -n "."
        sleep 2
        continue
      fi
      echo " FAIL"
      echo "ERROR: dev 进程已退出，见 ${LOG_DIR}/dev.log" >&2
      tail -25 "${LOG_DIR}/dev.log" >&2 || true
      exit 1
    fi
    echo -n "."
    sleep 2
  done
  echo ""
  if [[ "${web_ok}" -ne 1 || "${api_ok}" -ne 1 ]]; then
    echo "ERROR: 本地 dev 启动超时 (web=${web_ok} api=${api_ok})，见 ${LOG_DIR}/dev.log" >&2
    tail -20 "${LOG_DIR}/dev.log" >&2 || true
    exit 1
  fi
fi

echo ""
echo "━━━━━━━━ Vibe Kanban 已启动 ━━━━━━━━"
echo "本地 desktop:  http://localhost:${FRONTEND_PORT}"
echo "Remote Web:    http://localhost:${VK_REMOTE_PORT}"
echo "Relay:         http://localhost:${VK_RELAY_PORT}"
echo "登录:          admin@local.dev / devpass123"
if [[ "${MOBILE}" -eq 1 && "${CADDY_STARTED}" -eq 1 ]]; then
  echo "手机/CardComputer (Tailscale + HTTPS):"
  echo "  https://${TS_HOSTNAME}:${VK_MOBILE_HTTPS_PORT}"
  echo "  (Relay: https://${TS_HOSTNAME}:${VK_MOBILE_RELAY_HTTPS_PORT})"
elif [[ -n "${LAN_IP}" ]]; then
  echo "手机 (同一 WiFi):"
  echo "  Kanban:  http://${LAN_IP}:${FRONTEND_PORT}"
  echo "  Remote:  http://${LAN_IP}:${VK_REMOTE_PORT}"
  echo "  Relay:   http://${LAN_IP}:${VK_RELAY_PORT}"
elif [[ "${MOBILE}" -eq 1 ]]; then
  echo "手机: 安装 caddy 后重新 vk-start，或见 mobile-testing.md"
fi
echo "日志: ${LOG_DIR}/"
echo "停止: vk-stop"
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
