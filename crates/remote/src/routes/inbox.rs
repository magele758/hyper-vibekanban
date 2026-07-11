use api_types::{InboxUnreadCountResponse, ListInboxResponse};
use axum::{
    Json, Router,
    extract::{Extension, Query, State},
    http::StatusCode,
    routing::{get, post},
};
use serde::Deserialize;
use tracing::instrument;
use uuid::Uuid;

use super::error::ErrorResponse;
use crate::{AppState, auth::RequestContext, db::inbox::InboxRepository};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/inbox", get(list_inbox))
        .route("/inbox/unread-count", get(unread_count))
        .route("/inbox/mark-read", post(mark_read))
        .route("/inbox/mark-all-read", post(mark_all_read))
        .route("/inbox/archive", post(archive))
}

#[derive(Debug, Deserialize)]
struct ListInboxQuery {
    #[serde(default)]
    include_archived: bool,
    #[serde(default = "default_limit")]
    limit: i64,
}

fn default_limit() -> i64 {
    50
}

#[derive(Deserialize)]
struct BatchIdsRequest {
    ids: Vec<Uuid>,
}

#[instrument(name = "inbox.list", skip(state, ctx), fields(user_id = %ctx.user.id))]
async fn list_inbox(
    State(state): State<AppState>,
    Extension(ctx): Extension<RequestContext>,
    Query(query): Query<ListInboxQuery>,
) -> Result<Json<ListInboxResponse>, ErrorResponse> {
    let limit = query.limit.min(200).max(1);
    let items = InboxRepository::list(state.pool(), ctx.user.id, query.include_archived, limit)
        .await
        .map_err(|e| {
            tracing::error!(?e, "failed to list inbox");
            ErrorResponse::new(StatusCode::INTERNAL_SERVER_ERROR, "failed to list inbox")
        })?;

    let unread_count = InboxRepository::unread_count(state.pool(), ctx.user.id)
        .await
        .unwrap_or(0);

    Ok(Json(ListInboxResponse {
        items,
        unread_count,
    }))
}

#[instrument(name = "inbox.unread_count", skip(state, ctx), fields(user_id = %ctx.user.id))]
async fn unread_count(
    State(state): State<AppState>,
    Extension(ctx): Extension<RequestContext>,
) -> Result<Json<InboxUnreadCountResponse>, ErrorResponse> {
    let unread_count = InboxRepository::unread_count(state.pool(), ctx.user.id)
        .await
        .map_err(|e| {
            tracing::error!(?e, "failed to count unread inbox");
            ErrorResponse::new(StatusCode::INTERNAL_SERVER_ERROR, "internal server error")
        })?;

    Ok(Json(InboxUnreadCountResponse { unread_count }))
}

#[instrument(name = "inbox.mark_read", skip(state, ctx, payload), fields(user_id = %ctx.user.id))]
async fn mark_read(
    State(state): State<AppState>,
    Extension(ctx): Extension<RequestContext>,
    Json(payload): Json<BatchIdsRequest>,
) -> Result<StatusCode, ErrorResponse> {
    InboxRepository::mark_read(state.pool(), ctx.user.id, &payload.ids)
        .await
        .map_err(|e| {
            tracing::error!(?e, "failed to mark inbox read");
            ErrorResponse::new(StatusCode::INTERNAL_SERVER_ERROR, "internal server error")
        })?;
    Ok(StatusCode::NO_CONTENT)
}

#[instrument(name = "inbox.mark_all_read", skip(state, ctx), fields(user_id = %ctx.user.id))]
async fn mark_all_read(
    State(state): State<AppState>,
    Extension(ctx): Extension<RequestContext>,
) -> Result<StatusCode, ErrorResponse> {
    InboxRepository::mark_all_read(state.pool(), ctx.user.id)
        .await
        .map_err(|e| {
            tracing::error!(?e, "failed to mark all inbox read");
            ErrorResponse::new(StatusCode::INTERNAL_SERVER_ERROR, "internal server error")
        })?;
    Ok(StatusCode::NO_CONTENT)
}

#[instrument(name = "inbox.archive", skip(state, ctx, payload), fields(user_id = %ctx.user.id))]
async fn archive(
    State(state): State<AppState>,
    Extension(ctx): Extension<RequestContext>,
    Json(payload): Json<BatchIdsRequest>,
) -> Result<StatusCode, ErrorResponse> {
    InboxRepository::archive(state.pool(), ctx.user.id, &payload.ids)
        .await
        .map_err(|e| {
            tracing::error!(?e, "failed to archive inbox items");
            ErrorResponse::new(StatusCode::INTERNAL_SERVER_ERROR, "internal server error")
        })?;
    Ok(StatusCode::NO_CONTENT)
}
