use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use ts_rs::TS;
use uuid::Uuid;

use crate::some_if_present;

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct CopilotSession {
    pub id: Uuid,
    pub project_id: Uuid,
    /// NULL = project-level Copilot; set = per-board-agent chat.
    pub agent_id: Option<Uuid>,
    pub issue_id: Option<Uuid>,
    pub created_by_user_id: Option<Uuid>,
    pub title: Option<String>,
    /// Cursor SDK agent id for Agent.resume().
    pub external_agent_id: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct CopilotMessage {
    pub id: Uuid,
    pub session_id: Uuid,
    pub role: String,
    pub content: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct CreateCopilotSessionRequest {
    #[ts(optional)]
    pub id: Option<Uuid>,
    pub project_id: Uuid,
    #[serde(default)]
    #[ts(optional)]
    pub agent_id: Option<Uuid>,
    pub issue_id: Option<Uuid>,
    pub title: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct UpdateCopilotSessionRequest {
    #[serde(
        default,
        deserialize_with = "some_if_present",
        skip_serializing_if = "Option::is_none"
    )]
    pub title: Option<String>,
    #[serde(
        default,
        deserialize_with = "some_if_present",
        skip_serializing_if = "Option::is_none"
    )]
    pub external_agent_id: Option<Option<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct CreateCopilotMessageRequest {
    #[ts(optional)]
    pub id: Option<Uuid>,
    pub session_id: Uuid,
    pub role: String,
    pub content: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ListCopilotSessionsQuery {
    pub project_id: Uuid,
    pub agent_id: Option<Uuid>,
    /// When true, only project-level sessions (agent_id IS NULL).
    pub project_copilot: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct ListCopilotSessionsResponse {
    pub copilot_sessions: Vec<CopilotSession>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ListCopilotMessagesQuery {
    pub session_id: Uuid,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct ListCopilotMessagesResponse {
    pub copilot_messages: Vec<CopilotMessage>,
}

/// Public view of LLM settings — never returns raw api_key.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct AgentLlmSettings {
    pub agent_id: Uuid,
    pub has_api_key: bool,
    pub base_url: Option<String>,
    pub model_name: Option<String>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct UpsertAgentLlmSettingsRequest {
    /// Omit to leave unchanged; empty string clears.
    #[serde(default)]
    #[ts(optional)]
    pub api_key: Option<String>,
    #[serde(default)]
    #[ts(optional)]
    pub base_url: Option<String>,
    #[serde(default)]
    #[ts(optional)]
    pub model_name: Option<String>,
}

/// Internal/sidecar view including api_key (auth required).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentLlmSettingsSecret {
    pub agent_id: Uuid,
    pub api_key: Option<String>,
    pub base_url: Option<String>,
    pub model_name: Option<String>,
}
