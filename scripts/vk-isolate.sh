#!/usr/bin/env bash
# vk-isolate — unified entry point for isolated testing (no impact on main service).
#
# Four modes:
#   lite           Local lite-mode instance (ports 14001+, state in .vk-test/)
#   unit           Local cargo/npm unit tests (no servers)
#   preview-remote Remote machine: Remote Docker stack only (independent ports/volumes)
#   preview-full   Remote machine: Remote + Desktop-style services (full stack)
#
# Preview modes need SSH Host + remote repo dir from the user (conversation),
# passed as flags or env — there is NO config file.
#
# Usage:
#   ./scripts/vk-isolate.sh lite up|down|status|logs|clean
#   ./scripts/vk-isolate.sh unit [cargo-test-args...]
#   ./scripts/vk-isolate.sh preview-remote up --host <ssh-host> --dir <abs-path-on-remote>
#   ./scripts/vk-isolate.sh preview-full <cmd> --host ... --dir ...
#
# Env alternative (same meaning as flags):
#   VK_PREVIEW_HOST  VK_PREVIEW_DIR
# Optional: VK_PREVIEW_GIT_REMOTE (default origin)
#           VK_PREVIEW_SYNC_METHOD (git|rsync, default git)
#           VK_PREVIEW_PORTS_BASE (default 23000)

set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"

# ── Defaults (no file load) ───────────────────────────────────────────────────
: "${VK_PREVIEW_HOST:=}"
: "${VK_PREVIEW_DIR:=}"
: "${VK_PREVIEW_GIT_REMOTE:=origin}"
: "${VK_PREVIEW_SYNC_METHOD:=git}"
: "${VK_PREVIEW_PORTS_BASE:=23000}"

# ── Parse --host / --dir from remaining args ──────────────────────────────────
parse_preview_flags() {
    local -a kept=()
    while [[ $# -gt 0 ]]; do
        case "$1" in
            --host)
                [[ $# -ge 2 ]] || { echo "ERROR: --host needs a value" >&2; exit 1; }
                VK_PREVIEW_HOST="$2"
                shift 2
                ;;
            --dir)
                [[ $# -ge 2 ]] || { echo "ERROR: --dir needs a value" >&2; exit 1; }
                VK_PREVIEW_DIR="$2"
                shift 2
                ;;
            --host=*)
                VK_PREVIEW_HOST="${1#--host=}"
                shift
                ;;
            --dir=*)
                VK_PREVIEW_DIR="${1#--dir=}"
                shift
                ;;
            *)
                kept+=("$1")
                shift
                ;;
        esac
    done
    PREVIEW_EXTRA_ARGS=("${kept[@]+"${kept[@]}"}")
}

require_preview_params() {
    if [[ -z "$VK_PREVIEW_HOST" || -z "$VK_PREVIEW_DIR" ]]; then
        echo "ERROR: preview 需要 SSH Host 与远端仓库绝对路径。" >&2
        echo "  在对话里向用户确认后传入，例如：" >&2
        echo "    ./scripts/vk-isolate.sh preview-remote up --host <ssh-host> --dir /abs/path/on/remote" >&2
        echo "  或：" >&2
        echo "    VK_PREVIEW_HOST=... VK_PREVIEW_DIR=... ./scripts/vk-isolate.sh preview-remote up" >&2
        echo "  不要写配置文件；不要猜测 Host。" >&2
        exit 1
    fi
    if [[ "$VK_PREVIEW_DIR" == ~* ]] || [[ "$VK_PREVIEW_DIR" == .* ]]; then
        echo "ERROR: --dir 请使用远端绝对路径（不要用 ~ 或相对路径）。" >&2
        exit 1
    fi
    if ! ssh -o BatchMode=yes -o ConnectTimeout=5 "$VK_PREVIEW_HOST" true 2>/dev/null; then
        echo "ERROR: Cannot SSH to $VK_PREVIEW_HOST (BatchMode)。检查 ~/.ssh/config 与密钥。" >&2
        exit 1
    fi
    export VK_PREVIEW_HOST VK_PREVIEW_DIR VK_PREVIEW_GIT_REMOTE
    export VK_PREVIEW_SYNC_METHOD VK_PREVIEW_PORTS_BASE
}

sync_to_remote() {
    require_preview_params

    local branch
    branch="$(git rev-parse --abbrev-ref HEAD)"
    echo "==> Syncing branch '$branch' to $VK_PREVIEW_HOST:$VK_PREVIEW_DIR"

    if [[ "$VK_PREVIEW_SYNC_METHOD" == "rsync" ]]; then
        echo "  Using rsync..."
        rsync -azP --exclude='.git' --exclude='node_modules' --exclude='target' \
            --exclude='.vk-test' --exclude='dev_assets' --exclude='vendor' \
            "$ROOT/" "$VK_PREVIEW_HOST:$VK_PREVIEW_DIR/"
    else
        echo "  Using git (push + remote pull)..."
        git push "$VK_PREVIEW_GIT_REMOTE" "$branch" || {
            echo "WARN: git push failed. Continuing (remote may already have the branch, or use rsync)."
        }
        if ! ssh "$VK_PREVIEW_HOST" \
            "cd '$VK_PREVIEW_DIR' && git fetch '$VK_PREVIEW_GIT_REMOTE' && git checkout '$branch' && git pull --ff-only '$VK_PREVIEW_GIT_REMOTE' '$branch'"; then
            echo "WARN: remote git pull failed. Set VK_PREVIEW_SYNC_METHOD=rsync and retry, or fix network on the preview host." >&2
            return 1
        fi
    fi
    echo "  Sync complete."
}

remote_cmd() {
    local mode="$1"
    local cmd="$2"
    shift 2 || true
    parse_preview_flags "$@"
    # shellcheck disable=SC2034
    set -- "${PREVIEW_EXTRA_ARGS[@]+"${PREVIEW_EXTRA_ARGS[@]}"}"

    case "$cmd" in
        sync)
            sync_to_remote
            ;;
        up)
            sync_to_remote
            ssh -t "$VK_PREVIEW_HOST" \
              "cd '$VK_PREVIEW_DIR' && VK_PREVIEW_PORTS_BASE='${VK_PREVIEW_PORTS_BASE}' bash scripts/vk-preview-remote.sh up"
            ;;
        down|status|logs|smoke|clean)
            require_preview_params
            ssh -t "$VK_PREVIEW_HOST" \
              "cd '$VK_PREVIEW_DIR' && VK_PREVIEW_PORTS_BASE='${VK_PREVIEW_PORTS_BASE}' bash scripts/vk-preview-remote.sh '$cmd'"
            ;;
        *)
            echo "Unknown preview command: $cmd" >&2
            echo "Available: sync, up, down, status, logs, smoke, clean" >&2
            exit 1
            ;;
    esac
}

lite_cmd() {
    local cmd="${1:-help}"
    shift 2>/dev/null || true
    exec "${ROOT}/scripts/vk-test-feature.sh" "$cmd" "$@"
}

unit_cmd() {
    cd "$ROOT"
    echo "==> Running unit tests (workspace-manager, local-deployment, db)..."
    exec cargo test -p workspace-manager -p local-deployment -p db "$@"
}

show_help() {
    cat <<EOF
vk-isolate — unified entry point for isolated testing

MODES:
  lite           Local lite-mode (ports 14001+); main :13001 unaffected
  unit           Local cargo tests
  preview-remote Remote Docker Remote stack (23xxx) — needs --host and --dir
  preview-full   Remote full stack — needs --host and --dir

PREVIEW (ask the user in chat; pass flags; NO config file):
  ./scripts/vk-isolate.sh preview-remote up --host <ssh-host> --dir /abs/path/on/remote
  ./scripts/vk-isolate.sh preview-remote sync|down|status|logs|smoke|clean --host ... --dir ...

  Optional env: VK_PREVIEW_GIT_REMOTE VK_PREVIEW_SYNC_METHOD=git|rsync VK_PREVIEW_PORTS_BASE

LITE:
  ./scripts/vk-isolate.sh lite up|down|status|logs|clean

UNIT:
  ./scripts/vk-isolate.sh unit [cargo-test-args...]

EOF
}

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
        if [[ "$CMD" == "help" || -z "$CMD" ]]; then
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
        show_help
        exit 1
        ;;
esac
