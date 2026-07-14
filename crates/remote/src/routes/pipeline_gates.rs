use api_types::{RespondPipelineGateRequest, RespondPipelineGateResponse};
use axum::{
    Json, Router,
    extract::{Extension, Path, State},
    http::StatusCode,
    routing::post,
};
use tracing::instrument;
use uuid::Uuid;

use super::error::ErrorResponse;
use crate::{
    AppState,
    auth::RequestContext,
    db::{inbox::InboxRepository, pipeline_gates::PipelineGateRepository},
};

pub fn router() -> Router<AppState> {
    Router::new().route("/pipeline-gates/{id}/respond", post(respond_gate))
}

#[instrument(
    name = "pipeline_gates.respond",
    skip(state, ctx, payload),
    fields(user_id = %ctx.user.id, gate_id = %id)
)]
async fn respond_gate(
    State(state): State<AppState>,
    Extension(ctx): Extension<RequestContext>,
    Path(id): Path<Uuid>,
    Json(payload): Json<RespondPipelineGateRequest>,
) -> Result<Json<RespondPipelineGateResponse>, ErrorResponse> {
    let gate = PipelineGateRepository::respond(
        state.pool(),
        id,
        ctx.user.id,
        &payload.decision,
        payload.note.as_deref(),
    )
    .await
    .map_err(|e| match e {
        crate::db::pipeline_gates::PipelineGateError::NotFound => {
            ErrorResponse::new(StatusCode::NOT_FOUND, "gate not found")
        }
        crate::db::pipeline_gates::PipelineGateError::AlreadyDecided => {
            ErrorResponse::new(StatusCode::CONFLICT, "gate already decided")
        }
        crate::db::pipeline_gates::PipelineGateError::InvalidDecision => ErrorResponse::new(
            StatusCode::BAD_REQUEST,
            "decision must be approve or reject",
        ),
        crate::db::pipeline_gates::PipelineGateError::Database(err) => {
            tracing::error!(?err, "failed to respond to pipeline gate");
            ErrorResponse::new(StatusCode::INTERNAL_SERVER_ERROR, "internal server error")
        }
    })?;

    // Archive related inbox items for this user (best-effort).
    if let Ok(items) = InboxRepository::list(state.pool(), ctx.user.id, false, 100).await {
        let related: Vec<Uuid> = items
            .into_iter()
            .filter(|item| {
                item.item_type == "merge_approval"
                    && item
                        .payload
                        .get("gate_id")
                        .and_then(|v| v.as_str())
                        .and_then(|s| Uuid::parse_str(s).ok())
                        == Some(id)
            })
            .map(|item| item.id)
            .collect();
        if !related.is_empty() {
            let _ = InboxRepository::archive(state.pool(), ctx.user.id, &related).await;
        }
    }

    Ok(Json(RespondPipelineGateResponse { gate }))
}
