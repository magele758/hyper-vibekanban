# Agentic SDLC 增强：审计（C）→ 方案（A）→ 纵向切片（B）

> 状态：**C 完成 · A 定稿 · B-slice 已落地（待本机演示验收）**  
> 日期：2026-08-11  
> 关联：[`ai-workflow-spine-plan.md`](./ai-workflow-spine-plan.md)、[`squad-sop-cookbook.md`](./squad-sop-cookbook.md)、[`agents-as-workforce.md`](./agents-as-workforce.md)  
> 用户优先级（全部要）：  
> 1. Stage Protocol（`issue_workflow` + artifacts + 工作流条）  
> 2. Full Feature Flow 前半（想法→方案→架构门禁→接 Closeout）  
> 3. 契约与可观测（结构化验收、看板徽章、失败必进 Inbox）  
> 4. 员工/执行层（executor 可见、reviewer 链）  
> 5. 入口与运营（Autopilot / 飞书 / Scout 体验）

---

## C. Batch 1/2 现状审计

### C.0 对照表（验收项 → 代码现实）

| 验收项 | 状态 | 证据 | 缺口严重度 |
|--------|------|------|------------|
| **B1.1** 纯 Issue 不回归 / `on_assign=leader_only` 默认 | ✅ | migration + `issue_assignees` 分支 | — |
| **B1.2** `full_pipeline` 指派后台跑全 pipeline | ✅ | `issue_assignees.rs` + `spawn_squad_run` | — |
| **B1.3** `start_from_node_id` + UI 选步 | ⚠️ 半完成 | API + Squad 编辑器 Run 对话框有；**Issue 面板无「从…开始」** | 中 |
| **B1.4** `wait_approval` + Inbox / Issue Approve | ✅ | Inbox + Issue 流水线区 | — |
| **B1.5** Issue 上可见 run 进度 | ⚠️ 半完成 | 有 status/计时/审批；**无节点进度点**（`ordered_node_ids`/`squad_run_nodes` 未暴露给 UI） | 中 |
| **B2.1** `script` / `git_op` Local 执行 | ⚠️ 半完成 | watcher 支持 script/rebase/merge/push；**`create_pr` 明确未实现** | 中 |
| **B2.2** Feature Closeout 一键安装 | ⚠️ 可用但不稳 | 模板存在；见 C.1 可靠性问题 | **高** |
| **B2.3** Review while 回修环 | ⚠️ 弱 | Closeout 用 **if 回边** 无 `max_iterations` → **可无限环** | **高** |
| **B2.4** script 失败不进 rebase | ⚠️ 半完成 | 无 error 边时**整支停止**（对，但不回修）；Relentless 有 error→audit | 中 |
| **B2.5** rebase 后 Ask Merge | ✅ | 拓扑有 | — |
| **B3** `issue_workflow` / artifacts / Full Flow 前半 | ❌ 未做 | 无表、无 API、无 UI | **高（P1/P2）** |
| **看板流水线徽章** | ❌ 未做 | 仅 agent task 排队/派发/执行中；`waiting_approval` 无任务时卡片空白 | **高（P3）** |
| **失败必进 Inbox** | ❌ | 仅 `wait_approval` 写 Inbox；`failed` 只写 `error_message` | **高（P3）** |
| **Electric 同步 squad_runs** | ❌ | 表有 `REPLICA IDENTITY FULL`，**未** `electric_sync_table` | 中（挡徽章实时性） |
| **Workforce executor + reviewer 链** | ✅ 骨架 | migration + API + UI 已有 | 体验可再打磨（P4） |
| **Scout / Autopilot / 飞书指令** | ✅ 可用 | 模板 + cookbook；飞书 `/feature` 等 | 体验打磨（P5） |

### C.1 Feature Closeout 可靠性问题（演示会翻车）

当前拓扑（`workflow_templates.rs`）：

```
Review → if(agent:需回修) → Fix → Review（无上限）
                └→ Test → script(check) → rebase → Ask Merge（终态，无 merge 节点）
```

| 问题 | 影响 |
|------|------|
| Review/Fix **未开 `handoff_diff`** | Reviewer 只看 summary，审不出真 diff（cookbook 已警告，模板默认未开） |
| if 回边 **无 `max_iterations`** | Agent 一直写「需回修」→ 无限环 |
| script **无 error 边** | check 失败直接停支，不进 Fixer |
| Ask Merge **无后继 merge/create_pr** | Approve 只标 completed，不真正合入（产品可接受，但演示「合并完成」缺一环） |
| 条件 `agent:需回修` 偏软 | 不如 Relentless 的 `verified:` 严格 |

Relentless Delivery 反而是更好的「硬验收」范本：`while` + `verified:` + `handoff_diff` + script error 边 + max 5。

### C.2 能力地图（已有 vs 缺）

```text
                    ┌─────────────────────────────────────────┐
  看板 / Issue      │ 纯 Issue SOP ✅  | 流水线区 ⚠️ | Stage 条 ❌ │
                    │ 卡片 agent 徽章 ✅ | 流水线徽章 ❌          │
                    └─────────────────────────────────────────┘
                                       │
                    ┌─────────────────────────────────────────┐
  编排 Squad        │ DAG ✅ wait_approval ✅ on_assign ✅       │
                    │ start_from ✅ script/git_op ⚠️ create_pr ❌│
                    │ Closeout ⚠️ Relentless ✅ Scout ✅        │
                    │ Full Feature Flow ❌                     │
                    └─────────────────────────────────────────┘
                                       │
                    ┌─────────────────────────────────────────┐
  Stage Protocol    │ issue_workflow ❌ issue_artifacts ❌      │
                    │ 节点 stage 字段有，但不驱动状态机          │
                    └─────────────────────────────────────────┘
                                       │
                    ┌─────────────────────────────────────────┐
  执行 / 员工       │ coding executor 可选 ✅ reviewer 链 ✅    │
                    │ executor_note 降级可见 ✅                 │
                    └─────────────────────────────────────────┘
                                       │
                    ┌─────────────────────────────────────────┐
  入口 / 运营       │ Autopilot ✅ 飞书指令 ✅ Scout 模板 ✅    │
                    │ Issue 内一键切入 ⚠️                      │
                    └─────────────────────────────────────────┘
```

### C.3 非问题（不要重做）

- 再发明一套 coding agent runtime —— Workspace executor 已够。
- 把默认看板改成 9 列状态机 —— 方案明确禁止。
- 默认无 Ask Merge 自动 merge main —— 安全默认保留。

---

## A. 增强方案（覆盖优先级 1–5）

### A.1 产品原则（继承 spine-plan）

1. **纯 Issue 零噪音** —— 无 run / 无 workflow 时 UI 与今日一致。  
2. **一条大 Pipeline，任意切入** —— `start_from_node_id` + `entry_label`。  
3. **门禁在 Inbox + Issue**，失败也必须可发现。  
4. **Stage 靠 artifact/节点契约推进，不靠拖列。**

### A.2 分期（在 spine Batch 之上继续）

| 波次 | 主题 | 对应优先级 | 交付 |
|------|------|------------|------|
| **B-slice（本分支）** | 可演示闭环加固 | 2+3 为主，触 1/5 | Closeout 硬化 · Full Flow 模板 · 失败 Inbox · 看板流水线徽章 · Issue 快捷切入 · Electric squad_runs |
| **B3a** | Stage Protocol MVP | **1** | `issue_workflows` + `issue_artifacts` 表/shape/API；节点完成写 stage；Issue 工作流条 + Artifacts 折叠 |
| **B3b** | 契约验收 | **3** | artifact 合格标准；`verified:` 默认用于门禁；script 产物写 `test_report` |
| **B4** | Release 尾段 | 2 尾 | regress / release 门禁；`create_pr` git_op |
| **B5 polish** | 员工与运营 | **4+5** | Workforce 可用性提示；Scout 默认命令向导；飞书审批与 Inbox 对齐文案 |

### A.3 优先级 1 — Stage Protocol

**数据**

```text
issue_workflows
  issue_id PK, current_stage, stage_updated_at, config jsonb, updated_at

issue_artifacts
  id, issue_id, kind, stage, body_md, payload jsonb,
  produced_by, created_at
```

**写入点**

- 节点完成且 `node.stage` 有值 → upsert `current_stage`  
- Agent/script 结束解析 ` ```vk-artifact` JSON 或约定标记 → insert artifact  
- 门禁 Approve（scheme/design/merge）可写审计 artifact  

**UI**

- Issue 面板顶部：仅当有 workflow 或活跃 run 时显示 stage 条 + 节点点  
- Artifacts 折叠列表  

**粗列**：可选 best-effort 同步（失败不阻断流水线）。

### A.4 优先级 2 — Full Feature Flow

模板名：`Full Feature Flow`，`on_assign=full_pipeline`。

```text
Research(agent, stage=ideate, handoff)
  → wait_approval(scheme) 「确认方案」
  → Architect(agent, design+impact, handoff)
  → wait_approval(design) 「选型确认」  // 可后续 skip_if_label=small
  → Implement(agent, implement_full, handoff)
  → [内嵌 Closeout 子图：Review while / Test / script / rebase / Ask Merge → merge]
```

入口 `entry_label`：采集研究 / 确认方案 / 架构 / 开始实现 / 代码审查 / Ask Merge —— 对应「任意切入」。

### A.5 优先级 3 — 契约与可观测

| 能力 | 做法 |
|------|------|
| 结构化验收 | 推广 `verified:` + script 退出码；Closeout 对齐 Relentless |
| 看板徽章 | `squad_runs` Electric shape；卡片显示 流水线中 / 待你批准 |
| 失败 Inbox | `status=failed` 时 `InboxRepository::create(type=workflow_failed)` |
| 进度点 | 暴露 `ordered_node_ids` 或订阅 `squad_run_nodes` |

### A.6 优先级 4 — 员工层

已落地：`default_executor` 校验、本机 executors 列表、`reviewer_agent_id` 审后自动入队、`executor_note`。

后续 polish：任务卡展示 executor_note；Closeout 角色默认挂 reviewer 链（可选）。

### A.7 优先级 5 — 入口与运营

已落地：Autopilot+squad、Scout 模板、飞书 `/feature` `/approve`。

补齐：Issue 面板「快捷运行」；安装模板后引导改 Scout 采集命令；失败/审批飞书镜像（可选）。

### A.8 成功标准（产品）

与 spine §12 一致，并加：

- Closeout：**不可能**因 if 回边无限跑（硬上限）。  
- 失败/待批：**看板或 Inbox 必有可见信号**。  
- 纯 Issue：无任何新控件占位。

### A.9 非目标（再强调）

9 列状态机、默认自动 merge main、把 GitHub Actions 整套搬进 VK、对话 runtime 替代 coding executor。

---

## B. 本分支纵向切片（可演示）

**演示故事**

> 项目安装 **Full Feature Flow** 或硬化后的 **Feature Closeout** →  
> 在 Issue 上指派 / 快捷「从代码审查开始」→  
> 看板出现「流水线中」；Review 看真实 diff；脚本失败回修有上限 →  
> Ask Merge 进 Inbox + 卡片「待批准」→ Approve 后可 merge →  
> 任意失败写 Inbox，不再静默。

**代码清单**

| # | 项 | 状态 |
|---|----|------|
| 1 | Harden Closeout：`handoff_diff`、while max 5、script error→Fix、Approve 后 `git_op merge` | ✅ |
| 2 | 新模板 `feature-full-flow` | ✅ |
| 3 | 失败 → Inbox `workflow_failed` | ✅ |
| 4 | `electric_sync_table(squad_runs)` + shape + 看板徽章 | ✅ |
| 5 | Issue：快捷运行（entry_label）+ 更清晰的 run 状态 | ✅ |
| — | while **until-true** 语义 + Relentless 审计通过后不再强制进 push | ✅ |

**明确留给 B3a 的**

- `issue_workflows` / `issue_artifacts` 完整表与 artifact 契约解析  
- `create_pr` git_op  

### B 验收清单（手动）

1. `VK_REBUILD=1 vk-stop && vk-start` 后 migration `20260811000000` 成功  
2. Agents → 安装 **Full Feature Flow** / 重装 **Feature Closeout**  
3. Issue 面板出现「从步骤运行」；无 squad entry 的项目不显示  
4. 启动 run 后看板卡片显示「流水线中」；`wait_approval` 时「待批准」  
5. 人为让 script 失败 → Inbox 出现 `workflow_failed`  
6. Ask Merge Approve → 进入 merge 节点（有 workspace 时）  

---

## 附录：文件索引

| 区域 | 路径 |
|------|------|
| 原方案 | `docs/ai-workflow-spine-plan.md` |
| 模板 | `crates/remote/src/routes/workflow_templates.rs` |
| 编排 | `crates/remote/src/routes/squads.rs` |
| Local job | `crates/local-deployment/src/agent_task_watcher.rs` |
| Issue 流水线 UI | `packages/web-core/src/pages/kanban/IssueSquadRunSectionContainer.tsx` |
| Inbox | `packages/web-core/src/pages/agents/ProjectInboxPage.tsx` |
| 看板卡 | `packages/ui/src/components/KanbanCardContent.tsx` |
| Workforce | `docs/agents-as-workforce.md` |
