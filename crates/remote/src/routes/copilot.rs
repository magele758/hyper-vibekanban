use api_types::{
    AgentLlmSettings, AgentLlmSettingsSecret, CopilotMessage, CopilotSession,
    CreateCopilotMessageRequest, CreateCopilotSessionRequest, DeleteResponse,
    ListCopilotMessagesQuery, ListCopilotMessagesResponse, ListCopilotSessionsQuery,
    ListCopilotSessionsResponse, MutationResponse, UpdateCopilotSessionRequest,
    UpsertAgentLlmSettingsRequest,
};
use axum::{
    Json, Router,
    extract::{Extension, Path, Query, State},
    http::StatusCode,
    routing::get,
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
    db::{agents::AgentRepository, copilot::CopilotRepository},
};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/copilot_sessions", get(list_sessions).post(create_session))
        .route(
            "/copilot_sessions/{session_id}",
            get(get_session)
                .patch(update_session)
                .delete(delete_session),
        )
        .route("/copilot_messages", get(list_messages).post(create_message))
        .route(
            "/agents/{agent_id}/llm_settings",
            get(get_llm_settings).put(upsert_llm_settings),
        )
        .route(
            "/agents/{agent_id}/llm_settings/secret",
            get(get_llm_settings_secret),
        )
}

#[instrument(name = "copilot.list_sessions", skip(state, ctx))]
async fn list_sessions(
    State(state): State<AppState>,
    Extension(ctx): Extension<RequestContext>,
    Query(query): Query<ListCopilotSessionsQuery>,
) -> Result<Json<ListCopilotSessionsResponse>, ErrorResponse> {
    ensure_project_access(state.pool(), ctx.user.id, query.project_id).await?;
    let sessions = CopilotRepository::list_sessions(
        state.pool(),
        query.project_id,
        query.agent_id,
        query.project_copilot.unwrap_or(false),
    )
    .await
    .map_err(|e| {
        tracing::error!(?e, "failed to list copilot sessions");
        ErrorResponse::new(StatusCode::INTERNAL_SERVER_ERROR, "failed to list sessions")
    })?;
    Ok(Json(ListCopilotSessionsResponse {
        copilot_sessions: sessions,
    }))
}

#[instrument(name = "copilot.get_session", skip(state, ctx))]
async fn get_session(
    State(state): State<AppState>,
    Extension(ctx): Extension<RequestContext>,
    Path(session_id): Path<Uuid>,
) -> Result<Json<CopilotSession>, ErrorResponse> {
    let session = CopilotRepository::find_session_by_id(state.pool(), session_id)
        .await
        .map_err(|e| {
            tracing::error!(?e, "failed to load session");
            ErrorResponse::new(StatusCode::INTERNAL_SERVER_ERROR, "failed to load session")
        })?
        .ok_or_else(|| ErrorResponse::new(StatusCode::NOT_FOUND, "session not found"))?;
    ensure_project_access(state.pool(), ctx.user.id, session.project_id).await?;
    Ok(Json(session))
}

#[instrument(name = "copilot.create_session", skip(state, ctx, payload))]
async fn create_session(
    State(state): State<AppState>,
    Extension(ctx): Extension<RequestContext>,
    Json(payload): Json<CreateCopilotSessionRequest>,
) -> Result<Json<MutationResponse<CopilotSession>>, ErrorResponse> {
    ensure_project_access(state.pool(), ctx.user.id, payload.project_id).await?;
    if let Some(agent_id) = payload.agent_id {
        let agent = AgentRepository::find_by_id(state.pool(), agent_id)
            .await
            .map_err(|e| {
                tracing::error!(?e, "failed to load agent");
                ErrorResponse::new(StatusCode::INTERNAL_SERVER_ERROR, "failed to load agent")
            })?
            .ok_or_else(|| ErrorResponse::new(StatusCode::NOT_FOUND, "agent not found"))?;
        if agent.project_id != payload.project_id {
            return Err(ErrorResponse::new(
                StatusCode::BAD_REQUEST,
                "agent must belong to project",
            ));
        }
    }
    if let Some(issue_id) = payload.issue_id {
        ensure_issue_access(state.pool(), ctx.user.id, issue_id).await?;
    }
    let response = CopilotRepository::create_session(
        state.pool(),
        payload.id,
        payload.project_id,
        payload.agent_id,
        payload.issue_id,
        Some(ctx.user.id),
        payload.title,
    )
    .await
    .map_err(|e| {
        tracing::error!(?e, "failed to create session");
        db_error(e, "failed to create session")
    })?;
    Ok(Json(response))
}

#[instrument(name = "copilot.update_session", skip(state, ctx, payload))]
async fn update_session(
    State(state): State<AppState>,
    Extension(ctx): Extension<RequestContext>,
    Path(session_id): Path<Uuid>,
    Json(payload): Json<UpdateCopilotSessionRequest>,
) -> Result<Json<MutationResponse<CopilotSession>>, ErrorResponse> {
    let existing = CopilotRepository::find_session_by_id(state.pool(), session_id)
        .await
        .map_err(|e| {
            tracing::error!(?e, "failed to load session");
            ErrorResponse::new(StatusCode::INTERNAL_SERVER_ERROR, "failed to load session")
        })?
        .ok_or_else(|| ErrorResponse::new(StatusCode::NOT_FOUND, "session not found"))?;
    ensure_project_access(state.pool(), ctx.user.id, existing.project_id).await?;
    let response = CopilotRepository::update_session(
        state.pool(),
        session_id,
        payload.title,
        payload.external_agent_id,
    )
    .await
    .map_err(|e| {
        tracing::error!(?e, "failed to update session");
        db_error(e, "failed to update session")
    })?;
    Ok(Json(response))
}

#[instrument(name = "copilot.delete_session", skip(state, ctx))]
async fn delete_session(
    State(state): State<AppState>,
    Extension(ctx): Extension<RequestContext>,
    Path(session_id): Path<Uuid>,
) -> Result<Json<DeleteResponse>, ErrorResponse> {
    let existing = CopilotRepository::find_session_by_id(state.pool(), session_id)
        .await
        .map_err(|e| {
            tracing::error!(?e, "failed to load session");
            ErrorResponse::new(StatusCode::INTERNAL_SERVER_ERROR, "failed to load session")
        })?
        .ok_or_else(|| ErrorResponse::new(StatusCode::NOT_FOUND, "session not found"))?;
    ensure_project_access(state.pool(), ctx.user.id, existing.project_id).await?;
    let response = CopilotRepository::delete_session(state.pool(), session_id)
        .await
        .map_err(|e| {
            tracing::error!(?e, "failed to delete session");
            ErrorResponse::new(
                StatusCode::INTERNAL_SERVER_ERROR,
                "failed to delete session",
            )
        })?;
    Ok(Json(response))
}

#[instrument(name = "copilot.list_messages", skip(state, ctx))]
async fn list_messages(
    State(state): State<AppState>,
    Extension(ctx): Extension<RequestContext>,
    Query(query): Query<ListCopilotMessagesQuery>,
) -> Result<Json<ListCopilotMessagesResponse>, ErrorResponse> {
    let session = CopilotRepository::find_session_by_id(state.pool(), query.session_id)
        .await
        .map_err(|e| {
            tracing::error!(?e, "failed to load session");
            ErrorResponse::new(StatusCode::INTERNAL_SERVER_ERROR, "failed to load session")
        })?
        .ok_or_else(|| ErrorResponse::new(StatusCode::NOT_FOUND, "session not found"))?;
    ensure_project_access(state.pool(), ctx.user.id, session.project_id).await?;
    let messages = CopilotRepository::list_messages(state.pool(), query.session_id)
        .await
        .map_err(|e| {
            tracing::error!(?e, "failed to list messages");
            ErrorResponse::new(StatusCode::INTERNAL_SERVER_ERROR, "failed to list messages")
        })?;
    Ok(Json(ListCopilotMessagesResponse {
        copilot_messages: messages,
    }))
}

#[instrument(name = "copilot.create_message", skip(state, ctx, payload))]
async fn create_message(
    State(state): State<AppState>,
    Extension(ctx): Extension<RequestContext>,
    Json(payload): Json<CreateCopilotMessageRequest>,
) -> Result<Json<MutationResponse<CopilotMessage>>, ErrorResponse> {
    let session = CopilotRepository::find_session_by_id(state.pool(), payload.session_id)
        .await
        .map_err(|e| {
            tracing::error!(?e, "failed to load session");
            ErrorResponse::new(StatusCode::INTERNAL_SERVER_ERROR, "failed to load session")
        })?
        .ok_or_else(|| ErrorResponse::new(StatusCode::NOT_FOUND, "session not found"))?;
    ensure_project_access(state.pool(), ctx.user.id, session.project_id).await?;
    let response = CopilotRepository::create_message(
        state.pool(),
        payload.id,
        payload.session_id,
        payload.role,
        payload.content,
    )
    .await
    .map_err(|e| {
        tracing::error!(?e, "failed to create message");
        db_error(e, "failed to create message")
    })?;
    Ok(Json(response))
}

#[instrument(name = "agents.get_llm_settings", skip(state, ctx))]
async fn get_llm_settings(
    State(state): State<AppState>,
    Extension(ctx): Extension<RequestContext>,
    Path(agent_id): Path<Uuid>,
) -> Result<Json<AgentLlmSettings>, ErrorResponse> {
    let agent = AgentRepository::find_by_id(state.pool(), agent_id)
        .await
        .map_err(|e| {
            tracing::error!(?e, "failed to load agent");
            ErrorResponse::new(StatusCode::INTERNAL_SERVER_ERROR, "failed to load agent")
        })?
        .ok_or_else(|| ErrorResponse::new(StatusCode::NOT_FOUND, "agent not found"))?;
    ensure_project_access(state.pool(), ctx.user.id, agent.project_id).await?;
    let settings = CopilotRepository::get_llm_settings(state.pool(), agent_id)
        .await
        .map_err(|e| {
            tracing::error!(?e, "failed to load llm settings");
            ErrorResponse::new(StatusCode::INTERNAL_SERVER_ERROR, "failed to load settings")
        })?
        .unwrap_or(AgentLlmSettings {
            agent_id,
            has_api_key: false,
            base_url: None,
            model_name: None,
            updated_at: agent.updated_at,
        });
    Ok(Json(settings))
}

/// Sidecar / trusted local host: returns raw api_key.
#[instrument(name = "agents.get_llm_settings_secret", skip(state, ctx))]
async fn get_llm_settings_secret(
    State(state): State<AppState>,
    Extension(ctx): Extension<RequestContext>,
    Path(agent_id): Path<Uuid>,
) -> Result<Json<AgentLlmSettingsSecret>, ErrorResponse> {
    let agent = AgentRepository::find_by_id(state.pool(), agent_id)
        .await
        .map_err(|e| {
            tracing::error!(?e, "failed to load agent");
            ErrorResponse::new(StatusCode::INTERNAL_SERVER_ERROR, "failed to load agent")
        })?
        .ok_or_else(|| ErrorResponse::new(StatusCode::NOT_FOUND, "agent not found"))?;
    ensure_project_access(state.pool(), ctx.user.id, agent.project_id).await?;
    let settings = CopilotRepository::get_llm_settings_secret(state.pool(), agent_id)
        .await
        .map_err(|e| {
            tracing::error!(?e, "failed to load llm settings");
            ErrorResponse::new(StatusCode::INTERNAL_SERVER_ERROR, "failed to load settings")
        })?
        .unwrap_or(AgentLlmSettingsSecret {
            agent_id,
            api_key: None,
            base_url: None,
            model_name: None,
        });
    Ok(Json(settings))
}

#[instrument(name = "agents.upsert_llm_settings", skip(state, ctx, payload))]
async fn upsert_llm_settings(
    State(state): State<AppState>,
    Extension(ctx): Extension<RequestContext>,
    Path(agent_id): Path<Uuid>,
    Json(payload): Json<UpsertAgentLlmSettingsRequest>,
) -> Result<Json<AgentLlmSettings>, ErrorResponse> {
    let agent = AgentRepository::find_by_id(state.pool(), agent_id)
        .await
        .map_err(|e| {
            tracing::error!(?e, "failed to load agent");
            ErrorResponse::new(StatusCode::INTERNAL_SERVER_ERROR, "failed to load agent")
        })?
        .ok_or_else(|| ErrorResponse::new(StatusCode::NOT_FOUND, "agent not found"))?;
    ensure_project_access(state.pool(), ctx.user.id, agent.project_id).await?;

    let update_api_key = payload.api_key.is_some();
    let api_key = payload.api_key.and_then(|k| {
        let t = k.trim().to_string();
        if t.is_empty() { None } else { Some(t) }
    });
    let base_url = payload.base_url.map(|u| {
        let t = u.trim().to_string();
        if t.is_empty() { None } else { Some(t) }
    });
    let model_name = payload.model_name.map(|m| {
        let t = m.trim().to_string();
        if t.is_empty() { None } else { Some(t) }
    });

    let settings = CopilotRepository::upsert_llm_settings(
        state.pool(),
        agent_id,
        api_key,
        base_url.flatten(),
        model_name.flatten(),
        update_api_key,
    )
    .await
    .map_err(|e| {
        tracing::error!(?e, "failed to upsert llm settings");
        db_error(e, "failed to save settings")
    })?;
    Ok(Json(settings))
}
