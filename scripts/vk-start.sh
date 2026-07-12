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
vk_configure_asset_dir "${ROOT}"

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
TS_IP=""
MOBILE=0
# Tailscale IP/hostname are detected whenever Tailscale is up (CORS, browser
# shared_api_base, optional HTTPS front door). VK_MOBILE=1 additionally points
# VITE_* build-time bases at the Tailscale HTTPS front door.
if vk_tailscale_ok; then
  TS_IP="$(vk_detect_tailscale_ip)"
  TS_HOSTNAME="$(vk_detect_tailscale_hostname)"
  if [[ "${VK_MOBILE}" == "1" ]]; then
    if [[ -n "${TS_HOSTNAME}" ]]; then
      if [[ "${VK_MOBILE_HTTPS_PORT}" == "${FRONTEND_PORT}" ]]; then
        echo "WARN: VK_MOBILE_HTTPS_PORT(${VK_MOBILE_HTTPS_PORT}) 与前端口相同会冲突，已跳过手机前门 (设不同端口再开)"
        TS_HOSTNAME=""
      else
        MOBILE=1
      fi
    fi
  fi
fi

echo "==> Starting Remote stack (Docker)..."
vk_configure_orbstack_proxy

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
DESKTOP_H2_UP=0
TS_CADDY_HOSTNAME=""
WANT_DESKTOP_H2=0
[[ "${VK_DESKTOP_H2:-1}" == "1" ]] && WANT_DESKTOP_H2=1

if [[ "${MOBILE}" -eq 1 || "${WANT_DESKTOP_H2}" -eq 1 ]]; then
  if ! command -v caddy >/dev/null 2>&1; then
    echo "WARN: 需要 Caddy 但未安装 — h2 前门不可用 (运行: brew install caddy)"
  else
    TS_ARGS_HOSTNAME=""
    if [[ -n "${TS_HOSTNAME}" ]]; then
      if [[ ! -f "${CERT_DIR}/${TS_HOSTNAME}.crt" ]]; then
        echo "==> 生成 Tailscale 证书 (${TS_HOSTNAME})..."
        (cd "${CERT_DIR}" && vk_tailscale cert "${TS_HOSTNAME}") \
          || echo "WARN: tailscale cert 生成失败，Tailscale HTTPS 前门跳过"
      fi
      if [[ -f "${CERT_DIR}/${TS_HOSTNAME}.crt" ]]; then
        TS_ARGS_HOSTNAME="${TS_HOSTNAME}"
        TS_CADDY_HOSTNAME="${TS_HOSTNAME}"
      fi
    fi

    # args: DESKTOP_HTTPS_PORT REMOTE_PORT OUT [TS_HOSTNAME FE BE RELAY \
    #        MOBILE_HTTPS MOBILE_RELAY_HTTPS CERT_DIR DESKTOP_RELAY_HTTPS]
    bash scripts/vk-render-caddyfile.sh \
      "${VK_DESKTOP_HTTPS_PORT}" "${VK_REMOTE_PORT}" "${CADDYFILE}" \
      "${TS_ARGS_HOSTNAME}" "${FRONTEND_PORT}" "${BACKEND_PORT}" "${VK_RELAY_PORT}" \
      "${VK_MOBILE_HTTPS_PORT}" "${VK_MOBILE_RELAY_HTTPS_PORT}" "${CERT_DIR}" \
      "${VK_DESKTOP_RELAY_HTTPS_PORT}"

    if [[ -f "${PID_DIR}/caddy.pid" ]] && kill -0 "$(cat "${PID_DIR}/caddy.pid")" 2>/dev/null; then
      caddy reload --config "${CADDYFILE}" 2>/dev/null || true
    else
      # Launch detached in its own session so caddy survives vk-start exiting
      # (and is not reaped by the caller's process group).
      if command -v setsid >/dev/null 2>&1; then
        setsid caddy run --config "${CADDYFILE}" --pidfile "${PID_DIR}/caddy.pid" \
          >> "${LOG_DIR}/caddy.log" 2>&1 &
      else
        nohup python3 -c "import os; os.setsid(); os.execvp('caddy', ['caddy','run','--config','${CADDYFILE}','--pidfile','${PID_DIR}/caddy.pid'])" \
          >> "${LOG_DIR}/caddy.log" 2>&1 &
      fi
      for _ in $(seq 1 10); do
        [[ -f "${PID_DIR}/caddy.pid" ]] && kill -0 "$(cat "${PID_DIR}/caddy.pid")" 2>/dev/null && break
        sleep 1
      done
    fi
    if [[ -f "${PID_DIR}/caddy.pid" ]] && kill -0 "$(cat "${PID_DIR}/caddy.pid")" 2>/dev/null; then
      CADDY_STARTED=1
    fi

    if [[ "${CADDY_STARTED}" -eq 1 && "${WANT_DESKTOP_H2}" -eq 1 ]]; then
      # System curl uses the macOS system trust store, just like the browser.
      SYS_CURL="/usr/bin/curl"; [[ -x "${SYS_CURL}" ]] || SYS_CURL="curl"
      # Probe /v1/health (proxied to the already-healthy Remote) rather than /
      # (now proxied to Vite, which has not started yet at this point).
      H2_PROBE="https://localhost:${VK_DESKTOP_HTTPS_PORT}/v1/health"
      h2_running=0
      for _ in $(seq 1 10); do
        if curl -skf "${H2_PROBE}" >/dev/null 2>&1; then
          h2_running=1
          break
        fi
        sleep 1
      done
      if [[ "${h2_running}" -ne 1 ]]; then
        echo "WARN: 桌面 h2 前门未就绪 (https://localhost:${VK_DESKTOP_HTTPS_PORT})，见 ${LOG_DIR}/caddy.log"
      elif "${SYS_CURL}" -sf "${H2_PROBE}" >/dev/null 2>&1; then
        # CA is browser-trusted → safe to route browser traffic over h2.
        DESKTOP_H2_UP=1
      else
        echo "==> 桌面 h2 已运行但本地 CA 未受信任，浏览器暂时仍走 HTTP/1.1。"
        echo "    一次性信任 (需管理员密码): caddy trust"
        echo "    然后: vk-stop && vk-start  即可启用 HTTP/2 丝滑切换。"
      fi
    fi
  fi
fi

# Mobile (Tailscale) HTTPS front door: phone + desktop-via-tailscale already h2.
# IMPORTANT: only the BROWSER-facing VITE_* bases point at the Tailscale HTTPS
# front door. Server-side VK_SHARED_API_BASE / VK_SHARED_RELAY_API_BASE stay on
# local http (set by vk_configure_public_urls) so the Rust reqwest client talks
# to the remote/relay directly. Routing the server through Caddy made it loop
# back over the tailscale hostname and fail /v1/tokens/refresh → 502 on
# /api/auth/token, breaking auth + relay registration + project loading.
#
# Both browser bases are pinned to the Tailscale https front door so the phone is
# ALWAYS HTTP/2 (h2 needs TLS — a plaintext http LAN base would be HTTP/1.1 and
# hit the ~6-per-origin connection limit, stalling the ~9 Electric shapes). The
# single Tailscale hostname works on every network: on the same WiFi Tailscale
# auto-selects a LAN P2P path (~5ms, direct, still h2), and on cellular it falls
# back to a relay — one address, h2 everywhere, no per-network reconfig. (We
# tried an http LAN base so same-WiFi went direct, but that path is HTTP/1.1;
# Tailscale already gives direct + h2 on the LAN, so the front door wins.)
# Allowed origins already include the tailscale https origins (see vk-dev-lib).
if [[ "${MOBILE}" -eq 1 && "${CADDY_STARTED}" -eq 1 ]]; then
  export VITE_VK_SHARED_API_BASE="https://${TS_HOSTNAME}:${VK_MOBILE_HTTPS_PORT}"
  export VITE_RELAY_API_BASE_URL="https://${TS_HOSTNAME}:${VK_MOBILE_RELAY_HTTPS_PORT}"
elif [[ "${DESKTOP_H2_UP}" -eq 1 ]]; then
  # Non-mobile: the browser opens the full app via the localhost h2 front door
  # (https://localhost:${VK_DESKTOP_HTTPS_PORT}) and talks same-origin, so all
  # REST + Electric shapes multiplex on one h2 connection. The frontend resolver
  # rewrites the reported http base to the page origin on https pages, so the
  # build-time base only acts as a fallback. Point the BROWSER relay base at the
  # desktop relay front door to avoid mixed-content (https page → http relay).
  # Server-side VK_SHARED_API_BASE / VK_SHARED_RELAY_API_BASE stay http so the
  # Rust reqwest client never needs to trust Caddy's local CA.
  export VITE_VK_SHARED_API_BASE="https://localhost:${VK_DESKTOP_HTTPS_PORT}"
  export VITE_RELAY_API_BASE_URL="https://localhost:${VK_DESKTOP_RELAY_HTTPS_PORT}"
fi

if [[ -n "${TS_CADDY_HOSTNAME}" ]]; then
  export VITE_TAILSCALE_RELAY_HTTPS_PORT="${VK_MOBILE_RELAY_HTTPS_PORT}"
fi

# Docker --force-recreate drops in-memory relay tunnels; backend must reconnect.
if vk_dev_running; then
  if vk_local_dev_healthy "${BACKEND_PORT}" "${FRONTEND_PORT}"; then
    echo "==> 重启 local dev（Remote/Relay 容器已重建，需重连 relay 隧道）..."
    vk_stop_local_dev "${ROOT}" "${PID_DIR}"
  else
    echo "==> 清理异常 dev 进程（backend 未响应）..."
    vk_stop_local_dev "${ROOT}" "${PID_DIR}"
  fi
fi

if vk_local_dev_healthy "${BACKEND_PORT}" "${FRONTEND_PORT}"; then
  echo "==> dev 已在运行"
  if [[ "${MOBILE}" -eq 1 && "${CADDY_STARTED}" -eq 1 ]]; then
    echo "WARN: 手机 HTTPS 已启用。若刚改过环境变量，请 vk-stop 后重新 vk-start 以重启 dev/backend。"
  fi
else
  echo "==> Starting dev (log: ${LOG_DIR}/dev.log)..."
  if [[ -s "${LOG_DIR}/dev.log" ]]; then
    cp "${LOG_DIR}/dev.log" "${LOG_DIR}/dev.log.prev" 2>/dev/null || true
  fi
  : > "${LOG_DIR}/dev.log"
  export FRONTEND_PORT BACKEND_PORT PREVIEW_PROXY_PORT
  unset PORT
  # Scrub any inherited Claude provider env. When vk-start is launched from a
  # Claude Code / cc-switch shell (e.g. the desktop 3p claude-code), that shell
  # exports ANTHROPIC_BASE_URL / ANTHROPIC_AUTH_TOKEN etc. The backend would pass
  # them to every spawned coding-agent, and a process-level ANTHROPIC_BASE_URL
  # OVERRIDES each executor variant's own CLAUDE_CONFIG_DIR/settings.json — so all
  # provider variants silently route to the same upstream (and provider billing
  # lands on the wrong account). Executors must rely solely on per-variant config,
  # so strip these before launching dev.
  for __vk_v in $(compgen -v 2>/dev/null | grep -E '^ANTHROPIC_' || true); do unset "${__vk_v}"; done
  unset ANTHROPIC_BASE_URL ANTHROPIC_AUTH_TOKEN ANTHROPIC_API_KEY \
        CLAUDE_CODE_PROVIDER_MANAGED_BY_HOST CLAUDE_CODE_HOST_AUTH_ENV_VAR 2>/dev/null || true
  export VK_ASSET_DIR VK_SHARED_API_BASE VK_SHARED_RELAY_API_BASE VK_BROWSER_SHARED_API_BASE VITE_VK_SHARED_API_BASE VK_ALLOWED_ORIGINS
  export VK_DEV_HOST VITE_RELAY_PORT VITE_RELAY_API_BASE_URL VITE_TAILSCALE_RELAY_HTTPS_PORT
  vk_configure_dev_cargo_target
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

# Board Agents chat depends on the local sidecar (Vite proxies /agent-sidecar).
vk_start_agent_sidecar "${ROOT}" "${PID_DIR}" "${LOG_DIR}" || true

echo ""
echo "━━━━━━━━ Vibe Kanban 已启动 ━━━━━━━━"
if [[ "${DESKTOP_H2_UP}" -eq 1 ]]; then
  echo "本机 h2（推荐，丝滑）: https://localhost:${VK_DESKTOP_HTTPS_PORT} (整个 app 走 HTTP/2 同源)"
  echo "  Relay 前门:         https://localhost:${VK_DESKTOP_RELAY_HTTPS_PORT}"
  echo "本机退路（不走 h2）:   http://localhost:${FRONTEND_PORT} (Vite 直连, HTTP/1.1)"
else
  echo "本地 desktop:  http://localhost:${FRONTEND_PORT}"
fi
echo "Remote Web:    http://localhost:${VK_REMOTE_PORT}"
echo "Relay:         http://localhost:${VK_RELAY_PORT}"
echo "登录:          admin@local.dev / devpass123"
if [[ "${MOBILE}" -eq 1 && "${CADDY_STARTED}" -eq 1 ]]; then
  echo "手机/CardComputer (Tailscale + HTTPS，推荐):"
  echo "  https://${TS_HOSTNAME}:${VK_MOBILE_HTTPS_PORT}"
  echo "  (Relay: https://${TS_HOSTNAME}:${VK_MOBILE_RELAY_HTTPS_PORT})"
fi
if [[ -n "${TS_IP}" ]]; then
  echo "Tailscale 其他机器:"
  if [[ -n "${TS_HOSTNAME}" && -f "${CERT_DIR}/${TS_HOSTNAME}.crt" ]]; then
    echo "  HTTPS (推荐): https://${TS_HOSTNAME}:${VK_MOBILE_HTTPS_PORT}"
    echo "  Relay HTTPS:  https://${TS_HOSTNAME}:${VK_MOBILE_RELAY_HTTPS_PORT}"
  fi
  echo "  HTTP Desktop: http://${TS_IP}:${FRONTEND_PORT}"
  echo "  HTTP Remote:  http://${TS_IP}:${VK_REMOTE_PORT}"
  echo "  HTTP Relay:   http://${TS_IP}:${VK_RELAY_PORT}"
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
