#!/usr/bin/env bash
# Local desktop client connected to OrbStack remote + relay (full stack).
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "${ROOT}"

# shellcheck source=vk-ports.sh
source "${ROOT}/scripts/vk-ports.sh"

export VK_SHARED_API_BASE="${VK_SHARED_API_BASE:-http://localhost:${VK_REMOTE_PORT}}"
export VK_SHARED_RELAY_API_BASE="${VK_SHARED_RELAY_API_BASE:-http://localhost:${VK_RELAY_PORT}}"
export VITE_VK_SHARED_API_BASE="${VITE_VK_SHARED_API_BASE:-$VK_SHARED_API_BASE}"

# The backend's Rust reqwest client reads the macOS system proxy. If a VPN/proxy
# app (Clash, etc.) sets a system HTTP(S) proxy without loopback in its bypass
# list, the local server's calls to the Remote/Relay on 127.0.0.1 get routed
# through the proxy, which refuses loopback and returns an empty-body 502 —
# breaking /api/auth/token, relay registration, and project loading. Force every
# local target to bypass the proxy. Append to any inherited NO_PROXY.
# Include Tailscale MagicDNS suffix so HTTPS front doors
# (https://*.ts.net:13444) are not MITM'd by Clash when proxy is on — that
# produces Node's "Client network socket disconnected before secure TLS…".
VK_NO_PROXY_HOSTS="localhost,127.0.0.1,::1,host.orb.internal,.ts.net,100.64.0.0/10"
export NO_PROXY="${NO_PROXY:+${NO_PROXY},}${VK_NO_PROXY_HOSTS}"
export no_proxy="${no_proxy:+${no_proxy},}${VK_NO_PROXY_HOSTS}"

# Node does not use the macOS trust store. When tools hit the Caddy h2 front
# door (https://localhost:13443), point them at Caddy's local root CA.
# Prefer space-free symlink — NODE_EXTRA_CA_CERTS breaks if unquoted paths split.
CADDY_ROOT_CA="${HOME}/.vk-kanban/certs/caddy-root.crt"
CADDY_ROOT_CA_SRC="${HOME}/Library/Application Support/Caddy/pki/authorities/local/root.crt"
if [[ ! -f "${CADDY_ROOT_CA}" && -f "${CADDY_ROOT_CA_SRC}" ]]; then
  mkdir -p "${HOME}/.vk-kanban/certs"
  ln -sfn "${CADDY_ROOT_CA_SRC}" "${CADDY_ROOT_CA}"
fi
if [[ -f "${CADDY_ROOT_CA}" ]]; then
  export NODE_EXTRA_CA_CERTS="${NODE_EXTRA_CA_CERTS:-${CADDY_ROOT_CA}}"
fi

# Re-enable relay (dev:lite turns it off)
node -e "
const fs=require('fs');
const p='dev_assets/config.json';
const c=JSON.parse(fs.readFileSync(p,'utf8'));
c.relay_enabled=true;
c.remote_onboarding_acknowledged=true;
fs.writeFileSync(p, JSON.stringify(c,null,2));
console.log('config: relay_enabled=true');
"

echo "Remote API:  ${VK_SHARED_API_BASE}"
echo "Relay API:   ${VK_SHARED_RELAY_API_BASE}"
echo "Local web:   see .dev-ports.json after start"
echo "Login:       admin@local.dev / devpass123 (SELF_HOST on remote-web)"
echo ""

# shellcheck source=vk-dev-lib.sh
source "${ROOT}/scripts/vk-dev-lib.sh"
vk_configure_dev_cargo_target

export VK_DEV_HOST="${VK_BIND_ADDR}"
export FRONTEND_PORT="${FRONTEND_PORT:-${VK_FRONTEND_PORT}}"
export BACKEND_PORT="${BACKEND_PORT:-${VK_BACKEND_PORT}}"
export PREVIEW_PROXY_PORT="${PREVIEW_PROXY_PORT:-${VK_PREVIEW_PROXY_PORT}}"
unset PORT
node scripts/setup-dev-environment.js get >/dev/null 2>&1 || true

pnpm run dev
