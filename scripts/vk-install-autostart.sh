#!/usr/bin/env bash
# Install macOS LaunchAgent: login → OrbStack wait → vk-start.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
LABEL="com.vibekanban.vk-start"
PLIST="${HOME}/Library/LaunchAgents/${LABEL}.plist"
STATE_DIR="${VK_STATE_DIR:-${HOME}/.vk-kanban}"
AUTOSTART_ENV="${STATE_DIR}/autostart.env"
AUTOSTART_SH="${ROOT}/scripts/vk-autostart.sh"

mkdir -p "${STATE_DIR}/logs" "${HOME}/Library/LaunchAgents"
chmod +x "${AUTOSTART_SH}"

if [[ ! -f "${AUTOSTART_ENV}" ]]; then
  cat >"${AUTOSTART_ENV}" <<EOF
# vk-autostart 环境（登录时由 LaunchAgent 读取）
# 取消注释以启用对应能力：

# export VK_MOBILE=1
# export VK_REBUILD=0
# OrbStack 容器代理：默认 none（直连）。仅当宿主代理开了 allow-lan 时才设：
#   1=用 http://host.orb.internal:7897；或直接写 http(s):// 自定义 URL
# export VK_ORBSTACK_PROXY=1
# export VK_DOCKER_WAIT_SEC=300
# 登录时等待 Tailscale 就绪的秒数（让浏览器 API base/CORS 覆盖 Tailscale 主机；0=不等待）
# export VK_TAILSCALE_WAIT_SEC=60
EOF
  echo "已创建 ${AUTOSTART_ENV}（可按需编辑）"
fi

cat >"${PLIST}" <<EOF
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>Label</key>
  <string>${LABEL}</string>
  <key>RunAtLoad</key>
  <true/>
  <key>KeepAlive</key>
  <false/>
  <key>ProcessType</key>
  <string>Background</string>
  <key>ProgramArguments</key>
  <array>
    <string>/bin/bash</string>
    <string>${AUTOSTART_SH}</string>
  </array>
  <key>EnvironmentVariables</key>
  <dict>
    <key>VK_REPO</key>
    <string>${ROOT}</string>
  </dict>
  <key>StandardOutPath</key>
  <string>${STATE_DIR}/logs/launchd-vk-start.out.log</string>
  <key>StandardErrorPath</key>
  <string>${STATE_DIR}/logs/launchd-vk-start.err.log</string>
</dict>
</plist>
EOF

UID_NUM="$(id -u)"
launchctl bootout "gui/${UID_NUM}/${LABEL}" 2>/dev/null || true
launchctl bootstrap "gui/${UID_NUM}" "${PLIST}"

echo ""
echo "已安装 LaunchAgent: ${PLIST}"
echo "  仓库:     ${ROOT}"
echo "  环境文件: ${AUTOSTART_ENV}"
echo "  日志:     ${STATE_DIR}/logs/autostart.log"
echo ""
echo "前置条件（OrbStack，非 Docker Desktop）："
echo "  1. OrbStack → Settings → General → Start OrbStack at login"
echo "  2. 退出 Docker Desktop，避免抢 docker CLI"
echo "  3. docker context use orbstack   # 若 docker info 失败"
echo ""
echo "立即试跑: launchctl kickstart -k gui/${UID_NUM}/${LABEL}"
echo "健康检查: bash ${ROOT}/scripts/vk-status.sh"
echo "卸载:     bash ${ROOT}/scripts/vk-uninstall-autostart.sh"
