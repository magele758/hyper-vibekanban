use api_types::{
    AgentTask, AgentTaskTrigger, ClaimAgentTaskRequest, ClaimAgentTaskResponse,
    CreateAgentTaskRequest, DeleteResponse, ListAgentTasksQuery, ListAgentTasksResponse,
    MutationResponse, UpdateAgentTaskRequest,
};
use axum::{
    Json, Router,
    extract::{Extension, Path, Query, State},
    http::StatusCode,
    routing::post,
};
use tracing::instrument;
use uuid::Uuid;

use super::{
    error::{ErrorResponse, db_error},
    organization_members::{ensure_issue_access, ensure_project_access},
};
use crate::{
    AppState,
    auth::RequestContext,
    db::{
        agent_tasks::AgentTaskRepository, agents::AgentRepository, inbox::InboxRepository,
        issue_assignees::IssueAssigneeRepository, issues::IssueRepository,
    },
    mutation_definition::MutationBuilder,
};

pub fn mutation() -> MutationBuilder<AgentTask, CreateAgentTaskRequest, UpdateAgentTaskRequest> {
    MutationBuilder::new("agent_tasks")
        .list(list_agent_tasks)
        .get(get_agent_task)
        .create(create_agent_task)
        .update(update_agent_task)
        .delete(delete_agent_task)
}

pub fn router() -> Router<AppState> {
    mutation()
        .router()
        .route("/agent_tasks/claim", post(claim_agent_task))
}

#[instrument(
    name = "agent_tasks.list_agent_tasks",
    skip(state, ctx),
    fields(user_id = %ctx.user.id)
)]
async fn list_agent_tasks(
    State(state): State<AppState>,
    Extension(ctx): Extension<RequestContext>,
    Query(query): Query<ListAgentTasksQuery>,
) -> Result<Json<ListAgentTasksResponse>, ErrorResponse> {
    let agent_tasks = if let Some(project_id) = query.project_id {
        ensure_project_access(state.pool(), ctx.user.id, project_id).await?;
        AgentTaskRepository::list_by_project(state.pool(), project_id)
            .await
            .map_err(|error| {
                tracing::error!(?error, %project_id, "failed to list agent tasks");
                ErrorResponse::new(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "failed to list agent tasks",
                )
            })?
    } else if let Some(issue_id) = query.issue_id {
        ensure_issue_access(state.pool(), ctx.user.id, issue_id).await?;
        AgentTaskRepository::list_by_issue(state.pool(), issue_id)
            .await
            .map_err(|error| {
                tracing::error!(?error, %issue_id, "failed to list agent tasks");
                ErrorResponse::new(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "failed to list agent tasks",
                )
            })?
    } else {
        return Err(ErrorResponse::new(
            StatusCode::BAD_REQUEST,
            "project_id or issue_id is required",
        ));
    };

    let agent_tasks = if let Some(agent_id) = query.agent_id {
        agent_tasks
            .into_iter()
            .filter(|t| t.agent_id == agent_id)
            .collect()
    } else {
        agent_tasks
    };

    let agent_tasks = if let Some(status) = query.status {
        agent_tasks
            .into_iter()
            .filter(|t| t.status == status)
            .collect()
    } else {
        agent_tasks
    };

    Ok(Json(ListAgentTasksResponse { agent_tasks }))
}

#[instrument(
    name = "agent_tasks.get_agent_task",
    skip(state, ctx),
    fields(agent_task_id = %agent_task_id, user_id = %ctx.user.id)
)]
async fn get_agent_task(
    State(state): State<AppState>,
    Extension(ctx): Extension<RequestContext>,
    Path(agent_task_id): Path<Uuid>,
) -> Result<Json<AgentTask>, ErrorResponse> {
    let task = AgentTaskRepository::find_by_id(state.pool(), agent_task_id)
        .await
        .map_err(|error| {
            tracing::error!(?error, %agent_task_id, "failed to load agent task");
            ErrorResponse::new(
                StatusCode::INTERNAL_SERVER_ERROR,
                "failed to load agent task",
            )
        })?
        .ok_or_else(|| ErrorResponse::new(StatusCode::NOT_FOUND, "agent task not found"))?;

    ensure_issue_access(state.pool(), ctx.user.id, task.issue_id).await?;

    Ok(Json(task))
}

#[instrument(
    name = "agent_tasks.create_agent_task",
    skip(state, ctx, payload),
    fields(user_id = %ctx.user.id)
)]
async fn create_agent_task(
    State(state): State<AppState>,
    Extension(ctx): Extension<RequestContext>,
    Json(payload): Json<CreateAgentTaskRequest>,
) -> Result<Json<MutationResponse<AgentTask>>, ErrorResponse> {
    ensure_issue_access(state.pool(), ctx.user.id, payload.issue_id).await?;

    let agent = AgentRepository::find_by_id(state.pool(), payload.agent_id)
        .await
        .map_err(|error| {
            tracing::error!(?error, "failed to load agent");
            ErrorResponse::new(StatusCode::INTERNAL_SERVER_ERROR, "failed to load agent")
        })?
        .ok_or_else(|| ErrorResponse::new(StatusCode::NOT_FOUND, "agent not found"))?;

    let issue = IssueRepository::find_by_id(state.pool(), payload.issue_id)
        .await
        .map_err(|error| {
            tracing::error!(?error, "failed to load issue");
            ErrorResponse::new(StatusCode::INTERNAL_SERVER_ERROR, "failed to load issue")
        })?
        .ok_or_else(|| ErrorResponse::new(StatusCode::NOT_FOUND, "issue not found"))?;

    if agent.project_id != issue.project_id {
        return Err(ErrorResponse::new(
            StatusCode::BAD_REQUEST,
            "agent and issue must belong to the same project",
        ));
    }

    let response = AgentTaskRepository::enqueue(
        state.pool(),
        payload.id,
        payload.agent_id,
        payload.issue_id,
        payload.trigger.unwrap_or(AgentTaskTrigger::Manual),
        payload.priority.unwrap_or(0),
        payload.force_fresh_session.unwrap_or(false),
        payload.squad_id,
        payload.is_leader_task.unwrap_or(false),
        payload.preferred_repo_id,
    )
    .await
    .map_err(|error| {
        tracing::error!(?error, "failed to enqueue agent task");
        db_error(error, "failed to enqueue agent task")
    })?;

    Ok(Json(response))
}

#[instrument(
    name = "agent_tasks.update_agent_task",
    skip(state, ctx, payload),
    fields(agent_task_id = %agent_task_id, user_id = %ctx.user.id)
)]
async fn update_agent_task(
    State(state): State<AppState>,
    Extension(ctx): Extension<RequestContext>,
    Path(agent_task_id): Path<Uuid>,
    Json(payload): Json<UpdateAgentTaskRequest>,
) -> Result<Json<MutationResponse<AgentTask>>, ErrorResponse> {
    let existing = AgentTaskRepository::find_by_id(state.pool(), agent_task_id)
        .await
        .map_err(|error| {
            tracing::error!(?error, %agent_task_id, "failed to load agent task");
            ErrorResponse::new(
                StatusCode::INTERNAL_SERVER_ERROR,
                "failed to load agent task",
            )
        })?
        .ok_or_else(|| ErrorResponse::new(StatusCode::NOT_FOUND, "agent task not found"))?;

    ensure_issue_access(state.pool(), ctx.user.id, existing.issue_id).await?;

    let response = AgentTaskRepository::update(
        state.pool(),
        agent_task_id,
        payload.status,
        payload.failure_reason,
        payload.local_workspace_id,
        payload.local_session_id,
        payload.claimed_by_host,
        payload.attempt,
    )
    .await
    .map_err(|error| {
        tracing::error!(?error, "failed to update agent task");
        db_error(error, "failed to update agent task")
    })?;

    // Notify humans when a task reaches a terminal state.
    if matches!(
        response.data.status,
        api_types::AgentTaskStatus::Completed
            | api_types::AgentTaskStatus::Failed
            | api_types::AgentTaskStatus::Cancelled
    ) && existing.status != response.data.status
    {
        notify_agent_task_terminal(state.pool(), &response.data).await;
    }

    Ok(Json(response))
}

#[instrument(
    name = "agent_tasks.delete_agent_task",
    skip(state, ctx),
    fields(agent_task_id = %agent_task_id, user_id = %ctx.user.id)
)]
async fn delete_agent_task(
    State(state): State<AppState>,
    Extension(ctx): Extension<RequestContext>,
    Path(agent_task_id): Path<Uuid>,
) -> Result<Json<DeleteResponse>, ErrorResponse> {
    let existing = AgentTaskRepository::find_by_id(state.pool(), agent_task_id)
        .await
        .map_err(|error| {
            tracing::error!(?error, %agent_task_id, "failed to load agent task");
            ErrorResponse::new(
                StatusCode::INTERNAL_SERVER_ERROR,
                "failed to load agent task",
            )
        })?
        .ok_or_else(|| ErrorResponse::new(StatusCode::NOT_FOUND, "agent task not found"))?;

    ensure_issue_access(state.pool(), ctx.user.id, existing.issue_id).await?;

    let response = AgentTaskRepository::delete(state.pool(), agent_task_id)
        .await
        .map_err(|error| {
            tracing::error!(?error, "failed to delete agent task");
            ErrorResponse::new(StatusCode::INTERNAL_SERVER_ERROR, "internal server error")
        })?;

    Ok(Json(response))
}

#[instrument(
    name = "agent_tasks.claim_agent_task",
    skip(state, ctx, payload),
    fields(user_id = %ctx.user.id, host_id = %payload.host_id)
)]
async fn claim_agent_task(
    State(state): State<AppState>,
    Extension(ctx): Extension<RequestContext>,
    Json(payload): Json<ClaimAgentTaskRequest>,
) -> Result<Json<ClaimAgentTaskResponse>, ErrorResponse> {
    // Any authenticated local host can claim; authorization is host-scoped.
    let _ = ctx;
    if payload.host_id.trim().is_empty() {
        return Err(ErrorResponse::new(
            StatusCode::BAD_REQUEST,
            "host_id is required",
        ));
    }

    let agent_task = AgentTaskRepository::claim_next(state.pool(), &payload.host_id)
        .await
        .map_err(|error| {
            tracing::error!(?error, "failed to claim agent task");
            ErrorResponse::new(
                StatusCode::INTERNAL_SERVER_ERROR,
                "failed to claim agent task",
            )
        })?;

    Ok(Json(ClaimAgentTaskResponse { agent_task }))
}

async fn notify_agent_task_terminal(pool: &sqlx::PgPool, task: &AgentTask) {
    let Ok(Some(issue)) = IssueRepository::find_by_id(pool, task.issue_id).await else {
        return;
    };
    let Ok(Some(agent)) = AgentRepository::find_by_id(pool, task.agent_id).await else {
        return;
    };

    let mut recipients = std::collections::HashSet::new();
    if let Some(creator) = issue.creator_user_id {
        recipients.insert(creator);
    }
    if let Ok(assignees) = IssueAssigneeRepository::list_by_issue(pool, issue.id).await {
        for a in assignees {
            if let Some(uid) = a.user_id {
                recipients.insert(uid);
            }
        }
    }
    // Board inbox subscribers (issue_subscribers table).
    if let Ok(rows) = sqlx::query_scalar::<_, Uuid>(
        "SELECT user_id FROM issue_subscribers WHERE issue_id = $1",
    )
    .bind(issue.id)
    .fetch_all(pool)
    .await
    {
        recipients.extend(rows);
    }
    if recipients.is_empty() {
        return;
    }

    let status_label = match task.status {
        api_types::AgentTaskStatus::Completed => "completed",
        api_types::AgentTaskStatus::Failed => "failed",
        api_types::AgentTaskStatus::Cancelled => "cancelled",
        _ => return,
    };
    let title = format!("Agent {} {}", agent.name, status_label);
    let body = format!(
        "Agent **{}** {} on issue {} — {}{}",
        agent.name,
        status_label,
        issue.simple_id,
        issue.title,
        task.failure_reason
            .as_ref()
            .map(|r| format!(": {r}"))
            .unwrap_or_default()
    );
    let payload = serde_json::json!({
        "agent_task_id": task.id,
        "agent_id": task.agent_id,
        "status": status_label,
        "trigger": format!("{:?}", task.trigger).to_lowercase(),
    });

    for user_id in recipients {
        if let Err(e) = InboxRepository::create(
            pool,
            user_id,
            Some(issue.project_id),
            Some(issue.id),
            "agent_task",
            &title,
            &body,
            payload.clone(),
        )
        .await
        {
            tracing::warn!(?e, %user_id, "failed to create inbox item for agent task");
        }
    }
}
