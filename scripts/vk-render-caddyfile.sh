#!/usr/bin/env bash
# Generate Caddyfile for Tailscale HTTPS (local-web + remote API + relay).
set -euo pipefail

TS_HOSTNAME="${1:?TS_HOSTNAME required}"
FRONTEND_PORT="${2:?FRONTEND_PORT required}"
BACKEND_PORT="${3:?BACKEND_PORT required}"
REMOTE_PORT="${4:?REMOTE_PORT required}"
RELAY_PORT="${5:?RELAY_PORT required}"
MOBILE_HTTPS_PORT="${6:?MOBILE_HTTPS_PORT required}"
MOBILE_RELAY_HTTPS_PORT="${7:?MOBILE_RELAY_HTTPS_PORT required}"
OUT="${8:?output path required}"
CERT_DIR="${9:?cert dir required}"

cat > "${OUT}" <<EOF
${TS_HOSTNAME}:${MOBILE_HTTPS_PORT} {
    tls ${CERT_DIR}/${TS_HOSTNAME}.crt ${CERT_DIR}/${TS_HOSTNAME}.key

    handle /v1/* {
        reverse_proxy 127.0.0.1:${REMOTE_PORT}
    }
    handle /shape/* {
        reverse_proxy 127.0.0.1:${REMOTE_PORT}
    }
    handle /api/* {
        reverse_proxy 127.0.0.1:${BACKEND_PORT}
    }
    handle {
        reverse_proxy 127.0.0.1:${FRONTEND_PORT}
    }
}

${TS_HOSTNAME}:${MOBILE_RELAY_HTTPS_PORT} {
    tls ${CERT_DIR}/${TS_HOSTNAME}.crt ${CERT_DIR}/${TS_HOSTNAME}.key
    reverse_proxy 127.0.0.1:${RELAY_PORT}
}
EOF
