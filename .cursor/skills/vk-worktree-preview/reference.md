# VK Worktree Preview — 参考

## 不要用配置文件

本流程**不使用** `~/.config/vk-preview/config.env` 或仓库内 preview env 模板。  
参数一律：对话询问 → `--host` / `--dir`（或当次命令的环境变量）。

远端机器上若已有 `crates/remote/.env.remote`，那是 **Remote 服务自己的运行密钥**（JWT / 本地登录等），由 Docker Compose 读取；不是 vk-isolate 的「用户配置层」。缺失时在对话里问用户如何处理，不要在本机再搞一份平行配置。

## 端口

| 栈 | Remote | Relay |
|----|--------|-------|
| 本机 `vk-start` | 13000 | 18082 |
| 预览 `vk-preview` | 23000 | 28082 |

## 换 worktree

进入目标 worktree → 问清 Host/路径（可与上次相同）→ `preview-remote up --host ... --dir ...`。

## 预览机无法访问 GitHub

1. 本机仍 `git push` 当前分支。  
2. `VK_PREVIEW_SYNC_METHOD=rsync` + 同样的 `--host` / `--dir`。  
3. 构建卡在 git 依赖时：本机 `vendor/` rsync 到远端（勿提交）；细节按现场再问用户。

## 命令速查

```bash
./scripts/vk-isolate.sh preview-remote up --host <host> --dir /abs/path
./scripts/vk-isolate.sh preview-remote down --host <host> --dir /abs/path
./scripts/vk-isolate.sh lite up
```
