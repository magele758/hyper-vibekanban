# Frontend Workspace 加载性能分析与优化

> 日期：2026-07-16  
> 范围：`packages/web-core` workspace 会话加载、看板数据层、打包体积

## 结论

- **「一直很慢」**：路由代码分割关闭 + 重型库同步进主 chunk，首屏基线差。
- **「越来越慢」**：成本与数据量耦合——workspace 数、会话轮次、单轮日志长度（本机已见单进程 JSONL 达 5.6MB）。

## 纵向切片

### 1. 静态资源

- `autoCodeSplitting: false`，全仓无 `React.lazy`
- lexical / shiki(@pierre/diffs) / xterm / highlight.js / @xyflow / @rjsf 同步进主 chunk
- 两套 diff 库并存；i18n 7 语言全量内联；codemirror / wa-sqlite 僵尸依赖

### 2. 启动瀑布与连接

```
sessions REST → 选中 session → processes WS Ready → normalized-logs WS 回放 → 首屏
```

- `_app` 常驻 active+archived workspaces 双 WS + 15s/60s 轮询
- `useDiffStream` 未用 `statsOnly` 时首屏拉全量 diff 正文
- `useExecutionProcesses` / `useApprovals` 按 hook 实例重复开 WS

### 3. 日志回放

- Turn 级分页已做（初始 1 轮）；**单轮内全量 JSON Patch 重放**
- `immediateFlush` 逐消息 immer produce → 近似 O(n²)
- running→completed 二次全量重拉

### 4. 状态派生

- live 流每帧 `entries.map(patchWithKey)` 破坏引用复用
- 每 rAF 全量 `deriveConversationEntries` / timeline
- `emitEntries` 每帧 sort+flatMap 只为取最后一条

### 5. 渲染

- streaming 尾部不虚拟化；`DisplayConversationEntry` 无 `React.memo`
- 看板：`ProjectProvider` 巨型 context；`KanbanContainer` O(issues×N) 扫描

## 优化清单与状态

| # | 项 | 批次 | 状态 |
|---|---|---|---|
| 1 | patchKey 仅增量打标 | 1 | ✅ |
| 2 | DisplayConversationEntry memo | 1 | ✅ |
| 3 | 历史回放 mutable + microtask 攒批（避免逐消息 immer O(n²)） | 1 | ✅ |
| 4 | WorkspaceProvider diff `statsOnly`，Changes 面板按需全量 | 1 | ✅ |
| 5 | emitEntries ExitPlanMode 增量化 | 1 | ✅ |
| 6 | 前端历史回放攒批（后端整帧快照 API 后续） | 2 | ✅ 前端 / 后端后续 |
| 7 | completed 后跳过无必要二次重拉 | 2 | ✅ |
| 8 | derive 按进程缓存 | 2 | ✅ |
| 9 | 启动瀑布压缩（sessions→processes→logs 聚合 API） | 2 | 📋 后续 |
| 10 | archived workspaces 流按需连接 | 2 | ✅ |
| 11 | 开启 autoCodeSplitting + optimizeDeps | 3 | ✅ |
| 12 | Approvals 共享 WS；ProcessesTab 复用 Provider | 3 | ✅ |
| 13 | Kanban 依赖 O(1) getWorkspacesForIssue | 3 | ✅ |
| 14 | ProjectProvider lookup 索引化 | 3 | ✅ |
| 15 | i18n 按语言动态加载 / 删僵尸依赖 / 双 diff 库收敛 | 3 | 📋 后续 |
| 16 | ProjectProvider 拆分 context | 3 | 📋 后续 |

## 已知回归与修复

- **加载更早消息为空（2026-07-16）**：历史回放曾用 `queueMicrotask` 攒批，与 WS `close`/`abort` 竞态，pending patch 被 `close()` 清空，Promise 以 `[]` resolve，并把空进程写入 displayed，导致无法再加载。已改回同步 flush（保留 mutable apply），`close()` 先 flush，loadMore 忽略空 batch。

## 验证建议

1. 打开长会话 workspace：首屏时间、流式时 FPS；点「Load earlier messages」应能 prepend 更早轮次
2. Network：diff WS 是否 `stats_only=true`；Approvals 是否只 1 条连接
3. 看板大 project：切换 status / 收 patch 时主线程是否不再线性卡顿
4. `pnpm run check` / `pnpm run format`
