use db::models::{
    scratch::DraftFollowUpData,
    session_queued_message::{SessionQueuedMessage, SessionQueuedMessageError},
};
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;
use ts_rs::TS;
use uuid::Uuid;

/// API-facing queued message (stable id + follow-up payload).
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct QueuedMessage {
    pub id: Uuid,
    pub session_id: Uuid,
    pub data: DraftFollowUpData,
    pub queued_at: chrono::DateTime<chrono::Utc>,
}

impl From<SessionQueuedMessage> for QueuedMessage {
    fn from(value: SessionQueuedMessage) -> Self {
        Self {
            id: value.id,
            session_id: value.session_id,
            data: value.data,
            queued_at: value.queued_at,
        }
    }
}

/// Status of the queue for a session (for frontend display)
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum QueueStatus {
    Empty,
    Queued { messages: Vec<QueuedMessage> },
}

impl QueueStatus {
    pub fn from_messages(messages: Vec<QueuedMessage>) -> Self {
        if messages.is_empty() {
            Self::Empty
        } else {
            Self::Queued { messages }
        }
    }
}

/// DB-backed service for managing queued follow-up messages (ordered list per session).
#[derive(Clone)]
pub struct QueuedMessageService {
    pool: SqlitePool,
}

impl QueuedMessageService {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    pub async fn list(
        &self,
        session_id: Uuid,
    ) -> Result<Vec<QueuedMessage>, SessionQueuedMessageError> {
        let messages = SessionQueuedMessage::list_by_session(&self.pool, session_id).await?;
        Ok(messages.into_iter().map(QueuedMessage::from).collect())
    }

    pub async fn enqueue(
        &self,
        session_id: Uuid,
        data: DraftFollowUpData,
    ) -> Result<QueuedMessage, SessionQueuedMessageError> {
        let queued = SessionQueuedMessage::enqueue(&self.pool, session_id, &data).await?;
        Ok(QueuedMessage::from(queued))
    }

    pub async fn update(
        &self,
        session_id: Uuid,
        item_id: Uuid,
        data: DraftFollowUpData,
    ) -> Result<QueuedMessage, SessionQueuedMessageError> {
        let queued = SessionQueuedMessage::update(&self.pool, session_id, item_id, &data).await?;
        Ok(QueuedMessage::from(queued))
    }

    pub async fn remove(
        &self,
        session_id: Uuid,
        item_id: Uuid,
    ) -> Result<(), SessionQueuedMessageError> {
        SessionQueuedMessage::remove(&self.pool, session_id, item_id).await
    }

    pub async fn clear(&self, session_id: Uuid) -> Result<(), SessionQueuedMessageError> {
        SessionQueuedMessage::clear(&self.pool, session_id).await
    }

    pub async fn reorder(
        &self,
        session_id: Uuid,
        ordered_ids: Vec<Uuid>,
    ) -> Result<Vec<QueuedMessage>, SessionQueuedMessageError> {
        let messages = SessionQueuedMessage::reorder(&self.pool, session_id, &ordered_ids).await?;
        Ok(messages.into_iter().map(QueuedMessage::from).collect())
    }

    pub async fn pop_front(
        &self,
        session_id: Uuid,
    ) -> Result<Option<QueuedMessage>, SessionQueuedMessageError> {
        let queued = SessionQueuedMessage::pop_front(&self.pool, session_id).await?;
        Ok(queued.map(QueuedMessage::from))
    }

    pub async fn has_queued(&self, session_id: Uuid) -> Result<bool, SessionQueuedMessageError> {
        SessionQueuedMessage::has_any(&self.pool, session_id).await
    }

    pub async fn get_status(
        &self,
        session_id: Uuid,
    ) -> Result<QueueStatus, SessionQueuedMessageError> {
        let messages = self.list(session_id).await?;
        Ok(QueueStatus::from_messages(messages))
    }
}
