# Phase 4 验收检查清单

## 部署信息
- 分支：`vk/8cfb-manual` (ubuntu1)
- 前端地址：http://10.12.0.34:3001 (或你的配置域名)
- 部署时间：2025-01-21 16:47 UTC+8
- 提交：a676ae3a "feat: 全局指挥台功能（手动部署）"

## 服务状态
- ✅ local-web dev: 运行中 (port 3001)
- ✅ agent-sidecar: 运行中 (port 13110)
- ✅ remote server: 应该在运行 (port 13000)

## 验收步骤

### Phase 1: 全局 Agents 入口
1. [ ] 打开前端页面并登录
2. [ ] 检查左侧导航栏是否有 **Agents 图标**（机器人图标 RobotIcon）
3. [ ] 点击 Agents 图标
4. [ ] 确认跳转到 `/agents` 路由（地址栏）
5. [ ] 确认页面标题为"全局 Copilot 指挥台"或类似文本

### Phase 2: Sidecar 工具扩展（间接验证）
工具已加载到 sidecar，将在 Phase 3 聊天测试中验证。

新增工具：
- `list_squads` - 列出 Squads
- `create_squad` - 创建 Squad
- `run_squad` - 运行 Squad
- `approve_squad_run` - 批准 Squad Run
- `list_autopilots` - 列出 Autopilots
- `create_autopilot` - 创建 Autopilot
- `trigger_autopilot` - 触发 Autopilot

### Phase 3: 全局指挥台聊天 UI
1. [ ] 页面顶部有 **Project 选择器**（下拉框，显示当前选中的项目）
2. [ ] 左侧有**会话列表**侧边栏，顶部有"新会话"按钮
3. [ ] 中间是**聊天区域**，底部有输入框
4. [ ] 输入框 placeholder 显示"描述需求或编排任务…"
5. [ ] 发送测试消息："列出所有 squads"
6. [ ] 确认消息发送成功，Copilot 有回复
7. [ ] 回复中应该包含工具调用结果（如果项目有 squad，会列出；如果没有，会提示"Found 0 squads"）
8. [ ] 测试切换项目：点击顶部 Project 选择器，切换到另一个项目
9. [ ] 确认会话列表清空，可以开始新对话

### Phase 3 高级测试（可选）
如果当前项目有 Agent/Squad/Autopilot，可以测试更复杂的编排：

**测试 1: 创建 Squad**
- 输入："帮我创建一个名为'Test Squad'的 squad"
- 确认工具调用 `create_squad` 成功
- 返回结果应包含新 squad 的 ID

**测试 2: 运行 Squad**
- 输入："运行刚才创建的 squad"（或指定 squad ID）
- 确认工具调用 `run_squad` 成功
- 返回结果应包含 run_id

**测试 3: 批准 Squad Run**
- 输入："批准 run_id <上面的 run_id>"
- 确认工具调用 `approve_squad_run` 成功

**测试 4: Autopilot 操作**
- 输入："创建一个 autopilot，每小时运行一次"
- 确认工具调用 `create_autopilot` 成功

## 已知限制
- 当前版本限定**单 project 范围**（通过顶部选择器切换）
- 不支持跨 project 的全局操作
- SOP 模板功能未实现（预留 Phase 3+）

## 回退方案
如果功能异常，回退到 main 分支：
```bash
cd /home/shinemo/penglei/hyper-vibekanban
git checkout main
# 重启服务（同上述启动步骤）
```

## 验收结论
- [ ] Phase 1 通过：全局 Agents 入口可访问
- [ ] Phase 2 通过：Sidecar 工具可调用
- [ ] Phase 3 通过：聊天 UI 正常工作
- [ ] 整体验收通过

验收人：_________  
验收时间：_________
