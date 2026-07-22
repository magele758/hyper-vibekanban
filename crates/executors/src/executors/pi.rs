use std::{collections::HashMap, path::Path, process::Stdio, sync::Arc, time::Duration};

use async_trait::async_trait;
use futures::StreamExt;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use strum_macros::{AsRefStr, EnumString};
use tokio::process::Command;
use ts_rs::TS;
use workspace_utils::{
    command_ext::GroupSpawnNoWindowExt, diff::create_unified_diff, msg_store::MsgStore,
    path::make_path_relative, shell::resolve_executable_path_blocking,
};

use crate::{
    command::{CmdOverrides, CommandBuildError, CommandBuilder, CommandParts, apply_overrides},
    env::ExecutionEnv,
    executor_discovery::ExecutorDiscoveredOptions,
    executors::{
        AppendPrompt, AvailabilityInfo, BaseCodingAgent, ExecutorError, SpawnedChild,
        StandardCodingAgentExecutor,
    },
    logs::{
        ActionType, CommandExitStatus, CommandRunResult, FileChange, NormalizedEntry,
        NormalizedEntryError, NormalizedEntryType, TokenUsageInfo, ToolStatus,
        plain_text_processor::PlainTextLogProcessor,
        utils::{
            ConversationPatch, EntryIndexProvider, patch, shell_command_parsing::CommandCategory,
        },
    },
    model_selector::{ModelSelectorConfig, PermissionPolicy},
    profile::ExecutorConfig,
};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS, JsonSchema, AsRefStr, EnumString)]
#[serde(rename_all = "lowercase")]
#[strum(serialize_all = "lowercase")]
pub enum PiThinking {
    Off,
    Minimal,
    Low,
    Medium,
    High,
    XHigh,
    Max,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, TS, JsonSchema)]
pub struct Pi {
    #[serde(default)]
    pub append_prompt: AppendPrompt,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(
        title = "Model",
        description = "Model pattern or provider/id (e.g. anthropic/claude-sonnet-4, openai/gpt-4o)"
    )]
    pub model: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(
        title = "Provider",
        description = "Provider name (e.g. anthropic, openai)"
    )]
    pub provider: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(
        title = "Thinking",
        description = "Thinking level: off, minimal, low, medium, high, xhigh, max"
    )]
    pub thinking: Option<PiThinking>,
    /// Trust project-local files (skills/extensions). Default true for headless runs.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub approve: Option<bool>,
    #[serde(flatten)]
    pub cmd: CmdOverrides,
}

impl Pi {
    pub fn base_command() -> &'static str {
        "pi"
    }

    fn build_command_builder(&self) -> Result<CommandBuilder, CommandBuildError> {
        let mut builder =
            CommandBuilder::new(Self::base_command()).params(["-p", "--mode", "json"]);

        if self.approve.unwrap_or(true) {
            builder = builder.extend_params(["--approve"]);
        } else {
            builder = builder.extend_params(["--no-approve"]);
        }

        if let Some(provider) = &self.provider {
            builder = builder.extend_params(["--provider", provider]);
        }
        if let Some(model) = &self.model {
            builder = builder.extend_params(["--model", model]);
        }
        if let Some(thinking) = &self.thinking {
            builder = builder.extend_params(["--thinking", thinking.as_ref()]);
        }

        apply_overrides(builder, &self.cmd)
    }

    fn session_dir_args(current_dir: &Path) -> Vec<String> {
        let session_dir = current_dir.join(".pi").join("sessions");
        vec![
            "--session-dir".to_string(),
            session_dir.to_string_lossy().to_string(),
        ]
    }

    async fn spawn_with_parts(
        &self,
        current_dir: &Path,
        prompt: &str,
        command_parts: CommandParts,
        env: &ExecutionEnv,
    ) -> Result<SpawnedChild, ExecutorError> {
        let (executable_path, mut args) = command_parts.into_resolved().await?;
        let combined_prompt = self.append_prompt.combine_prompt(prompt);
        args.push(combined_prompt);

        let mut command = Command::new(executable_path);
        command
            .kill_on_drop(true)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .current_dir(current_dir)
            .env("NPM_CONFIG_LOGLEVEL", "error")
            .args(&args);

        env.clone()
            .with_profile(&self.cmd)
            .apply_to_command(&mut command);

        let child = command.group_spawn_no_window()?;
        Ok(child.into())
    }
}

#[async_trait]
impl StandardCodingAgentExecutor for Pi {
    fn apply_overrides(&mut self, executor_config: &ExecutorConfig) {
        if let Some(model_id) = &executor_config.model_id {
            self.model = Some(model_id.clone());
        }
        if let Some(reasoning_id) = &executor_config.reasoning_id {
            self.thinking = reasoning_id.parse().ok();
        }
        if let Some(permission_policy) = executor_config.permission_policy.clone() {
            // Headless Pi has no interactive approval UI; Auto trusts project files.
            self.approve = Some(matches!(permission_policy, PermissionPolicy::Auto));
        }
    }

    async fn spawn(
        &self,
        current_dir: &Path,
        prompt: &str,
        env: &ExecutionEnv,
    ) -> Result<SpawnedChild, ExecutorError> {
        let mut extra = Self::session_dir_args(current_dir);
        // Let Pi create a stable project session id we can resume later.
        extra.extend(["--session-id".to_string(), uuid::Uuid::new_v4().to_string()]);
        let command_parts = self.build_command_builder()?.build_follow_up(&extra)?;
        self.spawn_with_parts(current_dir, prompt, command_parts, env)
            .await
    }

    async fn spawn_follow_up(
        &self,
        current_dir: &Path,
        prompt: &str,
        session_id: &str,
        reset_to_message_id: Option<&str>,
        env: &ExecutionEnv,
    ) -> Result<SpawnedChild, ExecutorError> {
        let mut extra = Self::session_dir_args(current_dir);
        if reset_to_message_id.is_some() {
            // Pi supports session fork; message-level truncate is not exposed via CLI.
            extra.extend(["--fork".to_string(), session_id.to_string()]);
        } else {
            extra.extend(["--session".to_string(), session_id.to_string()]);
        }
        let command_parts = self.build_command_builder()?.build_follow_up(&extra)?;
        self.spawn_with_parts(current_dir, prompt, command_parts, env)
            .await
    }

    fn normalize_logs(
        &self,
        msg_store: Arc<MsgStore>,
        worktree_path: &Path,
    ) -> Vec<tokio::task::JoinHandle<()>> {
        let entry_index_provider = EntryIndexProvider::start_from(&msg_store);

        let msg_store_stderr = msg_store.clone();
        let entry_index_provider_stderr = entry_index_provider.clone();
        let h1 = tokio::spawn(async move {
            let mut stderr = msg_store_stderr.stderr_chunked_stream();
            let mut processor = PlainTextLogProcessor::builder()
                .normalized_entry_producer(Box::new(|content: String| {
                    let content = strip_ansi_escapes::strip_str(&content);
                    NormalizedEntry {
                        timestamp: None,
                        entry_type: NormalizedEntryType::ErrorMessage {
                            error_type: NormalizedEntryError::Other,
                        },
                        content,
                        metadata: None,
                    }
                }))
                .time_gap(Duration::from_secs(2))
                .index_provider(entry_index_provider_stderr)
                .build();

            while let Some(Ok(chunk)) = stderr.next().await {
                for patch in processor.process(chunk) {
                    msg_store_stderr.push_patch(patch);
                }
            }
        });

        let current_dir = worktree_path.to_path_buf();
        let h2 = tokio::spawn(async move {
            normalize_pi_stdout(msg_store, current_dir, entry_index_provider).await;
        });

        vec![h1, h2]
    }

    fn default_mcp_config_path(&self) -> Option<std::path::PathBuf> {
        // Pi does not use a Cursor-style mcp.json today; settings live under ~/.pi/agent.
        dirs::home_dir().map(|home| home.join(".pi").join("agent").join("settings.json"))
    }

    fn get_availability_info(&self) -> AvailabilityInfo {
        if resolve_executable_path_blocking(Self::base_command()).is_some() {
            AvailabilityInfo::InstallationFound
        } else {
            AvailabilityInfo::NotFound
        }
    }

    fn get_preset_options(&self) -> ExecutorConfig {
        ExecutorConfig {
            executor: BaseCodingAgent::Pi,
            variant: None,
            model_id: self.model.clone(),
            agent_id: None,
            reasoning_id: self.thinking.as_ref().map(|t| t.as_ref().to_string()),
            permission_policy: Some(PermissionPolicy::Auto),
        }
    }

    async fn discover_options(
        &self,
        _workdir: Option<&Path>,
        _repo_path: Option<&Path>,
    ) -> Result<futures::stream::BoxStream<'static, json_patch::Patch>, ExecutorError> {
        use crate::{
            executor_discovery::ExecutorConfigCacheKey, executors::utils::executor_options_cache,
            model_discovery::pi_providers_from_models,
        };

        let cache = executor_options_cache();
        let cmd_key = serde_json::to_string(&self.cmd).unwrap_or_default();
        let cache_key = ExecutorConfigCacheKey::new(None, cmd_key, BaseCodingAgent::Pi);

        if let Some(cached) = cache.get(&cache_key) {
            return Ok(Box::pin(futures::stream::once(async move {
                patch::executor_discovered_options(cached.as_ref().clone().with_loading(false))
            })));
        }

        let initial_options = ExecutorDiscoveredOptions {
            model_selector: ModelSelectorConfig {
                models: Vec::new(),
                permissions: vec![PermissionPolicy::Auto, PermissionPolicy::Supervised],
                ..Default::default()
            },
            loading_models: true,
            ..Default::default()
        };
        let initial_patch = patch::executor_discovered_options(initial_options);

        let this = self.clone();
        let discovery_stream = async_stream::stream! {
            let models = match crate::model_discovery::discover_pi_models(
                Self::base_command(),
                &this.cmd,
            )
            .await
            {
                Ok(models) => models,
                Err(error) => {
                    tracing::warn!(
                        ?error,
                        "Pi model discovery failed; leaving model list empty"
                    );
                    yield patch::models_loaded();
                    return;
                }
            };

            let providers = pi_providers_from_models(&models);
            yield patch::update_models(models.clone());
            yield patch::models_loaded();

            let options = ExecutorDiscoveredOptions {
                model_selector: ModelSelectorConfig {
                    providers,
                    models,
                    permissions: vec![PermissionPolicy::Auto, PermissionPolicy::Supervised],
                    ..Default::default()
                },
                loading_models: false,
                ..Default::default()
            };
            cache.put(cache_key, options);
        };

        Ok(Box::pin(
            futures::stream::once(async move { initial_patch }).chain(discovery_stream),
        ))
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum PiEvent {
    Session {
        id: String,
    },
    MessageStart {
        message: PiMessage,
    },
    MessageUpdate {
        message: PiMessage,
        #[serde(default)]
        assistant_message_event: Option<PiAssistantEvent>,
    },
    MessageEnd {
        message: PiMessage,
    },
    ToolExecutionStart {
        #[serde(rename = "toolCallId")]
        tool_call_id: String,
        #[serde(rename = "toolName")]
        tool_name: String,
        #[serde(default)]
        args: Value,
    },
    ToolExecutionEnd {
        #[serde(rename = "toolCallId")]
        tool_call_id: String,
        #[serde(rename = "toolName")]
        tool_name: String,
        #[serde(default)]
        result: Value,
        #[serde(default, rename = "isError")]
        is_error: bool,
    },
    AgentEnd {
        #[serde(default)]
        #[allow(dead_code)]
        messages: Vec<PiMessage>,
    },
    #[serde(other)]
    Other,
}

#[derive(Debug, Clone, Deserialize)]
struct PiMessage {
    #[serde(default)]
    role: Option<String>,
    #[serde(default)]
    content: Vec<PiContent>,
    #[serde(default)]
    model: Option<String>,
    #[serde(default)]
    provider: Option<String>,
    #[serde(default)]
    usage: Option<PiUsage>,
    #[serde(default, rename = "toolCallId")]
    tool_call_id: Option<String>,
    #[serde(default, rename = "toolName")]
    tool_name: Option<String>,
    #[serde(default, rename = "isError")]
    is_error: Option<bool>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
enum PiContent {
    Text {
        #[serde(default)]
        text: String,
    },
    Thinking {
        #[serde(default)]
        thinking: String,
    },
    #[allow(dead_code)]
    ToolCall {
        id: String,
        name: String,
        #[serde(default)]
        arguments: Value,
    },
    #[serde(other)]
    Other,
}

#[derive(Debug, Clone, Deserialize)]
struct PiAssistantEvent {
    #[serde(rename = "type")]
    event_type: String,
    #[serde(default)]
    delta: Option<String>,
    #[serde(default)]
    content: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
struct PiUsage {
    #[serde(default, rename = "totalTokens")]
    total_tokens: Option<u64>,
}

async fn normalize_pi_stdout(
    msg_store: Arc<MsgStore>,
    current_dir: std::path::PathBuf,
    entry_index_provider: EntryIndexProvider,
) {
    let worktree_str = current_dir.to_string_lossy().to_string();
    let mut lines = msg_store.stdout_lines_stream();

    let mut session_id_reported = false;
    let mut model_reported = false;
    let mut assistant_buffer = String::new();
    let mut assistant_index: Option<usize> = None;
    let mut thinking_buffer = String::new();
    let mut thinking_index: Option<usize> = None;
    let mut call_index_map: HashMap<String, usize> = HashMap::new();
    let mut call_args_map: HashMap<String, (String, Value)> = HashMap::new();
    let mut last_usage: Option<u32> = None;

    while let Some(Ok(line)) = lines.next().await {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        let event: PiEvent = match serde_json::from_str(trimmed) {
            Ok(v) => v,
            Err(_) => {
                let entry = NormalizedEntry {
                    timestamp: None,
                    entry_type: NormalizedEntryType::SystemMessage,
                    content: strip_ansi_escapes::strip_str(trimmed),
                    metadata: None,
                };
                let id = entry_index_provider.next();
                msg_store.push_patch(ConversationPatch::add_normalized_entry(id, entry));
                continue;
            }
        };

        match event {
            PiEvent::Session { id } => {
                if !session_id_reported {
                    msg_store.push_session_id(id);
                    session_id_reported = true;
                }
            }
            PiEvent::MessageUpdate {
                message,
                assistant_message_event,
            } => {
                report_model_once(
                    &msg_store,
                    &entry_index_provider,
                    &message,
                    &mut model_reported,
                );

                if let Some(ev) = assistant_message_event.as_ref() {
                    match ev.event_type.as_str() {
                        "text_delta" | "text_start" => {
                            if let Some(delta) = ev.delta.as_deref().or(ev.content.as_deref()) {
                                stream_text(
                                    &msg_store,
                                    &entry_index_provider,
                                    &mut assistant_buffer,
                                    &mut assistant_index,
                                    delta,
                                    NormalizedEntryType::AssistantMessage,
                                );
                            }
                        }
                        "thinking_delta" | "thinking_start" => {
                            if let Some(delta) = ev.delta.as_deref().or(ev.content.as_deref()) {
                                stream_text(
                                    &msg_store,
                                    &entry_index_provider,
                                    &mut thinking_buffer,
                                    &mut thinking_index,
                                    delta,
                                    NormalizedEntryType::Thinking,
                                );
                            }
                        }
                        "text_end" | "thinking_end" => {
                            // keep buffers until role switches / message_end
                        }
                        _ => {}
                    }
                }
            }
            PiEvent::MessageEnd { message } => {
                report_model_once(
                    &msg_store,
                    &entry_index_provider,
                    &message,
                    &mut model_reported,
                );
                if let Some(usage) = message.usage.as_ref().and_then(|u| u.total_tokens) {
                    last_usage = Some(usage.min(u64::from(u32::MAX)) as u32);
                }

                let role = message.role.as_deref().unwrap_or("");
                match role {
                    "assistant" => {
                        // Prefer streamed content; if none streamed, dump final text/thinking.
                        if assistant_index.is_none() {
                            let text = extract_text(&message.content);
                            if !text.is_empty() {
                                let entry = NormalizedEntry {
                                    timestamp: None,
                                    entry_type: NormalizedEntryType::AssistantMessage,
                                    content: text,
                                    metadata: None,
                                };
                                let id = entry_index_provider.next();
                                msg_store
                                    .push_patch(ConversationPatch::add_normalized_entry(id, entry));
                            }
                        }
                        if thinking_index.is_none() {
                            let thinking = extract_thinking(&message.content);
                            if !thinking.is_empty() {
                                let entry = NormalizedEntry {
                                    timestamp: None,
                                    entry_type: NormalizedEntryType::Thinking,
                                    content: thinking,
                                    metadata: None,
                                };
                                let id = entry_index_provider.next();
                                msg_store
                                    .push_patch(ConversationPatch::add_normalized_entry(id, entry));
                            }
                        }
                        assistant_buffer.clear();
                        assistant_index = None;
                        thinking_buffer.clear();
                        thinking_index = None;
                    }
                    "toolResult" => {
                        if let Some(tool_call_id) = message.tool_call_id.as_ref()
                            && let Some(&idx) = call_index_map.get(tool_call_id)
                        {
                            let tool_name = message
                                .tool_name
                                .clone()
                                .unwrap_or_else(|| "tool".to_string());
                            let output = extract_text(&message.content);
                            let failed = message.is_error.unwrap_or(false);
                            let entry = NormalizedEntry {
                                timestamp: None,
                                entry_type: NormalizedEntryType::ToolUse {
                                    tool_name: tool_name.clone(),
                                    action_type: ActionType::Tool {
                                        tool_name,
                                        arguments: None,
                                        result: Some(crate::logs::ToolResult {
                                            r#type: crate::logs::ToolResultValueType::Markdown,
                                            value: Value::String(output),
                                        }),
                                    },
                                    status: if failed {
                                        ToolStatus::Failed
                                    } else {
                                        ToolStatus::Success
                                    },
                                },
                                content: String::new(),
                                metadata: None,
                            };
                            msg_store.push_patch(ConversationPatch::replace(idx, entry));
                        }
                    }
                    _ => {}
                }
            }
            PiEvent::ToolExecutionStart {
                tool_call_id,
                tool_name,
                args,
            } => {
                assistant_buffer.clear();
                assistant_index = None;
                thinking_buffer.clear();
                thinking_index = None;

                let (action_type, content) =
                    tool_to_action_and_content(&tool_name, &args, &worktree_str);
                let entry = NormalizedEntry {
                    timestamp: None,
                    entry_type: NormalizedEntryType::ToolUse {
                        tool_name: tool_name.clone(),
                        action_type,
                        status: ToolStatus::Created,
                    },
                    content,
                    metadata: None,
                };
                let id = entry_index_provider.next();
                call_index_map.insert(tool_call_id.clone(), id);
                call_args_map.insert(tool_call_id, (tool_name, args));
                msg_store.push_patch(ConversationPatch::add_normalized_entry(id, entry));
            }
            PiEvent::ToolExecutionEnd {
                tool_call_id,
                tool_name,
                result,
                is_error,
            } => {
                if let Some(&idx) = call_index_map.get(&tool_call_id) {
                    let args = call_args_map
                        .get(&tool_call_id)
                        .map(|(_, a)| a.clone())
                        .unwrap_or(Value::Null);
                    let (mut action_type, content) =
                        tool_to_action_and_content(&tool_name, &args, &worktree_str);
                    if tool_name == "bash" || tool_name == "shell" {
                        let output = result_to_text(&result);
                        let command = args
                            .get("command")
                            .and_then(|v| v.as_str())
                            .unwrap_or(content.as_str())
                            .to_string();
                        action_type = ActionType::CommandRun {
                            command: command.clone(),
                            result: Some(CommandRunResult {
                                exit_status: Some(CommandExitStatus::ExitCode {
                                    code: if is_error { 1 } else { 0 },
                                }),
                                output: if output.is_empty() {
                                    None
                                } else {
                                    Some(output)
                                },
                            }),
                            category: CommandCategory::from_command(&command),
                        };
                    } else if matches!(action_type, ActionType::Tool { .. }) {
                        let output = result_to_text(&result);
                        action_type = ActionType::Tool {
                            tool_name: tool_name.clone(),
                            arguments: Some(args),
                            result: Some(crate::logs::ToolResult {
                                r#type: crate::logs::ToolResultValueType::Markdown,
                                value: Value::String(output),
                            }),
                        };
                    }

                    let entry = NormalizedEntry {
                        timestamp: None,
                        entry_type: NormalizedEntryType::ToolUse {
                            tool_name,
                            action_type,
                            status: if is_error {
                                ToolStatus::Failed
                            } else {
                                ToolStatus::Success
                            },
                        },
                        content,
                        metadata: None,
                    };
                    msg_store.push_patch(ConversationPatch::replace(idx, entry));
                }
            }
            PiEvent::AgentEnd { .. } => {
                if let Some(total_tokens) = last_usage {
                    let entry = NormalizedEntry {
                        timestamp: None,
                        entry_type: NormalizedEntryType::TokenUsageInfo(TokenUsageInfo {
                            total_tokens,
                            // Pi models vary widely; UI mainly needs a non-zero denominator.
                            model_context_window: total_tokens.max(1),
                        }),
                        content: String::new(),
                        metadata: None,
                    };
                    let id = entry_index_provider.next();
                    msg_store.push_patch(ConversationPatch::add_normalized_entry(id, entry));
                }
            }
            PiEvent::MessageStart { message } => {
                report_model_once(
                    &msg_store,
                    &entry_index_provider,
                    &message,
                    &mut model_reported,
                );
            }
            PiEvent::Other => {}
        }
    }
}

fn report_model_once(
    msg_store: &MsgStore,
    entry_index_provider: &EntryIndexProvider,
    message: &PiMessage,
    model_reported: &mut bool,
) {
    if *model_reported {
        return;
    }
    let Some(model) = message.model.as_ref() else {
        return;
    };
    let label = match message.provider.as_ref() {
        Some(provider) => format!("model: {provider}/{model}"),
        None => format!("model: {model}"),
    };
    let entry = NormalizedEntry {
        timestamp: None,
        entry_type: NormalizedEntryType::SystemMessage,
        content: label,
        metadata: None,
    };
    let id = entry_index_provider.next();
    msg_store.push_patch(ConversationPatch::add_normalized_entry(id, entry));
    *model_reported = true;
}

fn stream_text(
    msg_store: &MsgStore,
    entry_index_provider: &EntryIndexProvider,
    buffer: &mut String,
    index: &mut Option<usize>,
    delta: &str,
    entry_type: NormalizedEntryType,
) {
    if delta.is_empty() {
        return;
    }
    buffer.push_str(delta);
    let entry = NormalizedEntry {
        timestamp: None,
        entry_type,
        content: buffer.clone(),
        metadata: None,
    };
    if let Some(id) = *index {
        msg_store.push_patch(ConversationPatch::replace(id, entry));
    } else {
        let id = entry_index_provider.next();
        *index = Some(id);
        msg_store.push_patch(ConversationPatch::add_normalized_entry(id, entry));
    }
}

fn extract_text(content: &[PiContent]) -> String {
    content
        .iter()
        .filter_map(|c| match c {
            PiContent::Text { text } => Some(text.as_str()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("")
}

fn extract_thinking(content: &[PiContent]) -> String {
    content
        .iter()
        .filter_map(|c| match c {
            PiContent::Thinking { thinking } => Some(thinking.as_str()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("")
}

fn result_to_text(result: &Value) -> String {
    if let Some(s) = result.as_str() {
        return s.to_string();
    }
    if let Some(arr) = result.as_array() {
        return arr
            .iter()
            .filter_map(|v| {
                v.get("text")
                    .and_then(|t| t.as_str())
                    .map(|s| s.to_string())
                    .or_else(|| v.as_str().map(|s| s.to_string()))
            })
            .collect::<Vec<_>>()
            .join("");
    }
    if let Some(obj) = result.as_object() {
        if let Some(content) = obj.get("content") {
            return result_to_text(content);
        }
        if let Some(text) = obj.get("text").and_then(|t| t.as_str()) {
            return text.to_string();
        }
    }
    if result.is_null() {
        String::new()
    } else {
        result.to_string()
    }
}

fn tool_to_action_and_content(
    tool_name: &str,
    args: &Value,
    worktree_str: &str,
) -> (ActionType, String) {
    let name = tool_name.to_ascii_lowercase();
    match name.as_str() {
        "bash" | "shell" => {
            let command = args
                .get("command")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            (
                ActionType::CommandRun {
                    command: command.clone(),
                    result: None,
                    category: CommandCategory::from_command(&command),
                },
                command,
            )
        }
        "read" | "read_file" => {
            let path = args
                .get("path")
                .or_else(|| args.get("file_path"))
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let rel = make_path_relative(path, worktree_str);
            (ActionType::FileRead { path: rel.clone() }, rel)
        }
        "write" | "write_file" => {
            let path = args
                .get("path")
                .or_else(|| args.get("file_path"))
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let content = args
                .get("content")
                .or_else(|| args.get("contents"))
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let rel = make_path_relative(path, worktree_str);
            (
                ActionType::FileEdit {
                    path: rel.clone(),
                    changes: vec![FileChange::Write {
                        content: content.to_string(),
                    }],
                },
                rel,
            )
        }
        "edit" | "edit_file" | "str_replace" => {
            let path = args
                .get("path")
                .or_else(|| args.get("file_path"))
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let old = args
                .get("oldText")
                .or_else(|| args.get("old_string"))
                .or_else(|| args.get("old"))
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let new = args
                .get("newText")
                .or_else(|| args.get("new_string"))
                .or_else(|| args.get("new"))
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let rel = make_path_relative(path, worktree_str);
            let diff = create_unified_diff(path, old, new);
            (
                ActionType::FileEdit {
                    path: rel.clone(),
                    changes: vec![FileChange::Edit {
                        unified_diff: diff,
                        has_line_numbers: false,
                    }],
                },
                rel,
            )
        }
        "grep" | "find" | "search" => {
            let query = args
                .get("pattern")
                .or_else(|| args.get("query"))
                .or_else(|| args.get("glob"))
                .and_then(|v| v.as_str())
                .unwrap_or(tool_name)
                .to_string();
            (
                ActionType::Search {
                    query: query.clone(),
                },
                query,
            )
        }
        "ls" | "glob" => {
            let query = args
                .get("path")
                .or_else(|| args.get("pattern"))
                .and_then(|v| v.as_str())
                .unwrap_or(tool_name)
                .to_string();
            (
                ActionType::Search {
                    query: query.clone(),
                },
                query,
            )
        }
        _ => (
            ActionType::Tool {
                tool_name: tool_name.to_string(),
                arguments: Some(args.clone()),
                result: None,
            },
            tool_name.to_string(),
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_session_event() {
        let raw = r#"{"type":"session","version":3,"id":"abc-123","timestamp":"2026-01-01T00:00:00Z","cwd":"/tmp"}"#;
        let event: PiEvent = serde_json::from_str(raw).unwrap();
        match event {
            PiEvent::Session { id } => assert_eq!(id, "abc-123"),
            _ => panic!("expected session"),
        }
    }

    #[test]
    fn parse_tool_execution_start() {
        let raw = r#"{"type":"tool_execution_start","toolCallId":"bash_0","toolName":"bash","args":{"command":"echo hi"}}"#;
        let event: PiEvent = serde_json::from_str(raw).unwrap();
        match event {
            PiEvent::ToolExecutionStart {
                tool_call_id,
                tool_name,
                args,
            } => {
                assert_eq!(tool_call_id, "bash_0");
                assert_eq!(tool_name, "bash");
                assert_eq!(args["command"], "echo hi");
            }
            _ => panic!("expected tool_execution_start"),
        }
    }

    #[test]
    fn tool_bash_maps_to_command_run() {
        let args = serde_json::json!({"command": "ls"});
        let (action, content) = tool_to_action_and_content("bash", &args, "/tmp");
        assert_eq!(content, "ls");
        assert!(matches!(action, ActionType::CommandRun { .. }));
    }
}
