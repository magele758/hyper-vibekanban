# Repository Guidelines

## Project Structure & Module Organization
- `crates/`: Rust workspace crates — `server` (API + bins), `db` (SQLx models/migrations), `executors`, `services`, `utils`, `git` (Git operations), `api-types` (shared API types for local + remote), `review` (PR review tool), `deployment`, `local-deployment`, `remote`.
- `packages/local-web/`: Local React + TypeScript app entrypoint (Vite, Tailwind). Shell source in `packages/local-web/src`.
- `packages/remote-web/`: Remote deployment frontend entrypoint.
- `packages/web-core/`: Shared React + TypeScript frontend library used by local + remote web (`packages/web-core/src`).
- `shared/`: Generated TypeScript types (`shared/types.ts`, `shared/remote-types.ts`) and agent tool schemas (`shared/schemas/`). Do not edit generated files directly.
- `assets/`, `dev_assets_seed/`, `dev_assets/`: Packaged and local dev assets.
- `npx-cli/`: Files published to the npm CLI package.
- `scripts/`: Dev helpers (ports, DB preparation).
- `docs/`: Documentation files.

### Crate-specific guides
- [`crates/remote/AGENTS.md`](crates/remote/AGENTS.md) — Remote server architecture, ElectricSQL integration, mutation patterns, environment variables.
- [`docs/AGENTS.md`](docs/AGENTS.md) — Mintlify documentation writing guidelines and component reference.
- [`packages/local-web/AGENTS.md`](packages/local-web/AGENTS.md) — Web app design system styling guidelines.

## Managing Shared Types Between Rust and TypeScript

ts-rs allows you to derive TypeScript types from Rust structs/enums. By annotating your Rust types with #[derive(TS)] and related macros, ts-rs will generate .ts declaration files for those types.
When making changes to the types, you can regenerate them using `pnpm run generate-types`
Do not manually edit shared/types.ts, instead edit crates/server/src/bin/generate_types.rs

For remote/cloud types, regenerate using `pnpm run remote:generate-types`
Do not manually edit shared/remote-types.ts, instead edit crates/remote/src/bin/remote-generate-types.rs (see crates/remote/AGENTS.md for details).

## Build, Test, and Development Commands
- Install: `pnpm i`
- Run dev (web app + backend with ports auto-assigned): `pnpm run dev`
- Backend (watch): `pnpm run backend:dev:watch`
- Web app (dev): `pnpm run local-web:dev`
- Type checks: `pnpm run check` (frontend + all backend Rust workspaces) and `pnpm run backend:check` (all backend Rust workspaces, including `crates/remote`)
- Rust tests: `cargo test --workspace`
- Generate TS types from Rust: `pnpm run generate-types` (or `generate-types:check` in CI)
- Prepare SQLx (offline): `pnpm run prepare-db`
- Prepare SQLx (remote package, postgres): `pnpm run remote:prepare-db`
- Local NPX build: `pnpm run build:npx` then `pnpm pack` in `npx-cli/`
- Format code: `pnpm run format` (runs `cargo fmt` for all backend Rust workspaces + web-core/web Prettier)
- Lint: `pnpm run lint` (runs web/ui ESLint + `cargo clippy` for all backend Rust workspaces)

## Before Completing a Task
- Run `pnpm run format` to format all Rust workspaces and web code.

## Local Dev Stack (`vk-*`) — Agent 自行重启

修改会影响运行中服务时，**不要等用户说「重启」**，在任务完成前自行执行并确认健康：

| 改了什么 | 怎么做 |
|----------|--------|
| `packages/web-core` / `packages/remote-web`（进 Remote 镜像） | 等 CI 出 GHCR 后 `vk-stop && vk-start`（会 `pull`）；急需才 `VK_REBUILD=1` 本机编镜像 |
| `crates/server` / Desktop 本机 API | 默认跑 `~/.vk-kanban/bin/server`（无 cargo-watch）。改 Rust 才 `VK_HOT=1 vk-start` |
| `crates/relay-*` | 等 CI 镜像，或 `VK_REBUILD=1`；容器重建后需重连隧道（`vk-start` 会重启 Desktop） |
| `scripts/vk-*.sh` / `docker-compose` / `.env.remote` | `vk-stop && vk-start` |
| 仅本地 Vite（`packages/local-web`） | 热更新；端口/代理/env 变了仍要 `vk-start` |

默认一键命令（仓库根目录）：

```bash
bash scripts/vk-stop.sh && bash scripts/vk-start.sh   # 拉 GHCR + 预编译 Desktop，不要加 VK_REBUILD
bash scripts/vk-status.sh   # 必须全 OK 再收工
# 改 Desktop Rust：VK_HOT=1 bash scripts/vk-start.sh
# 本机编 Remote/Relay 镜像：VK_REBUILD=1 bash scripts/vk-start.sh
```

默认端口见 `scripts/vk-ports.sh`（Desktop **13001**、Remote **13000**、Relay **18082**、API **13002**、桌面 h2 前门 **13443**）。手机经 Tailscale 访问 Remote 时用 **当前页面的 Tailscale IP/主机名** 配 Relay，不要用局域网 IP。

### OrbStack `:13000` 端口转发卡死（容器 healthy 但本机连不上）

**现象**：`docker ps` 里 `remote-remote-server` 仍 healthy，但本机 `curl http://127.0.0.1:13000/v1/health` connection refused；`vk-status` 报 Remote 挂。

**原因**：不是 Remote 进程挂了，而是 **OrbStack 在宿主机上的 `:13000` 端口转发层卡死**（常见大量 `CLOSED` 连接堆积）。容器内健康检查只打 `127.0.0.1:8081`，不经过宿主机端口映射，所以仍显示 healthy。

**诱因**：
1. **Docker Remote 与本地 `remote` 二进制混用同一 `:13000`**（最危险）——只用一种来源：Docker XOR 本地 binary。
2. Electric/shape 长连接经 Caddy → `:13000` 频繁建连/断连，加重转发层压力。
3. 多 tab 长时间挂看板、机器休眠唤醒（次要）。

**恢复（无需 `VK_REBUILD`）**：
1. `docker restart remote-remote-server-1`
2. 不够再：`vk-stop && vk-start`
3. 仍不行：重启 OrbStack

**避免**：日常只走 `vk-start` Docker 栈时，不要再起本地 `cargo-target-remote` / `remote-local`；以 `vk-status`（测宿主机端口）为准，别只看容器 healthy。过期的 `~/.vk-kanban/pids/remote-local.pid` 可删，避免误判本地 remote 还在。

### HTTP/2 桌面前门（切换丝滑的关键）

Remote API 是 HTTP/1.1，浏览器同源并发连接上限约 6 条；而一个看板需要约 9 条 Electric live 长连接，导致切换 project 时新请求被 `stalled/blocked`。解决办法是让浏览器经 **HTTP/2** 多路复用访问，所有 shape 共用一条连接。

- `vk-start` 会用 Caddy + 本地 CA 起一个桌面 h2 前门 `https://localhost:13443`，反代 `/v1`、`/shape` → Remote(13000)。
- **只把浏览器**的 `VITE_VK_SHARED_API_BASE` 指向它；服务端 `VK_SHARED_API_BASE` 保持 http（Rust reqwest 不需要信任本地 CA）。
- 脚本**安全自适应**：仅当本地 CA 已被系统信任时才把浏览器切到 h2，否则自动回退 http（应用照常工作）。
- 一次性信任本地 CA（需管理员密码，Caddy 必须在运行）：
  ```bash
  caddy trust            # 安装 Caddy 本地 CA 到系统信任库
  vk-stop && vk-start    # 重启后浏览器自动启用 HTTP/2
  ```
  完成后 `vk-status` 的 `Desktop h2` 显示 `(HTTP/2)`。临时关闭：`VK_DESKTOP_H2=0 vk-start`。
- h2 前门只服务**本机浏览器**；手机/局域网走 Tailscale h2 或 http。

### 各端访问方式（localhost vs Tailscale IP）

| 场景 | 打开的地址 | 说明 |
|------|-----------|------|
| 本机桌面 | `http://localhost:13001` | UI 仍从 13001 加载；CA 信任后 Electric 自动经 `https://localhost:13443` 走 h2。**不要**手动开 13443（它只是 API 前门，根路径返回占位文本） |
| 本机 Remote Web | `http://localhost:13000` | 同源，HTTP/1.1（如需 h2 同理需走 Caddy） |
| 手机（Tailscale IP 直连，默认可用） | `http://<tailscale-ip>:13001`（Desktop）/ `:13000`（Remote）/ `:18082`（Relay） | 走 HTTP/1.1，无需证书；配 Relay 用**当前页面的 Tailscale IP/主机名**，不要用局域网 IP |
| 手机（Tailscale + HTTPS，需 opt-in） | `https://<tailscale-hostname>:<VK_MOBILE_HTTPS_PORT>`，Relay `:18443` | 见下方「手机 HTTPS 前门」 |
| 手机（同一 WiFi 局域网） | `http://<lan-ip>:13001` / `:13000` / `:18082` | 仅同网段可达；跨网段用 Tailscale |

> Tailscale 主机名/IP 由脚本自动探测，启动横幅会打印当前可用地址。**注意**：脚本优先用 `/Applications/Tailscale.app/Contents/MacOS/Tailscale`（App Store 版 GUI 二进制经 PATH 软链接调用会 SIGTRAP 崩溃，必须用完整路径），需要时可用 `VK_TAILSCALE_BIN` 覆盖。

#### 手机 HTTPS 前门（opt-in）

默认**关闭**（`VK_MOBILE=0`）。开启就一条命令：

```bash
VK_MOBILE=1 vk-start
```

`VK_MOBILE_HTTPS_PORT` 默认 `13444`（已避开 Vite 的 13001，不再冲突）。开启后 `vk-start` 会用 `tailscale cert` 签发证书并起 `https://<tailscale-hostname>:13444` 前门（证书 Tailscale 原生签发，手机直接信任，顺带 h2）。开启时只把**浏览器**的 `VITE_VK_SHARED_API_BASE` / `VITE_RELAY_API_BASE_URL` 指向该 HTTPS，桌面 `localhost:13001` 也随之走该地址（公网受信证书，无需 `caddy trust`）。Relay 走 `:18443`。

> **关键**：mobile 模式下**服务端** `VK_SHARED_API_BASE` / `VK_SHARED_RELAY_API_BASE` 必须保持 `http://localhost:13000` / `http://127.0.0.1:18082`，**不要**改成 tailscale https。否则本地 Rust reqwest 会绕回 Caddy 经 `https://<tailscale>:13444/v1/tokens/refresh` 取 token 失败，导致 `Failed to get access token for relay`、`/api/auth/token` 502、project 拉不回来。原则：**只有浏览器走 HTTPS 前门，服务端始终直连本地 http。**

> 注意：`pnpm run dev` 脚本会保留外部已设置的 `VITE_VK_SHARED_API_BASE`（`${VITE_VK_SHARED_API_BASE:-${VK_SHARED_API_BASE:-}}`），否则浏览器会被强制改回 http LAN，h2 前门形同虚设。

## Remote/Relay 镜像发布与部署（GHCR）

自托管 Remote/Relay **只拉镜像、不本地编译**（避免几十 G 的本地 `target/` 缓存）。CI 由 `.github/workflows/remote-image.yml` 负责，发布两个**多架构（`linux/amd64` + `linux/arm64`）**镜像到 GHCR：

- `ghcr.io/magele758/hyper-vibekanban-remote`（`crates/remote/Dockerfile`）
- `ghcr.io/magele758/hyper-vibekanban-relay`（`crates/relay-tunnel/Dockerfile`）

### 构建时机（每天最多一次，无更新则跳过）

- **触发器**：每日定时 `schedule`（`cron: '0 19 * * *'` UTC）、`remote-v*` / `relay-v*` tag、手动 `workflow_dispatch`。**不再**在每次 push `main` 时构建。
- **变更门禁**：定时/手动（非 force）运行时，`changes` job 只有在「相关源码自上次已发布镜像以来有变化」才构建——判据是读取镜像最新 `sha-<short>` tag 对应的 commit，再 `git diff <last> HEAD -- <paths>`（**基于 commit 比对，不用 `--since` 时钟，避免时钟漂移误判**）。tag / `force` 一律构建。
- 相关 `paths`：`crates/remote`、`crates/relay-*`、`crates/ws-bridge`、`crates/api-types`、`packages/remote-web`、`packages/web-core`、以及该 workflow 自身。

### 架构与加速

- 每个架构在**原生 runner** 上分别构建（amd64→`ubuntu-latest`，arm64→`ubuntu-24.04-arm`，public 仓库免费），再用 `docker buildx imagetools` 合并成一个多架构清单。**不要用 QEMU 模拟**（remote arm64 模拟约 2h41m，原生约 7min）。
- `FEATURES=` 留空 → Dockerfile 会剥离私有 `billing` 依赖，构建**无需私有仓库/SSH**；GHCR 用内置 `GITHUB_TOKEN`，**无需额外 secret**。

### tag 规则与 `:latest`

- `:latest` 在 **tag 构建 或 默认分支（`main`）构建**时更新；其它分支不动 `:latest`。规则：
  `type=raw,value=latest,enable=${{ github.ref_type == 'tag' || github.ref == format('refs/heads/{0}', github.event.repository.default_branch) }}`
- 每次构建还会打 `sha-<short>`（可复现部署时固定它）、分支名、以及 tag 的语义版本。
- `cleanup` job 每个镜像只保留最新 10 个版本，持续构建不会无限占用存储（GHCR 容器镜像目前免费）。

### 手动出一个最新镜像

- 推荐：GitHub → Actions → “Build & Publish Images” → **Run workflow**（选 `main`）：构建 `main` 最新代码并刷新 `:latest`。
- 或**打一个 `remote-v*` / `relay-v*` tag**（发版即构建，且现在也会刷新 `:latest`）。
- 或**等每日定时**（有变更才会重建）。
- 注意：Cloud Agent 对 GitHub Actions 是**只读**权限，不能触发 `workflow_dispatch`，只能靠 **push tag** 间接触发构建。

### 部署（服务器，需 Docker + compose）

`crates/remote/docker-compose.yml` 的 `remote-server` / `relay-server` 已带 `image: ${REMOTE_IMAGE:-...}` / `${RELAY_IMAGE:-...}`（`build:` 仅作本地开发回退）。配好 `crates/remote/.env.remote` 后：

```bash
cd crates/remote
docker compose --env-file .env.remote --profile relay pull remote-server relay-server
docker compose --env-file .env.remote --profile relay up -d   # 不加 --build
```

- 首次把两个 GHCR 包设为 **public**（Package settings），部署机 `pull` 免登录。
- **数据在命名卷 `remote_remote-db-data`（Postgres），与镜像分离**：升级镜像（`pull` + `up -d`）会自动跑迁移、**数据保留**。务必保持同一 compose 项目（在 `crates/remote/` 目录里跑），卷才会自动挂回；**绝不要带 `-v`**（`docker compose down -v`、`pnpm run remote:dev:clean`，以及退出即 `down -v` 的 `pnpm run remote:dev`）除非要清库。停服务用 `docker compose stop` / `pnpm run remote:down`。

## Coding Style & Naming Conventions
- Rust: `rustfmt` enforced (`rustfmt.toml`); group imports by crate; snake_case modules, PascalCase types.
- TypeScript/React: ESLint + Prettier (2 spaces, single quotes, 80 cols). PascalCase components, camelCase vars/functions, kebab-case file names where practical.
- Keep functions small, add `Debug`/`Serialize`/`Deserialize` where useful.

## Testing Guidelines
- Rust: prefer unit tests alongside code (`#[cfg(test)]`), run `cargo test --workspace`. Add tests for new logic and edge cases.
- Web app: ensure `pnpm run check` and `pnpm run lint` pass. If adding runtime logic, include lightweight tests (e.g., Vitest) in the same directory.

## Security & Config Tips
- Use `.env` for local overrides; never commit secrets. Key envs: `FRONTEND_PORT`, `BACKEND_PORT`, `HOST` 
- Dev ports and assets are managed by `scripts/setup-dev-environment.js`.


