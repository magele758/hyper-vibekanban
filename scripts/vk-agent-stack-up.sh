#!/usr/bin/env bash
# Durable local stack for Agents / Copilot (Cursor SDK sidecar + Vite).
# Run this in YOUR terminal (not via Agent) so processes survive agent turns.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
export PATH="/opt/homebrew/bin:/usr/local/bin:$HOME/.cargo/bin:$PATH"

FE_PORT="${FE_PORT:-14007}"
# Prefer the cargo-watch worktree server (see .vk-test/ports.env). Fallback 14008
# is the live Agents-capable instance; stale 14005 pointed shared_api_base at :13000.
BE_PORT="${BE_PORT:-${VK_TEST_BE_PORT:-14008}}"
PP_PORT="${PP_PORT:-${VK_TEST_PP_PORT:-14009}}"
REMOTE_URL="${REMOTE_URL:-http://127.0.0.1:13010}"
SIDECAR_PORT="${SIDECAR_PORT:-13110}"
if [[ -f "$ROOT/.vk-test/ports.env" ]]; then
  # shellcheck disable=SC1091
  source "$ROOT/.vk-test/ports.env"
  BE_PORT="${VK_TEST_BE_PORT:-$BE_PORT}"
  PP_PORT="${VK_TEST_PP_PORT:-$PP_PORT}"
  FE_PORT="${VK_TEST_FE_PORT:-$FE_PORT}"
fi
ENV_FILE="${ENV_FILE:-/tmp/vk-test-remote.env}"
LOG_DIR="$ROOT/.vk-test/logs"
PID_DIR="$ROOT/.vk-test/pids"
mkdir -p "$LOG_DIR" "$PID_DIR"

need() {
  if ! curl -sf -m 2 "$1" >/dev/null; then
    echo "缺少依赖服务: $1"
    echo "请先保证 Remote($REMOTE_URL) 与 Local API(:$BE_PORT) 已启动。"
    exit 1
  fi
}

echo "==> 检查 Remote / Local API"
need "$REMOTE_URL/v1/health"
need "http://127.0.0.1:${BE_PORT}/api/health"

if [[ -f "$ENV_FILE" ]]; then
  # shellcheck disable=SC1090
  set -a; source "$ENV_FILE"; set +a
fi
EMAIL="${SELF_HOST_LOCAL_AUTH_EMAIL:-admin@local.dev}"
PASS="${SELF_HOST_LOCAL_AUTH_PASSWORD:-devpass123}"

echo "==> 登录 Remote 拿 token"
curl -s -X POST "$REMOTE_URL/v1/auth/local/login" \
  -H 'Content-Type: application/json' \
  -d "{\"email\":\"$EMAIL\",\"password\":\"$PASS\"}" > /tmp/vk-login.json
TOKEN="$(python3 -c 'import json;print(json.load(open("/tmp/vk-login.json"))["access_token"])')"

stop_pidfile() {
  local name="$1"
  local f="$PID_DIR/$name.pid"
  if [[ -f "$f" ]]; then
    local pid
    pid="$(cat "$f" || true)"
    if [[ -n "${pid:-}" ]] && kill -0 "$pid" 2>/dev/null; then
      kill "$pid" 2>/dev/null || true
      sleep 0.5
      kill -9 "$pid" 2>/dev/null || true
    fi
    rm -f "$f"
  fi
}

echo "==> 重启 sidecar :$SIDECAR_PORT"
stop_pidfile agent-sidecar
# only kill our sidecar on this port
if lsof -tiTCP:"$SIDECAR_PORT" -sTCP:LISTEN >/dev/null 2>&1; then
  lsof -tiTCP:"$SIDECAR_PORT" -sTCP:LISTEN | xargs kill 2>/dev/null || true
  sleep 0.5
fi
# nohup + disown: survive parent shell exit (Agent 会话结束也不会带走)
nohup env \
  PATH="$PATH" \
  VK_REMOTE_API_BASE="$REMOTE_URL" \
  VK_REMOTE_TOKEN="$TOKEN" \
  PORT="$SIDECAR_PORT" \
  /opt/homebrew/bin/pnpm --dir "$ROOT/packages/agent-sidecar" start \
  >>"$LOG_DIR/agent-sidecar.log" 2>&1 &
echo $! >"$PID_DIR/agent-sidecar.pid"
disown $! 2>/dev/null || true

echo "==> 重启 Vite :$FE_PORT"
stop_pidfile vite
if lsof -tiTCP:"$FE_PORT" -sTCP:LISTEN >/dev/null 2>&1; then
  lsof -tiTCP:"$FE_PORT" -sTCP:LISTEN | xargs kill 2>/dev/null || true
  sleep 0.5
fi
nohup env \
  PATH="$PATH" \
  FRONTEND_PORT="$FE_PORT" \
  BACKEND_PORT="$BE_PORT" \
  PREVIEW_PROXY_PORT="$PP_PORT" \
  VK_DEV_HOST=localhost \
  VITE_OPEN=false \
  VITE_VK_SHARED_API_BASE="$REMOTE_URL" \
  VITE_AGENT_SIDECAR_BASE="/agent-sidecar" \
  AGENT_SIDECAR_PROXY_TARGET="http://127.0.0.1:${SIDECAR_PORT}" \
  VITE_RELAY_API_BASE_URL="${VITE_RELAY_API_BASE_URL:-http://localhost:18082}" \
  /opt/homebrew/bin/pnpm --dir "$ROOT/packages/local-web" run dev \
  >>"$LOG_DIR/vite.log" 2>&1 &
echo $! >"$PID_DIR/vite.pid"
disown $! 2>/dev/null || true

cat >"$ROOT/.vk-test/ports.env" <<EOF
VK_TEST_FE_PORT=$FE_PORT
VK_TEST_BE_PORT=$BE_PORT
VK_TEST_PP_PORT=$PP_PORT
EOF

sleep 2
echo
echo "UI:      http://localhost:${FE_PORT}/"
echo "Remote:  $REMOTE_URL"
echo "Sidecar: http://127.0.0.1:${SIDECAR_PORT}/health"
echo "Logs:    $LOG_DIR/{vite,agent-sidecar}.log"
echo
curl -s -m 2 "http://127.0.0.1:${SIDECAR_PORT}/health" || echo "sidecar 还在启动…"
echo
curl -s -m 2 -o /dev/null -w "vite:%{http_code}\n" "http://localhost:${FE_PORT}/" || true
echo "停掉: kill \$(cat $PID_DIR/vite.pid) \$(cat $PID_DIR/agent-sidecar.pid)"
