//! WebSocket control channel handler for local server connections.

use std::sync::Arc;

use axum::{
    Extension,
    extract::{Query, State, ws::WebSocketUpgrade},
    http::StatusCode,
    response::{IntoResponse, Response},
};
use relay_tunnel_core::server::run_control_channel;
use serde::Deserialize;
use uuid::Uuid;

use super::super::{
    auth::RequestContext,
    relay_registry::{ActiveRelay, RelayRegistry},
    state::RelayAppState,
};

#[derive(Debug, Deserialize)]
pub struct ConnectQuery {
    pub machine_id: String,
    pub name: String,
    #[serde(default)]
    pub agent_version: Option<String>,
}

/// Local server connects here to establish a relay control channel.
/// The host record is upserted from the authenticated user + machine_id query param.
pub async fn relay_connect(
    State(state): State<RelayAppState>,
    Extension(ctx): Extension<RequestContext>,
    Query(query): Query<ConnectQuery>,
    ws: WebSocketUpgrade,
) -> Response {
    let registry = state.relay_registry.clone();
    let pool = state.pool.clone();

    let host_id = match registry
        .register_host(
            &pool,
            ctx.user.id,
            &query.machine_id,
            &query.name,
            query.agent_version.as_deref(),
        )
        .await
    {
        Ok(id) => id,
        Err(error) => {
            tracing::error!(?error, "failed to upsert host for relay connect");
            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
        }
    };

    // Identifies this attempt in `hosts.relay_connection_token`. The matching
    // disconnect is the only one allowed to mark the shared row offline.
    let connection_token = Uuid::new_v4();
    if let Err(error) = registry
        .publish_online(
            &pool,
            host_id,
            query.agent_version.as_deref(),
            connection_token,
        )
        .await
    {
        tracing::warn!(?error, "failed to mark host online");
    }

    ws.on_upgrade(move |socket| async move {
        handle_control_channel(socket, pool, registry, host_id, connection_token).await;
    })
}

async fn handle_control_channel(
    socket: axum::extract::ws::WebSocket,
    pool: sqlx::PgPool,
    registry: RelayRegistry,
    host_id: Uuid,
    connection_token: Uuid,
) {
    let registry_for_connect = registry.clone();
    let connected_relay = Arc::new(tokio::sync::Mutex::new(None::<Arc<ActiveRelay>>));
    let connected_relay_for_connect = connected_relay.clone();
    let run_result = run_control_channel(socket, move |control| {
        let registry_for_connect = registry_for_connect.clone();
        let connected_relay_for_connect = connected_relay_for_connect.clone();
        async move {
            let relay = Arc::new(ActiveRelay::new(control));
            registry_for_connect.insert(host_id, relay.clone()).await;
            *connected_relay_for_connect.lock().await = Some(relay);
            tracing::debug!(%host_id, "Relay control channel connected");
        }
    })
    .await;

    if let Err(error) = run_result {
        tracing::warn!(?error, %host_id, "relay session error");
    }

    let should_mark_offline = if let Some(relay) = connected_relay.lock().await.clone() {
        registry.remove_if_same(&host_id, &relay).await
    } else {
        registry.get(&host_id).await.is_none()
    };

    if should_mark_offline {
        match registry
            .publish_offline(&pool, host_id, connection_token)
            .await
        {
            Ok(true) => {
                tracing::debug!(%host_id, "marked host offline");
            }
            Ok(false) => {
                tracing::debug!(
                    %host_id,
                    "left host online; a newer relay connection owns the session"
                );
            }
            Err(error) => {
                tracing::warn!(?error, "failed to mark host offline");
            }
        }
    } else {
        tracing::debug!(
            %host_id,
            "Relay control channel disconnected; keeping host online because a newer channel is active"
        );
    }
    tracing::debug!(%host_id, "Relay control channel disconnected");
}
