#!/usr/bin/env bash
# vk-preview-full.sh — Remote + Relay + Desktop Host on the preview machine.
#
# Topology (ports from VK_PREVIEW_PORTS_BASE, default 23000):
#   Remote UI/API   :23000
#   Desktop UI      :23001   ← open this for full product (workspaces)
#   Local API       :23002
#   Preview proxy   :23003
#   Agent sidecar   :23110
#   Relay           :28082
#   DB              127.0.0.1:25433
#
# Usage (on preview host):
#   cd /path/to/repo && bash scripts/vk-preview-full.sh up|down|status|logs|smoke|clean

set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"

# Prefer nvm Node >=20 when available (system node may be 18).
if [[ -s "${HOME}/.nvm/nvm.sh" ]]; then
  # shellcheck disable=SC1091
  . "${HOME}/.nvm/nvm.sh"
  nvm use 22 >/dev/null 2>&1 || nvm use 20 >/dev/null 2>&1 || nvm use default >/dev/null 2>&1 || true
fi
export PATH="${HOME}/.cargo/bin:${HOME}/.local/share/pnpm:${PATH}"
# After nvm, put active node bin first.
if command -v node >/dev/null 2>&1; then
  NODE_BIN="$(dirname "$(command -v node)")"
  export PATH="${NODE_BIN}:${PATH}"
fi

STATE_DIR="${HOME}/.vk-preview"
LOG_DIR="${STATE_DIR}/logs"
PID_DIR="${STATE_DIR}/pids"
ASSETS_DIR="${STATE_DIR}/assets"
TMP_DIR="${STATE_DIR}/tmp"
CARGO_TARGET="${STATE_DIR}/cargo-target"
COMPOSE_PROJECT="vk-preview"

mkdir -p "${LOG_DIR}" "${PID_DIR}" "${ASSETS_DIR}" "${TMP_DIR}" "${CARGO_TARGET}"

PORTS_BASE="${VK_PREVIEW_PORTS_BASE:-23000}"
REMOTE_PORT="${PORTS_BASE}"
FRONTEND_PORT="$((PORTS_BASE + 1))"
BACKEND_PORT="$((PORTS_BASE + 2))"
PREVIEW_PROXY_PORT="$((PORTS_BASE + 3))"
RELAY_PORT="$((PORTS_BASE + 18082 - 13000))"
DB_PORT="$((PORTS_BASE + 15433 - 13000))"
SIDECAR_PORT="$((PORTS_BASE + 110))"

_pid_file() { echo "${PID_DIR}/$1.pid"; }

_is_running() {
  local pidfile
  pidfile="$(_pid_file "$1")"
  [[ -f "$pidfile" ]] && kill -0 "$(cat "$pidfile")" 2>/dev/null
}

_stop_named() {
  local name="$1"
  local pidfile
  pidfile="$(_pid_file "$name")"
  if [[ -f "$pidfile" ]]; then
    local pid
    pid="$(cat "$pidfile")"
    if kill -0 "$pid" 2>/dev/null; then
      kill "$pid" 2>/dev/null || true
      sleep 0.5
      kill -9 "$pid" 2>/dev/null || true
      echo "  stopped $name (pid $pid)"
    fi
    rm -f "$pidfile"
  fi
}

_http_ok() {
  curl -sf --max-time 3 "$1" >/dev/null 2>&1
}

_access_host() {
  local h="${VK_PREVIEW_ACCESS_HOST:-}"
  if [[ -z "$h" ]] && command -v tailscale >/dev/null 2>&1; then
    h="$(tailscale ip -4 2>/dev/null | head -1 || true)"
  fi
  echo "${h:-127.0.0.1}"
}

_ensure_toolchain() {
  local node_major
  node_major="$(node -v 2>/dev/null | sed -E 's/^v([0-9]+).*/\1/' || echo 0)"
  if [[ "${node_major}" -lt 20 ]]; then
    echo "ERROR: Node >=20 required (got $(node -v 2>/dev/null || echo missing)). Source nvm first." >&2
    exit 1
  fi
  command -v cargo >/dev/null || {
    echo "ERROR: cargo not on PATH" >&2
    exit 1
  }
  command -v pnpm >/dev/null || {
    echo "ERROR: pnpm not on PATH" >&2
    exit 1
  }
  command -v cargo-watch >/dev/null || {
    echo "ERROR: cargo-watch missing (cargo install cargo-watch)" >&2
    exit 1
  }
  echo "  toolchain: node $(node -v), pnpm $(pnpm -v), $(cargo -V)"
}

_ensure_deps() {
  if [[ ! -d "${ROOT}/node_modules" ]]; then
    echo "==> pnpm install (first time on preview host)..."
    (cd "${ROOT}" && pnpm install --frozen-lockfile)
  fi
}

cmd_up_remote() {
  bash "${ROOT}/scripts/vk-preview-remote.sh" up
}

cmd_up_host() {
  _ensure_toolchain
  _ensure_deps

  if _is_running server || _is_running vite || _is_running sidecar; then
    echo "==> Desktop host already running; restarting..."
    _stop_named server
    _stop_named vite
    _stop_named sidecar
  fi
  # Free host ports + kill stray preview sidecar (often left on default :13110).
  if command -v fuser >/dev/null 2>&1; then
    fuser -k "${FRONTEND_PORT}/tcp" 2>/dev/null || true
    fuser -k "${BACKEND_PORT}/tcp" 2>/dev/null || true
    fuser -k "${PREVIEW_PROXY_PORT}/tcp" 2>/dev/null || true
    fuser -k "${SIDECAR_PORT}/tcp" 2>/dev/null || true
  fi
  pkill -f "${ROOT}/packages/agent-sidecar" 2>/dev/null || true
  pkill -f "cargo-watch watch -q -w ${ROOT}/crates" 2>/dev/null || true
  sleep 1

  local access
  access="$(_access_host)"
  local browser_api="http://${access}:${REMOTE_PORT}"
  local browser_relay="http://${access}:${RELAY_PORT}"
  local server_api="http://127.0.0.1:${REMOTE_PORT}"
  local server_relay="http://127.0.0.1:${RELAY_PORT}"
  local origins="http://${access}:${FRONTEND_PORT},http://127.0.0.1:${FRONTEND_PORT},http://localhost:${FRONTEND_PORT}"

  echo "==> Building local server (may take a while on first run)..."
  (
    cd "${ROOT}"
    env CARGO_TARGET_DIR="${CARGO_TARGET}" \
      cargo build --bin server
  )

  echo "==> Starting local API (:${BACKEND_PORT})..."
  (
    cd "${ROOT}"
    env \
      VK_ASSET_DIR="${ASSETS_DIR}" \
      TMPDIR="${TMP_DIR}" \
      CARGO_TARGET_DIR="${CARGO_TARGET}" \
      DISABLE_WORKTREE_CLEANUP=1 \
      BACKEND_PORT="${BACKEND_PORT}" \
      PREVIEW_PROXY_PORT="${PREVIEW_PROXY_PORT}" \
      RUST_LOG="${RUST_LOG:-info}" \
      VK_ALLOWED_ORIGINS="${origins}" \
      VK_SHARED_API_BASE="${server_api}" \
      VK_SHARED_RELAY_API_BASE="${server_relay}" \
      VITE_VK_SHARED_API_BASE="${browser_api}" \
      cargo watch -q -w "${ROOT}/crates" \
        -x "run --bin server" \
      >"${LOG_DIR}/server.log" 2>&1 &
    echo $! >"$(_pid_file server)"
  )

  echo "==> Starting Desktop UI (:${FRONTEND_PORT})..."
  (
    cd "${ROOT}"
    env \
      FRONTEND_PORT="${FRONTEND_PORT}" \
      BACKEND_PORT="${BACKEND_PORT}" \
      PREVIEW_PROXY_PORT="${PREVIEW_PROXY_PORT}" \
      VK_DEV_HOST="0.0.0.0" \
      VITE_OPEN="false" \
      VITE_VK_SHARED_API_BASE="${browser_api}" \
      VITE_RELAY_API_BASE_URL="${browser_relay}" \
      AGENT_SIDECAR_PROXY_TARGET="http://127.0.0.1:${SIDECAR_PORT}" \
      VITE_AGENT_SIDECAR_BASE="/agent-sidecar" \
      pnpm --dir "${ROOT}/packages/local-web" run dev \
      >"${LOG_DIR}/vite.log" 2>&1 &
    echo $! >"$(_pid_file vite)"
  )

  echo "==> Starting agent-sidecar (:${SIDECAR_PORT})..."
  (
    cd "${ROOT}/packages/agent-sidecar"
    env \
      PORT="${SIDECAR_PORT}" \
      VK_REMOTE_API_BASE="${server_api}" \
      VK_LOCAL_API_BASE="http://127.0.0.1:${BACKEND_PORT}" \
      pnpm exec tsx src/index.ts \
      >"${LOG_DIR}/sidecar.log" 2>&1 &
    echo $! >"$(_pid_file sidecar)"
  )

  echo "  Waiting for Desktop + sidecar health..."
  local ok=0
  local side_ok=0
  for _ in $(seq 1 90); do
    if _http_ok "http://127.0.0.1:${SIDECAR_PORT}/health"; then
      side_ok=1
    fi
    if _http_ok "http://127.0.0.1:${BACKEND_PORT}/api/health" \
      && _http_ok "http://127.0.0.1:${FRONTEND_PORT}/"; then
      ok=1
      if [[ "$side_ok" -eq 1 ]]; then
        break
      fi
    fi
    echo -n "."
    sleep 2
  done
  echo ""
  if [[ "$ok" -eq 0 ]]; then
    echo "WARN: Desktop did not become healthy in time. See ${LOG_DIR}/" >&2
  else
    echo "  Desktop host healthy."
  fi
  if [[ "$side_ok" -eq 0 ]]; then
    echo "WARN: agent-sidecar :${SIDECAR_PORT} 未就绪（/agents 模型配置会失败）。见 ${LOG_DIR}/sidecar.log" >&2
  else
    echo "  agent-sidecar healthy (:${SIDECAR_PORT})."
  fi
}

cmd_up() {
  cmd_up_remote
  cmd_up_host
  _print_banner
}

_print_banner() {
  local access
  access="$(_access_host)"
  echo ""
  echo "╔══ vk-preview (full) ════════════════════════════════════╗"
  printf "║  Desktop    http://%s:%-5s  ← 完整功能入口        ║\n" "$access" "$FRONTEND_PORT"
  printf "║  Local API  http://%s:%-5s                       ║\n" "$access" "$BACKEND_PORT"
  printf "║  Remote UI  http://%s:%-5s                       ║\n" "$access" "$REMOTE_PORT"
  printf "║  Relay      http://%s:%-5s                       ║\n" "$access" "$RELAY_PORT"
  printf "║  Sidecar    http://127.0.0.1:%-5s                 ║\n" "$SIDECAR_PORT"
  echo "║  Data dir   ${STATE_DIR}/assets                          ║"
  echo "╚═════════════════════════════════════════════════════════╝"
  echo ""
  echo "用 Desktop 地址测 Workspace；Remote 地址只是数据中心 UI。"
  echo ""
}

cmd_down() {
  echo "==> Stopping Desktop host..."
  _stop_named sidecar
  _stop_named vite
  _stop_named server
  # Also free ports if orphans remain
  if command -v fuser >/dev/null 2>&1; then
    fuser -k "${FRONTEND_PORT}/tcp" 2>/dev/null || true
    fuser -k "${BACKEND_PORT}/tcp" 2>/dev/null || true
    fuser -k "${SIDECAR_PORT}/tcp" 2>/dev/null || true
  fi
  echo "==> Stopping Remote Docker..."
  bash "${ROOT}/scripts/vk-preview-remote.sh" down || true
  echo "  Full preview stopped."
}

cmd_status() {
  local access
  access="$(_access_host)"
  echo ""
  echo "vk-preview-full status"
  echo "  Desktop:  http://${access}:${FRONTEND_PORT}  $(_is_running vite && echo running || echo stopped)"
  echo "  LocalAPI: http://${access}:${BACKEND_PORT}  $(_is_running server && echo running || echo stopped)"
  echo "  Sidecar:  :${SIDECAR_PORT}  $(_is_running sidecar && echo running || echo stopped)"
  echo "  Remote:   http://${access}:${REMOTE_PORT}"
  echo "  Relay:    http://${access}:${RELAY_PORT}"
  echo ""
  bash "${ROOT}/scripts/vk-preview-remote.sh" status || true
  echo "Desktop health:"
  if _http_ok "http://127.0.0.1:${FRONTEND_PORT}/"; then echo "  Vite: OK"; else echo "  Vite: FAIL"; fi
  if _http_ok "http://127.0.0.1:${BACKEND_PORT}/api/health"; then echo "  API:  OK"; else echo "  API:  FAIL"; fi
  if _http_ok "http://127.0.0.1:${SIDECAR_PORT}/health"; then echo "  Side: OK"; else echo "  Side: FAIL/optional"; fi
  echo ""
}

cmd_logs() {
  echo "=== ${LOG_DIR} (Ctrl-C to stop) ==="
  tail -n 80 -f "${LOG_DIR}/server.log" "${LOG_DIR}/vite.log" "${LOG_DIR}/sidecar.log" 2>/dev/null
}

cmd_smoke() {
  local fail=0
  bash "${ROOT}/scripts/vk-preview-remote.sh" smoke || fail=1
  echo -n "  Desktop UI:   "
  if _http_ok "http://127.0.0.1:${FRONTEND_PORT}/"; then echo OK; else echo FAIL; fail=1; fi
  echo -n "  Local API:    "
  if _http_ok "http://127.0.0.1:${BACKEND_PORT}/api/health"; then echo OK; else echo FAIL; fail=1; fi
  echo -n "  Sidecar:      "
  if _http_ok "http://127.0.0.1:${SIDECAR_PORT}/health"; then echo OK; else echo FAIL; fail=1; fi
  echo -n "  Sidecar proxy:"
  if _http_ok "http://127.0.0.1:${FRONTEND_PORT}/agent-sidecar/health"; then echo OK; else echo FAIL; fail=1; fi
  if [[ "$fail" -eq 0 ]]; then
    echo ""; echo "✓ Full smoke passed."
  else
    echo ""; echo "✗ Full smoke failed." >&2
    exit 1
  fi
}

cmd_clean() {
  cmd_down
  bash "${ROOT}/scripts/vk-preview-remote.sh" clean || true
  echo "==> Removing preview host state under ${STATE_DIR} (keeping cargo-target)..."
  rm -rf "${ASSETS_DIR}" "${TMP_DIR}" "${LOG_DIR}" "${PID_DIR}"
  mkdir -p "${LOG_DIR}" "${PID_DIR}" "${ASSETS_DIR}" "${TMP_DIR}"
  echo "  Clean done."
}

CMD="${1:-help}"
case "$CMD" in
  up)     cmd_up ;;
  down)   cmd_down ;;
  status) cmd_status ;;
  logs)   cmd_logs ;;
  smoke)  cmd_smoke ;;
  clean)  cmd_clean ;;
  *)
    echo "Usage: $(basename "$0") {up|down|status|logs|smoke|clean}" >&2
    exit 1
    ;;
esac
