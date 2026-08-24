use std::collections::HashMap;

use axum::{
    Extension, Router,
    extract::{Query, State},
    response::Json as ResponseJson,
    routing::get,
};
use db::models::{
    coding_agent_turn::CodingAgentTurn,
    execution_process::{ExecutionProcess, ExecutionProcessRunReason, ExecutionProcessStatus},
    session::Session,
};
use deployment::Deployment;
use executors::logs::{
    NormalizedEntry, NormalizedEntryType, utils::patch::extract_normalized_entry_from_patch,
};
use futures_util::StreamExt;
use serde::{Deserialize, Serialize};
use services::services::container::ContainerService;
use ts_rs::TS;
use utils::{log_msg::LogMsg, response::ApiResponse};
use uuid::Uuid;

use crate::{DeploymentImpl, error::ApiError};

/// Query parameters for agent_session_id lookup
#[derive(Debug, Deserialize)]
struct AgentSessionIdQuery {
    /// The executor's internal session ID (e.g., "session_3n48rqp5lk_0")
    agent_session_id: String,
    /// Whether to include full entries in segments (default: true)
    #[serde(default = "default_include_entries")]
    include_entries: bool,
}

/// Query parameters for session trajectory lookup
#[derive(Debug, Deserialize)]
struct SessionTrajectoryQuery {
    /// Whether to include full entries in segments (default: false for performance)
    #[serde(default)]
    include_entries: bool,
}

fn default_include_entries() -> bool {
    true
}

/// A single execution process segment in the trajectory
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct TrajectorySegment {
    pub execution_process_id: Uuid,
    pub run_reason: ExecutionProcessRunReason,
    pub status: ExecutionProcessStatus,
    pub exit_code: Option<i64>,
    /// True if this process was excluded from current history (restore/trim)
    pub dropped: bool,
    pub started_at: String,
    pub completed_at: Option<String>,
    /// Number of normalized entries in this segment
    pub entry_count: usize,
    /// True if log files exist for this process
    pub has_logs: bool,
    /// Final assistant message/summary from coding_agent_turns
    pub final_message: Option<String>,
    /// Full entries (only included when include_entries=true)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub entries: Option<Vec<NormalizedEntry>>,
}

/// Completeness check for the session trajectory
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct TrajectoryCompleteness {
    /// Total number of execution processes in this session
    pub total_processes: usize,
    /// Number of processes with available logs
    pub with_logs: usize,
    /// Number of dropped (soft-deleted) processes
    pub dropped: usize,
    /// IDs of processes missing log files
    pub missing_logs: Vec<Uuid>,
}

/// Aggregated statistics across all segments
#[derive(Debug, Clone, Default, Serialize, Deserialize, TS)]
pub struct TrajectoryTotals {
    /// Entry counts by type
    pub entries_by_type: HashMap<String, usize>,
    /// Tool call counts by status
    pub tool_calls_by_status: HashMap<String, usize>,
    /// Last known token usage info
    pub last_token_usage: Option<TokenUsageSummary>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct TokenUsageSummary {
    pub total_tokens: u32,
    pub model_context_window: u32,
}

/// Complete trajectory response for a session
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct TrajectoryResponse {
    pub session_id: Uuid,
    pub workspace_id: Uuid,
    pub session_name: Option<String>,
    pub executor: Option<String>,
    /// Segments ordered by started_at (chronological)
    pub segments: Vec<TrajectorySegment>,
    pub completeness: TrajectoryCompleteness,
    pub totals: TrajectoryTotals,
}

/// Lookup session_id by agent_session_id, then return trajectory
async fn get_trajectory_by_agent_session_id(
    State(deployment): State<DeploymentImpl>,
    Query(query): Query<AgentSessionIdQuery>,
) -> Result<ResponseJson<ApiResponse<TrajectoryResponse>>, ApiError> {
    let pool = &deployment.db().pool;

    // Find execution_process_id via coding_agent_turns
    let turn = CodingAgentTurn::find_by_agent_session_id(pool, &query.agent_session_id)
        .await?
        .ok_or_else(|| {
            ApiError::BadRequest(format!(
                "No turn found for agent_session_id: {}",
                query.agent_session_id
            ))
        })?;

    // Find the session_id from execution_process
    let process = ExecutionProcess::find_by_id(pool, turn.execution_process_id)
        .await?
        .ok_or_else(|| {
            ApiError::BadRequest(format!(
                "Execution process {} not found",
                turn.execution_process_id
            ))
        })?;

    // Build trajectory for the session
    let trajectory =
        build_trajectory(&deployment, process.session_id, query.include_entries).await?;
    Ok(ResponseJson(ApiResponse::success(trajectory)))
}

/// Get trajectory for a session by its database ID
async fn get_trajectory_by_session_id(
    Extension(session): Extension<Session>,
    State(deployment): State<DeploymentImpl>,
    Query(query): Query<SessionTrajectoryQuery>,
) -> Result<ResponseJson<ApiResponse<TrajectoryResponse>>, ApiError> {
    let trajectory = build_trajectory(&deployment, session.id, query.include_entries).await?;
    Ok(ResponseJson(ApiResponse::success(trajectory)))
}

/// Core logic: build trajectory from session_id
async fn build_trajectory(
    deployment: &DeploymentImpl,
    session_id: Uuid,
    include_entries: bool,
) -> Result<TrajectoryResponse, ApiError> {
    let pool = &deployment.db().pool;

    // Fetch session metadata
    let session = Session::find_by_id(pool, session_id)
        .await?
        .ok_or_else(|| ApiError::BadRequest(format!("Session {} not found", session_id)))?;

    // Fetch all execution processes for this session (include dropped=true)
    let processes = ExecutionProcess::find_by_session_id(pool, session_id, true).await?;

    let mut segments = Vec::new();
    let mut completeness = TrajectoryCompleteness {
        total_processes: processes.len(),
        with_logs: 0,
        dropped: 0,
        missing_logs: Vec::new(),
    };

    let mut totals = TrajectoryTotals {
        entries_by_type: HashMap::new(),
        tool_calls_by_status: HashMap::new(),
        last_token_usage: None,
    };

    for process in processes {
        if process.dropped {
            completeness.dropped += 1;
        }

        // Try to stream normalized logs
        let log_stream = deployment
            .container()
            .stream_normalized_logs(&process.id)
            .await;
        let has_logs = log_stream.is_some();

        if has_logs {
            completeness.with_logs += 1;
        } else {
            completeness.missing_logs.push(process.id);
        }

        let (entries_opt, entry_count) = if let Some(stream) = log_stream {
            extract_entries_from_stream(stream, include_entries, &mut totals).await
        } else {
            (None, 0)
        };

        // Fetch final message from coding_agent_turns
        let final_message = CodingAgentTurn::find_by_execution_process_id(pool, process.id)
            .await
            .ok()
            .flatten()
            .and_then(|turn| turn.summary);

        segments.push(TrajectorySegment {
            execution_process_id: process.id,
            run_reason: process.run_reason,
            status: process.status,
            exit_code: process.exit_code,
            dropped: process.dropped,
            started_at: process.started_at.to_rfc3339(),
            completed_at: process.completed_at.map(|dt| dt.to_rfc3339()),
            entry_count,
            has_logs,
            final_message,
            entries: entries_opt,
        });
    }

    Ok(TrajectoryResponse {
        session_id: session.id,
        workspace_id: session.workspace_id,
        session_name: session.name.clone(),
        executor: session.executor.clone(),
        segments,
        completeness,
        totals,
    })
}

/// Extract entries from a log stream and update totals
async fn extract_entries_from_stream(
    stream: impl futures_util::Stream<Item = Result<LogMsg, std::io::Error>>,
    include_full_entries: bool,
    totals: &mut TrajectoryTotals,
) -> (Option<Vec<NormalizedEntry>>, usize) {
    let mut by_index: std::collections::BTreeMap<usize, NormalizedEntry> = Default::default();
    let mut stream = std::pin::pin!(stream);

    while let Some(Ok(msg)) = stream.next().await {
        match msg {
            LogMsg::Finished => break,
            LogMsg::JsonPatch(patch) => {
                if let Some((index, entry)) = extract_normalized_entry_from_patch(&patch) {
                    // Update totals
                    update_totals_from_entry(&entry, totals);
                    by_index.insert(index, entry);
                }
            }
            _ => {}
        }
    }

    let entries: Vec<NormalizedEntry> = by_index.into_values().collect();
    let count = entries.len();

    if include_full_entries {
        (Some(entries), count)
    } else {
        (None, count)
    }
}

/// Update totals from a single entry
fn update_totals_from_entry(entry: &NormalizedEntry, totals: &mut TrajectoryTotals) {
    // Count entry by type
    let type_key = match &entry.entry_type {
        NormalizedEntryType::UserMessage => "user_message",
        NormalizedEntryType::UserFeedback { .. } => "user_feedback",
        NormalizedEntryType::AssistantMessage => "assistant_message",
        NormalizedEntryType::ToolUse { .. } => "tool_use",
        NormalizedEntryType::SystemMessage => "system_message",
        NormalizedEntryType::ErrorMessage { .. } => "error_message",
        NormalizedEntryType::Thinking => "thinking",
        NormalizedEntryType::Loading => "loading",
        NormalizedEntryType::NextAction { .. } => "next_action",
        NormalizedEntryType::TokenUsageInfo(_) => "token_usage_info",
        NormalizedEntryType::UserAnsweredQuestions { .. } => "user_answered_questions",
    };
    *totals
        .entries_by_type
        .entry(type_key.to_string())
        .or_insert(0) += 1;

    // Count tool calls by status
    if let NormalizedEntryType::ToolUse { status, .. } = &entry.entry_type {
        let status_key = match status {
            executors::logs::ToolStatus::Created => "created",
            executors::logs::ToolStatus::Success => "success",
            executors::logs::ToolStatus::Failed => "failed",
            executors::logs::ToolStatus::Denied { .. } => "denied",
            executors::logs::ToolStatus::PendingApproval { .. } => "pending_approval",
            executors::logs::ToolStatus::TimedOut => "timed_out",
        };
        *totals
            .tool_calls_by_status
            .entry(status_key.to_string())
            .or_insert(0) += 1;
    }

    // Track last token usage
    if let NormalizedEntryType::TokenUsageInfo(usage) = &entry.entry_type {
        totals.last_token_usage = Some(TokenUsageSummary {
            total_tokens: usage.total_tokens,
            model_context_window: usage.model_context_window,
        });
    }
}

#[cfg(test)]
#[path = "trajectory_test.rs"]
mod tests;

pub fn router(deployment: &DeploymentImpl) -> Router<DeploymentImpl> {
    use axum::middleware::from_fn_with_state;

    use crate::middleware::load_session_middleware;

    Router::new()
        // Route 1: Query by agent_session_id (no auth middleware needed)
        .route("/trajectory", get(get_trajectory_by_agent_session_id))
        // Route 2: Query by session_id (requires session auth)
        .route(
            "/sessions/{session_id}/trajectory",
            get(get_trajectory_by_session_id).layer(from_fn_with_state(
                deployment.clone(),
                load_session_middleware,
            )),
        )
}
