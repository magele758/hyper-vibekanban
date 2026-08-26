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
    /// When set, a review task is enqueued for this agent after each of this
    /// agent's tasks completes successfully. Must not be self.
    pub reviewer_agent_id: Option<Uuid>,
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
    /// Optional reviewer agent; reviews this agent's completed work.
    #[serde(default)]
    #[ts(optional)]
    pub reviewer_agent_id: Option<Uuid>,
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
    #[serde(default, deserialize_with = "some_if_present")]
    pub reviewer_agent_id: Option<Option<Uuid>>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ListAgentsQuery {
    pub project_id: Uuid,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct ListAgentsResponse {
    pub agents: Vec<Agent>,
}

/// Query for the organization-wide roster.
#[derive(Debug, Clone, Deserialize)]
pub struct ListOrgAgentsQuery {
    pub organization_id: Uuid,
}

/// One configured agent plus the project it belongs to, so the roster can be
/// rendered without a second lookup per row.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct OrgAgentEntry {
    #[serde(flatten)]
    #[ts(flatten)]
    pub agent: Agent,
    pub project_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct ListOrgAgentsResponse {
    pub agents: Vec<OrgAgentEntry>,
}

/// Coding agents ("hands") a board agent may be pinned to via
/// `Agent::default_executor`.
///
/// This mirrors `executors::executors::BaseCodingAgent`. It is duplicated here
/// rather than imported because `remote` (which validates incoming writes) does
/// not depend on the `executors` crate, and cannot: the executor list describes
/// what a *local host* has installed, while remote is host-agnostic.
///
/// `server` owns a drift test asserting these two lists stay in sync.
pub const KNOWN_CODING_AGENTS: &[&str] = &[
    "CLAUDE_CODE",
    "AMP",
    "GEMINI",
    "ANTIGRAVITY",
    "CODEX",
    "OPENCODE",
    "CURSOR_AGENT",
    "QWEN_CODE",
    "COPILOT",
    "DROID",
    "PI",
    "OH_MY_PI",
    "GROK",
];

/// Historic aliases accepted on input and folded onto their canonical name.
const CODING_AGENT_ALIASES: &[(&str, &str)] = &[("CURSOR", "CURSOR_AGENT"), ("OMP", "OH_MY_PI")];

/// Normalize and validate a `default_executor` value.
///
/// Accepts any case and the aliases above; returns the canonical
/// SCREAMING_SNAKE_CASE name. `None`/blank means "unset", which is valid and
/// leaves executor choice to the host default.
pub fn normalize_default_executor(value: Option<&str>) -> Result<Option<String>, String> {
    let Some(raw) = value else {
        return Ok(None);
    };
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Ok(None);
    }

    let upper = trimmed.to_ascii_uppercase();
    let canonical = CODING_AGENT_ALIASES
        .iter()
        .find(|(alias, _)| *alias == upper)
        .map(|(_, canonical)| (*canonical).to_string())
        .or_else(|| {
            KNOWN_CODING_AGENTS
                .iter()
                .find(|known| **known == upper)
                .map(|known| (*known).to_string())
        });

    canonical.map(Some).ok_or_else(|| {
        format!(
            "unknown executor '{trimmed}'. Expected one of: {}",
            KNOWN_CODING_AGENTS.join(", ")
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unset_and_blank_are_valid_and_mean_none() {
        assert_eq!(normalize_default_executor(None), Ok(None));
        assert_eq!(normalize_default_executor(Some("")), Ok(None));
        assert_eq!(normalize_default_executor(Some("   ")), Ok(None));
    }

    #[test]
    fn canonicalizes_case_and_aliases() {
        assert_eq!(
            normalize_default_executor(Some("codex")),
            Ok(Some("CODEX".to_string()))
        );
        assert_eq!(
            normalize_default_executor(Some("  Claude_Code ")),
            Ok(Some("CLAUDE_CODE".to_string()))
        );
        // `CURSOR` is the historic name for `CURSOR_AGENT`.
        assert_eq!(
            normalize_default_executor(Some("CURSOR")),
            Ok(Some("CURSOR_AGENT".to_string()))
        );
    }

    #[test]
    fn rejects_unknown_executor_instead_of_silently_accepting() {
        let err = normalize_default_executor(Some("NOT_A_REAL_AGENT")).unwrap_err();
        assert!(err.contains("NOT_A_REAL_AGENT"));
        assert!(err.contains("CLAUDE_CODE"));
    }
}
