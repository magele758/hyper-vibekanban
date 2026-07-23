<p align="center">
  <a href="https://github.com/magele758/hyper-vibekanban">
    <picture>
      <source srcset="packages/public/vibe-kanban-logo-dark.svg" media="(prefers-color-scheme: dark)">
      <source srcset="packages/public/vibe-kanban-logo.svg" media="(prefers-color-scheme: light)">
      <img src="packages/public/vibe-kanban-logo.svg" alt="Vibe Kanban Logo">
    </picture>
  </a>
</p>

<p align="center">讓 Claude Code、Gemini CLI、Codex、Cursor、Pi 等 AI 程式設計 Agent 的效率提升 10 倍...</p>
<p align="center">
  <a href="https://www.npmjs.com/package/vibe-kanban"><img alt="npm" src="https://img.shields.io/npm/v/vibe-kanban?style=flat-square" /></a>
  <a href="https://github.com/magele758/hyper-vibekanban/blob/main/.github/workflows/publish.yml"><img alt="建置狀態" src="https://img.shields.io/github/actions/workflow/status/magele758/hyper-vibekanban/.github%2Fworkflows%2Fpublish.yml" /></a>
</p>

<p align="center">
  <a href="README.md">English</a> |
  <a href="README.zh-Hans.md">简体中文</a> |
  <a href="README.ja.md">日本語</a> |
  <a href="README.ko.md">한국어</a> |
  <a href="README.es.md">Español</a> |
  <a href="README.fr.md">Français</a>
</p>

> **注意：** 官方 Vibe Kanban 雲服務已停服。本倉庫是上游 [BloopAI/vibe-kanban](https://github.com/BloopAI/vibe-kanban) 的 fork（hyper-vibekanban）：**原版能力完整保留**，並在其上新增「動態看板 Agent」編排與自託管增強。

![](packages/public/screenshots/hyper-board.png)

## 相對原版：繼承了什麼 / 新增了什麼

一句話：原版是「人手開 Workspace 的 Agent 工作台 + 看板」；本 fork 在其上疊一層 **看板事件 → 自動入隊執行 → 結果回寫**，並把停服的雲能力改成可自託管。

### ✅ 繼承自原版（完整保留，用法不變）

| 能力 | 說明 |
|------|------|
| **看板 Issue** | 建立 / 優先級 / 標籤 / 子任務 / Team·Personal |
| **Workspace + git worktree** | 選 Agent 開工，獨立 worktree，即時日誌流 |
| **會話與 follow-up** | 多 Session、繼續對話、附件 / @ 檔案 |
| **Diff 行內審查** | 統一 / 並排檢視，註解回傳 Agent |
| **應用 Preview** | 內建瀏覽器、DevTools、元素檢查、裝置模擬 |
| **Coding Agents** | Claude Code、Codex、Gemini、Copilot、Amp、Cursor、OpenCode、Droid、CCR、Qwen |
| **Git / PR** | rebase、衝突處理、AI 生成 PR 描述、GitHub / Azure 合併 |
| **MCP + Review CLI** | `npx vibe-kanban --mcp` / `review` |
| **設定面** | Agent 設定、MCP、編輯器整合、通知、組織 / 專案 |

原版典型路徑仍可用：**建 Issue → 手開 Workspace → 看日誌 → Review Diff → 開 PR**。

### ✨ 本 fork 新增（原版沒有）

| 能力 | 說明 |
|------|------|
| **Board Agents（動態看板）** | Agent 成為看板一等公民；**指派即入隊**，Local watcher 自動開 Workspace 並回寫進度 / 評論 |
| **專案 Copilot** | 看板側對話層（預設 Cursor SDK）：釐清需求、拆 Issue、建議指派；**不等於** Workspace 裡寫程式的 Coding Agent |
| **Squad DAG 編排** | 多 Agent 流水線：Fork / Join / If / While，畫布編輯；可對話生成 pipeline |
| **Autopilot** | Cron + 時區；定時建 Issue 或觸發 Agent / Squad；併發 skip / queue |
| **Webhook 觸發** | 外部 POST → 建立 Issue / 入隊執行 |
| **飛書機器人** | 飛書訊息 → Issue 入隊；完成後可回飛書 |
| **Console 工作區** | 在主倉庫**目前目錄 / 目前分支**直接跑，不強制新建 worktree / 分支 |
| **建立時 Host 選擇** | Workspace 可指定在本機或已配對的遠端 Worker 上跑 |
| **手機看板佈局** | 窄螢幕單欄 + 狀態 pill，適合手機查看看板 |
| **Pi Coding Agent** | 新增 Pi CLI 作為 Workspace 執行器之一 |
| **自託管 Remote 棧** | 官方雲停服後，用 Docker Remote + Relay + ElectricSQL 續上多端同步（見 `scripts/vk-*.sh`） |

### 🔄 相對原版的增強（有基礎，本 fork 加強）

| 能力 | 原版 | 本 fork |
|------|------|---------|
| Remote Access | 官方雲配對 | **本地自託管** Remote / Relay；Worker Host SOP |
| 看板 | 靜態卡片 + 人手開 Workspace | **可指派 Agent / Squad**，進度回寫看板 |
| 執行入口 | UI / MCP 手動建立 Workspace | 另增：指派、@、Autopilot、Webhook、飛書 |

---

## 功能展示

截圖使用示範資料（Demo Org / Demo Showcase）。標註 **【新增】** / **【繼承】**。

### 1. 【新增】動態看板 + Board Agents

看板可指派 Agent；指派後自動入隊執行並回寫。

![](packages/public/screenshots/hyper-board.png)

![](packages/public/screenshots/hyper-agents.png)

### 2. 【新增】專案 Copilot

對話層負責釐清與編排；真正改程式仍走 Workspace 裡的 Coding Agent。

![](packages/public/screenshots/hyper-copilot.png)

### 3. 【新增】Squad 流水線（DAG）

Plan → Fork → Implement / Review → Join；可對話建立，再畫布微調。

![](packages/public/screenshots/hyper-squad.png)

![](packages/public/screenshots/hyper-squad-canvas.png)

### 4. 【新增】Autopilot / Webhook / 飛書

定時、外部事件、飛書訊息三種入口，統一落到「建 Issue → 入隊」。

![](packages/public/screenshots/hyper-autopilot.png)

![](packages/public/screenshots/hyper-webhooks.png)

![](packages/public/screenshots/hyper-feishu.png)

### 5. 【新增】Console 工作區 + Host 選擇

- **隔離 worktree（繼承，預設）** — 獨立分支 / 目錄
- **主目錄控制台 Console（新增）** — 目前目錄與分支直接跑，不自動建分支 / 提交
- **執行 Host（新增）** — 本機或已配對遠端 Worker

![](packages/public/screenshots/hyper-create-console.png)

![](packages/public/screenshots/hyper-remote-access.png)

### 6. 【新增】手機看板佈局

![](packages/public/screenshots/hyper-mobile-board.png)

### 7. 【繼承】Workspace 會話 / Diff / Preview

原版核心能力，本 fork 原樣保留並繼續打磨。

![](packages/public/screenshots/hyper-sessions.png)

![](packages/public/screenshots/hyper-diffs.png)

![](packages/public/screenshots/hyper-preview.png)

---

## 快速開始

先完成所用 AI Agent 的登入認證，然後：

```bash
npx vibe-kanban
```

會啟動本地伺服器並開啟瀏覽器。

### 自託管 Remote（可選）

官方雲停服後，本倉庫用 Docker Remote + Relay + ElectricSQL 續上「多端同步 / 遠端看板」。開發棧一鍵腳本見倉庫根目錄 `scripts/vk-*.sh`（埠約定見 `scripts/vk-ports.sh`）。部署說明：[自託管指南](docs/self-hosting/deploy-docker.mdx)。

---

## 工作原理

### 核心概念

| 概念 | 說明 |
|------|------|
| **Project** | 看板專案（可關聯多個本地 git 倉庫） |
| **Issue** | 看板任務卡片 |
| **Workspace** | 執行環境：worktree 或 Console + Coding Agent |
| **Board Agent** | 可指派、可對話的看板角色；觸發後複用 Workspace 執行 |
| **Squad** | 多 Agent + DAG 流水線 |
| **Host** | 實際跑 Agent 的機器（本機或配對遠端） |

### 兩條工作流

**A. 原版流程（繼承，仍完全可用）**

1. 建 Issue → 手動建立 Workspace  
2. 看日誌 / Preview → Diff 審查 → 迭代  
3. 開 PR 合併  

**B. 動態看板流程（本 fork 新增）**

1. 建立 Board Agent（人設 + 預設 executor）  
2. 指派 Issue（或 @ / Webhook / 飛書 / Autopilot）  
3. Local watcher 入隊 → 自動開 Workspace  
4. 看板回寫進度 / 評論；可用 Copilot 釐清下一輪  
5. 複雜任務用 Squad 畫布編排  
6. Diff 審查 → 開 PR（與原版相同）

---

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
| Pi | Pi（**本 fork 新增**） |

安裝與認證見 [supported-coding-agents](docs/supported-coding-agents.mdx)。看板對話 runtime（Copilot / Agent 聊天）預設可接 Cursor SDK，與上表 Coding Executor 是不同層——前者是本 fork 新增的編排層，後者是原版執行層。

---

## MCP 伺服器

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

## CLI 參考

```bash
npx vibe-kanban               # 啟動本地 UI
npx vibe-kanban --mcp         # MCP stdio
npx vibe-kanban review        # 程式碼審查 CLI
npx vibe-kanban --help
```

## 文件

- [`docs/`](docs/) — 使用者與自託管文件
- [`docs/board-agents-plan.md`](docs/board-agents-plan.md) — 動態看板 Agent 設計與分期
- [`docs/remote-access.mdx`](docs/remote-access.mdx) — Remote Access / 配對

## 支援與貢獻

功能建議用 [Discussions](https://github.com/magele758/hyper-vibekanban/discussions)，Bug 用 [Issues](https://github.com/magele758/hyper-vibekanban/issues)。提 PR 前建議先開 Discussion 對齊方案。

---

## 開發

### 環境要求

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

同時啟動 Rust 後端（`cargo-watch`）與 Vite。首次會從 `dev_assets_seed/` 複製空白 SQLite。

完整本機棧（Remote Docker + Relay + Desktop）可用：

```bash
bash scripts/vk-start.sh
bash scripts/vk-status.sh
```

### 從原始碼建置 npx 套件

```bash
./local-build.sh
cd npx-cli && node bin/cli.js
```

### 檢查與型別

```bash
pnpm run check
pnpm run lint
pnpm run format
pnpm run generate-types   # 勿手改 shared/types.ts
```

### 常用環境變數

| 變數 | 說明 |
|------|------|
| `FRONTEND_PORT` / `BACKEND_PORT` / `HOST` | 開發埠與繫結 |
| `VK_ALLOWED_ORIGINS` | 反向代理場景允許的來源 |
| `VK_SHARED_API_BASE` | Remote API（伺服端用 http） |
| `VK_SHARED_RELAY_API_BASE` | Relay API |
| `VK_TUNNEL` | 啟用 Relay 隧道 |

反向代理時必須設定 `VK_ALLOWED_ORIGINS`，否則後端會 `403`。遠端 SSH 編輯器整合見 **設定 → 編輯器整合**。
