use axum::{
    Router,
    body::{Body, to_bytes},
    extract::{
        Path, Request, State,
        ws::{WebSocketUpgrade, rejection::WebSocketUpgradeRejection},
    },
    http::{HeaderMap, StatusCode, Uri},
    response::{IntoResponse, Response},
    routing::any,
};
use deployment::Deployment;
use hyper::{Request as HyperRequest, body::Incoming, client::conn::http1 as client_http1};
use hyper_util::rt::TokioIo;
use relay_client::RELAY_HEADER;
use relay_control::signing::{
    NONCE_HEADER, REQUEST_SIGNATURE_HEADER, SIGNING_SESSION_HEADER, TIMESTAMP_HEADER,
};
use relay_hosts::ProxiedResponse;
use tokio::net::TcpStream;
use url::form_urlencoded;
use utils::http_headers::is_hop_by_hop_header;
use uuid::Uuid;
use ws_bridge::{bridge_axum_ws, connect_upstream_ws};

use crate::{DeploymentImpl, error::ApiError};

type MaybeWsUpgrade = Result<WebSocketUpgrade, WebSocketUpgradeRejection>;
type HyperResponse = hyper::Response<Incoming>;

pub(super) fn router() -> Router<DeploymentImpl> {
    Router::new().route("/host/{host_id}/{*tail}", any(proxy_host_request))
}

async fn proxy_host_request(
    State(deployment): State<DeploymentImpl>,
    Path((host_id, tail)): Path<(Uuid, String)>,
    ws_upgrade: MaybeWsUpgrade,
    mut request: Request,
) -> Result<Response, ApiError> {
    let query = request.uri().query().map(str::to_owned);
    let upstream_uri = upstream_api_uri(&tail, query.as_deref())?;
    *request.uri_mut() = upstream_uri;

    match ws_upgrade {
        Ok(ws_upgrade) => forward_ws(&deployment, host_id, request, ws_upgrade).await,
        Err(_) => forward_http(&deployment, host_id, request).await,
    }
}

async fn forward_http(
    deployment: &DeploymentImpl,
    host_id: Uuid,
    request: Request,
) -> Result<Response, ApiError> {
    if is_inbound_relay_tunnel_request(&request) {
        let response = send_loopback_request(deployment, request).await?;
        return Ok(hyper_response_to_axum(response));
    }

    let relay_hosts = deployment.relay_hosts()?;
    let (parts, body) = request.into_parts();
    let method = parts.method;
    let headers = parts.headers;
    let target_path = parts
        .uri
        .path_and_query()
        .map(|pq| pq.as_str())
        .unwrap_or("/")
        .to_string();
    let body_bytes = to_bytes(body, usize::MAX).await.map_err(|error| {
        tracing::warn!(?error, "Failed to read relay proxy request body");
        ApiError::BadRequest("Invalid request body".to_string())
    })?;
    let relay_host = relay_hosts.host(host_id).await?;
    let response = relay_host
        .proxy_http(&method, &target_path, &headers, &body_bytes)
        .await?;

    Ok(relay_http_response(response))
}

async fn forward_ws(
    deployment: &DeploymentImpl,
    host_id: Uuid,
    request: Request,
    ws_upgrade: WebSocketUpgrade,
) -> Result<Response, ApiError> {
    if is_inbound_relay_tunnel_request(&request) {
        let protocols = request
            .headers()
            .get("sec-websocket-protocol")
            .and_then(|v| v.to_str().ok())
            .map(str::to_owned);
        let path_and_query = request
            .uri()
            .path_and_query()
            .map(|pq| pq.as_str())
            .unwrap_or("/");
        let sanitized_path = strip_relay_signing_query(path_and_query);
        let server_addr = deployment.client_info().get_server_addr().ok_or_else(|| {
            ApiError::BadGateway("Local server address is not available".to_string())
        })?;
        let ws_url = format!("ws://{server_addr}{sanitized_path}");

        let mut ws = ws_upgrade;
        if let Some(protocol) = protocols.as_deref() {
            ws = ws.protocols([protocol.to_string()]);
        }

        let (upstream, selected_protocol) = connect_upstream_ws(ws_url, protocols.as_deref())
            .await
            .map_err(|error| {
                tracing::warn!(?error, "Relay loopback WebSocket connect failed");
                ApiError::BadGateway("Local WebSocket proxy failed".to_string())
            })?;
        if let Some(protocol) = selected_protocol {
            ws = ws.protocols([protocol]);
        }

        return Ok(ws
            .on_upgrade(move |client_socket| async move {
                if let Err(error) = bridge_axum_ws(client_socket, upstream).await {
                    tracing::debug!(?error, "Relay loopback WS bridge closed with error");
                }
            })
            .into_response());
    }

    let relay_hosts = deployment.relay_hosts()?;
    let target_path = request
        .uri()
        .path_and_query()
        .map(|pq| pq.as_str())
        .unwrap_or("/")
        .to_string();
    let protocols = request
        .headers()
        .get("sec-websocket-protocol")
        .and_then(|v| v.to_str().ok())
        .map(ToOwned::to_owned);
    let relay_host = relay_hosts.host(host_id).await?;

    let connection = relay_host
        .proxy_ws(&target_path, protocols.as_deref())
        .await?;
    let selected_protocol = connection.selected_protocol.clone();

    let mut ws = ws_upgrade;
    if let Some(protocol) = &selected_protocol {
        ws = ws.protocols([protocol.clone()]);
    }
    Ok(ws
        .on_upgrade(|socket| async move {
            if let Err(error) = connection.bridge(socket).await {
                tracing::debug!(?error, "WS bridge closed with error");
            }
        })
        .into_response())
}

async fn send_loopback_request(
    deployment: &DeploymentImpl,
    request: Request,
) -> Result<HyperResponse, ApiError> {
    let server_addr = deployment
        .client_info()
        .get_server_addr()
        .ok_or_else(|| ApiError::BadGateway("Local server address is not available".to_string()))?;

    let (mut parts, body) = request.into_parts();
    strip_relay_loopback_headers(&mut parts.headers);
    let path_and_query = parts
        .uri
        .path_and_query()
        .map(|pq| pq.as_str())
        .unwrap_or("/");
    parts.uri = strip_relay_signing_query(path_and_query)
        .parse()
        .map_err(|_| ApiError::BadRequest("Invalid relay loopback path".to_string()))?;

    let body_bytes = to_bytes(body, usize::MAX).await.map_err(|error| {
        tracing::warn!(?error, "Failed to read relay loopback request body");
        ApiError::BadRequest("Invalid request body".to_string())
    })?;

    let local_stream = TcpStream::connect(server_addr).await.map_err(|error| {
        tracing::warn!(
            ?error,
            "Failed to connect to local server for relay loopback"
        );
        ApiError::BadGateway("Failed to connect to local server".to_string())
    })?;

    let (mut sender, connection) = client_http1::Builder::new()
        .handshake(TokioIo::new(local_stream))
        .await
        .map_err(|error| {
            tracing::warn!(
                ?error,
                "Failed to initialize relay loopback HTTP connection"
            );
            ApiError::BadGateway("Failed to initialize local proxy connection".to_string())
        })?;

    tokio::spawn(async move {
        if let Err(error) = connection.with_upgrades().await {
            tracing::debug!(?error, "Relay loopback connection closed");
        }
    });

    let outbound = HyperRequest::from_parts(parts, Body::from(body_bytes));
    sender.send_request(outbound).await.map_err(|error| {
        tracing::warn!(?error, "Relay loopback request failed");
        ApiError::BadGateway("Local proxy request failed".to_string())
    })
}

fn hyper_response_to_axum(response: HyperResponse) -> Response {
    let (parts, body) = response.into_parts();
    Response::from_parts(parts, Body::new(body))
}

fn is_inbound_relay_tunnel_request<B>(request: &Request<B>) -> bool {
    request
        .headers()
        .get(RELAY_HEADER)
        .and_then(|value| value.to_str().ok())
        .is_some_and(|value| value.trim() == "1")
}

fn strip_relay_loopback_headers(headers: &mut HeaderMap) {
    headers.remove(RELAY_HEADER);
    headers.remove(SIGNING_SESSION_HEADER);
    headers.remove(TIMESTAMP_HEADER);
    headers.remove(NONCE_HEADER);
    headers.remove(REQUEST_SIGNATURE_HEADER);
}

fn strip_relay_signing_query(path_and_query: &str) -> String {
    let (path, query) = match path_and_query.split_once('?') {
        Some((path, query)) => (path, query),
        None => return path_and_query.to_string(),
    };

    let mut filtered_query = form_urlencoded::Serializer::new(String::new());
    for (key, value) in form_urlencoded::parse(query.as_bytes()) {
        match key.as_ref() {
            SIGNING_SESSION_HEADER | TIMESTAMP_HEADER | NONCE_HEADER | REQUEST_SIGNATURE_HEADER => {
            }
            _ => {
                filtered_query.append_pair(&key, &value);
            }
        }
    }

    let filtered = filtered_query.finish();
    if filtered.is_empty() {
        path.to_string()
    } else {
        format!("{path}?{filtered}")
    }
}

#[allow(clippy::result_large_err)]
fn upstream_api_uri(tail: &str, query: Option<&str>) -> Result<Uri, ApiError> {
    let mut uri = String::from("/api/");
    uri.push_str(tail);

    if let Some(query) = query {
        uri.push('?');
        uri.push_str(query);
    }

    uri.parse()
        .map_err(|_| ApiError::BadRequest("Invalid rewritten relay path".to_string()))
}

fn relay_http_response(response: ProxiedResponse) -> Response {
    let mut builder = Response::builder().status(response.status);
    for (name, value) in &response.headers {
        if !is_hop_by_hop_header(name.as_str()) {
            builder = builder.header(name, value);
        }
    }

    builder
        .body(Body::from_stream(response.body))
        .unwrap_or_else(|_| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                "Failed to build relay proxy response",
            )
                .into_response()
        })
}
