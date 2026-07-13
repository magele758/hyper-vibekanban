#!/usr/bin/env bash
# vk-isolate — unified entry point for isolated testing (no impact on main service).
#
# Four modes:
#   lite           Local lite-mode instance (ports 14001+, state in .vk-test/)
#   unit           Local cargo/npm unit tests (no servers)
#   preview-remote Remote machine: Remote Docker stack only (independent ports/volumes)
#   preview-full   Remote machine: Remote + Desktop-style services (full stack)
#
# Usage:
#   ./scripts/vk-isolate.sh [mode] <command> [args...]
#   ./scripts/vk-isolate.sh                          # show help
#   ./scripts/vk-isolate.sh lite up|down|status|logs|clean
#   ./scripts/vk-isolate.sh unit [cargo-test-args...]
#   ./scripts/vk-isolate.sh preview-remote sync|up|down|status|logs|smoke|clean
#   ./scripts/vk-isolate.sh preview-full sync|up|down|status|logs|smoke|clean

set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
CONFIG_FILE="${HOME}/.config/vk-preview/config.env"

# ── Load preview config (for preview-* modes) ─────────────────────────────────
load_preview_config() {
    if [[ -f "$CONFIG_FILE" ]]; then
        # shellcheck source=/dev/null
        source "$CONFIG_FILE"
    fi
    # Allow env overrides
    : "${VK_PREVIEW_HOST:=}"
    : "${VK_PREVIEW_DIR:=}"
    : "${VK_PREVIEW_GIT_REMOTE:=origin}"
    : "${VK_PREVIEW_SYNC_METHOD:=git}"
    : "${VK_PREVIEW_PUBLIC_BASE:=}"
    : "${VK_PREVIEW_PORTS_BASE:=23000}"
    export VK_PREVIEW_HOST VK_PREVIEW_DIR VK_PREVIEW_GIT_REMOTE
    export VK_PREVIEW_SYNC_METHOD VK_PREVIEW_PUBLIC_BASE VK_PREVIEW_PORTS_BASE
}

# ── Check preview config is valid ─────────────────────────────────────────────
check_preview_config() {
    local example="${ROOT}/scripts/vk-preview.config.env.example"
    if [[ -z "$VK_PREVIEW_HOST" ]]; then
        echo "ERROR: VK_PREVIEW_HOST not set." >&2
        echo "" >&2
        echo "Preview mode requires configuration. Create:" >&2
        echo "  mkdir -p ~/.config/vk-preview" >&2
        echo "  cp $example ~/.config/vk-preview/config.env" >&2
        echo "  # Edit config.env with your SSH host and paths" >&2
        echo "" >&2
        exit 1
    fi
    if [[ -z "$VK_PREVIEW_DIR" ]]; then
        echo "ERROR: VK_PREVIEW_DIR not set in $CONFIG_FILE" >&2
        exit 1
    fi
    # Check SSH connectivity (BatchMode = no interactive password prompt)
    if ! ssh -o BatchMode=yes -o ConnectTimeout=5 "$VK_PREVIEW_HOST" true 2>/dev/null; then
        echo "ERROR: Cannot SSH to $VK_PREVIEW_HOST" >&2
        echo "  Check ~/.ssh/config and key-based auth." >&2
        exit 1
    fi
}

# ── Sync code to remote ───────────────────────────────────────────────────────
sync_to_remote() {
    load_preview_config
    check_preview_config

    local branch
    branch="$(git rev-parse --abbrev-ref HEAD)"
    echo "==> Syncing branch '$branch' to $VK_PREVIEW_HOST:$VK_PREVIEW_DIR"

    if [[ "$VK_PREVIEW_SYNC_METHOD" == "rsync" ]]; then
        echo "  Using rsync..."
        rsync -azP --exclude='.git' --exclude='node_modules' --exclude='target' \
            --exclude='.vk-test' --exclude='dev_assets' \
            "$ROOT/" "$VK_PREVIEW_HOST:$VK_PREVIEW_DIR/"
    else
        echo "  Using git (push + remote pull)..."
        # Push current branch to remote
        git push "$VK_PREVIEW_GIT_REMOTE" "$branch" || {
            echo "WARN: git push failed. Continuing (remote may have the code)."
        }
        # Pull on remote
        ssh "$VK_PREVIEW_HOST" "cd '$VK_PREVIEW_DIR' && git fetch '$VK_PREVIEW_GIT_REMOTE' && git checkout '$branch' && git pull '$VK_PREVIEW_GIT_REMOTE' '$branch'"
    fi
    echo "  Sync complete."
}

# ── Remote command dispatcher ─────────────────────────────────────────────────
remote_cmd() {
    local mode="$1"  # preview-remote or preview-full
    local cmd="$2"
    load_preview_config
    check_preview_config

    case "$cmd" in
        sync)
            sync_to_remote
            ;;
        up|down|status|logs|smoke|clean)
            # Run vk-preview-remote.sh on the remote machine
            ssh -t "$VK_PREVIEW_HOST" "cd '$VK_PREVIEW_DIR' && bash scripts/vk-preview-remote.sh '$cmd'"
            ;;
        *)
            echo "Unknown preview command: $cmd" >&2
            echo "Available: sync, up, down, status, logs, smoke, clean" >&2
            exit 1
            ;;
    esac
}

# ── Lite mode (wrapper for vk-test-feature.sh) ────────────────────────────────
lite_cmd() {
    local cmd="${1:-help}"
    shift 2>/dev/null || true
    exec "${ROOT}/scripts/vk-test-feature.sh" "$cmd" "$@"
}

# ── Unit tests ────────────────────────────────────────────────────────────────
unit_cmd() {
    cd "$ROOT"
    echo "==> Running unit tests (workspace-manager, local-deployment, db)..."
    exec cargo test -p workspace-manager -p local-deployment -p db "$@"
}

# ── Help ──────────────────────────────────────────────────────────────────────
show_help() {
    cat <<EOF
vk-isolate — unified entry point for isolated testing

MODES:
  lite           Local lite-mode instance (ports 14001+, state in .vk-test/)
                 Wraps vk-test-feature.sh; main service on 13001 unaffected.

  unit           Local cargo/npm unit tests (no servers).

  preview-remote Remote machine: Remote Docker stack only.
                 Independent ports/volumes (23xxx, vk-preview project).
                 Requires ~/.config/vk-preview/config.env.

  preview-full   Remote machine: Remote + Desktop-style services.
                 Full stack with independent VK_ASSET_DIR.

USAGE:
  ./scripts/vk-isolate.sh [mode] <command> [args...]

LITE MODE:
  ./scripts/vk-isolate.sh lite up           Start isolated instance
  ./scripts/vk-isolate.sh lite down         Stop
  ./scripts/vk-isolate.sh lite status       Show ports/pids
  ./scripts/vk-isolate.sh lite logs [n]     Tail logs
  ./scripts/vk-isolate.sh lite clean        Stop + delete .vk-test/

UNIT MODE:
  ./scripts/vk-isolate.sh unit [args...]    Run cargo tests

PREVIEW MODE:
  ./scripts/vk-isolate.sh preview-remote sync     Push branch to remote
  ./scripts/vk-isolate.sh preview-remote up       Start remote stack
  ./scripts/vk-isolate.sh preview-remote down     Stop remote stack
  ./scripts/vk-isolate.sh preview-remote status   Check remote status
  ./scripts/vk-isolate.sh preview-remote logs     Show remote logs
  ./scripts/vk-isolate.sh preview-remote smoke    Basic health checks
  ./scripts/vk-isolate.sh preview-remote clean    Stop + remove volumes

  ./scripts/vk-isolate.sh preview-full <cmd>      Same commands for full stack

CONFIGURATION:
  Preview mode requires ~/.config/vk-preview/config.env
  Example: scripts/vk-preview.config.env.example

EOF
}

# ── Main dispatch ─────────────────────────────────────────────────────────────
MODE="${1:-help}"

case "$MODE" in
    lite)
        shift
        lite_cmd "$@"
        ;;
    unit)
        shift
        unit_cmd "$@"
        ;;
    preview-remote|preview-full)
        CMD="${2:-help}"
        shift 2 2>/dev/null || true
        if [[ "$CMD" == "help" ]] || [[ -z "$CMD" ]]; then
            show_help
            exit 0
        fi
        remote_cmd "$MODE" "$CMD" "$@"
        ;;
    help|--help|-h|"")
        show_help
        ;;
    *)
        echo "Unknown mode: $MODE" >&2
        echo "" >&2
        show_help
        exit 1
        ;;
esac
