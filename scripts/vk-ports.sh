#!/usr/bin/env bash
# Default ports for vk-* scripts (override via env before vk-start).

: "${VK_FRONTEND_PORT:=13001}"
: "${VK_BACKEND_PORT:=13002}"
: "${VK_PREVIEW_PROXY_PORT:=13003}"
: "${VK_REMOTE_PORT:=13000}"
: "${VK_RELAY_PORT:=18082}"
: "${VK_REMOTE_DB_PORT:=15433}"
: "${VK_MOBILE_HTTPS_PORT:=13444}"
: "${VK_MOBILE_RELAY_HTTPS_PORT:=18443}"
: "${VK_BIND_ADDR:=0.0.0.0}"
# Desktop HTTP/2 front door (Caddy + local CA). The browser talks to this over
# h2 so all Electric shapes multiplex on one connection instead of stalling on
# the ~6-per-origin HTTP/1.1 limit. Set VK_DESKTOP_H2=0 to disable.
: "${VK_DESKTOP_HTTPS_PORT:=13443}"
# Desktop relay HTTPS front door — lets an HTTPS desktop page (13443) reach the
# relay without mixed-content (https page → http relay). Mirrors the mobile
# relay front door (18443) but for localhost + Caddy local CA.
: "${VK_DESKTOP_RELAY_HTTPS_PORT:=13445}"
: "${VK_DESKTOP_H2:=1}"
# Board Agent chat sidecar (Cursor SDK / Pi / OpenCode). Vite proxies /agent-sidecar → this port.
: "${VK_AGENT_SIDECAR_PORT:=13110}"
# Tailscale mobile HTTPS front door is opt-in: it binds the mobile HTTPS port,
# which by default collides with the Vite frontend port (both 13001). Enable
# with VK_MOBILE=1 (and ensure VK_MOBILE_HTTPS_PORT differs from the frontend).
: "${VK_MOBILE:=0}"

export VK_FRONTEND_PORT VK_BACKEND_PORT VK_PREVIEW_PROXY_PORT
export VK_REMOTE_PORT VK_RELAY_PORT VK_REMOTE_DB_PORT
export VK_MOBILE_HTTPS_PORT VK_MOBILE_RELAY_HTTPS_PORT
export VK_DESKTOP_HTTPS_PORT VK_DESKTOP_RELAY_HTTPS_PORT VK_DESKTOP_H2 VK_MOBILE
export VK_AGENT_SIDECAR_PORT
export VK_BIND_ADDR
