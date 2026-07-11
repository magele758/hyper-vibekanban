# Agent Definition of Done — E2E 自测

完成带前端改动的任务前，Agent **必须**对主服务自测，禁止只靠口头「我看过了」。

## 必过

1. `bash scripts/vk-status.sh` — 相关行 OK  
2. `pnpm run check`（或至少改动面的 package check）  
3. `pnpm run test:e2e` — 对 **localhost:13001 主服务** 跑通  

失败则：修代码 → 再跑 → 最多 3 轮。仍失败则在报告里写清失败用例 + `e2e/test-results` 截图路径，不要标 Done。

## 何时更新视觉基线

仅当本次 **故意改了 UI** 且 diff 符合预期：

```bash
pnpm run test:e2e:update-snapshots
```

并在交付说明里写一句：更新了哪些 `*-snapshots`、为什么。

## 改功能时的测试义务

| 改了什么 | 至少覆盖 |
|----------|----------|
| 看板 / issue | `kanban.spec.ts` 相关断言或新用例 |
| 壳 / 导航 / 侧栏 | `app-shell.spec.ts` |
| 样式大改 | 更新 `visual.spec.ts` 基线 |
| API / auth / relay | `health.api.spec.ts` 或加 API 断言 |

## 禁止

- 用「更新基线」掩盖非预期回归  
- 跳过 preflight 去测一个挂掉的栈  
- 没有证据（测试绿 / 失败报告）就声称已验证  
