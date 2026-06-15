#!/usr/bin/env bash
# Render the Caddyfile used by vk-start.
#
# Always emits a localhost HTTP/2 "desktop front door" so the browser can
# multiplex all Electric shape connections over a single h2 connection instead
# of hitting the ~6-per-origin HTTP/1.1 limit (the main cause of slow project
# switching). Optionally also emits the Tailscale HTTPS blocks for mobile.
set -euo pipefail

DESKTOP_HTTPS_PORT="${1:?DESKTOP_HTTPS_PORT required}"
REMOTE_PORT="${2:?REMOTE_PORT required}"
OUT="${3:?output path required}"
# Optional Tailscale mobile front door (pass empty TS_HOSTNAME to skip).
TS_HOSTNAME="${4:-}"
FRONTEND_PORT="${5:-}"
BACKEND_PORT="${6:-}"
RELAY_PORT="${7:-}"
MOBILE_HTTPS_PORT="${8:-}"
MOBILE_RELAY_HTTPS_PORT="${9:-}"
CERT_DIR="${10:-}"
DESKTOP_RELAY_HTTPS_PORT="${11:-}"

# Desktop h2 front door: a full same-origin app front door (mirrors the mobile
# block) so the browser loads the whole app over HTTP/2 and multiplexes all
# Electric shapes on one connection instead of hitting the ~6-per-origin
# HTTP/1.1 limit. `tls internal` makes Caddy serve HTTP/2 using its local CA.
cat > "${OUT}" <<EOF
localhost:${DESKTOP_HTTPS_PORT} {
    tls internal

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
EOF

# Desktop relay front door: an HTTPS desktop page must reach the relay over
# HTTPS too (avoid mixed-content https page → http relay).
if [[ -n "${DESKTOP_RELAY_HTTPS_PORT}" ]]; then
  cat >> "${OUT}" <<EOF

localhost:${DESKTOP_RELAY_HTTPS_PORT} {
    tls internal
    reverse_proxy 127.0.0.1:${RELAY_PORT}
}
EOF
fi

if [[ -n "${TS_HOSTNAME}" ]]; then
  cat >> "${OUT}" <<EOF

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
fi
