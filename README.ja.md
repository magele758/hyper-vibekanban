<p align="center">
  <a href="https://vibekanban.com">
    <picture>
      <source srcset="packages/public/vibe-kanban-logo-dark.svg" media="(prefers-color-scheme: dark)">
      <source srcset="packages/public/vibe-kanban-logo.svg" media="(prefers-color-scheme: light)">
      <img src="packages/public/vibe-kanban-logo.svg" alt="Vibe Kanban Logo">
    </picture>
  </a>
</p>

<p align="center">Claude Code、Gemini CLI、Codex、Amp などのコーディング Agent を 10 倍活用しよう...</p>
<p align="center">
  <a href="https://www.npmjs.com/package/vibe-kanban"><img alt="npm" src="https://img.shields.io/npm/v/vibe-kanban?style=flat-square" /></a>
  <a href="https://github.com/BloopAI/vibe-kanban/blob/main/.github/workflows/publish.yml"><img alt="ビルド状態" src="https://img.shields.io/github/actions/workflow/status/BloopAI/vibe-kanban/.github%2Fworkflows%2Fpublish.yml" /></a>
  <a href="https://deepwiki.com/BloopAI/vibe-kanban"><img src="https://deepwiki.com/badge.svg" alt="Ask DeepWiki"></a>
</p>

<p align="center">
  <a href="README.md">English</a> |
  <a href="README.zh-Hans.md">简体中文</a> |
  <a href="README.zh-Hant.md">繁體中文</a> |
  <a href="README.ko.md">한국어</a> |
  <a href="README.es.md">Español</a> |
  <a href="README.fr.md">Français</a>
</p>

> **お知らせ：** Vibe Kanban はサービス終了を発表しました。[アナウンスをご確認ください](https://www.vibekanban.com/blog/shutdown)。プロジェクトはオープンソースとして公開を継続しており、ローカル自己ホスティングは引き続き完全に利用可能です。

![](packages/public/vibe-kanban-screenshot-overview.png)

## 概要

Vibe Kanban は、AI コーディング Agent と連携する開発者向けのローカルファーストなプロジェクト管理ツールです。**計画 → 実行 → レビュー** のサイクルを効率化し、より速くコードをリリースできます。

- **カンバン Issue で計画管理** — カンバンボード上で Issue を作成・優先順位付け・管理
- **Workspace で AI Agent を実行** — 各 Workspace は独立した git worktree を自動作成し、選択した Agent を起動してログをリアルタイムストリーミング
- **Diff をレビューしてインラインコメントを追加** — UI 内で変更内容を1行ずつ確認し、コメントを付けて Agent にフィードバックを送信
- **アプリプレビュー** — DevTools・要素検査・デバイスエミュレーション対応の内蔵ブラウザ
- **10 種以上の AI Agent に対応** — Claude Code、OpenAI Codex、Gemini CLI、GitHub Copilot、Amp、Cursor Agent CLI、OpenCode、Factory Droid、Claude Code Router (CCR)、Qwen Code
- **PR 作成 & マージ** — AI 生成の説明文で PR を作成し、GitHub/Azure でレビューしてマージ

![](packages/public/vibe-kanban-screenshot-workspace.png)

## クイックスタート

使用する AI Agent の認証を事前に完了させてから、以下を実行してください：

```bash
npx vibe-kanban
```

たった 1 コマンド。ローカルサーバーが起動し、ブラウザが自動で開きます。

## 仕組み

### コアコンセプト

| 概念 | 説明 |
|------|------|
| **Project（プロジェクト）** | ローカルマシン上の git リポジトリ |
| **Issue（タスク）** | カンバンボードのタスクカード（タイトル + 説明 + 優先度 + タグ） |
| **Workspace（ワークスペース）** | 独立した実行環境 — git worktree + AI Agent + オプションの開発サーバー |

### 典型的なワークフロー

1. **プロジェクトを作成** — ローカルの git リポジトリを Vibe Kanban に登録
2. **Issue を追加** — カンバンボードにやるべき作業を記述
3. **Workspace を起動** — Agent・ブランチ・セットアップスクリプトを選択し、git worktree を自動作成
4. **Agent の作業を監視** — Workspace ビューでリアルタイムのログストリームを確認
5. **Diff をレビュー** — ユニファイド表示またはサイドバイサイド表示で行レベルのコメントを追加
6. **イテレーション** — レビューコメントを送信し、Agent が内容を読んで修正を続行
7. **リリース** — AI 生成の説明文で PR を作成し、GitHub でレビューしてマージ

## 対応コーディング Agent

| Agent | 提供元 |
|-------|--------|
| Claude Code | Anthropic |
| OpenAI Codex CLI | OpenAI |
| Gemini CLI | Google |
| GitHub Copilot CLI | GitHub |
| Amp | Sourcegraph |
| Cursor Agent CLI | Anysphere |
| OpenCode | SST |
| Factory Droid | Factory AI |
| Claude Code Router (CCR) | コミュニティ |
| Qwen Code | Alibaba |

各 Agent のインストールと認証方法については[公式ドキュメント](https://vibekanban.com/docs/supported-coding-agents)をご覧ください。

## MCP サーバー

Vibe Kanban はローカル [MCP（Model Context Protocol）](https://modelcontextprotocol.io/) サーバーを内蔵しており、外部クライアント（Claude Desktop、Raycast など）からプログラマティックに Issue や Workspace を管理できます。

```bash
# MCP サーバーを起動
npx vibe-kanban --mcp
```

または Agent の MCP 設定に追加：

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
npx vibe-kanban               # ローカル UI を起動（デフォルト）
npx vibe-kanban --mcp         # MCP stdio サーバーを起動
npx vibe-kanban review        # コードレビュー CLI を実行
npx vibe-kanban --help
npx vibe-kanban --version
```

## ドキュメント

完全なドキュメントとユーザーガイドは[公式サイト](https://vibekanban.com/docs)をご覧ください。

## セルフホスティング

自分の Vibe Kanban Cloud インスタンスをデプロイしたい場合は[セルフホスティングガイド](https://vibekanban.com/docs/self-hosting/deploy-docker)をご参照ください。

## サポート

機能リクエストは [GitHub Discussions](https://github.com/BloopAI/vibe-kanban/discussions)、バグ報告は [GitHub Issues](https://github.com/BloopAI/vibe-kanban/issues) をご利用ください。

## コントリビュート

PR を提出する前に、[GitHub Discussions](https://github.com/BloopAI/vibe-kanban/discussions) または [Discord](https://discord.gg/AC4nwVtJM3) でコアチームと実装方針・ロードマップの整合性について事前に相談してください。

---

## 開発

### 必要環境

- [Rust](https://rustup.rs/)（最新安定版）
- [Node.js](https://nodejs.org/)（≥ 20）
- [pnpm](https://pnpm.io/)（≥ 8）

```bash
cargo install cargo-watch
cargo install sqlx-cli
pnpm i
```

### 開発サーバーの起動

```bash
pnpm run dev
```

Rust バックエンド（`cargo-watch` によるホットリロード）と Vite フロントエンド開発サーバーを同時に起動します。初回実行時は `dev_assets_seed/` から空の SQLite データベースがコピーされます。

### フロントエンドのみビルド

```bash
cd packages/local-web
pnpm run build
```

### ソースからビルド（npx-cli 配布パッケージを生成）

```bash
./local-build.sh
# テスト:
cd npx-cli && node bin/cli.js
```

### 型チェックとリント

```bash
pnpm run check   # TypeScript（全パッケージ）+ Rust cargo check
pnpm run lint    # ESLint + cargo clippy
pnpm run format  # Prettier + cargo fmt
```

### 共有 TypeScript 型の再生成

```bash
pnpm run generate-types
```

型は [ts-rs](https://github.com/Aleph-Alpha/ts-rs) を通じて Rust の構造体から生成されます。`shared/types.ts` を**直接編集しないでください** — `crates/server/src/bin/generate_types.rs` を編集してください。

### 環境変数

| 変数 | タイミング | デフォルト | 説明 |
|------|-----------|-----------|------|
| `PORT` | 実行時 | 自動 | 本番環境のサーバーポート。開発時はフロントエンドポート（バックエンド = PORT+1） |
| `FRONTEND_PORT` | 実行時 | `3000` | 開発モードの Vite ポート |
| `BACKEND_PORT` | 実行時 | `0`（自動）| 開発モードのバックエンドポート |
| `HOST` | 実行時 | `127.0.0.1` | バックエンドのバインドアドレス |
| `VK_ALLOWED_ORIGINS` | 実行時 | — | 許可するオリジン（カンマ区切り）、リバースプロキシ使用時は必須 |
| `DISABLE_WORKTREE_CLEANUP` | 実行時 | — | git worktree の自動クリーンアップを無効化（デバッグ用） |
| `POSTHOG_API_KEY` | ビルド時 | — | PostHog アナリティクスキー（空の場合は無効） |

#### リバースプロキシ使用時

`VK_ALLOWED_ORIGINS` にフロントエンドの完全なオリジン URL を設定してください。設定がない場合、バックエンドは `403 Forbidden` を返します：

```bash
VK_ALLOWED_ORIGINS=https://vk.example.com npx vibe-kanban
```
