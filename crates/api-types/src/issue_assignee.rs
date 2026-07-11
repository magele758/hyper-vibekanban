use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use ts_rs::TS;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct IssueAssignee {
    pub id: Uuid,
    pub issue_id: Uuid,
    /// Present when the assignee is a human user.
    pub user_id: Option<Uuid>,
    /// Present when the assignee is an agent.
    pub agent_id: Option<Uuid>,
    /// Present when the assignee is a squad.
    pub squad_id: Option<Uuid>,
    pub assigned_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct CreateIssueAssigneeRequest {
    /// Optional client-generated ID. If not provided, server generates one.
    /// Using client-generated IDs enables stable optimistic updates.
    #[ts(optional)]
    pub id: Option<Uuid>,
    pub issue_id: Uuid,
    /// Assign a human user. Mutually exclusive with `agent_id` and `squad_id`.
    #[serde(default)]
    #[ts(optional)]
    pub user_id: Option<Uuid>,
    /// Assign an agent. Mutually exclusive with `user_id` and `squad_id`.
    #[serde(default)]
    #[ts(optional)]
    pub agent_id: Option<Uuid>,
    /// Assign a squad. Mutually exclusive with `user_id` and `agent_id`.
    #[serde(default)]
    #[ts(optional)]
    pub squad_id: Option<Uuid>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ListIssueAssigneesQuery {
    pub issue_id: Uuid,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct ListIssueAssigneesResponse {
    pub issue_assignees: Vec<IssueAssignee>,
}
