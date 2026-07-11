use api_types::InboxItem;
use chrono::{DateTime, Utc};
use serde_json::Value;
use sqlx::PgPool;
use thiserror::Error;
use uuid::Uuid;

#[derive(Debug, Error)]
pub enum InboxError {
    #[error("database error: {0}")]
    Database(#[from] sqlx::Error),
}

pub struct InboxRepository;

impl InboxRepository {
    pub async fn list(
        pool: &PgPool,
        user_id: Uuid,
        include_archived: bool,
        limit: i64,
    ) -> Result<Vec<InboxItem>, InboxError> {
        let records = sqlx::query_as!(
            InboxItem,
            r#"
            SELECT
                id                  AS "id!: Uuid",
                recipient_user_id   AS "recipient_user_id!: Uuid",
                project_id,
                issue_id,
                type                AS "item_type!",
                title               AS "title!",
                body                AS "body!",
                payload             AS "payload!: Value",
                read_at,
                archived_at,
                created_at          AS "created_at!: DateTime<Utc>"
            FROM inbox_items
            WHERE recipient_user_id = $1
              AND ($2 OR archived_at IS NULL)
            ORDER BY created_at DESC
            LIMIT $3
            "#,
            user_id,
            include_archived,
            limit
        )
        .fetch_all(pool)
        .await?;

        Ok(records)
    }

    pub async fn unread_count(pool: &PgPool, user_id: Uuid) -> Result<i64, InboxError> {
        let row: (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM inbox_items WHERE recipient_user_id = $1 AND read_at IS NULL AND archived_at IS NULL"
        )
        .bind(user_id)
        .fetch_one(pool)
        .await?;
        Ok(row.0)
    }

    pub async fn mark_read(pool: &PgPool, user_id: Uuid, ids: &[Uuid]) -> Result<u64, InboxError> {
        let result = sqlx::query!(
            r#"
            UPDATE inbox_items
            SET read_at = NOW()
            WHERE recipient_user_id = $1
              AND id = ANY($2)
              AND read_at IS NULL
            "#,
            user_id,
            ids
        )
        .execute(pool)
        .await?;
        Ok(result.rows_affected())
    }

    pub async fn mark_all_read(pool: &PgPool, user_id: Uuid) -> Result<u64, InboxError> {
        let result = sqlx::query!(
            "UPDATE inbox_items SET read_at = NOW() WHERE recipient_user_id = $1 AND read_at IS NULL",
            user_id
        )
        .execute(pool)
        .await?;
        Ok(result.rows_affected())
    }

    pub async fn archive(pool: &PgPool, user_id: Uuid, ids: &[Uuid]) -> Result<u64, InboxError> {
        let result = sqlx::query!(
            r#"
            UPDATE inbox_items
            SET archived_at = NOW(), read_at = COALESCE(read_at, NOW())
            WHERE recipient_user_id = $1
              AND id = ANY($2)
              AND archived_at IS NULL
            "#,
            user_id,
            ids
        )
        .execute(pool)
        .await?;
        Ok(result.rows_affected())
    }

    /// Insert an inbox notification.
    pub async fn create(
        pool: &PgPool,
        recipient_user_id: Uuid,
        project_id: Option<Uuid>,
        issue_id: Option<Uuid>,
        item_type: &str,
        title: &str,
        body: &str,
        payload: Value,
    ) -> Result<InboxItem, InboxError> {
        let record = sqlx::query_as!(
            InboxItem,
            r#"
            INSERT INTO inbox_items (
                recipient_user_id, project_id, issue_id, type, title, body, payload
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7)
            RETURNING
                id                  AS "id!: Uuid",
                recipient_user_id   AS "recipient_user_id!: Uuid",
                project_id,
                issue_id,
                type                AS "item_type!",
                title               AS "title!",
                body                AS "body!",
                payload             AS "payload!: Value",
                read_at,
                archived_at,
                created_at          AS "created_at!: DateTime<Utc>"
            "#,
            recipient_user_id,
            project_id,
            issue_id,
            item_type,
            title,
            body,
            payload
        )
        .fetch_one(pool)
        .await?;

        Ok(record)
    }
}
