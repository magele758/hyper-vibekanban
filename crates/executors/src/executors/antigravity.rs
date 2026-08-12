use std::{collections::HashMap, path::Path, process::Stdio, sync::Arc, time::Duration};

use async_trait::async_trait;
use derivative::Derivative;
use futures::StreamExt;
use regex::Regex;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio::process::Command;
use ts_rs::TS;
use workspace_utils::{command_ext::GroupSpawnNoWindowExt, msg_store::MsgStore};

use crate::{
    command::{CmdOverrides, CommandBuildError, CommandBuilder, CommandParts, apply_overrides},
    env::ExecutionEnv,
    executor_discovery::{ExecutorConfigCacheKey, ExecutorDiscoveredOptions},
    executors::{
        AppendPrompt, AvailabilityInfo, BaseCodingAgent, ExecutorError, SpawnedChild,
        StandardCodingAgentExecutor, utils::executor_options_cache,
    },
    logs::{
        ActionType, CommandExitStatus, CommandRunResult, NormalizedEntry, NormalizedEntryError,
        NormalizedEntryType, ToolResult, ToolStatus,
        plain_text_processor::PlainTextLogProcessor,
        utils::{
            EntryIndexProvider,
            patch::{self, ConversationPatch},
            shell_command_parsing::CommandCategory,
        },
    },
    model_discovery::{ANTIGRAVITY_DEFAULT_MODEL, antigravity_default_models},
    model_selector::{ModelSelectorConfig, PermissionPolicy},
    profile::ExecutorConfig,
};

/// `agy` does not ship ACP stdio yet (upstream issue #31). VK drives it via
/// headless `--print` + `--output-format=stream-json` instead of `AcpAgentHarness`.
const SUPPRESSED_STDERR_PATTERNS: &[&str] = &[
    "logging before google.Init",
    "YOLO mode is enabled",
    "Failed to redirect output for CLI",
    "Singleflight refresh failed",
    "admin controls not applicable",
    "Entering local chrome mode",
    "failed to get cs path",
    "Continuous pprof profiling is disabled",
    "Auth mode is unspecified",
    "Skipping telemetry propagation",
    "Last check was less than",
    "Language server",
    "Migrat",
    "Cache(",
    "http_helpers.go",
    "model_config_manager.go",
    "quota_manager.go",
    "experiment_manager.go",
    "cli_setting_manager.go",
    "conversation_manager.go",
    "server.go",
    "common.go",
    "auth.go",
    "server_oauth.go",
    "keyring.go",
    "auto_updater.go",
    "hooks_manager.go",
    "manager.go",
    "defaults.go",
    "profiler.go",
    "printmode.go",
    "errorreport.go",
    "launchmanager.go",
    "auth_provider.go",
];

#[derive(Derivative, Clone, Serialize, Deserialize, TS, JsonSchema)]
#[derivative(Debug, PartialEq)]
pub struct Antigravity {
    #[serde(default)]
    pub append_prompt: AppendPrompt,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dangerously_skip_permissions: Option<bool>,
    #[serde(flatten)]
    pub cmd: CmdOverrides,
}

#[derive(Debug, Deserialize)]
struct StreamEvent {
    event: String,
    #[serde(default)]
    conversation_id: Option<String>,
    #[serde(default)]
    init: Option<InitPayload>,
    #[serde(default)]
    step_update: Option<StepUpdate>,
    #[serde(default)]
    result: Option<ResultPayload>,
}

#[derive(Debug, Deserialize)]
struct InitPayload {
    #[serde(default)]
    conversation_id: Option<String>,
}

#[derive(Debug, Deserialize)]
struct StepUpdate {
    #[serde(default)]
    conversation_id: Option<String>,
    #[serde(default)]
    step_index: Option<u64>,
    #[serde(default)]
    state: Option<String>,
    #[serde(default)]
    step_type: Option<String>,
    #[serde(default)]
    text_delta: Option<String>,
    #[serde(default)]
    tool_name: Option<String>,
    #[serde(default)]
    tool_info: Option<ToolInfo>,
}

#[derive(Debug, Deserialize)]
struct ToolInfo {
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    parameters: Option<Value>,
    #[serde(default)]
    output: Option<String>,
    #[serde(default)]
    error: Option<ToolErrorInfo>,
}

#[derive(Debug, Deserialize)]
struct ToolErrorInfo {
    #[serde(default)]
    message: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ResultPayload {
    #[serde(default)]
    conversation_id: Option<String>,
    #[serde(default)]
    status: Option<String>,
    #[serde(default)]
    response: Option<String>,
    #[serde(default)]
    error: Option<String>,
}

impl Antigravity {
    pub fn base_command() -> &'static str {
        "agy"
    }

    fn build_command_builder(&self, prompt: &str) -> Result<CommandBuilder, CommandBuildError> {
        // Headless print mode — `agy` has no `--acp` yet.
        //
        // IMPORTANT: `--print` / `-p` is a string flag. The prompt MUST be the
        // immediate value (`--print=<prompt>` or `-p <prompt>`). Putting the
        // prompt at the end makes later flags get eaten as the prompt.
        //
        // Prefer stream-json: plain `text` buffers the whole run, so explore
        // prompts look hung for minutes with an empty transcript.
        let mut builder = CommandBuilder::new(Self::base_command()).params([
            format!("--print={prompt}"),
            "--output-format=stream-json".to_string(),
            // Prevent skill/slash expansion from triggering tool calls that
            // headless mode soft-denies even with --dangerously-skip-permissions.
            "--disable-slash-commands".to_string(),
        ]);

        if let Some(model) = &self.model {
            builder = builder.extend_params(["--model", model.as_str()]);
        }

        if self.dangerously_skip_permissions.unwrap_or(false) {
            builder = builder.extend_params(["--dangerously-skip-permissions"]);
        }

        apply_overrides(builder, &self.cmd)
    }

    async fn spawn_with_parts(
        &self,
        current_dir: &Path,
        command_parts: CommandParts,
        env: &ExecutionEnv,
    ) -> Result<SpawnedChild, ExecutorError> {
        let (executable_path, args) = command_parts.into_resolved().await?;

        let mut command = Command::new(executable_path);
        command
            .kill_on_drop(true)
            // Critical: do not leave stdin open — interactive TUI will hang forever.
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

fn push_session_id(msg_store: &MsgStore, session_pushed: &mut bool, id: Option<&str>) {
    if *session_pushed {
        return;
    }
    if let Some(id) = id.filter(|s| !s.is_empty()) {
        msg_store.push_session_id(id.to_string());
        *session_pushed = true;
    }
}

fn stream_assistant_text(
    msg_store: &MsgStore,
    entry_index_provider: &EntryIndexProvider,
    buffer: &mut String,
    index: &mut Option<usize>,
    delta: &str,
) {
    if delta.is_empty() {
        return;
    }
    buffer.push_str(delta);
    let entry = NormalizedEntry {
        timestamp: None,
        entry_type: NormalizedEntryType::AssistantMessage,
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

fn tool_action_and_content(tool_name: &str, parameters: Option<&Value>) -> (ActionType, String) {
    let params = parameters.cloned().unwrap_or(Value::Null);
    match tool_name {
        "run_command" | "bash" | "shell" => {
            let command = params
                .get("CommandLine")
                .or_else(|| params.get("command"))
                .and_then(|v| v.as_str())
                .unwrap_or(tool_name)
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
        "list_dir" | "read_file" | "view_file" | "cat" => {
            let path = params
                .get("DirectoryPath")
                .or_else(|| params.get("AbsolutePath"))
                .or_else(|| params.get("path"))
                .or_else(|| params.get("file_path"))
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            (ActionType::FileRead { path: path.clone() }, path)
        }
        "write_file" | "replace_file_content" | "edit_file" => {
            let path = params
                .get("AbsolutePath")
                .or_else(|| params.get("path"))
                .or_else(|| params.get("file_path"))
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            (
                ActionType::FileEdit {
                    path: path.clone(),
                    changes: vec![],
                },
                path,
            )
        }
        "search" | "grep" | "find" => {
            let query = params
                .get("Query")
                .or_else(|| params.get("query"))
                .or_else(|| params.get("pattern"))
                .and_then(|v| v.as_str())
                .unwrap_or("")
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
                arguments: if params.is_null() { None } else { Some(params) },
                result: None,
            },
            String::new(),
        ),
    }
}

fn finalize_tool_action(
    tool_name: &str,
    parameters: Option<&Value>,
    output: Option<&str>,
    failed: bool,
) -> (ActionType, String) {
    let (action_type, content) = tool_action_and_content(tool_name, parameters);
    match action_type {
        ActionType::CommandRun {
            command, category, ..
        } => (
            ActionType::CommandRun {
                command: command.clone(),
                result: Some(CommandRunResult {
                    exit_status: Some(CommandExitStatus::ExitCode {
                        code: if failed { 1 } else { 0 },
                    }),
                    output: output.filter(|s| !s.is_empty()).map(|s| s.to_string()),
                }),
                category,
            },
            command,
        ),
        ActionType::Tool {
            tool_name: name,
            arguments,
            ..
        } => (
            ActionType::Tool {
                tool_name: name,
                arguments,
                result: output.filter(|s| !s.is_empty()).map(ToolResult::markdown),
            },
            content,
        ),
        other => (other, content),
    }
}

async fn normalize_stream_json(msg_store: Arc<MsgStore>, entry_index_provider: EntryIndexProvider) {
    let mut lines = msg_store.stdout_lines_stream();
    let mut session_pushed = false;
    let mut assistant_buffer = String::new();
    let mut assistant_index: Option<usize> = None;
    // Survives agent_response DONE so `result.response` can replace the last
    // streamed bubble. `agy` stream-json text_delta frequently splits multibyte
    // UTF-8 and embeds U+FFFD (e.g. "🛠️" → "🛠���", "新增" → "���增").
    let mut last_assistant_index: Option<usize> = None;
    let mut tool_index_map: HashMap<u64, usize> = HashMap::new();

    while let Some(Ok(line)) = lines.next().await {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        let event: StreamEvent = match serde_json::from_str(trimmed) {
            Ok(v) => v,
            Err(_) => {
                // Non-JSON noise on stdout — surface as system text.
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

        match event.event.as_str() {
            "init" => {
                let id = event.conversation_id.as_deref().or(event
                    .init
                    .as_ref()
                    .and_then(|i| i.conversation_id.as_deref()));
                push_session_id(&msg_store, &mut session_pushed, id);
            }
            "step_update" => {
                let Some(step) = event.step_update else {
                    continue;
                };
                push_session_id(
                    &msg_store,
                    &mut session_pushed,
                    step.conversation_id.as_deref(),
                );

                let step_type = step.step_type.as_deref().unwrap_or("");
                let state = step.state.as_deref().unwrap_or("");

                match step_type {
                    "agent_response" => {
                        if let Some(delta) = step.text_delta.as_deref() {
                            stream_assistant_text(
                                &msg_store,
                                &entry_index_provider,
                                &mut assistant_buffer,
                                &mut assistant_index,
                                delta,
                            );
                            if let Some(id) = assistant_index {
                                last_assistant_index = Some(id);
                            }
                        }
                        if state.eq_ignore_ascii_case("DONE") {
                            // Next tool/assistant turn starts a fresh bubble.
                            assistant_buffer.clear();
                            assistant_index = None;
                        }
                    }
                    "tool" => {
                        let tool_name = step
                            .tool_name
                            .clone()
                            .or_else(|| step.tool_info.as_ref().and_then(|t| t.name.clone()))
                            .unwrap_or_else(|| "tool".to_string());
                        let params = step.tool_info.as_ref().and_then(|t| t.parameters.as_ref());
                        let step_index = step.step_index.unwrap_or(0);

                        if state.eq_ignore_ascii_case("ACTIVE") {
                            assistant_buffer.clear();
                            assistant_index = None;

                            let (action_type, content) =
                                tool_action_and_content(&tool_name, params);
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
                            if let Some(&idx) = tool_index_map.get(&step_index) {
                                msg_store.push_patch(ConversationPatch::replace(idx, entry));
                            } else {
                                let id = entry_index_provider.next();
                                tool_index_map.insert(step_index, id);
                                msg_store
                                    .push_patch(ConversationPatch::add_normalized_entry(id, entry));
                            }
                        } else if state.eq_ignore_ascii_case("DONE")
                            || state.eq_ignore_ascii_case("ERROR")
                        {
                            let failed = state.eq_ignore_ascii_case("ERROR");
                            let output = step
                                .tool_info
                                .as_ref()
                                .and_then(|t| t.output.as_deref())
                                .or_else(|| {
                                    step.tool_info.as_ref().and_then(|t| {
                                        t.error.as_ref().and_then(|e| e.message.as_deref())
                                    })
                                });
                            let (action_type, content) =
                                finalize_tool_action(&tool_name, params, output, failed);
                            let entry = NormalizedEntry {
                                timestamp: None,
                                entry_type: NormalizedEntryType::ToolUse {
                                    tool_name,
                                    action_type,
                                    status: if failed {
                                        ToolStatus::Failed
                                    } else {
                                        ToolStatus::Success
                                    },
                                },
                                content,
                                metadata: None,
                            };
                            if let Some(&idx) = tool_index_map.get(&step_index) {
                                msg_store.push_patch(ConversationPatch::replace(idx, entry));
                            } else {
                                let id = entry_index_provider.next();
                                tool_index_map.insert(step_index, id);
                                msg_store
                                    .push_patch(ConversationPatch::add_normalized_entry(id, entry));
                            }
                        }
                    }
                    _ => {}
                }
            }
            "result" => {
                if let Some(result) = event.result {
                    push_session_id(
                        &msg_store,
                        &mut session_pushed,
                        result
                            .conversation_id
                            .as_deref()
                            .or(event.conversation_id.as_deref()),
                    );

                    let status = result.status.as_deref().unwrap_or("");
                    if status.eq_ignore_ascii_case("SUCCESS") {
                        // Always prefer the final response over streamed deltas.
                        // text_delta from `agy` is often corrupted at UTF-8
                        // boundaries; result.response is complete and clean.
                        if let Some(response) = result.response.as_deref().filter(|s| !s.is_empty())
                        {
                            let entry = NormalizedEntry {
                                timestamp: None,
                                entry_type: NormalizedEntryType::AssistantMessage,
                                content: response.to_string(),
                                metadata: None,
                            };
                            if let Some(id) = last_assistant_index.or(assistant_index) {
                                msg_store.push_patch(ConversationPatch::replace(id, entry));
                            } else {
                                let id = entry_index_provider.next();
                                msg_store
                                    .push_patch(ConversationPatch::add_normalized_entry(id, entry));
                            }
                            assistant_buffer.clear();
                            assistant_index = None;
                        }
                    } else {
                        let message = result.error.or(result.response).unwrap_or_else(|| {
                            format!("Antigravity finished with status {status}")
                        });
                        let entry = NormalizedEntry {
                            timestamp: None,
                            entry_type: NormalizedEntryType::ErrorMessage {
                                error_type: NormalizedEntryError::Other,
                            },
                            content: message,
                            metadata: None,
                        };
                        let id = entry_index_provider.next();
                        msg_store.push_patch(ConversationPatch::add_normalized_entry(id, entry));
                    }
                }
            }
            _ => {}
        }
    }
}

#[async_trait]
impl StandardCodingAgentExecutor for Antigravity {
    fn apply_overrides(&mut self, executor_config: &ExecutorConfig) {
        if let Some(model_id) = &executor_config.model_id {
            self.model = Some(model_id.clone());
        }
        if let Some(permission_policy) = executor_config.permission_policy.clone() {
            self.dangerously_skip_permissions = Some(matches!(
                permission_policy,
                crate::model_selector::PermissionPolicy::Auto
            ));
        }
    }

    async fn spawn(
        &self,
        current_dir: &Path,
        prompt: &str,
        env: &ExecutionEnv,
    ) -> Result<SpawnedChild, ExecutorError> {
        let combined_prompt = self.append_prompt.combine_prompt(prompt);
        let command_parts = self
            .build_command_builder(&combined_prompt)?
            .build_initial()?;
        self.spawn_with_parts(current_dir, command_parts, env).await
    }

    async fn spawn_follow_up(
        &self,
        current_dir: &Path,
        prompt: &str,
        session_id: &str,
        _reset_to_message_id: Option<&str>,
        env: &ExecutionEnv,
    ) -> Result<SpawnedChild, ExecutorError> {
        let combined_prompt = self.append_prompt.combine_prompt(prompt);
        let command_parts = self
            .build_command_builder(&combined_prompt)?
            .build_follow_up(&["--conversation".to_string(), session_id.to_string()])?;
        self.spawn_with_parts(current_dir, command_parts, env).await
    }

    fn normalize_logs(
        &self,
        msg_store: Arc<MsgStore>,
        _worktree_path: &Path,
    ) -> Vec<tokio::task::JoinHandle<()>> {
        let entry_index_provider = EntryIndexProvider::start_from(&msg_store);

        let msg_store_stdout = msg_store.clone();
        let entry_index_provider_stdout = entry_index_provider.clone();
        let h1 = tokio::spawn(async move {
            normalize_stream_json(msg_store_stdout, entry_index_provider_stdout).await;
        });

        let msg_store_stderr = msg_store.clone();
        let entry_index_provider_stderr = entry_index_provider;
        let h2 = tokio::spawn(async move {
            let conversation_re = Regex::new(
                r"(?:Created conversation |Print mode: conversation=|Streaming conversation )([0-9a-fA-F-]{36})",
            )
            .expect("conversation id regex");
            let mut session_pushed = false;
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
                let cleaned = strip_ansi_escapes::strip_str(&chunk);

                if let Some(id) = (!session_pushed)
                    .then(|| conversation_re.captures(&cleaned))
                    .flatten()
                    .and_then(|caps| caps.get(1))
                {
                    msg_store_stderr.push_session_id(id.as_str().to_string());
                    session_pushed = true;
                }

                if SUPPRESSED_STDERR_PATTERNS
                    .iter()
                    .any(|pattern| cleaned.contains(pattern))
                {
                    continue;
                }

                // agy dumps most diagnostics to stderr; keep only likely failures.
                let lower = cleaned.to_ascii_lowercase();
                if !(lower.contains("error")
                    || lower.contains("failed")
                    || lower.contains("not logged")
                    || lower.contains("permission")
                    || lower.contains("timeout"))
                {
                    continue;
                }

                for patch in processor.process(cleaned) {
                    msg_store_stderr.push_patch(patch);
                }
            }
        });

        vec![h1, h2]
    }

    fn default_mcp_config_path(&self) -> Option<std::path::PathBuf> {
        dirs::home_dir().map(|home| {
            home.join(".gemini")
                .join("antigravity-cli")
                .join("settings.json")
        })
    }

    fn get_availability_info(&self) -> AvailabilityInfo {
        if let Some(timestamp) = dirs::home_dir()
            .and_then(|home| {
                std::fs::metadata(
                    home.join(".gemini")
                        .join("antigravity-cli")
                        .join("oauth_creds.json"),
                )
                .ok()
            })
            .and_then(|m| m.modified().ok())
            .and_then(|modified| modified.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|d| d.as_secs() as i64)
        {
            return AvailabilityInfo::LoginDetected {
                last_auth_timestamp: timestamp,
            };
        }

        let mcp_config_found = self
            .default_mcp_config_path()
            .map(|p| p.exists())
            .unwrap_or(false);

        let installation_indicator_found = dirs::home_dir()
            .map(|home| {
                home.join(".gemini")
                    .join("antigravity-cli")
                    .join("installation_id")
                    .exists()
            })
            .unwrap_or(false);

        // `agy` itself on PATH is enough to surface the executor; auth may live
        // in the OS keyring rather than oauth_creds.json.
        let binary_found =
            workspace_utils::shell::resolve_executable_path_blocking(Self::base_command())
                .is_some();

        if mcp_config_found || installation_indicator_found || binary_found {
            AvailabilityInfo::InstallationFound
        } else {
            AvailabilityInfo::NotFound
        }
    }

    fn get_preset_options(&self) -> ExecutorConfig {
        ExecutorConfig {
            executor: BaseCodingAgent::Antigravity,
            variant: None,
            model_id: self.model.clone(),
            agent_id: None,
            reasoning_id: None,
            permission_policy: Some(if self.dangerously_skip_permissions.unwrap_or(false) {
                PermissionPolicy::Auto
            } else {
                PermissionPolicy::Supervised
            }),
        }
    }

    async fn discover_options(
        &self,
        _workdir: Option<&std::path::Path>,
        _repo_path: Option<&std::path::Path>,
    ) -> Result<futures::stream::BoxStream<'static, json_patch::Patch>, ExecutorError> {
        let cache = executor_options_cache();
        let cmd_key = serde_json::to_string(&self.cmd).unwrap_or_default();
        let cache_key = ExecutorConfigCacheKey::new(None, cmd_key, BaseCodingAgent::Antigravity);

        if let Some(cached) = cache.get(&cache_key) {
            return Ok(Box::pin(futures::stream::once(async move {
                patch::executor_discovered_options(cached.as_ref().clone().with_loading(false))
            })));
        }

        // Serve the built-in catalog immediately so the selector is usable while
        // `agy models` (often slow / pipe-sticky) refreshes in the background.
        let initial_options = ExecutorDiscoveredOptions {
            model_selector: ModelSelectorConfig {
                models: antigravity_default_models(),
                default_model: Some(ANTIGRAVITY_DEFAULT_MODEL.to_string()),
                permissions: vec![PermissionPolicy::Auto, PermissionPolicy::Supervised],
                ..Default::default()
            },
            loading_models: true,
            ..Default::default()
        };
        let initial_patch = patch::executor_discovered_options(initial_options);

        let this = self.clone();
        let discovery_stream = async_stream::stream! {
            let models = match crate::model_discovery::discover_antigravity_models(
                Self::base_command(),
                &this.cmd,
            )
            .await
            {
                Ok(models) if !models.is_empty() => models,
                Ok(_) => {
                    yield patch::models_loaded();
                    return;
                }
                Err(error) => {
                    tracing::warn!(
                        ?error,
                        "Antigravity model discovery failed; keeping fallback list"
                    );
                    yield patch::models_loaded();
                    return;
                }
            };

            let default_model = models
                .iter()
                .find(|m| m.id == ANTIGRAVITY_DEFAULT_MODEL)
                .map(|m| m.id.clone())
                .or_else(|| models.first().map(|m| m.id.clone()));

            yield patch::update_models(models.clone());
            yield patch::update_default_model(default_model.clone());
            yield patch::models_loaded();

            let options = ExecutorDiscoveredOptions {
                model_selector: ModelSelectorConfig {
                    models,
                    default_model,
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

#[cfg(test)]
mod tests {
    use workspace_utils::log_msg::LogMsg;

    use super::*;

    #[tokio::test]
    async fn result_response_replaces_garbled_text_deltas() {
        let msg_store = Arc::new(MsgStore::new());
        // Simulate agy splitting "新增" / "🛠️" across text_delta chunks with U+FFFD.
        let garbled = format!(
            "{hammer}{fffd}{fffd}{fffd} 核心",
            hammer = '\u{1F6E0}',
            fffd = '\u{FFFD}',
        );
        msg_store.push_stdout(format!(
            "{{\"event\":\"step_update\",\"step_update\":{{\"step_index\":1,\"state\":\"ACTIVE\",\"step_type\":\"agent_response\",\"text_delta\":{}}}}}\n",
            serde_json::to_string(&garbled).unwrap()
        ));
        msg_store.push_stdout(
            "{\"event\":\"step_update\",\"step_update\":{\"step_index\":1,\"state\":\"DONE\",\"step_type\":\"agent_response\",\"text_delta\":\"完成工作\"}}\n"
                .to_string(),
        );
        let clean = format!("{} 核心完成工作", "🛠️");
        msg_store.push_stdout(format!(
            "{{\"event\":\"result\",\"result\":{{\"status\":\"SUCCESS\",\"response\":{}}}}}\n",
            serde_json::to_string(&clean).unwrap()
        ));
        msg_store.push_finished();

        let entry_index_provider = EntryIndexProvider::start_from(&msg_store);
        normalize_stream_json(msg_store.clone(), entry_index_provider).await;

        let assistant_contents: Vec<String> = msg_store
            .get_history()
            .into_iter()
            .filter_map(|msg| {
                let LogMsg::JsonPatch(patch) = msg else {
                    return None;
                };
                let value = serde_json::to_value(&patch).ok()?;
                let ops = value.as_array()?;
                for op in ops {
                    let entry = op.get("value")?;
                    if entry.get("type")?.as_str()? != "NORMALIZED_ENTRY" {
                        continue;
                    }
                    let content = entry.get("content")?;
                    let entry_type = content.get("entry_type")?;
                    let type_name = entry_type
                        .get("type")
                        .and_then(|v| v.as_str())
                        .or_else(|| entry_type.as_str())?;
                    if type_name != "assistant_message"
                        && type_name != "ASSISTANT_MESSAGE"
                        && type_name != "AssistantMessage"
                    {
                        continue;
                    }
                    if let Some(text) = content.get("content").and_then(|v| v.as_str()) {
                        return Some(text.to_string());
                    }
                }
                None
            })
            .collect();

        assert!(
            !assistant_contents.is_empty(),
            "expected assistant patches, got history: {:?}",
            msg_store.get_history()
        );
        let final_content = assistant_contents.last().unwrap();
        assert_eq!(final_content, "🛠️ 核心完成工作");
        assert!(
            !final_content.contains('\u{FFFD}'),
            "final assistant text still has replacement chars: {final_content:?}"
        );
    }
}
