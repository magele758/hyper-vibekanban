#!/usr/bin/env bash
# Remove vk-start LaunchAgent.
set -euo pipefail

LABEL="com.vibekanban.vk-start"
PLIST="${HOME}/Library/LaunchAgents/${LABEL}.plist"
UID_NUM="$(id -u)"

launchctl bootout "gui/${UID_NUM}/${LABEL}" 2>/dev/null || true
rm -f "${PLIST}"

echo "已卸载 ${LABEL}"
echo "（未删除 ~/.vk-kanban/autostart.env 与日志；手动删即可）"
