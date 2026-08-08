# Squad SOP Cookbook

五个真实用法的落地配方。每个都给出「装什么模板 / 关键字段 / 为什么这样配」，
以及需要人工确认的地方。

> 前置：Squad 编辑器在 **项目 → Agents → Squads**。三个内置模板可一键安装，
> 装完再在画布里微调即可（重复安装只会更新同名 Squad，不会产生副本）。

---

## 1. 盯到 100% 完成（防 coding-agent 半途而废）

**装模板**：`盯到完成 (Relentless)`

Coding agent 经常改一半就宣布完工。这条流水线不信它的自述：

```
实现 → while(最多 5 轮) → 硬验证脚本 → 完成度审计 → 鞭策回修 ─┐
                ↑                                          │
                └──────────────────────────────────────────┘
         退出 → Ask Merge
```

关键字段：

| 字段 | 值 | 作用 |
|------|-----|------|
| While `condition` | `verified:验收通过` | **严格判据**：上一步必须 `Completed` **且**输出含「验收通过」才退出 |
| While `max_iterations` | `5` | 硬上限，绝不无限循环 |
| Script `command` | `pnpm run check` | 真实退出码做闸门，脚本失败 → 该步 Failed → 严格判据判定「未完成」 |
| Agent `handoff_diff` | ✅ | 审计员看真实 diff，不是总结文字 |

**为什么能防死循环**：三重保险 —— `max_iterations` 上限、`script` 节点有 30
分钟超时（`VK_PIPELINE_SCRIPT_TIMEOUT_SECS` 可调）、agent 等待有 45 分钟上限。

**为什么能防「假完成」**：`verified:` 前缀是本次新增的严格条件形式。普通条件
在「agent 说完成了」时会走软判定放行，`verified:` 不会 —— 它要求上一步真的
成功。把它指向 `script` 节点，完成度就由退出码而不是措辞决定。

---

## 2. A 写 → B review → A 改

**装模板**：`Feature Closeout`（已有）

```
Review → 完成度? ──需回修──→ 回修 ──→ (回到 Review)
             └──通过──→ 测试 → 跑 check → Rebase → Ask Merge
```

**必须打开的开关**：给 coder 节点勾上 **「把本步 diff 传给下一步」**
(`handoff_diff`)。

不勾的话，reviewer 的 prompt 里只有上一步的 `summary`（一段自我描述），
它没法判断代码对不对。勾上以后，运行时会在该步之后自动跑一次
`git diff`，把补丁塞进下一步的 prompt：

```
## Diff from previous step
Review the actual changes below rather than trusting the summary.
```diff
...真实补丁...
```
```

diff 超过 12000 字符会截断（保护 agent 上下文窗口）。收集失败不会中断流水线，
只是退回「仅传总结」。

---

## 3. 定时采集情报 → 飞书问我 → 同意才开干

**装模板**：`情报侦察 (Scout)` + 配一个 Autopilot cron

```
采集(script) → 价值分析(agent) → 等我拍板(wait_approval) → 实现 → 跑 check
```

步骤：

1. 装 `情报侦察 (Scout)` 模板
2. **改采集命令**：`采集信息` 节点的 `command` 默认是占位 echo，改成真实命令，例如
   `curl -s https://example.com/feed.xml`、`gh api ...`、或仓库里自建的抓取脚本
3. 在 **Autopilots** 里新建一个 autopilot，`squad` 选 Scout Digest，
   `cron_expression` 填调度时间（scheduler 每 60 秒扫一次到期任务）
4. 分析结果会停在 `wait_approval`，同时进 **Inbox**；Approve 才继续实现，
   Reject 直接结束

**安全默认**：采集和分析都不改代码（分析 agent 的 instructions 明确禁止），
只有你 Approve 之后才进入实现节点。

> 飞书通知：目前 `wait_approval` 走 Inbox。要同时收到飞书提醒，
> 在 **Feishu** 标签页绑定机器人；机器人的对话能力见下面第 5 条。

---

## 4. 跨项目 / 多仓库协同（PM 角色）

**用**：任意 Squad + 逐节点 `working_directory`

这是本次新增能力。以前一个 Squad 只能操作一个仓库；现在**每个节点都能指定
自己的仓库**：

1. 在画布里选中节点
2. 填 **「本步仓库 working_directory」**：绝对路径或 repo UUID
3. 留空的节点继续沿用 Squad 级设置

典型的对接类需求：

```
        ┌→ 改后端接口 (working_directory=/repos/api)      ─┐
fork ──→│                                                  ├→ join → 联调验证
        └→ 改前端调用 (working_directory=/repos/web)      ─┘
```

配 `fork` 让两个仓库并行开工，`join` 做汇合屏障（有 1 小时超时，
单个分支失败不会把其他分支永久卡住）。运行日志里会打印每步落在哪个仓库：

```
- ↳ `改后端接口` targets repo `/repos/api`
```

**注意**：跨项目的 Issue 目前仍归属发起的那个 project；多仓库共享同一个 Issue
作为协调点。真正的「跨 project meta-issue」还没做。

---

## 5. 飞书发消息 → 对话决定 → 指定项目开特性

**用**：Feishu 机器人绑定 + 聊天指令

以前飞书是一次性触发：每条消息都直接建 Issue 并派 agent。现在有了指令分流，
可以先聊、后开工：

| 你在飞书发 | 行为 |
|-----------|------|
| 普通消息（贴一条新闻） | **不建 feature**。Agent 评估它对项目有无价值并回复理由 |
| `/feature 加个导出按钮` | 在机器人绑定的默认项目开 Issue 并开工 |
| `/feature hyper-vibekanban: 加个导出按钮` | 在**指定项目**开 Issue（需与机器人同组织） |
| `/approve 就按方案二做` | 同意上一条提案，开始实现 |
| `/reject 不做` | 放弃，不建任务 |
| `/help` | 显示指令说明 |

设计细节：

- **项目名解析限定在机器人所属组织内**，聊天指令不能碰到无关项目
- 项目名只按 ASCII 识别，所以 `/feature 修复: 登录失败` 会被当成需求文本，
  而不是名叫「修复」的项目
- `/usr/bin/env 找不到` 这种以斜杠开头但不是指令的消息，仍按普通聊天处理

---

## 排错

| 现象 | 原因 / 处理 |
|------|------------|
| Run 一直「执行中」不动 | Remote 重启会把遗留的 running run 标记 failed（启动时自动清理）。若仍卡住，看 agent 是否在等 45 分钟超时 |
| While 一轮就退出了 | 条件用了普通形式，被软判定放行。改成 `verified:<关键词>` |
| Reviewer 说「看不到代码」 | 上游 agent 节点没勾 `handoff_diff` |
| script 节点永远不结束 | 已有 30 分钟超时；确认命令不是在等 stdin（stdin 已置 null） |
| 跨仓库节点跑错目录 | `working_directory` 需是执行机上的绝对路径，或该 repo 的 UUID |
