use std::time::Duration;

use api_types::{
    AgentTask, AgentTaskStatus, ClaimAgentTaskRequest, CreateIssueCommentRequest,
    UpdateAgentTaskRequest,
};
use db::{
    DBService,
    models::{
        coding_agent_turn::CodingAgentTurn,
        execution_process::{ExecutionProcess, ExecutionProcessRunReason, ExecutionProcessStatus},
        repo::Repo,
        session::Session,
        workspace::{CreateWorkspace, Workspace},
        workspace_repo::{CreateWorkspaceRepo, WorkspaceRepo},
    },
};
use executors::{
    actions::{
        ExecutorAction, ExecutorActionType, coding_agent_follow_up::CodingAgentFollowUpRequest,
        coding_agent_initial::CodingAgentInitialRequest,
    },
    executors::BaseCodingAgent,
    profile::ExecutorConfig,
};
use services::services::{
    container::ContainerService,
    remote_client::{RemoteClient, RemoteClientError},
    remote_sync,
};
use tokio::time::interval;
use tracing::{error, info, warn};
use uuid::Uuid;

/// Polls Remote for queued agent tasks and starts local coding workspaces.
pub struct AgentTaskWatcher<C: ContainerService> {
    db: DBService,
    container: C,
    remote_client: RemoteClient,
    host_id: String,
    poll_interval: Duration,
}

impl<C: ContainerService + Clone + Send + Sync + 'static> AgentTaskWatcher<C> {
    pub fn spawn(
        db: DBService,
        container: C,
        remote_client: RemoteClient,
        host_id: String,
    ) -> tokio::task::JoinHandle<()> {
        let service = Self {
            db,
            container,
            remote_client,
            host_id,
            poll_interval: Duration::from_secs(5),
        };
        tokio::spawn(async move {
            service.start().await;
        })
    }

    async fn start(&self) {
        info!(
            host_id = %self.host_id,
            interval_secs = self.poll_interval.as_secs(),
            "Starting agent task watcher"
        );
        let mut ticker = interval(self.poll_interval);
        loop {
            ticker.tick().await;
            if let Err(e) = self.claim_and_run_once().await {
                warn!(?e, "agent task watcher iteration failed");
            }
        }
    }

    async fn claim_and_run_once(&self) -> Result<(), RemoteClientError> {
        let claimed = self
            .remote_client
            .claim_agent_task(&ClaimAgentTaskRequest {
                host_id: self.host_id.clone(),
            })
            .await?;

        let Some(task) = claimed.agent_task else {
            return Ok(());
        };

        info!(
            task_id = %task.id,
            agent_id = %task.agent_id,
            issue_id = %task.issue_id,
            "Claimed agent task"
        );

        let agent = match self.remote_client.get_agent(task.agent_id).await {
            Ok(agent) => agent,
            Err(e) => {
                self.fail_or_requeue(
                    &task,
                    Some(task.issue_id),
                    "agent",
                    format!("failed to load agent: {e}"),
                )
                .await;
                return Ok(());
            }
        };

        let issue = match self.remote_client.get_issue(task.issue_id).await {
            Ok(issue) => issue,
            Err(e) => {
                self.fail_or_requeue(
                    &task,
                    Some(task.issue_id),
                    &agent.name,
                    format!("failed to load issue: {e}"),
                )
                .await;
                return Ok(());
            }
        };

        let repos = match Repo::list_all(&self.db.pool).await {
            Ok(repos) if !repos.is_empty() => repos,
            Ok(_) => {
                self.fail_or_requeue(
                    &task,
                    Some(issue.id),
                    &agent.name,
                    "no local repos configured; register a repo before assigning agents".into(),
                )
                .await;
                return Ok(());
            }
            Err(e) => {
                self.fail_or_requeue(
                    &task,
                    Some(issue.id),
                    &agent.name,
                    format!("failed to list repos: {e}"),
                )
                .await;
                return Ok(());
            }
        };

        let project_name = self
            .remote_client
            .get_remote_project(agent.project_id)
            .await
            .ok()
            .map(|p| p.name.to_lowercase());

        let selected = select_repo_for_task(
            &repos,
            task.preferred_repo_id.as_deref(),
            project_name.as_deref(),
        );
        if selected.is_none() && repos.len() > 1 {
            warn!(
                task_id = %task.id,
                repo_count = repos.len(),
                repos = ?repos.iter().map(|r| &r.name).collect::<Vec<_>>(),
                preferred = ?task.preferred_repo_id,
                project = ?project_name,
                "multiple local repos; falling back to first repo"
            );
        }
        let Some(repo) = selected.cloned().or_else(|| repos.first().cloned()) else {
            self.fail_or_requeue(
                &task,
                Some(issue.id),
                &agent.name,
                "no repo available".into(),
            )
            .await;
            return Ok(());
        };
        info!(
            task_id = %task.id,
            repo_id = %repo.id,
            repo_name = %repo.name,
            preferred_repo_id = ?task.preferred_repo_id,
            resume_session_id = ?task.resume_session_id,
            force_fresh = task.force_fresh_session,
            attempt = task.attempt,
            max_attempts = task.max_attempts,
            "Selected repo for agent task"
        );

        let executor = agent
            .default_executor
            .as_deref()
            .and_then(|name| name.parse::<BaseCodingAgent>().ok())
            .unwrap_or(BaseCodingAgent::ClaudeCode);
        let executor_config = ExecutorConfig::new(executor);

        let mut prompt = format!(
            "You are agent \"{}\".\n\n{}\n\nWork on issue {} — {}.\n\n{}",
            agent.name,
            agent.instructions,
            issue.simple_id,
            issue.title,
            issue.description.as_deref().unwrap_or("(no description)")
        );
        if let Some(step) = task
            .execution_prompt
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
        {
            prompt.push_str("\n\n---\n## Pipeline step context\n\n");
            prompt.push_str(step);
        }

        // Prefer (agent, issue) session resume when claim populated resume_session_id.
        if !task.force_fresh_session
            && let Some(resume_id) = task.resume_session_id
            && let Ok(Some(session)) = Session::find_by_id(&self.db.pool, resume_id).await
            && let Ok(Some(workspace)) =
                Workspace::find_by_id(&self.db.pool, session.workspace_id).await
        {
            info!(
                task_id = %task.id,
                session_id = %session.id,
                workspace_id = %workspace.id,
                "Resuming prior (agent, issue) coding session"
            );

            if let Err(e) = self.container.ensure_container_exists(&workspace).await {
                warn!(
                    ?e,
                    "ensure_container_exists failed; falling back to fresh workspace"
                );
            } else {
                let repos = match WorkspaceRepo::find_repos_for_workspace(
                    &self.db.pool,
                    workspace.id,
                )
                .await
                {
                    Ok(r) => r,
                    Err(e) => {
                        self.fail_or_requeue(
                            &task,
                            Some(issue.id),
                            &agent.name,
                            format!("failed to load workspace repos for resume: {e}"),
                        )
                        .await;
                        return Ok(());
                    }
                };
                let cleanup_action = self.container.cleanup_actions_for_repos(&repos);
                let working_dir = session
                    .agent_working_dir
                    .as_ref()
                    .filter(|dir| !dir.is_empty())
                    .cloned();
                let latest =
                    CodingAgentTurn::find_latest_session_info(&self.db.pool, session.id).await;
                let action_type = match latest {
                    Ok(Some(info)) => {
                        ExecutorActionType::CodingAgentFollowUpRequest(CodingAgentFollowUpRequest {
                            prompt: prompt.clone(),
                            session_id: info.session_id,
                            reset_to_message_id: None,
                            executor_config: executor_config.clone(),
                            working_dir: working_dir.clone(),
                        })
                    }
                    _ => ExecutorActionType::CodingAgentInitialRequest(CodingAgentInitialRequest {
                        prompt: prompt.clone(),
                        executor_config: executor_config.clone(),
                        working_dir,
                    }),
                };
                let action = ExecutorAction::new(action_type, cleanup_action.map(Box::new));

                let _ = self
                    .remote_client
                    .update_agent_task(
                        task.id,
                        &UpdateAgentTaskRequest {
                            status: Some(AgentTaskStatus::Running),
                            failure_reason: None,
                            local_workspace_id: Some(Some(workspace.id)),
                            local_session_id: Some(Some(session.id)),
                            claimed_by_host: None,
                            attempt: None,
                        },
                    )
                    .await;

                match self
                    .container
                    .start_execution(
                        &workspace,
                        &session,
                        &action,
                        &ExecutionProcessRunReason::CodingAgent,
                    )
                    .await
                {
                    Ok(_execution) => {
                        let remote = self.remote_client.clone();
                        let db = self.db.clone();
                        let task_id = task.id;
                        let issue_id = issue.id;
                        let agent_name = agent.name.clone();
                        let session_id = session.id;
                        tokio::spawn(async move {
                            Self::wait_and_finalize(
                                remote, db, task_id, issue_id, agent_name, session_id,
                            )
                            .await;
                        });
                        return Ok(());
                    }
                    Err(e) => {
                        warn!(
                            ?e,
                            task_id = %task.id,
                            "resume follow-up failed; falling back to fresh workspace"
                        );
                    }
                }
            }
        }

        let workspace_id = Uuid::new_v4();
        let branch = format!(
            "vk/agent-{}-{}",
            agent.name.replace(' ', "-").to_lowercase(),
            &issue.simple_id.to_lowercase()
        );

        let workspace = match Workspace::create(
            &self.db.pool,
            &CreateWorkspace {
                branch,
                name: Some(format!("{} · {}", agent.name, issue.simple_id)),
                kind: Default::default(),
            },
            workspace_id,
        )
        .await
        {
            Ok(ws) => ws,
            Err(e) => {
                self.fail_or_requeue(
                    &task,
                    Some(issue.id),
                    &agent.name,
                    format!("failed to create workspace: {e}"),
                )
                .await;
                return Ok(());
            }
        };

        if let Err(e) = WorkspaceRepo::create_many(
            &self.db.pool,
            workspace.id,
            &[CreateWorkspaceRepo {
                repo_id: repo.id,
                target_branch: repo
                    .default_target_branch
                    .clone()
                    .unwrap_or_else(|| "main".to_string()),
            }],
        )
        .await
        {
            self.fail_or_requeue(
                &task,
                Some(issue.id),
                &agent.name,
                format!("failed to attach repo: {e}"),
            )
            .await;
            return Ok(());
        }

        if let Err(e) = remote_sync::register_local_workspace_on_remote(
            &self.remote_client,
            &workspace,
            agent.project_id,
            issue.id,
            None,
        )
        .await
        {
            warn!(?e, "failed to register agent workspace on remote");
        }

        let _ = self
            .remote_client
            .update_agent_task(
                task.id,
                &UpdateAgentTaskRequest {
                    status: Some(AgentTaskStatus::Running),
                    failure_reason: None,
                    local_workspace_id: Some(Some(workspace.id)),
                    local_session_id: None,
                    claimed_by_host: None,
                    attempt: None,
                },
            )
            .await;

        match self
            .container
            .start_workspace(&workspace, executor_config, prompt)
            .await
        {
            Ok(execution) => {
                let session_id = execution.session_id;
                let _ = self
                    .remote_client
                    .update_agent_task(
                        task.id,
                        &UpdateAgentTaskRequest {
                            status: Some(AgentTaskStatus::Running),
                            failure_reason: None,
                            local_workspace_id: Some(Some(workspace.id)),
                            local_session_id: Some(Some(session_id)),
                            claimed_by_host: None,
                            attempt: None,
                        },
                    )
                    .await;

                let remote = self.remote_client.clone();
                let db = self.db.clone();
                let task_id = task.id;
                let issue_id = issue.id;
                let agent_name = agent.name.clone();
                tokio::spawn(async move {
                    Self::wait_and_finalize(remote, db, task_id, issue_id, agent_name, session_id)
                        .await;
                });
            }
            Err(e) => {
                error!(?e, task_id = %task.id, "failed to start workspace for agent task");
                self.fail_or_requeue(
                    &task,
                    Some(issue.id),
                    &agent.name,
                    format!("failed to start workspace: {e}"),
                )
                .await;
            }
        }

        Ok(())
    }

    async fn fail_or_requeue(
        &self,
        task: &AgentTask,
        issue_id: Option<Uuid>,
        agent_name: &str,
        reason: String,
    ) {
        let can_retry = task.attempt < task.max_attempts;
        if can_retry {
            warn!(
                task_id = %task.id,
                attempt = task.attempt,
                max_attempts = task.max_attempts,
                %reason,
                "requeueing agent task after failure"
            );
            let _ = self
                .remote_client
                .update_agent_task(
                    task.id,
                    &UpdateAgentTaskRequest {
                        status: Some(AgentTaskStatus::Queued),
                        failure_reason: Some(Some(format!(
                            "retry {}/{}: {reason}",
                            task.attempt, task.max_attempts
                        ))),
                        local_workspace_id: Some(None),
                        local_session_id: Some(None),
                        claimed_by_host: Some(None),
                        attempt: None,
                    },
                )
                .await;
            if let Some(issue_id) = issue_id {
                let message = format!(
                    "Agent **{agent_name}** will retry ({}/{}): `{reason}`",
                    task.attempt, task.max_attempts
                );
                let _ = self
                    .remote_client
                    .create_issue_comment(&CreateIssueCommentRequest {
                        id: None,
                        issue_id,
                        parent_id: None,
                        message,
                    })
                    .await;
            }
            return;
        }

        warn!(task_id = %task.id, %reason, "marking agent task failed");
        let _ = self
            .remote_client
            .update_agent_task(
                task.id,
                &UpdateAgentTaskRequest {
                    status: Some(AgentTaskStatus::Failed),
                    failure_reason: Some(Some(reason.clone())),
                    local_workspace_id: None,
                    local_session_id: None,
                    claimed_by_host: None,
                    attempt: None,
                },
            )
            .await;

        if let Some(issue_id) = issue_id {
            let message = format!("Agent **{agent_name}** failed to start this task: `{reason}`");
            let _ = self
                .remote_client
                .create_issue_comment(&CreateIssueCommentRequest {
                    id: None,
                    issue_id,
                    parent_id: None,
                    message,
                })
                .await;
        }
    }

    async fn wait_and_finalize(
        remote: RemoteClient,
        db: DBService,
        task_id: Uuid,
        issue_id: Uuid,
        agent_name: String,
        session_id: Uuid,
    ) {
        for _ in 0..720 {
            tokio::time::sleep(Duration::from_secs(5)).await;
            match ExecutionProcess::find_by_session_id(&db.pool, session_id, false).await {
                Ok(procs) if procs.is_empty() => continue,
                Ok(procs) => {
                    let any_running = procs
                        .iter()
                        .any(|p| p.status == ExecutionProcessStatus::Running);
                    if any_running {
                        continue;
                    }
                    let failed = procs
                        .iter()
                        .any(|p| p.status == ExecutionProcessStatus::Failed);
                    if failed {
                        // Requeue while attempt budget remains; otherwise mark failed.
                        let can_retry = match remote.get_agent_task(task_id).await {
                            Ok(task) => task.attempt < task.max_attempts,
                            Err(_) => false,
                        };
                        if can_retry {
                            warn!(
                                %task_id,
                                "coding agent failed; requeueing for retry"
                            );
                            let _ = remote
                                .update_agent_task(
                                    task_id,
                                    &UpdateAgentTaskRequest {
                                        status: Some(AgentTaskStatus::Queued),
                                        failure_reason: Some(Some(
                                            "coding agent execution failed; retrying".into(),
                                        )),
                                        local_workspace_id: Some(None),
                                        local_session_id: Some(None),
                                        claimed_by_host: Some(None),
                                        attempt: None,
                                    },
                                )
                                .await;
                            let _ = remote
                                .create_issue_comment(&CreateIssueCommentRequest {
                                    id: None,
                                    issue_id,
                                    parent_id: None,
                                    message: format!(
                                        "Agent **{agent_name}** failed and will retry. See linked workspace."
                                    ),
                                })
                                .await;
                            return;
                        }
                    }
                    let status = if failed {
                        AgentTaskStatus::Failed
                    } else {
                        AgentTaskStatus::Completed
                    };
                    let reason = if failed {
                        Some(Some("coding agent execution failed".into()))
                    } else {
                        None
                    };
                    let _ = remote
                        .update_agent_task(
                            task_id,
                            &UpdateAgentTaskRequest {
                                status: Some(status),
                                failure_reason: reason,
                                local_workspace_id: None,
                                local_session_id: None,
                                claimed_by_host: None,
                                attempt: None,
                            },
                        )
                        .await;

                    let message = if failed {
                        format!(
                            "Agent **{agent_name}** finished with failures. See linked workspace session."
                        )
                    } else {
                        format!(
                            "Agent **{agent_name}** completed this task. See linked workspace for details."
                        )
                    };
                    let _ = remote
                        .create_issue_comment(&CreateIssueCommentRequest {
                            id: None,
                            issue_id,
                            parent_id: None,
                            message,
                        })
                        .await;
                    return;
                }
                Err(e) => {
                    warn!(?e, %session_id, "failed to list execution processes");
                }
            }
        }

        let timeout_reason = "timed out waiting for coding agent";
        let can_retry = match remote.get_agent_task(task_id).await {
            Ok(task) => task.attempt < task.max_attempts,
            Err(_) => false,
        };
        if can_retry {
            warn!(%task_id, "coding agent timed out; requeueing for retry");
            let _ = remote
                .update_agent_task(
                    task_id,
                    &UpdateAgentTaskRequest {
                        status: Some(AgentTaskStatus::Queued),
                        failure_reason: Some(Some(format!("{timeout_reason}; retrying"))),
                        local_workspace_id: Some(None),
                        local_session_id: Some(None),
                        claimed_by_host: Some(None),
                        attempt: None,
                    },
                )
                .await;
            let _ = remote
                .create_issue_comment(&CreateIssueCommentRequest {
                    id: None,
                    issue_id,
                    parent_id: None,
                    message: format!(
                        "Agent **{agent_name}** timed out and will retry. See linked workspace."
                    ),
                })
                .await;
            return;
        }

        let _ = remote
            .update_agent_task(
                task_id,
                &UpdateAgentTaskRequest {
                    status: Some(AgentTaskStatus::Failed),
                    failure_reason: Some(Some(timeout_reason.into())),
                    local_workspace_id: None,
                    local_session_id: None,
                    claimed_by_host: None,
                    attempt: None,
                },
            )
            .await;
        let _ = remote
            .create_issue_comment(&CreateIssueCommentRequest {
                id: None,
                issue_id,
                parent_id: None,
                message: format!("Agent **{agent_name}** failed: `{timeout_reason}`"),
            })
            .await;
    }
}

fn select_repo_for_task<'a>(
    repos: &'a [Repo],
    preferred_repo_id: Option<&str>,
    project_name: Option<&str>,
) -> Option<&'a Repo> {
    if let Some(preferred) = preferred_repo_id.map(str::trim).filter(|s| !s.is_empty()) {
        if let Ok(id) = Uuid::parse_str(preferred) {
            if let Some(repo) = repos.iter().find(|r| r.id == id) {
                return Some(repo);
            }
        }
        // Squad working_directory is often an absolute path — match repo.path.
        let preferred_path = std::path::Path::new(preferred);
        if preferred_path.is_absolute() || preferred.contains('/') || preferred.contains('\\') {
            if let Some(repo) = repos.iter().find(|r| {
                let rp = r.path.as_path();
                rp == preferred_path
                    || preferred_path.starts_with(rp)
                    || rp.starts_with(preferred_path)
            }) {
                return Some(repo);
            }
            // Case-insensitive string compare for path strings
            let pref_norm = preferred.trim_end_matches('/').to_lowercase();
            if let Some(repo) = repos.iter().find(|r| {
                let rp = r
                    .path
                    .to_string_lossy()
                    .trim_end_matches('/')
                    .to_lowercase();
                rp == pref_norm || pref_norm.starts_with(&rp) || rp.starts_with(&pref_norm)
            }) {
                return Some(repo);
            }
        }
        if let Some(repo) = repos.iter().find(|r| {
            r.name.eq_ignore_ascii_case(preferred) || r.display_name.eq_ignore_ascii_case(preferred)
        }) {
            return Some(repo);
        }
    }

    if let Some(project) = project_name {
        let needle = project.to_lowercase();
        if let Some(repo) = repos.iter().find(|r| {
            let n = r.name.to_lowercase();
            let d = r.display_name.to_lowercase();
            n == needle
                || d == needle
                || n.contains(&needle)
                || d.contains(&needle)
                || needle.contains(&n)
        }) {
            return Some(repo);
        }
    }

    // Prefer vibekanban / hyper-vibekanban as a last heuristic before first repo.
    repos.iter().find(|r| {
        let n = r.name.to_lowercase();
        let d = r.display_name.to_lowercase();
        n.contains("vibekanban")
            || n.contains("hyper-vibekanban")
            || d.contains("vibekanban")
            || d.contains("hyper-vibekanban")
    })
}
