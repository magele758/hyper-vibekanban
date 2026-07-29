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
        // Position allocation + insert share one transaction so two concurrent
        // enqueues cannot both claim MAX(position)+1.
        let mut tx = pool.begin().await?;
        let next_position: i64 = sqlx::query_scalar!(
            r#"SELECT COALESCE(MAX(position), -1) + 1 AS "position!: i64"
               FROM session_queued_messages
               WHERE session_id = $1"#,
            session_id
        )
        .fetch_one(&mut *tx)
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
        .fetch_one(&mut *tx)
        .await?;

        tx.commit().await?;

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

        Self::compact_positions(&mut tx, session_id).await?;

        tx.commit().await?;
        Ok(())
    }

    /// Renumber a session's queue to 0..n-1 so `position` stays dense.
    async fn compact_positions(
        tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
        session_id: Uuid,
    ) -> Result<(), SessionQueuedMessageError> {
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
        .fetch_all(&mut **tx)
        .await?;

        for (idx, row) in remaining.iter().enumerate() {
            let position = idx as i64;
            sqlx::query!(
                r#"UPDATE session_queued_messages SET position = $1 WHERE id = $2"#,
                position,
                row.id
            )
            .execute(&mut **tx)
            .await?;
        }

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
    ///
    /// Claim + delete happen in one transaction and the DELETE is conditional on
    /// the row still existing, so two concurrent consumers can never execute the
    /// same follow-up twice: the loser sees 0 rows affected and returns `None`.
    pub async fn pop_front(
        pool: &SqlitePool,
        session_id: Uuid,
    ) -> Result<Option<Self>, SessionQueuedMessageError> {
        let mut tx = pool.begin().await?;

        let front = sqlx::query_as!(
            SessionQueuedMessageRow,
            r#"SELECT id AS "id!: Uuid",
                      session_id AS "session_id!: Uuid",
                      position AS "position!: i64",
                      message,
                      executor_config,
                      queued_at AS "queued_at!: DateTime<Utc>"
               FROM session_queued_messages
               WHERE session_id = $1
               ORDER BY position ASC, queued_at ASC
               LIMIT 1"#,
            session_id
        )
        .fetch_optional(&mut *tx)
        .await?;

        let Some(front) = front else {
            return Ok(None);
        };

        let deleted = sqlx::query!(
            r#"DELETE FROM session_queued_messages WHERE id = $1"#,
            front.id
        )
        .execute(&mut *tx)
        .await?
        .rows_affected();

        if deleted == 0 {
            // Another consumer won the race.
            return Ok(None);
        }

        Self::compact_positions(&mut tx, session_id).await?;
        tx.commit().await?;

        Some(Self::try_from(front)).transpose()
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

#[cfg(test)]
mod tests {
    use executors::executors::BaseCodingAgent;
    use sqlx::sqlite::SqlitePoolOptions;

    use super::*;

    /// Minimal schema: the queue table only, so tests stay independent of the
    /// full migration chain (and its `sessions` FK).
    async fn test_pool() -> SqlitePool {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .expect("in-memory sqlite");

        sqlx::query(
            r#"CREATE TABLE session_queued_messages (
                   id BLOB PRIMARY KEY NOT NULL,
                   session_id BLOB NOT NULL,
                   position INTEGER NOT NULL,
                   message TEXT NOT NULL,
                   executor_config TEXT NOT NULL,
                   queued_at TEXT NOT NULL DEFAULT (datetime('now', 'subsec'))
               )"#,
        )
        .execute(&pool)
        .await
        .expect("create table");

        pool
    }

    fn draft(message: &str) -> DraftFollowUpData {
        DraftFollowUpData {
            message: message.to_string(),
            executor_config: ExecutorConfig {
                executor: BaseCodingAgent::ClaudeCode,
                variant: None,
                model_id: None,
                agent_id: None,
                reasoning_id: None,
                permission_policy: None,
            },
        }
    }

    #[tokio::test]
    async fn enqueue_assigns_dense_increasing_positions() {
        let pool = test_pool().await;
        let session = Uuid::new_v4();

        for msg in ["a", "b", "c"] {
            SessionQueuedMessage::enqueue(&pool, session, &draft(msg))
                .await
                .unwrap();
        }

        let all = SessionQueuedMessage::list_by_session(&pool, session)
            .await
            .unwrap();
        assert_eq!(
            all.iter().map(|m| m.position).collect::<Vec<_>>(),
            vec![0, 1, 2]
        );
        assert_eq!(
            all.iter()
                .map(|m| m.data.message.as_str())
                .collect::<Vec<_>>(),
            vec!["a", "b", "c"]
        );
    }

    #[tokio::test]
    async fn pop_front_is_fifo_and_compacts_positions() {
        let pool = test_pool().await;
        let session = Uuid::new_v4();
        for msg in ["first", "second", "third"] {
            SessionQueuedMessage::enqueue(&pool, session, &draft(msg))
                .await
                .unwrap();
        }

        let popped = SessionQueuedMessage::pop_front(&pool, session)
            .await
            .unwrap()
            .expect("queue not empty");
        assert_eq!(popped.data.message, "first");

        let rest = SessionQueuedMessage::list_by_session(&pool, session)
            .await
            .unwrap();
        assert_eq!(
            rest.iter().map(|m| m.position).collect::<Vec<_>>(),
            vec![0, 1],
            "positions must stay dense after a pop"
        );
        assert_eq!(rest[0].data.message, "second");
    }

    #[tokio::test]
    async fn pop_front_on_empty_queue_returns_none() {
        let pool = test_pool().await;
        assert!(
            SessionQueuedMessage::pop_front(&pool, Uuid::new_v4())
                .await
                .unwrap()
                .is_none()
        );
    }

    #[tokio::test]
    async fn concurrent_pop_front_never_yields_the_same_message_twice() {
        let pool = test_pool().await;
        let session = Uuid::new_v4();
        SessionQueuedMessage::enqueue(&pool, session, &draft("only"))
            .await
            .unwrap();

        let (a, b) = tokio::join!(
            SessionQueuedMessage::pop_front(&pool, session),
            SessionQueuedMessage::pop_front(&pool, session),
        );

        let claimed = [a.unwrap(), b.unwrap()]
            .into_iter()
            .flatten()
            .collect::<Vec<_>>();
        assert_eq!(
            claimed.len(),
            1,
            "exactly one caller may claim a queued message"
        );
        assert!(
            !SessionQueuedMessage::has_any(&pool, session).await.unwrap(),
            "queue must be drained"
        );
    }

    #[tokio::test]
    async fn clear_removes_only_the_target_session() {
        let pool = test_pool().await;
        let keep = Uuid::new_v4();
        let drop = Uuid::new_v4();
        SessionQueuedMessage::enqueue(&pool, keep, &draft("keep"))
            .await
            .unwrap();
        SessionQueuedMessage::enqueue(&pool, drop, &draft("drop"))
            .await
            .unwrap();

        SessionQueuedMessage::clear(&pool, drop).await.unwrap();

        assert!(SessionQueuedMessage::has_any(&pool, keep).await.unwrap());
        assert!(!SessionQueuedMessage::has_any(&pool, drop).await.unwrap());
    }

    #[tokio::test]
    async fn remove_rejects_a_foreign_session() {
        let pool = test_pool().await;
        let owner = Uuid::new_v4();
        let queued = SessionQueuedMessage::enqueue(&pool, owner, &draft("mine"))
            .await
            .unwrap();

        let err = SessionQueuedMessage::remove(&pool, Uuid::new_v4(), queued.id)
            .await
            .expect_err("cross-session remove must fail");
        assert!(matches!(err, SessionQueuedMessageError::SessionMismatch));
        assert!(SessionQueuedMessage::has_any(&pool, owner).await.unwrap());
    }
}
