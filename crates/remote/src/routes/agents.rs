use api_types::{
    Agent, CreateAgentRequest, DeleteResponse, ListAgentsQuery, ListAgentsResponse,
    ListOrgAgentsQuery, ListOrgAgentsResponse, MutationResponse, OrgAgentEntry, UpdateAgentRequest,
    agent::normalize_default_executor,
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
    organization_members::{ensure_member_access, ensure_project_access},
};
use crate::{
    AppState,
    auth::RequestContext,
    db::{agents::AgentRepository, copilot::CopilotRepository},
    mutation_definition::MutationBuilder,
};

pub fn mutation() -> MutationBuilder<Agent, CreateAgentRequest, UpdateAgentRequest> {
    MutationBuilder::new("agents")
        .list(list_agents)
        .get(get_agent)
        .create(create_agent)
        .update(update_agent)
        .delete(delete_agent)
}

pub fn router() -> axum::Router<AppState> {
    mutation()
        .router()
        .route("/agents/roster", axum::routing::get(list_org_agents))
}

/// Organization-wide roster of configured agents ("the workforce").
///
/// Distinct from `list_agents`, which is project-scoped: the workforce menu
/// spans every project the caller can see in one organization.
#[instrument(name = "agents.list_org_agents", skip(state, ctx))]
async fn list_org_agents(
    State(state): State<AppState>,
    Extension(ctx): Extension<RequestContext>,
    Query(query): Query<ListOrgAgentsQuery>,
) -> Result<Json<ListOrgAgentsResponse>, ErrorResponse> {
    ensure_member_access(state.pool(), query.organization_id, ctx.user.id).await?;

    let rows = AgentRepository::list_by_organization(state.pool(), query.organization_id)
        .await
        .map_err(|error| {
            tracing::error!(
                ?error,
                organization_id = %query.organization_id,
                "failed to list organization agents"
            );
            ErrorResponse::new(StatusCode::INTERNAL_SERVER_ERROR, "failed to list agents")
        })?;

    Ok(Json(ListOrgAgentsResponse {
        agents: rows
            .into_iter()
            .map(|(agent, project_name)| OrgAgentEntry {
                agent,
                project_name,
            })
            .collect(),
    }))
}

/// Reject unknown executor names at the write boundary.
///
/// `default_executor` is stored as TEXT because remote cannot know what a given
/// local host has installed, but that is no reason to accept arbitrary strings:
/// an invalid value would otherwise surface much later as a silent fallback to
/// the host default.
fn validate_executor(value: Option<&str>) -> Result<Option<String>, ErrorResponse> {
    normalize_default_executor(value)
        .map_err(|message| ErrorResponse::new(StatusCode::BAD_REQUEST, message))
}

/// A reviewer must exist, live in the same project, and never be the agent itself.
async fn validate_reviewer(
    state: &AppState,
    agent_id: Option<Uuid>,
    project_id: Uuid,
    reviewer_agent_id: Uuid,
) -> Result<(), ErrorResponse> {
    if Some(reviewer_agent_id) == agent_id {
        return Err(ErrorResponse::new(
            StatusCode::BAD_REQUEST,
            "an agent cannot review its own work",
        ));
    }

    let reviewer = AgentRepository::find_by_id(state.pool(), reviewer_agent_id)
        .await
        .map_err(|error| {
            tracing::error!(?error, %reviewer_agent_id, "failed to load reviewer agent");
            ErrorResponse::new(
                StatusCode::INTERNAL_SERVER_ERROR,
                "failed to load reviewer agent",
            )
        })?
        .ok_or_else(|| ErrorResponse::new(StatusCode::BAD_REQUEST, "reviewer agent not found"))?;

    if reviewer.project_id != project_id {
        return Err(ErrorResponse::new(
            StatusCode::BAD_REQUEST,
            "reviewer agent must belong to the same project",
        ));
    }

    Ok(())
}

#[instrument(
    name = "agents.list_agents",
    skip(state, ctx),
    fields(project_id = %query.project_id, user_id = %ctx.user.id)
)]
async fn list_agents(
    State(state): State<AppState>,
    Extension(ctx): Extension<RequestContext>,
    Query(query): Query<ListAgentsQuery>,
) -> Result<Json<ListAgentsResponse>, ErrorResponse> {
    ensure_project_access(state.pool(), ctx.user.id, query.project_id).await?;

    let agents = AgentRepository::list_by_project(state.pool(), query.project_id)
        .await
        .map_err(|error| {
            tracing::error!(?error, project_id = %query.project_id, "failed to list agents");
            ErrorResponse::new(StatusCode::INTERNAL_SERVER_ERROR, "failed to list agents")
        })?;

    Ok(Json(ListAgentsResponse { agents }))
}

#[instrument(
    name = "agents.get_agent",
    skip(state, ctx),
    fields(agent_id = %agent_id, user_id = %ctx.user.id)
)]
async fn get_agent(
    State(state): State<AppState>,
    Extension(ctx): Extension<RequestContext>,
    Path(agent_id): Path<Uuid>,
) -> Result<Json<Agent>, ErrorResponse> {
    let agent = AgentRepository::find_by_id(state.pool(), agent_id)
        .await
        .map_err(|error| {
            tracing::error!(?error, %agent_id, "failed to load agent");
            ErrorResponse::new(StatusCode::INTERNAL_SERVER_ERROR, "failed to load agent")
        })?
        .ok_or_else(|| ErrorResponse::new(StatusCode::NOT_FOUND, "agent not found"))?;

    ensure_project_access(state.pool(), ctx.user.id, agent.project_id).await?;

    Ok(Json(agent))
}

#[instrument(
    name = "agents.create_agent",
    skip(state, ctx, payload),
    fields(project_id = %payload.project_id, user_id = %ctx.user.id)
)]
async fn create_agent(
    State(state): State<AppState>,
    Extension(ctx): Extension<RequestContext>,
    Json(payload): Json<CreateAgentRequest>,
) -> Result<Json<MutationResponse<Agent>>, ErrorResponse> {
    ensure_project_access(state.pool(), ctx.user.id, payload.project_id).await?;

    let max_concurrent = payload.max_concurrent_tasks.unwrap_or(1).max(1);
    let chat_runtime = payload.chat_runtime.unwrap_or_default();
    let default_executor = validate_executor(payload.default_executor.as_deref())?;

    if let Some(reviewer_agent_id) = payload.reviewer_agent_id {
        validate_reviewer(&state, payload.id, payload.project_id, reviewer_agent_id).await?;
    }

    let response = AgentRepository::create(
        state.pool(),
        payload.id,
        payload.project_id,
        payload.name,
        payload.instructions,
        default_executor,
        max_concurrent,
        chat_runtime,
        payload.reviewer_agent_id,
        Some(ctx.user.id),
    )
    .await
    .map_err(|error| {
        tracing::error!(?error, "failed to create agent");
        db_error(error, "failed to create agent")
    })?;

    if payload.api_key.is_some()
        || payload.base_url.is_some()
        || payload.model_name.is_some()
        || payload.working_directory.is_some()
    {
        let update_api_key = payload.api_key.is_some();
        let update_working_directory = payload.working_directory.is_some();
        let api_key = payload.api_key.and_then(|k| {
            let t = k.trim().to_string();
            if t.is_empty() { None } else { Some(t) }
        });
        let working_directory = payload.working_directory.and_then(|d| {
            let t = d.trim().to_string();
            if t.is_empty() { None } else { Some(t) }
        });
        if let Err(error) = CopilotRepository::upsert_llm_settings(
            state.pool(),
            response.data.id,
            api_key,
            payload.base_url.filter(|u| !u.trim().is_empty()),
            payload.model_name.filter(|m| !m.trim().is_empty()),
            working_directory,
            update_api_key,
            update_working_directory,
        )
        .await
        {
            tracing::error!(?error, "failed to save agent llm settings on create");
        }
    }

    Ok(Json(response))
}

#[instrument(
    name = "agents.update_agent",
    skip(state, ctx, payload),
    fields(agent_id = %agent_id, user_id = %ctx.user.id)
)]
async fn update_agent(
    State(state): State<AppState>,
    Extension(ctx): Extension<RequestContext>,
    Path(agent_id): Path<Uuid>,
    Json(payload): Json<UpdateAgentRequest>,
) -> Result<Json<MutationResponse<Agent>>, ErrorResponse> {
    let existing = AgentRepository::find_by_id(state.pool(), agent_id)
        .await
        .map_err(|error| {
            tracing::error!(?error, %agent_id, "failed to load agent");
            ErrorResponse::new(StatusCode::INTERNAL_SERVER_ERROR, "failed to load agent")
        })?
        .ok_or_else(|| ErrorResponse::new(StatusCode::NOT_FOUND, "agent not found"))?;

    ensure_project_access(state.pool(), ctx.user.id, existing.project_id).await?;

    // `Option<Option<T>>`: outer None = field absent, inner None = explicit clear.
    let default_executor = match payload.default_executor {
        Some(value) => Some(validate_executor(value.as_deref())?),
        None => None,
    };

    if let Some(Some(reviewer_agent_id)) = payload.reviewer_agent_id {
        validate_reviewer(
            &state,
            Some(agent_id),
            existing.project_id,
            reviewer_agent_id,
        )
        .await?;
    }

    let response = AgentRepository::update(
        state.pool(),
        agent_id,
        payload.name,
        payload.instructions,
        default_executor,
        payload.max_concurrent_tasks,
        payload.status,
        payload.chat_runtime,
        payload.reviewer_agent_id,
    )
    .await
    .map_err(|error| {
        tracing::error!(?error, "failed to update agent");
        db_error(error, "failed to update agent")
    })?;

    Ok(Json(response))
}

#[instrument(
    name = "agents.delete_agent",
    skip(state, ctx),
    fields(agent_id = %agent_id, user_id = %ctx.user.id)
)]
async fn delete_agent(
    State(state): State<AppState>,
    Extension(ctx): Extension<RequestContext>,
    Path(agent_id): Path<Uuid>,
) -> Result<Json<DeleteResponse>, ErrorResponse> {
    let existing = AgentRepository::find_by_id(state.pool(), agent_id)
        .await
        .map_err(|error| {
            tracing::error!(?error, %agent_id, "failed to load agent");
            ErrorResponse::new(StatusCode::INTERNAL_SERVER_ERROR, "failed to load agent")
        })?
        .ok_or_else(|| ErrorResponse::new(StatusCode::NOT_FOUND, "agent not found"))?;

    ensure_project_access(state.pool(), ctx.user.id, existing.project_id).await?;

    let response = AgentRepository::delete(state.pool(), agent_id)
        .await
        .map_err(|error| {
            tracing::error!(?error, "failed to delete agent");
            ErrorResponse::new(StatusCode::INTERNAL_SERVER_ERROR, "internal server error")
        })?;

    Ok(Json(response))
}

#[cfg(test)]
mod tests {
    /// `/agents/roster` is registered next to `/agents/{id}`. Axum panics on
    /// ambiguous routes when the router is built, so constructing it is the
    /// assertion: a literal segment must be allowed to coexist with the
    /// parameterised one.
    #[test]
    fn router_builds_with_roster_route_alongside_id_route() {
        let _router = super::router();
    }
}
