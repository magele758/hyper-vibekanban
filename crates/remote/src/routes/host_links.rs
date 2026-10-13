use axum::{
    Json, Router,
    extract::{Extension, Path, State},
    http::StatusCode,
    routing::{get, post},
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use uuid::Uuid;

use super::error::ErrorResponse;
use crate::{
    AppState,
    auth::RequestContext,
    db::host_links::{HostLinkRecord, HostLinkRepository},
};

#[derive(Debug, Deserialize)]
struct UpsertHostLinkBody {
    host_name: Option<String>,
    client_id: Uuid,
    client_name: Option<String>,
    public_key_b64: String,
    private_key_jwk: Value,
    server_public_key_b64: String,
    signing_session_id: Option<Uuid>,
}

#[derive(Debug, Serialize)]
struct SavedHostLinkResponse {
    saved: bool,
}

#[derive(Debug, Serialize)]
struct HostLinkResponse {
    host_id: Uuid,
    host_name: Option<String>,
    client_id: Uuid,
    client_name: Option<String>,
    public_key_b64: String,
    private_key_jwk: Value,
    server_public_key_b64: String,
    signing_session_id: Option<Uuid>,
    paired_at: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
struct ListHostLinksResponse {
    links: Vec<HostLinkResponse>,
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/hosts/links", get(list_host_links))
        .route("/hosts/{host_id}/link", post(upsert_host_link))
}

async fn list_host_links(
    State(state): State<AppState>,
    Extension(ctx): Extension<RequestContext>,
) -> Result<Json<ListHostLinksResponse>, ErrorResponse> {
    let records = HostLinkRepository::new(state.pool())
        .list_for_owner(ctx.user.id)
        .await
        .map_err(|error| {
            tracing::warn!(?error, "failed to list host links");
            ErrorResponse::new(
                StatusCode::INTERNAL_SERVER_ERROR,
                "Failed to list host links",
            )
        })?;

    Ok(Json(ListHostLinksResponse {
        links: records.into_iter().map(to_response).collect(),
    }))
}

async fn upsert_host_link(
    State(state): State<AppState>,
    Extension(ctx): Extension<RequestContext>,
    Path(host_id): Path<Uuid>,
    Json(body): Json<UpsertHostLinkBody>,
) -> Result<Json<SavedHostLinkResponse>, ErrorResponse> {
    validate_link(&body)?;
    let repo = HostLinkRepository::new(state.pool());
    let owner = repo.owner_of(host_id).await.map_err(|error| {
        tracing::warn!(?error, "failed to look up host owner");
        ErrorResponse::new(
            StatusCode::INTERNAL_SERVER_ERROR,
            "Failed to save host link",
        )
    })?;
    match owner {
        Some(owner) if owner == ctx.user.id => {}
        Some(_) => {
            return Err(ErrorResponse::new(
                StatusCode::FORBIDDEN,
                "Only the host owner can store a host link",
            ));
        }
        None => {
            return Err(ErrorResponse::new(StatusCode::NOT_FOUND, "Host not found"));
        }
    }

    let record = HostLinkRecord {
        host_id,
        host_name: body.host_name,
        client_id: body.client_id,
        client_name: body.client_name,
        public_key_b64: body.public_key_b64,
        private_key_jwk: body.private_key_jwk,
        server_public_key_b64: body.server_public_key_b64,
        signing_session_id: body.signing_session_id,
        paired_at: Utc::now(),
    };
    repo.upsert(ctx.user.id, &record).await.map_err(|error| {
        tracing::warn!(?error, "failed to save host link");
        ErrorResponse::new(
            StatusCode::INTERNAL_SERVER_ERROR,
            "Failed to save host link",
        )
    })?;

    Ok(Json(SavedHostLinkResponse { saved: true }))
}

fn validate_link(body: &UpsertHostLinkBody) -> Result<(), ErrorResponse> {
    if body.public_key_b64.trim().is_empty() || body.server_public_key_b64.trim().is_empty() {
        return Err(ErrorResponse::new(
            StatusCode::BAD_REQUEST,
            "missing signing key",
        ));
    }
    if body.public_key_b64.len() > 512 || body.server_public_key_b64.len() > 512 {
        return Err(ErrorResponse::new(
            StatusCode::BAD_REQUEST,
            "signing key is too large",
        ));
    }
    let Some(jwk) = body.private_key_jwk.as_object() else {
        return Err(ErrorResponse::new(
            StatusCode::BAD_REQUEST,
            "private_key_jwk must be an object",
        ));
    };
    if jwk_too_large(jwk) {
        return Err(ErrorResponse::new(
            StatusCode::BAD_REQUEST,
            "private_key_jwk is too large",
        ));
    }
    for field in ["kty", "crv", "d", "x"] {
        let value = jwk.get(field).and_then(Value::as_str).unwrap_or("");
        if value.is_empty() {
            return Err(ErrorResponse::new(
                StatusCode::BAD_REQUEST,
                "private_key_jwk is missing required fields",
            ));
        }
    }
    Ok(())
}

fn jwk_too_large(jwk: &Map<String, Value>) -> bool {
    serde_json::to_string(jwk)
        .map(|encoded| encoded.len() > 4096)
        .unwrap_or(true)
}

fn to_response(record: HostLinkRecord) -> HostLinkResponse {
    HostLinkResponse {
        host_id: record.host_id,
        host_name: record.host_name,
        client_id: record.client_id,
        client_name: record.client_name,
        public_key_b64: record.public_key_b64,
        private_key_jwk: record.private_key_jwk,
        server_public_key_b64: record.server_public_key_b64,
        signing_session_id: record.signing_session_id,
        paired_at: record.paired_at,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_body() -> UpsertHostLinkBody {
        UpsertHostLinkBody {
            host_name: Some("my-host".to_string()),
            client_id: Uuid::nil(),
            client_name: Some("cli".to_string()),
            public_key_b64: "cHVi".to_string(),
            private_key_jwk: serde_json::json!({
                "kty": "OKP",
                "crv": "Ed25519",
                "d": "abc",
                "x": "def"
            }),
            server_public_key_b64: "c2VydmVy".to_string(),
            signing_session_id: None,
        }
    }

    #[test]
    fn rejects_a_jwk_without_a_private_scalar() {
        let mut body = sample_body();
        body.private_key_jwk = serde_json::json!({ "kty": "OKP", "crv": "Ed25519", "x": "def" });
        assert!(validate_link(&body).is_err());
    }

    #[test]
    fn accepts_an_ed25519_jwk() {
        assert!(validate_link(&sample_body()).is_ok());
    }
}
