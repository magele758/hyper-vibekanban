use std::{path::Path, sync::Arc};

use async_trait::async_trait;
use derivative::Derivative;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use ts_rs::TS;
use workspace_utils::{msg_store::MsgStore, shell::resolve_executable_path_blocking};

pub use super::acp::AcpAgentHarness;
use crate::{
    approvals::ExecutorApprovalService,
    command::{CmdOverrides, CommandBuildError, CommandBuilder, apply_overrides},
    env::ExecutionEnv,
    executor_discovery::ExecutorDiscoveredOptions,
    executors::{
        AppendPrompt, AvailabilityInfo, BaseCodingAgent, ExecutorError, SpawnedChild,
        StandardCodingAgentExecutor,
    },
    logs::utils::patch,
    model_selector::{ModelInfo, ModelSelectorConfig, PermissionPolicy},
    profile::ExecutorConfig,
};

/// Only ever resolve the `grok` binary — never bare `agent`, which Grok's
/// installer also installs and which collides with Cursor's CLI name.
const EXECUTABLE_NAME: &str = "grok";

#[derive(Derivative, Clone, Serialize, Deserialize, TS, JsonSchema)]
#[derivative(Debug, PartialEq)]
pub struct Grok {
    #[serde(default)]
    pub append_prompt: AppendPrompt,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub always_approve: Option<bool>,
    #[serde(flatten)]
    pub cmd: CmdOverrides,
    #[serde(skip)]
    #[ts(skip)]
    #[derivative(Debug = "ignore", PartialEq = "ignore")]
    pub approvals: Option<Arc<dyn ExecutorApprovalService>>,
}

impl Grok {
    pub fn base_command() -> &'static str {
        EXECUTABLE_NAME
    }

    fn build_command_builder(&self) -> Result<CommandBuilder, CommandBuildError> {
        // Prefer PATH `grok` (typically ~/.local/bin/grok → ~/.grok/bin/grok).
        // Do not use the `agent` symlink — that name is shared with Cursor.
        let mut builder = CommandBuilder::new(Self::base_command()).params(["agent"]);

        if let Some(model) = &self.model {
            builder = builder.extend_params(["-m", model.as_str()]);
        }

        if self.always_approve.unwrap_or(false) {
            builder = builder.extend_params(["--always-approve"]);
        }

        builder = builder.extend_params(["stdio"]);

        apply_overrides(builder, &self.cmd)
    }
}

#[async_trait]
impl StandardCodingAgentExecutor for Grok {
    fn apply_overrides(&mut self, executor_config: &ExecutorConfig) {
        if let Some(model_id) = &executor_config.model_id {
            self.model = Some(model_id.clone());
        }
        if let Some(permission_policy) = executor_config.permission_policy.clone() {
            self.always_approve = Some(matches!(
                permission_policy,
                crate::model_selector::PermissionPolicy::Auto
            ));
        }
    }

    fn use_approvals(&mut self, approvals: Arc<dyn ExecutorApprovalService>) {
        self.approvals = Some(approvals);
    }

    async fn spawn(
        &self,
        current_dir: &Path,
        prompt: &str,
        env: &ExecutionEnv,
    ) -> Result<SpawnedChild, ExecutorError> {
        let mut harness = AcpAgentHarness::with_session_namespace("grok_sessions");
        if let Some(model) = &self.model {
            harness = harness.with_model(model);
        }
        let combined_prompt = self.append_prompt.combine_prompt(prompt);
        let grok_command = self.build_command_builder()?.build_initial()?;
        let approvals = if self.always_approve.unwrap_or(false) {
            None
        } else {
            self.approvals.clone()
        };
        harness
            .spawn_with_command(
                current_dir,
                combined_prompt,
                grok_command,
                env,
                &self.cmd,
                approvals,
            )
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
        let mut harness = AcpAgentHarness::with_session_namespace("grok_sessions");
        if let Some(model) = &self.model {
            harness = harness.with_model(model);
        }
        let combined_prompt = self.append_prompt.combine_prompt(prompt);
        let grok_command = self.build_command_builder()?.build_follow_up(&[])?;
        let approvals = if self.always_approve.unwrap_or(false) {
            None
        } else {
            self.approvals.clone()
        };
        harness
            .spawn_follow_up_with_command(
                current_dir,
                combined_prompt,
                session_id,
                grok_command,
                env,
                &self.cmd,
                approvals,
            )
            .await
    }

    fn normalize_logs(
        &self,
        msg_store: Arc<MsgStore>,
        worktree_path: &Path,
    ) -> Vec<tokio::task::JoinHandle<()>> {
        // Grok stderr is almost entirely RUST_LOG/tracing (often multi-line dumps of
        // thinking JSON / tool payloads). Drop it so historic replay shows chat, not
        // a single giant red ErrorMessage.
        super::acp::normalize_logs_dropping_tracing_stderr(msg_store, worktree_path)
    }

    fn default_mcp_config_path(&self) -> Option<std::path::PathBuf> {
        // MCP servers live under [mcp_servers.*] in ~/.grok/config.toml
        dirs::home_dir().map(|home| home.join(".grok").join("config.toml"))
    }

    fn get_availability_info(&self) -> AvailabilityInfo {
        if let Some(timestamp) = dirs::home_dir()
            .and_then(|home| std::fs::metadata(home.join(".grok").join("auth.json")).ok())
            .and_then(|m| m.modified().ok())
            .and_then(|modified| modified.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|d| d.as_secs() as i64)
        {
            return AvailabilityInfo::LoginDetected {
                last_auth_timestamp: timestamp,
            };
        }

        let binary_found = resolve_executable_path_blocking(Self::base_command()).is_some()
            || dirs::home_dir()
                .map(|home| home.join(".grok").join("bin").join("grok").exists())
                .unwrap_or(false);

        let mcp_config_found = self
            .default_mcp_config_path()
            .map(|p| p.exists())
            .unwrap_or(false);

        if binary_found || mcp_config_found {
            AvailabilityInfo::InstallationFound
        } else {
            AvailabilityInfo::NotFound
        }
    }

    fn get_preset_options(&self) -> ExecutorConfig {
        ExecutorConfig {
            executor: BaseCodingAgent::Grok,
            variant: None,
            model_id: self.model.clone(),
            agent_id: None,
            reasoning_id: None,
            permission_policy: Some(if self.always_approve.unwrap_or(false) {
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
        let options = ExecutorDiscoveredOptions {
            model_selector: ModelSelectorConfig {
                models: vec![ModelInfo {
                    id: "grok-4.5".to_string(),
                    name: "Grok 4.5".to_string(),
                    provider_id: None,
                    reasoning_options: vec![],
                }],
                default_model: Some("grok-4.5".to_string()),
                permissions: vec![PermissionPolicy::Auto, PermissionPolicy::Supervised],
                ..Default::default()
            },
            ..Default::default()
        };
        Ok(Box::pin(futures::stream::once(async move {
            patch::executor_discovered_options(options)
        })))
    }
}
