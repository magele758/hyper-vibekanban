<p align="center">
  <a href="https://vibekanban.com">
    <picture>
      <source srcset="packages/public/vibe-kanban-logo-dark.svg" media="(prefers-color-scheme: dark)">
      <source srcset="packages/public/vibe-kanban-logo.svg" media="(prefers-color-scheme: light)">
      <img src="packages/public/vibe-kanban-logo.svg" alt="Vibe Kanban Logo">
    </picture>
  </a>
</p>

<p align="center">让 Claude Code、Gemini CLI、Codex、Amp 等 AI 编程 Agent 的效率提升 10 倍...</p>
<p align="center">
  <a href="https://www.npmjs.com/package/vibe-kanban"><img alt="npm" src="https://img.shields.io/npm/v/vibe-kanban?style=flat-square" /></a>
  <a href="https://github.com/BloopAI/vibe-kanban/blob/main/.github/workflows/publish.yml"><img alt="构建状态" src="https://img.shields.io/github/actions/workflow/status/BloopAI/vibe-kanban/.github%2Fworkflows%2Fpublish.yml" /></a>
  <a href="https://deepwiki.com/BloopAI/vibe-kanban"><img src="https://deepwiki.com/badge.svg" alt="Ask DeepWiki"></a>
</p>

<p align="center">
  <a href="README.md">English</a> |
  <a href="README.zh-Hant.md">繁體中文</a> |
  <a href="README.ja.md">日本語</a> |
  <a href="README.ko.md">한국어</a> |
  <a href="README.es.md">Español</a> |
  <a href="README.fr.md">Français</a>
</p>

> **注意：** Vibe Kanban 已宣布停服。[查看公告](https://www.vibekanban.com/blog/shutdown)。项目仍保持开源，本地自托管完全可用。

![](packages/public/vibe-kanban-screenshot-overview.png)

## 简介

Vibe Kanban 是一款面向开发者的本地优先项目管理工具，专为配合 AI 编程 Agent 工作而设计。它将 **计划 → 执行 → 审查** 的工作循环流程化，帮助你更高效地交付代码。

- **用看板 Issue 规划工作** — 在看板上创建、排优先级、管理任务卡片
- **在 Workspace 中运行 AI Agent** — 每个 Workspace 自动创建独立的 git worktree，启动选定的 Agent 并实时流式输出日志
- **审查 Diff 并添加行内注释** — 在 UI 内逐行检查改动、添加注释，直接将反馈发回 Agent
- **应用预览** — 内置浏览器，支持 DevTools、元素检查和设备模拟
- **支持 10+ 款 AI Agent** — Claude Code、OpenAI Codex、Gemini CLI、GitHub Copilot、Amp、Cursor Agent CLI、OpenCode、Factory Droid、Claude Code Router (CCR)、Qwen Code
- **创建 PR 并合并** — 用 AI 生成 PR 描述，在 GitHub/Azure 上审查，一键合并

![](packages/public/vibe-kanban-screenshot-workspace.png)

## 快速开始

先完成你所用 AI Agent 的登录认证，然后执行：

```bash
npx vibe-kanban
```

就这一条命令。Vibe Kanban 会启动本地服务器并自动打开浏览器。

## 工作原理

### 核心概念

| 概念 | 说明 |
|------|------|
| **Project（项目）** | 本地机器上的一个 git 仓库 |
| **Issue（任务）** | 看板上的任务卡片（标题 + 描述 + 优先级 + 标签） |
| **Workspace（工作区）** | 独立执行环境 — git worktree + AI Agent + 可选的开发服务器 |

### 典型工作流

1. **创建项目** — 将 Vibe Kanban 指向本地 git 仓库
2. **添加 Issue** — 在看板上描述待完成的工作
3. **启动 Workspace** — 选择 Agent、分支和可选的安装/清理脚本，自动创建 git worktree
4. **观察 Agent 工作** — 工作区视图中实时展示日志流
5. **审查 Diff** — 统一视图或并排视图，支持行级注释
6. **迭代** — 提交审查意见，Agent 读取后继续修改
7. **交付** — 创建带 AI 生成描述的 PR，在 GitHub 上审查并合并

## 支持的 AI 编程 Agent

| Agent | 提供方 |
|-------|--------|
| Claude Code | Anthropic |
| OpenAI Codex CLI | OpenAI |
| Gemini CLI | Google |
| GitHub Copilot CLI | GitHub |
| Amp | Sourcegraph |
| Cursor Agent CLI | Anysphere |
| OpenCode | SST |
| Factory Droid | Factory AI |
| Claude Code Router (CCR) | 社区 |
| Qwen Code | 阿里巴巴 |

各 Agent 的安装与认证方法见[官方文档](https://vibekanban.com/docs/supported-coding-agents)。

## MCP 服务器

Vibe Kanban 内置本地 [MCP（模型上下文协议）](https://modelcontextprotocol.io/) 服务器，允许外部客户端（Claude Desktop、Raycast 等）通过程序化方式管理 Issue 和 Workspace。

```bash
# 启动 MCP 服务器
npx vibe-kanban --mcp
```

或将其加入 Agent 的 MCP 配置：

```json
{
  "mcpServers": {
    "vibe_kanban": {
      "command": "npx",
      "args": ["-y", "vibe-kanban@latest", "--mcp"]
    }
  }
}
```

## CLI 参考

```bash
npx vibe-kanban               # 启动本地 UI（默认）
npx vibe-kanban --mcp         # 启动 MCP stdio 服务器
npx vibe-kanban review        # 运行代码审查 CLI
npx vibe-kanban --help
npx vibe-kanban --version
```

## 文档

完整文档和使用指南请访问[官方网站](https://vibekanban.com/docs)。

## 自托管

想部署自己的 Vibe Kanban Cloud 实例？参见[自托管指南](https://vibekanban.com/docs/self-hosting/deploy-docker)。

## 支持与反馈

功能建议请使用 [GitHub Discussions](https://github.com/BloopAI/vibe-kanban/discussions)，Bug 报告请提交 [GitHub Issues](https://github.com/BloopAI/vibe-kanban/issues)。

## 参与贡献

提交 PR 前请先在 [GitHub Discussions](https://github.com/BloopAI/vibe-kanban/discussions) 或 [Discord](https://discord.gg/AC4nwVtJM3) 与核心团队讨论实现方案和路线图契合度。

---

## 开发

### 环境要求

- [Rust](https://rustup.rs/)（最新稳定版）
- [Node.js](https://nodejs.org/)（≥ 20）
- [pnpm](https://pnpm.io/)（≥ 8）

```bash
cargo install cargo-watch
cargo install sqlx-cli
pnpm i
```

### 启动开发服务器

```bash
pnpm run dev
```

同时启动 Rust 后端（通过 `cargo-watch` 热重载）和 Vite 前端开发服务器。首次运行时，会从 `dev_assets_seed/` 复制一个空白 SQLite 数据库。

### 仅构建前端

```bash
cd packages/local-web
pnpm run build
```

### 从源码构建（生成 npx-cli 发布包）

```bash
./local-build.sh
# 测试结果：
cd npx-cli && node bin/cli.js
```

该脚本会构建 React 前端，编译三个 Rust 二进制文件（`server`、`vibe-kanban-mcp`、`review`），并组装 npx-cli 包。

### 类型检查与代码检查

```bash
pnpm run check   # TypeScript（所有包）+ Rust cargo check
pnpm run lint    # ESLint + cargo clippy
pnpm run format  # Prettier + cargo fmt
```

### 重新生成共享 TypeScript 类型

```bash
pnpm run generate-types
```

类型由 Rust 结构体通过 [ts-rs](https://github.com/Aleph-Alpha/ts-rs) 派生生成。**请勿直接编辑** `shared/types.ts`，应修改 `crates/server/src/bin/generate_types.rs`。

### 环境变量

| 变量 | 时机 | 默认值 | 说明 |
|------|------|--------|------|
| `PORT` | 运行时 | 自动 | 生产环境服务端口；开发时为前端端口（后端 = PORT+1） |
| `FRONTEND_PORT` | 运行时 | `3000` | 开发模式 Vite 端口 |
| `BACKEND_PORT` | 运行时 | `0`（自动）| 开发模式后端端口 |
| `HOST` | 运行时 | `127.0.0.1` | 后端绑定地址 |
| `MCP_HOST` | 运行时 | `HOST` | MCP 服务器连接主机 |
| `MCP_PORT` | 运行时 | `BACKEND_PORT` | MCP 服务器连接端口 |
| `VK_ALLOWED_ORIGINS` | 运行时 | — | 允许的来源（逗号分隔），反向代理场景必填 |
| `VK_SHARED_API_BASE` | 运行时 | — | 远程/云端 API 基础 URL |
| `VK_SHARED_RELAY_API_BASE` | 运行时 | — | 隧道模式 Relay API 基础 URL |
| `VK_TUNNEL` | 运行时 | — | 启用 Relay 隧道模式 |
| `DISABLE_WORKTREE_CLEANUP` | 运行时 | — | 禁用 git worktree 自动清理（调试用） |
| `POSTHOG_API_KEY` | 构建时 | — | PostHog 分析 Key（为空则禁用分析） |
| `POSTHOG_API_ENDPOINT` | 构建时 | — | PostHog 分析端点 |

#### 反向代理场景

设置 `VK_ALLOWED_ORIGINS` 为前端完整来源地址，否则后端会返回 `403 Forbidden`：

```bash
VK_ALLOWED_ORIGINS=https://vk.example.com npx vibe-kanban
# 多个来源：
VK_ALLOWED_ORIGINS=https://vk.example.com,https://vk-staging.example.com npx vibe-kanban
```

#### 远程 SSH 编辑器集成

在远程服务器上运行 Vibe Kanban 时，在 **设置 → 编辑器集成** 中配置 SSH 主机和用户名。"在 VSCode 中打开"按钮将生成 `vscode://vscode-remote/ssh-remote+…` 格式的 URL，自动连接到远程机器上的本地编辑器。
