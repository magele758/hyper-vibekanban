# Coding-agent：worktree 分支 → 开发机预览（vk-isolate）

给 coding-agent / 开发者的固定流程。目标：在**本机主服务之外**的另一台机器上试跑 **当前 worktree 分支**，不污染本机 `vk-start`（1300x）。

## 隐私

- 仓库内禁止写死真实 SSH Host、Tailscale IP、密码、JWT、个人路径。
- 用户先在本机配好 SSH（`~/.ssh/config` Host 别名 + key），再把机器信息写进**本机私有配置**：
  - `~/.config/vk-preview/config.env`（由 `scripts/vk-preview.config.env.example` 复制）
- Agent **只在用户明确告知**「用哪台机器 / Host 别名」且配置已存在时，才对该 Host 执行 sync/up。
- 不要把 `config.env`、`.env.remote` 提交进 git。

## 用户需要提前准备

1. SSH：`ssh -o BatchMode=yes <Host>` 可登录。
2. 远端已 clone 同一仓库，路径写入 `VK_PREVIEW_DIR`（**绝对路径**）。
3. 远端已有 `crates/remote/.env.remote`（按 README，可与本机不同 JWT）。
4. 远端 Docker + Compose；Node≥20（Docker 构建用镜像内 Node，宿主机可不装 pnpm）。
5. 告诉 agent：`请用 SSH Host <别名> 做 preview`（不要让 agent 猜机器）。

## 标准流程（始终基于当前 worktree 分支）

在 **worktree 仓库根目录**（当前分支，例如 `vk/xxxx-...`）：

```bash
# 1) 推当前分支到 git remote，并在开发机 checkout+pull 同名分支
./scripts/vk-isolate.sh preview-remote sync

# 2) 仅在开发机起独立 Remote 栈（本机不启服务）
./scripts/vk-isolate.sh preview-remote up

# 3) 健康检查 / 日志
./scripts/vk-isolate.sh preview-remote smoke
./scripts/vk-isolate.sh preview-remote status
```

`up` 会先 `sync` 再远端启动。端口默认 `23000`（Remote）/ `28082`（Relay），与本机 `13000`/`18082` 隔离。

完成后 agent 应回报 **开发机 Tailscale（或用户配置的）访问地址**，例如：

- Remote UI：`http://<tailscale-ip>:23000`
- Health：`http://<tailscale-ip>:23000/v1/health`
- Relay：`http://<tailscale-ip>:28082`

（具体 host 来自远端 `tailscale ip -4` 或 `VK_PREVIEW_PUBLIC_BASE`，不要写进仓库。）

## 模式选择

| 模式 | 用途 |
|------|------|
| `lite` | 仅本机隔离壳（14001+），不连 Remote；快速测 local API |
| `unit` | 本机单测 |
| `preview-remote` | **推荐**：开发机 Docker Remote，测看板/登录/Electric |
| `preview-full` | 开发机 Remote + Desktop 风格（更重） |

主服务本机继续用 `vk-start`；preview **禁止**默认 `VK_TEST_REMOTE=1` 指回本机 Remote。

## Agent 验收清单

- [ ] 当前是 worktree 功能分支（非擅自改用户主工作区）
- [ ] 已 push 到 origin（或配置的 `VK_PREVIEW_GIT_REMOTE`）
- [ ] 开发机 `git rev-parse HEAD` 与 worktree 一致
- [ ] compose project 为 `vk-preview`，端口为 23xxx/28xxx
- [ ] 本机 `vk-status` 主服务仍 OK
- [ ] 向用户返回可访问 URL（Tailscale），不把私钥/完整 `.env` 贴进对话或 commit

## 开发机无法访问 GitHub 时

Docker 构建若依赖 `git` 源（如 `ts-rs`）且预览机访问不了 `github.com`：

1. 在**能访问 GitHub 的机器**上从 `~/.cargo/git/checkouts/` 取出对应 crate，放到预览机仓库的 `vendor/`（仅预览机本地，勿提交）。
2. 预览机临时把 workspace/`crates/remote` 的 `ts-rs` 改成 `path = "..."`，并 `COPY vendor` 进 Dockerfile；或用 `rsync` 从开发本机同步 `vendor/` + 临时 patch。
3. 构建仍用 `COMPOSE_PROJECT_NAME=vk-preview` 与 23xxx 端口。
4. 这些 patch **不要 push 进业务分支**；业务分支继续用正常 git remote 依赖。

预览机若也拉不下 `origin`，改用本机 `git push` 后经 Tailscale `rsync`/`scp` 同步当前 worktree 文件（仍以当前分支内容为准）。
