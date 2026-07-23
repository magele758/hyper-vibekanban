<p align="center">
  <a href="https://github.com/magele758/hyper-vibekanban">
    <picture>
      <source srcset="packages/public/vibe-kanban-logo-dark.svg" media="(prefers-color-scheme: dark)">
      <source srcset="packages/public/vibe-kanban-logo.svg" media="(prefers-color-scheme: light)">
      <img src="packages/public/vibe-kanban-logo.svg" alt="Vibe Kanban Logo">
    </picture>
  </a>
</p>

<p align="center">让 Claude Code、Gemini CLI、Codex、Cursor、Pi 等 AI 编程 Agent 的效率提升 10 倍...</p>
<p align="center">
  <a href="https://www.npmjs.com/package/vibe-kanban"><img alt="npm" src="https://img.shields.io/npm/v/vibe-kanban?style=flat-square" /></a>
  <a href="https://github.com/magele758/hyper-vibekanban/blob/main/.github/workflows/publish.yml"><img alt="构建状态" src="https://img.shields.io/github/actions/workflow/status/magele758/hyper-vibekanban/.github%2Fworkflows%2Fpublish.yml" /></a>
</p>

<p align="center">
  <a href="README.md">English</a> |
  <a href="README.zh-Hant.md">繁體中文</a> |
  <a href="README.ja.md">日本語</a> |
  <a href="README.ko.md">한국어</a> |
  <a href="README.es.md">Español</a> |
  <a href="README.fr.md">Français</a>
</p>

> **注意：** 官方 Vibe Kanban 云服务已停服。本仓库是上游 [BloopAI/vibe-kanban](https://github.com/BloopAI/vibe-kanban) 的 fork（hyper-vibekanban）：**原版能力完整保留**，并在其上新增「动态看板 Agent」编排与自托管增强。

![](packages/public/screenshots/hyper-board.png)

## 相对原版：继承了什么 / 新增了什么

一句话：原版是「人手开 Workspace 的 Agent 工作台 + 看板」；本 fork 在其上叠一层 **看板事件 → 自动入队执行 → 结果回写**，并把停服的云能力改成可自托管。

### ✅ 继承自原版（完整保留，用法不变）

| 能力 | 说明 |
|------|------|
| **看板 Issue** | 创建 / 优先级 / 标签 / 子任务 / Team·Personal |
| **Workspace + git worktree** | 选 Agent 开工，独立 worktree，实时日志流 |
| **会话与 follow-up** | 多 Session、继续对话、附件 / @ 文件 |
| **Diff 行内审查** | 统一 / 并排视图，注释回传 Agent |
| **应用 Preview** | 内置浏览器、DevTools、元素检查、设备模拟 |
| **Coding Agents** | Claude Code、Codex、Gemini、Copilot、Amp、Cursor、OpenCode、Droid、CCR、Qwen |
| **Git / PR** | rebase、冲突处理、AI 生成 PR 描述、GitHub / Azure 合并 |
| **MCP + Review CLI** | `npx vibe-kanban --mcp` / `review` |
| **设置面** | Agent 配置、MCP、编辑器集成、通知、组织 / 项目 |

原版典型路径仍可用：**建 Issue → 手开 Workspace → 看日志 → Review Diff → 开 PR**。

### ✨ 本 fork 新增（原版没有）

| 能力 | 说明 |
|------|------|
| **Board Agents（动态看板）** | Agent 成为看板一等公民；**指派即入队**，Local watcher 自动开 Workspace 并回写进度 / 评论 |
| **项目 Copilot** | 看板侧对话层（默认 Cursor SDK）：澄清需求、拆 Issue、建议指派；**不等于** Workspace 里写代码的 Coding Agent |
| **Squad DAG 编排** | 多 Agent 流水线：Fork / Join / If / While，画布编辑；可对话生成 pipeline |
| **Autopilot** | Cron + 时区；定时建 Issue 或触发 Agent / Squad；并发 skip / queue |
| **Webhook 触发** | 外部 POST → 创建 Issue / 入队执行 |
| **飞书机器人** | 飞书消息 → Issue 入队；完成后可回飞书 |
| **Console 工作区** | 在主仓库**当前目录 / 当前分支**直接跑，不强制新建 worktree / 分支 |
| **创建时 Host 选择** | Workspace 可指定在本机或已配对的远程 Worker 上跑 |
| **手机看板布局** | 窄屏单列 + 状态 pill，适合手机查看看板 |
| **Pi Coding Agent** | 新增 Pi CLI 作为 Workspace 执行器之一 |
| **自托管 Remote 栈** | 官方云停服后，用 Docker Remote + Relay + ElectricSQL 续上多端同步（见 `scripts/vk-*.sh`） |

### 🔄 相对原版的增强（有基础，本 fork 加强）

| 能力 | 原版 | 本 fork |
|------|------|---------|
| Remote Access | 官方云配对 | **本地自托管** Remote / Relay；Worker Host SOP |
| 看板 | 静态卡片 + 人手开 Workspace | **可指派 Agent / Squad**，进度回写看板 |
| 执行入口 | UI / MCP 手动创建 Workspace | 另增：指派、@、Autopilot、Webhook、飞书 |

---

## 功能展示

截图使用演示数据（Demo Org / Demo Showcase）。标注 **【新增】** / **【继承】**。

### 1. 【新增】动态看板 + Board Agents

看板可指派 Agent；指派后自动入队执行并回写。

![](packages/public/screenshots/hyper-board.png)

![](packages/public/screenshots/hyper-agents.png)

### 2. 【新增】项目 Copilot

对话层负责澄清与编排；真正改代码仍走 Workspace 里的 Coding Agent。

![](packages/public/screenshots/hyper-copilot.png)

### 3. 【新增】Squad 流水线（DAG）

Plan → Fork → Implement / Review → Join；可对话创建，再画布微调。

![](packages/public/screenshots/hyper-squad.png)

![](packages/public/screenshots/hyper-squad-canvas.png)

### 4. 【新增】Autopilot / Webhook / 飞书

定时、外部事件、飞书消息三种入口，统一落到「建 Issue → 入队」。

![](packages/public/screenshots/hyper-autopilot.png)

![](packages/public/screenshots/hyper-webhooks.png)

![](packages/public/screenshots/hyper-feishu.png)

### 5. 【新增】Console 工作区 + Host 选择

- **隔离 worktree（继承，默认）** — 独立分支 / 目录
- **主目录控制台 Console（新增）** — 当前目录与分支直接跑，不自动建分支 / 提交
- **运行 Host（新增）** — 本机或已配对远程 Worker

![](packages/public/screenshots/hyper-create-console.png)

![](packages/public/screenshots/hyper-remote-access.png)

### 6. 【新增】手机看板布局

![](packages/public/screenshots/hyper-mobile-board.png)

### 7. 【继承】Workspace 会话 / Diff / Preview

原版核心能力，本 fork 原样保留并继续打磨。

![](packages/public/screenshots/hyper-sessions.png)

![](packages/public/screenshots/hyper-diffs.png)

![](packages/public/screenshots/hyper-preview.png)

---

## 快速开始

先完成所用 AI Agent 的登录认证，然后：

```bash
npx vibe-kanban
```

会启动本地服务器并打开浏览器。

### 自托管 Remote（可选）

官方云停服后，本仓库用 Docker Remote + Relay + ElectricSQL 续上「多端同步 / 远程看板」。开发栈一键脚本见仓库根目录 `scripts/vk-*.sh`（端口约定见 `scripts/vk-ports.sh`）。部署说明：[自托管指南](docs/self-hosting/deploy-docker.mdx)。

---

## 工作原理

### 核心概念

| 概念 | 说明 |
|------|------|
| **Project** | 看板项目（可关联多个本地 git 仓库） |
| **Issue** | 看板任务卡片 |
| **Workspace** | 执行环境：worktree 或 Console + Coding Agent |
| **Board Agent** | 可指派、可对话的看板角色；触发后复用 Workspace 执行 |
| **Squad** | 多 Agent + DAG 流水线 |
| **Host** | 实际跑 Agent 的机器（本机或配对远端） |

### 两条工作流

**A. 原版流程（继承，仍完全可用）**

1. 建 Issue → 手动创建 Workspace  
2. 看日志 / Preview → Diff 审查 → 迭代  
3. 开 PR 合并  

**B. 动态看板流程（本 fork 新增）**

1. 创建 Board Agent（人设 + 默认 executor）  
2. 指派 Issue（或 @ / Webhook / 飞书 / Autopilot）  
3. Local watcher 入队 → 自动开 Workspace  
4. 看板回写进度 / 评论；可用 Copilot 澄清下一轮  
5. 复杂任务用 Squad 画布编排  
6. Diff 审查 → 开 PR（与原版相同）

---

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
| Pi | Pi（**本 fork 新增**） |

安装与认证见 [supported-coding-agents](docs/supported-coding-agents.mdx)。看板对话 runtime（Copilot / Agent 聊天）默认可接 Cursor SDK，与上表 Coding Executor 是不同层——前者是本 fork 新增的编排层，后者是原版执行层。

---

## MCP 服务器

```bash
npx vibe-kanban --mcp
```

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
npx vibe-kanban               # 启动本地 UI
npx vibe-kanban --mcp         # MCP stdio
npx vibe-kanban review        # 代码审查 CLI
npx vibe-kanban --help
```

## 文档

- [`docs/`](docs/) — 用户与自托管文档
- [`docs/board-agents-plan.md`](docs/board-agents-plan.md) — 动态看板 Agent 设计与分期
- [`docs/remote-access.mdx`](docs/remote-access.mdx) — Remote Access / 配对

## 支持与贡献

功能建议用 [Discussions](https://github.com/magele758/hyper-vibekanban/discussions)，Bug 用 [Issues](https://github.com/magele758/hyper-vibekanban/issues)。提 PR 前建议先开 Discussion 对齐方案。

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

同时启动 Rust 后端（`cargo-watch`）与 Vite。首次会从 `dev_assets_seed/` 复制空白 SQLite。

完整本机栈（Remote Docker + Relay + Desktop）可用：

```bash
bash scripts/vk-start.sh
bash scripts/vk-status.sh
```

### 从源码构建 npx 包

```bash
./local-build.sh
cd npx-cli && node bin/cli.js
```

### 检查与类型

```bash
pnpm run check
pnpm run lint
pnpm run format
pnpm run generate-types   # 勿手改 shared/types.ts
```

### 常用环境变量

| 变量 | 说明 |
|------|------|
| `FRONTEND_PORT` / `BACKEND_PORT` / `HOST` | 开发端口与绑定 |
| `VK_ALLOWED_ORIGINS` | 反代场景允许的来源 |
| `VK_SHARED_API_BASE` | Remote API（服务端用 http） |
| `VK_SHARED_RELAY_API_BASE` | Relay API |
| `VK_TUNNEL` | 启用 Relay 隧道 |

反向代理时必须设置 `VK_ALLOWED_ORIGINS`，否则后端会 `403`。远程 SSH 编辑器集成见 **设置 → 编辑器集成**。
