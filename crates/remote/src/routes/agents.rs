use api_types::{
    Agent, CreateAgentRequest, DeleteResponse, ListAgentsQuery, ListAgentsResponse,
    MutationResponse, UpdateAgentRequest,
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
    organization_members::ensure_project_access,
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
    mutation().router()
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

    let response = AgentRepository::create(
        state.pool(),
        payload.id,
        payload.project_id,
        payload.name,
        payload.instructions,
        payload.default_executor,
        max_concurrent,
        chat_runtime,
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

    let response = AgentRepository::update(
        state.pool(),
        agent_id,
        payload.name,
        payload.instructions,
        payload.default_executor,
        payload.max_concurrent_tasks,
        payload.status,
        payload.chat_runtime,
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
