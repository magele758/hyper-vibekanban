use api_types::{
    AgentTaskTrigger, CreateIssueAssigneeRequest, DeleteResponse, IssueAssignee,
    ListIssueAssigneesQuery, ListIssueAssigneesResponse, MutationResponse, NotificationPayload,
    NotificationType,
};
use axum::{
    Json,
    extract::{Extension, Path, Query, State},
    http::StatusCode,
};
use tracing::instrument;
use uuid::Uuid;

use super::{
    error::{ErrorResponse, db_error},
    organization_members::ensure_issue_access,
};
use crate::{
    AppState,
    auth::RequestContext,
    db::{
        agent_tasks::AgentTaskRepository,
        agents::AgentRepository,
        issue_assignees::{IssueAssigneeError, IssueAssigneeRepository},
        issues::IssueRepository,
        projects::ProjectRepository,
        squads::SquadRepository,
    },
    mutation_definition::{MutationBuilder, NoUpdate},
    notifications::notify_user,
};

/// Mutation definition for IssueAssignee - provides both router and TypeScript metadata.
pub fn mutation() -> MutationBuilder<IssueAssignee, CreateIssueAssigneeRequest, NoUpdate> {
    MutationBuilder::new("issue_assignees")
        .list(list_issue_assignees)
        .get(get_issue_assignee)
        .create(create_issue_assignee)
        .delete(delete_issue_assignee)
}

pub fn router() -> axum::Router<AppState> {
    mutation().router()
}

#[instrument(
    name = "issue_assignees.list_issue_assignees",
    skip(state, ctx),
    fields(issue_id = %query.issue_id, user_id = %ctx.user.id)
)]
async fn list_issue_assignees(
    State(state): State<AppState>,
    Extension(ctx): Extension<RequestContext>,
    Query(query): Query<ListIssueAssigneesQuery>,
) -> Result<Json<ListIssueAssigneesResponse>, ErrorResponse> {
    ensure_issue_access(state.pool(), ctx.user.id, query.issue_id).await?;

    let issue_assignees = IssueAssigneeRepository::list_by_issue(state.pool(), query.issue_id)
        .await
        .map_err(|error| {
            tracing::error!(?error, issue_id = %query.issue_id, "failed to list issue assignees");
            ErrorResponse::new(
                StatusCode::INTERNAL_SERVER_ERROR,
                "failed to list issue assignees",
            )
        })?;

    Ok(Json(ListIssueAssigneesResponse { issue_assignees }))
}

#[instrument(
    name = "issue_assignees.get_issue_assignee",
    skip(state, ctx),
    fields(issue_assignee_id = %issue_assignee_id, user_id = %ctx.user.id)
)]
async fn get_issue_assignee(
    State(state): State<AppState>,
    Extension(ctx): Extension<RequestContext>,
    Path(issue_assignee_id): Path<Uuid>,
) -> Result<Json<IssueAssignee>, ErrorResponse> {
    let assignee = IssueAssigneeRepository::find_by_id(state.pool(), issue_assignee_id)
        .await
        .map_err(|error| {
            tracing::error!(?error, %issue_assignee_id, "failed to load issue assignee");
            ErrorResponse::new(
                StatusCode::INTERNAL_SERVER_ERROR,
                "failed to load issue assignee",
            )
        })?
        .ok_or_else(|| ErrorResponse::new(StatusCode::NOT_FOUND, "issue assignee not found"))?;

    ensure_issue_access(state.pool(), ctx.user.id, assignee.issue_id).await?;

    Ok(Json(assignee))
}

#[instrument(
    name = "issue_assignees.create_issue_assignee",
    skip(state, ctx, payload),
    fields(issue_id = %payload.issue_id, user_id = %ctx.user.id)
)]
async fn create_issue_assignee(
    State(state): State<AppState>,
    Extension(ctx): Extension<RequestContext>,
    Json(payload): Json<CreateIssueAssigneeRequest>,
) -> Result<Json<MutationResponse<IssueAssignee>>, ErrorResponse> {
    let organization_id = ensure_issue_access(state.pool(), ctx.user.id, payload.issue_id).await?;

    match (payload.user_id, payload.agent_id, payload.squad_id) {
        (Some(_), None, None) | (None, Some(_), None) | (None, None, Some(_)) => {}
        _ => {
            return Err(ErrorResponse::new(
                StatusCode::BAD_REQUEST,
                "exactly one of user_id, agent_id, or squad_id is required",
            ));
        }
    }

    if let Some(agent_id) = payload.agent_id {
        let agent = AgentRepository::find_by_id(state.pool(), agent_id)
            .await
            .map_err(|error| {
                tracing::error!(?error, %agent_id, "failed to load agent");
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
    }

    let response = IssueAssigneeRepository::create(
        state.pool(),
        payload.id,
        payload.issue_id,
        payload.user_id,
        payload.agent_id,
        payload.squad_id,
    )
    .await
    .map_err(|error| match error {
        IssueAssigneeError::InvalidAssignee => ErrorResponse::new(
            StatusCode::BAD_REQUEST,
            "exactly one of user_id, agent_id, or squad_id is required",
        ),
        other => {
            tracing::error!(?other, "failed to create issue assignee");
            db_error(other, "failed to create issue assignee")
        }
    })?;

    // Resolve preferred repo hint from project name for the watcher.
    let preferred_repo_hint = async {
        let issue = IssueRepository::find_by_id(state.pool(), payload.issue_id)
            .await
            .ok()
            .flatten()?;
        ProjectRepository::find_by_id(state.pool(), issue.project_id)
            .await
            .ok()
            .flatten()
            .map(|p| p.name)
    }
    .await;

    // Assigning an agent (or squad leader) enqueues a task.
    let enqueue_target = if let Some(agent_id) = payload.agent_id {
        Some((agent_id, None, false))
    } else if let Some(squad_id) = payload.squad_id {
        match SquadRepository::find_by_id(state.pool(), squad_id).await {
            Ok(Some(squad)) => {
                if let Some(leader_id) = squad.leader_agent_id {
                    Some((leader_id, Some(squad_id), true))
                } else {
                    tracing::warn!(%squad_id, "squad has no leader_agent_id; skip enqueue");
                    None
                }
            }
            Ok(None) => {
                let _ = IssueAssigneeRepository::delete(state.pool(), response.data.id).await;
                return Err(ErrorResponse::new(StatusCode::NOT_FOUND, "squad not found"));
            }
            Err(error) => {
                tracing::error!(?error, %squad_id, "failed to load squad");
                let _ = IssueAssigneeRepository::delete(state.pool(), response.data.id).await;
                return Err(ErrorResponse::new(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "failed to load squad",
                ));
            }
        }
    } else {
        None
    };

    if let Some((agent_id, squad_id, is_leader_task)) = enqueue_target
        && let Err(error) = AgentTaskRepository::enqueue(
            state.pool(),
            None,
            agent_id,
            payload.issue_id,
            AgentTaskTrigger::Assign,
            0,
            false,
            squad_id,
            is_leader_task,
            preferred_repo_hint,
        )
        .await
    {
        tracing::error!(?error, %agent_id, issue_id = %payload.issue_id, "failed to enqueue agent task");
        if let Err(rollback_err) =
            IssueAssigneeRepository::delete(state.pool(), response.data.id).await
        {
            tracing::error!(
                ?rollback_err,
                assignee_id = %response.data.id,
                "failed to roll back assignee after enqueue failure"
            );
        }
        return Err(ErrorResponse::new(
            StatusCode::INTERNAL_SERVER_ERROR,
            "failed to enqueue agent task; assignee was not kept",
        ));
    }

    // Auto-subscribe the actor so they receive inbox on agent task completion.
    if let Err(e) = sqlx::query!(
        r#"
        INSERT INTO issue_subscribers (issue_id, user_id, reason)
        VALUES ($1, $2, 'assignee')
        ON CONFLICT (issue_id, user_id) DO NOTHING
        "#,
        payload.issue_id,
        ctx.user.id
    )
    .execute(state.pool())
    .await
    {
        tracing::warn!(?e, "failed to upsert issue subscriber on assign");
    }

    if let Some(assignee_user_id) = payload.user_id
        && assignee_user_id != ctx.user.id
        && let Ok(Some(issue)) = IssueRepository::find_by_id(state.pool(), payload.issue_id).await
    {
        let _ = sqlx::query!(
            r#"
            INSERT INTO issue_subscribers (issue_id, user_id, reason)
            VALUES ($1, $2, 'assignee')
            ON CONFLICT (issue_id, user_id) DO NOTHING
            "#,
            payload.issue_id,
            assignee_user_id
        )
        .execute(state.pool())
        .await;
        notify_user(
            state.pool(),
            organization_id,
            ctx.user.id,
            assignee_user_id,
            &issue,
            NotificationType::IssueAssigneeChanged,
            NotificationPayload {
                assignee_user_id: Some(assignee_user_id),
                ..Default::default()
            },
        )
        .await;
    }

    Ok(Json(response))
}

#[instrument(
    name = "issue_assignees.delete_issue_assignee",
    skip(state, ctx),
    fields(issue_assignee_id = %issue_assignee_id, user_id = %ctx.user.id)
)]
async fn delete_issue_assignee(
    State(state): State<AppState>,
    Extension(ctx): Extension<RequestContext>,
    Path(issue_assignee_id): Path<Uuid>,
) -> Result<Json<DeleteResponse>, ErrorResponse> {
    let assignee = IssueAssigneeRepository::find_by_id(state.pool(), issue_assignee_id)
        .await
        .map_err(|error| {
            tracing::error!(?error, %issue_assignee_id, "failed to load issue assignee");
            ErrorResponse::new(
                StatusCode::INTERNAL_SERVER_ERROR,
                "failed to load issue assignee",
            )
        })?
        .ok_or_else(|| ErrorResponse::new(StatusCode::NOT_FOUND, "issue assignee not found"))?;

    let organization_id = ensure_issue_access(state.pool(), ctx.user.id, assignee.issue_id).await?;

    let response = IssueAssigneeRepository::delete(state.pool(), issue_assignee_id)
        .await
        .map_err(|error| {
            tracing::error!(?error, "failed to delete issue assignee");
            ErrorResponse::new(StatusCode::INTERNAL_SERVER_ERROR, "internal server error")
        })?;

    if let Some(assignee_user_id) = assignee.user_id
        && assignee_user_id != ctx.user.id
        && let Ok(Some(issue)) = IssueRepository::find_by_id(state.pool(), assignee.issue_id).await
    {
        notify_user(
            state.pool(),
            organization_id,
            ctx.user.id,
            assignee_user_id,
            &issue,
            NotificationType::IssueUnassigned,
            NotificationPayload {
                assignee_user_id: Some(assignee_user_id),
                ..Default::default()
            },
        )
        .await;
    }

    Ok(Json(response))
}
