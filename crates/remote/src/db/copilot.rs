use api_types::{
    AgentLlmSettings, AgentLlmSettingsSecret, CopilotMessage, CopilotSession, DeleteResponse,
    MutationResponse,
};
use chrono::{DateTime, Utc};
use sqlx::PgPool;
use thiserror::Error;
use uuid::Uuid;

use super::get_txid;

#[derive(Debug, Error)]
pub enum CopilotError {
    #[error("database error: {0}")]
    Database(#[from] sqlx::Error),
}

pub struct CopilotRepository;

impl CopilotRepository {
    pub async fn find_session_by_id(
        pool: &PgPool,
        id: Uuid,
    ) -> Result<Option<CopilotSession>, CopilotError> {
        let record = sqlx::query_as!(
            CopilotSession,
            r#"
            SELECT
                id                  AS "id!: Uuid",
                project_id          AS "project_id!: Uuid",
                agent_id,
                issue_id,
                created_by_user_id,
                title,
                external_agent_id,
                created_at          AS "created_at!: DateTime<Utc>",
                updated_at          AS "updated_at!: DateTime<Utc>"
            FROM copilot_sessions
            WHERE id = $1
            "#,
            id
        )
        .fetch_optional(pool)
        .await?;
        Ok(record)
    }

    pub async fn list_sessions(
        pool: &PgPool,
        project_id: Uuid,
        agent_id: Option<Uuid>,
        project_copilot: bool,
    ) -> Result<Vec<CopilotSession>, CopilotError> {
        let records = if project_copilot {
            sqlx::query_as!(
                CopilotSession,
                r#"
                SELECT
                    id                  AS "id!: Uuid",
                    project_id          AS "project_id!: Uuid",
                    agent_id,
                    issue_id,
                    created_by_user_id,
                    title,
                    external_agent_id,
                    created_at          AS "created_at!: DateTime<Utc>",
                    updated_at          AS "updated_at!: DateTime<Utc>"
                FROM copilot_sessions
                WHERE project_id = $1 AND agent_id IS NULL
                ORDER BY updated_at DESC
                "#,
                project_id
            )
            .fetch_all(pool)
            .await?
        } else if let Some(agent_id) = agent_id {
            sqlx::query_as!(
                CopilotSession,
                r#"
                SELECT
                    id                  AS "id!: Uuid",
                    project_id          AS "project_id!: Uuid",
                    agent_id,
                    issue_id,
                    created_by_user_id,
                    title,
                    external_agent_id,
                    created_at          AS "created_at!: DateTime<Utc>",
                    updated_at          AS "updated_at!: DateTime<Utc>"
                FROM copilot_sessions
                WHERE project_id = $1 AND agent_id = $2
                ORDER BY updated_at DESC
                "#,
                project_id,
                agent_id
            )
            .fetch_all(pool)
            .await?
        } else {
            sqlx::query_as!(
                CopilotSession,
                r#"
                SELECT
                    id                  AS "id!: Uuid",
                    project_id          AS "project_id!: Uuid",
                    agent_id,
                    issue_id,
                    created_by_user_id,
                    title,
                    external_agent_id,
                    created_at          AS "created_at!: DateTime<Utc>",
                    updated_at          AS "updated_at!: DateTime<Utc>"
                FROM copilot_sessions
                WHERE project_id = $1
                ORDER BY updated_at DESC
                "#,
                project_id
            )
            .fetch_all(pool)
            .await?
        };
        Ok(records)
    }

    pub async fn create_session(
        pool: &PgPool,
        id: Option<Uuid>,
        project_id: Uuid,
        agent_id: Option<Uuid>,
        issue_id: Option<Uuid>,
        created_by_user_id: Option<Uuid>,
        title: Option<String>,
    ) -> Result<MutationResponse<CopilotSession>, CopilotError> {
        let id = id.unwrap_or_else(Uuid::new_v4);
        let mut tx = super::begin_tx(pool).await?;
        let data = sqlx::query_as!(
            CopilotSession,
            r#"
            INSERT INTO copilot_sessions (
                id, project_id, agent_id, issue_id, created_by_user_id, title
            )
            VALUES ($1, $2, $3, $4, $5, $6)
            RETURNING
                id                  AS "id!: Uuid",
                project_id          AS "project_id!: Uuid",
                agent_id,
                issue_id,
                created_by_user_id,
                title,
                external_agent_id,
                created_at          AS "created_at!: DateTime<Utc>",
                updated_at          AS "updated_at!: DateTime<Utc>"
            "#,
            id,
            project_id,
            agent_id,
            issue_id,
            created_by_user_id,
            title
        )
        .fetch_one(&mut *tx)
        .await?;
        let txid = get_txid(&mut *tx).await?;
        tx.commit().await?;
        Ok(MutationResponse { data, txid })
    }

    pub async fn update_session(
        pool: &PgPool,
        id: Uuid,
        title: Option<String>,
        external_agent_id: Option<Option<String>>,
    ) -> Result<MutationResponse<CopilotSession>, CopilotError> {
        let mut tx = super::begin_tx(pool).await?;
        let clear_ext = matches!(external_agent_id, Some(None));
        let set_ext = external_agent_id.flatten();
        let data = sqlx::query_as!(
            CopilotSession,
            r#"
            UPDATE copilot_sessions
            SET
                title = COALESCE($2, title),
                external_agent_id = CASE
                    WHEN $3 THEN NULL
                    WHEN $4::text IS NOT NULL THEN $4
                    ELSE external_agent_id
                END,
                updated_at = NOW()
            WHERE id = $1
            RETURNING
                id                  AS "id!: Uuid",
                project_id          AS "project_id!: Uuid",
                agent_id,
                issue_id,
                created_by_user_id,
                title,
                external_agent_id,
                created_at          AS "created_at!: DateTime<Utc>",
                updated_at          AS "updated_at!: DateTime<Utc>"
            "#,
            id,
            title,
            clear_ext,
            set_ext
        )
        .fetch_one(&mut *tx)
        .await?;
        let txid = get_txid(&mut *tx).await?;
        tx.commit().await?;
        Ok(MutationResponse { data, txid })
    }

    pub async fn delete_session(pool: &PgPool, id: Uuid) -> Result<DeleteResponse, CopilotError> {
        let mut tx = super::begin_tx(pool).await?;
        sqlx::query!("DELETE FROM copilot_sessions WHERE id = $1", id)
            .execute(&mut *tx)
            .await?;
        let txid = get_txid(&mut *tx).await?;
        tx.commit().await?;
        Ok(DeleteResponse { txid })
    }

    pub async fn list_messages(
        pool: &PgPool,
        session_id: Uuid,
    ) -> Result<Vec<CopilotMessage>, CopilotError> {
        let records = sqlx::query_as!(
            CopilotMessage,
            r#"
            SELECT
                id          AS "id!: Uuid",
                session_id  AS "session_id!: Uuid",
                role        AS "role!",
                content     AS "content!",
                created_at  AS "created_at!: DateTime<Utc>"
            FROM copilot_messages
            WHERE session_id = $1
            ORDER BY created_at ASC
            "#,
            session_id
        )
        .fetch_all(pool)
        .await?;
        Ok(records)
    }

    pub async fn create_message(
        pool: &PgPool,
        id: Option<Uuid>,
        session_id: Uuid,
        role: String,
        content: String,
    ) -> Result<MutationResponse<CopilotMessage>, CopilotError> {
        let id = id.unwrap_or_else(Uuid::new_v4);
        let mut tx = super::begin_tx(pool).await?;
        let data = sqlx::query_as!(
            CopilotMessage,
            r#"
            INSERT INTO copilot_messages (id, session_id, role, content)
            VALUES ($1, $2, $3, $4)
            RETURNING
                id          AS "id!: Uuid",
                session_id  AS "session_id!: Uuid",
                role        AS "role!",
                content     AS "content!",
                created_at  AS "created_at!: DateTime<Utc>"
            "#,
            id,
            session_id,
            role,
            content
        )
        .fetch_one(&mut *tx)
        .await?;

        sqlx::query!(
            r#"UPDATE copilot_sessions SET updated_at = NOW() WHERE id = $1"#,
            session_id
        )
        .execute(&mut *tx)
        .await?;

        let txid = get_txid(&mut *tx).await?;
        tx.commit().await?;
        Ok(MutationResponse { data, txid })
    }

    pub async fn get_llm_settings(
        pool: &PgPool,
        agent_id: Uuid,
    ) -> Result<Option<AgentLlmSettings>, CopilotError> {
        let row = sqlx::query!(
            r#"
            SELECT
                agent_id AS "agent_id!: Uuid",
                api_key,
                base_url,
                model_name,
                working_directory,
                updated_at AS "updated_at!: DateTime<Utc>"
            FROM agent_llm_settings
            WHERE agent_id = $1
            "#,
            agent_id
        )
        .fetch_optional(pool)
        .await?;

        Ok(row.map(|r| AgentLlmSettings {
            agent_id: r.agent_id,
            has_api_key: r.api_key.as_ref().is_some_and(|k| !k.is_empty()),
            base_url: r.base_url,
            model_name: r.model_name,
            working_directory: r.working_directory,
            updated_at: r.updated_at,
        }))
    }

    pub async fn get_llm_settings_secret(
        pool: &PgPool,
        agent_id: Uuid,
    ) -> Result<Option<AgentLlmSettingsSecret>, CopilotError> {
        let row = sqlx::query!(
            r#"
            SELECT
                agent_id AS "agent_id!: Uuid",
                api_key,
                base_url,
                model_name,
                working_directory
            FROM agent_llm_settings
            WHERE agent_id = $1
            "#,
            agent_id
        )
        .fetch_optional(pool)
        .await?;

        Ok(row.map(|r| AgentLlmSettingsSecret {
            agent_id: r.agent_id,
            api_key: r.api_key,
            base_url: r.base_url,
            model_name: r.model_name,
            working_directory: r.working_directory,
        }))
    }

    pub async fn upsert_llm_settings(
        pool: &PgPool,
        agent_id: Uuid,
        api_key: Option<String>,
        base_url: Option<String>,
        model_name: Option<String>,
        working_directory: Option<String>,
        update_api_key: bool,
        update_working_directory: bool,
    ) -> Result<AgentLlmSettings, CopilotError> {
        let row = sqlx::query!(
            r#"
            INSERT INTO agent_llm_settings (
                agent_id, api_key, base_url, model_name, working_directory
            )
            VALUES ($1, $2, $3, $4, $5)
            ON CONFLICT (agent_id) DO UPDATE SET
                api_key = CASE WHEN $6 THEN EXCLUDED.api_key ELSE agent_llm_settings.api_key END,
                base_url = COALESCE(EXCLUDED.base_url, agent_llm_settings.base_url),
                model_name = COALESCE(EXCLUDED.model_name, agent_llm_settings.model_name),
                working_directory = CASE
                    WHEN $7 THEN EXCLUDED.working_directory
                    ELSE agent_llm_settings.working_directory
                END,
                updated_at = NOW()
            RETURNING
                agent_id AS "agent_id!: Uuid",
                api_key,
                base_url,
                model_name,
                working_directory,
                updated_at AS "updated_at!: DateTime<Utc>"
            "#,
            agent_id,
            api_key,
            base_url,
            model_name,
            working_directory,
            update_api_key,
            update_working_directory
        )
        .fetch_one(pool)
        .await?;

        Ok(AgentLlmSettings {
            agent_id: row.agent_id,
            has_api_key: row.api_key.as_ref().is_some_and(|k| !k.is_empty()),
            base_url: row.base_url,
            model_name: row.model_name,
            working_directory: row.working_directory,
            updated_at: row.updated_at,
        })
    }
}
