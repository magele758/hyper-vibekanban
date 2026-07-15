use axum::{
    Extension, Json, Router,
    extract::{Path, State},
    middleware::from_fn_with_state,
    response::Json as ResponseJson,
    routing::{get, put},
};
use db::models::{
    scratch::DraftFollowUpData, session::Session, session_queued_message::SessionQueuedMessageError,
};
use deployment::Deployment;
use executors::profile::ExecutorConfig;
use serde::Deserialize;
use services::services::queued_message::QueueStatus;
use ts_rs::TS;
use utils::response::ApiResponse;
use uuid::Uuid;

use crate::{DeploymentImpl, error::ApiError, middleware::load_session_middleware};

/// Request body for queueing a follow-up message
#[derive(Debug, Deserialize, TS)]
struct QueueMessageRequest {
    pub message: String,
    pub executor_config: ExecutorConfig,
}

#[derive(Debug, Deserialize, TS)]
struct UpdateQueuedMessageRequest {
    pub message: String,
    pub executor_config: ExecutorConfig,
}

#[derive(Debug, Deserialize, TS)]
struct ReorderQueueRequest {
    pub item_ids: Vec<Uuid>,
}

fn map_queue_error(err: SessionQueuedMessageError) -> ApiError {
    match err {
        SessionQueuedMessageError::NotFound => {
            ApiError::BadRequest("Queued message not found".to_string())
        }
        SessionQueuedMessageError::SessionMismatch => {
            ApiError::BadRequest("Queued message does not belong to this session".to_string())
        }
        SessionQueuedMessageError::InvalidReorder => ApiError::BadRequest(
            "Reorder must include exactly the current queue item ids".to_string(),
        ),
        SessionQueuedMessageError::Database(e) => ApiError::Database(e),
        SessionQueuedMessageError::Serde(e) => {
            ApiError::BadRequest(format!("Invalid queue payload: {e}"))
        }
    }
}

/// Queue a follow-up message to be executed when the current execution finishes
async fn queue_message(
    Extension(session): Extension<Session>,
    State(deployment): State<DeploymentImpl>,
    Json(payload): Json<QueueMessageRequest>,
) -> Result<ResponseJson<ApiResponse<QueueStatus>>, ApiError> {
    let data = DraftFollowUpData {
        message: payload.message,
        executor_config: payload.executor_config,
    };

    deployment
        .queued_message_service()
        .enqueue(session.id, data)
        .await
        .map_err(map_queue_error)?;

    deployment
        .track_if_analytics_allowed(
            "follow_up_queued",
            serde_json::json!({
                "session_id": session.id.to_string(),
                "workspace_id": session.workspace_id.to_string(),
            }),
        )
        .await;

    let status = deployment
        .queued_message_service()
        .get_status(session.id)
        .await
        .map_err(map_queue_error)?;

    Ok(ResponseJson(ApiResponse::success(status)))
}

/// Clear all queued follow-up messages for the session
async fn clear_queued_messages(
    Extension(session): Extension<Session>,
    State(deployment): State<DeploymentImpl>,
) -> Result<ResponseJson<ApiResponse<QueueStatus>>, ApiError> {
    deployment
        .queued_message_service()
        .clear(session.id)
        .await
        .map_err(map_queue_error)?;

    deployment
        .track_if_analytics_allowed(
            "follow_up_queue_cancelled",
            serde_json::json!({
                "session_id": session.id.to_string(),
                "workspace_id": session.workspace_id.to_string(),
            }),
        )
        .await;

    Ok(ResponseJson(ApiResponse::success(QueueStatus::Empty)))
}

/// Get the current queue status for a session
async fn get_queue_status(
    Extension(session): Extension<Session>,
    State(deployment): State<DeploymentImpl>,
) -> Result<ResponseJson<ApiResponse<QueueStatus>>, ApiError> {
    let status = deployment
        .queued_message_service()
        .get_status(session.id)
        .await
        .map_err(map_queue_error)?;

    Ok(ResponseJson(ApiResponse::success(status)))
}

async fn update_queued_message(
    Extension(session): Extension<Session>,
    State(deployment): State<DeploymentImpl>,
    Path(item_id): Path<Uuid>,
    Json(payload): Json<UpdateQueuedMessageRequest>,
) -> Result<ResponseJson<ApiResponse<QueueStatus>>, ApiError> {
    let data = DraftFollowUpData {
        message: payload.message,
        executor_config: payload.executor_config,
    };

    deployment
        .queued_message_service()
        .update(session.id, item_id, data)
        .await
        .map_err(map_queue_error)?;

    let status = deployment
        .queued_message_service()
        .get_status(session.id)
        .await
        .map_err(map_queue_error)?;

    Ok(ResponseJson(ApiResponse::success(status)))
}

async fn remove_queued_message(
    Extension(session): Extension<Session>,
    State(deployment): State<DeploymentImpl>,
    Path(item_id): Path<Uuid>,
) -> Result<ResponseJson<ApiResponse<QueueStatus>>, ApiError> {
    deployment
        .queued_message_service()
        .remove(session.id, item_id)
        .await
        .map_err(map_queue_error)?;

    let status = deployment
        .queued_message_service()
        .get_status(session.id)
        .await
        .map_err(map_queue_error)?;

    Ok(ResponseJson(ApiResponse::success(status)))
}

async fn reorder_queued_messages(
    Extension(session): Extension<Session>,
    State(deployment): State<DeploymentImpl>,
    Json(payload): Json<ReorderQueueRequest>,
) -> Result<ResponseJson<ApiResponse<QueueStatus>>, ApiError> {
    let messages = deployment
        .queued_message_service()
        .reorder(session.id, payload.item_ids)
        .await
        .map_err(map_queue_error)?;

    Ok(ResponseJson(ApiResponse::success(
        QueueStatus::from_messages(messages),
    )))
}

pub(super) fn router(deployment: &DeploymentImpl) -> Router<DeploymentImpl> {
    Router::new()
        .route(
            "/",
            get(get_queue_status)
                .post(queue_message)
                .delete(clear_queued_messages),
        )
        .route("/reorder", put(reorder_queued_messages))
        .route(
            "/{item_id}",
            axum::routing::patch(update_queued_message).delete(remove_queued_message),
        )
        .layer(from_fn_with_state(
            deployment.clone(),
            load_session_middleware,
        ))
}
