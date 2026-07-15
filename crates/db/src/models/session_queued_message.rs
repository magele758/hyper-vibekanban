use chrono::{DateTime, Utc};
use executors::profile::ExecutorConfig;
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, SqlitePool};
use thiserror::Error;
use ts_rs::TS;
use uuid::Uuid;

use super::scratch::DraftFollowUpData;

#[derive(Debug, Error)]
pub enum SessionQueuedMessageError {
    #[error(transparent)]
    Database(#[from] sqlx::Error),
    #[error(transparent)]
    Serde(#[from] serde_json::Error),
    #[error("Queued message not found")]
    NotFound,
    #[error("Queued message does not belong to this session")]
    SessionMismatch,
    #[error("Invalid reorder: item ids must match the session queue exactly")]
    InvalidReorder,
}

/// Persisted queued follow-up message for a session.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct SessionQueuedMessage {
    pub id: Uuid,
    pub session_id: Uuid,
    pub position: i64,
    pub data: DraftFollowUpData,
    pub queued_at: DateTime<Utc>,
}

#[derive(Debug, Clone, FromRow)]
struct SessionQueuedMessageRow {
    id: Uuid,
    session_id: Uuid,
    position: i64,
    message: String,
    executor_config: String,
    queued_at: DateTime<Utc>,
}

impl TryFrom<SessionQueuedMessageRow> for SessionQueuedMessage {
    type Error = SessionQueuedMessageError;

    fn try_from(row: SessionQueuedMessageRow) -> Result<Self, Self::Error> {
        let executor_config: ExecutorConfig = serde_json::from_str(&row.executor_config)?;
        Ok(Self {
            id: row.id,
            session_id: row.session_id,
            position: row.position,
            data: DraftFollowUpData {
                message: row.message,
                executor_config,
            },
            queued_at: row.queued_at,
        })
    }
}

impl SessionQueuedMessage {
    pub async fn list_by_session(
        pool: &SqlitePool,
        session_id: Uuid,
    ) -> Result<Vec<Self>, SessionQueuedMessageError> {
        let rows = sqlx::query_as!(
            SessionQueuedMessageRow,
            r#"SELECT id AS "id!: Uuid",
                      session_id AS "session_id!: Uuid",
                      position AS "position!: i64",
                      message,
                      executor_config,
                      queued_at AS "queued_at!: DateTime<Utc>"
               FROM session_queued_messages
               WHERE session_id = $1
               ORDER BY position ASC, queued_at ASC"#,
            session_id
        )
        .fetch_all(pool)
        .await?;

        rows.into_iter().map(Self::try_from).collect()
    }

    pub async fn find_by_id(
        pool: &SqlitePool,
        id: Uuid,
    ) -> Result<Option<Self>, SessionQueuedMessageError> {
        let row = sqlx::query_as!(
            SessionQueuedMessageRow,
            r#"SELECT id AS "id!: Uuid",
                      session_id AS "session_id!: Uuid",
                      position AS "position!: i64",
                      message,
                      executor_config,
                      queued_at AS "queued_at!: DateTime<Utc>"
               FROM session_queued_messages
               WHERE id = $1"#,
            id
        )
        .fetch_optional(pool)
        .await?;

        row.map(Self::try_from).transpose()
    }

    pub async fn enqueue(
        pool: &SqlitePool,
        session_id: Uuid,
        data: &DraftFollowUpData,
    ) -> Result<Self, SessionQueuedMessageError> {
        let id = Uuid::new_v4();
        let executor_config = serde_json::to_string(&data.executor_config)?;
        let next_position: i64 = sqlx::query_scalar!(
            r#"SELECT COALESCE(MAX(position), -1) + 1 AS "position!: i64"
               FROM session_queued_messages
               WHERE session_id = $1"#,
            session_id
        )
        .fetch_one(pool)
        .await?;

        let row = sqlx::query_as!(
            SessionQueuedMessageRow,
            r#"INSERT INTO session_queued_messages (id, session_id, position, message, executor_config)
               VALUES ($1, $2, $3, $4, $5)
               RETURNING id AS "id!: Uuid",
                         session_id AS "session_id!: Uuid",
                         position AS "position!: i64",
                         message,
                         executor_config,
                         queued_at AS "queued_at!: DateTime<Utc>""#,
            id,
            session_id,
            next_position,
            data.message,
            executor_config
        )
        .fetch_one(pool)
        .await?;

        Self::try_from(row)
    }

    pub async fn update(
        pool: &SqlitePool,
        session_id: Uuid,
        id: Uuid,
        data: &DraftFollowUpData,
    ) -> Result<Self, SessionQueuedMessageError> {
        let existing = Self::find_by_id(pool, id)
            .await?
            .ok_or(SessionQueuedMessageError::NotFound)?;
        if existing.session_id != session_id {
            return Err(SessionQueuedMessageError::SessionMismatch);
        }

        let executor_config = serde_json::to_string(&data.executor_config)?;
        let row = sqlx::query_as!(
            SessionQueuedMessageRow,
            r#"UPDATE session_queued_messages
               SET message = $1, executor_config = $2
               WHERE id = $3
               RETURNING id AS "id!: Uuid",
                         session_id AS "session_id!: Uuid",
                         position AS "position!: i64",
                         message,
                         executor_config,
                         queued_at AS "queued_at!: DateTime<Utc>""#,
            data.message,
            executor_config,
            id
        )
        .fetch_one(pool)
        .await?;

        Self::try_from(row)
    }

    pub async fn remove(
        pool: &SqlitePool,
        session_id: Uuid,
        id: Uuid,
    ) -> Result<(), SessionQueuedMessageError> {
        let existing = Self::find_by_id(pool, id)
            .await?
            .ok_or(SessionQueuedMessageError::NotFound)?;
        if existing.session_id != session_id {
            return Err(SessionQueuedMessageError::SessionMismatch);
        }

        let mut tx = pool.begin().await?;
        sqlx::query!(r#"DELETE FROM session_queued_messages WHERE id = $1"#, id)
            .execute(&mut *tx)
            .await?;

        // Compact positions
        let remaining = sqlx::query_as!(
            SessionQueuedMessageRow,
            r#"SELECT id AS "id!: Uuid",
                      session_id AS "session_id!: Uuid",
                      position AS "position!: i64",
                      message,
                      executor_config,
                      queued_at AS "queued_at!: DateTime<Utc>"
               FROM session_queued_messages
               WHERE session_id = $1
               ORDER BY position ASC, queued_at ASC"#,
            session_id
        )
        .fetch_all(&mut *tx)
        .await?;

        for (idx, row) in remaining.iter().enumerate() {
            let position = idx as i64;
            sqlx::query!(
                r#"UPDATE session_queued_messages SET position = $1 WHERE id = $2"#,
                position,
                row.id
            )
            .execute(&mut *tx)
            .await?;
        }

        tx.commit().await?;
        Ok(())
    }

    pub async fn clear(
        pool: &SqlitePool,
        session_id: Uuid,
    ) -> Result<(), SessionQueuedMessageError> {
        sqlx::query!(
            r#"DELETE FROM session_queued_messages WHERE session_id = $1"#,
            session_id
        )
        .execute(pool)
        .await?;
        Ok(())
    }

    pub async fn reorder(
        pool: &SqlitePool,
        session_id: Uuid,
        ordered_ids: &[Uuid],
    ) -> Result<Vec<Self>, SessionQueuedMessageError> {
        let existing = Self::list_by_session(pool, session_id).await?;
        if existing.len() != ordered_ids.len() {
            return Err(SessionQueuedMessageError::InvalidReorder);
        }

        let mut existing_ids: Vec<Uuid> = existing.iter().map(|m| m.id).collect();
        existing_ids.sort();
        let mut requested = ordered_ids.to_vec();
        requested.sort();
        if existing_ids != requested {
            return Err(SessionQueuedMessageError::InvalidReorder);
        }

        let mut tx = pool.begin().await?;
        for (idx, id) in ordered_ids.iter().enumerate() {
            let position = idx as i64;
            sqlx::query!(
                r#"UPDATE session_queued_messages
                   SET position = $1
                   WHERE id = $2 AND session_id = $3"#,
                position,
                id,
                session_id
            )
            .execute(&mut *tx)
            .await?;
        }
        tx.commit().await?;

        Self::list_by_session(pool, session_id).await
    }

    /// Remove and return the front of the queue (lowest position).
    pub async fn pop_front(
        pool: &SqlitePool,
        session_id: Uuid,
    ) -> Result<Option<Self>, SessionQueuedMessageError> {
        let messages = Self::list_by_session(pool, session_id).await?;
        let Some(front) = messages.first().cloned() else {
            return Ok(None);
        };

        Self::remove(pool, session_id, front.id).await?;
        Ok(Some(front))
    }

    pub async fn has_any(
        pool: &SqlitePool,
        session_id: Uuid,
    ) -> Result<bool, SessionQueuedMessageError> {
        let count: i64 = sqlx::query_scalar!(
            r#"SELECT COUNT(*) AS "count!: i64"
               FROM session_queued_messages
               WHERE session_id = $1"#,
            session_id
        )
        .fetch_one(pool)
        .await?;
        Ok(count > 0)
    }
}
