use api_types::{Agent, AgentChatRuntime, AgentStatus, DeleteResponse, MutationResponse};
use chrono::{DateTime, Utc};
use sqlx::PgPool;
use thiserror::Error;
use uuid::Uuid;

use super::get_txid;

#[derive(Debug, Error)]
pub enum AgentError {
    #[error("database error: {0}")]
    Database(#[from] sqlx::Error),
}

pub struct AgentRepository;

impl AgentRepository {
    pub async fn find_by_id(pool: &PgPool, id: Uuid) -> Result<Option<Agent>, AgentError> {
        let record = sqlx::query_as!(
            Agent,
            r#"
            SELECT
                id                      AS "id!: Uuid",
                project_id              AS "project_id!: Uuid",
                name                    AS "name!",
                instructions            AS "instructions!",
                default_executor,
                max_concurrent_tasks    AS "max_concurrent_tasks!",
                status                  AS "status!: AgentStatus",
                chat_runtime            AS "chat_runtime!: AgentChatRuntime",
                reviewer_agent_id,
                created_by_user_id,
                created_at              AS "created_at!: DateTime<Utc>",
                updated_at              AS "updated_at!: DateTime<Utc>"
            FROM agents
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
    ) -> Result<Vec<Agent>, AgentError> {
        let records = sqlx::query_as!(
            Agent,
            r#"
            SELECT
                id                      AS "id!: Uuid",
                project_id              AS "project_id!: Uuid",
                name                    AS "name!",
                instructions            AS "instructions!",
                default_executor,
                max_concurrent_tasks    AS "max_concurrent_tasks!",
                status                  AS "status!: AgentStatus",
                chat_runtime            AS "chat_runtime!: AgentChatRuntime",
                reviewer_agent_id,
                created_by_user_id,
                created_at              AS "created_at!: DateTime<Utc>",
                updated_at              AS "updated_at!: DateTime<Utc>"
            FROM agents
            WHERE project_id = $1
            ORDER BY name ASC
            "#,
            project_id
        )
        .fetch_all(pool)
        .await?;

        Ok(records)
    }

    /// Roster across every project in an organization.
    ///
    /// Agents are project-scoped, but the workforce view is org-wide, so this
    /// joins the project name to avoid a per-row lookup in the UI.
    pub async fn list_by_organization(
        pool: &PgPool,
        organization_id: Uuid,
    ) -> Result<Vec<(Agent, String)>, AgentError> {
        let records = sqlx::query!(
            r#"
            SELECT
                a.id                      AS "id!: Uuid",
                a.project_id              AS "project_id!: Uuid",
                a.name                    AS "name!",
                a.instructions            AS "instructions!",
                a.default_executor,
                a.max_concurrent_tasks    AS "max_concurrent_tasks!",
                a.status                  AS "status!: AgentStatus",
                a.chat_runtime            AS "chat_runtime!: AgentChatRuntime",
                a.reviewer_agent_id,
                a.created_by_user_id,
                a.created_at              AS "created_at!: DateTime<Utc>",
                a.updated_at              AS "updated_at!: DateTime<Utc>",
                p.name                    AS "project_name!"
            FROM agents a
            JOIN projects p ON p.id = a.project_id
            WHERE p.organization_id = $1
            ORDER BY p.name ASC, a.name ASC
            "#,
            organization_id
        )
        .fetch_all(pool)
        .await?;

        Ok(records
            .into_iter()
            .map(|record| {
                (
                    Agent {
                        id: record.id,
                        project_id: record.project_id,
                        name: record.name,
                        instructions: record.instructions,
                        default_executor: record.default_executor,
                        max_concurrent_tasks: record.max_concurrent_tasks,
                        status: record.status,
                        chat_runtime: record.chat_runtime,
                        reviewer_agent_id: record.reviewer_agent_id,
                        created_by_user_id: record.created_by_user_id,
                        created_at: record.created_at,
                        updated_at: record.updated_at,
                    },
                    record.project_name,
                )
            })
            .collect())
    }

    pub async fn create(
        pool: &PgPool,
        id: Option<Uuid>,
        project_id: Uuid,
        name: String,
        instructions: String,
        default_executor: Option<String>,
        max_concurrent_tasks: i32,
        chat_runtime: AgentChatRuntime,
        reviewer_agent_id: Option<Uuid>,
        created_by_user_id: Option<Uuid>,
    ) -> Result<MutationResponse<Agent>, AgentError> {
        let id = id.unwrap_or_else(Uuid::new_v4);
        let mut tx = super::begin_tx(pool).await?;
        let data = sqlx::query_as!(
            Agent,
            r#"
            INSERT INTO agents (
                id, project_id, name, instructions, default_executor,
                max_concurrent_tasks, chat_runtime, reviewer_agent_id, created_by_user_id
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
            RETURNING
                id                      AS "id!: Uuid",
                project_id              AS "project_id!: Uuid",
                name                    AS "name!",
                instructions            AS "instructions!",
                default_executor,
                max_concurrent_tasks    AS "max_concurrent_tasks!",
                status                  AS "status!: AgentStatus",
                chat_runtime            AS "chat_runtime!: AgentChatRuntime",
                reviewer_agent_id,
                created_by_user_id,
                created_at              AS "created_at!: DateTime<Utc>",
                updated_at              AS "updated_at!: DateTime<Utc>"
            "#,
            id,
            project_id,
            name,
            instructions,
            default_executor,
            max_concurrent_tasks,
            chat_runtime as AgentChatRuntime,
            reviewer_agent_id,
            created_by_user_id
        )
        .fetch_one(&mut *tx)
        .await?;
        let txid = get_txid(&mut *tx).await?;
        tx.commit().await?;

        Ok(MutationResponse { data, txid })
    }

    pub async fn update(
        pool: &PgPool,
        id: Uuid,
        name: Option<String>,
        instructions: Option<String>,
        default_executor: Option<Option<String>>,
        max_concurrent_tasks: Option<i32>,
        status: Option<AgentStatus>,
        chat_runtime: Option<AgentChatRuntime>,
        reviewer_agent_id: Option<Option<Uuid>>,
    ) -> Result<MutationResponse<Agent>, AgentError> {
        let mut tx = super::begin_tx(pool).await?;

        // Resolve Option<Option<T>> for nullable default_executor
        let clear_executor = matches!(default_executor, Some(None));
        let set_executor = default_executor.clone().flatten();
        // Same pattern for the nullable reviewer relationship.
        let clear_reviewer = matches!(reviewer_agent_id, Some(None));
        let set_reviewer = reviewer_agent_id.flatten();

        let data = sqlx::query_as!(
            Agent,
            r#"
            UPDATE agents
            SET
                name = COALESCE($1, name),
                instructions = COALESCE($2, instructions),
                default_executor = CASE
                    WHEN $3 THEN NULL
                    WHEN $4::text IS NOT NULL THEN $4
                    ELSE default_executor
                END,
                max_concurrent_tasks = COALESCE($5, max_concurrent_tasks),
                status = COALESCE($6, status),
                chat_runtime = COALESCE($7, chat_runtime),
                reviewer_agent_id = CASE
                    WHEN $8 THEN NULL
                    WHEN $9::uuid IS NOT NULL THEN $9
                    ELSE reviewer_agent_id
                END,
                updated_at = NOW()
            WHERE id = $10
            RETURNING
                id                      AS "id!: Uuid",
                project_id              AS "project_id!: Uuid",
                name                    AS "name!",
                instructions            AS "instructions!",
                default_executor,
                max_concurrent_tasks    AS "max_concurrent_tasks!",
                status                  AS "status!: AgentStatus",
                chat_runtime            AS "chat_runtime!: AgentChatRuntime",
                reviewer_agent_id,
                created_by_user_id,
                created_at              AS "created_at!: DateTime<Utc>",
                updated_at              AS "updated_at!: DateTime<Utc>"
            "#,
            name,
            instructions,
            clear_executor,
            set_executor,
            max_concurrent_tasks,
            status as Option<AgentStatus>,
            chat_runtime as Option<AgentChatRuntime>,
            clear_reviewer,
            set_reviewer,
            id
        )
        .fetch_one(&mut *tx)
        .await?;
        let txid = get_txid(&mut *tx).await?;
        tx.commit().await?;

        Ok(MutationResponse { data, txid })
    }

    pub async fn delete(pool: &PgPool, id: Uuid) -> Result<DeleteResponse, AgentError> {
        let mut tx = super::begin_tx(pool).await?;
        sqlx::query!("DELETE FROM agents WHERE id = $1", id)
            .execute(&mut *tx)
            .await?;
        let txid = get_txid(&mut *tx).await?;
        tx.commit().await?;
        Ok(DeleteResponse { txid })
    }
}
