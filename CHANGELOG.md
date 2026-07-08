# Changelog

本 changelog 记录本 fork（`hyper-vibekanban`）从上游 [vibe-kanban](https://github.com/BloopAI/vibe-kanban) **v0.1.44** 分叉之后新增的能力点。上游自身的更新不在此列。

分叉基点：`4deb7eca chore: bump version to 0.1.44`

---

## 能力点（按主题）

### Workspace 模式

- **Console 模式（控制台工作区）** — 新增 `WorkspaceKind::Console`。Agent 直接在仓库自身的工作树、当前分支上运行，不创建 `vk/` 分支、不 checkout、不自动提交，工作树也无需保持干净。定位为主目录上的「控制平面」（管理 worktree/分支、合并、执行命令），而非隔离的功能开发。
  - 支持 **非 git 目录**：新增 `repos.is_git` 标记（迁移 `20260707000000_add_is_git_to_repos.sql`），Console 工作区可挂载到普通目录，分支/git 检查会对非 git 目录跳过。
  - Console 工作区的删除处理与分支状态预检做了非 git 目录的守卫，避免 git2 `NotFound` 阻塞删除等流程。
- **强制基于 worktree 的开发模式** — 创建工作区时统一走隔离 worktree 模式，收敛创建流程与相关 UI（`CreateChatBoxContainer`）。

### 对话流 / 前端渲染

- **Mermaid 图表渲染** — 对话流前端渲染支持 Mermaid 图表（`WYSIWYGEditor` + 新增 `mermaid-node`）。
- **对话流中的 Markdown 文件链接** — 对话流里生成的 Markdown 文件会以链接呈现，新增只读文件预览（`MarkdownFilePreviewContainer`、`ReadOnlyLinkPlugin`）与后端文件读取接口（`routes/workspaces/files.rs`），配套 7 种语言的 i18n。
- **对话滚动抖动修复** — 进入工作区、以及离开底部锁定时的滚动抖动问题修复。

### Issue / Workspace 管理

- **删除 Issue 时级联删除关联工作区** — 删除 Issue（含子 Issue）时可确认一并删除关联的本地工作区及其目录，Console 模式工作区被豁免。
- **侧边栏 diff 统计懒加载** — 将 git diff 统计从工作区 summary 接口拆出到独立的 `/workspaces/diff-stats`，侧边栏按需拉取；mark-seen 改为本地 patch 缓存而非重新请求。

### 多目录 workspace rebase 修复

- **修复多目录 workspace rebase 冲突解决后无效的问题** — rebase 冲突时 git 处于 detached HEAD，原自动提交路径 `git add -A && git commit` 会把冲突解决 commit 落在 detached HEAD 上、`vk/` 分支不前进、rebase 一直挂着，导致重新 rebase 又重放同一冲突。新增 `GitService::commit_or_continue_rebase`：rebase 进行中时改走 `git add -A` + `git rebase --continue`（带 `GIT_EDITOR=true` 防挂起），残留冲突标记时保持 rebase 进行中不误提交。附回归测试。

### 编码 Agent / Executor 支持

- **OpenCode 模型支持** — 完善 OpenCode executor 的模型发现与配置。
- **Cursor 相关修复** — Cursor executor 的命令、模型发现与安装流程修复（含文档更新）。
- **Claude 空 thinking 事件修复** — 排查并修复对话流中出现大量空 thinking 事件的问题。
- **Kimi 日志解析修复** — 正确解析无 content 的 Kimi 流事件，忽略 `thinking_tokens` 噪声。
- **executor 输出 UTF-8 边界修复** — 跨 stdout chunk 边界正确解码 UTF-8，避免多字节字符被截断。
- **coding-agent npx 包浮动到 `@latest`**；Claude Code CLI 升级到 2.1.183。

### 本地 / 远程开发栈（vk-* dev stack）

- **HTTP/2 前门加速项目切换** — 通过同源 Caddy HTTP/2 前门让约 9 条 Electric shape 长连接复用单条连接，绕开浏览器每源约 6 条 HTTP/1.1 并发上限（桌面 `https://localhost:13443`、手机 Tailscale `:13444`、relay 前门 `13445/18443`）。缩短 Electric 集合 GC、去掉相邻 project 预取以更快释放连接槽。
- **手机（Tailscale）访问链路修复** — 前门双栈绑定（`0.0.0.0 ::`）修复手机经 Tailscale IPv4 访问；浏览器 Remote API base 跟随页面 host；服务端 base 始终保持本地 http。
- **Remote relay / 隧道修复** — 修复本地 relay dev stack、隧道上的远程 workspace 代理、Remote Web 的 relay URL（Tailscale）、relay 流与 workspace 同步。
- **登录自启脚本** — 新增 `vk-autostart` 及安装/卸载脚本与 npm scripts，把本地 dev stack 注册为登录 LaunchAgent。
- **服务端与浏览器共享 API base 分离**；启动前清理继承的 `ANTHROPIC_*` provider 环境变量；loopback/OrbStack 走直连绕过代理、共享资产目录、等待 Tailscale 就绪。
- **Tailscale 移动端 5G direct-vs-DERP 排查文档**。

### CI / 文档 / 其他

- **CI 精简** — 去掉 macOS 与 Tauri 桌面构建，仅发布 npm CLI，消除 GitHub Actions 计费来源（macOS runner），Linux/Windows 后端 + npm CLI 产物不变。
- **README 重写 + 多语言** — 重写主 README，新增 zh-Hans / zh-Hant / ja / ko / es / fr 版本。
- **高风险子系统回归测试** — 为最高风险子系统补充回归覆盖。
- **cargo target 目录隔离** — 主 dev stack 与工作区 agent 的 cargo 构建使用各自的 `target/`，避免共享 `target/` 竞争。
- **忽略 playwright-mcp 产物与本地 derp note**。
