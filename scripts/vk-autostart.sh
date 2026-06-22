#!/usr/bin/env bash
# Login-time bootstrap: wait for OrbStack/Docker, then vk-start.
# Invoked by LaunchAgent (see vk-install-autostart.sh). Do not run manually
# unless debugging — use vk-start instead.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
export VK_REPO="${VK_REPO:-${ROOT}}"

# launchd does not load ~/.zshrc; cover Homebrew, OrbStack, common node managers.
export PATH="${HOME}/.orbstack/bin:/opt/homebrew/bin:/usr/local/bin:${PATH}"
if [[ -d "${HOME}/.fnm" ]]; then
  export PATH="${HOME}/.fnm/current/bin:${PATH}"
fi
if [[ -d "${HOME}/.nvm/versions/node" ]]; then
  # shellcheck disable=SC2012
  NVM_NODE="$(ls -1d "${HOME}/.nvm/versions/node/"*/bin 2>/dev/null | tail -1 || true)"
  [[ -n "${NVM_NODE}" ]] && export PATH="${NVM_NODE}:${PATH}"
fi
# Rust (rustup): launchd lacks ~/.cargo/bin, which backend:dev:watch needs
# (cargo / cargo-watch). Without it the Rust backend dies at boot with
# "cargo: command not found" and concurrently tears down the whole dev process,
# so local web/API never come up (Docker survives because docker is on PATH).
if [[ -f "${HOME}/.cargo/env" ]]; then
  # shellcheck disable=SC1091
  source "${HOME}/.cargo/env"
elif [[ -d "${HOME}/.cargo/bin" ]]; then
  export PATH="${HOME}/.cargo/bin:${PATH}"
fi

STATE_DIR="${VK_STATE_DIR:-${HOME}/.vk-kanban}"
LOG_DIR="${STATE_DIR}/logs"
PID_DIR="${STATE_DIR}/pids"
mkdir -p "${LOG_DIR}" "${PID_DIR}"

AUTOSTART_LOCK="${PID_DIR}/vk-autostart.lock.d"
if ! mkdir "${AUTOSTART_LOCK}" 2>/dev/null; then
  echo "vk-autostart already running, exit"
  exit 0
fi
trap 'rm -rf "${AUTOSTART_LOCK}"' EXIT

AUTOSTART_ENV="${STATE_DIR}/autostart.env"
if [[ -f "${AUTOSTART_ENV}" ]]; then
  # shellcheck disable=SC1090
  source "${AUTOSTART_ENV}"
fi

exec >>"${LOG_DIR}/autostart.log" 2>&1
echo "=== vk-autostart $(date -Iseconds) VK_REPO=${VK_REPO} ==="

DOCKER_WAIT_SEC="${VK_DOCKER_WAIT_SEC:-300}"
elapsed=0
until docker info >/dev/null 2>&1; do
  if (( elapsed >= DOCKER_WAIT_SEC )); then
    echo "ERROR: docker/OrbStack not ready after ${DOCKER_WAIT_SEC}s"
    exit 1
  fi
  sleep 5
  elapsed=$((elapsed + 5))
done
echo "Docker ready after ${elapsed}s"

# At login Tailscale often connects a little after Docker; wait briefly so the
# baked browser API base + CORS origins cover the Tailscale host (otherwise
# remote/cellular devices on the tailnet can't reach the API). Best-effort.
# (OrbStack container proxy is configured by vk-start via VK_ORBSTACK_PROXY.)
# shellcheck source=vk-dev-lib.sh
source "${VK_REPO}/scripts/vk-dev-lib.sh"
vk_wait_tailscale

bash "${VK_REPO}/scripts/vk-start.sh"
echo "=== vk-autostart finished $(date -Iseconds) ==="
