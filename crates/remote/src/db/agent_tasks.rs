use api_types::{AgentTask, AgentTaskStatus, AgentTaskTrigger, DeleteResponse, MutationResponse};
use chrono::{DateTime, Utc};
use sqlx::PgPool;
use thiserror::Error;
use uuid::Uuid;

use super::get_txid;

#[derive(Debug, Error)]
pub enum AgentTaskError {
    #[error("database error: {0}")]
    Database(#[from] sqlx::Error),
}

pub struct AgentTaskRepository;

impl AgentTaskRepository {
    pub async fn find_by_id(pool: &PgPool, id: Uuid) -> Result<Option<AgentTask>, AgentTaskError> {
        let record = sqlx::query_as!(
            AgentTask,
            r#"
            SELECT
                id                      AS "id!: Uuid",
                agent_id                AS "agent_id!: Uuid",
                issue_id                AS "issue_id!: Uuid",
                status                  AS "status!: AgentTaskStatus",
                trigger                 AS "trigger!: AgentTaskTrigger",
                priority                AS "priority!",
                attempt                 AS "attempt!",
                max_attempts            AS "max_attempts!",
                failure_reason,
                local_workspace_id,
                local_session_id,
                claimed_by_host,
                claimed_at,
                started_at,
                completed_at,
                force_fresh_session     AS "force_fresh_session!",
                resume_session_id,
                squad_id,
                is_leader_task          AS "is_leader_task!",
                preferred_repo_id,
                execution_prompt,
                created_at              AS "created_at!: DateTime<Utc>",
                updated_at              AS "updated_at!: DateTime<Utc>"
            FROM agent_tasks
            WHERE id = $1
            "#,
            id
        )
        .fetch_optional(pool)
        .await?;

        Ok(record)
    }

    pub async fn list_by_project(
        pool: &PgPool,
        project_id: Uuid,
    ) -> Result<Vec<AgentTask>, AgentTaskError> {
        let records = sqlx::query_as!(
            AgentTask,
            r#"
            SELECT
                t.id                      AS "id!: Uuid",
                t.agent_id                AS "agent_id!: Uuid",
                t.issue_id                AS "issue_id!: Uuid",
                t.status                  AS "status!: AgentTaskStatus",
                t.trigger                 AS "trigger!: AgentTaskTrigger",
                t.priority                AS "priority!",
                t.attempt                 AS "attempt!",
                t.max_attempts            AS "max_attempts!",
                t.failure_reason,
                t.local_workspace_id,
                t.local_session_id,
                t.claimed_by_host,
                t.claimed_at,
                t.started_at,
                t.completed_at,
                t.force_fresh_session     AS "force_fresh_session!",
                t.resume_session_id,
                t.squad_id,
                t.is_leader_task          AS "is_leader_task!",
                t.preferred_repo_id,
                t.execution_prompt,
                t.created_at              AS "created_at!: DateTime<Utc>",
                t.updated_at              AS "updated_at!: DateTime<Utc>"
            FROM agent_tasks t
            JOIN issues i ON i.id = t.issue_id
            WHERE i.project_id = $1
            ORDER BY t.created_at DESC
            "#,
            project_id
        )
        .fetch_all(pool)
        .await?;

        Ok(records)
    }

    pub async fn list_by_issue(
        pool: &PgPool,
        issue_id: Uuid,
    ) -> Result<Vec<AgentTask>, AgentTaskError> {
        let records = sqlx::query_as!(
            AgentTask,
            r#"
            SELECT
                id                      AS "id!: Uuid",
                agent_id                AS "agent_id!: Uuid",
                issue_id                AS "issue_id!: Uuid",
                status                  AS "status!: AgentTaskStatus",
                trigger                 AS "trigger!: AgentTaskTrigger",
                priority                AS "priority!",
                attempt                 AS "attempt!",
                max_attempts            AS "max_attempts!",
                failure_reason,
                local_workspace_id,
                local_session_id,
                claimed_by_host,
                claimed_at,
                started_at,
                completed_at,
                force_fresh_session     AS "force_fresh_session!",
                resume_session_id,
                squad_id,
                is_leader_task          AS "is_leader_task!",
                preferred_repo_id,
                execution_prompt,
                created_at              AS "created_at!: DateTime<Utc>",
                updated_at              AS "updated_at!: DateTime<Utc>"
            FROM agent_tasks
            WHERE issue_id = $1
            ORDER BY created_at DESC
            "#,
            issue_id
        )
        .fetch_all(pool)
        .await?;

        Ok(records)
    }

    pub async fn enqueue(
        pool: &PgPool,
        id: Option<Uuid>,
        agent_id: Uuid,
        issue_id: Uuid,
        trigger: AgentTaskTrigger,
        priority: i32,
        force_fresh_session: bool,
        squad_id: Option<Uuid>,
        is_leader_task: bool,
        preferred_repo_id: Option<String>,
        execution_prompt: Option<String>,
    ) -> Result<MutationResponse<AgentTask>, AgentTaskError> {
        let id = id.unwrap_or_else(Uuid::new_v4);
        let mut tx = super::begin_tx(pool).await?;

        // Soft-dedupe only for non-squad tasks (squad pipelines need serial/parallel
        // steps on the same agent+issue). Unique index also scopes to squad_id IS NULL.
        if squad_id.is_none() {
            if let Some(existing) = sqlx::query_as!(
                AgentTask,
                r#"
                SELECT
                    id                      AS "id!: Uuid",
                    agent_id                AS "agent_id!: Uuid",
                    issue_id                AS "issue_id!: Uuid",
                    status                  AS "status!: AgentTaskStatus",
                    trigger                 AS "trigger!: AgentTaskTrigger",
                    priority                AS "priority!",
                    attempt                 AS "attempt!",
                    max_attempts            AS "max_attempts!",
                    failure_reason,
                    local_workspace_id,
                    local_session_id,
                    claimed_by_host,
                    claimed_at,
                    started_at,
                    completed_at,
                    force_fresh_session     AS "force_fresh_session!",
                    resume_session_id,
                    squad_id,
                    is_leader_task          AS "is_leader_task!",
                    preferred_repo_id,
                    execution_prompt,
                    created_at              AS "created_at!: DateTime<Utc>",
                    updated_at              AS "updated_at!: DateTime<Utc>"
                FROM agent_tasks
                WHERE agent_id = $1
                  AND issue_id = $2
                  AND status IN ('queued', 'dispatched', 'running')
                  AND squad_id IS NULL
                LIMIT 1
                "#,
                agent_id,
                issue_id
            )
            .fetch_optional(&mut *tx)
            .await?
            {
                let txid = get_txid(&mut *tx).await?;
                tx.commit().await?;
                return Ok(MutationResponse {
                    data: existing,
                    txid,
                });
            }
        }

        let data = sqlx::query_as!(
            AgentTask,
            r#"
            INSERT INTO agent_tasks (
                id, agent_id, issue_id, trigger, priority,
                force_fresh_session, squad_id, is_leader_task, preferred_repo_id,
                execution_prompt
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
            RETURNING
                id                      AS "id!: Uuid",
                agent_id                AS "agent_id!: Uuid",
                issue_id                AS "issue_id!: Uuid",
                status                  AS "status!: AgentTaskStatus",
                trigger                 AS "trigger!: AgentTaskTrigger",
                priority                AS "priority!",
                attempt                 AS "attempt!",
                max_attempts            AS "max_attempts!",
                failure_reason,
                local_workspace_id,
                local_session_id,
                claimed_by_host,
                claimed_at,
                started_at,
                completed_at,
                force_fresh_session     AS "force_fresh_session!",
                resume_session_id,
                squad_id,
                is_leader_task          AS "is_leader_task!",
                preferred_repo_id,
                execution_prompt,
                created_at              AS "created_at!: DateTime<Utc>",
                updated_at              AS "updated_at!: DateTime<Utc>"
            "#,
            id,
            agent_id,
            issue_id,
            trigger as AgentTaskTrigger,
            priority,
            force_fresh_session,
            squad_id,
            is_leader_task,
            preferred_repo_id,
            execution_prompt
        )
        .fetch_one(&mut *tx)
        .await?;
        let txid = get_txid(&mut *tx).await?;
        tx.commit().await?;

        Ok(MutationResponse { data, txid })
    }

    /// Claim the oldest queued task the host can run (respecting agent concurrency).
    /// Automatically populates resume_session_id from the last completed task for the same
    /// (agent_id, issue_id) pair, unless force_fresh_session is true.
    pub async fn claim_next(
        pool: &PgPool,
        host_id: &str,
    ) -> Result<Option<AgentTask>, AgentTaskError> {
        let mut tx = super::begin_tx(pool).await?;

        let candidate = sqlx::query_as!(
            AgentTask,
            r#"
            SELECT
                t.id                      AS "id!: Uuid",
                t.agent_id                AS "agent_id!: Uuid",
                t.issue_id                AS "issue_id!: Uuid",
                t.status                  AS "status!: AgentTaskStatus",
                t.trigger                 AS "trigger!: AgentTaskTrigger",
                t.priority                AS "priority!",
                t.attempt                 AS "attempt!",
                t.max_attempts            AS "max_attempts!",
                t.failure_reason,
                t.local_workspace_id,
                t.local_session_id,
                t.claimed_by_host,
                t.claimed_at,
                t.started_at,
                t.completed_at,
                t.force_fresh_session     AS "force_fresh_session!",
                t.resume_session_id,
                t.squad_id,
                t.is_leader_task          AS "is_leader_task!",
                t.preferred_repo_id,
                t.execution_prompt,
                t.created_at              AS "created_at!: DateTime<Utc>",
                t.updated_at              AS "updated_at!: DateTime<Utc>"
            FROM agent_tasks t
            JOIN agents a ON a.id = t.agent_id
            WHERE t.status = 'queued'
              AND (
                SELECT COUNT(*) FROM agent_tasks active
                WHERE active.agent_id = t.agent_id
                  AND active.status IN ('dispatched', 'running')
              ) < a.max_concurrent_tasks
            ORDER BY t.priority DESC, t.created_at ASC
            LIMIT 1
            FOR UPDATE OF t SKIP LOCKED
            "#
        )
        .fetch_optional(&mut *tx)
        .await?;

        let Some(task) = candidate else {
            tx.commit().await?;
            return Ok(None);
        };

        let data = sqlx::query_as!(
            AgentTask,
            r#"
            UPDATE agent_tasks
            SET
                status = 'dispatched',
                claimed_by_host = $2,
                claimed_at = NOW(),
                attempt = attempt + 1,
                resume_session_id = CASE
                    WHEN NOT force_fresh_session THEN (
                        SELECT prev.local_session_id
                        FROM agent_tasks prev
                        WHERE prev.agent_id = agent_tasks.agent_id
                          AND prev.issue_id = agent_tasks.issue_id
                          AND prev.status = 'completed'
                          AND prev.local_session_id IS NOT NULL
                        ORDER BY prev.completed_at DESC
                        LIMIT 1
                    )
                    ELSE NULL
                END,
                updated_at = NOW()
            WHERE id = $1
            RETURNING
                id                      AS "id!: Uuid",
                agent_id                AS "agent_id!: Uuid",
                issue_id                AS "issue_id!: Uuid",
                status                  AS "status!: AgentTaskStatus",
                trigger                 AS "trigger!: AgentTaskTrigger",
                priority                AS "priority!",
                attempt                 AS "attempt!",
                max_attempts            AS "max_attempts!",
                failure_reason,
                local_workspace_id,
                local_session_id,
                claimed_by_host,
                claimed_at,
                started_at,
                completed_at,
                force_fresh_session     AS "force_fresh_session!",
                resume_session_id,
                squad_id,
                is_leader_task          AS "is_leader_task!",
                preferred_repo_id,
                execution_prompt,
                created_at              AS "created_at!: DateTime<Utc>",
                updated_at              AS "updated_at!: DateTime<Utc>"
            "#,
            task.id,
            host_id
        )
        .fetch_one(&mut *tx)
        .await?;

        tx.commit().await?;
        Ok(Some(data))
    }

    pub async fn update(
        pool: &PgPool,
        id: Uuid,
        status: Option<AgentTaskStatus>,
        failure_reason: Option<Option<String>>,
        local_workspace_id: Option<Option<Uuid>>,
        local_session_id: Option<Option<Uuid>>,
        claimed_by_host: Option<Option<String>>,
        attempt: Option<i32>,
    ) -> Result<MutationResponse<AgentTask>, AgentTaskError> {
        let mut tx = super::begin_tx(pool).await?;

        let clear_failure = matches!(failure_reason, Some(None));
        let set_failure = failure_reason.flatten();
        let clear_workspace = matches!(local_workspace_id, Some(None));
        let set_workspace = local_workspace_id.flatten();
        let clear_session = matches!(local_session_id, Some(None));
        let set_session = local_session_id.flatten();
        let clear_host = matches!(claimed_by_host, Some(None));
        let set_host = claimed_by_host.flatten();

        let terminal = matches!(
            status,
            Some(AgentTaskStatus::Completed | AgentTaskStatus::Failed | AgentTaskStatus::Cancelled)
        );
        let starting = matches!(status, Some(AgentTaskStatus::Running));

        let data = sqlx::query_as!(
            AgentTask,
            r#"
            UPDATE agent_tasks
            SET
                status = COALESCE($2, status),
                failure_reason = CASE
                    WHEN $3 THEN NULL
                    WHEN $4::text IS NOT NULL THEN $4
                    ELSE failure_reason
                END,
                local_workspace_id = CASE
                    WHEN $5 THEN NULL
                    WHEN $6::uuid IS NOT NULL THEN $6
                    ELSE local_workspace_id
                END,
                local_session_id = CASE
                    WHEN $7 THEN NULL
                    WHEN $8::uuid IS NOT NULL THEN $8
                    ELSE local_session_id
                END,
                claimed_by_host = CASE
                    WHEN $9 THEN NULL
                    WHEN $10::text IS NOT NULL THEN $10
                    ELSE claimed_by_host
                END,
                attempt = COALESCE($11, attempt),
                started_at = CASE WHEN $12 THEN COALESCE(started_at, NOW()) ELSE started_at END,
                completed_at = CASE WHEN $13 THEN NOW() ELSE completed_at END,
                updated_at = NOW()
            WHERE id = $1
            RETURNING
                id                      AS "id!: Uuid",
                agent_id                AS "agent_id!: Uuid",
                issue_id                AS "issue_id!: Uuid",
                status                  AS "status!: AgentTaskStatus",
                trigger                 AS "trigger!: AgentTaskTrigger",
                priority                AS "priority!",
                attempt                 AS "attempt!",
                max_attempts            AS "max_attempts!",
                failure_reason,
                local_workspace_id,
                local_session_id,
                claimed_by_host,
                claimed_at,
                started_at,
                completed_at,
                force_fresh_session     AS "force_fresh_session!",
                resume_session_id,
                squad_id,
                is_leader_task          AS "is_leader_task!",
                preferred_repo_id,
                execution_prompt,
                created_at              AS "created_at!: DateTime<Utc>",
                updated_at              AS "updated_at!: DateTime<Utc>"
            "#,
            id,
            status as Option<AgentTaskStatus>,
            clear_failure,
            set_failure,
            clear_workspace,
            set_workspace,
            clear_session,
            set_session,
            clear_host,
            set_host,
            attempt,
            starting,
            terminal
        )
        .fetch_one(&mut *tx)
        .await?;

        let txid = get_txid(&mut *tx).await?;
        tx.commit().await?;
        Ok(MutationResponse { data, txid })
    }

    pub async fn delete(pool: &PgPool, id: Uuid) -> Result<DeleteResponse, AgentTaskError> {
        let mut tx = super::begin_tx(pool).await?;
        sqlx::query!("DELETE FROM agent_tasks WHERE id = $1", id)
            .execute(&mut *tx)
            .await?;
        let txid = get_txid(&mut *tx).await?;
        tx.commit().await?;
        Ok(DeleteResponse { txid })
    }
}
