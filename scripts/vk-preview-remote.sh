#!/usr/bin/env bash
# vk-preview-remote.sh — run on the remote machine to manage preview stack.
#
# Starts Remote Docker stack with independent ports/volumes (vk-preview project).
# Called by vk-isolate.sh via SSH; can also be run directly on the remote.
#
# Usage (on remote machine):
#   cd /path/to/repo && bash scripts/vk-preview-remote.sh up|down|status|logs|smoke|clean
#
# Ports (offset from VK_PREVIEW_PORTS_BASE, default 23000):
#   Remote:       23000
#   Frontend:     23001 (if full mode)
#   Backend:      23002 (if full mode)
#   PreviewProxy: 23003 (if full mode)
#   Relay:        28082
#   DB:           127.0.0.1:25433

set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
STATE_DIR="${HOME}/.vk-preview"
LOG_DIR="${STATE_DIR}/logs"
PID_DIR="${STATE_DIR}/pids"
COMPOSE_PROJECT="vk-preview"

mkdir -p "${LOG_DIR}" "${PID_DIR}"

# ── Port configuration ────────────────────────────────────────────────────────
PORTS_BASE="${VK_PREVIEW_PORTS_BASE:-23000}"
REMOTE_PORT="${PORTS_BASE}"
FRONTEND_PORT="$((PORTS_BASE + 1))"
BACKEND_PORT="$((PORTS_BASE + 2))"
PREVIEW_PROXY_PORT="$((PORTS_BASE + 3))"
RELAY_PORT="$((PORTS_BASE + 8082 - 13000))"  # 23000 + (18082 - 13000) = 28082
DB_PORT="$((PORTS_BASE + 5433 - 13000))"     # 25433

# ── Commands ──────────────────────────────────────────────────────────────────
cmd_up() {
    echo "==> Starting preview Remote stack (Docker, project: $COMPOSE_PROJECT)..."
    cd "${ROOT}/crates/remote"

    if [[ ! -f .env.remote ]]; then
        echo "ERROR: crates/remote/.env.remote not found" >&2
        echo "  Copy from crates/remote/README.md and configure for preview." >&2
        exit 1
    fi

    # Check if pnpm is available (needed for building)
    if ! command -v pnpm &>/dev/null; then
        echo "WARN: pnpm not found. Attempting to enable via corepack..."
        if command -v corepack &>/dev/null; then
            corepack enable || true
        else
            echo "ERROR: pnpm and corepack not available. Install Node.js 18+ with corepack." >&2
            exit 1
        fi
    fi

    export DOCKER_BUILDKIT=1
    export COMPOSE_DOCKER_CLI_BUILD=1
    export COMPOSE_PROJECT_NAME="$COMPOSE_PROJECT"
    export REMOTE_DB_PORTS="127.0.0.1:${DB_PORT}:5432"
    export REMOTE_SERVER_PORTS="0.0.0.0:${REMOTE_PORT}:8081"
    export REMOTE_RELAY_PORTS="0.0.0.0:${RELAY_PORT}:8082"

    # Build frontend first (needed for Remote Docker image)
    echo "  Building frontend assets..."
    cd "${ROOT}"
    pnpm install --frozen-lockfile || pnpm install
    cd "${ROOT}/packages/remote-web"
    pnpm run build

    cd "${ROOT}/crates/remote"
    docker compose --env-file .env.remote --profile relay \
        up --build -d --force-recreate

    echo "  Waiting for health checks..."
    local remote_ok=0
    for i in $(seq 1 30); do
        if curl -sf "http://127.0.0.1:${REMOTE_PORT}/v1/health" >/dev/null 2>&1 \
            && curl -sf "http://127.0.0.1:${RELAY_PORT}/health" >/dev/null 2>&1; then
            remote_ok=1
            echo "  Remote stack healthy."
            break
        fi
        echo -n "."
        sleep 2
    done
    echo ""

    if [[ "$remote_ok" -eq 0 ]]; then
        echo "WARN: Health checks did not pass within 60s. Check logs." >&2
    fi

    echo ""
    echo "╔══ vk-preview (remote) ══════════════════════════════════╗"
    echo "║  Remote:  http://0.0.0.0:${REMOTE_PORT}                            ║"
    echo "║  Relay:   http://0.0.0.0:${RELAY_PORT}                            ║"
    echo "║  DB:      127.0.0.1:${DB_PORT}                              ║"
    echo "║  Project: ${COMPOSE_PROJECT}                                   ║"
    echo "╚═════════════════════════════════════════════════════════╝"
    echo ""
}

cmd_down() {
    echo "==> Stopping preview Remote stack..."
    cd "${ROOT}/crates/remote"
    export COMPOSE_PROJECT_NAME="$COMPOSE_PROJECT"
    docker compose --env-file .env.remote --profile relay down
    echo "  Preview stack stopped."
}

cmd_status() {
    echo ""
    echo "vk-preview status (remote)"
    echo "  Project: $COMPOSE_PROJECT"
    echo "  Remote:  http://127.0.0.1:${REMOTE_PORT}"
    echo "  Relay:   http://127.0.0.1:${RELAY_PORT}"
    echo ""

    cd "${ROOT}/crates/remote"
    export COMPOSE_PROJECT_NAME="$COMPOSE_PROJECT"
    docker compose ps

    echo ""
    echo "Health checks:"
    if curl -sf "http://127.0.0.1:${REMOTE_PORT}/v1/health" >/dev/null 2>&1; then
        echo "  Remote: OK"
    else
        echo "  Remote: FAIL"
    fi
    if curl -sf "http://127.0.0.1:${RELAY_PORT}/health" >/dev/null 2>&1; then
        echo "  Relay:  OK"
    else
        echo "  Relay:  FAIL"
    fi
    echo ""
}

cmd_logs() {
    cd "${ROOT}/crates/remote"
    export COMPOSE_PROJECT_NAME="$COMPOSE_PROJECT"
    exec docker compose logs -f --tail=100
}

cmd_smoke() {
    echo "==> Running smoke tests..."
    local fail=0

    echo -n "  Remote health: "
    if curl -sf "http://127.0.0.1:${REMOTE_PORT}/v1/health" >/dev/null 2>&1; then
        echo "OK"
    else
        echo "FAIL"
        fail=1
    fi

    echo -n "  Relay health:  "
    if curl -sf "http://127.0.0.1:${RELAY_PORT}/health" >/dev/null 2>&1; then
        echo "OK"
    else
        echo "FAIL"
        fail=1
    fi

    if [[ "$fail" -eq 0 ]]; then
        echo ""
        echo "✓ All smoke tests passed."
    else
        echo ""
        echo "✗ Some smoke tests failed. Check logs." >&2
        exit 1
    fi
}

cmd_clean() {
    echo "==> Cleaning preview stack (stop + remove volumes)..."
    cd "${ROOT}/crates/remote"
    export COMPOSE_PROJECT_NAME="$COMPOSE_PROJECT"
    docker compose --env-file .env.remote --profile relay down -v
    echo "  Volumes removed. State wiped."
}

# ── Dispatch ──────────────────────────────────────────────────────────────────
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
        echo "" >&2
        echo "  up      Start preview Remote stack (Docker)" >&2
        echo "  down    Stop preview stack" >&2
        echo "  status  Show container status + health" >&2
        echo "  logs    Follow container logs" >&2
        echo "  smoke   Run basic health checks" >&2
        echo "  clean   Stop + remove volumes (wipes data)" >&2
        exit 1
        ;;
esac
