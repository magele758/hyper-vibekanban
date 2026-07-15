use api_types::{SquadRun, SquadRunStatus};
use sqlx::PgPool;
use thiserror::Error;
use uuid::Uuid;

const SQUAD_RUN_COLUMNS: &str = r#"
    id,
    squad_id,
    issue_id,
    status,
    start_from_node_id,
    pause_node_id,
    resume_node_id,
    approval_kind,
    approval_prompt,
    working_directory,
    error_message,
    created_by_user_id,
    started_at,
    completed_at,
    created_at,
    updated_at
"#;

#[derive(Debug, Error)]
pub enum SquadRunError {
    #[error("database error: {0}")]
    Database(#[from] sqlx::Error),
    #[error("serde error: {0}")]
    Serde(#[from] serde_json::Error),
}

pub struct SquadRunRepository;

impl SquadRunRepository {
    pub async fn create(
        pool: &PgPool,
        squad_id: Uuid,
        issue_id: Uuid,
        status: SquadRunStatus,
        start_from_node_id: Option<String>,
        working_directory: Option<String>,
        created_by_user_id: Option<Uuid>,
    ) -> Result<SquadRun, SquadRunError> {
        let id = Uuid::new_v4();
        let status_str = status.as_str();

        let run = sqlx::query_as::<_, SquadRun>(&format!(
            r#"
            INSERT INTO squad_runs (
                id, squad_id, issue_id, status,
                start_from_node_id, working_directory, created_by_user_id
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7)
            RETURNING {SQUAD_RUN_COLUMNS}
            "#
        ))
        .bind(id)
        .bind(squad_id)
        .bind(issue_id)
        .bind(status_str)
        .bind(start_from_node_id)
        .bind(working_directory)
        .bind(created_by_user_id)
        .fetch_one(pool)
        .await?;

        Ok(run)
    }

    pub async fn find_by_id(pool: &PgPool, id: Uuid) -> Result<Option<SquadRun>, SquadRunError> {
        let run = sqlx::query_as::<_, SquadRun>(&format!(
            r#"
            SELECT {SQUAD_RUN_COLUMNS}
            FROM squad_runs
            WHERE id = $1
            "#
        ))
        .bind(id)
        .fetch_optional(pool)
        .await?;

        Ok(run)
    }

    pub async fn list_by_issue(
        pool: &PgPool,
        issue_id: Uuid,
        limit: i64,
    ) -> Result<Vec<SquadRun>, SquadRunError> {
        let runs = sqlx::query_as::<_, SquadRun>(&format!(
            r#"
            SELECT {SQUAD_RUN_COLUMNS}
            FROM squad_runs
            WHERE issue_id = $1
            ORDER BY created_at DESC
            LIMIT $2
            "#
        ))
        .bind(issue_id)
        .bind(limit)
        .fetch_all(pool)
        .await?;

        Ok(runs)
    }

    pub async fn list_active_by_issue(
        pool: &PgPool,
        issue_id: Uuid,
    ) -> Result<Vec<SquadRun>, SquadRunError> {
        let runs = sqlx::query_as::<_, SquadRun>(&format!(
            r#"
            SELECT {SQUAD_RUN_COLUMNS}
            FROM squad_runs
            WHERE issue_id = $1
              AND status IN ('running', 'waiting_approval', 'queued')
            ORDER BY created_at DESC
            "#
        ))
        .bind(issue_id)
        .fetch_all(pool)
        .await?;

        Ok(runs)
    }

    pub async fn mark_waiting_approval(
        pool: &PgPool,
        id: Uuid,
        pause_node_id: String,
        resume_node_id: Option<String>,
        approval_kind: String,
        approval_prompt: String,
    ) -> Result<SquadRun, SquadRunError> {
        let run = sqlx::query_as::<_, SquadRun>(&format!(
            r#"
            UPDATE squad_runs
            SET
                status = $2,
                pause_node_id = $3,
                resume_node_id = $4,
                approval_kind = $5,
                approval_prompt = $6,
                updated_at = NOW()
            WHERE id = $1
            RETURNING {SQUAD_RUN_COLUMNS}
            "#
        ))
        .bind(id)
        .bind(SquadRunStatus::WaitingApproval.as_str())
        .bind(pause_node_id)
        .bind(resume_node_id)
        .bind(approval_kind)
        .bind(approval_prompt)
        .fetch_one(pool)
        .await?;

        Ok(run)
    }

    pub async fn mark_status(
        pool: &PgPool,
        id: Uuid,
        status: SquadRunStatus,
        error_message: Option<String>,
    ) -> Result<SquadRun, SquadRunError> {
        let status_str = status.as_str();
        let run = sqlx::query_as::<_, SquadRun>(&format!(
            r#"
            UPDATE squad_runs
            SET
                status = $2,
                error_message = $3,
                updated_at = NOW()
            WHERE id = $1
            RETURNING {SQUAD_RUN_COLUMNS}
            "#
        ))
        .bind(id)
        .bind(status_str)
        .bind(error_message)
        .fetch_one(pool)
        .await?;

        Ok(run)
    }

    pub async fn mark_completed(
        pool: &PgPool,
        id: Uuid,
        agent_task_ids: &[Uuid],
        ordered_node_ids: &[String],
    ) -> Result<SquadRun, SquadRunError> {
        let agent_task_ids_json = serde_json::to_value(agent_task_ids)?;
        let ordered_node_ids_json = serde_json::to_value(ordered_node_ids)?;

        let run = sqlx::query_as::<_, SquadRun>(&format!(
            r#"
            UPDATE squad_runs
            SET
                status = $2,
                agent_task_ids = $3,
                ordered_node_ids = $4,
                completed_at = NOW(),
                updated_at = NOW()
            WHERE id = $1
            RETURNING {SQUAD_RUN_COLUMNS}
            "#
        ))
        .bind(id)
        .bind(SquadRunStatus::Completed.as_str())
        .bind(agent_task_ids_json)
        .bind(ordered_node_ids_json)
        .fetch_one(pool)
        .await?;

        Ok(run)
    }
}
