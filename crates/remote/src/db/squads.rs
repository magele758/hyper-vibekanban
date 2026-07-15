use api_types::{
    DeleteResponse, MutationResponse, Squad, SquadMember, SquadOnAssign, SquadPipeline,
    SquadTargetType,
};
use chrono::{DateTime, Utc};
use serde_json::Value;
use sqlx::PgPool;
use thiserror::Error;
use uuid::Uuid;

use super::get_txid;

#[derive(Debug, Error)]
pub enum SquadError {
    #[error("database error: {0}")]
    Database(#[from] sqlx::Error),
    #[error("invalid pipeline JSON: {0}")]
    InvalidPipeline(#[from] serde_json::Error),
}

fn parse_pipeline(value: Value) -> SquadPipeline {
    serde_json::from_value(value).unwrap_or_default()
}

#[derive(Debug, sqlx::FromRow)]
struct SquadRow {
    id: Uuid,
    project_id: Uuid,
    name: String,
    leader_agent_id: Option<Uuid>,
    pipeline: Value,
    target_type: String,
    issue_id: Option<Uuid>,
    working_directory: Option<String>,
    on_assign: String,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

const SQUAD_COLUMNS: &str = r#"
    id,
    project_id,
    name,
    leader_agent_id,
    pipeline,
    target_type,
    issue_id,
    working_directory,
    on_assign,
    created_at,
    updated_at
"#;

fn map_squad_row(
    id: Uuid,
    project_id: Uuid,
    name: String,
    leader_agent_id: Option<Uuid>,
    pipeline: Value,
    target_type: String,
    issue_id: Option<Uuid>,
    working_directory: Option<String>,
    on_assign: &str,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
) -> Squad {
    Squad {
        id,
        project_id,
        name,
        leader_agent_id,
        pipeline: parse_pipeline(pipeline),
        target_type: SquadTargetType::parse(&target_type),
        issue_id,
        working_directory,
        on_assign: SquadOnAssign::parse(on_assign),
        created_at,
        updated_at,
    }
}

fn squad_from_row(r: SquadRow) -> Squad {
    map_squad_row(
        r.id,
        r.project_id,
        r.name,
        r.leader_agent_id,
        r.pipeline,
        r.target_type,
        r.issue_id,
        r.working_directory,
        &r.on_assign,
        r.created_at,
        r.updated_at,
    )
}

pub struct SquadRepository;

impl SquadRepository {
    pub async fn find_by_id(pool: &PgPool, id: Uuid) -> Result<Option<Squad>, SquadError> {
        let record = sqlx::query_as::<_, SquadRow>(&format!(
            r#"
            SELECT {SQUAD_COLUMNS}
            FROM squads
            WHERE id = $1
            "#
        ))
        .bind(id)
        .fetch_optional(pool)
        .await?;

        Ok(record.map(squad_from_row))
    }

    pub async fn list_by_project(
        pool: &PgPool,
        project_id: Uuid,
    ) -> Result<Vec<Squad>, SquadError> {
        let records = sqlx::query_as::<_, SquadRow>(&format!(
            r#"
            SELECT {SQUAD_COLUMNS}
            FROM squads
            WHERE project_id = $1
            ORDER BY name ASC
            "#
        ))
        .bind(project_id)
        .fetch_all(pool)
        .await?;

        Ok(records.into_iter().map(squad_from_row).collect())
    }

    pub async fn create(
        pool: &PgPool,
        id: Option<Uuid>,
        project_id: Uuid,
        name: String,
        leader_agent_id: Option<Uuid>,
        pipeline: Option<SquadPipeline>,
        target_type: SquadTargetType,
        issue_id: Option<Uuid>,
        working_directory: Option<String>,
        on_assign: SquadOnAssign,
    ) -> Result<MutationResponse<Squad>, SquadError> {
        let id = id.unwrap_or_else(Uuid::new_v4);
        let pipeline = pipeline.unwrap_or_default();
        let pipeline_json = serde_json::to_value(&pipeline)?;
        let mut tx = super::begin_tx(pool).await?;

        let r = sqlx::query_as::<_, SquadRow>(&format!(
            r#"
            INSERT INTO squads (
                id, project_id, name, leader_agent_id, pipeline,
                target_type, issue_id, working_directory, on_assign
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
            RETURNING {SQUAD_COLUMNS}
            "#
        ))
        .bind(id)
        .bind(project_id)
        .bind(name)
        .bind(leader_agent_id)
        .bind(pipeline_json)
        .bind(target_type.as_str())
        .bind(issue_id)
        .bind(working_directory)
        .bind(on_assign.as_str())
        .fetch_one(&mut *tx)
        .await?;

        let data = squad_from_row(r);
        let txid = get_txid(&mut *tx).await?;
        tx.commit().await?;
        Ok(MutationResponse { data, txid })
    }

    pub async fn update(
        pool: &PgPool,
        id: Uuid,
        name: Option<String>,
        leader_agent_id: Option<Option<Uuid>>,
        pipeline: Option<SquadPipeline>,
        target_type: Option<SquadTargetType>,
        issue_id: Option<Option<Uuid>>,
        working_directory: Option<Option<String>>,
        on_assign: Option<SquadOnAssign>,
    ) -> Result<MutationResponse<Squad>, SquadError> {
        let mut tx = super::begin_tx(pool).await?;

        let clear_leader = matches!(leader_agent_id, Some(None));
        let set_leader = leader_agent_id.flatten();
        let clear_issue = matches!(issue_id, Some(None));
        let set_issue = issue_id.flatten();
        let clear_workdir = matches!(working_directory, Some(None));
        let set_workdir = working_directory.flatten();
        let pipeline_json = pipeline.as_ref().map(serde_json::to_value).transpose()?;
        let set_pipeline = pipeline_json.is_some();
        let set_target = target_type.is_some();
        let target_str = target_type.map(|t| t.as_str().to_string());
        let set_on_assign = on_assign.is_some();
        let on_assign_str = on_assign.map(|o| o.as_str().to_string());

        let r = sqlx::query_as::<_, SquadRow>(
            r#"
            UPDATE squads
            SET
                name = COALESCE($2, name),
                leader_agent_id = CASE
                    WHEN $3 THEN NULL
                    WHEN $4::uuid IS NOT NULL THEN $4
                    ELSE leader_agent_id
                END,
                pipeline = CASE
                    WHEN $5 THEN $6
                    ELSE pipeline
                END,
                target_type = CASE
                    WHEN $7 THEN $8
                    ELSE target_type
                END,
                issue_id = CASE
                    WHEN $9 THEN NULL
                    WHEN $10::uuid IS NOT NULL THEN $10
                    ELSE issue_id
                END,
                working_directory = CASE
                    WHEN $11 THEN NULL
                    WHEN $12::text IS NOT NULL THEN $12
                    ELSE working_directory
                END,
                on_assign = CASE
                    WHEN $13 THEN $14
                    ELSE on_assign
                END,
                updated_at = NOW()
            WHERE id = $1
            RETURNING
                id,
                project_id,
                name,
                leader_agent_id,
                pipeline,
                target_type,
                issue_id,
                working_directory,
                on_assign,
                created_at,
                updated_at
            "#,
        )
        .bind(id)
        .bind(name)
        .bind(clear_leader)
        .bind(set_leader)
        .bind(set_pipeline)
        .bind(pipeline_json)
        .bind(set_target)
        .bind(target_str)
        .bind(clear_issue)
        .bind(set_issue)
        .bind(clear_workdir)
        .bind(set_workdir)
        .bind(set_on_assign)
        .bind(on_assign_str)
        .fetch_one(&mut *tx)
        .await?;

        let data = squad_from_row(r);
        let txid = get_txid(&mut *tx).await?;
        tx.commit().await?;
        Ok(MutationResponse { data, txid })
    }

    pub async fn delete(pool: &PgPool, id: Uuid) -> Result<DeleteResponse, SquadError> {
        let mut tx = super::begin_tx(pool).await?;
        sqlx::query!("DELETE FROM squads WHERE id = $1", id)
            .execute(&mut *tx)
            .await?;
        let txid = get_txid(&mut *tx).await?;
        tx.commit().await?;
        Ok(DeleteResponse { txid })
    }

    pub async fn list_members(
        pool: &PgPool,
        squad_id: Uuid,
    ) -> Result<Vec<SquadMember>, SquadError> {
        let records = sqlx::query_as!(
            SquadMember,
            r#"
            SELECT
                id          AS "id!: Uuid",
                squad_id    AS "squad_id!: Uuid",
                agent_id,
                user_id,
                created_at  AS "created_at!: DateTime<Utc>"
            FROM squad_members
            WHERE squad_id = $1
            ORDER BY created_at ASC
            "#,
            squad_id
        )
        .fetch_all(pool)
        .await?;

        Ok(records)
    }

    pub async fn add_member(
        pool: &PgPool,
        squad_id: Uuid,
        agent_id: Option<Uuid>,
        user_id: Option<Uuid>,
    ) -> Result<MutationResponse<SquadMember>, SquadError> {
        let id = Uuid::new_v4();
        let mut tx = super::begin_tx(pool).await?;

        let data = sqlx::query_as!(
            SquadMember,
            r#"
            INSERT INTO squad_members (id, squad_id, agent_id, user_id)
            VALUES ($1, $2, $3, $4)
            RETURNING
                id          AS "id!: Uuid",
                squad_id    AS "squad_id!: Uuid",
                agent_id,
                user_id,
                created_at  AS "created_at!: DateTime<Utc>"
            "#,
            id,
            squad_id,
            agent_id,
            user_id
        )
        .fetch_one(&mut *tx)
        .await?;

        let txid = get_txid(&mut *tx).await?;
        tx.commit().await?;
        Ok(MutationResponse { data, txid })
    }

    pub async fn remove_member(
        pool: &PgPool,
        member_id: Uuid,
    ) -> Result<DeleteResponse, SquadError> {
        let mut tx = super::begin_tx(pool).await?;
        sqlx::query!("DELETE FROM squad_members WHERE id = $1", member_id)
            .execute(&mut *tx)
            .await?;
        let txid = get_txid(&mut *tx).await?;
        tx.commit().await?;
        Ok(DeleteResponse { txid })
    }
}

/// Topological sort of pipeline nodes. Falls back to declaration order on cycles.
pub fn topological_order(pipeline: &SquadPipeline) -> Vec<String> {
    use std::collections::{HashMap, HashSet, VecDeque};

    if pipeline.nodes.is_empty() {
        return Vec::new();
    }

    let node_ids: HashSet<&str> = pipeline.nodes.iter().map(|n| n.id.as_str()).collect();
    let mut indegree: HashMap<&str, usize> =
        pipeline.nodes.iter().map(|n| (n.id.as_str(), 0)).collect();
    let mut adj: HashMap<&str, Vec<&str>> = HashMap::new();

    for edge in &pipeline.edges {
        if !node_ids.contains(edge.source.as_str()) || !node_ids.contains(edge.target.as_str()) {
            continue;
        }
        adj.entry(edge.source.as_str())
            .or_default()
            .push(edge.target.as_str());
        *indegree.entry(edge.target.as_str()).or_default() += 1;
    }

    let mut queue: VecDeque<&str> = pipeline
        .nodes
        .iter()
        .filter(|n| indegree.get(n.id.as_str()).copied().unwrap_or(0) == 0)
        .map(|n| n.id.as_str())
        .collect();

    let mut ordered: Vec<String> = Vec::with_capacity(pipeline.nodes.len());
    while let Some(id) = queue.pop_front() {
        ordered.push(id.to_string());
        if let Some(neighbors) = adj.get(id) {
            for &next in neighbors {
                if let Some(d) = indegree.get_mut(next) {
                    *d = d.saturating_sub(1);
                    if *d == 0 {
                        queue.push_back(next);
                    }
                }
            }
        }
    }

    if ordered.len() < pipeline.nodes.len() {
        for n in &pipeline.nodes {
            if !ordered.iter().any(|id| id == &n.id) {
                ordered.push(n.id.clone());
            }
        }
    }

    ordered
}

#[cfg(test)]
mod tests {
    use api_types::{SquadPipelineEdge, SquadPipelineNode};

    use super::*;

    #[test]
    fn topo_respects_edges() {
        let pipeline = SquadPipeline {
            nodes: vec![
                SquadPipelineNode {
                    id: "a".into(),
                    node_type: Default::default(),
                    agent_id: None,
                    role: None,
                    prompt: None,
                    label: Some("A".into()),
                    position: None,
                    condition: None,
                    max_iterations: None,
                    wait_seconds: None,
                    wait_for: None,
                    join_count: None,
                    entry_label: None,
                    stage: None,
                    approval_kind: None,
                    prompt_template: None,
                    command: None,
                    git_op: None,
                },
                SquadPipelineNode {
                    id: "b".into(),
                    node_type: Default::default(),
                    agent_id: None,
                    role: None,
                    prompt: None,
                    label: Some("B".into()),
                    position: None,
                    condition: None,
                    max_iterations: None,
                    wait_seconds: None,
                    wait_for: None,
                    join_count: None,
                    entry_label: None,
                    stage: None,
                    approval_kind: None,
                    prompt_template: None,
                    command: None,
                    git_op: None,
                },
                SquadPipelineNode {
                    id: "c".into(),
                    node_type: Default::default(),
                    agent_id: None,
                    role: None,
                    prompt: None,
                    label: Some("C".into()),
                    position: None,
                    condition: None,
                    max_iterations: None,
                    wait_seconds: None,
                    wait_for: None,
                    join_count: None,
                    entry_label: None,
                    stage: None,
                    approval_kind: None,
                    prompt_template: None,
                    command: None,
                    git_op: None,
                },
            ],
            edges: vec![
                SquadPipelineEdge {
                    id: "e1".into(),
                    source: "a".into(),
                    target: "b".into(),
                    branch: None,
                },
                SquadPipelineEdge {
                    id: "e2".into(),
                    source: "b".into(),
                    target: "c".into(),
                    branch: None,
                },
            ],
            loop_config: None,
        };
        assert_eq!(
            topological_order(&pipeline),
            vec!["a".to_string(), "b".to_string(), "c".to_string()]
        );
    }
}
