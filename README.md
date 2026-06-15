<p align="center">
  <a href="https://vibekanban.com">
    <picture>
      <source srcset="packages/public/vibe-kanban-logo-dark.svg" media="(prefers-color-scheme: dark)">
      <source srcset="packages/public/vibe-kanban-logo.svg" media="(prefers-color-scheme: light)">
      <img src="packages/public/vibe-kanban-logo.svg" alt="Vibe Kanban Logo">
    </picture>
  </a>
</p>

<p align="center">Get 10X more out of Claude Code, Gemini CLI, Codex, Amp and other coding agents...</p>
<p align="center">
  <a href="https://www.npmjs.com/package/vibe-kanban"><img alt="npm" src="https://img.shields.io/npm/v/vibe-kanban?style=flat-square" /></a>
  <a href="https://github.com/BloopAI/vibe-kanban/blob/main/.github/workflows/publish.yml"><img alt="Build status" src="https://img.shields.io/github/actions/workflow/status/BloopAI/vibe-kanban/.github%2Fworkflows%2Fpublish.yml" /></a>
  <a href="https://deepwiki.com/BloopAI/vibe-kanban"><img src="https://deepwiki.com/badge.svg" alt="Ask DeepWiki"></a>
</p>

<p align="center">
  <a href="README.zh-Hans.md">简体中文</a> |
  <a href="README.zh-Hant.md">繁體中文</a> |
  <a href="README.ja.md">日本語</a> |
  <a href="README.ko.md">한국어</a> |
  <a href="README.es.md">Español</a> |
  <a href="README.fr.md">Français</a>
</p>

> **Note:** Vibe Kanban is sunsetting. [Read the announcement.](https://www.vibekanban.com/blog/shutdown) The project remains open source and fully functional for local self-hosting.

![](packages/public/vibe-kanban-screenshot-overview.png)

## Overview

Vibe Kanban is a local-first project management tool built for developers running AI coding agents. It streamlines the plan → execute → review loop so you can ship more, faster.

- **Plan with kanban issues** — create, prioritise, and manage issues on a kanban board
- **Run coding agents in workspaces** — each workspace provisions a git worktree, launches your chosen agent, and streams its output live
- **Review diffs and leave inline comments** — examine every changed line, annotate it, and send feedback back to the agent without leaving the UI
- **Preview your app** — built-in browser with devtools, inspect mode, and device emulation
- **10+ supported coding agents** — Claude Code, OpenAI Codex, Gemini CLI, GitHub Copilot, Amp, Cursor Agent CLI, OpenCode, Factory Droid, Claude Code Router (CCR), Qwen Code
- **Create pull requests and merge** — open PRs with AI-generated descriptions, review on GitHub/Azure, and merge

![](packages/public/vibe-kanban-screenshot-workspace.png)

## Quick Start

Make sure you have authenticated with your preferred coding agent first. Then run:

```bash
npx vibe-kanban
```

That's it. Vibe Kanban starts a local server, opens your browser, and you're ready to go.

## How It Works

### Core Concepts

| Concept | What it is |
|---------|-----------|
| **Project** | A git repository on your local machine |
| **Issue** | A task card on the kanban board (title + description + priority + tags) |
| **Workspace** | An isolated execution environment — git worktree + coding agent + optional dev server |

### Typical Workflow

1. **Create a project** — point Vibe Kanban at a local git repository
2. **Add issues** — describe what needs to be done on the kanban board
3. **Start a workspace** — choose your agent, branch, and optional setup/cleanup scripts; a git worktree is created automatically
4. **Watch the agent work** — live log streaming in the workspace view
5. **Review the diff** — unified or side-by-side diff viewer with line-level comment support
6. **Iterate** — submit your review comments; the agent reads them and continues
7. **Ship** — create a PR with an AI-generated description, review on GitHub, and merge

## Supported Coding Agents

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

See the [supported coding agents docs](https://vibekanban.com/docs/supported-coding-agents) for installation and authentication instructions.

## MCP Server

Vibe Kanban exposes a local [MCP (Model Context Protocol)](https://modelcontextprotocol.io/) server, so external clients (Claude Desktop, Raycast, other agents) can manage issues and workspaces programmatically.

```bash
# Start the MCP server
npx vibe-kanban --mcp
```

Or add it to your agent's MCP configuration:

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

## CLI Reference

```bash
npx vibe-kanban               # Start the local UI (default)
npx vibe-kanban --mcp         # Start the MCP stdio server
npx vibe-kanban review        # Run the code review CLI
npx vibe-kanban --help
npx vibe-kanban --version
```

## Documentation

Head to the [website](https://vibekanban.com/docs) for full documentation and user guides.

## Self-Hosting

Want to host your own Vibe Kanban Cloud instance? See the [self-hosting guide](https://vibekanban.com/docs/self-hosting/deploy-docker).

## Support

Use [GitHub Discussions](https://github.com/BloopAI/vibe-kanban/discussions) for feature requests and [GitHub Issues](https://github.com/BloopAI/vibe-kanban/issues) for bugs.

## Contributing

Please open a [GitHub Discussion](https://github.com/BloopAI/vibe-kanban/discussions) or join [Discord](https://discord.gg/AC4nwVtJM3) before submitting a PR so we can align on implementation details and roadmap fit.

---

## Development

### Prerequisites

- [Rust](https://rustup.rs/) (latest stable)
- [Node.js](https://nodejs.org/) (≥ 20)
- [pnpm](https://pnpm.io/) (≥ 8)

```bash
cargo install cargo-watch
cargo install sqlx-cli
pnpm i
```

### Running the dev server

```bash
pnpm run dev
```

Starts the Rust backend (hot-reload via `cargo-watch`) and the Vite frontend dev server concurrently. A blank SQLite database is copied from `dev_assets_seed/` on first run.

### Building the web app only

```bash
cd packages/local-web
pnpm run build
```

### Build from source (creates npx-cli distributable)

```bash
./local-build.sh
# Test the result:
cd npx-cli && node bin/cli.js
```

The script builds the React frontend, compiles three Rust binaries (`server`, `vibe-kanban-mcp`, `review`), zips them, and assembles the npx-cli package.

### Type checks & linting

```bash
pnpm run check   # TypeScript (all packages) + Rust cargo check
pnpm run lint    # ESLint + cargo clippy
pnpm run format  # Prettier + cargo fmt
```

### Regenerate shared TypeScript types

```bash
pnpm run generate-types
```

Types are derived from Rust structs via [ts-rs](https://github.com/Aleph-Alpha/ts-rs). Do **not** edit `shared/types.ts` directly — edit `crates/server/src/bin/generate_types.rs` instead.

### Environment Variables

| Variable | When | Default | Description |
|----------|------|---------|-------------|
| `PORT` | Runtime | Auto | Production server port. In dev: frontend port (backend = PORT+1) |
| `FRONTEND_PORT` | Runtime | `3000` | Dev-mode Vite port |
| `BACKEND_PORT` | Runtime | `0` (auto) | Dev-mode backend port |
| `HOST` | Runtime | `127.0.0.1` | Backend bind address |
| `MCP_HOST` | Runtime | `HOST` | MCP server connection host |
| `MCP_PORT` | Runtime | `BACKEND_PORT` | MCP server connection port |
| `VK_ALLOWED_ORIGINS` | Runtime | — | Comma-separated allowed origins (required behind a reverse proxy) |
| `VK_SHARED_API_BASE` | Runtime | — | Remote/cloud API base URL |
| `VK_SHARED_RELAY_API_BASE` | Runtime | — | Relay API base URL for tunnel-mode connections |
| `VK_TUNNEL` | Runtime | — | Enable relay tunnel mode |
| `DISABLE_WORKTREE_CLEANUP` | Runtime | — | Disable git worktree cleanup (useful during debugging) |
| `POSTHOG_API_KEY` | Build-time | — | PostHog analytics key (analytics disabled if empty) |
| `POSTHOG_API_ENDPOINT` | Build-time | — | PostHog analytics endpoint |

**Build-time variables** must be set when running `pnpm run build`. **Runtime variables** are read at startup.

#### Running behind a reverse proxy

Set `VK_ALLOWED_ORIGINS` to the full origin of your frontend — otherwise the backend rejects requests with `403 Forbidden`:

```bash
VK_ALLOWED_ORIGINS=https://vk.example.com npx vibe-kanban
# Multiple origins:
VK_ALLOWED_ORIGINS=https://vk.example.com,https://vk-staging.example.com npx vibe-kanban
```

#### Remote SSH editor integration

When running Vibe Kanban on a remote server, configure **Settings → Editor Integration** with your SSH host and user. The "Open in VSCode" buttons will generate `vscode://vscode-remote/ssh-remote+…` URLs that open your local editor connected to the remote machine.
