#!/usr/bin/env bash
# Start remote stack (detached). Pre-pulls base images to avoid stuck Hub downloads.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "${ROOT}"

# shellcheck source=vk-dev-lib.sh
source "${ROOT}/scripts/vk-dev-lib.sh"

if [[ ! -f crates/remote/.env.remote ]]; then
  echo "Missing crates/remote/.env.remote — see crates/remote/README.md"
  exit 1
fi

# Use Clash/system proxy inside OrbStack VM when available
if command -v orbctl >/dev/null 2>&1; then
  orbctl config set network_proxy http://host.orb.internal:7897 2>/dev/null || true
fi

echo "==> Pre-pulling base images (mirror)..."
bash scripts/pull-remote-docker-images.sh

# npm/apt/cargo mirrors for builds inside Docker (override via env if needed)
export NPM_REGISTRY="${NPM_REGISTRY:-https://registry.npmmirror.com}"
export APT_MIRROR="${APT_MIRROR:-mirrors.aliyun.com}"
export CARGO_RS_PROXY="${CARGO_RS_PROXY:-1}"
export HTTP_PROXY="${HTTP_PROXY:-http://host.orb.internal:7897}"
export HTTPS_PROXY="${HTTPS_PROXY:-http://host.orb.internal:7897}"
export NO_PROXY="${NO_PROXY:-localhost,127.0.0.1,remote-db,electric,azurite}"

echo "==> Build mirrors: npm=${NPM_REGISTRY} apt=${APT_MIRROR} cargo_rsproxy=${CARGO_RS_PROXY}"
echo "==> Building and starting remote + relay..."
cd crates/remote
export DOCKER_BUILDKIT=1
export COMPOSE_DOCKER_CLI_BUILD=1
export REMOTE_SERVER_PORTS="${VK_BIND_ADDR}:${VK_REMOTE_PORT}:8081"
export REMOTE_RELAY_PORTS="${VK_BIND_ADDR}:${VK_RELAY_PORT}:8082"
export REMOTE_DB_PORTS="127.0.0.1:${VK_REMOTE_DB_PORT}:5432"
LAN_IP="$(vk_detect_lan_ip)"
export PUBLIC_BASE_URL="http://${LAN_IP:-localhost}:${VK_REMOTE_PORT}"
docker compose --env-file .env.remote --profile relay up --build -d

echo "==> Waiting for health..."
for i in $(seq 1 60); do
  remote_ok=0
  relay_ok=0
  curl -sf "http://127.0.0.1:${VK_REMOTE_PORT}/v1/health" >/dev/null 2>&1 && remote_ok=1
  curl -sf "http://127.0.0.1:${VK_RELAY_PORT}/health" >/dev/null 2>&1 && relay_ok=1
  if [[ "${remote_ok}" -eq 1 && "${relay_ok}" -eq 1 ]]; then
    echo ""
    echo "Remote stack is up:"
    echo "  Remote UI/API: http://localhost:${VK_REMOTE_PORT}"
    echo "  Relay:         http://localhost:${VK_RELAY_PORT}"
    echo "  Login:         admin@local.dev / devpass123"
    exit 0
  fi
  printf "."
  sleep 5
done

echo ""
echo "Health check timed out. Recent logs:"
docker compose --env-file .env.remote --profile relay ps
docker compose --env-file .env.remote --profile relay logs --tail 30 remote-server relay-server
exit 1
