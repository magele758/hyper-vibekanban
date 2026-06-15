<p align="center">
  <a href="https://vibekanban.com">
    <picture>
      <source srcset="packages/public/vibe-kanban-logo-dark.svg" media="(prefers-color-scheme: dark)">
      <source srcset="packages/public/vibe-kanban-logo.svg" media="(prefers-color-scheme: light)">
      <img src="packages/public/vibe-kanban-logo.svg" alt="Vibe Kanban Logo">
    </picture>
  </a>
</p>

<p align="center">讓 Claude Code、Gemini CLI、Codex、Amp 等 AI 程式設計 Agent 的效率提升 10 倍...</p>
<p align="center">
  <a href="https://www.npmjs.com/package/vibe-kanban"><img alt="npm" src="https://img.shields.io/npm/v/vibe-kanban?style=flat-square" /></a>
  <a href="https://github.com/BloopAI/vibe-kanban/blob/main/.github/workflows/publish.yml"><img alt="建置狀態" src="https://img.shields.io/github/actions/workflow/status/BloopAI/vibe-kanban/.github%2Fworkflows%2Fpublish.yml" /></a>
  <a href="https://deepwiki.com/BloopAI/vibe-kanban"><img src="https://deepwiki.com/badge.svg" alt="Ask DeepWiki"></a>
</p>

<p align="center">
  <a href="README.md">English</a> |
  <a href="README.zh-Hans.md">简体中文</a> |
  <a href="README.ja.md">日本語</a> |
  <a href="README.ko.md">한국어</a> |
  <a href="README.es.md">Español</a> |
  <a href="README.fr.md">Français</a>
</p>

> **注意：** Vibe Kanban 已宣布停止服務。[查看公告](https://www.vibekanban.com/blog/shutdown)。本專案仍保持開源，本地自托管完全可用。

![](packages/public/vibe-kanban-screenshot-overview.png)

## 簡介

Vibe Kanban 是一款面向開發者的本地優先專案管理工具，專為配合 AI 程式設計 Agent 工作而設計。它將 **計劃 → 執行 → 審查** 的工作循環流程化，幫助你更高效地交付程式碼。

- **用看板 Issue 規劃工作** — 在看板上建立、排優先級、管理任務卡片
- **在 Workspace 中執行 AI Agent** — 每個 Workspace 自動建立獨立的 git worktree，啟動選定的 Agent 並即時串流輸出日誌
- **審查 Diff 並新增行內注解** — 在 UI 內逐行檢查改動、新增注解，直接將回饋發回 Agent
- **應用預覽** — 內建瀏覽器，支援 DevTools、元素檢查和裝置模擬
- **支援 10+ 款 AI Agent** — Claude Code、OpenAI Codex、Gemini CLI、GitHub Copilot、Amp、Cursor Agent CLI、OpenCode、Factory Droid、Claude Code Router (CCR)、Qwen Code
- **建立 PR 並合併** — 用 AI 生成 PR 描述，在 GitHub/Azure 上審查，一鍵合併

![](packages/public/vibe-kanban-screenshot-workspace.png)

## 快速開始

先完成你所用 AI Agent 的登入認證，然後執行：

```bash
npx vibe-kanban
```

就這一條指令。Vibe Kanban 會啟動本地伺服器並自動開啟瀏覽器。

## 運作原理

### 核心概念

| 概念 | 說明 |
|------|------|
| **Project（專案）** | 本地機器上的一個 git 儲存庫 |
| **Issue（任務）** | 看板上的任務卡片（標題 + 描述 + 優先級 + 標籤） |
| **Workspace（工作區）** | 獨立執行環境 — git worktree + AI Agent + 可選的開發伺服器 |

### 典型工作流程

1. **建立專案** — 將 Vibe Kanban 指向本地 git 儲存庫
2. **新增 Issue** — 在看板上描述待完成的工作
3. **啟動 Workspace** — 選擇 Agent、分支和可選的安裝/清理腳本，自動建立 git worktree
4. **觀察 Agent 工作** — 工作區視圖中即時展示日誌串流
5. **審查 Diff** — 統一視圖或並排視圖，支援行級注解
6. **迭代** — 提交審查意見，Agent 讀取後繼續修改
7. **交付** — 建立帶 AI 生成描述的 PR，在 GitHub 上審查並合併

## 支援的 AI 程式設計 Agent

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
| Claude Code Router (CCR) | 社群 |
| Qwen Code | 阿里巴巴 |

各 Agent 的安裝與認證方法請參見[官方文件](https://vibekanban.com/docs/supported-coding-agents)。

## MCP 伺服器

Vibe Kanban 內建本地 [MCP（模型上下文協定）](https://modelcontextprotocol.io/) 伺服器，允許外部用戶端（Claude Desktop、Raycast 等）透過程式化方式管理 Issue 和 Workspace。

```bash
# 啟動 MCP 伺服器
npx vibe-kanban --mcp
```

或將其加入 Agent 的 MCP 設定：

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

## CLI 參考

```bash
npx vibe-kanban               # 啟動本地 UI（預設）
npx vibe-kanban --mcp         # 啟動 MCP stdio 伺服器
npx vibe-kanban review        # 執行程式碼審查 CLI
npx vibe-kanban --help
npx vibe-kanban --version
```

## 文件

完整文件和使用指南請造訪[官方網站](https://vibekanban.com/docs)。

## 自托管

想部署自己的 Vibe Kanban Cloud 實例？請參見[自托管指南](https://vibekanban.com/docs/self-hosting/deploy-docker)。

## 支援與回饋

功能建議請使用 [GitHub Discussions](https://github.com/BloopAI/vibe-kanban/discussions)，Bug 回報請提交 [GitHub Issues](https://github.com/BloopAI/vibe-kanban/issues)。

## 參與貢獻

提交 PR 前請先在 [GitHub Discussions](https://github.com/BloopAI/vibe-kanban/discussions) 或 [Discord](https://discord.gg/AC4nwVtJM3) 與核心團隊討論實作方案和路線圖契合度。

---

## 開發

### 環境需求

- [Rust](https://rustup.rs/)（最新穩定版）
- [Node.js](https://nodejs.org/)（≥ 20）
- [pnpm](https://pnpm.io/)（≥ 8）

```bash
cargo install cargo-watch
cargo install sqlx-cli
pnpm i
```

### 啟動開發伺服器

```bash
pnpm run dev
```

同時啟動 Rust 後端（透過 `cargo-watch` 熱重載）和 Vite 前端開發伺服器。首次執行時，會從 `dev_assets_seed/` 複製一個空白 SQLite 資料庫。

### 僅建置前端

```bash
cd packages/local-web
pnpm run build
```

### 從原始碼建置（生成 npx-cli 發布包）

```bash
./local-build.sh
# 測試結果：
cd npx-cli && node bin/cli.js
```

### 型別檢查與程式碼檢查

```bash
pnpm run check   # TypeScript（所有套件）+ Rust cargo check
pnpm run lint    # ESLint + cargo clippy
pnpm run format  # Prettier + cargo fmt
```

### 重新生成共享 TypeScript 型別

```bash
pnpm run generate-types
```

型別由 Rust 結構體透過 [ts-rs](https://github.com/Aleph-Alpha/ts-rs) 派生生成。**請勿直接編輯** `shared/types.ts`，應修改 `crates/server/src/bin/generate_types.rs`。

### 環境變數

| 變數 | 時機 | 預設值 | 說明 |
|------|------|--------|------|
| `PORT` | 執行時 | 自動 | 正式環境服務埠；開發時為前端埠（後端 = PORT+1） |
| `FRONTEND_PORT` | 執行時 | `3000` | 開發模式 Vite 埠 |
| `BACKEND_PORT` | 執行時 | `0`（自動）| 開發模式後端埠 |
| `HOST` | 執行時 | `127.0.0.1` | 後端綁定位址 |
| `VK_ALLOWED_ORIGINS` | 執行時 | — | 允許的來源（逗號分隔），反向代理場景必填 |
| `VK_TUNNEL` | 執行時 | — | 啟用 Relay 隧道模式 |
| `DISABLE_WORKTREE_CLEANUP` | 執行時 | — | 停用 git worktree 自動清理（偵錯用） |
| `POSTHOG_API_KEY` | 建置時 | — | PostHog 分析 Key（為空則停用分析） |

#### 反向代理場景

設定 `VK_ALLOWED_ORIGINS` 為前端完整來源位址，否則後端會回傳 `403 Forbidden`：

```bash
VK_ALLOWED_ORIGINS=https://vk.example.com npx vibe-kanban
```
