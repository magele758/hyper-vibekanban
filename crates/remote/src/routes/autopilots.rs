use api_types::{
    Autopilot, AutopilotConcurrencyPolicy, AutopilotExecutionMode, CreateAutopilotRequest,
    DeleteResponse, ListAutopilotQuery, ListAutopilotResponse, ListAutopilotRunsResponse,
    MutationResponse, UpdateAutopilotRequest,
};
use axum::{
    Json, Router,
    extract::{Extension, Path, Query, State},
    http::StatusCode,
    routing::{get, post},
};
use tracing::instrument;
use uuid::Uuid;

use super::{
    error::{ErrorResponse, db_error},
    organization_members::ensure_project_access,
};
use crate::{
    AppState, auth::RequestContext, db::autopilots::AutopilotRepository,
    scheduler::dispatch_autopilot,
};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/autopilots", get(list_autopilots).post(create_autopilot))
        .route(
            "/autopilots/{id}",
            get(get_autopilot)
                .put(update_autopilot)
                .delete(delete_autopilot),
        )
        .route("/autopilots/{id}/trigger", post(trigger_autopilot))
        .route("/autopilots/{id}/runs", get(list_autopilot_runs))
}

#[instrument(name = "autopilots.list", skip(state, ctx), fields(user_id = %ctx.user.id))]
async fn list_autopilots(
    State(state): State<AppState>,
    Extension(ctx): Extension<RequestContext>,
    Query(query): Query<ListAutopilotQuery>,
) -> Result<Json<ListAutopilotResponse>, ErrorResponse> {
    ensure_project_access(state.pool(), ctx.user.id, query.project_id).await?;

    let autopilots = AutopilotRepository::list_by_project(state.pool(), query.project_id)
        .await
        .map_err(|e| {
            tracing::error!(?e, "failed to list autopilots");
            ErrorResponse::new(
                StatusCode::INTERNAL_SERVER_ERROR,
                "failed to list autopilots",
            )
        })?;

    Ok(Json(ListAutopilotResponse { autopilots }))
}

#[instrument(name = "autopilots.get", skip(state, ctx), fields(id = %id, user_id = %ctx.user.id))]
async fn get_autopilot(
    State(state): State<AppState>,
    Extension(ctx): Extension<RequestContext>,
    Path(id): Path<Uuid>,
) -> Result<Json<Autopilot>, ErrorResponse> {
    let ap = load_and_authorize(&state, ctx.user.id, id).await?;
    Ok(Json(ap))
}

#[instrument(name = "autopilots.create", skip(state, ctx, payload), fields(user_id = %ctx.user.id))]
async fn create_autopilot(
    State(state): State<AppState>,
    Extension(ctx): Extension<RequestContext>,
    Json(payload): Json<CreateAutopilotRequest>,
) -> Result<Json<MutationResponse<Autopilot>>, ErrorResponse> {
    ensure_project_access(state.pool(), ctx.user.id, payload.project_id).await?;

    let response = AutopilotRepository::create(
        state.pool(),
        payload.id,
        payload.project_id,
        payload.name,
        payload.agent_id,
        payload.enabled.unwrap_or(true),
        payload
            .execution_mode
            .unwrap_or(AutopilotExecutionMode::CreateIssue),
        payload
            .cron_expression
            .unwrap_or_else(|| "0 * * * *".to_string()),
        payload.timezone.unwrap_or_else(|| "UTC".to_string()),
        payload
            .concurrency_policy
            .unwrap_or(AutopilotConcurrencyPolicy::Skip),
        payload
            .issue_title_template
            .unwrap_or_else(|| "Autopilot run {{date}}".to_string()),
        payload.issue_description_template.unwrap_or_default(),
    )
    .await
    .map_err(|e| db_error(e, "failed to create autopilot"))?;

    Ok(Json(response))
}

#[instrument(name = "autopilots.update", skip(state, ctx, payload), fields(id = %id, user_id = %ctx.user.id))]
async fn update_autopilot(
    State(state): State<AppState>,
    Extension(ctx): Extension<RequestContext>,
    Path(id): Path<Uuid>,
    Json(payload): Json<UpdateAutopilotRequest>,
) -> Result<Json<MutationResponse<Autopilot>>, ErrorResponse> {
    load_and_authorize(&state, ctx.user.id, id).await?;

    let response = AutopilotRepository::update(
        state.pool(),
        id,
        payload.name,
        payload.agent_id,
        payload.enabled,
        payload.execution_mode,
        payload.cron_expression,
        payload.timezone,
        payload.concurrency_policy,
        payload.issue_title_template,
        payload.issue_description_template,
    )
    .await
    .map_err(|e| db_error(e, "failed to update autopilot"))?;

    Ok(Json(response))
}

#[instrument(name = "autopilots.delete", skip(state, ctx), fields(id = %id, user_id = %ctx.user.id))]
async fn delete_autopilot(
    State(state): State<AppState>,
    Extension(ctx): Extension<RequestContext>,
    Path(id): Path<Uuid>,
) -> Result<Json<DeleteResponse>, ErrorResponse> {
    load_and_authorize(&state, ctx.user.id, id).await?;

    let response = AutopilotRepository::delete(state.pool(), id)
        .await
        .map_err(|e| {
            tracing::error!(?e, "failed to delete autopilot");
            ErrorResponse::new(StatusCode::INTERNAL_SERVER_ERROR, "internal server error")
        })?;

    Ok(Json(response))
}

#[instrument(name = "autopilots.trigger", skip(state, ctx), fields(id = %id, user_id = %ctx.user.id))]
async fn trigger_autopilot(
    State(state): State<AppState>,
    Extension(ctx): Extension<RequestContext>,
    Path(id): Path<Uuid>,
) -> Result<StatusCode, ErrorResponse> {
    let ap = load_and_authorize(&state, ctx.user.id, id).await?;
    let pool = state.pool().clone();

    tokio::spawn(async move {
        if let Err(e) = dispatch_autopilot(&pool, &ap).await {
            tracing::error!(?e, autopilot_id = %id, "manual autopilot dispatch failed");
        }
    });

    Ok(StatusCode::ACCEPTED)
}

#[instrument(name = "autopilots.list_runs", skip(state, ctx), fields(id = %id, user_id = %ctx.user.id))]
async fn list_autopilot_runs(
    State(state): State<AppState>,
    Extension(ctx): Extension<RequestContext>,
    Path(id): Path<Uuid>,
) -> Result<Json<ListAutopilotRunsResponse>, ErrorResponse> {
    load_and_authorize(&state, ctx.user.id, id).await?;

    let runs = AutopilotRepository::list_runs(state.pool(), id, 50)
        .await
        .map_err(|e| {
            tracing::error!(?e, "failed to list autopilot runs");
            ErrorResponse::new(StatusCode::INTERNAL_SERVER_ERROR, "failed to list runs")
        })?;

    Ok(Json(ListAutopilotRunsResponse { runs }))
}

async fn load_and_authorize(
    state: &AppState,
    user_id: Uuid,
    id: Uuid,
) -> Result<Autopilot, ErrorResponse> {
    let ap = AutopilotRepository::find_by_id(state.pool(), id)
        .await
        .map_err(|e| {
            tracing::error!(?e, %id, "failed to load autopilot");
            ErrorResponse::new(
                StatusCode::INTERNAL_SERVER_ERROR,
                "failed to load autopilot",
            )
        })?
        .ok_or_else(|| ErrorResponse::new(StatusCode::NOT_FOUND, "autopilot not found"))?;

    ensure_project_access(state.pool(), user_id, ap.project_id).await?;
    Ok(ap)
}
