<p align="center">
  <a href="https://github.com/magele758/hyper-vibekanban">
    <picture>
      <source srcset="packages/public/vibe-kanban-logo-dark.svg" media="(prefers-color-scheme: dark)">
      <source srcset="packages/public/vibe-kanban-logo.svg" media="(prefers-color-scheme: light)">
      <img src="packages/public/vibe-kanban-logo.svg" alt="Vibe Kanban Logo">
    </picture>
  </a>
</p>

<p align="center">Claude Code, Gemini CLI, Codex, Cursor, Pi 및 기타 코딩 에이전트의 생산성을 10배 높이세요...</p>
<p align="center">
  <a href="https://www.npmjs.com/package/vibe-kanban"><img alt="npm" src="https://img.shields.io/npm/v/vibe-kanban?style=flat-square" /></a>
  <a href="https://github.com/magele758/hyper-vibekanban/blob/main/.github/workflows/publish.yml"><img alt="Build status" src="https://img.shields.io/github/actions/workflow/status/magele758/hyper-vibekanban/.github%2Fworkflows%2Fpublish.yml" /></a>
</p>

<p align="center">
  <a href="README.md">English</a> |
  <a href="README.zh-Hans.md">简体中文</a> |
  <a href="README.zh-Hant.md">繁體中文</a> |
  <a href="README.ja.md">日本語</a> |
  <a href="README.es.md">Español</a> |
  <a href="README.fr.md">Français</a>
</p>

> **참고:** 공식 Vibe Kanban 클라우드는 종료되었습니다. 이 저장소는 [BloopAI/vibe-kanban](https://github.com/BloopAI/vibe-kanban) 의 포크(hyper-vibekanban)입니다: **업스트림 기능은 모두 유지**되며, 새로운 **동적 보드 에이전트** 레이어와 셀프호스팅 Remote가 추가됩니다.

![](packages/public/screenshots/hyper-board.png)

## 업스트림 대비: 유지하는 것 / 추가하는 것

업스트림은 강력한 “수동으로 Workspace를 여는” 에이전트 워크벤치 + 칸반입니다. 이 포크는 그 구조를 그대로 유지하고, **보드 이벤트 → 자동 인큐 → 실행 → 결과 기록**과 종료된 클라우드를 대체하는 셀프호스팅을 추가합니다.

### ✅ 업스트림에서 계승 (완전 유지)

| 기능 | 설명 |
|------------|------------|
| **Kanban issues** | 생성 / 우선순위 / 태그 / 서브 이슈 / Team·Personal |
| **Workspace + git worktree** | 에이전트 선택, 격리된 worktree, 실시간 로그 스트림 |
| **Sessions & follow-ups** | 멀티 세션 채팅, 첨부, @-files |
| **Inline diff review** | Unified / side-by-side; 댓글은 에이전트로 다시 전달 |
| **App preview** | 내장 브라우저, DevTools, inspect, 기기 에뮬레이션 |
| **Coding agents** | Claude Code, Codex, Gemini, Copilot, Amp, Cursor, OpenCode, Droid, CCR, Qwen |
| **Git / PRs** | Rebase, 충돌 UX, AI PR 설명, GitHub / Azure 머지 |
| **MCP + Review CLI** | `npx vibe-kanban --mcp` / `review` |
| **Settings** | Agent profiles, MCP, 에디터 통합, 알림, org / projects |

기존 경로도 그대로 동작합니다: **issue → 직접 Workspace 열기 → 로그 → diff 리뷰 → PR**.

### ✨ 이 포크에서 추가 (업스트림에 없음)

| 기능 | 설명 |
|------------|------------|
| **Board Agents** | 보드에서 에이전트가 일급 객체; **assign → enqueue**; 로컬 watcher가 Workspace를 열고 진행 / 댓글을 다시 기록 |
| **Project Copilot** | 보드 측 채팅(기본 Cursor SDK)으로 작업을 명확히 하고 할당을 제안 — 파일을 수정하는 코딩 실행기가 **아님** |
| **Squad DAG** | 멀티 에이전트 파이프라인: Fork / Join / If / While; 캔버스 에디터; 선택적 chat-to-pipeline |
| **Autopilot** | Cron + 타임존; issue 생성 또는 agent / squad 실행; 동시성 skip / queue |
| **Webhooks** | 외부 POST → issue 생성 / 작업 인큐 |
| **Feishu bot** | Feishu 메시지 → issue 큐; 완료 시 선택적 회신 |
| **Console workspaces** | 새 worktree를 강제하지 않고 저장소의 **현재 dir / branch**에서 실행 |
| **Host picker on create** | 이 머신 또는 페어링된 remote worker에서 workspace 실행 |
| **Mobile board layout** | 스마트폰용 단일 열 + 상태 pills |
| **Pi coding agent** | 추가 Workspace 실행기로서의 Pi CLI |
| **Self-hosted Remote stack** | 클라우드 종료 후 Docker Remote + Relay + ElectricSQL (`scripts/vk-*.sh`) |

### 🔄 업스트림 대비 강화

| 영역 | 업스트림 | 이 포크 |
|------|----------|-----------|
| Remote Access | 공식 클라우드 페어링 | **Self-hosted** Remote / Relay; worker-host SOP |
| Board | 정적 카드 + 수동 Workspace | 진행 기록 쓰기가 있는 **할당 가능한 agents / squads** |
| Triggers | UI / MCP로 Workspace 생성 | 추가로: assign, @, Autopilot, webhook, Feishu |

---

## 기능 쇼케이스

데모 데이터만 사용(Demo Org / Demo Showcase). **[신규]** / **[계승]** 으로 표시.

### 1. [신규] 동적 보드 + Board Agents

보드에서 에이전트를 할당하면 실행이 자동으로 인큐되고 결과가 다시 기록됩니다.

![](packages/public/screenshots/hyper-board.png)

![](packages/public/screenshots/hyper-agents.png)

### 2. [신규] Project Copilot

작업을 명확히 하기 위한 채팅/오케스트레이션 레이어; 코딩은 여전히 Workspace 실행기에서 수행됩니다.

![](packages/public/screenshots/hyper-copilot.png)

### 3. [신규] Squad 파이프라인 (DAG)

Plan → Fork → Implement / Review → Join; 채팅으로 만들고 캔버스에서 미세 조정.

![](packages/public/screenshots/hyper-squad.png)

![](packages/public/screenshots/hyper-squad-canvas.png)

### 4. [신규] Autopilot / Webhooks / Feishu

모두 “issue 생성 → 인큐”로 이어지는 세 가지 추가 진입점.

![](packages/public/screenshots/hyper-autopilot.png)

![](packages/public/screenshots/hyper-webhooks.png)

![](packages/public/screenshots/hyper-feishu.png)

### 5. [신규] Console workspace + host picker

- **Isolated worktree (계승, 기본)** — 전용 branch / dir
- **Console (신규)** — 현재 dir / branch; 자동 branch / commit 없음
- **Execution host (신규)** — 이 머신 또는 페어링된 remote worker

![](packages/public/screenshots/hyper-create-console.png)

![](packages/public/screenshots/hyper-remote-access.png)

### 6. [신규] 모바일 보드 레이아웃

![](packages/public/screenshots/hyper-mobile-board.png)

### 7. [계승] Workspace sessions / diffs / preview

업스트림 코어를 유지하고 다듬었습니다.

![](packages/public/screenshots/hyper-sessions.png)

![](packages/public/screenshots/hyper-diffs.png)

![](packages/public/screenshots/hyper-preview.png)

---

## 빠른 시작

선호하는 코딩 에이전트로 먼저 인증한 다음:

```bash
npx vibe-kanban
```

로컬 서버가 시작되고 브라우저가 열립니다.

### Self-hosted Remote (선택)

공식 클라우드 종료 이후, 이 저장소는 멀티 디바이스 동기화를 위한 Docker Remote + Relay + ElectricSQL 스택을 제공합니다. 개발 헬퍼는 `scripts/vk-*.sh`(포트는 `scripts/vk-ports.sh`)에 있습니다. [셀프호스팅 가이드](docs/self-hosting/deploy-docker.mdx)를 참고하세요.

---

## 동작 방식

### 핵심 개념

| 개념 | 설명 |
|---------|------------|
| **Project** | 칸반 프로젝트(여러 로컬 git 저장소 연결 가능) |
| **Issue** | 보드 위의 작업 카드 |
| **Workspace** | 실행 환경: worktree 또는 Console + coding agent |
| **Board Agent** | 할당 가능한 채팅 역할; 실행은 workspaces를 재사용 |
| **Squad** | 멀티 에이전트 + DAG 파이프라인 |
| **Host** | 에이전트를 실제로 실행하는 머신(로컬 또는 페어링됨) |

### 두 가지 워크플로

**A. 업스트림 플로 (계승, 여전히 완전 지원)**

1. Issue 생성 → Workspace를 수동으로 열기  
2. 로그 / Preview 확인 → diffs 리뷰 → 반복  
3. PR을 열고 머지  

**B. 동적 보드 플로 (이 포크의 신규)**

1. Board Agent 생성(persona + 기본 executor)  
2. Issue 할당(또는 @ / webhook / Feishu / Autopilot으로 트리거)  
3. 로컬 watcher가 작업을 인큐하고 Workspace를 염  
4. 진행 / 댓글이 다시 기록됨; 선택적으로 Copilot으로 명확화  
5. Squad 캔버스로 멀티 역할 작업 오케스트레이션  
6. Diffs 리뷰 → PR 열기(업스트림과 동일)

---

## 지원 코딩 에이전트

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
| Pi | Pi (**이 포크에서 추가**) |

[지원 코딩 에이전트](docs/supported-coding-agents.mdx)를 참고하세요. 보드 채팅 런타임(Copilot / agent chat)은 이러한 코딩 실행기와 별도 레이어입니다 — 채팅/오케스트레이션은 이 포크의 신규 기능이고, 코딩 실행기는 업스트림 실행 레이어입니다.

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

## CLI 참고

```bash
npx vibe-kanban               # Local UI
npx vibe-kanban --mcp         # MCP stdio
npx vibe-kanban review        # Review CLI
npx vibe-kanban --help
```

## 문서

- [`docs/`](docs/) — 사용자 + 셀프호스팅 문서
- [`docs/board-agents-plan.md`](docs/board-agents-plan.md) — board-agent 설계
- [`docs/remote-access.mdx`](docs/remote-access.mdx) — remote access / pairing

## 지원 및 기여

아이디어는 [Discussions](https://github.com/magele758/hyper-vibekanban/discussions), 버그는 [Issues](https://github.com/magele758/hyper-vibekanban/issues)를 이용하세요. 큰 PR 전에 Discussion을 열어 주세요.

---

## 개발

### 사전 요구 사항

- [Rust](https://rustup.rs/) (최신 stable)
- [Node.js](https://nodejs.org/) (≥ 20)
- [pnpm](https://pnpm.io/) (≥ 8)

```bash
cargo install cargo-watch
cargo install sqlx-cli
pnpm i
```

### 개발 서버

```bash
pnpm run dev
```

Rust 백엔드(`cargo-watch`)와 Vite를 시작합니다. 최초 실행 시 빈 SQLite DB가 `dev_assets_seed/`에서 복사됩니다.

전체 로컬 스택(Remote Docker + Relay + Desktop):

```bash
bash scripts/vk-start.sh
bash scripts/vk-status.sh
```

### 소스에서 npx 패키지 빌드

```bash
./local-build.sh
cd npx-cli && node bin/cli.js
```

### 검사 및 타입

```bash
pnpm run check
pnpm run lint
pnpm run format
pnpm run generate-types   # do not edit shared/types.ts by hand
```

### 자주 쓰는 환경 변수

| Variable | Description |
|----------|-------------|
| `FRONTEND_PORT` / `BACKEND_PORT` / `HOST` | 개발 포트 / bind |
| `VK_ALLOWED_ORIGINS` | 리버스 프록시 뒤에서 허용할 origins |
| `VK_SHARED_API_BASE` | Remote API (서버는 http를 사용해야 함) |
| `VK_SHARED_RELAY_API_BASE` | Relay API |
| `VK_TUNNEL` | Relay 터널 모드 활성화 |

리버스 프록시 시 `VK_ALLOWED_ORIGINS`를 설정하지 않으면 백엔드가 `403`을 반환합니다. Remote SSH 에디터 통합은 **Settings → Editor Integration**에 있습니다.
