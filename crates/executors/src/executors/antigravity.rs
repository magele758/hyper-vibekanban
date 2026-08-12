use std::{path::Path, sync::Arc};

use async_trait::async_trait;
use derivative::Derivative;
use futures::StreamExt;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use ts_rs::TS;
use workspace_utils::msg_store::MsgStore;

pub use super::acp::AcpAgentHarness;
use crate::{
    approvals::ExecutorApprovalService,
    command::{CmdOverrides, CommandBuildError, CommandBuilder, apply_overrides},
    env::ExecutionEnv,
    executor_discovery::{ExecutorConfigCacheKey, ExecutorDiscoveredOptions},
    executors::{
        AppendPrompt, AvailabilityInfo, BaseCodingAgent, ExecutorError, SpawnedChild,
        StandardCodingAgentExecutor, utils::executor_options_cache,
    },
    logs::utils::patch,
    model_discovery::{ANTIGRAVITY_DEFAULT_MODEL, antigravity_default_models},
    model_selector::{ModelSelectorConfig, PermissionPolicy},
    profile::ExecutorConfig,
};

const SUPPRESSED_STDERR_PATTERNS: &[&str] = &[
    "was started but never ended. Skipping metrics.",
    "YOLO mode is enabled. All tool calls will be automatically approved.",
    "Failed to redirect output for CLI",
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
    #[serde(skip)]
    #[ts(skip)]
    #[derivative(Debug = "ignore", PartialEq = "ignore")]
    pub approvals: Option<Arc<dyn ExecutorApprovalService>>,
}

impl Antigravity {
    pub fn base_command() -> &'static str {
        "agy"
    }

    fn build_command_builder(&self) -> Result<CommandBuilder, CommandBuildError> {
        // Default to `agy` command if installed, fallback to `@google/antigravity-cli`
        let mut builder = CommandBuilder::new(Self::base_command());

        if let Some(model) = &self.model {
            builder = builder.extend_params(["--model", model.as_str()]);
        }

        if self.dangerously_skip_permissions.unwrap_or(false) {
            builder = builder.extend_params(["--dangerously-skip-permissions"]);
        }

        apply_overrides(builder, &self.cmd)
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

    fn use_approvals(&mut self, approvals: Arc<dyn ExecutorApprovalService>) {
        self.approvals = Some(approvals);
    }

    async fn spawn(
        &self,
        current_dir: &Path,
        prompt: &str,
        env: &ExecutionEnv,
    ) -> Result<SpawnedChild, ExecutorError> {
        let harness = AcpAgentHarness::with_session_namespace("antigravity_sessions");
        let combined_prompt = self.append_prompt.combine_prompt(prompt);
        let agy_command = self.build_command_builder()?.build_initial()?;
        let approvals = if self.dangerously_skip_permissions.unwrap_or(false) {
            None
        } else {
            self.approvals.clone()
        };
        harness
            .spawn_with_command(
                current_dir,
                combined_prompt,
                agy_command,
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
        let harness = AcpAgentHarness::with_session_namespace("antigravity_sessions");
        let combined_prompt = self.append_prompt.combine_prompt(prompt);
        let agy_command = self.build_command_builder()?.build_follow_up(&[])?;
        let approvals = if self.dangerously_skip_permissions.unwrap_or(false) {
            None
        } else {
            self.approvals.clone()
        };
        harness
            .spawn_follow_up_with_command(
                current_dir,
                combined_prompt,
                session_id,
                agy_command,
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
        super::acp::normalize_logs_with_suppressed_stderr_patterns(
            msg_store,
            worktree_path,
            SUPPRESSED_STDERR_PATTERNS,
        )
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

        if mcp_config_found || installation_indicator_found {
            AvailabilityInfo::InstallationFound
        } else {
            AvailabilityInfo::NotFound
        }
    }

    fn get_preset_options(&self) -> ExecutorConfig {
        use crate::model_selector::*;
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
