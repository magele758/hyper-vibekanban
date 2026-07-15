//! Structured jobs embedded in `agent_tasks.execution_prompt` for squad
//! pipeline `script` / `git_op` nodes. Local watcher detects the prefix and
//! runs the job instead of a coding agent.

use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Prefix marking an execution_prompt as a pipeline job (not an agent prompt).
pub const PIPELINE_JOB_PREFIX: &str = "__vk_pipeline_job__:";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PipelineJobKind {
    Script,
    GitOp,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PipelineJobSpec {
    pub kind: PipelineJobKind,
    /// Shell command for `script` nodes.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub command: Option<String>,
    /// `rebase` | `merge` | `push` | `create_pr` for `git_op` nodes.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub op: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target_branch: Option<String>,
    /// Prefer an existing local workspace from a prior coding step.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub local_workspace_id: Option<Uuid>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
}

impl PipelineJobSpec {
    pub fn encode(&self) -> Result<String, serde_json::Error> {
        Ok(format!(
            "{}{}",
            PIPELINE_JOB_PREFIX,
            serde_json::to_string(self)?
        ))
    }

    pub fn parse(prompt: &str) -> Option<Self> {
        let trimmed = prompt.trim();
        let json = trimmed.strip_prefix(PIPELINE_JOB_PREFIX)?;
        serde_json::from_str(json.trim()).ok()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_script() {
        let spec = PipelineJobSpec {
            kind: PipelineJobKind::Script,
            command: Some("pnpm run check".into()),
            op: None,
            target_branch: None,
            local_workspace_id: None,
            label: Some("verify".into()),
        };
        let encoded = spec.encode().unwrap();
        assert!(encoded.starts_with(PIPELINE_JOB_PREFIX));
        let parsed = PipelineJobSpec::parse(&encoded).unwrap();
        assert_eq!(parsed, spec);
    }
}
