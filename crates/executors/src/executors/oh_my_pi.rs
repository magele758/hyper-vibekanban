//! Oh My Pi (`omp`) executor.
//!
//! Oh My Pi is a fork of pi (`@oh-my-pi/pi-coding-agent`). It emits the *same*
//! `--mode json` event stream as pi, so log normalization is shared via
//! [`crate::executors::pi::normalize_pi_stdout`]. The CLI surface differs:
//!
//! | concern      | pi                      | omp                                |
//! |--------------|-------------------------|------------------------------------|
//! | approvals    | `--approve/--no-approve`| `--auto-approve` / `--approval-mode`|
//! | resume       | `--session <id>`        | `-r <id-prefix>`                   |
//! | fork         | `--fork <id>`           | not supported (resume instead)     |
//! | model list   | `--list-models`         | `omp models --json`                |
//! | config dir   | `~/.pi/agent`           | `~/.omp/agent`                     |

use std::{path::Path, process::Stdio, sync::Arc, time::Duration};

use async_trait::async_trait;
use futures::StreamExt;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use tokio::process::Command;
use ts_rs::TS;
use workspace_utils::{
    command_ext::GroupSpawnNoWindowExt, msg_store::MsgStore,
    shell::resolve_executable_path_blocking,
};

use crate::{
    command::{CmdOverrides, CommandBuildError, CommandBuilder, CommandParts, apply_overrides},
    env::ExecutionEnv,
    executor_discovery::ExecutorDiscoveredOptions,
    executors::{
        AppendPrompt, AvailabilityInfo, BaseCodingAgent, ExecutorError, SpawnedChild,
        StandardCodingAgentExecutor,
        pi::{PiThinking, normalize_pi_stdout},
    },
    logs::{
        NormalizedEntry, NormalizedEntryError, NormalizedEntryType,
        plain_text_processor::PlainTextLogProcessor,
        utils::{EntryIndexProvider, patch},
    },
    model_selector::{ModelSelectorConfig, PermissionPolicy},
    profile::ExecutorConfig,
};

/// `omp --approval-mode` values.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum OhMyPiApprovalMode {
    /// Auto-approve every tool call (`--auto-approve`).
    Yolo,
    /// Auto-approve writes, ask for the rest.
    Write,
}

impl OhMyPiApprovalMode {
    fn as_str(self) -> &'static str {
        match self {
            Self::Yolo => "yolo",
            Self::Write => "write",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, TS, JsonSchema)]
pub struct OhMyPi {
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
        description = "Provider name (e.g. anthropic, openai). Legacy; prefer model."
    )]
    pub provider: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(
        title = "Thinking",
        description = "Thinking level: off, minimal, low, medium, high, xhigh, max"
    )]
    pub thinking: Option<PiThinking>,
    /// Auto-approve all tool calls. Default true for headless runs, which have
    /// no interactive approval UI.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub auto_approve: Option<bool>,
    /// Smol/fast model for lightweight subtasks (omp-specific).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(
        title = "Smol model",
        description = "Fast/cheap model for lightweight tasks"
    )]
    pub smol: Option<String>,
    /// Slow/reasoning model for thorough analysis (omp-specific).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(
        title = "Slow model",
        description = "Reasoning model for thorough analysis"
    )]
    pub slow: Option<String>,
    /// Disable skills discovery (`--no-skills`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub no_skills: Option<bool>,
    /// Disable LSP tools, formatting and diagnostics (`--no-lsp`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub no_lsp: Option<bool>,
    #[serde(flatten)]
    pub cmd: CmdOverrides,
}

impl OhMyPi {
    pub fn base_command() -> &'static str {
        "omp"
    }

    fn approval_mode(&self) -> OhMyPiApprovalMode {
        if self.auto_approve.unwrap_or(true) {
            OhMyPiApprovalMode::Yolo
        } else {
            OhMyPiApprovalMode::Write
        }
    }

    fn build_command_builder(&self) -> Result<CommandBuilder, CommandBuildError> {
        let mut builder =
            CommandBuilder::new(Self::base_command()).params(["-p", "--mode", "json"]);

        // omp has no `--approve`; it uses --auto-approve / --approval-mode.
        match self.approval_mode() {
            OhMyPiApprovalMode::Yolo => {
                builder = builder.extend_params(["--auto-approve"]);
            }
            mode => {
                builder = builder.extend_params(["--approval-mode", mode.as_str()]);
            }
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
        if let Some(smol) = &self.smol {
            builder = builder.extend_params(["--smol", smol]);
        }
        if let Some(slow) = &self.slow {
            builder = builder.extend_params(["--slow", slow]);
        }
        if self.no_skills.unwrap_or(false) {
            builder = builder.extend_params(["--no-skills"]);
        }
        if self.no_lsp.unwrap_or(false) {
            builder = builder.extend_params(["--no-lsp"]);
        }

        apply_overrides(builder, &self.cmd)
    }

    /// Keep sessions inside the worktree so each workspace is self-contained.
    fn session_dir_args(current_dir: &Path) -> Vec<String> {
        let session_dir = current_dir.join(".omp").join("sessions");
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
impl StandardCodingAgentExecutor for OhMyPi {
    fn apply_overrides(&mut self, executor_config: &ExecutorConfig) {
        if let Some(model_id) = &executor_config.model_id {
            self.model = Some(model_id.clone());
        }
        if let Some(reasoning_id) = &executor_config.reasoning_id {
            self.thinking = reasoning_id.parse().ok();
        }
        if let Some(permission_policy) = executor_config.permission_policy.clone() {
            self.auto_approve = Some(matches!(permission_policy, PermissionPolicy::Auto));
        }
    }

    async fn spawn(
        &self,
        current_dir: &Path,
        prompt: &str,
        env: &ExecutionEnv,
    ) -> Result<SpawnedChild, ExecutorError> {
        // Let omp allocate the session id; it is captured from the stdout
        // `session` event by normalize_logs and reused for `-r` follow-ups.
        let extra = Self::session_dir_args(current_dir);
        let command_parts = self.build_command_builder()?.build_follow_up(&extra)?;
        self.spawn_with_parts(current_dir, prompt, command_parts, env)
            .await
    }

    async fn spawn_follow_up(
        &self,
        current_dir: &Path,
        prompt: &str,
        session_id: &str,
        _reset_to_message_id: Option<&str>,
        env: &ExecutionEnv,
    ) -> Result<SpawnedChild, ExecutorError> {
        let mut extra = Self::session_dir_args(current_dir);
        // omp exposes no `--fork`; `-r` resumes by id prefix, path, or picker.
        // Message-level truncation is not available on the CLI, so a reset
        // request degrades to a plain resume.
        extra.extend(["-r".to_string(), session_id.to_string()]);
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
            // omp shares pi's JSON event schema.
            normalize_pi_stdout(msg_store, current_dir, entry_index_provider).await;
        });

        vec![h1, h2]
    }

    fn default_mcp_config_path(&self) -> Option<std::path::PathBuf> {
        // omp's canonical user-level MCP file is `<agentDir>/mcp.json` with a
        // `mcpServers` key (see @oh-my-pi/pi-utils getMCPConfigPath("user")).
        //
        // `agentDir` is resolved at runtime from the active --profile, XDG dirs
        // and PI_CODING_AGENT_DIR, so it cannot be reconstructed reliably here.
        // Ask omp itself, and only fall back to the default layout.
        Some(oh_my_pi_agent_dir_blocking(&self.cmd)?.join("mcp.json"))
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
            executor: BaseCodingAgent::OhMyPi,
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
        let cache_key = ExecutorConfigCacheKey::new(None, cmd_key, BaseCodingAgent::OhMyPi);

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
            let models = match crate::model_discovery::discover_oh_my_pi_models(
                Self::base_command(),
                &this.cmd,
            )
            .await
            {
                Ok(models) => models,
                Err(error) => {
                    tracing::warn!(
                        ?error,
                        "Oh My Pi model discovery failed; leaving model list empty"
                    );
                    yield patch::models_loaded();
                    return;
                }
            };

            let providers = pi_providers_from_models(&models);
            yield patch::update_models(models.clone());
            yield patch::models_loaded();

            // Surface omp's own `modelRoles.default` so the selector shows the
            // model the user already configured. We never pass `--model` unless
            // it is explicitly set, so this is display-only: omp keeps resolving
            // the model from its config.
            let default_model = crate::model_discovery::oh_my_pi_default_model(
                Self::base_command(),
                &this.cmd,
            )
            .await
            .filter(|configured| {
                // default is a selector (`provider/model`); ModelInfo.id is bare.
                let known = models.iter().any(|m| model_matches_selector(m, configured));
                if !known {
                    tracing::debug!(
                        configured,
                        "omp default model is not in the discovered list; leaving selector unset"
                    );
                }
                known
            });
            if default_model.is_some() {
                yield patch::update_default_model(default_model.clone());
            }

            let options = ExecutorDiscoveredOptions {
                model_selector: ModelSelectorConfig {
                    providers,
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

fn model_matches_selector(model: &crate::model_selector::ModelInfo, selector: &str) -> bool {
    if model.id == selector {
        return true;
    }
    match model.provider_id.as_deref() {
        Some(provider) => {
            selector == format!("{provider}/{}", model.id)
                || selector.eq_ignore_ascii_case(&format!("{provider}/{}", model.id))
        }
        None => false,
    }
}

/// Default agent dir when omp cannot be asked (`~/.omp/agent`).
fn default_oh_my_pi_agent_dir() -> Option<std::path::PathBuf> {
    dirs::home_dir().map(|home| home.join(".omp").join("agent"))
}

/// Resolve omp's active agent (config) directory, cached.
///
/// Shells out to `omp config path`; cached because it is hit on UI paths.
fn oh_my_pi_agent_dir_blocking(cmd: &CmdOverrides) -> Option<std::path::PathBuf> {
    use std::sync::OnceLock;
    static CACHE: OnceLock<Option<std::path::PathBuf>> = OnceLock::new();

    let probe = || {
        workspace_utils::tokio::block_on(crate::model_discovery::oh_my_pi_agent_dir(
            OhMyPi::base_command(),
            cmd,
        ))
        .or_else(default_oh_my_pi_agent_dir)
    };

    // Overrides can change the resolved dir, so only cache the plain case.
    if cmd.base_command_override.is_some() || cmd.additional_params.is_some() {
        return probe();
    }
    CACHE.get_or_init(probe).clone()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn default_omp() -> OhMyPi {
        serde_json::from_value(serde_json::json!({})).unwrap()
    }

    fn params_of(agent: &OhMyPi) -> Vec<String> {
        agent
            .build_command_builder()
            .unwrap()
            .params
            .unwrap_or_default()
    }

    #[test]
    fn base_command_is_omp() {
        assert_eq!(OhMyPi::base_command(), "omp");
    }

    #[test]
    fn defaults_to_json_print_mode_and_auto_approve() {
        let params = params_of(&default_omp());
        assert_eq!(params[0], "-p");
        assert!(params.windows(2).any(|w| w == ["--mode", "json"]));
        // omp has no `--approve`; that flag is pi-only.
        assert!(params.iter().any(|p| p == "--auto-approve"));
        assert!(!params.iter().any(|p| p == "--approve"));
    }

    #[test]
    fn supervised_uses_approval_mode_write() {
        let mut agent = default_omp();
        agent.auto_approve = Some(false);
        let params = params_of(&agent);
        assert!(params.windows(2).any(|w| w == ["--approval-mode", "write"]));
        assert!(!params.iter().any(|p| p == "--auto-approve"));
    }

    #[test]
    fn passes_model_thinking_and_role_models() {
        let mut agent = default_omp();
        agent.model = Some("anthropic/claude-sonnet-4".to_string());
        agent.thinking = Some(PiThinking::High);
        agent.smol = Some("haiku".to_string());
        agent.slow = Some("opus".to_string());
        let params = params_of(&agent);
        assert!(
            params
                .windows(2)
                .any(|w| w == ["--model", "anthropic/claude-sonnet-4"])
        );
        assert!(params.windows(2).any(|w| w == ["--thinking", "high"]));
        assert!(params.windows(2).any(|w| w == ["--smol", "haiku"]));
        assert!(params.windows(2).any(|w| w == ["--slow", "opus"]));
    }

    #[test]
    fn optional_toggles_are_off_by_default() {
        let params = params_of(&default_omp());
        assert!(!params.iter().any(|p| p == "--no-skills"));
        assert!(!params.iter().any(|p| p == "--no-lsp"));
    }

    #[test]
    fn session_dir_is_worktree_local_under_dot_omp() {
        let args = OhMyPi::session_dir_args(Path::new("/tmp/wt"));
        assert_eq!(args[0], "--session-dir");
        assert!(args[1].ends_with("/.omp/sessions"));
    }

    #[test]
    fn apply_overrides_maps_permission_policy() {
        let mut agent = default_omp();
        agent.apply_overrides(&ExecutorConfig {
            executor: BaseCodingAgent::OhMyPi,
            variant: None,
            model_id: Some("openai/gpt-4o".to_string()),
            agent_id: None,
            reasoning_id: Some("max".to_string()),
            permission_policy: Some(PermissionPolicy::Supervised),
        });
        assert_eq!(agent.model.as_deref(), Some("openai/gpt-4o"));
        assert_eq!(agent.thinking, Some(PiThinking::Max));
        assert_eq!(agent.auto_approve, Some(false));
    }
}

#[cfg(test)]
mod agent_dir_tests {
    use super::*;

    /// The MCP path must land on `<agentDir>/mcp.json`, which is what omp reads
    /// (pi-utils getMCPConfigPath("user")). Only runs when omp is installed.
    #[test]
    fn mcp_config_path_is_agent_dir_mcp_json() {
        let agent: OhMyPi = serde_json::from_value(serde_json::json!({})).unwrap();
        if resolve_executable_path_blocking(OhMyPi::base_command()).is_none() {
            return;
        }
        let path = agent.default_mcp_config_path().expect("mcp path");
        assert_eq!(path.file_name().unwrap(), "mcp.json");
        assert_eq!(path.parent().unwrap().file_name().unwrap(), "agent");
        eprintln!("resolved mcp path: {}", path.display());
    }
}
