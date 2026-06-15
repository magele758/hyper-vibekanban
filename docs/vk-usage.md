# Vibe Kanban 本地启动说明

## 默认端口（避免与常见 3000/8080 冲突）

| 服务 | 端口 | 说明 |
|------|------|------|
| 本地 Desktop Web (Vite) | **13001** | 日常用的 Kanban 客户端 |
| 本地 Rust API | **13002** | Desktop 后端 |
| Preview Proxy | **13003** | 预览代理 |
| Remote Web + API (Docker) | **13000** | 云端/自托管 Remote |
| Relay (Docker) | **18082** | 设备中继 |
| Remote Postgres | **15433** | 仅调试 DB 时用 |

端口定义在 `scripts/vk-ports.sh`，可用环境变量覆盖（见下文）。

## 一键命令（推荐）

在 `~/.zshrc` 里配置一次：

```bash
export VK_REPO="/path/to/vibe-kanban"
alias vk-start='bash "$VK_REPO/scripts/vk-start.sh"'
alias vk-stop='bash "$VK_REPO/scripts/vk-stop.sh"'
alias vk-status='bash "$VK_REPO/scripts/vk-status.sh"'
alias vk-logs='tail -f "$HOME/.vk-kanban/logs/dev.log"'
```

或在仓库根目录：

```bash
pnpm run vk:start    # 启动 Remote Docker + 本地 dev
pnpm run vk:status   # 健康检查
pnpm run vk:stop     # 停本地 dev（Remote Docker 仍运行）
```

### 常用操作

```bash
vk-start              # 全栈启动
vk-status             # 看各服务是否 OK
vk-stop               # 只停本地 Vite + Rust backend
vk-stop --remote      # 再停 Remote Docker（保留数据）
vk-stop --all         # 停 Docker 并删卷（清库）
```

### 首次 / 改过端口后

旧容器若仍占用 3000/8082，需重建一次：

```bash
vk-stop --remote
vk-start
```

## 启动后访问

- **Desktop 客户端**：http://localhost:13001
- **Remote 管理页**：http://localhost:13000
- **登录**：`admin@local.dev` / `devpass123`（见 `crates/remote/.env.remote`）

日志：`~/.vk-kanban/logs/dev.log`

## 自定义端口

```bash
export VK_FRONTEND_PORT=14001
export VK_BACKEND_PORT=14002
export VK_REMOTE_PORT=14000
export VK_RELAY_PORT=19082
vk-start
```

可写进 `~/.zshrc` 长期生效。改 Remote/Relay 端口后建议 `vk-stop --remote && vk-start`。

## 手机访问（Tailscale + Caddy）

安装 `tailscale`、`caddy` 并登录 Tailscale 后，`vk-start` 会自动：

- 生成 HTTPS 证书
- 暴露 `https://<你的Tailscale主机名>:13001`（Web）
- Relay：`https://<主机名>:18443`

详见 `mobile-testing.md`。

## 仅 Remote（不启本地 Desktop）

```bash
pnpm run remote:up
# 然后浏览器打开 http://localhost:13000
```

## 纯本地 Lite（无 Docker、无云）

```bash
pnpm run dev:lite
# 端口同样走 vk-ports / setup-dev-environment，默认 13001/13002
```

## 架构速记

```
浏览器 :13001  →  Vite (local-web)
                    ↓ /api 代理
                 :13002  local Rust server  →  :13000 Remote (Docker)
                                              →  :18082 Relay (Docker)
```
