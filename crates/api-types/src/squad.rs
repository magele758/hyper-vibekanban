use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use ts_rs::TS;
use uuid::Uuid;

use crate::some_if_present;

/// How a squad run scopes work: Issue (goal), Path (cwd), or both.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum SquadTargetType {
    /// Use an existing project Issue as the work goal/context.
    Issue,
    /// Use a local directory as the agent working directory (creates a run Issue).
    #[default]
    Path,
    /// Issue = task goal/context; Path = agent local cwd.
    IssueAndPath,
}

impl SquadTargetType {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Issue => "issue",
            Self::Path => "path",
            Self::IssueAndPath => "issue_and_path",
        }
    }

    pub fn parse(s: &str) -> Self {
        match s {
            "issue" => Self::Issue,
            "issue_and_path" => Self::IssueAndPath,
            _ => Self::Path,
        }
    }

    pub fn uses_issue(self) -> bool {
        matches!(self, Self::Issue | Self::IssueAndPath)
    }

    pub fn uses_path(self) -> bool {
        matches!(self, Self::Path | Self::IssueAndPath)
    }
}

/// Loop / schedule settings for a squad pipeline (Claude Loops–inspired).
#[derive(Debug, Clone, Default, Serialize, Deserialize, TS, PartialEq, Eq)]
pub struct SquadLoopConfig {
    #[serde(default)]
    #[ts(optional)]
    pub max_iterations: Option<i32>,
    #[serde(default)]
    #[ts(optional)]
    pub success_condition: Option<String>,
    #[serde(default)]
    #[ts(optional)]
    pub cron_expression: Option<String>,
    #[serde(default)]
    #[ts(optional)]
    pub enabled: Option<bool>,
}

/// Canvas position for a pipeline node (free-layout editor). Optional for
/// backward compatibility — missing positions get auto-laid out in the UI.
#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq, Eq)]
pub struct SquadNodePosition {
    pub x: i32,
    pub y: i32,
}

/// Pipeline node kind. Missing / unknown values deserialize as `agent`
/// for backward compatibility with older squads.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum SquadPipelineNodeType {
    #[default]
    Agent,
    If,
    While,
    Break,
    Wait,
    /// Fan-out: walk all default outgoing edges concurrently.
    Fork,
    /// Barrier: wait until expected inbound branches complete, then continue.
    Join,
}

impl SquadPipelineNodeType {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Agent => "agent",
            Self::If => "if",
            Self::While => "while",
            Self::Break => "break",
            Self::Wait => "wait",
            Self::Fork => "fork",
            Self::Join => "join",
        }
    }

    pub fn is_agent(self) -> bool {
        matches!(self, Self::Agent)
    }

    pub fn is_control(self) -> bool {
        !self.is_agent()
    }
}

/// Labeled branch on a control-flow edge (if / while).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum SquadPipelineEdgeBranch {
    #[default]
    Default,
    True,
    False,
    Body,
    Exit,
    /// Taken when an agent step fails / times out (optional recovery path).
    Error,
}

impl SquadPipelineEdgeBranch {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Default => "default",
            Self::True => "true",
            Self::False => "false",
            Self::Body => "body",
            Self::Exit => "exit",
            Self::Error => "error",
        }
    }

    pub fn parse(s: &str) -> Self {
        match s {
            "true" => Self::True,
            "false" => Self::False,
            "body" => Self::Body,
            "exit" => Self::Exit,
            "error" => Self::Error,
            _ => Self::Default,
        }
    }
}

/// A single step / node in the squad pipeline DAG.
#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq, Eq)]
pub struct SquadPipelineNode {
    pub id: String,
    /// Node kind. Defaults to `agent` when omitted (legacy pipelines).
    #[serde(default, rename = "type")]
    #[ts(rename = "type")]
    pub node_type: SquadPipelineNodeType,
    #[serde(default)]
    #[ts(optional)]
    pub agent_id: Option<Uuid>,
    #[serde(default)]
    #[ts(optional)]
    pub role: Option<String>,
    #[serde(default)]
    #[ts(optional)]
    pub prompt: Option<String>,
    #[serde(default)]
    #[ts(optional)]
    pub label: Option<String>,
    /// Free-layout canvas coordinates; ignored by topo / run logic.
    #[serde(default)]
    #[ts(optional)]
    pub position: Option<SquadNodePosition>,
    /// Condition text for `if` / `while` (MVP: heuristic evaluation).
    #[serde(default)]
    #[ts(optional)]
    pub condition: Option<String>,
    /// Max loop iterations for `while` nodes.
    #[serde(default)]
    #[ts(optional)]
    pub max_iterations: Option<i32>,
    /// Sleep duration for `wait` nodes (seconds).
    #[serde(default)]
    #[ts(optional)]
    pub wait_seconds: Option<i32>,
    /// Optional human-readable wait reason / condition for `wait`.
    #[serde(default)]
    #[ts(optional)]
    pub wait_for: Option<String>,
    /// For `join`: require N of M inbound branches (default = all inbound edges).
    #[serde(default)]
    #[ts(optional)]
    pub join_count: Option<i32>,
}

/// Directed edge: source before target (or control-flow branch).
#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq, Eq)]
pub struct SquadPipelineEdge {
    pub id: String,
    pub source: String,
    pub target: String,
    /// Branch label for if (`true`/`false`) or while (`body`/`exit`).
    #[serde(default)]
    #[ts(optional)]
    pub branch: Option<SquadPipelineEdgeBranch>,
}

/// Editable pipeline DAG stored as JSON on the squad.
#[derive(Debug, Clone, Default, Serialize, Deserialize, TS, PartialEq, Eq)]
pub struct SquadPipeline {
    #[serde(default)]
    pub nodes: Vec<SquadPipelineNode>,
    #[serde(default)]
    pub edges: Vec<SquadPipelineEdge>,
    #[serde(default)]
    #[ts(optional)]
    pub loop_config: Option<SquadLoopConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct Squad {
    pub id: Uuid,
    pub project_id: Uuid,
    pub name: String,
    pub leader_agent_id: Option<Uuid>,
    #[serde(default)]
    pub pipeline: SquadPipeline,
    /// Work scope: issue / path / issue_and_path.
    #[serde(default)]
    pub target_type: SquadTargetType,
    /// When target includes Issue — the goal/context Issue.
    pub issue_id: Option<Uuid>,
    /// When target includes Path — local codebase/workdir for agents.
    pub working_directory: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS, sqlx::FromRow)]
pub struct SquadMember {
    pub id: Uuid,
    pub squad_id: Uuid,
    pub agent_id: Option<Uuid>,
    pub user_id: Option<Uuid>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct CreateSquadRequest {
    #[ts(optional)]
    pub id: Option<Uuid>,
    pub project_id: Uuid,
    pub name: String,
    #[ts(optional)]
    pub leader_agent_id: Option<Uuid>,
    #[serde(default)]
    #[ts(optional)]
    pub pipeline: Option<SquadPipeline>,
    #[serde(default)]
    #[ts(optional)]
    pub target_type: Option<SquadTargetType>,
    #[ts(optional)]
    pub issue_id: Option<Uuid>,
    #[ts(optional)]
    pub working_directory: Option<String>,
}

#[derive(Debug, Clone, Deserialize, TS)]
pub struct UpdateSquadRequest {
    #[serde(default, deserialize_with = "some_if_present")]
    pub name: Option<String>,
    #[serde(default, deserialize_with = "some_if_present")]
    pub leader_agent_id: Option<Option<Uuid>>,
    #[serde(default, deserialize_with = "some_if_present")]
    pub pipeline: Option<SquadPipeline>,
    #[serde(default, deserialize_with = "some_if_present")]
    pub target_type: Option<SquadTargetType>,
    #[serde(default, deserialize_with = "some_if_present")]
    pub issue_id: Option<Option<Uuid>>,
    #[serde(default, deserialize_with = "some_if_present")]
    pub working_directory: Option<Option<String>>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ListSquadsQuery {
    pub project_id: Uuid,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct ListSquadsResponse {
    pub squads: Vec<Squad>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct AddSquadMemberRequest {
    pub squad_id: Uuid,
    #[ts(optional)]
    pub agent_id: Option<Uuid>,
    #[ts(optional)]
    pub user_id: Option<Uuid>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct ListSquadMembersResponse {
    pub members: Vec<SquadMember>,
}

/// Optional overrides for POST /squads/{id}/run
#[derive(Debug, Clone, Default, Deserialize, TS)]
pub struct RunSquadRequest {
    #[ts(optional)]
    pub issue_id: Option<Uuid>,
    #[ts(optional)]
    pub working_directory: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct RunSquadResponse {
    pub issue_id: Uuid,
    pub agent_task_ids: Vec<Uuid>,
    pub ordered_node_ids: Vec<String>,
    pub target_type: SquadTargetType,
    pub working_directory: Option<String>,
}
