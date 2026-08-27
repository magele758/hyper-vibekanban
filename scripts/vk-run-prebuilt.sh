#!/usr/bin/env bash
# Daily-use Desktop: prebuilt server binary + Vite (no cargo-watch).
# Env must be exported by vk-start before invocation.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "${ROOT}"

# shellcheck source=vk-ports.sh
source "${ROOT}/scripts/vk-ports.sh"
# shellcheck source=vk-dev-lib.sh
source "${ROOT}/scripts/vk-dev-lib.sh"

SERVER_BIN="${VK_SERVER_BIN:-$(vk_resolve_server_bin)}"
if [[ -z "${SERVER_BIN}" || ! -x "${SERVER_BIN}" ]]; then
  echo "ERROR: 找不到预编译 server。放到 ~/.vk-kanban/bin/server，或 VK_HOT=1 走 cargo-watch。" >&2
  exit 1
fi

export VK_SHARED_API_BASE="${VK_SHARED_API_BASE:-http://localhost:${VK_REMOTE_PORT}}"
export VK_SHARED_RELAY_API_BASE="${VK_SHARED_RELAY_API_BASE:-http://localhost:${VK_RELAY_PORT}}"
export VITE_VK_SHARED_API_BASE="${VITE_VK_SHARED_API_BASE:-$VK_SHARED_API_BASE}"

VK_NO_PROXY_HOSTS="localhost,127.0.0.1,::1,host.orb.internal,.ts.net,100.64.0.0/10"
export NO_PROXY="${NO_PROXY:+${NO_PROXY},}${VK_NO_PROXY_HOSTS}"
export no_proxy="${no_proxy:+${no_proxy},}${VK_NO_PROXY_HOSTS}"

CADDY_ROOT_CA="${HOME}/.vk-kanban/certs/caddy-root.crt"
CADDY_ROOT_CA_SRC="${HOME}/Library/Application Support/Caddy/pki/authorities/local/root.crt"
if [[ ! -f "${CADDY_ROOT_CA}" && -f "${CADDY_ROOT_CA_SRC}" ]]; then
  mkdir -p "${HOME}/.vk-kanban/certs"
  ln -sfn "${CADDY_ROOT_CA_SRC}" "${CADDY_ROOT_CA}"
fi
if [[ -f "${CADDY_ROOT_CA}" ]]; then
  export NODE_EXTRA_CA_CERTS="${NODE_EXTRA_CA_CERTS:-${CADDY_ROOT_CA}}"
fi

SHARED_CONFIG="${VK_ASSET_DIR:-${HOME}/.vk-kanban/dev_assets}/config.json"
if [[ -f "${SHARED_CONFIG}" ]]; then
  python3 - "${SHARED_CONFIG}" <<'PY'
import json, sys
p = sys.argv[1]
with open(p, encoding="utf-8") as f:
    c = json.load(f)
c["relay_enabled"] = True
c["remote_onboarding_acknowledged"] = True
with open(p, "w", encoding="utf-8") as f:
    json.dump(c, f, indent=2)
    f.write("\n")
print("config: relay_enabled=true")
PY
fi

export VK_DEV_HOST="${VK_DEV_HOST:-${VK_BIND_ADDR}}"
export HOST="${HOST:-127.0.0.1}"
export FRONTEND_PORT="${FRONTEND_PORT:-${VK_FRONTEND_PORT}}"
export BACKEND_PORT="${BACKEND_PORT:-${VK_BACKEND_PORT}}"
export PREVIEW_PROXY_PORT="${PREVIEW_PROXY_PORT:-${VK_PREVIEW_PROXY_PORT}}"
export DISABLE_WORKTREE_CLEANUP="${DISABLE_WORKTREE_CLEANUP:-1}"
export RUST_LOG="${RUST_LOG:-info}"
unset PORT
node scripts/setup-dev-environment.js get >/dev/null 2>&1 || true

echo "Prebuilt server: ${SERVER_BIN}"
echo "Remote API:      ${VK_SHARED_API_BASE}"
echo "Relay API:       ${VK_SHARED_RELAY_API_BASE}"

exec pnpm exec concurrently \
  --names server,web \
  "${SERVER_BIN}" \
  "pnpm run local-web:dev:supervised"
