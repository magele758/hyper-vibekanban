use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::Type;
use ts_rs::TS;
use uuid::Uuid;

use crate::some_if_present;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type, TS)]
#[sqlx(type_name = "autopilot_execution_mode", rename_all = "snake_case")]
#[serde(rename_all = "snake_case")]
pub enum AutopilotExecutionMode {
    CreateIssue,
    RunOnly,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type, TS)]
#[sqlx(type_name = "autopilot_concurrency_policy", rename_all = "snake_case")]
#[serde(rename_all = "snake_case")]
pub enum AutopilotConcurrencyPolicy {
    Skip,
    Queue,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type, TS)]
#[sqlx(type_name = "autopilot_run_status", rename_all = "snake_case")]
#[serde(rename_all = "snake_case")]
pub enum AutopilotRunStatus {
    Queued,
    Running,
    Completed,
    Failed,
    Skipped,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct Autopilot {
    pub id: Uuid,
    pub project_id: Uuid,
    pub name: String,
    pub agent_id: Option<Uuid>,
    /// When set, scheduler runs the squad pipeline instead of a single agent.
    pub squad_id: Option<Uuid>,
    pub enabled: bool,
    pub execution_mode: AutopilotExecutionMode,
    pub cron_expression: String,
    pub timezone: String,
    pub concurrency_policy: AutopilotConcurrencyPolicy,
    pub issue_title_template: String,
    pub issue_description_template: String,
    pub next_run_at: Option<DateTime<Utc>>,
    pub last_run_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct AutopilotRun {
    pub id: Uuid,
    pub autopilot_id: Uuid,
    pub status: AutopilotRunStatus,
    pub planned_at: DateTime<Utc>,
    pub started_at: Option<DateTime<Utc>>,
    pub completed_at: Option<DateTime<Utc>>,
    pub issue_id: Option<Uuid>,
    pub agent_task_id: Option<Uuid>,
    pub error_message: Option<String>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct CreateAutopilotRequest {
    #[ts(optional)]
    pub id: Option<Uuid>,
    pub project_id: Uuid,
    pub name: String,
    #[ts(optional)]
    pub agent_id: Option<Uuid>,
    #[ts(optional)]
    pub squad_id: Option<Uuid>,
    #[serde(default = "default_true")]
    #[ts(optional)]
    pub enabled: Option<bool>,
    #[ts(optional)]
    pub execution_mode: Option<AutopilotExecutionMode>,
    #[ts(optional)]
    pub cron_expression: Option<String>,
    #[ts(optional)]
    pub timezone: Option<String>,
    #[ts(optional)]
    pub concurrency_policy: Option<AutopilotConcurrencyPolicy>,
    #[ts(optional)]
    pub issue_title_template: Option<String>,
    #[ts(optional)]
    pub issue_description_template: Option<String>,
}

fn default_true() -> Option<bool> {
    Some(true)
}

#[derive(Debug, Clone, Deserialize, TS)]
pub struct UpdateAutopilotRequest {
    #[serde(default, deserialize_with = "some_if_present")]
    pub name: Option<String>,
    #[serde(default, deserialize_with = "some_if_present")]
    pub agent_id: Option<Option<Uuid>>,
    #[serde(default, deserialize_with = "some_if_present")]
    pub squad_id: Option<Option<Uuid>>,
    #[serde(default, deserialize_with = "some_if_present")]
    pub enabled: Option<bool>,
    #[serde(default, deserialize_with = "some_if_present")]
    pub execution_mode: Option<AutopilotExecutionMode>,
    #[serde(default, deserialize_with = "some_if_present")]
    pub cron_expression: Option<String>,
    #[serde(default, deserialize_with = "some_if_present")]
    pub timezone: Option<String>,
    #[serde(default, deserialize_with = "some_if_present")]
    pub concurrency_policy: Option<AutopilotConcurrencyPolicy>,
    #[serde(default, deserialize_with = "some_if_present")]
    pub issue_title_template: Option<String>,
    #[serde(default, deserialize_with = "some_if_present")]
    pub issue_description_template: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ListAutopilotQuery {
    pub project_id: Uuid,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct ListAutopilotResponse {
    pub autopilots: Vec<Autopilot>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct ListAutopilotRunsResponse {
    pub runs: Vec<AutopilotRun>,
}
