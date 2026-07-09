<p align="center">
  <a href="https://github.com/magele758/hyper-vibekanban">
    <picture>
      <source srcset="packages/public/vibe-kanban-logo-dark.svg" media="(prefers-color-scheme: dark)">
      <source srcset="packages/public/vibe-kanban-logo.svg" media="(prefers-color-scheme: light)">
      <img src="packages/public/vibe-kanban-logo.svg" alt="Vibe Kanban Logo">
    </picture>
  </a>
</p>

<p align="center">Claude Code, Gemini CLI, Codex, Amp 등 AI 코딩 에이전트의 생산성을 10배로 높이세요...</p>
<p align="center">
  <a href="https://www.npmjs.com/package/vibe-kanban"><img alt="npm" src="https://img.shields.io/npm/v/vibe-kanban?style=flat-square" /></a>
  <a href="https://github.com/magele758/hyper-vibekanban/blob/main/.github/workflows/publish.yml"><img alt="빌드 상태" src="https://img.shields.io/github/actions/workflow/status/magele758/hyper-vibekanban/.github%2Fworkflows%2Fpublish.yml" /></a>
</p>

<p align="center">
  <a href="README.md">English</a> |
  <a href="README.zh-Hans.md">简体中文</a> |
  <a href="README.zh-Hant.md">繁體中文</a> |
  <a href="README.ja.md">日本語</a> |
  <a href="README.es.md">Español</a> |
  <a href="README.fr.md">Français</a>
</p>

> **알림:** 공식 Vibe Kanban 클라우드 서비스는 종료되었습니다. 이 저장소는 오픈소스로 유지되며 로컬 셀프 호스팅은 완전히 사용 가능합니다.

![](packages/public/vibe-kanban-screenshot-overview.png)

## 개요

Vibe Kanban은 AI 코딩 에이전트와 협력하는 개발자를 위한 로컬 우선 프로젝트 관리 도구입니다. **계획 → 실행 → 리뷰** 사이클을 효율화하여 더 빠르게 코드를 출시할 수 있도록 도와줍니다.

- **칸반 이슈로 작업 계획** — 칸반 보드에서 이슈를 생성하고 우선순위를 관리
- **Workspace에서 AI 에이전트 실행** — 각 Workspace는 독립된 git worktree를 자동 생성하고 에이전트를 실행하며 로그를 실시간 스트리밍
- **Diff 리뷰 및 인라인 댓글 추가** — UI에서 변경 사항을 한 줄씩 확인하고 댓글을 달아 에이전트에 피드백 전송
- **앱 미리보기** — DevTools, 요소 검사, 디바이스 에뮬레이션을 지원하는 내장 브라우저
- **10종 이상의 AI 에이전트 지원** — Claude Code, OpenAI Codex, Gemini CLI, GitHub Copilot, Amp, Cursor Agent CLI, OpenCode, Factory Droid, Claude Code Router (CCR), Qwen Code
- **PR 생성 및 병합** — AI 생성 설명으로 PR을 만들고 GitHub/Azure에서 리뷰 후 병합

![](packages/public/vibe-kanban-screenshot-workspace.png)

## 빠른 시작

사용하려는 AI 에이전트 인증을 먼저 완료한 후 실행하세요:

```bash
npx vibe-kanban
```

명령 하나면 충분합니다. 로컬 서버가 시작되고 브라우저가 자동으로 열립니다.

## 동작 원리

### 핵심 개념

| 개념 | 설명 |
|------|------|
| **Project(프로젝트)** | 로컬 머신의 git 저장소 |
| **Issue(이슈)** | 칸반 보드의 태스크 카드 (제목 + 설명 + 우선순위 + 태그) |
| **Workspace(워크스페이스)** | 독립된 실행 환경 — git worktree + AI 에이전트 + 선택적 개발 서버 |

### 일반적인 워크플로

1. **프로젝트 생성** — 로컬 git 저장소를 Vibe Kanban에 연결
2. **이슈 추가** — 칸반 보드에 해야 할 작업 기술
3. **Workspace 시작** — 에이전트, 브랜치, 설정/정리 스크립트 선택 후 git worktree 자동 생성
4. **에이전트 작업 모니터링** — Workspace 뷰에서 실시간 로그 스트림 확인
5. **Diff 리뷰** — 통합 보기 또는 나란히 보기로 행 수준 댓글 추가
6. **반복** — 리뷰 댓글 제출, 에이전트가 내용을 읽고 수정 계속
7. **배포** — AI 생성 설명으로 PR 생성, GitHub에서 리뷰 후 병합

## 지원하는 AI 코딩 에이전트

| 에이전트 | 제공사 |
|----------|--------|
| Claude Code | Anthropic |
| OpenAI Codex CLI | OpenAI |
| Gemini CLI | Google |
| GitHub Copilot CLI | GitHub |
| Amp | Sourcegraph |
| Cursor Agent CLI | Anysphere |
| OpenCode | SST |
| Factory Droid | Factory AI |
| Claude Code Router (CCR) | 커뮤니티 |
| Qwen Code | Alibaba |

각 에이전트의 설치 및 인증 단계는 [문서](https://github.com/magele758/hyper-vibekanban/blob/main/docs/supported-coding-agents.mdx)를 참조하세요.

## MCP 서버

Vibe Kanban은 로컬 [MCP(Model Context Protocol)](https://modelcontextprotocol.io/) 서버를 내장하여 외부 클라이언트(Claude Desktop, Raycast 등)가 프로그래밍 방식으로 이슈와 Workspace를 관리할 수 있습니다.

```bash
# MCP 서버 시작
npx vibe-kanban --mcp
```

또는 에이전트의 MCP 설정에 추가:

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

## CLI 레퍼런스

```bash
npx vibe-kanban               # 로컬 UI 시작 (기본)
npx vibe-kanban --mcp         # MCP stdio 서버 시작
npx vibe-kanban review        # 코드 리뷰 CLI 실행
npx vibe-kanban --help
npx vibe-kanban --version
```

## 문서

전체 문서와 사용자 가이드는 이 저장소의 [`docs/`](docs/) 디렉터리를 참조하세요.


## 셀프 호스팅

자체 Vibe Kanban Cloud 인스턴스를 배포하려면 [셀프 호스팅 가이드](https://github.com/magele758/hyper-vibekanban/blob/main/docs/self-hosting/deploy-docker.mdx)를 참조하세요.


## 지원

기능 요청은 [GitHub Discussions](https://github.com/magele758/hyper-vibekanban/discussions), 버그 보고는 [GitHub Issues](https://github.com/magele758/hyper-vibekanban/issues)를 이용해 주세요.

## 기여

PR을 제출하기 전에 [GitHub Discussions](https://github.com/magele758/hyper-vibekanban/discussions) 또는 [Discord](https://discord.gg/AC4nwVtJM3)에서 구현 방향과 로드맵 적합성에 대해 핵심 팀과 먼저 논의해 주세요.

---

## 개발

### 사전 요구사항

- [Rust](https://rustup.rs/) (최신 안정 버전)
- [Node.js](https://nodejs.org/) (≥ 20)
- [pnpm](https://pnpm.io/) (≥ 8)

```bash
cargo install cargo-watch
cargo install sqlx-cli
pnpm i
```

### 개발 서버 실행

```bash
pnpm run dev
```

Rust 백엔드(`cargo-watch` 핫 리로드)와 Vite 프론트엔드 개발 서버를 동시에 시작합니다. 최초 실행 시 `dev_assets_seed/`에서 빈 SQLite 데이터베이스가 복사됩니다.

### 프론트엔드만 빌드

```bash
cd packages/local-web
pnpm run build
```

### 소스에서 빌드 (npx-cli 배포 패키지 생성)

```bash
./local-build.sh
# 테스트:
cd npx-cli && node bin/cli.js
```

### 타입 검사 및 린팅

```bash
pnpm run check   # TypeScript (전체 패키지) + Rust cargo check
pnpm run lint    # ESLint + cargo clippy
pnpm run format  # Prettier + cargo fmt
```

### 공유 TypeScript 타입 재생성

```bash
pnpm run generate-types
```

타입은 [ts-rs](https://github.com/Aleph-Alpha/ts-rs)를 통해 Rust 구조체에서 생성됩니다. `shared/types.ts`를 **직접 수정하지 마세요** — `crates/server/src/bin/generate_types.rs`를 수정하세요.

### 환경 변수

| 변수 | 시점 | 기본값 | 설명 |
|------|------|--------|------|
| `PORT` | 런타임 | 자동 | 프로덕션 서버 포트. 개발 시 프론트엔드 포트 (백엔드 = PORT+1) |
| `FRONTEND_PORT` | 런타임 | `3000` | 개발 모드 Vite 포트 |
| `BACKEND_PORT` | 런타임 | `0` (자동) | 개발 모드 백엔드 포트 |
| `HOST` | 런타임 | `127.0.0.1` | 백엔드 바인드 주소 |
| `VK_ALLOWED_ORIGINS` | 런타임 | — | 허용된 오리진 (쉼표 구분), 리버스 프록시 사용 시 필수 |
| `DISABLE_WORKTREE_CLEANUP` | 런타임 | — | git worktree 자동 정리 비활성화 (디버깅용) |
| `POSTHOG_API_KEY` | 빌드 시 | — | PostHog 분석 키 (비어있으면 분석 비활성화) |

#### 리버스 프록시 사용 시

`VK_ALLOWED_ORIGINS`를 프론트엔드의 전체 오리진 URL로 설정하세요. 설정이 없으면 백엔드가 `403 Forbidden`을 반환합니다:

```bash
VK_ALLOWED_ORIGINS=https://vk.example.com npx vibe-kanban
```
