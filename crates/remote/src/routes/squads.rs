use api_types::{
    AddSquadMemberRequest, AgentTaskTrigger, CreateSquadRequest, DeleteResponse,
    ListSquadMembersResponse, ListSquadsQuery, ListSquadsResponse, MutationResponse,
    RunSquadRequest, RunSquadResponse, Squad, SquadMember, SquadTargetType, UpdateSquadRequest,
};
use axum::{
    Json, Router,
    extract::{Extension, Path, Query, State},
    http::StatusCode,
    routing::{delete, get, post},
};
use chrono::Utc;
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

    let result = execute_squad_pipeline(state.pool(), &squad, &overrides)
        .await
        .map_err(|e| {
            tracing::error!(?e, squad_id = %id, "failed to run squad pipeline");
            ErrorResponse::new(StatusCode::BAD_REQUEST, e.to_string())
        })?;

    Ok(Json(result))
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
pub async fn execute_squad_pipeline(
    pool: &sqlx::PgPool,
    squad: &Squad,
    overrides: &RunSquadRequest,
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
        local_workspace_id: Option<Uuid>,
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
        last_workspace_id: Mutex<Option<Uuid>>,
        join_gates: Mutex<HashMap<String, Arc<JoinGate>>>,
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

        // Feature Babysitter: verdict:ready / verdict:needs_work
        if let Some(rest) = c.strip_prefix("verdict:") {
            let want = rest.trim();
            let hay = last
                .map(|r| {
                    format!(
                        "{} {}",
                        r.summary.to_lowercase(),
                        r.failure_reason.as_deref().unwrap_or("")
                    )
                    .to_lowercase()
                })
                .unwrap_or_default();
            let ok = match want {
                "ready" | "ok" | "pass" => {
                    hay.contains("babysitter_verdict: ready")
                        || hay.contains("babysitter_verdict:ready")
                }
                "needs_work" | "needs-work" | "fix" | "fail" => {
                    hay.contains("babysitter_verdict: needs_work")
                        || hay.contains("babysitter_verdict:needs_work")
                        || hay.contains("babysitter_verdict: needs work")
                }
                _ => hay.contains(&format!("babysitter_verdict: {want}")),
            };
            return (ok, format!("verdict:{want} → {ok}"));
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
        issue_id: Uuid,
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
                    let mut comment_bits = String::new();
                    if let Ok(comments) =
                        crate::db::issue_comments::IssueCommentRepository::list_by_issue(
                            pool, issue_id,
                        )
                        .await
                    {
                        // Newest comments first — look for babysitter verdict markers.
                        let mut newest = comments;
                        newest.sort_by(|a, b| b.created_at.cmp(&a.created_at));
                        for c in newest.into_iter().take(8) {
                            comment_bits.push(' ');
                            comment_bits.push_str(&c.message);
                        }
                    }
                    let summary = format!(
                        "task {} ended as {:?}{}{}",
                        task_id,
                        task.status,
                        task.failure_reason
                            .as_ref()
                            .map(|r| format!(" — {r}"))
                            .unwrap_or_default(),
                        comment_bits
                    );
                    return Ok(AgentStepResult {
                        status: task.status,
                        failure_reason: task.failure_reason,
                        summary,
                        local_workspace_id: task.local_workspace_id,
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
                    local_workspace_id: None,
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

                let result = await_agent_task(pool, task_id, issue_id, agent_await_timeout).await?;
                let ok = result.status == AgentTaskStatus::Completed;
                if let Some(ws) = result.local_workspace_id {
                    *shared.last_workspace_id.lock().await = Some(ws);
                }
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
            SquadPipelineNodeType::Rebase => {
                let workspace_id = shared.last_workspace_id.lock().await.clone().or_else(|| {
                    // Fall back: last completed task on this issue with a workspace.
                    None
                });
                let workspace_id = if let Some(ws) = workspace_id {
                    Some(ws)
                } else {
                    // Query DB for most recent task with workspace.
                    match AgentTaskRepository::list_by_issue(pool, issue_id).await {
                        Ok(tasks) => tasks
                            .into_iter()
                            .filter(|t| t.local_workspace_id.is_some())
                            .max_by_key(|t| t.updated_at)
                            .and_then(|t| t.local_workspace_id),
                        Err(_) => None,
                    }
                };
                let Some(workspace_id) = workspace_id else {
                    shared.run_log.lock().await.push(format!(
                        "- **rebase** `{label}` skipped — no workspace from prior agent step"
                    ));
                    *shared.last_agent_result.lock().await = Some(AgentStepResult {
                        status: AgentTaskStatus::Failed,
                        failure_reason: Some("rebase: no local workspace".into()),
                        summary: "BABYSITTER rebase failed: no workspace".into(),
                        local_workspace_id: None,
                    });
                    return Ok(false);
                };
                *shared.last_workspace_id.lock().await = Some(workspace_id);

                let agent_id = node.agent_id.or(squad.leader_agent_id).ok_or_else(|| {
                    anyhow::anyhow!("rebase 步骤需要 Leader Agent（系统任务认领用）")
                })?;
                let priority = {
                    let mut p = shared.agent_priority.lock().await;
                    let cur = *p;
                    *p = p.saturating_sub(1);
                    cur
                };
                let execution_prompt = Some(format!(
                    "__VK_SYSTEM_ACTION__:rebase\nworkspace_id:{workspace_id}\n## Step instructions\nRebase workspace onto its target branch(es). Do not write code."
                ));
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
                    execution_prompt,
                )
                .await?;
                let task_id = task.data.id;
                shared.agent_task_ids.lock().await.push(task_id);
                shared.run_log.lock().await.push(format!(
                    "- **rebase** `{label}` → system task `{task_id}` workspace `{workspace_id}` — awaiting…"
                ));
                let result = await_agent_task(pool, task_id, issue_id, agent_await_timeout).await?;
                let ok = result.status == AgentTaskStatus::Completed;
                shared.run_log.lock().await.push(format!(
                    "  - rebase finished {:?}: {}",
                    result.status, result.summary
                ));
                *shared.last_agent_result.lock().await = Some(result.clone());
                if !ok {
                    let err_edges = edges_for_branch(outgoing, SquadPipelineEdgeBranch::Error);
                    if !err_edges.is_empty() {
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
            SquadPipelineNodeType::HumanGate => {
                let gate_kind = node
                    .gate_kind
                    .as_deref()
                    .filter(|s| !s.is_empty())
                    .unwrap_or("merge_approval");
                let question = node
                    .prompt
                    .as_deref()
                    .or(node.wait_for.as_deref())
                    .filter(|s| !s.is_empty())
                    .unwrap_or("Feature 已完成验收与 rebase，是否合并到目标分支？");
                let workspace_id = shared.last_workspace_id.lock().await.clone();

                let payload = serde_json::json!({
                    "gate_kind": gate_kind,
                    "squad_id": squad.id,
                    "local_workspace_id": workspace_id,
                });
                let gate = crate::db::pipeline_gates::PipelineGateRepository::create(
                    pool,
                    squad.project_id,
                    issue_id,
                    Some(squad.id),
                    gate_kind,
                    workspace_id,
                    question,
                    payload.clone(),
                )
                .await?;

                // Collect inbox recipients (issue creator + human assignees + subscribers).
                let mut recipients = std::collections::HashSet::new();
                if let Ok(Some(issue)) =
                    crate::db::issues::IssueRepository::find_by_id(pool, issue_id).await
                {
                    if let Some(creator) = issue.creator_user_id {
                        recipients.insert(creator);
                    }
                }
                if let Ok(assignees) =
                    crate::db::issue_assignees::IssueAssigneeRepository::list_by_issue(
                        pool, issue_id,
                    )
                    .await
                {
                    for a in assignees {
                        if let Some(uid) = a.user_id {
                            recipients.insert(uid);
                        }
                    }
                }
                if let Ok(rows) = sqlx::query_scalar::<_, Uuid>(
                    "SELECT user_id FROM issue_subscribers WHERE issue_id = $1",
                )
                .bind(issue_id)
                .fetch_all(pool)
                .await
                {
                    recipients.extend(rows);
                }

                let title = if gate_kind == "merge_approval" {
                    "Feature 待合并确认".to_string()
                } else {
                    format!("需要确认：{label}")
                };
                let body = format!("{question}\n\n(gate `{gate_id}`)", gate_id = gate.id);
                let inbox_payload = serde_json::json!({
                    "gate_id": gate.id,
                    "gate_kind": gate_kind,
                    "squad_id": squad.id,
                    "local_workspace_id": workspace_id,
                    "issue_id": issue_id,
                });
                for user_id in &recipients {
                    let _ = crate::db::inbox::InboxRepository::create(
                        pool,
                        *user_id,
                        Some(squad.project_id),
                        Some(issue_id),
                        if gate_kind == "merge_approval" {
                            "merge_approval"
                        } else {
                            "human_gate"
                        },
                        &title,
                        &body,
                        inbox_payload.clone(),
                    )
                    .await;
                }
                if recipients.is_empty() {
                    shared.run_log.lock().await.push(format!(
                        "- **human_gate** `{label}` warning: no inbox recipients"
                    ));
                }

                shared.run_log.lock().await.push(format!(
                    "- **human_gate** `{label}` kind=`{gate_kind}` gate=`{}` — awaiting human…",
                    gate.id
                ));

                let gate_timeout = std::env::var("HUMAN_GATE_TIMEOUT_SECS")
                    .ok()
                    .and_then(|s| s.parse::<u64>().ok())
                    .filter(|&n| n > 0)
                    .unwrap_or(24 * 60 * 60);
                let deadline = tokio::time::Instant::now() + Duration::from_secs(gate_timeout);
                let poll = Duration::from_secs(5);
                let final_status = loop {
                    let current = crate::db::pipeline_gates::PipelineGateRepository::find_by_id(
                        pool, gate.id,
                    )
                    .await?
                    .ok_or_else(|| anyhow::anyhow!("human gate disappeared"))?;
                    if current.status != "pending" {
                        break current.status;
                    }
                    if tokio::time::Instant::now() >= deadline {
                        let _ = crate::db::pipeline_gates::PipelineGateRepository::expire(
                            pool, gate.id,
                        )
                        .await;
                        break "expired".to_string();
                    }
                    tokio::time::sleep(poll).await;
                };

                let approved = final_status == "approved";
                shared
                    .run_log
                    .lock()
                    .await
                    .push(format!("  - human_gate decided: {final_status}"));
                *shared.last_agent_result.lock().await = Some(AgentStepResult {
                    status: if approved {
                        AgentTaskStatus::Completed
                    } else {
                        AgentTaskStatus::Failed
                    },
                    failure_reason: if approved {
                        None
                    } else {
                        Some(format!("human_gate:{final_status}"))
                    },
                    summary: format!("human_gate {final_status}"),
                    local_workspace_id: workspace_id,
                });

                if approved {
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
                } else {
                    let false_edges = edges_for_branch(outgoing, SquadPipelineEdgeBranch::False);
                    if !false_edges.is_empty() {
                        follow_edges(
                            pool,
                            squad,
                            issue_id,
                            preferred_repo,
                            nodes_by_id,
                            outs,
                            ins,
                            shared,
                            false_edges,
                            while_stack,
                            visiting,
                            agent_await_timeout,
                            loop_success,
                            node_id,
                        )
                        .await
                    } else {
                        Ok(false)
                    }
                }
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
        last_workspace_id: Mutex::new(None),
        join_gates: Mutex::new(HashMap::new()),
    });

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
                let result = await_agent_task(pool, task.data.id, issue_id, timeout).await?;
                if let Some(ws) = result.local_workspace_id {
                    *shared.last_workspace_id.lock().await = Some(ws);
                }
                *shared.last_agent_result.lock().await = Some(result);
            }
        }
    }

    let agent_task_ids = shared.agent_task_ids.lock().await.clone();
    let ordered_node_ids = shared.ordered_node_ids.lock().await.clone();
    let run_log = shared.run_log.lock().await.clone();

    let mut plan = String::from("## Squad pipeline run\n\n");
    plan.push_str(&format!("Squad: **{}**\n", squad.name));
    plan.push_str(&format!("Target: `{}`\n", target_type.as_str()));
    if let Some(ref wd) = working_directory {
        plan.push_str(&format!("Working directory: `{wd}`\n"));
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
         edge else stop branch. Parallel only via Fork._\n",
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

    Ok(RunSquadResponse {
        issue_id,
        agent_task_ids,
        ordered_node_ids,
        target_type,
        working_directory,
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
