use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::Type;
use ts_rs::TS;
use uuid::Uuid;

use crate::some_if_present;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type, TS)]
#[sqlx(type_name = "agent_task_status", rename_all = "snake_case")]
#[serde(rename_all = "snake_case")]
pub enum AgentTaskStatus {
    Queued,
    Dispatched,
    Running,
    Completed,
    Failed,
    Cancelled,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type, TS)]
#[sqlx(type_name = "agent_task_trigger", rename_all = "snake_case")]
#[serde(rename_all = "snake_case")]
pub enum AgentTaskTrigger {
    Assign,
    Mention,
    Manual,
    Copilot,
    Autopilot,
    Feishu,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct AgentTask {
    pub id: Uuid,
    pub agent_id: Uuid,
    pub issue_id: Uuid,
    pub status: AgentTaskStatus,
    pub trigger: AgentTaskTrigger,
    pub priority: i32,
    pub attempt: i32,
    pub max_attempts: i32,
    pub failure_reason: Option<String>,
    pub local_workspace_id: Option<Uuid>,
    pub local_session_id: Option<Uuid>,
    pub claimed_by_host: Option<String>,
    pub claimed_at: Option<DateTime<Utc>>,
    pub started_at: Option<DateTime<Utc>>,
    pub completed_at: Option<DateTime<Utc>>,
    /// If true, the executor must not resume any previous session.
    pub force_fresh_session: bool,
    /// Session to resume (populated automatically during claim unless force_fresh_session).
    pub resume_session_id: Option<Uuid>,
    /// Squad this task belongs to (for leader/squad coordination).
    pub squad_id: Option<Uuid>,
    pub is_leader_task: bool,
    /// Preferred local repo path/id hint for the executor.
    pub preferred_repo_id: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct CreateAgentTaskRequest {
    #[ts(optional)]
    pub id: Option<Uuid>,
    pub agent_id: Uuid,
    pub issue_id: Uuid,
    #[ts(optional)]
    pub trigger: Option<AgentTaskTrigger>,
    #[ts(optional)]
    pub priority: Option<i32>,
    #[serde(default)]
    #[ts(optional)]
    pub force_fresh_session: Option<bool>,
    #[ts(optional)]
    pub squad_id: Option<Uuid>,
    #[serde(default)]
    #[ts(optional)]
    pub is_leader_task: Option<bool>,
    #[ts(optional)]
    pub preferred_repo_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct UpdateAgentTaskRequest {
    #[serde(
        default,
        deserialize_with = "some_if_present",
        skip_serializing_if = "Option::is_none"
    )]
    pub status: Option<AgentTaskStatus>,
    #[serde(
        default,
        deserialize_with = "some_if_present",
        skip_serializing_if = "Option::is_none"
    )]
    pub failure_reason: Option<Option<String>>,
    #[serde(
        default,
        deserialize_with = "some_if_present",
        skip_serializing_if = "Option::is_none"
    )]
    pub local_workspace_id: Option<Option<Uuid>>,
    #[serde(
        default,
        deserialize_with = "some_if_present",
        skip_serializing_if = "Option::is_none"
    )]
    pub local_session_id: Option<Option<Uuid>>,
    #[serde(
        default,
        deserialize_with = "some_if_present",
        skip_serializing_if = "Option::is_none"
    )]
    pub claimed_by_host: Option<Option<String>>,
    #[serde(
        default,
        deserialize_with = "some_if_present",
        skip_serializing_if = "Option::is_none"
    )]
    pub attempt: Option<i32>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ListAgentTasksQuery {
    pub project_id: Option<Uuid>,
    pub issue_id: Option<Uuid>,
    pub agent_id: Option<Uuid>,
    pub status: Option<AgentTaskStatus>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct ListAgentTasksResponse {
    pub agent_tasks: Vec<AgentTask>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClaimAgentTaskRequest {
    pub host_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct ClaimAgentTaskResponse {
    pub agent_task: Option<AgentTask>,
}
