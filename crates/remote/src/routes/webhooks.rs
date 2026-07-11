use api_types::{
    CreateWebhookEndpointRequest, DeleteResponse, ListWebhookEndpointsQuery,
    ListWebhookEndpointsResponse, WebhookEndpoint, WebhookIngressPayload,
};
use axum::{
    Json, Router,
    body::Bytes,
    extract::{Extension, Path, Query, State},
    http::{HeaderMap, StatusCode},
    routing::{delete, get, post, put},
};
use hmac::{Hmac, Mac};
use sha2::Sha256;
use tracing::instrument;
use uuid::Uuid;

use super::{
    error::{ErrorResponse, db_error},
    organization_members::ensure_project_access,
};
use crate::{
    AppState,
    auth::RequestContext,
    db::{autopilots::AutopilotRepository, webhooks::WebhookRepository},
    scheduler::dispatch_autopilot,
};

type HmacSha256 = Hmac<Sha256>;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/webhooks", get(list_webhooks).post(create_webhook))
        .route("/webhooks/{id}", delete(delete_webhook))
        .route("/webhooks/{id}/rotate-token", post(rotate_token))
        .route("/webhooks/{id}/signing-secret", put(set_signing_secret))
}

/// Public webhook ingress - token (+ optional HMAC signature).
pub fn public_router() -> Router<AppState> {
    Router::new().route("/webhooks/{token}", post(webhook_ingress))
}

#[instrument(name = "webhooks.list", skip(state, ctx), fields(user_id = %ctx.user.id))]
async fn list_webhooks(
    State(state): State<AppState>,
    Extension(ctx): Extension<RequestContext>,
    Query(query): Query<ListWebhookEndpointsQuery>,
) -> Result<Json<ListWebhookEndpointsResponse>, ErrorResponse> {
    ensure_project_access(state.pool(), ctx.user.id, query.project_id).await?;

    let endpoints = WebhookRepository::list_by_project(state.pool(), query.project_id)
        .await
        .map_err(|e| {
            tracing::error!(?e, "failed to list webhooks");
            ErrorResponse::new(StatusCode::INTERNAL_SERVER_ERROR, "failed to list webhooks")
        })?;

    Ok(Json(ListWebhookEndpointsResponse { endpoints }))
}

#[instrument(name = "webhooks.create", skip(state, ctx, payload), fields(user_id = %ctx.user.id))]
async fn create_webhook(
    State(state): State<AppState>,
    Extension(ctx): Extension<RequestContext>,
    Json(payload): Json<CreateWebhookEndpointRequest>,
) -> Result<Json<WebhookEndpoint>, ErrorResponse> {
    ensure_project_access(state.pool(), ctx.user.id, payload.project_id).await?;

    let endpoint = WebhookRepository::create(
        state.pool(),
        payload.id,
        payload.project_id,
        payload.name,
        payload.autopilot_id,
        payload.signing_secret,
    )
    .await
    .map_err(|e| db_error(e, "failed to create webhook"))?;

    Ok(Json(endpoint))
}

#[instrument(name = "webhooks.delete", skip(state, ctx), fields(id = %id, user_id = %ctx.user.id))]
async fn delete_webhook(
    State(state): State<AppState>,
    Extension(ctx): Extension<RequestContext>,
    Path(id): Path<Uuid>,
) -> Result<Json<DeleteResponse>, ErrorResponse> {
    let endpoint = WebhookRepository::find_by_id(state.pool(), id)
        .await
        .map_err(|e| {
            tracing::error!(?e, %id, "failed to load webhook");
            ErrorResponse::new(StatusCode::INTERNAL_SERVER_ERROR, "failed to load webhook")
        })?
        .ok_or_else(|| ErrorResponse::new(StatusCode::NOT_FOUND, "webhook not found"))?;

    ensure_project_access(state.pool(), ctx.user.id, endpoint.project_id).await?;

    let response = WebhookRepository::delete(state.pool(), id)
        .await
        .map_err(|e| {
            tracing::error!(?e, "failed to delete webhook");
            ErrorResponse::new(StatusCode::INTERNAL_SERVER_ERROR, "internal server error")
        })?;

    Ok(Json(response))
}

#[instrument(name = "webhooks.rotate_token", skip(state, ctx), fields(id = %id, user_id = %ctx.user.id))]
async fn rotate_token(
    State(state): State<AppState>,
    Extension(ctx): Extension<RequestContext>,
    Path(id): Path<Uuid>,
) -> Result<Json<WebhookEndpoint>, ErrorResponse> {
    let endpoint = WebhookRepository::find_by_id(state.pool(), id)
        .await
        .map_err(|e| {
            tracing::error!(?e, %id, "failed to load webhook");
            ErrorResponse::new(StatusCode::INTERNAL_SERVER_ERROR, "failed to load webhook")
        })?
        .ok_or_else(|| ErrorResponse::new(StatusCode::NOT_FOUND, "webhook not found"))?;

    ensure_project_access(state.pool(), ctx.user.id, endpoint.project_id).await?;

    let updated = WebhookRepository::rotate_token(state.pool(), id)
        .await
        .map_err(|e| db_error(e, "failed to rotate webhook token"))?;

    Ok(Json(updated))
}

#[derive(serde::Deserialize)]
struct SetSigningSecretRequest {
    signing_secret: Option<String>,
}

#[instrument(name = "webhooks.set_signing_secret", skip(state, ctx, payload), fields(id = %id, user_id = %ctx.user.id))]
async fn set_signing_secret(
    State(state): State<AppState>,
    Extension(ctx): Extension<RequestContext>,
    Path(id): Path<Uuid>,
    Json(payload): Json<SetSigningSecretRequest>,
) -> Result<StatusCode, ErrorResponse> {
    let endpoint = WebhookRepository::find_by_id(state.pool(), id)
        .await
        .map_err(|e| {
            tracing::error!(?e, %id, "failed to load webhook");
            ErrorResponse::new(StatusCode::INTERNAL_SERVER_ERROR, "failed to load webhook")
        })?
        .ok_or_else(|| ErrorResponse::new(StatusCode::NOT_FOUND, "webhook not found"))?;

    ensure_project_access(state.pool(), ctx.user.id, endpoint.project_id).await?;

    WebhookRepository::update_signing_secret(state.pool(), id, payload.signing_secret)
        .await
        .map_err(|e| db_error(e, "failed to update signing secret"))?;

    Ok(StatusCode::NO_CONTENT)
}

fn verify_signature(secret: &str, body: &[u8], headers: &HeaderMap) -> bool {
    let provided = headers
        .get("x-vk-signature")
        .or_else(|| headers.get("x-hub-signature-256"))
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");

    let provided = provided
        .strip_prefix("sha256=")
        .unwrap_or(provided)
        .trim();
    if provided.is_empty() {
        return false;
    }

    let Ok(mut mac) = HmacSha256::new_from_slice(secret.as_bytes()) else {
        return false;
    };
    mac.update(body);
    let expected = hex::encode(mac.finalize().into_bytes());
    // Constant-time compare via subtle if lengths match.
    use subtle::ConstantTimeEq;
    if expected.len() != provided.len() {
        return false;
    }
    expected.as_bytes().ct_eq(provided.as_bytes()).into()
}

/// Public webhook ingress: POST /v1/webhooks/{token}
#[instrument(name = "webhooks.ingress", skip(state, body, headers), fields(token = %token))]
async fn webhook_ingress(
    State(state): State<AppState>,
    Path(token): Path<String>,
    headers: HeaderMap,
    body: Bytes,
) -> Result<StatusCode, ErrorResponse> {
    let endpoint = WebhookRepository::find_by_token(state.pool(), &token)
        .await
        .map_err(|e| {
            tracing::error!(?e, "webhook token lookup failed");
            ErrorResponse::new(StatusCode::INTERNAL_SERVER_ERROR, "internal server error")
        })?
        .ok_or_else(|| ErrorResponse::new(StatusCode::NOT_FOUND, "webhook not found"))?;

    if !endpoint.enabled {
        return Ok(StatusCode::OK);
    }

    if let Some(secret) = endpoint.signing_secret.as_deref().filter(|s| !s.is_empty())
        && !verify_signature(secret, &body, &headers)
    {
        let _ = WebhookRepository::record_delivery(
            state.pool(),
            endpoint.id,
            None,
            String::from_utf8_lossy(&body).into_owned(),
            Some("invalid signature".into()),
            "rejected",
        )
        .await;
        return Err(ErrorResponse::new(
            StatusCode::UNAUTHORIZED,
            "invalid webhook signature",
        ));
    }

    let payload: WebhookIngressPayload = serde_json::from_slice(&body).unwrap_or_default();
    let body_str = String::from_utf8_lossy(&body).into_owned();

    let _ = WebhookRepository::record_delivery(
        state.pool(),
        endpoint.id,
        payload.dedupe_key.clone(),
        body_str,
        None,
        "received",
    )
    .await;

    if let Some(autopilot_id) = endpoint.autopilot_id
        && let Ok(Some(ap)) = AutopilotRepository::find_by_id(state.pool(), autopilot_id).await
        && ap.enabled
    {
        let pool = state.pool().clone();
        tokio::spawn(async move {
            if let Err(e) = dispatch_autopilot(&pool, &ap).await {
                tracing::error!(?e, %autopilot_id, "webhook-triggered autopilot dispatch failed");
            }
        });
    }

    Ok(StatusCode::OK)
}
