use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use ts_rs::TS;
use uuid::Uuid;

use crate::some_if_present;

/// Public Feishu bot binding (secrets masked).
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct FeishuBotBinding {
    pub id: Uuid,
    pub project_id: Uuid,
    pub agent_id: Uuid,
    pub name: String,
    pub app_id: String,
    /// Always true when a secret is stored; the raw secret is never returned.
    pub has_app_secret: bool,
    pub has_encrypt_key: bool,
    pub has_verification_token: bool,
    pub callback_token: String,
    pub enabled: bool,
    pub reply_on_complete: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct CreateFeishuBotBindingRequest {
    #[ts(optional)]
    pub id: Option<Uuid>,
    pub project_id: Uuid,
    pub agent_id: Uuid,
    #[ts(optional)]
    pub name: Option<String>,
    pub app_id: String,
    pub app_secret: String,
    #[ts(optional)]
    pub encrypt_key: Option<String>,
    #[ts(optional)]
    pub verification_token: Option<String>,
    #[serde(default)]
    #[ts(optional)]
    pub reply_on_complete: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct UpdateFeishuBotBindingRequest {
    #[serde(
        default,
        deserialize_with = "some_if_present",
        skip_serializing_if = "Option::is_none"
    )]
    pub name: Option<String>,
    #[serde(
        default,
        deserialize_with = "some_if_present",
        skip_serializing_if = "Option::is_none"
    )]
    pub agent_id: Option<Uuid>,
    #[serde(
        default,
        deserialize_with = "some_if_present",
        skip_serializing_if = "Option::is_none"
    )]
    pub app_id: Option<String>,
    #[serde(
        default,
        deserialize_with = "some_if_present",
        skip_serializing_if = "Option::is_none"
    )]
    pub app_secret: Option<String>,
    #[serde(
        default,
        deserialize_with = "some_if_present",
        skip_serializing_if = "Option::is_none"
    )]
    pub encrypt_key: Option<Option<String>>,
    #[serde(
        default,
        deserialize_with = "some_if_present",
        skip_serializing_if = "Option::is_none"
    )]
    pub verification_token: Option<Option<String>>,
    #[serde(
        default,
        deserialize_with = "some_if_present",
        skip_serializing_if = "Option::is_none"
    )]
    pub enabled: Option<bool>,
    #[serde(
        default,
        deserialize_with = "some_if_present",
        skip_serializing_if = "Option::is_none"
    )]
    pub reply_on_complete: Option<bool>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ListFeishuBotBindingsQuery {
    pub project_id: Uuid,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct ListFeishuBotBindingsResponse {
    pub bindings: Vec<FeishuBotBinding>,
}
