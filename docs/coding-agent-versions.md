# Coding Agent 版本记录

本项目通过 `npx -y <pkg>@<tag>` 启动各 coding agent，命令定义在
`crates/executors/src/executors/<name>.rs` 的 `base_command()` / `build_command_builder()`。

## 当前策略：默认 `@latest`

自 2026-06-22 起，npx 类 agent 改为浮动 `@latest`，默认跟随上游最新版。
代价是失去可复现性（不同时间冷启动可能装到不同版本），且更频繁触发下载。

## ⚠️ 已验证稳定版（回滚基准）

如果某个 agent 升到 `@latest` 后日志解析异常 / 工具调用显示错乱 /
diff 丢失，把对应文件里的 `@latest` 改回下面这一版即可恢复（这些是
2026-06-22 之前长期使用、确认能被 `normalize_logs` 正确解析的版本）：

| Agent | 包名 | 已验证稳定版 | 源码位置 |
|---|---|---|---|
| Claude Code | `@anthropic-ai/claude-code` | `2.1.183` | `crates/executors/src/executors/claude.rs` |
| Claude Router (CCR) | `@musistudio/claude-code-router` | `1.0.66` | `crates/executors/src/executors/claude.rs` |
| Codex | `@openai/codex` | `0.124.0` | `crates/executors/src/executors/codex.rs` |
| Gemini CLI | `@google/gemini-cli` | `0.29.3` | `crates/executors/src/executors/gemini.rs` |
| GitHub Copilot | `@github/copilot` | `0.0.403` | `crates/executors/src/executors/copilot.rs` |
| Qwen Code | `@qwen-code/qwen-code` | `0.9.1` | `crates/executors/src/executors/qwen.rs` |
| opencode | `opencode-ai` | `1.4.7` | `crates/executors/src/executors/opencode.rs` |

## ⚠️⚠️ Codex 特例：CLI 与 Rust 协议 crate 必须同版

Codex 不同于其他 agent：它的输出不是靠宽松 JSON 解析，而是依赖 OpenAI
codex 仓库的**强类型 Rust crate**，在 `crates/executors/Cargo.toml` 里钉死：

```
codex-protocol            = { git = "...codex.git", tag = "rust-v0.124.0" }
codex-app-server-protocol = { git = "...codex.git", tag = "rust-v0.124.0" }
```

现在 npx CLI 是 `@latest`，而 Rust 解析器仍锁在 `0.124.0`。**两者一旦
协议字段不一致，Codex 任务会解析失败**——这是所有 agent 里 `@latest` 风险
最高的一个。

> **当前决定（2026-06-22）：Codex 也保持 `@latest`，CLI 与 Rust crate
> 故意不同步——这是有意为之，不是 bug，请勿"修复"成一致。** 若 Codex 任务
> 真的解析失败，按下面两个选项之一恢复：

1. **回滚钉版**：把 codex.rs 的 `@latest` 改回 `@0.124.0`，
   与 Cargo.toml 的 `rust-v0.124.0` 对齐。
2. 同步跟最新：同时把 codex.rs 的版本号、Cargo.toml 两个 `tag` 一起 bump 到同一版
   （如 0.141.0 / rust-v0.141.0），再重新编译验证。
切勿只动其中一边。

> 参考：切到 `@latest` 当天（2026-06-22）npm 上的最新版分别为
> claude-code 2.1.185 / ccr 2.0.0 / codex 0.141.0 / gemini 0.47.0 /
> copilot 1.0.63 / qwen 0.18.5 / opencode 1.17.9。其中 copilot 与 ccr
> 跨了大版本，是日志格式 breaking change 风险最高的两个，回滚时优先怀疑。

## 非 npx 的 agent（不受本策略影响）

| Agent | 启动方式 | 版本来源 |
|---|---|---|
| Cursor | 本地二进制 `cursor-agent` | 用户自行安装，需 `cursor-agent login` |
| Droid | 本地二进制 `droid exec` | 用户自行安装 |
| Amp | `npx -y @sourcegraph/amp@latest` | 一直是浮动 latest（日历版本号） |

## 单独 bump 某个 agent 到固定版本

若想把某个 agent 钉回固定版本（恢复可复现性），把对应 `base_command()`
里的 `@latest` 改成 `@<version>`，然后 `vk-stop && vk-start` 起一个该
agent 的真实任务，确认日志面板正常即可。
