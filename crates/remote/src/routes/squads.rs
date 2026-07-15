use api_types::{
    AddSquadMemberRequest, AgentTaskTrigger, ApproveSquadRunRequest, ApproveSquadRunResponse,
    CreateSquadRequest, DeleteResponse, ListSquadMembersResponse, ListSquadRunsResponse,
    ListSquadsQuery, ListSquadsResponse, MutationResponse, RunSquadRequest, RunSquadResponse,
    Squad, SquadMember, SquadRunStatus, SquadTargetType, UpdateSquadRequest,
};
use axum::{
    Json, Router,
    extract::{Extension, Path, Query, State},
    http::StatusCode,
    routing::{delete, get, post},
};
use chrono::Utc;
use serde_json::json;
use tracing::instrument;
use uuid::Uuid;

use super::{
    error::{ErrorResponse, db_error},
    organization_members::ensure_project_access,
};
use crate::{
    AppState,
    auth::RequestContext,
    db::{
        agent_tasks::AgentTaskRepository,
        inbox::InboxRepository,
        squad_runs::SquadRunRepository,
        squads::{SquadRepository, topological_order},
    },
};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/squads", get(list_squads).post(create_squad))
        .route(
            "/squads/{id}",
            get(get_squad).put(update_squad).delete(delete_squad),
        )
        .route("/squads/{id}/run", post(run_squad))
        .route(
            "/squads/{id}/members",
            get(list_squad_members).post(add_squad_member),
        )
        .route(
            "/squads/{squad_id}/members/{member_id}",
            delete(remove_squad_member),
        )
        .route("/issues/{issue_id}/squad-runs", get(list_issue_squad_runs))
        .route("/squad-runs/{id}/approve", post(approve_squad_run))
}

#[instrument(name = "squads.list", skip(state, ctx), fields(user_id = %ctx.user.id))]
async fn list_squads(
    State(state): State<AppState>,
    Extension(ctx): Extension<RequestContext>,
    Query(query): Query<ListSquadsQuery>,
) -> Result<Json<ListSquadsResponse>, ErrorResponse> {
    ensure_project_access(state.pool(), ctx.user.id, query.project_id).await?;

    let squads = SquadRepository::list_by_project(state.pool(), query.project_id)
        .await
        .map_err(|e| {
            tracing::error!(?e, "failed to list squads");
            ErrorResponse::new(StatusCode::INTERNAL_SERVER_ERROR, "failed to list squads")
        })?;

    Ok(Json(ListSquadsResponse { squads }))
}

#[instrument(name = "squads.get", skip(state, ctx), fields(id = %id, user_id = %ctx.user.id))]
async fn get_squad(
    State(state): State<AppState>,
    Extension(ctx): Extension<RequestContext>,
    Path(id): Path<Uuid>,
) -> Result<Json<Squad>, ErrorResponse> {
    let squad = load_and_authorize(&state, ctx.user.id, id).await?;
    Ok(Json(squad))
}

#[instrument(name = "squads.create", skip(state, ctx, payload), fields(user_id = %ctx.user.id))]
async fn create_squad(
    State(state): State<AppState>,
    Extension(ctx): Extension<RequestContext>,
    Json(payload): Json<CreateSquadRequest>,
) -> Result<Json<MutationResponse<Squad>>, ErrorResponse> {
    ensure_project_access(state.pool(), ctx.user.id, payload.project_id).await?;

    let target_type = payload.target_type.unwrap_or_default();
    validate_target(
        target_type,
        payload.issue_id,
        payload.working_directory.as_deref(),
    )?;

    let response = SquadRepository::create(
        state.pool(),
        payload.id,
        payload.project_id,
        payload.name,
        payload.leader_agent_id,
        payload.pipeline,
        target_type,
        payload.issue_id,
        payload.working_directory,
        payload.on_assign.unwrap_or_default(),
    )
    .await
    .map_err(|e| db_error(e, "failed to create squad"))?;

    Ok(Json(response))
}

#[instrument(name = "squads.update", skip(state, ctx, payload), fields(id = %id, user_id = %ctx.user.id))]
async fn update_squad(
    State(state): State<AppState>,
    Extension(ctx): Extension<RequestContext>,
    Path(id): Path<Uuid>,
    Json(payload): Json<UpdateSquadRequest>,
) -> Result<Json<MutationResponse<Squad>>, ErrorResponse> {
    let existing = load_and_authorize(&state, ctx.user.id, id).await?;

    let target_type = payload.target_type.unwrap_or(existing.target_type);
    let issue_id = match &payload.issue_id {
        Some(v) => *v,
        None => existing.issue_id,
    };
    let working_directory = match &payload.working_directory {
        Some(v) => v.clone(),
        None => existing.working_directory.clone(),
    };
    validate_target(target_type, issue_id, working_directory.as_deref())?;

    let response = SquadRepository::update(
        state.pool(),
        id,
        payload.name,
        payload.leader_agent_id,
        payload.pipeline,
        payload.target_type,
        payload.issue_id,
        payload.working_directory,
        payload.on_assign,
    )
    .await
    .map_err(|e| db_error(e, "failed to update squad"))?;

    Ok(Json(response))
}

#[instrument(name = "squads.delete", skip(state, ctx), fields(id = %id, user_id = %ctx.user.id))]
async fn delete_squad(
    State(state): State<AppState>,
    Extension(ctx): Extension<RequestContext>,
    Path(id): Path<Uuid>,
) -> Result<Json<DeleteResponse>, ErrorResponse> {
    load_and_authorize(&state, ctx.user.id, id).await?;

    let response = SquadRepository::delete(state.pool(), id)
        .await
        .map_err(|e| {
            tracing::error!(?e, "failed to delete squad");
            ErrorResponse::new(StatusCode::INTERNAL_SERVER_ERROR, "internal server error")
        })?;

    Ok(Json(response))
}

#[instrument(name = "squads.run", skip(state, ctx, payload), fields(id = %id, user_id = %ctx.user.id))]
async fn run_squad(
    State(state): State<AppState>,
    Extension(ctx): Extension<RequestContext>,
    Path(id): Path<Uuid>,
    payload: Option<Json<RunSquadRequest>>,
) -> Result<Json<RunSquadResponse>, ErrorResponse> {
    let squad = load_and_authorize(&state, ctx.user.id, id).await?;
    let overrides = payload.map(|j| j.0).unwrap_or_default();

    let result = execute_squad_pipeline(state.pool(), &squad, &overrides, Some(ctx.user.id))
        .await
        .map_err(|e| {
            tracing::error!(?e, squad_id = %id, "failed to run squad pipeline");
            ErrorResponse::new(StatusCode::BAD_REQUEST, e.to_string())
        })?;

    Ok(Json(result))
}

#[instrument(name = "squad_runs.list_by_issue", skip(state, ctx), fields(issue_id = %issue_id, user_id = %ctx.user.id))]
async fn list_issue_squad_runs(
    State(state): State<AppState>,
    Extension(ctx): Extension<RequestContext>,
    Path(issue_id): Path<Uuid>,
) -> Result<Json<ListSquadRunsResponse>, ErrorResponse> {
    let issue = sqlx::query_scalar::<_, Uuid>("SELECT project_id FROM issues WHERE id = $1")
        .bind(issue_id)
        .fetch_optional(state.pool())
        .await
        .map_err(|e| {
            tracing::error!(?e, "failed to load issue");
            ErrorResponse::new(StatusCode::INTERNAL_SERVER_ERROR, "failed to load issue")
        })?
        .ok_or_else(|| ErrorResponse::new(StatusCode::NOT_FOUND, "issue not found"))?;
    ensure_project_access(state.pool(), ctx.user.id, issue).await?;

    let runs = SquadRunRepository::list_by_issue(state.pool(), issue_id, 20)
        .await
        .map_err(|e| {
            tracing::error!(?e, "failed to list squad runs");
            ErrorResponse::new(
                StatusCode::INTERNAL_SERVER_ERROR,
                "failed to list squad runs",
            )
        })?;
    Ok(Json(ListSquadRunsResponse { runs }))
}

#[instrument(name = "squad_runs.approve", skip(state, ctx, payload), fields(id = %id, user_id = %ctx.user.id))]
async fn approve_squad_run(
    State(state): State<AppState>,
    Extension(ctx): Extension<RequestContext>,
    Path(id): Path<Uuid>,
    Json(payload): Json<ApproveSquadRunRequest>,
) -> Result<Json<ApproveSquadRunResponse>, ErrorResponse> {
    let run = SquadRunRepository::find_by_id(state.pool(), id)
        .await
        .map_err(|e| {
            tracing::error!(?e, "failed to load squad run");
            ErrorResponse::new(
                StatusCode::INTERNAL_SERVER_ERROR,
                "failed to load squad run",
            )
        })?
        .ok_or_else(|| ErrorResponse::new(StatusCode::NOT_FOUND, "squad run not found"))?;

    let squad = load_and_authorize(&state, ctx.user.id, run.squad_id).await?;
    if run.status != SquadRunStatus::WaitingApproval.as_str() {
        return Err(ErrorResponse::new(
            StatusCode::BAD_REQUEST,
            format!("run is not waiting for approval (status={})", run.status),
        ));
    }

    let decision = payload.decision.trim().to_lowercase();
    match decision.as_str() {
        "approve" => {
            let resume_node = run.resume_node_id.clone().ok_or_else(|| {
                ErrorResponse::new(StatusCode::BAD_REQUEST, "run has no resume_node_id")
            })?;
            let _ =
                SquadRunRepository::mark_status(state.pool(), id, SquadRunStatus::Running, None)
                    .await
                    .map_err(|e| {
                        tracing::error!(?e, "failed to mark run running");
                        ErrorResponse::new(
                            StatusCode::INTERNAL_SERVER_ERROR,
                            "failed to resume run",
                        )
                    })?;

            let overrides = RunSquadRequest {
                issue_id: Some(run.issue_id),
                working_directory: run.working_directory.clone(),
                start_from_node_id: Some(resume_node),
                resume_run_id: Some(id),
            };
            let resumed =
                execute_squad_pipeline(state.pool(), &squad, &overrides, Some(ctx.user.id))
                    .await
                    .map_err(|e| {
                        tracing::error!(?e, "failed to resume squad pipeline");
                        ErrorResponse::new(StatusCode::BAD_REQUEST, e.to_string())
                    })?;

            let run = SquadRunRepository::find_by_id(state.pool(), id)
                .await
                .ok()
                .flatten()
                .unwrap_or(run);
            Ok(Json(ApproveSquadRunResponse {
                run,
                resumed: Some(resumed),
            }))
        }
        "reject" => {
            let msg = payload
                .comment
                .clone()
                .unwrap_or_else(|| "rejected by user".into());
            let run = SquadRunRepository::mark_status(
                state.pool(),
                id,
                SquadRunStatus::Cancelled,
                Some(msg),
            )
            .await
            .map_err(|e| {
                tracing::error!(?e, "failed to cancel run");
                ErrorResponse::new(StatusCode::INTERNAL_SERVER_ERROR, "failed to cancel run")
            })?;
            Ok(Json(ApproveSquadRunResponse { run, resumed: None }))
        }
        "comment" => Err(ErrorResponse::new(
            StatusCode::BAD_REQUEST,
            "comment-only resume is not implemented yet; use approve/reject",
        )),
        _ => Err(ErrorResponse::new(
            StatusCode::BAD_REQUEST,
            "decision must be approve, reject, or comment",
        )),
    }
}

#[instrument(name = "squads.list_members", skip(state, ctx), fields(id = %id, user_id = %ctx.user.id))]
async fn list_squad_members(
    State(state): State<AppState>,
    Extension(ctx): Extension<RequestContext>,
    Path(id): Path<Uuid>,
) -> Result<Json<ListSquadMembersResponse>, ErrorResponse> {
    load_and_authorize(&state, ctx.user.id, id).await?;

    let members = SquadRepository::list_members(state.pool(), id)
        .await
        .map_err(|e| {
            tracing::error!(?e, "failed to list squad members");
            ErrorResponse::new(
                StatusCode::INTERNAL_SERVER_ERROR,
                "failed to list squad members",
            )
        })?;

    Ok(Json(ListSquadMembersResponse { members }))
}

#[instrument(name = "squads.add_member", skip(state, ctx, payload), fields(id = %id, user_id = %ctx.user.id))]
async fn add_squad_member(
    State(state): State<AppState>,
    Extension(ctx): Extension<RequestContext>,
    Path(id): Path<Uuid>,
    Json(payload): Json<AddSquadMemberRequest>,
) -> Result<Json<MutationResponse<SquadMember>>, ErrorResponse> {
    load_and_authorize(&state, ctx.user.id, id).await?;

    if payload.agent_id.is_none() && payload.user_id.is_none() {
        return Err(ErrorResponse::new(
            StatusCode::BAD_REQUEST,
            "agent_id or user_id is required",
        ));
    }
    if payload.agent_id.is_some() && payload.user_id.is_some() {
        return Err(ErrorResponse::new(
            StatusCode::BAD_REQUEST,
            "only one of agent_id or user_id may be set",
        ));
    }

    let response = SquadRepository::add_member(state.pool(), id, payload.agent_id, payload.user_id)
        .await
        .map_err(|e| db_error(e, "failed to add squad member"))?;

    Ok(Json(response))
}

#[instrument(name = "squads.remove_member", skip(state, ctx), fields(squad_id = %squad_id, member_id = %member_id, user_id = %ctx.user.id))]
async fn remove_squad_member(
    State(state): State<AppState>,
    Extension(ctx): Extension<RequestContext>,
    Path((squad_id, member_id)): Path<(Uuid, Uuid)>,
) -> Result<Json<DeleteResponse>, ErrorResponse> {
    load_and_authorize(&state, ctx.user.id, squad_id).await?;

    let response = SquadRepository::remove_member(state.pool(), member_id)
        .await
        .map_err(|e| {
            tracing::error!(?e, "failed to remove squad member");
            ErrorResponse::new(StatusCode::INTERNAL_SERVER_ERROR, "internal server error")
        })?;

    Ok(Json(response))
}

fn validate_target(
    target_type: SquadTargetType,
    issue_id: Option<Uuid>,
    working_directory: Option<&str>,
) -> Result<(), ErrorResponse> {
    if target_type.uses_issue() && issue_id.is_none() {
        // Allow saving without issue yet (editor draft), but warn only at run time.
        // Soft validation: ok for persist.
    }
    if target_type.uses_path() {
        let _ = working_directory; // path may be filled later
    }
    Ok(())
}

async fn load_and_authorize(
    state: &AppState,
    user_id: Uuid,
    id: Uuid,
) -> Result<Squad, ErrorResponse> {
    let squad = SquadRepository::find_by_id(state.pool(), id)
        .await
        .map_err(|e| {
            tracing::error!(?e, %id, "failed to load squad");
            ErrorResponse::new(StatusCode::INTERNAL_SERVER_ERROR, "failed to load squad")
        })?
        .ok_or_else(|| ErrorResponse::new(StatusCode::NOT_FOUND, "squad not found"))?;

    ensure_project_access(state.pool(), user_id, squad.project_id).await?;
    Ok(squad)
}

/// Execute a squad pipeline: resolve Issue+Path target, create/use issue, then
/// walk the pipeline (await agents, fork/join, control-flow).
///
/// `actor_user_id` receives Inbox notifications for `wait_approval` gates.
pub async fn execute_squad_pipeline(
    pool: &sqlx::PgPool,
    squad: &Squad,
    overrides: &RunSquadRequest,
    actor_user_id: Option<Uuid>,
) -> anyhow::Result<RunSquadResponse> {
    use std::{
        collections::{HashMap, HashSet},
        sync::{
            Arc,
            atomic::{AtomicBool, AtomicUsize, Ordering},
        },
        time::Duration,
    };

    use api_types::{AgentTaskStatus, SquadPipelineEdgeBranch, SquadPipelineNodeType};
    use tokio::sync::{Mutex, Notify};

    let target_type = squad.target_type;
    let working_directory = overrides
        .working_directory
        .clone()
        .or_else(|| squad.working_directory.clone())
        .filter(|s| !s.trim().is_empty());
    let issue_override = overrides.issue_id.or(squad.issue_id);

    if target_type.uses_issue() && issue_override.is_none() {
        anyhow::bail!("工作目标包含 Issue，但未选择 Issue");
    }
    if target_type.uses_path() && working_directory.is_none() {
        anyhow::bail!("工作目标包含目录，但未设置 working_directory");
    }

    if squad.pipeline.nodes.is_empty() {
        anyhow::bail!("流水线没有步骤，请先添加节点");
    }

    let issue_id = match target_type {
        SquadTargetType::Issue | SquadTargetType::IssueAndPath => {
            let id = issue_override.expect("validated");
            let row: Option<(Uuid,)> =
                sqlx::query_as("SELECT id FROM issues WHERE id = $1 AND project_id = $2")
                    .bind(id)
                    .bind(squad.project_id)
                    .fetch_optional(pool)
                    .await?;
            if row.is_none() {
                anyhow::bail!("Issue 不存在或不属于本项目");
            }
            id
        }
        SquadTargetType::Path => {
            create_path_run_issue(pool, squad, working_directory.as_deref()).await?
        }
    };

    let nodes_by_id: HashMap<&str, _> = squad
        .pipeline
        .nodes
        .iter()
        .map(|n| (n.id.as_str(), n))
        .collect();

    let mut outs: HashMap<&str, Vec<&api_types::SquadPipelineEdge>> = HashMap::new();
    let mut ins: HashMap<&str, Vec<&api_types::SquadPipelineEdge>> = HashMap::new();
    let mut indegree: HashMap<&str, usize> = squad
        .pipeline
        .nodes
        .iter()
        .map(|n| (n.id.as_str(), 0))
        .collect();
    for edge in &squad.pipeline.edges {
        if !nodes_by_id.contains_key(edge.source.as_str())
            || !nodes_by_id.contains_key(edge.target.as_str())
        {
            continue;
        }
        outs.entry(edge.source.as_str()).or_default().push(edge);
        ins.entry(edge.target.as_str()).or_default().push(edge);
        *indegree.entry(edge.target.as_str()).or_default() += 1;
    }

    let roots: Vec<&str> = squad
        .pipeline
        .nodes
        .iter()
        .filter(|n| indegree.get(n.id.as_str()).copied().unwrap_or(0) == 0)
        .map(|n| n.id.as_str())
        .collect();
    let roots = if roots.is_empty() {
        vec![squad.pipeline.nodes[0].id.as_str()]
    } else {
        roots
    };

    // Mid-pipeline entry: treat start_from as the sole root; upstream is skipped.
    let roots = if let Some(ref start_id) = overrides.start_from_node_id {
        if !nodes_by_id.contains_key(start_id.as_str()) {
            anyhow::bail!("start_from_node_id `{start_id}` not found in pipeline");
        }
        vec![start_id.as_str()]
    } else {
        roots
    };

    let loop_config = squad.pipeline.loop_config.clone();
    let loop_note = loop_config
        .as_ref()
        .map(|lc| {
            format!(
                "\n\nLoop config: max_iterations={}, success_condition={}",
                lc.max_iterations
                    .map(|n| n.to_string())
                    .unwrap_or_else(|| "1".into()),
                lc.success_condition.as_deref().unwrap_or("(none)")
            )
        })
        .unwrap_or_default();

    /// Cap sync Wait sleeps so HTTP /run doesn't hang on wait alone.
    const MAX_SYNC_WAIT_SECS: i32 = 30;
    /// Default agent await timeout (overridable via SQUAD_AGENT_AWAIT_TIMEOUT_SECS).
    const DEFAULT_AGENT_AWAIT_SECS: u64 = 45 * 60;

    let agent_await_timeout = std::env::var("SQUAD_AGENT_AWAIT_TIMEOUT_SECS")
        .ok()
        .and_then(|s| s.parse::<u64>().ok())
        .filter(|&n| n > 0)
        .unwrap_or(DEFAULT_AGENT_AWAIT_SECS);

    #[derive(Clone)]
    struct AgentStepResult {
        status: AgentTaskStatus,
        failure_reason: Option<String>,
        summary: String,
    }

    struct JoinGate {
        expected: usize,
        arrived: AtomicUsize,
        notify: Notify,
        leader_taken: AtomicBool,
    }

    struct SharedWalk {
        agent_task_ids: Mutex<Vec<Uuid>>,
        ordered_node_ids: Mutex<Vec<String>>,
        run_log: Mutex<Vec<String>>,
        step_budget: Mutex<usize>,
        agent_priority: Mutex<i32>,
        last_agent_result: Mutex<Option<AgentStepResult>>,
        join_gates: Mutex<HashMap<String, Arc<JoinGate>>>,
        /// Set when a wait_approval node pauses the walk.
        pause: Mutex<Option<PipelinePause>>,
    }

    struct PipelinePause {
        pause_node_id: String,
        resume_node_id: Option<String>,
        approval_kind: String,
        approval_prompt: String,
    }

    struct WhileFrame {
        node_id: String,
        broke: bool,
    }

    fn edges_for_branch<'a>(
        edges: &[&'a api_types::SquadPipelineEdge],
        want: SquadPipelineEdgeBranch,
    ) -> Vec<&'a api_types::SquadPipelineEdge> {
        let exact: Vec<_> = edges
            .iter()
            .copied()
            .filter(|e| e.branch == Some(want))
            .collect();
        if !exact.is_empty() {
            return exact;
        }
        if matches!(
            want,
            SquadPipelineEdgeBranch::Default | SquadPipelineEdgeBranch::True
        ) {
            return edges
                .iter()
                .copied()
                .filter(|e| matches!(e.branch, None | Some(SquadPipelineEdgeBranch::Default)))
                .collect();
        }
        Vec::new()
    }

    fn eval_condition(
        condition: Option<&str>,
        last: Option<&AgentStepResult>,
        success_condition: Option<&str>,
    ) -> (bool, String) {
        let Some(raw) = condition.map(str::trim).filter(|s| !s.is_empty()) else {
            // Empty if/while: use last agent success, else true.
            if let Some(r) = last {
                let ok = r.status == AgentTaskStatus::Completed;
                return (
                    ok,
                    format!("empty → last agent status={:?} → {ok}", r.status),
                );
            }
            return (true, "empty → true".into());
        };

        let c = raw.to_lowercase();
        if matches!(c.as_str(), "true" | "always" | "yes" | "1" | "ok" | "pass") {
            return (true, format!("literal true ({raw})"));
        }
        if matches!(
            c.as_str(),
            "false" | "never" | "no" | "0" | "fail" | "failed"
        ) {
            return (false, format!("literal false ({raw})"));
        }

        // status:completed / status:failed / status:cancelled
        if let Some(rest) = c.strip_prefix("status:") {
            let want = rest.trim();
            let got = last
                .map(|r| match r.status {
                    AgentTaskStatus::Completed => "completed",
                    AgentTaskStatus::Failed => "failed",
                    AgentTaskStatus::Cancelled => "cancelled",
                    AgentTaskStatus::Queued => "queued",
                    AgentTaskStatus::Dispatched => "dispatched",
                    AgentTaskStatus::Running => "running",
                })
                .unwrap_or("none");
            let ok = got == want;
            return (ok, format!("status:{want} vs last={got} → {ok}"));
        }

        // agent:<needle> or plain needle → match against last summary / status keywords
        let needle = c
            .strip_prefix("agent:")
            .map(str::trim)
            .unwrap_or(c.as_str());

        if needle == "success" || needle == "completed" {
            let ok = last.is_some_and(|r| r.status == AgentTaskStatus::Completed);
            return (ok, format!("success check → {ok}"));
        }
        if needle == "failed" || needle == "failure" || needle == "error" {
            let ok = last.is_some_and(|r| {
                matches!(
                    r.status,
                    AgentTaskStatus::Failed | AgentTaskStatus::Cancelled
                )
            });
            return (ok, format!("failure check → {ok}"));
        }

        if let Some(r) = last {
            let hay = format!(
                "{} {}",
                r.summary.to_lowercase(),
                r.failure_reason.as_deref().unwrap_or("")
            )
            .to_lowercase();
            if hay.contains(needle) {
                return (true, format!("keyword match in last result (`{needle}`)"));
            }
            // success_condition from loop_config as secondary signal
            if let Some(sc) = success_condition.map(str::trim).filter(|s| !s.is_empty()) {
                if hay.contains(&sc.to_lowercase()) {
                    return (true, format!("loop success_condition match (`{sc}`)"));
                }
            }
            if r.status == AgentTaskStatus::Completed
                && !c.contains("never")
                && !c.contains("false")
            {
                // Soft: completed + unknown condition → true (legacy MVP behavior)
                return (
                    true,
                    format!("last completed + no keyword match → soft true (`{raw}`)"),
                );
            }
            if matches!(
                r.status,
                AgentTaskStatus::Failed | AgentTaskStatus::Cancelled
            ) {
                return (false, format!("last failed/cancelled → false (`{raw}`)"));
            }
        }

        if c.contains("never") || c.contains("false") || c.contains("fail") {
            return (false, format!("keyword false ({raw})"));
        }
        (true, format!("default true (no last result for `{raw}`)"))
    }

    async fn await_agent_task(
        pool: &sqlx::PgPool,
        task_id: Uuid,
        timeout: Duration,
    ) -> anyhow::Result<AgentStepResult> {
        let deadline = tokio::time::Instant::now() + timeout;
        let poll = Duration::from_secs(2);
        loop {
            let task = AgentTaskRepository::find_by_id(pool, task_id)
                .await?
                .ok_or_else(|| anyhow::anyhow!("agent task {task_id} disappeared"))?;
            match task.status {
                AgentTaskStatus::Completed
                | AgentTaskStatus::Failed
                | AgentTaskStatus::Cancelled => {
                    let summary = format!(
                        "task {} ended as {:?}{}",
                        task_id,
                        task.status,
                        task.failure_reason
                            .as_ref()
                            .map(|r| format!(" — {r}"))
                            .unwrap_or_default()
                    );
                    return Ok(AgentStepResult {
                        status: task.status,
                        failure_reason: task.failure_reason,
                        summary,
                    });
                }
                _ => {}
            }
            if tokio::time::Instant::now() >= deadline {
                return Ok(AgentStepResult {
                    status: AgentTaskStatus::Failed,
                    failure_reason: Some(format!(
                        "pipeline await timed out after {}s",
                        timeout.as_secs()
                    )),
                    summary: format!("task {task_id} await timeout"),
                });
            }
            tokio::time::sleep(poll).await;
        }
    }

    async fn follow_edges(
        pool: &sqlx::PgPool,
        squad: &Squad,
        issue_id: Uuid,
        preferred_repo: &Option<String>,
        nodes_by_id: &HashMap<&str, &api_types::SquadPipelineNode>,
        outs: &HashMap<&str, Vec<&api_types::SquadPipelineEdge>>,
        ins: &HashMap<&str, Vec<&api_types::SquadPipelineEdge>>,
        shared: &Arc<SharedWalk>,
        edges: Vec<&api_types::SquadPipelineEdge>,
        while_stack: &mut Vec<WhileFrame>,
        visiting: &mut HashSet<String>,
        agent_await_timeout: Duration,
        loop_success: Option<&str>,
        from_node: &str,
    ) -> anyhow::Result<bool> {
        for edge in edges {
            let key = format!("{}->{}", from_node, edge.target);
            if visiting.contains(&key) {
                continue;
            }
            visiting.insert(key);
            if Box::pin(walk_node(
                pool,
                squad,
                issue_id,
                preferred_repo,
                nodes_by_id,
                outs,
                ins,
                shared,
                edge.target.as_str(),
                while_stack,
                visiting,
                agent_await_timeout,
                loop_success,
            ))
            .await?
            {
                return Ok(true);
            }
        }
        Ok(false)
    }

    async fn walk_node(
        pool: &sqlx::PgPool,
        squad: &Squad,
        issue_id: Uuid,
        preferred_repo: &Option<String>,
        nodes_by_id: &HashMap<&str, &api_types::SquadPipelineNode>,
        outs: &HashMap<&str, Vec<&api_types::SquadPipelineEdge>>,
        ins: &HashMap<&str, Vec<&api_types::SquadPipelineEdge>>,
        shared: &Arc<SharedWalk>,
        node_id: &str,
        while_stack: &mut Vec<WhileFrame>,
        visiting: &mut HashSet<String>,
        agent_await_timeout: Duration,
        loop_success: Option<&str>,
    ) -> anyhow::Result<bool> {
        {
            let mut budget = shared.step_budget.lock().await;
            if *budget == 0 {
                shared
                    .run_log
                    .lock()
                    .await
                    .push("⚠ step budget exhausted — stopping walk".into());
                return Ok(false);
            }
            *budget -= 1;
        }

        let node = nodes_by_id
            .get(node_id)
            .ok_or_else(|| anyhow::anyhow!("missing node {node_id}"))?;
        let kind = node.node_type;
        let label = node.label.as_deref().unwrap_or(node_id);
        shared
            .ordered_node_ids
            .lock()
            .await
            .push(node_id.to_string());

        let outgoing = outs.get(node_id).map(|v| v.as_slice()).unwrap_or(&[]);

        match kind {
            SquadPipelineNodeType::Agent => {
                let agent_id = node.agent_id.or(squad.leader_agent_id).ok_or_else(|| {
                    anyhow::anyhow!("步骤「{label}」未指定 Agent，且 Squad 无 Leader")
                })?;
                let is_leader = {
                    let ids = shared.agent_task_ids.lock().await;
                    Some(agent_id) == squad.leader_agent_id && ids.is_empty()
                };
                let priority = {
                    let mut p = shared.agent_priority.lock().await;
                    let cur = *p;
                    *p = p.saturating_sub(1);
                    cur
                };

                let last = shared.last_agent_result.lock().await.clone();
                let mut prompt_parts: Vec<String> = Vec::new();
                if let Some(role) = node.role.as_deref().filter(|s| !s.is_empty()) {
                    prompt_parts.push(format!("## Role\n{role}"));
                }
                if let Some(prompt) = node.prompt.as_deref().filter(|s| !s.is_empty()) {
                    prompt_parts.push(format!("## Step instructions\n{prompt}"));
                }
                if let Some(prev) = last.as_ref() {
                    prompt_parts.push(format!(
                        "## Previous step handoff\nStatus: {:?}\n{}\n{}",
                        prev.status,
                        prev.summary,
                        prev.failure_reason
                            .as_ref()
                            .map(|r| format!("Failure: {r}"))
                            .unwrap_or_default()
                    ));
                }
                let execution_prompt = if prompt_parts.is_empty() {
                    None
                } else {
                    Some(prompt_parts.join("\n\n"))
                };

                let task = AgentTaskRepository::enqueue(
                    pool,
                    None,
                    agent_id,
                    issue_id,
                    AgentTaskTrigger::Manual,
                    priority,
                    true, // distinct session per pipeline step
                    Some(squad.id),
                    is_leader,
                    preferred_repo.clone(),
                    execution_prompt,
                )
                .await?;
                let task_id = task.data.id;
                shared.agent_task_ids.lock().await.push(task_id);
                shared.run_log.lock().await.push(format!(
                    "- **agent** `{label}` → task `{task_id}` (agent `{agent_id}`) — awaiting…"
                ));

                let result = await_agent_task(pool, task_id, agent_await_timeout).await?;
                let ok = result.status == AgentTaskStatus::Completed;
                shared.run_log.lock().await.push(format!(
                    "  - finished {:?}: {}",
                    result.status, result.summary
                ));
                *shared.last_agent_result.lock().await = Some(result.clone());

                if !ok {
                    let err_edges = edges_for_branch(outgoing, SquadPipelineEdgeBranch::Error);
                    if !err_edges.is_empty() {
                        shared
                            .run_log
                            .lock()
                            .await
                            .push("  - following `error` edge(s)".into());
                        return follow_edges(
                            pool,
                            squad,
                            issue_id,
                            preferred_repo,
                            nodes_by_id,
                            outs,
                            ins,
                            shared,
                            err_edges,
                            while_stack,
                            visiting,
                            agent_await_timeout,
                            loop_success,
                            node_id,
                        )
                        .await;
                    }
                    shared.run_log.lock().await.push(
                        "  - agent failed/cancelled and no `error` edge — stopping this branch"
                            .into(),
                    );
                    return Ok(false);
                }

                // Success: follow default outs sequentially (parallel only via Fork).
                let defaults = edges_for_branch(outgoing, SquadPipelineEdgeBranch::Default);
                follow_edges(
                    pool,
                    squad,
                    issue_id,
                    preferred_repo,
                    nodes_by_id,
                    outs,
                    ins,
                    shared,
                    defaults,
                    while_stack,
                    visiting,
                    agent_await_timeout,
                    loop_success,
                    node_id,
                )
                .await
            }
            SquadPipelineNodeType::Wait => {
                let requested = node.wait_seconds.unwrap_or(0).max(0);
                let secs = requested.min(MAX_SYNC_WAIT_SECS) as u64;
                let wait_for = node.wait_for.as_deref().unwrap_or("");
                if secs > 0 {
                    shared.run_log.lock().await.push(format!(
                        "- **wait** `{label}` sleeping {secs}s{}",
                        if wait_for.is_empty() {
                            String::new()
                        } else {
                            format!(" (wait_for: {wait_for})")
                        }
                    ));
                    if requested > MAX_SYNC_WAIT_SECS {
                        shared.run_log.lock().await.push(format!(
                            "  - capped from {requested}s to {MAX_SYNC_WAIT_SECS}s"
                        ));
                    }
                    tokio::time::sleep(Duration::from_secs(secs)).await;
                } else {
                    shared.run_log.lock().await.push(format!(
                        "- **wait** `{label}` recorded{}",
                        if wait_for.is_empty() {
                            String::new()
                        } else {
                            format!(" — {wait_for}")
                        }
                    ));
                }
                let defaults = edges_for_branch(outgoing, SquadPipelineEdgeBranch::Default);
                follow_edges(
                    pool,
                    squad,
                    issue_id,
                    preferred_repo,
                    nodes_by_id,
                    outs,
                    ins,
                    shared,
                    defaults,
                    while_stack,
                    visiting,
                    agent_await_timeout,
                    loop_success,
                    node_id,
                )
                .await
            }
            SquadPipelineNodeType::If => {
                let last = shared.last_agent_result.lock().await.clone();
                let (take_true, reason) =
                    eval_condition(node.condition.as_deref(), last.as_ref(), loop_success);
                let branch = if take_true {
                    SquadPipelineEdgeBranch::True
                } else {
                    SquadPipelineEdgeBranch::False
                };
                shared.run_log.lock().await.push(format!(
                    "- **if** `{label}` → {} ({reason})",
                    branch.as_str()
                ));
                let mut taken = edges_for_branch(outgoing, branch);
                if taken.is_empty() && take_true {
                    taken = edges_for_branch(outgoing, SquadPipelineEdgeBranch::Default);
                }
                if taken.is_empty() {
                    shared
                        .run_log
                        .lock()
                        .await
                        .push(format!("  - no `{}` edge — skipping", branch.as_str()));
                }
                follow_edges(
                    pool,
                    squad,
                    issue_id,
                    preferred_repo,
                    nodes_by_id,
                    outs,
                    ins,
                    shared,
                    taken,
                    while_stack,
                    visiting,
                    agent_await_timeout,
                    loop_success,
                    node_id,
                )
                .await
            }
            SquadPipelineNodeType::While => {
                let max_iter = node.max_iterations.unwrap_or(3).clamp(1, 20) as usize;
                while_stack.push(WhileFrame {
                    node_id: node_id.to_string(),
                    broke: false,
                });
                shared
                    .run_log
                    .lock()
                    .await
                    .push(format!("- **while** `{label}` max_iterations={max_iter}"));
                for iter in 0..max_iter {
                    if while_stack.last().map(|f| f.broke).unwrap_or(false) {
                        break;
                    }
                    let last = shared.last_agent_result.lock().await.clone();
                    // Early exit if loop success_condition already satisfied
                    if let Some(sc) = loop_success.map(str::trim).filter(|s| !s.is_empty()) {
                        if let Some(r) = last.as_ref() {
                            let hay = r.summary.to_lowercase();
                            if r.status == AgentTaskStatus::Completed
                                && hay.contains(&sc.to_lowercase())
                            {
                                shared.run_log.lock().await.push(format!(
                                    "  - iter {}: loop success_condition met — exit",
                                    iter + 1
                                ));
                                break;
                            }
                        }
                    }
                    let (cont, reason) =
                        eval_condition(node.condition.as_deref(), last.as_ref(), loop_success);
                    shared.run_log.lock().await.push(format!(
                        "  - iter {}: condition → {} ({reason})",
                        iter + 1,
                        cont
                    ));
                    if !cont {
                        break;
                    }
                    let body = edges_for_branch(outgoing, SquadPipelineEdgeBranch::Body);
                    if body.is_empty() {
                        shared
                            .run_log
                            .lock()
                            .await
                            .push("  - no `body` edge — ending while".into());
                        break;
                    }
                    for edge in body {
                        let broke = Box::pin(walk_node(
                            pool,
                            squad,
                            issue_id,
                            preferred_repo,
                            nodes_by_id,
                            outs,
                            ins,
                            shared,
                            edge.target.as_str(),
                            while_stack,
                            visiting,
                            agent_await_timeout,
                            loop_success,
                        ))
                        .await?;
                        if broke || while_stack.last().map(|f| f.broke).unwrap_or(false) {
                            break;
                        }
                    }
                }
                while_stack.pop();
                let exit = edges_for_branch(outgoing, SquadPipelineEdgeBranch::Exit);
                let exit = if exit.is_empty() {
                    edges_for_branch(outgoing, SquadPipelineEdgeBranch::Default)
                } else {
                    exit
                };
                follow_edges(
                    pool,
                    squad,
                    issue_id,
                    preferred_repo,
                    nodes_by_id,
                    outs,
                    ins,
                    shared,
                    exit,
                    while_stack,
                    visiting,
                    agent_await_timeout,
                    loop_success,
                    node_id,
                )
                .await
            }
            SquadPipelineNodeType::Break => {
                shared
                    .run_log
                    .lock()
                    .await
                    .push(format!("- **break** `{label}`"));
                if let Some(frame) = while_stack.last_mut() {
                    frame.broke = true;
                    shared
                        .run_log
                        .lock()
                        .await
                        .push(format!("  - exiting while `{}`", frame.node_id));
                    Ok(true)
                } else {
                    shared
                        .run_log
                        .lock()
                        .await
                        .push("  - no active while — ignored".into());
                    Ok(false)
                }
            }
            SquadPipelineNodeType::Fork => {
                let branches = edges_for_branch(outgoing, SquadPipelineEdgeBranch::Default);
                shared.run_log.lock().await.push(format!(
                    "- **fork** `{label}` → {} parallel branch(es)",
                    branches.len()
                ));
                if branches.is_empty() {
                    return Ok(false);
                }
                // Concurrent I/O on the same task (no Send required).
                let mut futs = Vec::with_capacity(branches.len());
                for edge in &branches {
                    let shared = Arc::clone(shared);
                    let target = edge.target.as_str();
                    let from = node_id;
                    futs.push(async move {
                        let mut while_stack = Vec::new();
                        let mut visiting = HashSet::new();
                        visiting.insert(format!("{from}->{target}"));
                        walk_node(
                            pool,
                            squad,
                            issue_id,
                            preferred_repo,
                            nodes_by_id,
                            outs,
                            ins,
                            &shared,
                            target,
                            &mut while_stack,
                            &mut visiting,
                            agent_await_timeout,
                            loop_success,
                        )
                        .await
                    });
                }
                let results = futures::future::join_all(futs).await;
                let mut any_break = false;
                for res in results {
                    match res {
                        Ok(broke) => {
                            if broke {
                                any_break = true;
                            }
                        }
                        Err(e) => {
                            shared
                                .run_log
                                .lock()
                                .await
                                .push(format!("  - fork branch error: {e}"));
                        }
                    }
                }
                Ok(any_break)
            }
            SquadPipelineNodeType::Join => {
                let inbound = ins.get(node_id).map(|v| v.len()).unwrap_or(0).max(1);
                let expected = node
                    .join_count
                    .map(|n| n.clamp(1, inbound as i32) as usize)
                    .unwrap_or(inbound);
                let gate = {
                    let mut gates = shared.join_gates.lock().await;
                    gates
                        .entry(node_id.to_string())
                        .or_insert_with(|| {
                            Arc::new(JoinGate {
                                expected,
                                arrived: AtomicUsize::new(0),
                                notify: Notify::new(),
                                leader_taken: AtomicBool::new(false),
                            })
                        })
                        .clone()
                };
                let n = gate.arrived.fetch_add(1, Ordering::SeqCst) + 1;
                shared
                    .run_log
                    .lock()
                    .await
                    .push(format!("- **join** `{label}` arrival {n}/{expected}"));
                if n >= gate.expected {
                    gate.notify.notify_waiters();
                }
                while gate.arrived.load(Ordering::SeqCst) < gate.expected {
                    gate.notify.notified().await;
                }
                // Only one branch continues past the join.
                if gate.leader_taken.swap(true, Ordering::SeqCst) {
                    shared
                        .run_log
                        .lock()
                        .await
                        .push(format!("  - join `{label}` non-leader — done"));
                    return Ok(false);
                }
                shared
                    .run_log
                    .lock()
                    .await
                    .push(format!("  - join `{label}` barrier released — continue"));
                let defaults = edges_for_branch(outgoing, SquadPipelineEdgeBranch::Default);
                follow_edges(
                    pool,
                    squad,
                    issue_id,
                    preferred_repo,
                    nodes_by_id,
                    outs,
                    ins,
                    shared,
                    defaults,
                    while_stack,
                    visiting,
                    agent_await_timeout,
                    loop_success,
                    node_id,
                )
                .await
            }
            SquadPipelineNodeType::WaitApproval => {
                let kind = node
                    .approval_kind
                    .clone()
                    .or_else(|| node.wait_for.clone())
                    .unwrap_or_else(|| "approval".into());
                let prompt = node
                    .prompt_template
                    .clone()
                    .or_else(|| node.prompt.clone())
                    .unwrap_or_else(|| {
                        format!("流水线在步骤「{label}」等待你的确认。Approve 继续，Reject 取消。")
                    });
                let resume = edges_for_branch(outgoing, SquadPipelineEdgeBranch::Default)
                    .first()
                    .map(|e| e.target.clone());
                shared.run_log.lock().await.push(format!(
                    "- **wait_approval** `{label}` kind={kind} resume={}",
                    resume.as_deref().unwrap_or("(end)")
                ));
                *shared.pause.lock().await = Some(PipelinePause {
                    pause_node_id: node_id.to_string(),
                    resume_node_id: resume,
                    approval_kind: kind,
                    approval_prompt: prompt,
                });
                // Stop this branch; caller persists waiting_approval and returns.
                Ok(false)
            }
            SquadPipelineNodeType::Script | SquadPipelineNodeType::GitOp => {
                let agent_id = node.agent_id.or(squad.leader_agent_id).ok_or_else(|| {
                    anyhow::anyhow!(
                        "步骤「{label}」需要 Agent（或 Squad Leader）以挂靠 script/git_op 任务"
                    )
                })?;
                let priority = {
                    let mut p = shared.agent_priority.lock().await;
                    let cur = *p;
                    *p = p.saturating_sub(1);
                    cur
                };
                let local_ws =
                    AgentTaskRepository::latest_local_workspace_for_issue(pool, issue_id)
                        .await
                        .ok()
                        .flatten();
                let job = match node.node_type {
                    SquadPipelineNodeType::Script => {
                        let command = node
                            .command
                            .clone()
                            .or_else(|| node.prompt.clone())
                            .filter(|s| !s.trim().is_empty())
                            .ok_or_else(|| anyhow::anyhow!("script 节点「{label}」缺少 command"))?;
                        api_types::PipelineJobSpec {
                            kind: api_types::PipelineJobKind::Script,
                            command: Some(command),
                            op: None,
                            target_branch: None,
                            local_workspace_id: local_ws,
                            label: Some(label.to_string()),
                        }
                    }
                    _ => {
                        let op = node
                            .git_op
                            .clone()
                            .or_else(|| node.prompt.clone())
                            .unwrap_or_else(|| "rebase".into());
                        api_types::PipelineJobSpec {
                            kind: api_types::PipelineJobKind::GitOp,
                            command: None,
                            op: Some(op),
                            target_branch: node.wait_for.clone().or_else(|| Some("main".into())),
                            local_workspace_id: local_ws,
                            label: Some(label.to_string()),
                        }
                    }
                };
                let execution_prompt = job.encode().map_err(|e| anyhow::anyhow!(e))?;
                let kind = node.node_type.as_str();
                let task = AgentTaskRepository::enqueue(
                    pool,
                    None,
                    agent_id,
                    issue_id,
                    AgentTaskTrigger::Manual,
                    priority,
                    true,
                    Some(squad.id),
                    false,
                    preferred_repo.clone(),
                    Some(execution_prompt),
                )
                .await?;
                let task_id = task.data.id;
                shared.agent_task_ids.lock().await.push(task_id);
                shared
                    .ordered_node_ids
                    .lock()
                    .await
                    .push(node_id.to_string());
                shared.run_log.lock().await.push(format!(
                    "- **{kind}** `{label}` → task `{task_id}` — awaiting local watcher…"
                ));

                let result = await_agent_task(pool, task_id, agent_await_timeout).await?;
                let ok = result.status == AgentTaskStatus::Completed;
                shared.run_log.lock().await.push(format!(
                    "  - finished {:?}: {}",
                    result.status, result.summary
                ));
                *shared.last_agent_result.lock().await = Some(result.clone());

                if !ok {
                    let err_edges = edges_for_branch(outgoing, SquadPipelineEdgeBranch::Error);
                    if !err_edges.is_empty() {
                        shared
                            .run_log
                            .lock()
                            .await
                            .push("  - following `error` edge(s)".into());
                        return follow_edges(
                            pool,
                            squad,
                            issue_id,
                            preferred_repo,
                            nodes_by_id,
                            outs,
                            ins,
                            shared,
                            err_edges,
                            while_stack,
                            visiting,
                            agent_await_timeout,
                            loop_success,
                            node_id,
                        )
                        .await;
                    }
                    shared.run_log.lock().await.push(
                        "  - script/git_op failed and no `error` edge — stopping this branch"
                            .into(),
                    );
                    return Ok(false);
                }

                let defaults = edges_for_branch(outgoing, SquadPipelineEdgeBranch::Default);
                follow_edges(
                    pool,
                    squad,
                    issue_id,
                    preferred_repo,
                    nodes_by_id,
                    outs,
                    ins,
                    shared,
                    defaults,
                    while_stack,
                    visiting,
                    agent_await_timeout,
                    loop_success,
                    node_id,
                )
                .await
            }
        }
    }

    let preferred_repo = working_directory.clone();
    let shared = Arc::new(SharedWalk {
        agent_task_ids: Mutex::new(Vec::new()),
        ordered_node_ids: Mutex::new(Vec::new()),
        run_log: Mutex::new(Vec::new()),
        step_budget: Mutex::new(256),
        agent_priority: Mutex::new(100),
        last_agent_result: Mutex::new(None),
        join_gates: Mutex::new(HashMap::new()),
        pause: Mutex::new(None),
    });

    if let Some(ref start_id) = overrides.start_from_node_id {
        shared.run_log.lock().await.push(format!(
            "- starting from node `{start_id}` (upstream skipped)"
        ));
    }

    let loop_success = loop_config
        .as_ref()
        .and_then(|lc| lc.success_condition.as_deref());
    let timeout = Duration::from_secs(agent_await_timeout);

    let mut while_stack: Vec<WhileFrame> = Vec::new();
    let mut visiting: HashSet<String> = HashSet::new();
    for root in &roots {
        Box::pin(walk_node(
            pool,
            squad,
            issue_id,
            &preferred_repo,
            &nodes_by_id,
            &outs,
            &ins,
            &shared,
            root,
            &mut while_stack,
            &mut visiting,
            timeout,
            loop_success,
        ))
        .await?;
    }

    // Fallback: agent-only pipeline with no walk progress
    {
        let ids = shared.agent_task_ids.lock().await;
        let ordered = shared.ordered_node_ids.lock().await;
        if ids.is_empty()
            && squad
                .pipeline
                .nodes
                .iter()
                .any(|n| n.node_type == SquadPipelineNodeType::Agent)
            && ordered.is_empty()
        {
            drop(ids);
            drop(ordered);
            let ordered = topological_order(&squad.pipeline);
            for (i, node_id) in ordered.iter().enumerate() {
                let node = nodes_by_id
                    .get(node_id.as_str())
                    .ok_or_else(|| anyhow::anyhow!("missing node {node_id}"))?;
                if node.node_type != SquadPipelineNodeType::Agent {
                    continue;
                }
                let agent_id = node.agent_id.or(squad.leader_agent_id).ok_or_else(|| {
                    anyhow::anyhow!("步骤「{node_id}」未指定 Agent，且 Squad 无 Leader")
                })?;
                let priority = (ordered.len() - i) as i32;
                let mut prompt_parts = Vec::new();
                if let Some(role) = node.role.as_deref().filter(|s| !s.is_empty()) {
                    prompt_parts.push(format!("## Role\n{role}"));
                }
                if let Some(prompt) = node.prompt.as_deref().filter(|s| !s.is_empty()) {
                    prompt_parts.push(format!("## Step instructions\n{prompt}"));
                }
                let execution_prompt = if prompt_parts.is_empty() {
                    None
                } else {
                    Some(prompt_parts.join("\n\n"))
                };
                let task = AgentTaskRepository::enqueue(
                    pool,
                    None,
                    agent_id,
                    issue_id,
                    AgentTaskTrigger::Manual,
                    priority,
                    true,
                    Some(squad.id),
                    Some(agent_id) == squad.leader_agent_id && i == 0,
                    preferred_repo.clone(),
                    execution_prompt,
                )
                .await?;
                shared.agent_task_ids.lock().await.push(task.data.id);
                shared.ordered_node_ids.lock().await.push(node_id.clone());
                let result = await_agent_task(pool, task.data.id, timeout).await?;
                *shared.last_agent_result.lock().await = Some(result);
            }
        }
    }

    let agent_task_ids = shared.agent_task_ids.lock().await.clone();
    let ordered_node_ids = shared.ordered_node_ids.lock().await.clone();
    let run_log = shared.run_log.lock().await.clone();
    let pause = shared.pause.lock().await.take();

    let mut plan = String::from("## Squad pipeline run\n\n");
    plan.push_str(&format!("Squad: **{}**\n", squad.name));
    plan.push_str(&format!("Target: `{}`\n", target_type.as_str()));
    if let Some(ref wd) = working_directory {
        plan.push_str(&format!("Working directory: `{wd}`\n"));
    }
    if let Some(ref start_id) = overrides.start_from_node_id {
        plan.push_str(&format!("Started from node: `{start_id}`\n"));
    }
    plan.push_str(&format!("Agent await timeout: {}s\n", agent_await_timeout));
    plan.push_str("\n### Execution trace\n\n");
    if run_log.is_empty() {
        plan.push_str("(no steps executed)\n");
    } else {
        for line in &run_log {
            plan.push_str(line);
            plan.push('\n');
        }
    }
    plan.push_str(&loop_note);
    plan.push_str(
        "\n\n_Orchestrator: agent nodes enqueue + await terminal status; Fork fans out \
         concurrently; Join barriers on inbound count (or join_count); if/while use literals, \
         status:, agent:keyword / last-result matching; failed agents follow optional `error` \
         edge else stop branch. Parallel only via Fork. Mid-entry via start_from_node_id; \
         wait_approval pauses for human Approve/Reject._\n",
    );

    let now = Utc::now();
    let _ = sqlx::query!(
        r#"
        INSERT INTO issue_comments (id, issue_id, author_id, parent_id, message, created_at, updated_at)
        VALUES ($1, $2, NULL, NULL, $3, $4, $5)
        "#,
        Uuid::new_v4(),
        issue_id,
        plan,
        now,
        now
    )
    .execute(pool)
    .await;

    let (status, pause_node_id, resume_node_id) = if let Some(ref p) = pause {
        (
            SquadRunStatus::WaitingApproval,
            Some(p.pause_node_id.clone()),
            p.resume_node_id.clone(),
        )
    } else {
        (SquadRunStatus::Completed, None, None)
    };

    let run = if let Some(resume_id) = overrides.resume_run_id {
        if let Some(p) = &pause {
            SquadRunRepository::mark_waiting_approval(
                pool,
                resume_id,
                p.pause_node_id.clone(),
                p.resume_node_id.clone(),
                p.approval_kind.clone(),
                p.approval_prompt.clone(),
            )
            .await
            .ok()
        } else {
            SquadRunRepository::mark_completed(pool, resume_id, &agent_task_ids, &ordered_node_ids)
                .await
                .ok()
        }
    } else {
        let created = SquadRunRepository::create(
            pool,
            squad.id,
            issue_id,
            if pause.is_some() {
                SquadRunStatus::WaitingApproval
            } else {
                SquadRunStatus::Running
            },
            overrides.start_from_node_id.clone(),
            working_directory.clone(),
            actor_user_id,
        )
        .await
        .ok();

        if let Some(run) = created {
            if let Some(p) = &pause {
                SquadRunRepository::mark_waiting_approval(
                    pool,
                    run.id,
                    p.pause_node_id.clone(),
                    p.resume_node_id.clone(),
                    p.approval_kind.clone(),
                    p.approval_prompt.clone(),
                )
                .await
                .ok()
            } else {
                SquadRunRepository::mark_completed(pool, run.id, &agent_task_ids, &ordered_node_ids)
                    .await
                    .ok()
            }
        } else {
            None
        }
    };

    if let Some(p) = &pause {
        let recipients: Vec<Uuid> = if let Some(uid) = actor_user_id {
            vec![uid]
        } else {
            sqlx::query_scalar::<_, Uuid>(
                r#"
                SELECT DISTINCT user_id FROM issue_subscribers
                WHERE issue_id = $1 AND user_id IS NOT NULL
                "#,
            )
            .bind(issue_id)
            .fetch_all(pool)
            .await
            .unwrap_or_default()
        };

        let run_id = run.as_ref().map(|r| r.id);
        for uid in recipients {
            let _ = InboxRepository::create(
                pool,
                uid,
                Some(squad.project_id),
                Some(issue_id),
                "workflow_approval",
                &format!("待确认：{}", squad.name),
                &p.approval_prompt,
                json!({
                    "squad_run_id": run_id,
                    "squad_id": squad.id,
                    "pause_node_id": p.pause_node_id,
                    "approval_kind": p.approval_kind,
                }),
            )
            .await;
        }
    }

    Ok(RunSquadResponse {
        issue_id,
        agent_task_ids,
        ordered_node_ids,
        target_type,
        working_directory,
        run_id: run.as_ref().map(|r| r.id),
        status: Some(status),
        pause_node_id,
        resume_node_id,
    })
}

async fn create_path_run_issue(
    pool: &sqlx::PgPool,
    squad: &Squad,
    working_directory: Option<&str>,
) -> anyhow::Result<Uuid> {
    let status_id: (Uuid,) = sqlx::query_as(
        "SELECT id FROM project_statuses WHERE project_id = $1 ORDER BY sort_order ASC LIMIT 1",
    )
    .bind(squad.project_id)
    .fetch_one(pool)
    .await?;

    let sort_order: f64 = sqlx::query_scalar(
        "SELECT COALESCE(MAX(sort_order), 0) + 1.0 FROM issues WHERE project_id = $1",
    )
    .bind(squad.project_id)
    .fetch_one(pool)
    .await
    .unwrap_or(1.0);

    let date_str = Utc::now().format("%Y-%m-%d %H:%M").to_string();
    let title = format!("Squad run: {} — {}", squad.name, date_str);
    let description = format!(
        "Automated squad pipeline run against directory.\n\nWorking directory: `{}`",
        working_directory.unwrap_or("(unset)")
    );

    let issue_id = Uuid::new_v4();
    sqlx::query!(
        r#"
        INSERT INTO issues (
            id, project_id, status_id, title, description,
            sort_order, extension_metadata
        )
        VALUES ($1, $2, $3, $4, $5, $6, '{}')
        "#,
        issue_id,
        squad.project_id,
        status_id.0,
        title,
        description,
        sort_order
    )
    .execute(pool)
    .await?;

    // Assign squad to the issue for visibility
    let _ = sqlx::query!(
        r#"
        INSERT INTO issue_assignees (id, issue_id, user_id, agent_id, squad_id)
        VALUES ($1, $2, NULL, NULL, $3)
        "#,
        Uuid::new_v4(),
        issue_id,
        squad.id
    )
    .execute(pool)
    .await;

    Ok(issue_id)
}
