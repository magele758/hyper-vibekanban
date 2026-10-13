use sqlx::PgPool;
use uuid::Uuid;

use super::identity_errors::IdentityError;

/// One row from the shared `hosts` table.
///
/// Registration, display name, and online/offline live here so every Relay
/// replica can read them. The yamux socket does not.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HostRecord {
    pub id: Uuid,
    pub owner_user_id: Uuid,
    pub machine_id: String,
    pub name: String,
    pub status: String,
}

pub struct HostRepository<'a> {
    pool: &'a PgPool,
}

impl<'a> HostRepository<'a> {
    pub fn new(pool: &'a PgPool) -> Self {
        Self { pool }
    }

    /// Find or create a host for the given user and machine identity.
    /// If a host with the same owner and machine_id exists, returns it and updates name/version.
    /// Otherwise, creates a new one.
    pub async fn upsert_host(
        &self,
        owner_user_id: Uuid,
        machine_id: &str,
        name: &str,
        agent_version: Option<&str>,
    ) -> Result<Uuid, sqlx::Error> {
        let row = sqlx::query!(
            r#"
            INSERT INTO hosts (
                owner_user_id,
                shared_with_organization_id,
                machine_id,
                name,
                status,
                agent_version
            )
            VALUES ($1, NULL, $2, $3, 'offline', $4)
            ON CONFLICT (owner_user_id, machine_id) DO UPDATE
                SET name = EXCLUDED.name,
                    agent_version = COALESCE(EXCLUDED.agent_version, hosts.agent_version),
                    updated_at = NOW()
            RETURNING id AS "id!: Uuid"
            "#,
            owner_user_id,
            machine_id,
            name,
            agent_version
        )
        .fetch_one(self.pool)
        .await?;

        Ok(row.id)
    }

    /// Every host row, including offline ones. Replicas load this at startup.
    pub async fn list_hosts(&self) -> Result<Vec<HostRecord>, sqlx::Error> {
        sqlx::query_as!(
            HostRecord,
            r#"
            SELECT
                id AS "id!: Uuid",
                owner_user_id AS "owner_user_id!: Uuid",
                machine_id AS "machine_id!",
                name,
                status
            FROM hosts
            ORDER BY updated_at DESC, id ASC
            "#,
        )
        .fetch_all(self.pool)
        .await
    }

    pub async fn assert_host_access(
        &self,
        host_id: Uuid,
        user_id: Uuid,
    ) -> Result<(), IdentityError> {
        let row = sqlx::query!(
            r#"
            SELECT EXISTS (
                SELECT 1
                FROM hosts h
                LEFT JOIN organization_member_metadata om
                    ON om.organization_id = h.shared_with_organization_id
                    AND om.user_id = $2
                WHERE h.id = $1
                  AND (h.owner_user_id = $2 OR om.user_id IS NOT NULL)
            ) AS "allowed!"
            "#,
            host_id,
            user_id
        )
        .fetch_one(self.pool)
        .await?;

        if row.allowed {
            Ok(())
        } else {
            Err(IdentityError::PermissionDenied)
        }
    }

    pub async fn is_host_online(&self, host_id: Uuid) -> Result<bool, sqlx::Error> {
        let row = sqlx::query!(r#"SELECT status FROM hosts WHERE id = $1"#, host_id)
            .fetch_optional(self.pool)
            .await?;
        Ok(row.map(|r| r.status == "online").unwrap_or(false))
    }

    /// Mark the host online and record which connection did it.
    ///
    /// `connection_token` replaces any previous token. A later offline update
    /// from an older connection will not match and will leave this row online.
    pub async fn mark_host_online(
        &self,
        host_id: Uuid,
        agent_version: Option<&str>,
        connection_token: Uuid,
    ) -> Result<(), sqlx::Error> {
        sqlx::query!(
            r#"
            UPDATE hosts
            SET status = 'online',
                last_seen_at = NOW(),
                agent_version = COALESCE($2, agent_version),
                relay_connection_token = $3,
                updated_at = NOW()
            WHERE id = $1
            "#,
            host_id,
            agent_version,
            connection_token
        )
        .execute(self.pool)
        .await?;
        Ok(())
    }

    /// Set the host offline only if `connection_token` is still current.
    ///
    /// Returns whether this call changed the row. A mismatch means another
    /// replica has a newer session; the shared status stays online.
    pub async fn mark_host_offline(
        &self,
        host_id: Uuid,
        connection_token: Uuid,
    ) -> Result<bool, sqlx::Error> {
        let result = sqlx::query!(
            r#"
            UPDATE hosts
            SET status = 'offline',
                relay_connection_token = NULL,
                updated_at = NOW()
            WHERE id = $1
              AND relay_connection_token = $2
            "#,
            host_id,
            connection_token
        )
        .execute(self.pool)
        .await?;
        Ok(result.rows_affected() > 0)
    }
}
