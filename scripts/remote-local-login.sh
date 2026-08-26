#!/usr/bin/env bash
#
# remote-local-login.sh — bring up a minimal, Docker-free self-hosted Remote
# backend so the local app can sign in with email/password (local auth).
#
# What it starts:
#   1. A user-owned PostgreSQL cluster (no root, no Docker) under $VK_REMOTE_HOME.
#   2. The `remote` server (crates/remote) with SELF_HOST local auth enabled.
#
# Login (account) works with this alone. Real-time board sync additionally needs
# ElectricSQL, which is distributed as a Docker image — see the note printed by
# `status`. This script deliberately does NOT touch the main vk-* stack or its
# ports (13000-13003, 18082, ...); it uses Postgres :5432 and Remote :8081.
#
# Usage:
#   bash scripts/remote-local-login.sh up           # start backend + connect the app
#   bash scripts/remote-local-login.sh start        # start Postgres + Remote only
#   bash scripts/remote-local-login.sh connect-dev   # (re)start `pnpm run dev` pointed at Remote
#   bash scripts/remote-local-login.sh status
#   bash scripts/remote-local-login.sh stop
#
# Config (env overrides, all optional):
#   VK_REMOTE_HOME            state dir            (default: $HOME/.vk-remote)
#   VK_REMOTE_PG_PORT         postgres port        (default: 5432)
#   VK_REMOTE_LISTEN          remote listen addr   (default: 127.0.0.1:8081)
#   VK_REMOTE_LOGIN_EMAIL     local auth email     (default: demo@vibe.local)
#   VK_REMOTE_LOGIN_PASSWORD  local auth password  (default: demo-password-123)
#   VK_REMOTE_ELECTRIC_URL    electric url (sync)  (default: http://localhost:9999 placeholder)
#
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "${ROOT}"

VK_REMOTE_HOME="${VK_REMOTE_HOME:-${HOME}/.vk-remote}"
PGDATA="${VK_REMOTE_HOME}/pgdata"
PGSOCK="${VK_REMOTE_HOME}/pgsock"
PGPORT="${VK_REMOTE_PG_PORT:-5432}"
REMOTE_LISTEN="${VK_REMOTE_LISTEN:-127.0.0.1:8081}"
REMOTE_URL="http://${REMOTE_LISTEN/#0.0.0.0:/localhost:}"
LOGIN_EMAIL="${VK_REMOTE_LOGIN_EMAIL:-demo@vibe.local}"
LOGIN_PASSWORD="${VK_REMOTE_LOGIN_PASSWORD:-demo-password-123}"
ELECTRIC_URL="${VK_REMOTE_ELECTRIC_URL:-http://localhost:9999}"
JWT_FILE="${VK_REMOTE_HOME}/jwt.secret"
REMOTE_LOG="${VK_REMOTE_HOME}/remote-server.log"
REMOTE_PID="${VK_REMOTE_HOME}/remote-server.pid"
DEV_LOG="${VK_REMOTE_HOME}/dev.log"
DEV_PID="${VK_REMOTE_HOME}/dev.pid"
BILLING_STUB="/tmp/billing-stub" # path is hard-coded in crates/remote/Cargo.toml

log() { printf '\033[36m==>\033[0m %s\n' "$*"; }
warn() { printf '\033[33m[warn]\033[0m %s\n' "$*" >&2; }
die() { printf '\033[31m[error]\033[0m %s\n' "$*" >&2; exit 1; }

pg_bin() {
  local d
  d="$(ls -d /usr/lib/postgresql/*/bin 2>/dev/null | sort -V | tail -1 || true)"
  [ -n "${d}" ] && { echo "${d}"; return; }
  command -v pg_ctl >/dev/null 2>&1 && { dirname "$(command -v pg_ctl)"; return; }
  echo ""
}

ensure_deps() {
  local need_pg=0 need_ssl=0
  local bin; bin="$(pg_bin)"
  [ -z "${bin}" ] && need_pg=1
  pkg-config --exists openssl 2>/dev/null || need_ssl=1
  if [ "${need_pg}" -eq 1 ] || [ "${need_ssl}" -eq 1 ]; then
    if command -v sudo >/dev/null 2>&1 && command -v apt-get >/dev/null 2>&1; then
      log "Installing system dependencies (postgresql, libssl-dev)..."
      sudo apt-get update -qq
      local pkgs=()
      [ "${need_pg}" -eq 1 ] && pkgs+=(postgresql postgresql-contrib)
      [ "${need_ssl}" -eq 1 ] && pkgs+=(libssl-dev pkg-config)
      sudo DEBIAN_FRONTEND=noninteractive apt-get install -y -qq "${pkgs[@]}"
    else
      die "Missing PostgreSQL and/or libssl-dev, and cannot auto-install (need sudo+apt-get). Install them manually."
    fi
  fi
}

pg_env() { PGBIN="$(pg_bin)"; [ -n "${PGBIN}" ] || die "PostgreSQL not found"; }

pg_running() {
  pg_env
  "${PGBIN}/pg_isready" -h "${PGSOCK}" -p "${PGPORT}" >/dev/null 2>&1
}

start_pg() {
  pg_env
  mkdir -p "${VK_REMOTE_HOME}" "${PGSOCK}"
  if [ ! -s "${PGDATA}/PG_VERSION" ]; then
    log "Initializing PostgreSQL cluster at ${PGDATA}"
    "${PGBIN}/initdb" -D "${PGDATA}" -U postgres --auth=trust >/dev/null
    cat >> "${PGDATA}/postgresql.conf" <<EOF
listen_addresses = 'localhost'
port = ${PGPORT}
wal_level = logical
unix_socket_directories = '${PGSOCK}'
EOF
  fi
  if pg_running; then
    log "PostgreSQL already running on :${PGPORT}"
  else
    log "Starting PostgreSQL on :${PGPORT}"
    "${PGBIN}/pg_ctl" -D "${PGDATA}" -l "${VK_REMOTE_HOME}/pg.log" -w start >/dev/null
  fi
  # Idempotent role + database creation.
  "${PGBIN}/psql" -U postgres -h "${PGSOCK}" -p "${PGPORT}" -d postgres -tAc \
    "SELECT 1 FROM pg_roles WHERE rolname='remote'" | grep -q 1 || \
    "${PGBIN}/psql" -U postgres -h "${PGSOCK}" -p "${PGPORT}" -d postgres -q \
      -c "CREATE ROLE remote WITH LOGIN SUPERUSER PASSWORD 'remote'"
  "${PGBIN}/psql" -U postgres -h "${PGSOCK}" -p "${PGPORT}" -d postgres -tAc \
    "SELECT 1 FROM pg_database WHERE datname='remote'" | grep -q 1 || \
    "${PGBIN}/psql" -U postgres -h "${PGSOCK}" -p "${PGPORT}" -d postgres -q \
      -c "CREATE DATABASE remote OWNER remote"
}

ensure_jwt() {
  if [ ! -s "${JWT_FILE}" ]; then
    log "Generating JWT secret"
    head -c 48 /dev/urandom | base64 | tr -d '\n' > "${JWT_FILE}"
  fi
}

ensure_billing_stub() {
  # crates/remote/Cargo.toml declares an optional `billing` dep at /tmp/billing-stub.
  # It is NOT compiled (vk-billing feature is off), but cargo must resolve its manifest.
  mkdir -p "${BILLING_STUB}/src"
  cat > "${BILLING_STUB}/Cargo.toml" <<'EOF'
[package]
name = "billing"
version = "0.1.0"
edition = "2021"

[lib]
path = "src/lib.rs"
EOF
  echo "// stub billing crate (unused: vk-billing feature is off)" > "${BILLING_STUB}/src/lib.rs"
}

remote_running() {
  local port="${REMOTE_LISTEN##*:}"
  curl -fs -m 3 "http://127.0.0.1:${port}/v1/auth/methods" >/dev/null 2>&1
}

start_remote() {
  ensure_billing_stub
  ensure_jwt
  log "Building remote server (SQLX_OFFLINE)"
  SQLX_OFFLINE=true cargo build --manifest-path crates/remote/Cargo.toml --bin remote >/dev/null
  if remote_running; then
    log "Remote server already running on ${REMOTE_URL}"
    return
  fi
  log "Starting remote server on ${REMOTE_LISTEN}"
  local jwt; jwt="$(cat "${JWT_FILE}")"
  RUST_LOG="info,remote=info" \
  SERVER_DATABASE_URL="postgres://remote:remote@localhost:${PGPORT}/remote" \
  SERVER_LISTEN_ADDR="${REMOTE_LISTEN}" \
  SERVER_PUBLIC_BASE_URL="${REMOTE_URL}" \
  ELECTRIC_URL="${ELECTRIC_URL}" \
  VIBEKANBAN_REMOTE_JWT_SECRET="${jwt}" \
  SELF_HOST_LOCAL_AUTH_EMAIL="${LOGIN_EMAIL}" \
  SELF_HOST_LOCAL_AUTH_PASSWORD="${LOGIN_PASSWORD}" \
    setsid ./crates/remote/target/debug/remote >"${REMOTE_LOG}" 2>&1 &
  echo $! > "${REMOTE_PID}"
  local i
  for i in $(seq 1 60); do
    remote_running && { log "Remote server is up"; return; }
    sleep 1
  done
  warn "Remote server did not become ready in time; see ${REMOTE_LOG}"
  tail -20 "${REMOTE_LOG}" >&2 || true
  return 1
}

kill_port() {
  # Kill the specific PID(s) listening on a TCP port (never uses pkill -f).
  local port="$1" pids
  pids="$(ss -ltnpH "sport = :${port}" 2>/dev/null | grep -oP 'pid=\K[0-9]+' | sort -u || true)"
  [ -z "${pids}" ] && command -v lsof >/dev/null 2>&1 && pids="$(lsof -ti tcp:"${port}" 2>/dev/null || true)"
  for p in ${pids}; do
    kill "${p}" 2>/dev/null || true
  done
  [ -n "${pids}" ] && sleep 2 || true
  for p in ${pids}; do
    kill -9 "${p}" 2>/dev/null || true
  done
}

connect_dev() {
  remote_running || die "Remote server is not running; run 'start' first."
  local fe be pp
  if [ -f "${ROOT}/.dev-ports.json" ]; then
    fe="$(grep -oP '"frontend"\s*:\s*\K[0-9]+' "${ROOT}/.dev-ports.json" 2>/dev/null || echo 3000)"
    be="$(grep -oP '"backend"\s*:\s*\K[0-9]+' "${ROOT}/.dev-ports.json" 2>/dev/null || echo 3001)"
    pp="$(grep -oP '"preview_proxy"\s*:\s*\K[0-9]+' "${ROOT}/.dev-ports.json" 2>/dev/null || echo 3002)"
  else
    fe=3000; be=3001; pp=3002
  fi
  log "Stopping any existing dev servers on :${fe} :${be} :${pp}"
  kill_port "${fe}"; kill_port "${be}"; kill_port "${pp}"
  log "Starting 'pnpm run dev' pointed at Remote (${REMOTE_URL})"
  SQLX_OFFLINE=true HOST=0.0.0.0 VK_SHARED_API_BASE="${REMOTE_URL}" \
    setsid pnpm run dev >"${DEV_LOG}" 2>&1 &
  echo $! > "${DEV_PID}"
  local i
  for i in $(seq 1 90); do
    if grep -q "Remote client initialized" "${DEV_LOG}" 2>/dev/null; then
      log "Local app connected to Remote. Frontend: http://localhost:${fe}"
      return
    fi
    sleep 1
  done
  warn "Dev did not report a Remote connection in time; see ${DEV_LOG}"
  tail -20 "${DEV_LOG}" >&2 || true
}

stop_all() {
  # Prefer stopping process groups we started (setsid group leaders), which also
  # tears down the Vite supervisor + its children.
  if [ -f "${DEV_PID}" ]; then
    kill -- "-$(cat "${DEV_PID}")" 2>/dev/null || kill "$(cat "${DEV_PID}")" 2>/dev/null || true
    rm -f "${DEV_PID}"
  fi
  if [ -f "${REMOTE_PID}" ]; then
    kill -- "-$(cat "${REMOTE_PID}")" 2>/dev/null || kill "$(cat "${REMOTE_PID}")" 2>/dev/null || true
    rm -f "${REMOTE_PID}"
  fi
  # Port-based fallback for anything left listening (specific PIDs only).
  sleep 1
  kill_port "${REMOTE_LISTEN##*:}"
  if [ -f "${ROOT}/.dev-ports.json" ]; then
    local fe be pp
    fe="$(grep -oP '"frontend"\s*:\s*\K[0-9]+' "${ROOT}/.dev-ports.json" 2>/dev/null || echo 3000)"
    be="$(grep -oP '"backend"\s*:\s*\K[0-9]+' "${ROOT}/.dev-ports.json" 2>/dev/null || echo 3001)"
    pp="$(grep -oP '"preview_proxy"\s*:\s*\K[0-9]+' "${ROOT}/.dev-ports.json" 2>/dev/null || echo 3002)"
    kill_port "${fe}"; kill_port "${be}"; kill_port "${pp}"
  fi
  if pg_running; then
    pg_env
    "${PGBIN}/pg_ctl" -D "${PGDATA}" -m fast stop >/dev/null 2>&1 || true
  fi
  log "Stopped remote server, dev (if started here), and PostgreSQL."
}

print_summary() {
  cat <<EOF

------------------------------------------------------------------
 Self-hosted Remote (login) is ready.
   Remote API : ${REMOTE_URL}
   Login email: ${LOGIN_EMAIL}
   Login pass : ${LOGIN_PASSWORD}

 Point the local app at it (if not using 'connect-dev'):
   VK_SHARED_API_BASE=${REMOTE_URL} pnpm run dev

 Then sign in via the app's account menu -> "Sign in" -> "Sign in with email".

 Note: real-time board SYNC additionally needs ElectricSQL (a Docker image,
 electricsql/electric). Account LOGIN works without it. To enable sync where
 Docker is available, use the full stack: 'pnpm run remote:dev'.
------------------------------------------------------------------
EOF
}

status() {
  if pg_running; then echo "PostgreSQL : UP (:${PGPORT})"; else echo "PostgreSQL : down"; fi
  if remote_running; then
    echo "Remote     : UP (${REMOTE_URL})"
    echo -n "auth/methods: "; curl -fs -m 3 "${REMOTE_URL}/v1/auth/methods" || true; echo
  else
    echo "Remote     : down"
  fi
}

cmd="${1:-up}"
case "${cmd}" in
  up)
    ensure_deps; start_pg; start_remote; connect_dev; print_summary ;;
  start)
    ensure_deps; start_pg; start_remote; print_summary ;;
  connect-dev)
    connect_dev ;;
  status)
    status ;;
  stop)
    stop_all ;;
  *)
    die "Unknown command '${cmd}'. Use: up | start | connect-dev | status | stop" ;;
esac
