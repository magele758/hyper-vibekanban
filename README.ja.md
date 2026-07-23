<p align="center">
  <a href="https://github.com/magele758/hyper-vibekanban">
    <picture>
      <source srcset="packages/public/vibe-kanban-logo-dark.svg" media="(prefers-color-scheme: dark)">
      <source srcset="packages/public/vibe-kanban-logo.svg" media="(prefers-color-scheme: light)">
      <img src="packages/public/vibe-kanban-logo.svg" alt="Vibe Kanban Logo">
    </picture>
  </a>
</p>

<p align="center">Claude Code、Gemini CLI、Codex、Cursor、Pi をはじめとするコーディングエージェントの生産性を 10 倍に...</p>
<p align="center">
  <a href="https://www.npmjs.com/package/vibe-kanban"><img alt="npm" src="https://img.shields.io/npm/v/vibe-kanban?style=flat-square" /></a>
  <a href="https://github.com/magele758/hyper-vibekanban/blob/main/.github/workflows/publish.yml"><img alt="Build status" src="https://img.shields.io/github/actions/workflow/status/magele758/hyper-vibekanban/.github%2Fworkflows%2Fpublish.yml" /></a>
</p>

<p align="center">
  <a href="README.md">English</a> |
  <a href="README.zh-Hans.md">简体中文</a> |
  <a href="README.zh-Hant.md">繁體中文</a> |
  <a href="README.ko.md">한국어</a> |
  <a href="README.es.md">Español</a> |
  <a href="README.fr.md">Français</a>
</p>

> **注意：** 公式の Vibe Kanban クラウドは終了しました。本リポジトリは [BloopAI/vibe-kanban](https://github.com/BloopAI/vibe-kanban) のフォーク（hyper-vibekanban）です：**上流の機能はすべて維持**しつつ、新しい **動的ボードエージェント** レイヤーとセルフホスト Remote を追加しています。

![](packages/public/screenshots/hyper-board.png)

## 上流との比較：継承するもの / 追加するもの

上流は強力な「手作業で Workspace を開く」エージェント作業台 + かんばんです。本フォークはそのまま維持し、さらに **ボードイベント → 自動エンキュー → 実行 → 書き戻し**、および終了したクラウドのセルフホスト代替を追加します。

### ✅ 上流から継承（完全維持）

| 機能 | 内容 |
|------------|------------|
| **Kanban issues** | 作成 / 優先度 / タグ / サブ issue / Team·Personal |
| **Workspace + git worktree** | エージェント選択、隔離された worktree、ライブログストリーム |
| **Sessions & follow-ups** | マルチセッションチャット、添付、@-files |
| **Inline diff review** | Unified / side-by-side；コメントはエージェントへ戻る |
| **App preview** | 内蔵ブラウザ、DevTools、inspect、デバイスエミュレーション |
| **Coding agents** | Claude Code、Codex、Gemini、Copilot、Amp、Cursor、OpenCode、Droid、CCR、Qwen |
| **Git / PRs** | Rebase、競合 UX、AI PR 説明、GitHub / Azure マージ |
| **MCP + Review CLI** | `npx vibe-kanban --mcp` / `review` |
| **Settings** | Agent profiles、MCP、エディタ連携、通知、org / projects |

従来のパスはそのまま使えます：**issue → 手で Workspace を開く → ログ → diff レビュー → PR**。

### ✨ 本フォークで追加（上流にはない）

| 機能 | 内容 |
|------------|------------|
| **Board Agents** | ボード上でエージェントが第一級；**assign → enqueue**；ローカル watcher が Workspace を開き、進捗 / コメントを書き戻す |
| **Project Copilot** | ボード側チャット（既定は Cursor SDK）で作業を明確化しアサインを提案 — ファイルを編集するコーディング実行器では**ない** |
| **Squad DAG** | マルチエージェントパイプライン：Fork / Join / If / While；キャンバスエディタ；任意でチャットからパイプライン生成 |
| **Autopilot** | Cron + タイムゾーン；issue 作成または agent / squad 実行；同時実行の skip / queue |
| **Webhooks** | 外部 POST → issue 作成 / 作業のエンキュー |
| **Feishu bot** | Feishu メッセージ → issue キュー；完了時の任意返信 |
| **Console workspaces** | 新しい worktree を強制せず、リポジトリの**現在の dir / branch**で実行 |
| **Host picker on create** | このマシン、またはペアリング済み remote worker で workspace を実行 |
| **Mobile board layout** | スマートフォン向け単一列 + ステータス pills |
| **Pi coding agent** | 追加の Workspace 実行器としての Pi CLI |
| **Self-hosted Remote stack** | クラウド終了後の Docker Remote + Relay + ElectricSQL（`scripts/vk-*.sh`） |

### 🔄 上流からの強化

| 領域 | 上流 | 本フォーク |
|------|----------|-----------|
| Remote Access | 公式クラウドペアリング | **Self-hosted** Remote / Relay；worker-host SOP |
| Board | 静的カード + 手動 Workspace | 進捗書き戻し付きの**アサイン可能な agents / squads** |
| Triggers | UI / MCP で Workspace 作成 | さらに：assign、@、Autopilot、webhook、Feishu |

---

## 機能紹介

デモデータのみ（Demo Org / Demo Showcase）。**[新規]** / **[継承]** で標記。

### 1. [新規] 動的ボード + Board Agents

ボード上でエージェントをアサイン；実行は自動エンキューされ、結果が書き戻されます。

![](packages/public/screenshots/hyper-board.png)

![](packages/public/screenshots/hyper-agents.png)

### 2. [新規] Project Copilot

作業を明確化するためのチャット / オーケストレーション層；コーディングは引き続き Workspace 実行器で行われます。

![](packages/public/screenshots/hyper-copilot.png)

### 3. [新規] Squad パイプライン（DAG）

Plan → Fork → Implement / Review → Join；チャットから作成し、キャンバスで微調整。

![](packages/public/screenshots/hyper-squad.png)

![](packages/public/screenshots/hyper-squad-canvas.png)

### 4. [新規] Autopilot / Webhooks / Feishu

いずれも「issue 作成 → エンキュー」に着地する 3 つの追加エントリポイント。

![](packages/public/screenshots/hyper-autopilot.png)

![](packages/public/screenshots/hyper-webhooks.png)

![](packages/public/screenshots/hyper-feishu.png)

### 5. [新規] Console workspace + host picker

- **Isolated worktree（継承、既定）** — 専用 branch / dir
- **Console（新規）** — 現在の dir / branch；自動 branch / commit なし
- **Execution host（新規）** — このマシン、またはペアリング済み remote worker

![](packages/public/screenshots/hyper-create-console.png)

![](packages/public/screenshots/hyper-remote-access.png)

### 6. [新規] モバイルボードレイアウト

![](packages/public/screenshots/hyper-mobile-board.png)

### 7. [継承] Workspace sessions / diffs / preview

上流のコア機能を維持し、磨き上げています。

![](packages/public/screenshots/hyper-sessions.png)

![](packages/public/screenshots/hyper-diffs.png)

![](packages/public/screenshots/hyper-preview.png)

---

## クイックスタート

まず好みのコーディングエージェントで認証してから：

```bash
npx vibe-kanban
```

ローカルサーバーが起動し、ブラウザが開きます。

### Self-hosted Remote（任意）

公式クラウド終了後、本リポジトリはマルチデバイス同期向けに Docker Remote + Relay + ElectricSQL スタックを同梱しています。開発ヘルパーは `scripts/vk-*.sh`（ポートは `scripts/vk-ports.sh`）にあります。[セルフホスティングガイド](docs/self-hosting/deploy-docker.mdx) を参照してください。

---

## 仕組み

### コア概念

| 概念 | 内容 |
|---------|------------|
| **Project** | かんばんプロジェクト（複数のローカル git リポジトリを紐付け可能） |
| **Issue** | ボード上のタスクカード |
| **Workspace** | 実行環境：worktree または Console + coding agent |
| **Board Agent** | アサイン可能なチャットロール；実行は workspaces を再利用 |
| **Squad** | マルチエージェント + DAG パイプライン |
| **Host** | エージェントを実際に実行するマシン（ローカルまたはペアリング済み） |

### 2 つのワークフロー

**A. 上流フロー（継承、引き続き完全サポート）**

1. Issue を作成 → Workspace を手動で開く  
2. ログ / Preview を確認 → diffs をレビュー → 反復  
3. PR を開いてマージ  

**B. 動的ボードフロー（本フォークの新規）**

1. Board Agent を作成（persona + 既定 executor）  
2. Issue をアサイン（または @ / webhook / Feishu / Autopilot でトリガー）  
3. ローカル watcher が作業をエンキューし Workspace を開く  
4. 進捗 / コメントが書き戻される；任意で Copilot で明確化  
5. Squad キャンバスでマルチロール作業をオーケストレーション  
6. Diffs をレビュー → PR を開く（上流と同じ）

---

## 対応コーディングエージェント

| Agent | Provider |
|-------|----------|
| Claude Code | Anthropic |
| OpenAI Codex CLI | OpenAI |
| Gemini CLI | Google |
| GitHub Copilot CLI | GitHub |
| Amp | Sourcegraph |
| Cursor Agent CLI | Anysphere |
| OpenCode | SST |
| Factory Droid | Factory AI |
| Claude Code Router (CCR) | Community |
| Qwen Code | Alibaba |
| Pi | Pi（**本フォークで追加**） |

[対応コーディングエージェント](docs/supported-coding-agents.mdx) を参照。ボードチャットランタイム（Copilot / agent chat）はこれらのコーディング実行器とは別レイヤーです — チャット / オーケストレーションは本フォークの新規機能；コーディング実行器は上流の実行レイヤーです。

---

## MCP Server

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

## CLI リファレンス

```bash
npx vibe-kanban               # Local UI
npx vibe-kanban --mcp         # MCP stdio
npx vibe-kanban review        # Review CLI
npx vibe-kanban --help
```

## ドキュメント

- [`docs/`](docs/) — ユーザー向け + セルフホスティングドキュメント
- [`docs/board-agents-plan.md`](docs/board-agents-plan.md) — board-agent 設計
- [`docs/remote-access.mdx`](docs/remote-access.mdx) — remote access / pairing

## サポートとコントリビューション

アイデアは [Discussions](https://github.com/magele758/hyper-vibekanban/discussions)、バグは [Issues](https://github.com/magele758/hyper-vibekanban/issues) へ。大きな PR の前に Discussion を開いてください。

---

## 開発

### 前提条件

- [Rust](https://rustup.rs/)（最新の stable）
- [Node.js](https://nodejs.org/)（≥ 20）
- [pnpm](https://pnpm.io/)（≥ 8）

```bash
cargo install cargo-watch
cargo install sqlx-cli
pnpm i
```

### 開発サーバー

```bash
pnpm run dev
```

Rust バックエンド（`cargo-watch`）と Vite を起動します。初回実行時に空の SQLite DB が `dev_assets_seed/` からコピーされます。

フルローカルスタック（Remote Docker + Relay + Desktop）：

```bash
bash scripts/vk-start.sh
bash scripts/vk-status.sh
```

### ソースから npx パッケージをビルド

```bash
./local-build.sh
cd npx-cli && node bin/cli.js
```

### チェックと型生成

```bash
pnpm run check
pnpm run lint
pnpm run format
pnpm run generate-types   # do not edit shared/types.ts by hand
```

### よく使う環境変数

| Variable | Description |
|----------|-------------|
| `FRONTEND_PORT` / `BACKEND_PORT` / `HOST` | 開発ポート / bind |
| `VK_ALLOWED_ORIGINS` | リバースプロキシ背後で許可する origins |
| `VK_SHARED_API_BASE` | Remote API（サーバーは http を使用すること） |
| `VK_SHARED_RELAY_API_BASE` | Relay API |
| `VK_TUNNEL` | Relay トンネルモードを有効化 |

リバースプロキシ時は `VK_ALLOWED_ORIGINS` を設定しないとバックエンドが `403` を返します。Remote SSH エディタ連携は **Settings → Editor Integration** にあります。
