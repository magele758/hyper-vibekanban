use anyhow;
use axum::{
    Extension, Router,
    extract::{Path, Query, State, ws::Message},
    middleware::from_fn_with_state,
    response::{IntoResponse, Json as ResponseJson},
    routing::{get, post},
};
use db::models::{
    coding_agent_turn::CodingAgentTurn,
    execution_process::{ExecutionProcess, ExecutionProcessStatus},
    execution_process_repo_state::ExecutionProcessRepoState,
};
use deployment::Deployment;
use executors::logs::{
    NormalizedEntry, NormalizedEntryType, utils::patch::extract_normalized_entry_from_patch,
};
use futures_util::{StreamExt, TryStreamExt};
use serde::{Deserialize, Serialize};
use services::services::container::ContainerService;
use utils::{log_msg::LogMsg, response::ApiResponse};
use uuid::Uuid;

use crate::{
    DeploymentImpl,
    error::ApiError,
    middleware::{
        load_execution_process_middleware,
        signed_ws::{MaybeSignedWebSocket, SignedWsUpgrade},
    },
};

#[derive(Debug, Deserialize)]
struct SessionExecutionProcessQuery {
    pub session_id: Uuid,
    /// If true, include soft-deleted (dropped) processes in results/stream
    #[serde(default)]
    pub show_soft_deleted: Option<bool>,
}

async fn get_execution_process_by_id(
    Extension(execution_process): Extension<ExecutionProcess>,
    State(_deployment): State<DeploymentImpl>,
) -> Result<ResponseJson<ApiResponse<ExecutionProcess>>, ApiError> {
    Ok(ResponseJson(ApiResponse::success(execution_process)))
}

/// Max normalized entries returned by a single `/normalized-logs` call.
const NORMALIZED_LOGS_MAX_LIMIT: usize = 500;

#[derive(Debug, Deserialize)]
struct NormalizedLogsQuery {
    /// Skip this many entries from the start (for paging through long runs).
    #[serde(default)]
    offset: Option<usize>,
    /// Max entries to return (capped at `NORMALIZED_LOGS_MAX_LIMIT`).
    #[serde(default)]
    limit: Option<usize>,
    /// When true, drop `thinking`/`loading` noise. Defaults to true.
    #[serde(default)]
    skip_noise: Option<bool>,
}

#[derive(Debug, Serialize)]
struct NormalizedLogsResponse {
    execution_id: Uuid,
    /// Total entries available after filtering, before offset/limit.
    total_count: usize,
    /// Offset applied to this page.
    offset: usize,
    /// True when more entries remain after this page.
    has_more: bool,
    /// Final assistant message persisted for this turn, when available.
    final_message: Option<String>,
    entries: Vec<NormalizedEntry>,
}

fn is_noise_entry(entry: &NormalizedEntry) -> bool {
    matches!(
        entry.entry_type,
        NormalizedEntryType::Thinking | NormalizedEntryType::Loading
    )
}

/// Collect normalized log entries for a finished (or running) execution over plain
/// HTTP. The WS variant is for live UI streaming; this is for programmatic
/// post-hoc review (e.g. an orchestrator agent inspecting another agent's run).
async fn get_normalized_logs(
    Extension(execution_process): Extension<ExecutionProcess>,
    State(deployment): State<DeploymentImpl>,
    Query(query): Query<NormalizedLogsQuery>,
) -> Result<ResponseJson<ApiResponse<NormalizedLogsResponse>>, ApiError> {
    let exec_id = execution_process.id;
    let skip_noise = query.skip_noise.unwrap_or(true);
    let offset = query.offset.unwrap_or(0);
    let limit = query
        .limit
        .unwrap_or(NORMALIZED_LOGS_MAX_LIMIT)
        .min(NORMALIZED_LOGS_MAX_LIMIT);

    // Entries are keyed by index in the patch stream; later patches for the same
    // index replace earlier ones (e.g. a tool_use transitioning to success).
    let mut by_index: std::collections::BTreeMap<usize, NormalizedEntry> = Default::default();
    if let Some(stream) = deployment
        .container()
        .stream_normalized_logs(&exec_id)
        .await
    {
        let mut stream = std::pin::pin!(stream);
        while let Some(Ok(msg)) = stream.next().await {
            match msg {
                LogMsg::Finished => break,
                LogMsg::JsonPatch(patch) => {
                    if let Some((index, entry)) = extract_normalized_entry_from_patch(&patch) {
                        by_index.insert(index, entry);
                    }
                }
                _ => {}
            }
        }
    }

    let filtered = by_index
        .into_values()
        .filter(|entry| !(skip_noise && is_noise_entry(entry)))
        .collect::<Vec<_>>();

    let total_count = filtered.len();
    let entries = filtered
        .into_iter()
        .skip(offset)
        .take(limit)
        .collect::<Vec<_>>();
    let has_more = offset.saturating_add(entries.len()) < total_count;

    let final_message =
        CodingAgentTurn::find_by_execution_process_id(&deployment.db().pool, exec_id)
            .await
            .ok()
            .flatten()
            .and_then(|turn| turn.summary);

    Ok(ResponseJson(ApiResponse::success(NormalizedLogsResponse {
        execution_id: exec_id,
        total_count,
        offset,
        has_more,
        final_message,
        entries,
    })))
}

async fn stream_raw_logs_ws(
    ws: SignedWsUpgrade,
    State(deployment): State<DeploymentImpl>,
    Path(exec_id): Path<Uuid>,
) -> impl IntoResponse {
    // Always accept the WebSocket upgrade — handle "not found" inside the
    // connection by sending `finished` and closing cleanly, instead of
    // rejecting with HTTP 404 which the browser surfaces as an opaque
    // connection failure.
    ws.on_upgrade(move |socket| async move {
        if let Err(e) = handle_raw_logs_ws(socket, deployment, exec_id).await {
            tracing::warn!("raw logs WS closed: {}", e);
        }
    })
}

async fn handle_raw_logs_ws(
    mut socket: MaybeSignedWebSocket,
    deployment: DeploymentImpl,
    exec_id: Uuid,
) -> anyhow::Result<()> {
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };

    use executors::logs::utils::patch::ConversationPatch;
    use utils::log_msg::LogMsg;

    // Get the raw stream — if not found, send finished and close cleanly
    let raw_stream = match deployment.container().stream_raw_logs(&exec_id).await {
        Some(stream) => stream,
        None => {
            // No logs available: send finished so the client gets a clean
            // close instead of retrying endlessly.
            let _ = socket
                .send(LogMsg::Finished.to_ws_message_unchecked())
                .await;
            let _ = socket.close().await;
            return Ok(());
        }
    };

    let counter = Arc::new(AtomicUsize::new(0));
    let mut stream = raw_stream.map_ok({
        let counter = counter.clone();
        move |m| match m {
            LogMsg::Stdout(content) => {
                let index = counter.fetch_add(1, Ordering::SeqCst);
                let patch = ConversationPatch::add_stdout(index, content);
                LogMsg::JsonPatch(patch).to_ws_message_unchecked()
            }
            LogMsg::Stderr(content) => {
                let index = counter.fetch_add(1, Ordering::SeqCst);
                let patch = ConversationPatch::add_stderr(index, content);
                LogMsg::JsonPatch(patch).to_ws_message_unchecked()
            }
            LogMsg::Finished => LogMsg::Finished.to_ws_message_unchecked(),
            _ => unreachable!("Raw stream should only have Stdout/Stderr/Finished"),
        }
    });

    loop {
        tokio::select! {
            item = stream.next() => {
                match item {
                    Some(Ok(msg)) => {
                        if socket.send(msg).await.is_err() {
                            break;
                        }
                    }
                    Some(Err(e)) => {
                        tracing::error!("stream error: {}", e);
                        break;
                    }
                    None => break,
                }
            }
            inbound = socket.recv() => {
                match inbound {
                    Ok(Some(Message::Close(_))) => break,
                    Ok(Some(_)) => {}
                    Ok(None) => break,
                    Err(_) => break,
                }
            }
        }
    }
    // Send a proper close frame so the client sees code 1000 (normal closure)
    // instead of an abnormal TCP drop that triggers reconnection attempts.
    let _ = socket.close().await;
    Ok(())
}

async fn stream_normalized_logs_ws(
    ws: SignedWsUpgrade,
    State(deployment): State<DeploymentImpl>,
    Path(exec_id): Path<Uuid>,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| async move {
        let stream = deployment
            .container()
            .stream_normalized_logs(&exec_id)
            .await;

        match stream {
            Some(stream) => {
                let stream = stream.err_into::<anyhow::Error>().into_stream();
                if let Err(e) = handle_normalized_logs_ws(socket, stream).await {
                    tracing::warn!("normalized logs WS closed: {}", e);
                }
            }
            None => {
                // No logs available: send finished and close cleanly
                let mut socket = socket;
                let _ = socket
                    .send(utils::log_msg::LogMsg::Finished.to_ws_message_unchecked())
                    .await;
                let _ = socket.close().await;
            }
        }
    })
}

async fn handle_normalized_logs_ws(
    mut socket: MaybeSignedWebSocket,
    stream: impl futures_util::Stream<Item = anyhow::Result<LogMsg>> + Unpin + Send + 'static,
) -> anyhow::Result<()> {
    let mut stream = stream.map_ok(|msg| msg.to_ws_message_unchecked());
    loop {
        tokio::select! {
            item = stream.next() => {
                match item {
                    Some(Ok(msg)) => {
                        if socket.send(msg).await.is_err() {
                            break;
                        }
                    }
                    Some(Err(e)) => {
                        tracing::error!("stream error: {}", e);
                        break;
                    }
                    None => break,
                }
            }
            inbound = socket.recv() => {
                match inbound {
                    Ok(Some(Message::Close(_))) => break,
                    Ok(Some(_)) => {}
                    Ok(None) => break,
                    Err(_) => break,
                }
            }
        }
    }
    let _ = socket.close().await;
    Ok(())
}

async fn stop_execution_process(
    Extension(execution_process): Extension<ExecutionProcess>,
    State(deployment): State<DeploymentImpl>,
) -> Result<ResponseJson<ApiResponse<()>>, ApiError> {
    deployment
        .container()
        .stop_execution(&execution_process, ExecutionProcessStatus::Killed)
        .await?;

    Ok(ResponseJson(ApiResponse::success(())))
}

async fn stream_execution_processes_by_session_ws(
    ws: SignedWsUpgrade,
    State(deployment): State<DeploymentImpl>,
    Query(query): Query<SessionExecutionProcessQuery>,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| async move {
        if let Err(e) = handle_execution_processes_by_session_ws(
            socket,
            deployment,
            query.session_id,
            query.show_soft_deleted.unwrap_or(false),
        )
        .await
        {
            tracing::warn!("execution processes by session WS closed: {}", e);
        }
    })
}

async fn handle_execution_processes_by_session_ws(
    mut socket: MaybeSignedWebSocket,
    deployment: DeploymentImpl,
    session_id: uuid::Uuid,
    show_soft_deleted: bool,
) -> anyhow::Result<()> {
    // Get the raw stream and convert LogMsg to WebSocket messages
    let mut stream = deployment
        .events()
        .stream_execution_processes_for_session_raw(session_id, show_soft_deleted)
        .await?
        .map_ok(|msg| msg.to_ws_message_unchecked());

    loop {
        tokio::select! {
            item = stream.next() => {
                match item {
                    Some(Ok(msg)) => {
                        if socket.send(msg).await.is_err() {
                            break;
                        }
                    }
                    Some(Err(e)) => {
                        tracing::error!("stream error: {}", e);
                        break;
                    }
                    None => break,
                }
            }
            inbound = socket.recv() => {
                match inbound {
                    Ok(Some(Message::Close(_))) => break,
                    Ok(Some(_)) => {}
                    Ok(None) => break,
                    Err(_) => break,
                }
            }
        }
    }
    Ok(())
}

async fn get_execution_process_repo_states(
    Extension(execution_process): Extension<ExecutionProcess>,
    State(deployment): State<DeploymentImpl>,
) -> Result<ResponseJson<ApiResponse<Vec<ExecutionProcessRepoState>>>, ApiError> {
    let pool = &deployment.db().pool;
    let repo_states =
        ExecutionProcessRepoState::find_by_execution_process_id(pool, execution_process.id).await?;
    Ok(ResponseJson(ApiResponse::success(repo_states)))
}

pub(super) fn router(deployment: &DeploymentImpl) -> Router<DeploymentImpl> {
    let workspace_id_router = Router::new()
        .route("/", get(get_execution_process_by_id))
        .route("/stop", post(stop_execution_process))
        .route("/repo-states", get(get_execution_process_repo_states))
        .route("/raw-logs/ws", get(stream_raw_logs_ws))
        .route("/normalized-logs", get(get_normalized_logs))
        .route("/normalized-logs/ws", get(stream_normalized_logs_ws))
        .layer(from_fn_with_state(
            deployment.clone(),
            load_execution_process_middleware,
        ));

    let workspaces_router = Router::new()
        .route(
            "/stream/session/ws",
            get(stream_execution_processes_by_session_ws),
        )
        .nest("/{id}", workspace_id_router);

    Router::new().nest("/execution-processes", workspaces_router)
}

#[cfg(test)]
mod tests {
    use executors::logs::{
        ActionType, NormalizedEntry, NormalizedEntryError, NormalizedEntryType, ToolStatus,
    };

    use super::is_noise_entry;

    fn entry(entry_type: NormalizedEntryType) -> NormalizedEntry {
        NormalizedEntry {
            timestamp: None,
            entry_type,
            content: "x".to_string(),
            metadata: None,
        }
    }

    #[test]
    fn noise_filter_drops_thinking_and_loading() {
        assert!(is_noise_entry(&entry(NormalizedEntryType::Thinking)));
        assert!(is_noise_entry(&entry(NormalizedEntryType::Loading)));
    }

    #[test]
    fn noise_filter_keeps_entries_a_reviewer_needs() {
        // A reviewing agent must still see the outcome, the tool calls, and errors.
        assert!(!is_noise_entry(&entry(
            NormalizedEntryType::AssistantMessage
        )));
        assert!(!is_noise_entry(&entry(NormalizedEntryType::ErrorMessage {
            error_type: NormalizedEntryError::Other,
        })));
        assert!(!is_noise_entry(&entry(NormalizedEntryType::ToolUse {
            tool_name: "edit".to_string(),
            action_type: ActionType::FileEdit {
                path: "src/main.rs".to_string(),
                changes: vec![],
            },
            status: ToolStatus::Success,
        })));
    }
}
