---
name: vk-worktree-preview
description: >-
  Isolates hyper-vibekanban worktree feature testing from the local main
  vk-start stack: push the current worktree branch, sync/pull on a
  developer-chosen SSH preview host, deploy vk-preview (ports 23xxx), and
  return Tailscale access URLs. Use when the user asks to preview/test a
  worktree on another machine, run vk-isolate preview-remote/full, deploy
  without touching local :1300x, or reuse the worktree→push→remote-pull→deploy
  flow. Ask the user for SSH Host and remote repo path in chat; never use
  config files or hardcode hosts/branch names.
---

# VK Worktree Preview（隔离预览）

在**本机主服务之外**试跑 **当前 worktree 的当前分支**。  
分支、SSH Host、远端路径：**动态解析或对话询问，禁止写死，禁止配置文件。**

## 硬约束

1. **分支**：`git rev-parse --abbrev-ref HEAD` / `HEAD`（当前 worktree 根）。禁止写死任何分支名。
2. **机器与路径**：若用户未提供 SSH Host 别名或远端仓库绝对路径 → **在对话中询问**，补齐后再跑。禁止猜 Host，禁止读写 `~/.config/vk-preview/` 或任何 preview config 文件。
3. **隐私**：不把真实 Host/IP/密码/JWT 写进仓库；回报 URL 时用探测结果即可。
4. **本机主服务**：勿动 `1300x`；禁止默认 `VK_TEST_REMOTE=1`。
5. **执行目录**：只在当前打开的 worktree 根执行。

## 对话里需要问清的参数

| 参数 | 用途 | 传入方式 |
|------|------|----------|
| SSH Host | `~/.ssh/config` 里的 Host 别名 | `--host` |
| 远端仓库绝对路径 | 预览机上的 clone 路径 | `--dir` |

可选再问：是否用 `rsync`（远端拉不下 GitHub 时）；登录账号若用户不知道再查远端 `.env.remote` 的 `SELF_HOST_*`（应用运行文件，不是本流程配置）。

## 标准流程

```bash
BRANCH=$(git rev-parse --abbrev-ref HEAD)
# 用户已告知 host / dir 之后：
./scripts/vk-isolate.sh preview-remote up --host <ssh-host> --dir /abs/path/on/remote
./scripts/vk-isolate.sh preview-remote smoke --host <ssh-host> --dir /abs/path/on/remote
```

`up` 会先 sync（push 当前分支 + 远端 checkout/pull 同名分支）。  
远端 `git fetch` 失败 → 问用户是否改用 `VK_PREVIEW_SYNC_METHOD=rsync`，见 [reference.md](reference.md)。

## 模式

| 前缀 | 用途 |
|------|------|
| `preview-remote` | 推荐：远端 Docker Remote |
| `preview-full` | 远端更重全栈 |
| `lite` / `unit` | 本机，不需要 Host |

## 完成后回报

SSH 到该 Host 执行 `tailscale ip -4`（或用户告知的访问方式）拼出：

- `http://<ip>:23000`（UI） / `/v1/health` / Relay `:28082`

确认本机 `vk-status` 仍 OK；远端 HEAD（或 rsync 内容）与当前 worktree 一致。

## 验收

- [ ] 缺 Host/路径时先问用户，未静默用配置文件
- [ ] 分支来自 `git rev-parse`
- [ ] `vk-preview` + 23xxx；本机主栈未动
- [ ] 已返回访问 URL
