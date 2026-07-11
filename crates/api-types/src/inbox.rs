use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use ts_rs::TS;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct InboxItem {
    pub id: Uuid,
    pub recipient_user_id: Uuid,
    pub project_id: Option<Uuid>,
    pub issue_id: Option<Uuid>,
    #[serde(rename = "type")]
    #[ts(rename = "type")]
    pub item_type: String,
    pub title: String,
    pub body: String,
    pub payload: Value,
    pub read_at: Option<DateTime<Utc>>,
    pub archived_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct ListInboxResponse {
    pub items: Vec<InboxItem>,
    pub unread_count: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct InboxUnreadCountResponse {
    pub unread_count: i64,
}
