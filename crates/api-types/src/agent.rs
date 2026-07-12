use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::Type;
use ts_rs::TS;
use uuid::Uuid;

use crate::some_if_present;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type, TS)]
#[sqlx(type_name = "agent_status", rename_all = "snake_case")]
#[serde(rename_all = "snake_case")]
pub enum AgentStatus {
    Idle,
    Working,
    Offline,
    Error,
}

/// Board-agent chat runtime (sidecar adapter). Coding executors are separate.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type, TS, Default)]
#[sqlx(type_name = "agent_chat_runtime", rename_all = "snake_case")]
#[serde(rename_all = "snake_case")]
pub enum AgentChatRuntime {
    #[default]
    Cursor,
    Pi,
    Opencode,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct Agent {
    pub id: Uuid,
    pub project_id: Uuid,
    pub name: String,
    pub instructions: String,
    pub default_executor: Option<String>,
    pub max_concurrent_tasks: i32,
    pub status: AgentStatus,
    pub chat_runtime: AgentChatRuntime,
    pub created_by_user_id: Option<Uuid>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct CreateAgentRequest {
    #[ts(optional)]
    pub id: Option<Uuid>,
    pub project_id: Uuid,
    pub name: String,
    #[serde(default)]
    pub instructions: String,
    pub default_executor: Option<String>,
    #[ts(optional)]
    pub max_concurrent_tasks: Option<i32>,
    #[serde(default)]
    #[ts(optional)]
    pub chat_runtime: Option<AgentChatRuntime>,
    /// Optional Cursor SDK credentials set at create time.
    #[serde(default)]
    #[ts(optional)]
    pub api_key: Option<String>,
    #[serde(default)]
    #[ts(optional)]
    pub base_url: Option<String>,
    #[serde(default)]
    #[ts(optional)]
    pub model_name: Option<String>,
    /// Optional local cwd for Cursor SDK file ops.
    #[serde(default)]
    #[ts(optional)]
    pub working_directory: Option<String>,
}

#[derive(Debug, Clone, Deserialize, TS)]
pub struct UpdateAgentRequest {
    #[serde(default, deserialize_with = "some_if_present")]
    pub name: Option<String>,
    #[serde(default, deserialize_with = "some_if_present")]
    pub instructions: Option<String>,
    #[serde(default, deserialize_with = "some_if_present")]
    pub default_executor: Option<Option<String>>,
    #[serde(default, deserialize_with = "some_if_present")]
    pub max_concurrent_tasks: Option<i32>,
    #[serde(default, deserialize_with = "some_if_present")]
    pub status: Option<AgentStatus>,
    #[serde(default, deserialize_with = "some_if_present")]
    pub chat_runtime: Option<AgentChatRuntime>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ListAgentsQuery {
    pub project_id: Uuid,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct ListAgentsResponse {
    pub agents: Vec<Agent>,
}
