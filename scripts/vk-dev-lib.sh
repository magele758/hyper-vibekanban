#!/usr/bin/env bash
# Shared helpers for vk-start / vk-stop / vk-status (source, do not execute).

# shellcheck source=vk-ports.sh
source "$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)/vk-ports.sh"

# Local state lives outside any git checkout so multiple clones (e.g. vibe-kanban +
# hyper-vibekanban) share one SQLite DB, session logs, credentials, and keys.
# Main dev stack (13001/13002) uses its own cargo target dir so workspace agents
# running `cargo build/check` in a worktree never corrupt or lock the dev target/.
vk_configure_dev_cargo_target() {
  local state_dir="${VK_STATE_DIR:-${HOME}/.vk-kanban}"
  export CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-${VK_DEV_CARGO_TARGET_DIR:-${state_dir}/cargo-target-main}}"
  mkdir -p "${CARGO_TARGET_DIR}"
}

vk_configure_asset_dir() {
  local root="${1:?root required}"
  local state_dir="${VK_STATE_DIR:-${HOME}/.vk-kanban}"
  local shared="${VK_ASSET_DIR:-${state_dir}/dev_assets}"
  local repo_assets="${root}/dev_assets"

  mkdir -p "${shared}"

  if [[ ! -f "${shared}/db.v2.sqlite" && -f "${repo_assets}/db.v2.sqlite" ]]; then
    echo "==> 初始化共享 dev_assets: ${shared}"
    rsync -a "${repo_assets}/" "${shared}/"
  fi

  export VK_ASSET_DIR="${shared}"
}

vk_dev_pgrep_pattern() {
  echo "vibe-kanban.*concurrently.*local-web:dev|vibe-kanban.*concurrently.*backend:dev:watch"
}

vk_kill_port_listeners() {
  local port
  for port in "$@"; do
    local pids
    pids="$(lsof -ti "tcp:${port}" -sTCP:LISTEN 2>/dev/null || true)"
    if [[ -n "${pids}" ]]; then
      echo "Killing listeners on port ${port}..."
      # shellcheck disable=SC2086
      kill -9 ${pids} 2>/dev/null || true
    fi
  done
}

vk_write_ports() {
  local root="${1:?root required}"
  local frontend="${2:?frontend required}"
  local backend="${3:?backend required}"
  local preview_proxy="${4:?preview_proxy required}"
  python3 - <<PY
import json, datetime
path = "${root}/.dev-ports.json"
json.dump({
  "frontend": ${frontend},
  "backend": ${backend},
  "preview_proxy": ${preview_proxy},
  "timestamp": datetime.datetime.now(datetime.timezone.utc).isoformat().replace("+00:00", "Z"),
}, open(path, "w"), indent=2)
PY
}

vk_http_ok() {
  local url="$1"
  local code
  code="$(curl -s -o /dev/null -w '%{http_code}' --max-time 3 "${url}" 2>/dev/null || echo 000)"
  [[ "${code}" =~ ^[23][0-9]{2}$ ]]
}

# Vite dev server may listen on IPv6 localhost only; prefer localhost over 127.0.0.1 for FE.
vk_local_url() {
  local port="$1"
  local path="${2:-/}"
  echo "http://localhost:${port}${path}"
}

vk_read_runtime_backend_port() {
  local tmp="${TMPDIR:-/tmp}"
  local port_file="${tmp%/}/vibe-kanban/vibe-kanban.port"
  if [[ -f "${port_file}" ]]; then
    python3 -c "import json; print(json.load(open('${port_file}'))['main_port'])" 2>/dev/null || true
  fi
}

# Resolve a usable tailscale binary. The Mac App Store GUI binary works only
# when invoked via its full path (a PATH symlink crashes with SIGTRAP/"unknown
# bundleIdentifier"), so prefer the app path over whatever is on PATH.
vk_tailscale_bin() {
  if [[ -n "${VK_TAILSCALE_BIN:-}" && -x "${VK_TAILSCALE_BIN}" ]]; then
    echo "${VK_TAILSCALE_BIN}"
    return 0
  fi
  local app="/Applications/Tailscale.app/Contents/MacOS/Tailscale"
  if [[ -x "${app}" ]]; then
    echo "${app}"
    return 0
  fi
  command -v tailscale 2>/dev/null || true
}

# Run the resolved tailscale binary, swallowing any crash signal message. The
# inner bash reports "Trace/BPT trap" to its own (redirected) stderr and exits
# with a normal code, so the caller's shell never prints the signal notice.
vk_tailscale() {
  local bin
  bin="$(vk_tailscale_bin)"
  [[ -n "${bin}" ]] || return 127
  bash -c '"$0" "$@"' "${bin}" "$@" 2>/dev/null
}

vk_tailscale_ok() {
  vk_tailscale status >/dev/null 2>&1
}

# Best-effort, bounded wait for Tailscale to come up. At login the LaunchAgent
# fires as soon as Docker is ready, which can be before Tailscale connects — if
# vk-start runs first, VK_BROWSER_SHARED_API_BASE / VK_ALLOWED_ORIGINS fall back
# to the LAN IP and remote devices (cellular over Tailscale) can't reach the API.
# Tailscale is optional, so a timeout is logged, not fatal.
vk_wait_tailscale() {
  local timeout="${VK_TAILSCALE_WAIT_SEC:-60}"
  vk_tailscale_bin >/dev/null 2>&1 || return 0   # not installed → nothing to wait for
  local elapsed=0
  while ! vk_tailscale_ok; do
    if (( elapsed >= timeout )); then
      echo "WARN: Tailscale 未在 ${timeout}s 内就绪，按无 Tailscale 启动（仅 WiFi/LAN 可达）" >&2
      return 0
    fi
    sleep 3
    elapsed=$((elapsed + 3))
  done
  [[ "${elapsed}" -gt 0 ]] && echo "Tailscale ready after ${elapsed}s"
  return 0
}

# Configure (or disable) the OrbStack container HTTP proxy.
#
# VK_ORBSTACK_PROXY controls it:
#   0 / unset (default) → set to "none". Containers go direct. Correct when no
#       proxy runs, OR when a host proxy (Clash/mihomo) binds loopback only
#       (allow-lan: false) — then host.orb.internal:7897 is unreachable from
#       containers and every egress (incl. the relay token fetch) returns 502,
#       which breaks relay registration and project loading.
#   1            → use the default URL http://host.orb.internal:7897
#   http(s)://…  → use that URL verbatim (e.g. a proxy with allow-lan enabled)
#
# Only meaningful with OrbStack; a no-op when orbctl is absent.
vk_configure_orbstack_proxy() {
  command -v orbctl >/dev/null 2>&1 || return 0
  local setting="${VK_ORBSTACK_PROXY:-0}"
  local url
  case "${setting}" in
    0 | "" | none | off | false) url="none" ;;
    1 | on | true) url="http://host.orb.internal:7897" ;;
    http://* | https://*) url="${setting}" ;;
    *)
      echo "WARN: 无法识别 VK_ORBSTACK_PROXY='${setting}'，按 none 处理" >&2
      url="none"
      ;;
  esac
  if orbctl config set network_proxy "${url}" 2>/dev/null; then
    echo "OrbStack 容器代理: ${url}"
  else
    echo "WARN: 设置 OrbStack network_proxy='${url}' 失败（已忽略）" >&2
  fi
}

vk_detect_tailscale_ip() {
  vk_tailscale_ok || return 0
  vk_tailscale ip -4 2>/dev/null | head -1
}

vk_detect_tailscale_hostname() {
  vk_tailscale_ok || return 0
  vk_tailscale status --json 2>/dev/null \
    | python3 -c "import sys,json; print(json.load(sys.stdin)['Self']['DNSName'].rstrip('.'))" 2>/dev/null \
    || true
}

vk_configure_public_urls() {
  local frontend_port="${1:?frontend port required}"
  local lan_ip
  lan_ip="$(vk_detect_lan_ip)"
  local ts_ip
  ts_ip="$(vk_detect_tailscale_ip)"
  local ts_hostname
  ts_hostname="$(vk_detect_tailscale_hostname)"

  export VK_DEV_HOST="${VK_BIND_ADDR}"
  export VITE_RELAY_PORT="${VK_RELAY_PORT}"

  local allowed_origins="http://localhost:${frontend_port}"
  # Desktop h2 front door page origins (relay at the desktop relay front door is
  # a cross-origin request from the 13443 page, so its origin must be allowed).
  allowed_origins="${allowed_origins},https://localhost:${VK_DESKTOP_HTTPS_PORT},https://localhost:${VK_DESKTOP_RELAY_HTTPS_PORT}"

  # Server-side Rust client always talks to local Remote/Relay over loopback so
  # token refresh survives WiFi/VPN changes (stale LAN IPs caused 502 on
  # /api/auth/token). Browser-facing bases may still use LAN/Tailscale below.
  export VK_SHARED_API_BASE="http://127.0.0.1:${VK_REMOTE_PORT}"

  if [[ -n "${ts_ip}" ]]; then
    export VK_BROWSER_SHARED_API_BASE="http://${ts_ip}:${VK_REMOTE_PORT}"
  elif [[ -n "${lan_ip}" ]]; then
    export VK_BROWSER_SHARED_API_BASE="http://${lan_ip}:${VK_REMOTE_PORT}"
  else
    export VK_BROWSER_SHARED_API_BASE="http://localhost:${VK_REMOTE_PORT}"
  fi

  if [[ -n "${lan_ip}" ]]; then
    export PUBLIC_BASE_URL="http://${lan_ip}:${VK_REMOTE_PORT}"
    export VITE_VK_SHARED_API_BASE="http://${lan_ip}:${VK_REMOTE_PORT}"
    export VITE_RELAY_API_BASE_URL="http://${lan_ip}:${VK_RELAY_PORT}"
    allowed_origins="${allowed_origins},http://${lan_ip}:${frontend_port},http://${lan_ip}:${VK_REMOTE_PORT}"
  else
    export PUBLIC_BASE_URL="http://localhost:${VK_REMOTE_PORT}"
    export VITE_VK_SHARED_API_BASE="http://localhost:${VK_REMOTE_PORT}"
    export VITE_RELAY_API_BASE_URL="http://localhost:${VK_RELAY_PORT}"
  fi

  if [[ -n "${ts_ip}" ]]; then
    allowed_origins="${allowed_origins},http://${ts_ip}:${frontend_port},http://${ts_ip}:${VK_REMOTE_PORT},http://${ts_ip}:${VK_RELAY_PORT}"
  fi
  if [[ -n "${ts_hostname}" ]]; then
    allowed_origins="${allowed_origins},https://${ts_hostname}:${VK_MOBILE_HTTPS_PORT},https://${ts_hostname}:${VK_MOBILE_RELAY_HTTPS_PORT}"
  fi

  export VK_ALLOWED_ORIGINS="${allowed_origins}"

  # Local Rust server connects to relay on the same machine.
  export VK_SHARED_RELAY_API_BASE="http://127.0.0.1:${VK_RELAY_PORT}"
}

vk_detect_lan_ip() {
  local ip=""
  ip="$(ipconfig getifaddr en0 2>/dev/null || true)"
  if [[ -z "${ip}" ]]; then
    ip="$(python3 -c "
import socket
s = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
s.connect(('8.8.8.8', 80))
print(s.getsockname()[0])
" 2>/dev/null || true)"
  fi
  echo "${ip}"
}

vk_stop_local_dev() {
  local root="${1:?root required}"
  local pid_dir="${2:?pid_dir required}"

  local dev_pattern
  dev_pattern="$(vk_dev_pgrep_pattern)"
  if pgrep -f "${dev_pattern}" >/dev/null 2>&1; then
    echo "Stopping dev supervisor..."
    pkill -f "${dev_pattern}" 2>/dev/null || true
    sleep 2
    pkill -9 -f "${dev_pattern}" 2>/dev/null || true
  fi

  vk_kill_port_listeners "${VK_FRONTEND_PORT}" "${VK_BACKEND_PORT}" "${VK_PREVIEW_PROXY_PORT}"

  if [[ -f "${pid_dir}/dev.pid" ]]; then
    local pid
    pid="$(cat "${pid_dir}/dev.pid")"
    if kill -0 "${pid}" 2>/dev/null; then
      echo "Stopping dev (pid ${pid})..."
      kill -- "-${pid}" 2>/dev/null || kill "${pid}" 2>/dev/null || true
      sleep 2
      kill -9 -- "-${pid}" 2>/dev/null || kill -9 "${pid}" 2>/dev/null || true
    fi
    rm -f "${pid_dir}/dev.pid"
  fi

  pkill -f "vibe-kanban.*concurrently.*local-web:dev" 2>/dev/null || true
  pkill -f "vibe-kanban.*concurrently.*backend:dev:watch" 2>/dev/null || true
  pkill -f "${root}/packages/local-web.*vite" 2>/dev/null || true
  pkill -f "${root}/target/debug/server" 2>/dev/null || true
  sleep 2
  node "${root}/scripts/setup-dev-environment.js" clear >/dev/null 2>&1 || true
}

vk_read_ports() {
  local root="${1:?root required}"
  local ports_file="${root}/.dev-ports.json"
  if [[ -f "${ports_file}" ]]; then
    python3 -c "import json; p=json.load(open('${ports_file}')); print(p['frontend'], p['backend'], p.get('preview_proxy', p['backend']+1))" 2>/dev/null || true
  fi
}

vk_dev_running() {
  pgrep -f "$(vk_dev_pgrep_pattern)" >/dev/null 2>&1
}

# True when concurrently is up AND the local Rust API responds.
vk_local_dev_healthy() {
  local backend_port="${1:?backend port required}"
  vk_dev_running && vk_http_ok "http://127.0.0.1:${backend_port}/health"
}

vk_launch_dev_background() {
  local root="${1:?root required}"
  local runner="${root}/scripts/vk-run-dev.sh"
  local log_file="${2:?log required}"
  if command -v setsid >/dev/null 2>&1; then
    setsid bash "${runner}" >> "${log_file}" 2>&1 &
  else
    # macOS: new session so workspace killpg does not hit dev stack
    nohup python3 -c "import os; os.setsid(); os.execvp('bash', ['bash', '${runner}'])" >> "${log_file}" 2>&1 &
  fi
  echo $!
}

vk_dev_supervisor_alive() {
  local pid_dir="${1:?pid_dir required}"
  local pid=""
  if [[ -f "${pid_dir}/dev.pid" ]]; then
    pid="$(cat "${pid_dir}/dev.pid" 2>/dev/null || true)"
    if [[ -n "${pid}" ]] && kill -0 "${pid}" 2>/dev/null; then
      return 0
    fi
  fi
  if vk_dev_running; then
    return 0
  fi
  return 1
}
