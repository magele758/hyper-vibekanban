use chrono::{DateTime, Utc};
use serde_json::Value;
use sqlx::{PgPool, Row as _};
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct HostLinkRecord {
    pub host_id: Uuid,
    pub host_name: Option<String>,
    pub client_id: Uuid,
    pub client_name: Option<String>,
    pub public_key_b64: String,
    pub private_key_jwk: Value,
    pub server_public_key_b64: String,
    pub signing_session_id: Option<Uuid>,
    pub paired_at: DateTime<Utc>,
}

pub struct HostLinkRepository<'a> {
    pool: &'a PgPool,
}

impl<'a> HostLinkRepository<'a> {
    pub fn new(pool: &'a PgPool) -> Self {
        Self { pool }
    }

    pub async fn owner_of(&self, host_id: Uuid) -> Result<Option<Uuid>, sqlx::Error> {
        sqlx::query_scalar("SELECT owner_user_id FROM hosts WHERE id = $1")
            .bind(host_id)
            .fetch_optional(self.pool)
            .await
    }

    pub async fn upsert(
        &self,
        owner_user_id: Uuid,
        record: &HostLinkRecord,
    ) -> Result<(), sqlx::Error> {
        sqlx::query(
            r#"
            INSERT INTO host_links (
                host_id,
                owner_user_id,
                host_name,
                client_id,
                client_name,
                public_key_b64,
                private_key_jwk,
                server_public_key_b64,
                signing_session_id,
                paired_at
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, NOW())
            ON CONFLICT (host_id) DO UPDATE
                SET owner_user_id = EXCLUDED.owner_user_id,
                    host_name = EXCLUDED.host_name,
                    client_id = EXCLUDED.client_id,
                    client_name = EXCLUDED.client_name,
                    public_key_b64 = EXCLUDED.public_key_b64,
                    private_key_jwk = EXCLUDED.private_key_jwk,
                    server_public_key_b64 = EXCLUDED.server_public_key_b64,
                    signing_session_id = EXCLUDED.signing_session_id,
                    paired_at = NOW()
            "#,
        )
        .bind(record.host_id)
        .bind(owner_user_id)
        .bind(&record.host_name)
        .bind(record.client_id)
        .bind(&record.client_name)
        .bind(&record.public_key_b64)
        .bind(&record.private_key_jwk)
        .bind(&record.server_public_key_b64)
        .bind(record.signing_session_id)
        .execute(self.pool)
        .await?;
        Ok(())
    }

    pub async fn list_for_owner(
        &self,
        owner_user_id: Uuid,
    ) -> Result<Vec<HostLinkRecord>, sqlx::Error> {
        let rows = sqlx::query(
            r#"
            SELECT
                host_id,
                host_name,
                client_id,
                client_name,
                public_key_b64,
                private_key_jwk,
                server_public_key_b64,
                signing_session_id,
                paired_at
            FROM host_links
            WHERE owner_user_id = $1
            ORDER BY paired_at DESC
            "#,
        )
        .bind(owner_user_id)
        .fetch_all(self.pool)
        .await?;

        rows.iter()
            .map(|row| {
                Ok(HostLinkRecord {
                    host_id: row.try_get("host_id")?,
                    host_name: row.try_get("host_name")?,
                    client_id: row.try_get("client_id")?,
                    client_name: row.try_get("client_name")?,
                    public_key_b64: row.try_get("public_key_b64")?,
                    private_key_jwk: row.try_get("private_key_jwk")?,
                    server_public_key_b64: row.try_get("server_public_key_b64")?,
                    signing_session_id: row.try_get("signing_session_id")?,
                    paired_at: row.try_get("paired_at")?,
                })
            })
            .collect()
    }
}
