# E2E 自测回归（针对本机 vk-start 主服务）

## 能不能直接测主服务？

**可以。** 默认就打正在跑的主栈：

| 目标 | 默认地址 |
|------|----------|
| Desktop UI | `http://localhost:13001` |
| Local API | `http://127.0.0.1:13002` |
| Remote | `http://127.0.0.1:13000` |
| Relay | `http://127.0.0.1:18082` |

先 `bash scripts/vk-status.sh` 全 OK，再跑测试。不新起一套环境。

## 命令

```bash
# 冒烟 + API 健康（不含视觉基线更新）
pnpm run test:e2e

# 首次 / UI 故意变更后更新截图基线
pnpm run test:e2e:update-snapshots

# 只跑 API 健康
pnpm exec playwright test --config e2e/playwright.config.ts e2e/tests/health.api.spec.ts

# 覆盖目标（极少需要）
VK_E2E_BASE_URL=http://localhost:13001 pnpm run test:e2e
```

## 分层

| 文件 | 层 | 内容 |
|------|----|------|
| `tests/health.api.spec.ts` | L1/L2 | 主服务 health / auth / repos |
| `tests/api-truth.spec.ts` | L2 | token→remote orgs、workspaces 列表 |
| `tests/app-shell.spec.ts` | L3 | 壳加载、workspaces、Cmd+K |
| `tests/kanban.spec.ts` | L3 | 看板 chrome、打开 composer、搜索 |
| `tests/issue-crud.spec.ts` | L3 真值 | 创建→搜索→打开→改标题→改状态 |
| `tests/board-filters.spec.ts` | L3 | 活动/全部/Team、Filters、Settings |
| `tests/workspaces-truth.spec.ts` | L3 | 列表/搜索/打开已有 workspace |
| `tests/visual.spec.ts` | L4 | 截图基线 diff |

## Agent Definition of Done

见 [`AGENT_DOD.md`](./AGENT_DOD.md)。
