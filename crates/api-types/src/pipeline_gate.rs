use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use ts_rs::TS;
use uuid::Uuid;

/// Human decision gate used by Feature Babysitter (`human_gate` pipeline node).
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct PipelineHumanGate {
    pub id: Uuid,
    pub project_id: Uuid,
    pub issue_id: Uuid,
    pub squad_id: Option<Uuid>,
    pub gate_kind: String,
    pub local_workspace_id: Option<Uuid>,
    pub question: String,
    pub status: String,
    pub payload: Value,
    pub decision_note: Option<String>,
    pub decided_by: Option<Uuid>,
    pub decided_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Deserialize, TS)]
pub struct RespondPipelineGateRequest {
    /// `approve` or `reject`
    pub decision: String,
    #[ts(optional)]
    pub note: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct RespondPipelineGateResponse {
    pub gate: PipelineHumanGate,
}
