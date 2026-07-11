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

    pub async fn create(
        pool: &PgPool,
        id: Option<Uuid>,
        project_id: Uuid,
        name: String,
        instructions: String,
        default_executor: Option<String>,
        max_concurrent_tasks: i32,
        chat_runtime: AgentChatRuntime,
        created_by_user_id: Option<Uuid>,
    ) -> Result<MutationResponse<Agent>, AgentError> {
        let id = id.unwrap_or_else(Uuid::new_v4);
        let mut tx = super::begin_tx(pool).await?;
        let data = sqlx::query_as!(
            Agent,
            r#"
            INSERT INTO agents (
                id, project_id, name, instructions, default_executor,
                max_concurrent_tasks, chat_runtime, created_by_user_id
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
            RETURNING
                id                      AS "id!: Uuid",
                project_id              AS "project_id!: Uuid",
                name                    AS "name!",
                instructions            AS "instructions!",
                default_executor,
                max_concurrent_tasks    AS "max_concurrent_tasks!",
                status                  AS "status!: AgentStatus",
                chat_runtime            AS "chat_runtime!: AgentChatRuntime",
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
    ) -> Result<MutationResponse<Agent>, AgentError> {
        let mut tx = super::begin_tx(pool).await?;

        // Resolve Option<Option<T>> for nullable default_executor
        let clear_executor = matches!(default_executor, Some(None));
        let set_executor = default_executor.clone().flatten();

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
                updated_at = NOW()
            WHERE id = $8
            RETURNING
                id                      AS "id!: Uuid",
                project_id              AS "project_id!: Uuid",
                name                    AS "name!",
                instructions            AS "instructions!",
                default_executor,
                max_concurrent_tasks    AS "max_concurrent_tasks!",
                status                  AS "status!: AgentStatus",
                chat_runtime            AS "chat_runtime!: AgentChatRuntime",
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
