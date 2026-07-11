use api_types::{DeleteResponse, WebhookDelivery, WebhookEndpoint};
use chrono::{DateTime, Utc};
use sqlx::PgPool;
use thiserror::Error;
use uuid::Uuid;

use super::get_txid;

#[derive(Debug, Error)]
pub enum WebhookError {
    #[error("database error: {0}")]
    Database(#[from] sqlx::Error),
}

pub struct WebhookRepository;

impl WebhookRepository {
    pub async fn find_by_id(
        pool: &PgPool,
        id: Uuid,
    ) -> Result<Option<WebhookEndpoint>, WebhookError> {
        let record = sqlx::query_as!(
            WebhookEndpoint,
            r#"
            SELECT
                id              AS "id!: Uuid",
                project_id      AS "project_id!: Uuid",
                autopilot_id,
                token           AS "token!",
                name            AS "name!",
                enabled         AS "enabled!",
                created_at      AS "created_at!: DateTime<Utc>"
            FROM webhook_endpoints
            WHERE id = $1
            "#,
            id
        )
        .fetch_optional(pool)
        .await?;

        Ok(record)
    }

    pub async fn find_by_token(
        pool: &PgPool,
        token: &str,
    ) -> Result<Option<WebhookEndpointFull>, WebhookError> {
        let record = sqlx::query_as!(
            WebhookEndpointFull,
            r#"
            SELECT
                id              AS "id!: Uuid",
                project_id      AS "project_id!: Uuid",
                autopilot_id,
                token           AS "token!",
                signing_secret,
                name            AS "name!",
                enabled         AS "enabled!",
                created_at      AS "created_at!: DateTime<Utc>"
            FROM webhook_endpoints
            WHERE token = $1
            "#,
            token
        )
        .fetch_optional(pool)
        .await?;

        Ok(record)
    }

    pub async fn list_by_project(
        pool: &PgPool,
        project_id: Uuid,
    ) -> Result<Vec<WebhookEndpoint>, WebhookError> {
        let records = sqlx::query_as!(
            WebhookEndpoint,
            r#"
            SELECT
                id              AS "id!: Uuid",
                project_id      AS "project_id!: Uuid",
                autopilot_id,
                token           AS "token!",
                name            AS "name!",
                enabled         AS "enabled!",
                created_at      AS "created_at!: DateTime<Utc>"
            FROM webhook_endpoints
            WHERE project_id = $1
            ORDER BY created_at DESC
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
        autopilot_id: Option<Uuid>,
        signing_secret: Option<String>,
    ) -> Result<WebhookEndpoint, WebhookError> {
        let id = id.unwrap_or_else(Uuid::new_v4);

        let record = sqlx::query_as!(
            WebhookEndpoint,
            r#"
            INSERT INTO webhook_endpoints (id, project_id, name, autopilot_id, signing_secret)
            VALUES ($1, $2, $3, $4, $5)
            RETURNING
                id              AS "id!: Uuid",
                project_id      AS "project_id!: Uuid",
                autopilot_id,
                token           AS "token!",
                name            AS "name!",
                enabled         AS "enabled!",
                created_at      AS "created_at!: DateTime<Utc>"
            "#,
            id,
            project_id,
            name,
            autopilot_id,
            signing_secret
        )
        .fetch_one(pool)
        .await?;

        Ok(record)
    }

    pub async fn delete(pool: &PgPool, id: Uuid) -> Result<DeleteResponse, WebhookError> {
        let mut tx = super::begin_tx(pool).await?;
        sqlx::query!("DELETE FROM webhook_endpoints WHERE id = $1", id)
            .execute(&mut *tx)
            .await?;
        let txid = get_txid(&mut *tx).await?;
        tx.commit().await?;
        Ok(DeleteResponse { txid })
    }

    pub async fn rotate_token(
        pool: &PgPool,
        id: Uuid,
    ) -> Result<WebhookEndpoint, WebhookError> {
        let new_token = Uuid::new_v4().to_string().replace('-', "");
        let record = sqlx::query_as!(
            WebhookEndpoint,
            r#"
            UPDATE webhook_endpoints
            SET token = $2
            WHERE id = $1
            RETURNING
                id              AS "id!: Uuid",
                project_id      AS "project_id!: Uuid",
                autopilot_id,
                token           AS "token!",
                name            AS "name!",
                enabled         AS "enabled!",
                created_at      AS "created_at!: DateTime<Utc>"
            "#,
            id,
            new_token
        )
        .fetch_one(pool)
        .await?;
        Ok(record)
    }

    pub async fn update_signing_secret(
        pool: &PgPool,
        id: Uuid,
        signing_secret: Option<String>,
    ) -> Result<(), WebhookError> {
        sqlx::query!(
            "UPDATE webhook_endpoints SET signing_secret = $2 WHERE id = $1",
            id,
            signing_secret
        )
        .execute(pool)
        .await?;
        Ok(())
    }

    pub async fn record_delivery(
        pool: &PgPool,
        endpoint_id: Uuid,
        dedupe_key: Option<String>,
        request_body: String,
        response_summary: Option<String>,
        status: &str,
    ) -> Result<WebhookDelivery, WebhookError> {
        let record = sqlx::query_as!(
            WebhookDelivery,
            r#"
            INSERT INTO webhook_deliveries (
                webhook_endpoint_id, dedupe_key, request_body, response_summary, status
            )
            VALUES ($1, $2, $3, $4, $5)
            RETURNING
                id                      AS "id!: Uuid",
                webhook_endpoint_id     AS "webhook_endpoint_id!: Uuid",
                dedupe_key,
                status                  AS "status!",
                request_body            AS "request_body!",
                response_summary,
                created_at              AS "created_at!: DateTime<Utc>"
            "#,
            endpoint_id,
            dedupe_key,
            request_body,
            response_summary,
            status
        )
        .fetch_one(pool)
        .await?;

        Ok(record)
    }
}

/// Full webhook endpoint including signing_secret (server-side only, never returned to clients).
#[derive(Debug, Clone)]
pub struct WebhookEndpointFull {
    pub id: Uuid,
    pub project_id: Uuid,
    pub autopilot_id: Option<Uuid>,
    pub token: String,
    pub signing_secret: Option<String>,
    pub name: String,
    pub enabled: bool,
    pub created_at: DateTime<Utc>,
}
