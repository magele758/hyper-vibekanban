use api_types::{DeleteResponse, MutationResponse, Squad, SquadMember};
use chrono::{DateTime, Utc};
use sqlx::PgPool;
use thiserror::Error;
use uuid::Uuid;

use super::get_txid;

#[derive(Debug, Error)]
pub enum SquadError {
    #[error("database error: {0}")]
    Database(#[from] sqlx::Error),
}

pub struct SquadRepository;

impl SquadRepository {
    pub async fn find_by_id(pool: &PgPool, id: Uuid) -> Result<Option<Squad>, SquadError> {
        let record = sqlx::query_as!(
            Squad,
            r#"
            SELECT
                id                  AS "id!: Uuid",
                project_id          AS "project_id!: Uuid",
                name                AS "name!",
                leader_agent_id,
                created_at          AS "created_at!: DateTime<Utc>",
                updated_at          AS "updated_at!: DateTime<Utc>"
            FROM squads
            WHERE id = $1
            "#,
            id
        )
        .fetch_optional(pool)
        .await?;

        Ok(record)
    }

    pub async fn list_by_project(
        pool: &PgPool,
        project_id: Uuid,
    ) -> Result<Vec<Squad>, SquadError> {
        let records = sqlx::query_as!(
            Squad,
            r#"
            SELECT
                id                  AS "id!: Uuid",
                project_id          AS "project_id!: Uuid",
                name                AS "name!",
                leader_agent_id,
                created_at          AS "created_at!: DateTime<Utc>",
                updated_at          AS "updated_at!: DateTime<Utc>"
            FROM squads
            WHERE project_id = $1
            ORDER BY name ASC
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
        leader_agent_id: Option<Uuid>,
    ) -> Result<MutationResponse<Squad>, SquadError> {
        let id = id.unwrap_or_else(Uuid::new_v4);
        let mut tx = super::begin_tx(pool).await?;

        let data = sqlx::query_as!(
            Squad,
            r#"
            INSERT INTO squads (id, project_id, name, leader_agent_id)
            VALUES ($1, $2, $3, $4)
            RETURNING
                id                  AS "id!: Uuid",
                project_id          AS "project_id!: Uuid",
                name                AS "name!",
                leader_agent_id,
                created_at          AS "created_at!: DateTime<Utc>",
                updated_at          AS "updated_at!: DateTime<Utc>"
            "#,
            id,
            project_id,
            name,
            leader_agent_id
        )
        .fetch_one(&mut *tx)
        .await?;

        let txid = get_txid(&mut *tx).await?;
        tx.commit().await?;
        Ok(MutationResponse { data, txid })
    }

    pub async fn update(
        pool: &PgPool,
        id: Uuid,
        name: Option<String>,
        leader_agent_id: Option<Option<Uuid>>,
    ) -> Result<MutationResponse<Squad>, SquadError> {
        let mut tx = super::begin_tx(pool).await?;

        let clear_leader = matches!(leader_agent_id, Some(None));
        let set_leader = leader_agent_id.flatten();

        let data = sqlx::query_as!(
            Squad,
            r#"
            UPDATE squads
            SET
                name = COALESCE($2, name),
                leader_agent_id = CASE
                    WHEN $3 THEN NULL
                    WHEN $4::uuid IS NOT NULL THEN $4
                    ELSE leader_agent_id
                END,
                updated_at = NOW()
            WHERE id = $1
            RETURNING
                id                  AS "id!: Uuid",
                project_id          AS "project_id!: Uuid",
                name                AS "name!",
                leader_agent_id,
                created_at          AS "created_at!: DateTime<Utc>",
                updated_at          AS "updated_at!: DateTime<Utc>"
            "#,
            id,
            name,
            clear_leader,
            set_leader
        )
        .fetch_one(&mut *tx)
        .await?;

        let txid = get_txid(&mut *tx).await?;
        tx.commit().await?;
        Ok(MutationResponse { data, txid })
    }

    pub async fn delete(pool: &PgPool, id: Uuid) -> Result<DeleteResponse, SquadError> {
        let mut tx = super::begin_tx(pool).await?;
        sqlx::query!("DELETE FROM squads WHERE id = $1", id)
            .execute(&mut *tx)
            .await?;
        let txid = get_txid(&mut *tx).await?;
        tx.commit().await?;
        Ok(DeleteResponse { txid })
    }

    // --- Members ---

    pub async fn list_members(
        pool: &PgPool,
        squad_id: Uuid,
    ) -> Result<Vec<SquadMember>, SquadError> {
        let records = sqlx::query_as!(
            SquadMember,
            r#"
            SELECT
                id          AS "id!: Uuid",
                squad_id    AS "squad_id!: Uuid",
                agent_id,
                user_id,
                created_at  AS "created_at!: DateTime<Utc>"
            FROM squad_members
            WHERE squad_id = $1
            ORDER BY created_at ASC
            "#,
            squad_id
        )
        .fetch_all(pool)
        .await?;

        Ok(records)
    }

    pub async fn add_member(
        pool: &PgPool,
        squad_id: Uuid,
        agent_id: Option<Uuid>,
        user_id: Option<Uuid>,
    ) -> Result<MutationResponse<SquadMember>, SquadError> {
        let id = Uuid::new_v4();
        let mut tx = super::begin_tx(pool).await?;

        let data = sqlx::query_as!(
            SquadMember,
            r#"
            INSERT INTO squad_members (id, squad_id, agent_id, user_id)
            VALUES ($1, $2, $3, $4)
            RETURNING
                id          AS "id!: Uuid",
                squad_id    AS "squad_id!: Uuid",
                agent_id,
                user_id,
                created_at  AS "created_at!: DateTime<Utc>"
            "#,
            id,
            squad_id,
            agent_id,
            user_id
        )
        .fetch_one(&mut *tx)
        .await?;

        let txid = get_txid(&mut *tx).await?;
        tx.commit().await?;
        Ok(MutationResponse { data, txid })
    }

    pub async fn remove_member(
        pool: &PgPool,
        member_id: Uuid,
    ) -> Result<DeleteResponse, SquadError> {
        let mut tx = super::begin_tx(pool).await?;
        sqlx::query!("DELETE FROM squad_members WHERE id = $1", member_id)
            .execute(&mut *tx)
            .await?;
        let txid = get_txid(&mut *tx).await?;
        tx.commit().await?;
        Ok(DeleteResponse { txid })
    }
}
