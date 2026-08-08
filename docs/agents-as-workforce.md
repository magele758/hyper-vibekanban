# Agent 即员工（Agents as Workforce）

> 状态：设计 + 实施中
> 关联：[`board-agents-plan.md`](board-agents-plan.md)（编排层分期）、[`crates/remote/AGENTS.md`](../crates/remote/AGENTS.md)

## 0. 一句话

把 Agent 从「一条配置记录」升级为**系统里的实体（员工）**：他有人设、有**自己的手（executor）**、在**属于自己的工作区**（目录 / 分支）里处理**某个上下文的任务**，并且**员工之间可以协作**（派活、审查）。

---

## 1. 概念模型

三个正交概念，不要混：

| 概念 | 是什么 | 载体 |
|------|--------|------|
| **任务看板** | 要做什么（目标 / 优先级 / 状态） | `issues` |
| **员工** | 谁来做（人设 / 能力 / 产能 / 工具） | `users` \| `agents` \| `squads` |
| **工作上下文** | 在哪做（目录 / 分支 / 会话记忆） | `workspaces` + `sessions` |

指派 = 把「看板上的一个目标」交给「一个员工」，系统为这个 (员工, 目标) 组合分配一个**工作上下文**。

### 1.1 员工的两只手

一个员工有两种工作方式，此前被建模成了两个不相关的字段：

- **说话的手** `chat_runtime` — 看板对话、澄清需求（Cursor SDK / OpenAI 兼容端点）
- **干活的手** `default_executor` — 在 workspace 里真正改代码（本机 coding agent CLI）

本设计把它们视作同一概念的两面：**这个员工用什么工具工作**。

---

## 2. 现状与缺口

### 2.1 已经存在（不需要重建）

- **多态 assignee**：`issue_assignees` 有 `user_id` / `agent_id` / `squad_id` 三个互斥字段。人 / Agent / 小组在看板上已是同级实体。
- **员工档案**：`agents` 表有 `name` / `instructions` / `max_concurrent_tasks` / `status` / `chat_runtime`。
- **工作上下文**：`SquadTargetType` 已区分 `Issue`（目标）/ `Path`（工作目录）/ `IssueAndPath`。
- **会话记忆**：`agent_tasks.resume_session_id` + `force_fresh_session`，按 `(agent, issue)` 续跑。
- **协作骨架**：`squads` + `squad_runs` + `squad_run_nodes`，含 `is_leader_task`、`execution_prompt`（handoff）、`on_assign`（`leader_only` / `full_pipeline`）、审批暂停/续跑。
- **事后审查**：MCP orchestrator 模式的 `wait_for_execution` / `get_execution_logs` / `get_execution.final_message`，以及
  `GET /api/execution-processes/{id}/normalized-logs`。

### 2.2 缺口

| # | 缺口 | 证据 | 后果 |
|---|------|------|------|
| **G1** | 员工没有「干活的手」 | `agents.default_executor` 是裸 `TEXT`；`agent_task_watcher.rs` 解析失败**静默** fallback 到 `ClaudeCode`；`ProjectAgentsPage.tsx` 创建时硬编码 `default_executor: null` | 本机装的 codex / droid / opencode / cursor 全部用不上，所有员工干活时都是同一个 Claude Code |
| **G2** | 不知道本机有哪些「手」可选 | `/api/config/agents/check-availability` 只能单个查询，无批量列举 | UI 无法呈现可选 executor，用户只能靠猜 |
| **G3** | 静默降级不可见 | 同 G1 | 用户以为配了 codex，实际跑的是 Claude Code，且无任何提示 |
| **G4** | 协作是运行时偶然，不是可配置职责 | MCP 审查能力仅存在于 orchestrator 会话；`agents` 表无 review 关系 | 「张三负责审查李四」只能写在 prompt 里，不可靠 |

**根因**：`chat_runtime` 是受约束的一等枚举（Postgres ENUM），`default_executor` 却是会静默降级的字符串。抽象没闭合。

---

## 3. 设计

### 3.1 员工的手：`default_executor` 升为受校验字段

**不改成 Postgres ENUM。** 理由：executor 列表由本机安装情况决定，是 local 侧的事实；remote（Postgres）不应该也无法知道某台机器装了什么。用 ENUM 会导致每次 VK 支持新 executor 都要迁移数据库。

改为：保持 `TEXT`，但在**三个边界**校验：

1. **写入时**（remote route）：校验值属于 `BaseCodingAgent` 的已知变体集合。拒绝未知值，而不是存进去等运行时炸。
2. **读取时**（UI）：从 local `/api/config/executors` 拉取本机实际可用列表并标注可用性。
3. **执行时**（watcher）：解析失败或未配置时 fallback，但**记录 warning 并写入 task 的 failure/日志**，不再静默。

新增一个 local 端点提供「本机有哪些手」：

```
GET /api/config/executors
→ { executors: [ { executor, available, availability, is_default } ] }
```

它基于已有的 `ExecutorConfigs::get_cached()` + `get_availability_info()`，把现有的单个查询变成批量列举。

### 3.2 静默降级可见化

`agent_task_watcher` 解析 executor 的逻辑集中成一个函数，返回「解析结果 + 是否降级 + 原因」，降级时：

- `tracing::warn!` 带上 agent name / 请求值 / 实际值
- 在发给 agent 的 prompt 里**不**提（那是噪音）
- 通过 task 的 `failure_reason` 之外的路径暴露：新增 `agent_tasks.executor_note`（可空 TEXT），记录「请求 X，实际用 Y，因为 Z」

这样看板上能看到「这个任务实际是谁干的」。

### 3.3 协作作为配置：`review` 关系

在 `agents` 上新增一个可空的自引用：

```sql
ALTER TABLE agents ADD COLUMN reviewer_agent_id UUID
    REFERENCES agents(id) ON DELETE SET NULL;
```

语义：当该 agent 的 task 成功完成后，自动为同一 issue 入队一个**审查 task**，指派给 `reviewer_agent_id`，并在其 `execution_prompt` 里带上被审对象的 `execution_id`，让审查者用 MCP 的 `get_execution_logs` 去读被审者的实际行为。

约束：
- `reviewer_agent_id != id`（不能自审）
- 审查 task 自身**不**再触发审查（避免无限链），用新的 trigger 值 `review` 标记。
- 审查者必须与被审者同 project。

这一步把 §2.2 的 G4 从「prompt 里的口头约定」变成「数据库里的组织关系」。

### 3.4 不做（明确排除）

- **不**统一 `agent_tasks` 队列与 MCP `run_session_prompt`。这两条路径合流会让 orchestrator 的每个探索性 session 都变成看板记录，很吵；且会动到 watcher 的 host claim 逻辑，风险与收益不匹配。留作后续，需要先引入 task 可见性维度。
- **不**把 `default_executor` 改成 Postgres ENUM（见 §3.1）。
- **不**做 `can_delegate_to` 通用委派矩阵。squad pipeline 已覆盖派活，review 是当前唯一明确缺失的关系。

---

## 4. 实施清单

| 项 | 位置 | 说明 |
|----|------|------|
| A1 | `crates/executors/src/executor_discovery.rs` | `ExecutorAvailability` 列举类型 |
| A2 | `crates/server/src/routes/config.rs` | `GET /api/config/executors` 批量列举 |
| B1 | `crates/api-types/src/agent.rs` | `default_executor` 校验 helper + `reviewer_agent_id` |
| B2 | `crates/remote/migrations/` | 新迁移：`reviewer_agent_id`、`executor_note`、`review` trigger |
| B3 | `crates/remote/src/routes/agents.rs` | 写入时校验 executor 值、校验 reviewer 约束 |
| B4 | `crates/remote/src/db/agents.rs` | 读写新字段 |
| C1 | `crates/local-deployment/src/agent_task_watcher.rs` | executor 解析集中化 + 降级可见 + 完成后入队 review task |
| D1 | `packages/web-core/src/pages/agents/ProjectAgentsPage.tsx` | executor 下拉（含可用性）+ reviewer 选择 |
| D2 | `packages/web-core/src/shared/lib/boardAgentsApi.ts` | 新字段透传 |
| E1 | 各处 | 单元测试：校验、降级、review 链防环 |
