use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use ts_rs::TS;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct WebhookEndpoint {
    pub id: Uuid,
    pub project_id: Uuid,
    pub autopilot_id: Option<Uuid>,
    pub token: String,
    pub name: String,
    pub enabled: bool,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct WebhookDelivery {
    pub id: Uuid,
    pub webhook_endpoint_id: Uuid,
    pub dedupe_key: Option<String>,
    pub status: String,
    pub request_body: String,
    pub response_summary: Option<String>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct CreateWebhookEndpointRequest {
    #[ts(optional)]
    pub id: Option<Uuid>,
    pub project_id: Uuid,
    pub name: String,
    #[ts(optional)]
    pub autopilot_id: Option<Uuid>,
    #[ts(optional)]
    pub signing_secret: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ListWebhookEndpointsQuery {
    pub project_id: Uuid,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct ListWebhookEndpointsResponse {
    pub endpoints: Vec<WebhookEndpoint>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, TS)]
pub struct WebhookIngressPayload {
    #[ts(optional)]
    pub title: Option<String>,
    #[ts(optional)]
    pub body: Option<String>,
    #[ts(optional)]
    pub dedupe_key: Option<String>,
}
