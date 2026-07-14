//! API types shared between local and remote backends.
//!
//! This crate contains:
//! - Row types (e.g., `Issue`, `Project`) - the API representation of database entities
//! - Request types (e.g., `CreateIssueRequest`, `UpdateIssueRequest`) - API input types
//! - Shared enums (e.g., `IssuePriority`, `PullRequestStatus`)

use serde::{Deserialize, Deserializer};

pub mod agent;
pub mod agent_task;
pub mod attachment;
pub mod auth;
pub mod autopilot;
pub mod blob;
pub mod copilot;
pub mod export;
pub mod feishu;
pub mod inbox;
pub mod issue;
pub mod issue_assignee;
pub mod issue_comment;
pub mod issue_comment_reaction;
pub mod issue_follower;
pub mod issue_relationship;
pub mod issue_tag;
pub mod notification;
pub mod oauth;
pub mod organization_member;
pub mod organizations;
pub mod pipeline_gate;
pub mod project;
pub mod project_status;
pub mod pull_request;
pub mod pull_requests_local;
pub mod response;
pub mod squad;
pub mod tag;
pub mod user;
pub mod webhook;
pub mod workspace;
pub mod workspaces;

pub use agent::*;
pub use agent_task::*;
pub use attachment::*;
pub use auth::*;
pub use autopilot::*;
pub use blob::*;
pub use copilot::*;
pub use export::*;
pub use feishu::*;
pub use inbox::*;
pub use issue::*;
pub use issue_assignee::*;
pub use issue_comment::*;
pub use issue_comment_reaction::*;
pub use issue_follower::*;
pub use issue_relationship::*;
pub use issue_tag::*;
pub use notification::*;
pub use oauth::*;
pub use organization_member::*;
pub use organizations::*;
pub use pipeline_gate::*;
pub use project::*;
pub use project_status::*;
pub use pull_request::*;
pub use pull_requests_local::*;
pub use response::*;
pub use squad::*;
pub use tag::*;
pub use user::*;
pub use webhook::*;
pub use workspace::*;
pub use workspaces::*;

pub fn some_if_present<'de, D, T>(deserializer: D) -> Result<Option<T>, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de>,
{
    T::deserialize(deserializer).map(Some)
}
