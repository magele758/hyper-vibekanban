#!/usr/bin/env bash
# vk-test-feature.sh — isolated lite-mode test instance for worktree feature development.
#
# Safe contract (enforced, not advisory):
#   - Never reads or writes dev_assets/ or the main VK_ASSET_DIR.
#   - DISABLE_WORKTREE_CLEANUP=1 always: orphan cleanup never runs, so this
#     instance cannot delete the main service's worktrees and vice versa.
#   - Ports are auto-discovered; refused if they collide with the main stack.
#   - All state lives under <worktree>/.vk-test/ and is fully self-contained.
#
# Usage:
#   ./scripts/vk-test-feature.sh up           # start in background
#   ./scripts/vk-test-feature.sh down         # stop
#   ./scripts/vk-test-feature.sh status       # ports / pids / data dir
#   ./scripts/vk-test-feature.sh logs [n]     # tail logs (default 50 lines)
#   ./scripts/vk-test-feature.sh clean        # stop + delete .vk-test/ (with confirm)
#   ./scripts/vk-test-feature.sh cargo-test   # run in-place feature unit tests (no server)

set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
TEST_DIR="${ROOT}/.vk-test"
ASSETS_DIR="${TEST_DIR}/assets"
TMP_DIR="${TEST_DIR}/tmp"
PIDS_DIR="${TEST_DIR}/pids"
LOGS_DIR="${TEST_DIR}/logs"
PORTS_FILE="${TEST_DIR}/ports.env"

# ── Main-service port set (we will NEVER pick any of these) ──────────────────
MAIN_PORTS=(13000 13001 13002 13003 13443 13444 13445 15433 18082 18443)

# ── Port slot discovery ───────────────────────────────────────────────────────
# A "slot" is 3 consecutive ports: frontend, backend, preview-proxy.
# Start at BASE (default 14001) and step by 3 until all three are free.
BASE_PORT="${VK_TEST_BASE_PORT:-14001}"

_port_in_use() {
    lsof -i TCP:"$1" -sTCP:LISTEN -t >/dev/null 2>&1
}

_is_main_port() {
    local p="$1"
    for mp in "${MAIN_PORTS[@]}"; do [ "$p" -eq "$mp" ] && return 0; done
    return 1
}

find_free_slot() {
    local p="$BASE_PORT"
    while [ "$p" -lt 20000 ]; do
        local fe="$p" be="$((p+1))" pp="$((p+2))"
        if _is_main_port "$fe" || _is_main_port "$be" || _is_main_port "$pp"; then
            p="$((p+3))"; continue
        fi
        if ! _port_in_use "$fe" && ! _port_in_use "$be" && ! _port_in_use "$pp"; then
            echo "$p"; return 0
        fi
        p="$((p+3))"
    done
    echo "ERROR: no free 3-port slot found between $BASE_PORT and 19999" >&2
    exit 1
}

# ── Safety guards ─────────────────────────────────────────────────────────────
guard_paths() {
    # Refuse if our asset dir would be the project's dev_assets (symlink-safe).
    local dev_assets="${ROOT}/dev_assets"
    local our_assets
    our_assets="$(mkdir -p "$ASSETS_DIR" && cd "$ASSETS_DIR" && pwd)"
    local main_assets
    main_assets="$(mkdir -p "$dev_assets" && cd "$dev_assets" && pwd)"
    if [ "$our_assets" = "$main_assets" ]; then
        echo "FATAL: ASSETS_DIR resolved to dev_assets — refusing to start." >&2
        exit 1
    fi
    # .vk-test/ must live inside this worktree.
    local test_canon
    test_canon="$(mkdir -p "$TEST_DIR" && cd "$TEST_DIR" && pwd)"
    local root_canon
    root_canon="$(cd "$ROOT" && pwd)"
    case "$test_canon" in
        "${root_canon}/"*) ;;  # OK: it's under the worktree
        *) echo "FATAL: TEST_DIR is not inside the worktree root." >&2; exit 1 ;;
    esac
}

# ── PID helpers ───────────────────────────────────────────────────────────────
_pid_file() { echo "${PIDS_DIR}/$1.pid"; }

_is_running() {
    local pidfile; pidfile="$(_pid_file "$1")"
    [ -f "$pidfile" ] && kill -0 "$(cat "$pidfile")" 2>/dev/null
}

_stop_process() {
    local name="$1"
    local pidfile; pidfile="$(_pid_file "$name")"
    if [ -f "$pidfile" ]; then
        local pid; pid="$(cat "$pidfile")"
        if kill -0 "$pid" 2>/dev/null; then
            kill "$pid" && echo "  stopped $name (pid $pid)"
        fi
        rm -f "$pidfile"
    fi
}

# ── Commands ──────────────────────────────────────────────────────────────────
cmd_up() {
    if _is_running server || _is_running vite; then
        echo "Already running. Use 'status' or 'down' first."
        exit 1
    fi

    guard_paths

    local slot; slot="$(find_free_slot)"
    local FE_PORT="$slot"
    local BE_PORT="$((slot+1))"
    local PP_PORT="$((slot+2))"

    mkdir -p "$ASSETS_DIR" "$TMP_DIR" "$PIDS_DIR" "$LOGS_DIR"

    # Persist chosen ports so status/logs can read them without a live server.
    cat >"$PORTS_FILE" <<EOF
VK_TEST_FE_PORT=${FE_PORT}
VK_TEST_BE_PORT=${BE_PORT}
VK_TEST_PP_PORT=${PP_PORT}
EOF

    echo ""
    echo "╔══ vk-test-feature ══════════════════════════════════════╗"
    echo "║  Isolated lite-mode instance (this worktree only)       ║"
    echo "╠══════════════════════════════════════════════════════════╣"
    printf "║  Frontend   http://localhost:%-5s                       ║\n" "${FE_PORT}"
    printf "║  Backend    http://localhost:%-5s/api                   ║\n" "${BE_PORT}"
    printf "║  Data dir   %-43s ║\n" ".vk-test/assets"
    printf "║  Logs       %-43s ║\n" ".vk-test/logs/{server,vite}.log"
    echo "╠══════════════════════════════════════════════════════════╣"
    echo "║  Main service (13001) is NOT affected.                   ║"
    echo "╚══════════════════════════════════════════════════════════╝"
    echo ""

    # ── Backend (cargo watch) ────────────────────────────────────────────────
    env \
        VK_ASSET_DIR="$ASSETS_DIR" \
        TMPDIR="$TMP_DIR" \
        DISABLE_WORKTREE_CLEANUP=1 \
        BACKEND_PORT="$BE_PORT" \
        PREVIEW_PROXY_PORT="$PP_PORT" \
        RUST_LOG="${RUST_LOG:-info}" \
        VK_ALLOWED_ORIGINS="http://localhost:${FE_PORT}" \
        VITE_VK_SHARED_API_BASE="" \
        VK_SHARED_API_BASE="" \
        VK_SHARED_RELAY_API_BASE="" \
        cargo watch -q -w "${ROOT}/crates" \
            -x "run --bin server" \
            >"${LOGS_DIR}/server.log" 2>&1 &
    echo "$!" >"$(_pid_file server)"

    # ── Frontend (vite) ──────────────────────────────────────────────────────
    # vite.config.ts reads the port/host/proxy target from env (FRONTEND_PORT,
    # VK_DEV_HOST, BACKEND_PORT) — NOT from `--port`. Passing `--port` through the
    # npm-script `--` separator gets double-wrapped and ignored, so vite falls
    # back to its default and can collide with the main service. Set the env.
    env \
        FRONTEND_PORT="$FE_PORT" \
        BACKEND_PORT="$BE_PORT" \
        PREVIEW_PROXY_PORT="$PP_PORT" \
        VK_DEV_HOST="localhost" \
        VITE_OPEN="false" \
        VITE_VK_SHARED_API_BASE="" \
        pnpm --dir "${ROOT}/packages/local-web" run dev \
            >"${LOGS_DIR}/vite.log" 2>&1 &
    echo "$!" >"$(_pid_file vite)"

    echo "Starting... (logs: .vk-test/logs/)"
    echo "  ./scripts/vk-test-feature.sh logs    — follow logs"
    echo "  ./scripts/vk-test-feature.sh down    — stop"
    echo ""
}

cmd_down() {
    _stop_process server
    _stop_process vite
    echo "Instance stopped."
}

cmd_status() {
    echo ""
    echo "vk-test-feature status"
    echo "  Root:     $TEST_DIR"
    if [ -f "$PORTS_FILE" ]; then
        # shellcheck source=/dev/null
        source "$PORTS_FILE"
        echo "  Frontend: http://localhost:${VK_TEST_FE_PORT}  $(_is_running vite  && echo '(running)' || echo '(stopped)')"
        echo "  Backend:  http://localhost:${VK_TEST_BE_PORT}  $(_is_running server && echo '(running)' || echo '(stopped)')"
    else
        echo "  No port info (not started yet)"
    fi
    echo "  Server PID:  $(cat "$(_pid_file server)" 2>/dev/null || echo '-')"
    echo "  Vite PID:    $(cat "$(_pid_file vite)"   2>/dev/null || echo '-')"
    echo ""
}

cmd_logs() {
    local lines="${1:-50}"
    local server_log="${LOGS_DIR}/server.log"
    local vite_log="${LOGS_DIR}/vite.log"
    if [ ! -f "$server_log" ] && [ ! -f "$vite_log" ]; then
        echo "No logs yet — run 'up' first."
        exit 1
    fi
    echo "=== Tailing server + vite logs (Ctrl-C to stop) ==="
    tail -n "$lines" -f "$server_log" "$vite_log" 2>/dev/null
}

cmd_clean() {
    if _is_running server || _is_running vite; then
        cmd_down
    fi
    if [ ! -d "$TEST_DIR" ]; then
        echo ".vk-test/ does not exist, nothing to clean."
        return
    fi
    echo "This will delete: $TEST_DIR"
    printf "Confirm? [y/N] "
    read -r answer
    case "$answer" in
        y|Y)
            # Safety: ensure we only delete inside the worktree.
            local test_canon
            test_canon="$(cd "$TEST_DIR" && pwd)"
            local root_canon
            root_canon="$(cd "$ROOT" && pwd)"
            case "$test_canon" in
                "${root_canon}/"*) rm -rf "$TEST_DIR" && echo "Deleted $TEST_DIR" ;;
                *) echo "FATAL: TEST_DIR not inside worktree — refusing rm." >&2; exit 1 ;;
            esac
            ;;
        *) echo "Aborted." ;;
    esac
}

cmd_cargo_test() {
    echo "Running in-place feature unit tests (no server required)..."
    cd "$ROOT"
    cargo test -p workspace-manager -p local-deployment -p db "$@"
}

# ── Dispatch ──────────────────────────────────────────────────────────────────
CMD="${1:-help}"
shift 2>/dev/null || true

case "$CMD" in
    up)           cmd_up ;;
    down)         cmd_down ;;
    status)       cmd_status ;;
    logs)         cmd_logs "${1:-50}" ;;
    clean)        cmd_clean ;;
    cargo-test)   cmd_cargo_test "$@" ;;
    *)
        echo "Usage: $(basename "$0") {up|down|status|logs [n]|clean|cargo-test}"
        echo ""
        echo "  up           Start isolated lite-mode instance in background"
        echo "  down         Stop the instance"
        echo "  status       Show ports / pids / data dir"
        echo "  logs [n]     Tail server+vite logs (default last 50 lines)"
        echo "  clean        Stop + delete all .vk-test/ state (with confirm)"
        echo "  cargo-test   Run workspace-manager/local-deployment unit tests"
        ;;
esac
